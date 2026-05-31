use anyhow::{anyhow, Result};
use chrono::Utc;
use domain_model::{DocumentLifecycle, UserId};
use event_bus::{workflow_task_enqueued_subject, EventBus, EventSubscription};
use memory_worker::{
    MemoryIndexer, MemoryRefreshJob, MemorySourceDocument, PlaceholderMemoryIndexer,
};
use serde_json::{json, Value};
use storage::{NewMemoryDirectory, PgStorage, DEFAULT_LOCAL_DATABASE_URL};
use tokio::time::Duration;
use uuid::Uuid;
use workflow_engine::{WorkflowCatalog, WorkflowSignal};

const DEFAULT_QUEUE: &str = "memory";
const DEFAULT_TASK_KEY: &str = "refresh_memory_directory";
const DEFAULT_POLL_INTERVAL_MS: u64 = 1_000;

#[tokio::main]
async fn main() -> Result<()> {
    observability::install("memory_worker")?;

    let database_url = std::env::var("PLATFORM_DATABASE_URL")
        .unwrap_or_else(|_| DEFAULT_LOCAL_DATABASE_URL.to_string());
    let queue = std::env::var("MEMORY_QUEUE").unwrap_or_else(|_| DEFAULT_QUEUE.to_string());
    let task_key =
        std::env::var("MEMORY_TASK_KEY").unwrap_or_else(|_| DEFAULT_TASK_KEY.to_string());
    let poll_interval = std::env::var("MEMORY_POLL_INTERVAL_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_POLL_INTERVAL_MS);

    let storage = PgStorage::connect_with_configured_max_connections(
        &database_url,
        "MEMORY_WORKER_DATABASE_MAX_CONNECTIONS",
    )
    .await?;
    let workflow_catalog = workflow_definitions::catalog();
    let event_bus = EventBus::connect_from_env_or_disabled("PLATFORM_NATS_URL").await;
    let indexer = PlaceholderMemoryIndexer;
    let wake_subject = workflow_task_enqueued_subject(&queue, &task_key);
    let mut task_waker = event_bus
        .subscribe_queue_or_disabled(
            &wake_subject,
            Some(&format!("memory_worker.{queue}.{task_key}")),
        )
        .await;

    tracing::info!(
        %queue,
        %task_key,
        %wake_subject,
        event_bus_enabled = event_bus.is_enabled(),
        poll_interval_ms = poll_interval,
        %database_url,
        "memory-worker polling started"
    );

    loop {
        match storage
            .workflow_tasks()
            .claim_next_available(&queue, Some(&task_key), Utc::now())
            .await
        {
            Ok(Some(task)) => {
                if let Err(error) =
                    process_task(&storage, &workflow_catalog, &event_bus, &indexer, task).await
                {
                    tracing::error!(error = ?error, "memory task processing failed");
                }
            }
            Ok(None) => {
                wait_for_next_task_signal(&mut task_waker, poll_interval).await;
            }
            Err(error) => {
                tracing::error!(error = ?error, "memory worker failed to claim task");
                wait_for_next_task_signal(&mut task_waker, poll_interval).await;
            }
        }
    }
}

async fn process_task(
    storage: &PgStorage,
    workflow_catalog: &WorkflowCatalog,
    event_bus: &EventBus,
    indexer: &impl MemoryIndexer,
    task: domain_model::WorkflowTask,
) -> Result<()> {
    let execution = storage
        .workflow_executions()
        .get_by_id(task.tenant_id, task.execution_id)
        .await?
        .ok_or_else(|| anyhow!("workflow execution {} not found", task.execution_id))?;
    let dataset_id = execution
        .dataset_id
        .ok_or_else(|| anyhow!("workflow execution {} has no dataset id", execution.id))?;
    let include_directory = context_bool(&execution.context, "include_directory").unwrap_or(true);
    let owner_user_id = context_uuid(&execution.context, "owner_user_id").map(UserId);
    let documents = storage
        .documents()
        .list_by_dataset(task.tenant_id, dataset_id)
        .await?;
    let indexed_documents: Vec<_> = documents
        .into_iter()
        .filter(|document| document.lifecycle == DocumentLifecycle::Indexed)
        .filter(|document| {
            document.owner_user_id.is_none() || document.owner_user_id == owner_user_id
        })
        .map(|document| MemorySourceDocument {
            document_id: document.id,
            title: document.title,
            lifecycle: document.lifecycle.as_str().to_string(),
            chunk_count: chunk_count_from_document(&document.metadata),
        })
        .collect();
    let source_document_ids = indexed_documents
        .iter()
        .map(|document| document.document_id)
        .collect::<Vec<_>>();

    let process_result: Result<()> = async {
        let outcome = indexer.refresh(&MemoryRefreshJob {
            dataset_id,
            owner_user_id,
            include_directory,
            documents: indexed_documents,
        });
        let directory = storage
            .memory_directories()
            .create(
                task.tenant_id,
                &NewMemoryDirectory {
                    execution_id: task.execution_id,
                    dataset_id,
                    owner_user_id,
                    source_document_ids,
                    directory_nodes: outcome.directory_nodes as i32,
                    refreshed_chunks: outcome.refreshed_chunks as i32,
                    directory_manifest: outcome.directory_manifest.clone(),
                    created_at: Utc::now(),
                },
            )
            .await?;
        let signal_output = json!({
            "dataset_id": dataset_id,
            "memory_directory_id": directory.id,
            "version_no": directory.version_no,
            "directory_nodes": directory.directory_nodes,
            "refreshed_chunks": directory.refreshed_chunks,
        });

        platform_api::apply_workflow_signal_with_dependencies(
            storage,
            workflow_catalog,
            event_bus,
            task.tenant_id,
            task.execution_id,
            WorkflowSignal::StepCompleted {
                task_key: task.task_key.clone(),
                output: Some(signal_output),
            },
        )
        .await?;

        Ok(())
    }
    .await;

    if let Err(error) = process_result {
        let error_message = error.to_string();
        if let Err(signal_error) = platform_api::apply_workflow_signal_with_dependencies(
            storage,
            workflow_catalog,
            event_bus,
            task.tenant_id,
            task.execution_id,
            WorkflowSignal::StepFailed {
                task_key: task.task_key.clone(),
                error: error_message.clone(),
            },
        )
        .await
        {
            tracing::error!(
                error = ?signal_error,
                task_id = %task.id,
                "memory worker failed to send workflow step_failed signal"
            );
        }

        storage
            .workflow_tasks()
            .mark_failed(task.id, &error_message, Utc::now())
            .await?;
        return Err(error);
    }

    storage
        .workflow_tasks()
        .mark_succeeded(task.id, Utc::now())
        .await?;

    tracing::info!(
        task_id = %task.id,
        execution_id = %task.execution_id,
        dataset_id = %dataset_id,
        "memory task completed"
    );

    Ok(())
}

fn context_bool(value: &Value, key: &str) -> Option<bool> {
    match value {
        Value::Object(map) => map.get(key).and_then(Value::as_bool),
        _ => None,
    }
}

fn context_uuid(value: &Value, key: &str) -> Option<Uuid> {
    match value {
        Value::Object(map) => map
            .get(key)
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok()),
        _ => None,
    }
}

fn chunk_count_from_document(metadata: &std::collections::BTreeMap<String, Value>) -> usize {
    metadata
        .get("ingest")
        .and_then(Value::as_object)
        .and_then(|ingest| ingest.get("chunk_count"))
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize
}

async fn wait_for_next_task_signal(task_waker: &mut EventSubscription, poll_interval_ms: u64) {
    if let Some(event) = task_waker
        .wait_for_event(Duration::from_millis(poll_interval_ms))
        .await
    {
        tracing::debug!(subject = %event.subject, "memory worker received task wake signal");
    }
}
