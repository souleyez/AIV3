use anyhow::{anyhow, Result};
use chrono::Utc;
use domain_model::{WorkflowKind, WorkflowTask};
use event_bus::{workflow_task_enqueued_subject, EventBus, EventSubscription};
use platform_api::{consume_assistant_run_model_completion_turn_dispatch, AppState};
use serde_json::Value;
use storage::{PgStorage, DEFAULT_LOCAL_DATABASE_URL};
use tokio::time::Duration;
use workflow_engine::{WorkflowCatalog, WorkflowSignal};

const DEFAULT_QUEUE: &str = "assistant_run";
const DEFAULT_WAKE_TASK_KEY: &str = "consume_model_completion_turn";
const DEFAULT_POLL_INTERVAL_MS: u64 = 1_000;

#[tokio::main]
async fn main() -> Result<()> {
    observability::install("assistant_run_worker")?;

    let database_url = std::env::var("PLATFORM_DATABASE_URL")
        .unwrap_or_else(|_| DEFAULT_LOCAL_DATABASE_URL.to_string());
    let queue = std::env::var("ASSISTANT_RUN_QUEUE").unwrap_or_else(|_| DEFAULT_QUEUE.to_string());
    let task_key = optional_env("ASSISTANT_RUN_TASK_KEY");
    let poll_interval = std::env::var("ASSISTANT_RUN_POLL_INTERVAL_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_POLL_INTERVAL_MS);

    let storage = PgStorage::connect_with_configured_max_connections(
        &database_url,
        "ASSISTANT_RUN_DATABASE_MAX_CONNECTIONS",
    )
    .await?;
    let workflow_catalog = workflow_definitions::catalog();
    let event_bus = EventBus::connect_from_env_or_disabled("PLATFORM_NATS_URL").await;
    let wake_task_key = task_key.as_deref().unwrap_or(DEFAULT_WAKE_TASK_KEY);
    let wake_subject = workflow_task_enqueued_subject(&queue, wake_task_key);
    let mut task_waker = event_bus
        .subscribe_queue_or_disabled(
            &wake_subject,
            Some(&format!("assistant_run_worker.{queue}.{wake_task_key}")),
        )
        .await;

    tracing::info!(
        %queue,
        task_key = task_key.as_deref().unwrap_or("*"),
        %wake_subject,
        event_bus_enabled = event_bus.is_enabled(),
        poll_interval_ms = poll_interval,
        %database_url,
        "assistant-run-worker polling started"
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
                    tracing::error!(error = ?error, "assistant run task processing failed");
                }
            }
            Ok(None) => wait_for_next_task_signal(&mut task_waker, poll_interval).await,
            Err(error) => {
                tracing::error!(error = ?error, "assistant run worker failed to claim task");
                wait_for_next_task_signal(&mut task_waker, poll_interval).await;
            }
        }
    }
}

async fn process_task(
    storage: &PgStorage,
    workflow_catalog: &WorkflowCatalog,
    event_bus: &EventBus,
    task: WorkflowTask,
) -> Result<()> {
    let process_result: Result<Value> = async {
        let execution = storage
            .workflow_executions()
            .get_by_id(task.tenant_id, task.execution_id)
            .await?
            .ok_or_else(|| anyhow!("workflow execution {} not found", task.execution_id))?;
        if execution.kind != WorkflowKind::AssistantRunModelCompletion {
            return Err(anyhow!(
                "workflow execution {} has unexpected kind {}",
                execution.id,
                execution.kind.as_str()
            ));
        }
        if task.task_key != DEFAULT_WAKE_TASK_KEY {
            return Err(anyhow!(
                "unsupported assistant run task key {}",
                task.task_key
            ));
        }
        let dispatch_request = execution
            .context
            .get("dispatch_request")
            .filter(|value| value.is_object())
            .ok_or_else(|| anyhow!("workflow context missing dispatch_request"))?;
        let state = AppState::new(
            storage.clone(),
            workflow_catalog.clone(),
            task.tenant_id,
            event_bus.clone(),
        );
        consume_assistant_run_model_completion_turn_dispatch(&state, dispatch_request)
            .await
            .map_err(|error| anyhow!(error.to_string()))
    }
    .await;

    match process_result {
        Ok(output) => {
            platform_api::apply_workflow_signal_with_dependencies(
                storage,
                workflow_catalog,
                event_bus,
                task.tenant_id,
                task.execution_id,
                WorkflowSignal::StepCompleted {
                    task_key: task.task_key.clone(),
                    output: Some(output.clone()),
                },
            )
            .await
            .map_err(|error| anyhow!(error.to_string()))?;
            storage
                .workflow_tasks()
                .mark_succeeded(task.id, Utc::now())
                .await?;
            let output_status = output
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            tracing::info!(
                task_id = %task.id,
                execution_id = %task.execution_id,
                task_key = %task.task_key,
                status = output_status,
                "assistant run task completed"
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
                    "failed to advance assistant run workflow after task failure"
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
        tracing::debug!(subject = %event.subject, "assistant run worker woke from event bus");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_env_treats_blank_and_wildcard_as_unset() {
        std::env::set_var("ASSISTANT_RUN_WORKER_OPTIONAL_TEST", " ");
        assert_eq!(optional_env("ASSISTANT_RUN_WORKER_OPTIONAL_TEST"), None);
        std::env::set_var("ASSISTANT_RUN_WORKER_OPTIONAL_TEST", "*");
        assert_eq!(optional_env("ASSISTANT_RUN_WORKER_OPTIONAL_TEST"), None);
        std::env::set_var(
            "ASSISTANT_RUN_WORKER_OPTIONAL_TEST",
            "consume_model_completion_turn",
        );
        assert_eq!(
            optional_env("ASSISTANT_RUN_WORKER_OPTIONAL_TEST").as_deref(),
            Some("consume_model_completion_turn")
        );
        std::env::remove_var("ASSISTANT_RUN_WORKER_OPTIONAL_TEST");
    }

    #[test]
    fn wake_subject_matches_registered_workflow_queue() {
        assert_eq!(
            workflow_task_enqueued_subject(DEFAULT_QUEUE, DEFAULT_WAKE_TASK_KEY),
            "workflow.task.enqueued.assistant_run.consume_model_completion_turn"
        );
    }

    #[test]
    fn workflow_registry_exposes_consumer_task() {
        let definition = workflow_definitions::catalog()
            .find_definition(WorkflowKind::AssistantRunModelCompletion)
            .expect("assistant run model-completion workflow exists");
        let pending =
            definition.initial_state(domain_model::WorkflowExecutionId::new(), Utc::now());
        assert_eq!(
            pending.context.get("queue"),
            Some(&serde_json::json!(DEFAULT_QUEUE))
        );
        assert_eq!(
            pending.context.get("task_key"),
            Some(&serde_json::json!(DEFAULT_WAKE_TASK_KEY))
        );
    }
}
