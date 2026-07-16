use anyhow::{anyhow, Result};
use chrono::Utc;
use event_bus::{workflow_task_enqueued_subject, EventBus, EventSubscription};
use serde_json::Value;
use storage::{PgStorage, DEFAULT_LOCAL_DATABASE_URL};
use tokio::time::Duration;
use workflow_engine::{WorkflowCatalog, WorkflowSignal};

const DEFAULT_QUEUE: &str = "external_action";
const DEFAULT_WAKE_TASK_KEY: &str = "dispatch_external_action";
const DEFAULT_POLL_INTERVAL_MS: u64 = 1_000;

#[tokio::main]
async fn main() -> Result<()> {
    observability::install("external_action_worker")?;

    let database_url = std::env::var("PLATFORM_DATABASE_URL")
        .unwrap_or_else(|_| DEFAULT_LOCAL_DATABASE_URL.to_string());
    let queue =
        std::env::var("EXTERNAL_ACTION_QUEUE").unwrap_or_else(|_| DEFAULT_QUEUE.to_string());
    let task_key = optional_env("EXTERNAL_ACTION_TASK_KEY");
    let poll_interval = std::env::var("EXTERNAL_ACTION_POLL_INTERVAL_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_POLL_INTERVAL_MS);

    let storage = PgStorage::connect_with_configured_max_connections(
        &database_url,
        "EXTERNAL_ACTION_DATABASE_MAX_CONNECTIONS",
    )
    .await?;
    let workflow_catalog = workflow_definitions::catalog();
    let event_bus = EventBus::connect_from_env_or_disabled("PLATFORM_NATS_URL").await;
    let wake_task_key = task_key.as_deref().unwrap_or(DEFAULT_WAKE_TASK_KEY);
    let wake_subject = workflow_task_enqueued_subject(&queue, wake_task_key);
    let mut task_waker = event_bus
        .subscribe_queue_or_disabled(
            &wake_subject,
            Some(&format!("external_action_worker.{queue}.{wake_task_key}")),
        )
        .await;

    tracing::info!(
        %queue,
        task_key = task_key.as_deref().unwrap_or("*"),
        %wake_subject,
        event_bus_enabled = event_bus.is_enabled(),
        poll_interval_ms = poll_interval,
        database_endpoint = %observability::redact_connection_endpoint(&database_url),
        "external-action-worker polling started"
    );

    loop {
        match storage
            .workflow_tasks()
            .claim_next_available(&queue, task_key.as_deref(), Utc::now())
            .await
        {
            Ok(Some(task)) => {
                if let Err(error) =
                    process_task(&storage, &workflow_catalog, &event_bus, task).await
                {
                    tracing::error!(error = ?error, "external action task processing failed");
                }
            }
            Ok(None) => wait_for_next_task_signal(&mut task_waker, poll_interval).await,
            Err(error) => {
                tracing::error!(error = ?error, "external action worker failed to claim task");
                wait_for_next_task_signal(&mut task_waker, poll_interval).await;
            }
        }
    }
}

async fn process_task(
    storage: &PgStorage,
    workflow_catalog: &WorkflowCatalog,
    event_bus: &EventBus,
    task: domain_model::WorkflowTask,
) -> Result<()> {
    let execution = storage
        .workflow_executions()
        .get_by_id(task.tenant_id, task.execution_id)
        .await?
        .ok_or_else(|| anyhow!("workflow execution {} not found", task.execution_id))?;
    let context = execution
        .context
        .as_object()
        .ok_or_else(|| anyhow!("workflow execution context must be an object"))?;
    let connection_id = required_context_string(context, "channel_connection_id")?;
    let action_id = required_context_string(context, "external_action_id")?;

    let result = match task.task_key.as_str() {
        DEFAULT_WAKE_TASK_KEY => platform_api::dispatch_external_action_workflow_task(
            storage.clone(),
            task.tenant_id,
            connection_id.to_string(),
            action_id.to_string(),
        )
        .await
        .map_err(|error| anyhow!(error.to_string())),
        unknown => Err(anyhow!("unsupported external action task key: {unknown}")),
    };

    match result {
        Ok(output) => {
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
            storage
                .workflow_tasks()
                .mark_succeeded(task.id, Utc::now())
                .await?;
            tracing::info!(
                task_id = %task.id,
                execution_id = %task.execution_id,
                task_key = %task.task_key,
                "external action task completed"
            );
            Ok(())
        }
        Err(error) => {
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
                    "failed to advance external action workflow after task failure"
                );
            }
            storage
                .workflow_tasks()
                .mark_failed(task.id, &error_message, Utc::now())
                .await?;
            Err(error)
        }
    }
}

fn required_context_string<'a>(
    context: &'a serde_json::Map<String, Value>,
    key: &str,
) -> Result<&'a str> {
    context
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow!("workflow context missing {key}"))
}

fn optional_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

async fn wait_for_next_task_signal(task_waker: &mut EventSubscription, poll_interval_ms: u64) {
    let _ = task_waker
        .wait_for_event(Duration::from_millis(poll_interval_ms))
        .await;
}
