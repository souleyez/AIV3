use anyhow::{anyhow, Result};
use chrono::Utc;
use codex_host_agent::{
    safe_log_excerpt, CodexCommandPlan, CodexHostAgentPolicy, CodexHostExecutionDecision,
    CodexHostExecutionMode, CodexHostTaskContext, CodexProcessOutput,
};
use contracts::CodexHostTaskOutputView;
use domain_model::{AssistantRunId, WorkflowKind};
use event_bus::{
    workflow_execution_transition_subject, workflow_task_enqueued_subject, EventBus, EventEnvelope,
    EventSubscription,
};
use serde_json::{json, Map, Value};
use std::process::Command;
use storage::{NewAssistantRunEvent, NewWorkflowTask, PgStorage, DEFAULT_LOCAL_DATABASE_URL};
use tokio::time::Duration;
use workflow_engine::{WorkflowCatalog, WorkflowRuntimeState, WorkflowSignal};

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
                run_codex_exec(command_plan, &task_context, &decision)?
            }
        };
        let event_name = codex_host_task_event_name(&output);
        apply_workflow_signal_with_dependencies(
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
            event_name,
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
        if let Err(signal_error) = apply_workflow_signal_with_dependencies(
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
    decision: &CodexHostExecutionDecision,
) -> Result<serde_json::Value> {
    if let Some(workspace_path) = command_plan.workspace_path.as_ref() {
        std::fs::create_dir_all(workspace_path).map_err(|error| {
            anyhow!(
                "failed to prepare Codex Host task workspace {}: {error}",
                workspace_path.display()
            )
        })?;
    }
    let mut command = Command::new(&command_plan.program);
    command.args(command_plan.process_args());
    if let Some(workspace_path) = command_plan.workspace_path.as_ref() {
        command.current_dir(workspace_path);
    }
    let output = command
        .output()
        .map_err(|error| anyhow!("failed to launch Codex Host command: {error}"))?;
    let process_output = CodexProcessOutput {
        exit_code: output.status.code(),
        stdout_excerpt: safe_log_excerpt(&output.stdout, 2_000),
        stderr_excerpt: safe_log_excerpt(&output.stderr, 2_000),
    };

    if !output.status.success() {
        return Err(anyhow!(
            "Codex Host command failed with status {:?}; stdout_chars={}; stderr_chars={}",
            process_output.exit_code,
            process_output.stdout_excerpt.chars().count(),
            process_output.stderr_excerpt.chars().count()
        ));
    }

    Ok(codex_exec_output(
        command_plan,
        task_context,
        decision,
        process_output,
    ))
}

fn codex_exec_output(
    command_plan: &CodexCommandPlan,
    task_context: &CodexHostTaskContext,
    decision: &CodexHostExecutionDecision,
    process_output: CodexProcessOutput,
) -> serde_json::Value {
    let html_artifacts = vec![task_context.html_report_artifact(
        "codex_exec",
        "completed",
        Some(decision),
        Some(&process_output),
    )];
    json!(CodexHostTaskOutputView {
        mode: "codex_exec".to_string(),
        codex_invoked: true,
        status: "completed".to_string(),
        host_kind: Some(decision.host_kind.clone()),
        assistant_run_id: task_context.assistant_run_id.to_string(),
        capability: task_context.capability.clone(),
        profile: Some(decision.profile.safe_summary()),
        command_plan: Some(command_plan.safe_summary()),
        process: Some(process_output.safe_summary()),
        task_chars: task_context
            .task
            .as_ref()
            .map(|task| task.chars().count())
            .unwrap_or(0),
        local_thread_id: task_context.local_thread_id.clone(),
        task_memory_isolated: task_context.task_memory_isolated,
        task_memory_space_id: task_context.task_memory_space_id.clone(),
        html_artifacts,
    })
}

fn codex_host_task_event_name(output: &serde_json::Value) -> &'static str {
    match output.get("mode").and_then(Value::as_str) {
        Some("dry_run") => "codex_host_task.dry_run_completed",
        Some("plan_only") => "codex_host_task.plan_only_completed",
        Some("codex_exec") => "codex_host_task.exec_completed",
        _ => "codex_host_task.completed",
    }
}

async fn apply_workflow_signal_with_dependencies(
    storage: &PgStorage,
    workflow_catalog: &WorkflowCatalog,
    event_bus: &EventBus,
    tenant_id: domain_model::TenantId,
    execution_id: domain_model::WorkflowExecutionId,
    signal: WorkflowSignal,
) -> Result<()> {
    let execution = storage
        .workflow_executions()
        .get_by_id(tenant_id, execution_id)
        .await?
        .ok_or_else(|| anyhow!("workflow execution {} not found", execution_id))?;
    let definition = workflow_catalog
        .find_definition(execution.kind.clone())
        .ok_or_else(|| {
            anyhow!(
                "workflow definition {} is not registered",
                execution.kind.as_str()
            )
        })?;
    let now = Utc::now();
    let runtime_state = workflow_runtime_state_from_execution(&execution)?;
    let transition = definition.transition(&runtime_state, signal, now)?;
    let workflow_engine::WorkflowTransition {
        next_state,
        persisted_event,
        enqueued_tasks,
    } = transition;
    let next_execution = workflow_execution_from_transition(&execution, next_state);
    let pending_tasks = enqueued_tasks
        .into_iter()
        .map(|task| NewWorkflowTask {
            queue: task.queue,
            task_key: task.task_key,
            payload: task.payload,
            available_at: persisted_event.occurred_at,
            max_attempts: 3,
        })
        .collect::<Vec<_>>();
    let (persisted_event, persisted_tasks) = storage
        .workflow_executions()
        .advance_with_tasks(
            &next_execution,
            &persisted_event.name,
            &persisted_event.detail,
            persisted_event.occurred_at,
            &pending_tasks,
        )
        .await?;
    publish_workflow_transition_events(
        event_bus,
        &next_execution,
        &persisted_event,
        &persisted_tasks,
    )
    .await;
    Ok(())
}

fn workflow_runtime_state_from_execution(
    execution: &domain_model::WorkflowExecution,
) -> Result<WorkflowRuntimeState> {
    let mut context = extract_context_object(&execution.context)?;
    let retries_remaining = context
        .remove("retries_remaining")
        .and_then(|value| value.as_u64())
        .unwrap_or(3) as u32;

    Ok(WorkflowRuntimeState {
        execution_id: execution.id,
        kind: execution.kind.clone(),
        version: execution.version.clone(),
        stage: execution.stage.clone(),
        status: execution.status.clone(),
        retries_remaining,
        context,
        updated_at: execution.updated_at,
    })
}

fn workflow_execution_from_transition(
    previous: &domain_model::WorkflowExecution,
    next_state: WorkflowRuntimeState,
) -> domain_model::WorkflowExecution {
    let next_attempt = if previous.status == domain_model::WorkflowStatus::Pending
        && next_state.status == domain_model::WorkflowStatus::Running
    {
        previous.attempt + 1
    } else {
        previous.attempt
    };
    let WorkflowRuntimeState {
        kind,
        version,
        stage,
        status,
        retries_remaining,
        mut context,
        updated_at,
        ..
    } = next_state;
    context.insert(
        "retries_remaining".to_string(),
        Value::Number(retries_remaining.into()),
    );
    domain_model::WorkflowExecution {
        id: previous.id,
        tenant_id: previous.tenant_id,
        dataset_id: previous.dataset_id,
        report_plan_id: previous.report_plan_id,
        kind,
        version,
        stage,
        status,
        attempt: next_attempt,
        context: Value::Object(context),
        created_at: previous.created_at,
        updated_at,
    }
}

fn extract_context_object(value: &Value) -> Result<Map<String, Value>> {
    match value {
        Value::Object(map) => Ok(map.clone()),
        Value::Null => Ok(Map::new()),
        _ => Err(anyhow!("workflow execution context must be a JSON object")),
    }
}

async fn publish_workflow_transition_events(
    event_bus: &EventBus,
    execution: &domain_model::WorkflowExecution,
    persisted_event: &domain_model::WorkflowEventRecord,
    persisted_tasks: &[domain_model::WorkflowTask],
) {
    event_bus
        .publish(EventEnvelope {
            subject: workflow_execution_transition_subject(execution.kind.as_str()),
            payload: json!({
                "execution_id": execution.id,
                "tenant_id": execution.tenant_id,
                "kind": execution.kind.as_str(),
                "status": execution.status.as_str(),
                "stage": execution.stage,
                "event_name": persisted_event.event_name,
                "event_sequence_no": persisted_event.sequence_no,
            }),
            published_at: persisted_event.created_at,
        })
        .await;

    for task in persisted_tasks {
        event_bus
            .publish(EventEnvelope {
                subject: workflow_task_enqueued_subject(&task.queue, &task.task_key),
                payload: json!({
                    "task_id": task.id,
                    "tenant_id": task.tenant_id,
                    "execution_id": task.execution_id,
                    "queue": task.queue,
                    "task_key": task.task_key,
                    "status": task.status.as_str(),
                    "available_at": task.available_at,
                }),
                published_at: task.created_at,
            })
            .await;
    }
}

async fn wait_for_next_task_signal(task_waker: &mut EventSubscription, poll_interval_ms: u64) {
    if let Some(event) = task_waker
        .wait_for_event(Duration::from_millis(poll_interval_ms))
        .await
    {
        tracing::debug!(subject = %event.subject, "codex host agent received task wake signal");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_exec_output_uses_shared_contract_shape() {
        let task_context = CodexHostTaskContext {
            assistant_run_id: AssistantRunId::new(),
            capability: "inspect_project".to_string(),
            task: Some("Inspect the repository safely".to_string()),
            local_thread_id: Some("thread-a".to_string()),
            task_memory_isolated: true,
            task_memory_space_id: Some("codex-host-task:test".to_string()),
            fixed_task: None,
        };
        let command_plan = CodexCommandPlan {
            program: "codex".to_string(),
            args_without_prompt: vec!["exec".to_string(), "--ephemeral".to_string()],
            prompt: "Inspect the repository safely".to_string(),
            sandbox: "read-only".to_string(),
            workspace_path: Some(std::path::PathBuf::from(
                "D:/codex-host/tasks/codex-host-task-test",
            )),
            workspace_label: Some("codex-host-task-test".to_string()),
        };
        let decision = CodexHostExecutionDecision {
            mode: CodexHostExecutionMode::CodexExec,
            profile: codex_host_agent::CodexHostProfile {
                id: "readonly".to_string(),
                kind: "codex-native".to_string(),
                model: Some("gpt-5.3-codex".to_string()),
                provider_id: None,
                base_url: None,
                env_key: None,
                wire_api: None,
                allowed_capabilities: vec!["inspect_project".to_string()],
            },
            host_kind: "windows_jump".to_string(),
            command_plan: Some(command_plan.clone()),
        };
        let process_output = CodexProcessOutput {
            exit_code: Some(0),
            stdout_excerpt: "ok".to_string(),
            stderr_excerpt: String::new(),
        };

        let output = codex_exec_output(&command_plan, &task_context, &decision, process_output);

        assert_eq!(output["mode"], json!("codex_exec"));
        assert_eq!(output["codex_invoked"], json!(true));
        assert_eq!(output["host_kind"], json!("windows_jump"));
        assert_eq!(output["profile"]["kind"], json!("codex-native"));
        assert_eq!(output["command_plan"]["prompt_redacted"], json!(true));
        assert_eq!(
            output["command_plan"]["workspace_label"],
            json!("codex-host-task-test")
        );
        assert_eq!(output["process"]["stdout_chars"], json!(2));
        assert_eq!(output["process"]["stderr_chars"], json!(0));
        assert_eq!(output["process"]["stdout_excerpt"], json!(""));
        assert_eq!(output["process"]["stderr_excerpt"], json!(""));
        assert_eq!(
            output["html_artifacts"][0]["template_id"],
            json!("codex_execution_report")
        );
        assert_eq!(
            output["html_artifacts"][0]["payload"]["process"]["stdoutChars"],
            json!(2)
        );
        assert!(output["html_artifacts"][0]["payload"]["process"]["stdoutExcerpt"].is_null());
        assert_eq!(
            output["task_memory_space_id"],
            json!("codex-host-task:test")
        );
    }

    #[test]
    fn codex_host_task_event_name_follows_output_mode() {
        assert_eq!(
            codex_host_task_event_name(&json!({"mode": "dry_run"})),
            "codex_host_task.dry_run_completed"
        );
        assert_eq!(
            codex_host_task_event_name(&json!({"mode": "plan_only"})),
            "codex_host_task.plan_only_completed"
        );
        assert_eq!(
            codex_host_task_event_name(&json!({"mode": "codex_exec"})),
            "codex_host_task.exec_completed"
        );
        assert_eq!(
            codex_host_task_event_name(&json!({"mode": "unexpected"})),
            "codex_host_task.completed"
        );
    }
}
