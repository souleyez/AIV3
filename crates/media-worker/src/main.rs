use anyhow::{anyhow, Result};
use chrono::Utc;
use domain_model::{WorkflowKind, WorkflowTask};
use event_bus::{workflow_task_enqueued_subject, EventBus, EventSubscription};
use media_worker::{
    extract_video_ppt_output_with_frame_extraction, frame_extraction_config_from_env,
    register_video_asset_output, resolve_video_source_output,
    run_video_frame_extraction_if_enabled, FrameExtractionConfig, MediaWorkflowTaskKind,
};
use serde_json::Value;
use storage::{PgStorage, DEFAULT_LOCAL_DATABASE_URL};
use tokio::time::Duration;
use uuid::Uuid;
use workflow_engine::{WorkflowCatalog, WorkflowSignal};

const DEFAULT_QUEUE: &str = "media";
const DEFAULT_WAKE_TASK_KEY: &str = "resolve_video_source";
const DEFAULT_POLL_INTERVAL_MS: u64 = 1_000;

#[tokio::main]
async fn main() -> Result<()> {
    observability::install("media_worker")?;

    let database_url = std::env::var("PLATFORM_DATABASE_URL")
        .unwrap_or_else(|_| DEFAULT_LOCAL_DATABASE_URL.to_string());
    let queue = std::env::var("MEDIA_QUEUE").unwrap_or_else(|_| DEFAULT_QUEUE.to_string());
    let task_key = optional_env("MEDIA_TASK_KEY");
    let poll_interval = std::env::var("MEDIA_POLL_INTERVAL_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_POLL_INTERVAL_MS);

    let storage = PgStorage::connect(&database_url).await?;
    let workflow_catalog = workflow_definitions::catalog();
    let event_bus = EventBus::connect_from_env_or_disabled("PLATFORM_NATS_URL").await;
    let frame_extraction_config = frame_extraction_config_from_env();
    let wake_task_key = task_key.as_deref().unwrap_or(DEFAULT_WAKE_TASK_KEY);
    let wake_subject = workflow_task_enqueued_subject(&queue, wake_task_key);
    let mut task_waker = event_bus
        .subscribe_queue_or_disabled(
            &wake_subject,
            Some(&format!("media_worker.{queue}.{wake_task_key}")),
        )
        .await;

    tracing::info!(
        %queue,
        task_key = task_key.as_deref().unwrap_or("*"),
        %wake_subject,
        event_bus_enabled = event_bus.is_enabled(),
        poll_interval_ms = poll_interval,
        frame_extraction_enabled = frame_extraction_config.enabled,
        frame_extraction_interval_seconds = frame_extraction_config.interval_seconds,
        %database_url,
        "media-worker polling started"
    );

    loop {
        match storage
            .workflow_tasks()
            .claim_next_available(&queue, task_key.as_deref(), Utc::now())
            .await
        {
            Ok(Some(task)) => {
                if let Err(error) = process_task(
                    &storage,
                    &workflow_catalog,
                    &event_bus,
                    &frame_extraction_config,
                    task,
                )
                .await
                {
                    tracing::error!(error = ?error, "media task processing failed");
                }
            }
            Ok(None) => {
                wait_for_next_task_signal(&mut task_waker, poll_interval).await;
            }
            Err(error) => {
                tracing::error!(error = ?error, "media worker failed to claim task");
                wait_for_next_task_signal(&mut task_waker, poll_interval).await;
            }
        }
    }
}

async fn process_task(
    storage: &PgStorage,
    workflow_catalog: &WorkflowCatalog,
    event_bus: &EventBus,
    frame_extraction_config: &FrameExtractionConfig,
    task: WorkflowTask,
) -> Result<()> {
    let process_result: Result<()> = async {
        let execution = storage
            .workflow_executions()
            .get_by_id(task.tenant_id, task.execution_id)
            .await?
            .ok_or_else(|| anyhow!("workflow execution {} not found", task.execution_id))?;
        if execution.kind != WorkflowKind::VideoExtraction {
            return Err(anyhow!(
                "workflow execution {} has unexpected kind {}",
                execution.id,
                execution.kind.as_str()
            ));
        }

        let task_kind = MediaWorkflowTaskKind::from_task_key(&task.task_key)
            .ok_or_else(|| anyhow!("unsupported media task key {}", task.task_key))?;
        let output = match task_kind {
            MediaWorkflowTaskKind::ResolveVideoSource => {
                resolve_video_source_output(&task, &execution.context)
            }
            MediaWorkflowTaskKind::RegisterVideoAsset => {
                let document_id = context_uuid(&execution.context, "document_id")?;
                let document = storage
                    .documents()
                    .get_by_id(task.tenant_id, domain_model::DocumentId(document_id))
                    .await?
                    .ok_or_else(|| anyhow!("document {} not found", document_id))?;
                register_video_asset_output(&document)
            }
            MediaWorkflowTaskKind::ExtractVideoPpt => {
                let document_id = context_uuid(&execution.context, "document_id")?;
                let document_id = domain_model::DocumentId(document_id);
                let document = storage
                    .documents()
                    .get_by_id(task.tenant_id, document_id)
                    .await?
                    .ok_or_else(|| anyhow!("document {} not found", document_id))?;
                let chunks = storage
                    .document_chunks()
                    .list_by_document(task.tenant_id, document_id)
                    .await?;
                let frame_extraction =
                    run_video_frame_extraction_if_enabled(&document, frame_extraction_config);
                extract_video_ppt_output_with_frame_extraction(&document, &chunks, frame_extraction)
            }
        };

        platform_api::apply_workflow_signal_with_dependencies(
            storage,
            workflow_catalog,
            event_bus,
            task.tenant_id,
            task.execution_id,
            WorkflowSignal::StepCompleted {
                task_key: task.task_key.clone(),
                output: Some(output),
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
                "media worker failed to send workflow step_failed signal"
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
        task_key = %task.task_key,
        "media task completed"
    );

    Ok(())
}

fn context_uuid(value: &Value, key: &str) -> Result<Uuid> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("workflow context missing {key}"))?
        .parse::<Uuid>()
        .map_err(|error| anyhow!("invalid workflow context {key}: {error}"))
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
        tracing::debug!(subject = %event.subject, "media worker woke from event bus");
    }
}
