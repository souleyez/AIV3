use anyhow::{anyhow, Result};
use chrono::Utc;
use domain_model::DocumentLifecycle;
use event_bus::{workflow_task_enqueued_subject, EventBus, EventSubscription};
use ingest_worker::{IngestJob, IngestOutcome, IngestProcessor, LocalIngestProcessor};
use serde_json::{json, Value};
use storage::{NewDocumentChunk, PgStorage, DEFAULT_LOCAL_DATABASE_URL};
use tokio::time::Duration;
use workflow_engine::{WorkflowCatalog, WorkflowSignal};

const DEFAULT_QUEUE: &str = "ingest";
const DEFAULT_WAKE_TASK_KEY: &str = "ingest_uploaded_document";
const DEFAULT_POLL_INTERVAL_MS: u64 = 1_000;

#[tokio::main]
async fn main() -> Result<()> {
    observability::install("ingest_worker")?;

    let database_url = std::env::var("PLATFORM_DATABASE_URL")
        .unwrap_or_else(|_| DEFAULT_LOCAL_DATABASE_URL.to_string());
    let queue = std::env::var("INGEST_QUEUE").unwrap_or_else(|_| DEFAULT_QUEUE.to_string());
    let task_key = optional_env("INGEST_TASK_KEY");
    let poll_interval = std::env::var("INGEST_POLL_INTERVAL_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_POLL_INTERVAL_MS);

    let storage = PgStorage::connect(&database_url).await?;
    let workflow_catalog = workflow_definitions::catalog();
    let event_bus = EventBus::connect_from_env_or_disabled("PLATFORM_NATS_URL").await;
    let processor = LocalIngestProcessor;
    let wake_task_key = task_key.as_deref().unwrap_or(DEFAULT_WAKE_TASK_KEY);
    let wake_subject = workflow_task_enqueued_subject(&queue, wake_task_key);
    let mut task_waker = event_bus
        .subscribe_queue_or_disabled(
            &wake_subject,
            Some(&format!("ingest_worker.{queue}.{wake_task_key}")),
        )
        .await;

    tracing::info!(
        %queue,
        task_key = task_key.as_deref().unwrap_or("*"),
        %wake_subject,
        event_bus_enabled = event_bus.is_enabled(),
        poll_interval_ms = poll_interval,
        %database_url,
        "ingest-worker polling started"
    );

    loop {
        match storage
            .workflow_tasks()
            .claim_next_available(&queue, task_key.as_deref(), Utc::now())
            .await
        {
            Ok(Some(task)) => {
                if let Err(error) =
                    process_task(&storage, &workflow_catalog, &event_bus, &processor, task).await
                {
                    tracing::error!(error = ?error, "ingest task processing failed");
                }
            }
            Ok(None) => {
                wait_for_next_task_signal(&mut task_waker, poll_interval).await;
            }
            Err(error) => {
                tracing::error!(error = ?error, "ingest worker failed to claim task");
                wait_for_next_task_signal(&mut task_waker, poll_interval).await;
            }
        }
    }
}

async fn process_task(
    storage: &PgStorage,
    workflow_catalog: &WorkflowCatalog,
    event_bus: &EventBus,
    processor: &impl IngestProcessor,
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
    let document_id = context_uuid_string(&execution.context, "document_id")?
        .map(domain_model::DocumentId)
        .ok_or_else(|| anyhow!("workflow execution {} missing document_id", execution.id))?;
    let document = storage
        .documents()
        .get_by_id(task.tenant_id, document_id)
        .await?
        .ok_or_else(|| anyhow!("document {} not found", document_id))?;

    let job = IngestJob {
        dataset_id,
        document_id,
        title: document.title.clone(),
        object_key: document.object_key.clone(),
        content_type: document.content_type.clone(),
    };
    let current_title = document.title.clone();
    let current_content_type = document.content_type.clone();
    let current_object_key = document.object_key.clone();

    let process_result: Result<()> = async {
        let outcome = processor.process(&job);
        let chunk_count = outcome.chunk_count();
        let chunks = build_document_chunks(dataset_id, document_id, &outcome);
        storage
            .document_chunks()
            .replace_for_document(task.tenant_id, document_id, &chunks)
            .await?;
        let metadata_updates = json!({
            "ingest": {
                "processor": if outcome.used_placeholder { "placeholder" } else { "local_parser" },
                "parse_method": outcome.parse_method.clone(),
                "cloud_structured_provider": if outcome.parse_method.contains("vlm") { "minimax" } else { "" },
                "parse_metadata": outcome.metadata.clone(),
                "chunk_count": chunk_count,
                "extracted_chars": outcome.extracted_chars,
                "content_type": current_content_type,
                "object_key": current_object_key,
                "extracted_at": Utc::now(),
            }
        });
        let updated_document = storage
            .documents()
            .update_state(
                task.tenant_id,
                document_id,
                DocumentLifecycle::Extracted,
                outcome.inferred_title.as_deref(),
                &metadata_updates,
                Utc::now(),
            )
            .await?;
        let signal_output = json!({
            "document_id": updated_document.id,
            "dataset_id": updated_document.dataset_id,
            "content_type": updated_document.content_type,
            "chunk_count": chunk_count,
                "parse_method": outcome.parse_method.clone(),
                "cloud_structured_provider": if outcome.parse_method.contains("vlm") { "minimax" } else { "" },
                "extracted_chars": outcome.extracted_chars,
            "lifecycle": updated_document.lifecycle.as_str(),
            "title": updated_document.title,
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
        let _ = storage
            .documents()
            .update_state(
                task.tenant_id,
                document_id,
                DocumentLifecycle::Failed,
                Some(&current_title),
                &json!({
                    "ingest": {
                        "processor": "placeholder",
                        "failed_at": Utc::now(),
                        "last_error": error_message,
                    }
                }),
                Utc::now(),
            )
            .await;

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
                "ingest worker failed to send workflow step_failed signal"
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
        document_id = %document_id,
        dataset_id = %dataset_id,
        "ingest task completed"
    );

    Ok(())
}

fn build_document_chunks(
    dataset_id: domain_model::DatasetId,
    document_id: domain_model::DocumentId,
    outcome: &IngestOutcome,
) -> Vec<NewDocumentChunk> {
    let created_at = Utc::now();

    outcome
        .chunks
        .iter()
        .enumerate()
        .map(|(index, content)| NewDocumentChunk {
            dataset_id,
            document_id,
            chunk_index: index as i32,
            content: content.clone(),
            token_count: estimate_token_count(content),
            metadata: json!({
                "extractor": if outcome.used_placeholder { "placeholder" } else { "local_parser" },
                "parse_method": outcome.parse_method.clone(),
                "parse_metadata": outcome.metadata.clone(),
                "source": "upload_ingest_workflow",
            }),
            created_at,
        })
        .collect()
}

fn estimate_token_count(content: &str) -> i32 {
    let estimated = (content.chars().count() / 4).max(1);
    estimated.try_into().unwrap_or(i32::MAX)
}

fn context_uuid_string(value: &Value, key: &str) -> Result<Option<uuid::Uuid>> {
    match value {
        Value::Object(map) => match map.get(key).and_then(Value::as_str) {
            Some(raw) => Ok(Some(uuid::Uuid::parse_str(raw)?)),
            None => Ok(None),
        },
        Value::Null => Ok(None),
        _ => Err(anyhow!("workflow execution context must be a JSON object")),
    }
}

fn optional_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && value != "*")
}

async fn wait_for_next_task_signal(task_waker: &mut EventSubscription, poll_interval_ms: u64) {
    if let Some(event) = task_waker
        .wait_for_event(Duration::from_millis(poll_interval_ms))
        .await
    {
        tracing::debug!(subject = %event.subject, "ingest worker received task wake signal");
    }
}
