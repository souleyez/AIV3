use anyhow::{anyhow, Result};
use chrono::Utc;
use codex_host_agent::{
    safe_log_excerpt, CodexCommandPlan, CodexHostAgentPolicy, CodexHostExecutionMode,
    CodexHostTaskContext, CodexProcessOutput,
};
use domain_model::{AssistantRunId, WorkflowKind};
use event_bus::{workflow_task_enqueued_subject, EventBus, EventSubscription};
use serde_json::json;
use std::process::Command;
use storage::{NewAssistantRunEvent, PgStorage, DEFAULT_LOCAL_DATABASE_URL};
use tokio::time::Duration;
use workflow_engine::{WorkflowCatalog, WorkflowSignal};

const DEFAULT_QUEUE: &str = "codex_host";
const DEFAULT_TASK_KEY: &str = "run_codex_host_task";
const DEFAULT_POLL_INTERVAL_MS: u64 = 1_000;

#[tokio::main]
async fn main() -> Result<()> {
    observability::install("codex_host_agent")?;

    let database_url = std::env::var("PLATFORM_DATABASE_URL")
        .unwrap_or_else(|_| DEFAULT_LOCAL_DATABASE_URL.to_string());
    let queue = std::env::var("CODEX_HOST_QUEUE").unwrap_or_else(|_| DEFAULT_QUEUE.to_string());
    let task_key =
        std::env::var("CODEX_HOST_TASK_KEY").unwrap_or_else(|_| DEFAULT_TASK_KEY.to_string());
    let poll_interval = std::env::var("CODEX_HOST_AGENT_POLL_INTERVAL_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_POLL_INTERVAL_MS);
    let execution_policy = CodexHostAgentPolicy::from_env()?;

    let storage = PgStorage::connect(&database_url).await?;
    let workflow_catalog = workflow_definitions::catalog();
    let event_bus = EventBus::connect_from_env_or_disabled("PLATFORM_NATS_URL").await;
    let wake_subject = workflow_task_enqueued_subject(&queue, &task_key);
    let mut task_waker = event_bus
        .subscribe_queue_or_disabled(
            &wake_subject,
            Some(&format!("codex_host_agent.{queue}.{task_key}")),
        )
        .await;

    tracing::info!(
        %queue,
        %task_key,
        %wake_subject,
        event_bus_enabled = event_bus.is_enabled(),
        poll_interval_ms = poll_interval,
        execution_mode = %execution_policy.mode.as_str(),
        profile_id = %execution_policy.profile.id,
        profile_kind = %execution_policy.profile.kind,
        host_kind = %execution_policy.host_kind,
        "codex-host-agent polling started"
    );

    loop {
        match storage
            .workflow_tasks()
            .claim_next_available(&queue, Some(&task_key), Utc::now())
            .await
        {
            Ok(Some(task)) => {
                if let Err(error) = process_task(
                    &storage,
                    &workflow_catalog,
                    &event_bus,
                    &execution_policy,
                    task,
                )
                .await
                {
                    tracing::error!(error = ?error, "codex host task processing failed");
                }
            }
            Ok(None) => {
                wait_for_next_task_signal(&mut task_waker, poll_interval).await;
            }
            Err(error) => {
                tracing::error!(error = ?error, "codex host agent failed to claim task");
                wait_for_next_task_signal(&mut task_waker, poll_interval).await;
            }
        }
    }
}

async fn process_task(
    storage: &PgStorage,
    workflow_catalog: &WorkflowCatalog,
    event_bus: &EventBus,
    execution_policy: &CodexHostAgentPolicy,
    task: domain_model::WorkflowTask,
) -> Result<()> {
    let execution = storage
        .workflow_executions()
        .get_by_id(task.tenant_id, task.execution_id)
        .await?
        .ok_or_else(|| anyhow!("workflow execution {} not found", task.execution_id))?;
    if execution.kind != WorkflowKind::CodexHostTask {
        return Err(anyhow!(
            "workflow execution {} has unexpected kind {}",
            execution.id,
            execution.kind.as_str()
        ));
    }
    let task_context = CodexHostTaskContext::from_execution(&execution)?;

    let process_result: Result<()> = async {
        let decision = execution_policy.prepare(&task_context)?;
        let output = match decision.mode {
            CodexHostExecutionMode::DryRun => task_context.dry_run_output(),
            CodexHostExecutionMode::PlanOnly => task_context.planned_output(&decision),
            CodexHostExecutionMode::CodexExec => {
                let command_plan = decision
                    .command_plan
                    .as_ref()
                    .ok_or_else(|| anyhow!("codex_exec mode missing command plan"))?;
                run_codex_exec(command_plan, &task_context)?
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
                output: Some(output.clone()),
            },
        )
        .await?;
        append_assistant_event(
            storage,
            task.tenant_id,
            task_context.assistant_run_id,
            "codex_host_task.dry_run_completed",
            output,
        )
        .await?;
        storage
            .workflow_tasks()
            .mark_succeeded(task.id, Utc::now())
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
                "codex host task failed to send workflow step_failed signal"
            );
        }
        storage
            .workflow_tasks()
            .mark_failed(task.id, &error_message, Utc::now())
            .await?;
        return Err(error);
    }

    tracing::info!(
        task_id = %task.id,
        execution_id = %task.execution_id,
        capability = %task_context.capability,
        mode = %execution_policy.mode.as_str(),
        "codex host task completed"
    );

    Ok(())
}

async fn append_assistant_event(
    storage: &PgStorage,
    tenant_id: domain_model::TenantId,
    run_id: AssistantRunId,
    event_name: &str,
    payload: serde_json::Value,
) -> Result<()> {
    storage
        .assistant_runs()
        .append_event(
            tenant_id,
            run_id,
            &NewAssistantRunEvent {
                event_name: event_name.to_string(),
                payload,
                created_at: Utc::now(),
            },
        )
        .await?;
    Ok(())
}

fn run_codex_exec(
    command_plan: &CodexCommandPlan,
    task_context: &CodexHostTaskContext,
) -> Result<serde_json::Value> {
    let output = Command::new(&command_plan.program)
        .args(command_plan.process_args())
        .output()
        .map_err(|error| anyhow!("failed to launch Codex Host command: {error}"))?;
    let process_output = CodexProcessOutput {
        exit_code: output.status.code(),
        stdout_excerpt: safe_log_excerpt(&output.stdout, 2_000),
        stderr_excerpt: safe_log_excerpt(&output.stderr, 2_000),
    };

    if !output.status.success() {
        return Err(anyhow!(
            "Codex Host command failed with status {:?}; stdout={}; stderr={}",
            process_output.exit_code,
            process_output.stdout_excerpt,
            process_output.stderr_excerpt
        ));
    }

    Ok(json!({
        "mode": "codex_exec",
        "codex_invoked": true,
        "status": "completed",
        "assistant_run_id": task_context.assistant_run_id.to_string(),
        "capability": task_context.capability,
        "command_plan": command_plan.safe_summary(),
        "process": process_output.safe_summary(),
        "task_chars": task_context.task.as_ref().map(|task| task.chars().count()).unwrap_or(0),
        "local_thread_id": task_context.local_thread_id,
        "task_memory_isolated": task_context.task_memory_isolated,
    }))
}

async fn wait_for_next_task_signal(task_waker: &mut EventSubscription, poll_interval_ms: u64) {
    if let Some(event) = task_waker
        .wait_for_event(Duration::from_millis(poll_interval_ms))
        .await
    {
        tracing::debug!(subject = %event.subject, "codex host agent received task wake signal");
    }
}
