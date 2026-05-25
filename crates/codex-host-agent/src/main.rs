use anyhow::{anyhow, Result};
use chrono::Utc;
use codex_host_agent::{
    extract_fixed_task_output_from_stdout, materialize_fixed_task_bundle, safe_log_excerpt,
    CodexCommandPlan, CodexHostAgentPolicy, CodexHostExecutionDecision, CodexHostExecutionMode,
    CodexHostRuntimeConfig, CodexHostTaskContext, CodexProcessOutput,
};
use contracts::CodexHostTaskOutputView;
use domain_model::{AssistantRunId, WorkflowKind};
use event_bus::{
    workflow_execution_transition_subject, workflow_task_enqueued_subject, EventBus, EventEnvelope,
    EventSubscription,
};
use serde_json::{json, Map, Value};
use storage::{NewAssistantRunEvent, NewWorkflowTask, PgStorage, DEFAULT_LOCAL_DATABASE_URL};
use tokio::process::Command;
use tokio::time::{Duration, Instant};
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
    let runtime_config = CodexHostRuntimeConfig::from_env();

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
        task_timeout_ms = runtime_config.task_timeout_ms(),
        heartbeat_ms = runtime_config.heartbeat_ms(),
        stdout_limit_bytes = runtime_config.stdout_limit_bytes(),
        stderr_limit_bytes = runtime_config.stderr_limit_bytes(),
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
                    &runtime_config,
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
    runtime_config: &CodexHostRuntimeConfig,
    task: domain_model::WorkflowTask,
) -> Result<()> {
    let execution = storage
        .workflow_executions()
        .get_by_id(task.tenant_id, task.execution_id)
        .await?
        .ok_or_else(|| anyhow!("workflow execution {} not found", task.execution_id))?;
    if execution.status == domain_model::WorkflowStatus::Cancelled {
        storage
            .workflow_tasks()
            .mark_failed(task.id, "workflow_cancelled_before_start", Utc::now())
            .await?;
        return Ok(());
    }
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
                run_codex_exec_with_heartbeat(
                    storage,
                    task.tenant_id,
                    task.execution_id,
                    command_plan,
                    &task_context,
                    &decision,
                    runtime_config,
                )
                .await?
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

async fn run_codex_exec_with_heartbeat(
    storage: &PgStorage,
    tenant_id: domain_model::TenantId,
    execution_id: domain_model::WorkflowExecutionId,
    command_plan: &CodexCommandPlan,
    task_context: &CodexHostTaskContext,
    decision: &CodexHostExecutionDecision,
    runtime_config: &CodexHostRuntimeConfig,
) -> Result<serde_json::Value> {
    let started_at = Instant::now();
    let mut heartbeat_interval =
        tokio::time::interval(Duration::from_millis(runtime_config.heartbeat_ms()));
    let mut heartbeat_count = 0usize;
    let exec = run_codex_exec(command_plan, task_context, decision, runtime_config);
    tokio::pin!(exec);

    loop {
        tokio::select! {
            result = &mut exec => return result,
            _ = heartbeat_interval.tick() => {
                heartbeat_count += 1;
                if codex_host_execution_cancelled(storage, tenant_id, execution_id).await? {
                    return Err(anyhow!("Codex Host command cancelled before completion"));
                }
                let payload = codex_exec_heartbeat_payload(
                    command_plan,
                    task_context,
                    decision,
                    started_at.elapsed().as_millis() as u64,
                    heartbeat_count,
                );
                if let Err(error) = append_assistant_event(
                    storage,
                    tenant_id,
                    task_context.assistant_run_id,
                    "codex_host_task.exec_heartbeat",
                    payload,
                )
                .await
                {
                    tracing::warn!(
                        error = ?error,
                        execution_id = %execution_id,
                        "failed to append Codex Host heartbeat event"
                    );
                }
            }
        }
    }
}

async fn codex_host_execution_cancelled(
    storage: &PgStorage,
    tenant_id: domain_model::TenantId,
    execution_id: domain_model::WorkflowExecutionId,
) -> Result<bool> {
    let Some(execution) = storage
        .workflow_executions()
        .get_by_id(tenant_id, execution_id)
        .await?
    else {
        return Ok(false);
    };
    Ok(execution.status == domain_model::WorkflowStatus::Cancelled)
}

fn codex_exec_heartbeat_payload(
    command_plan: &CodexCommandPlan,
    task_context: &CodexHostTaskContext,
    decision: &CodexHostExecutionDecision,
    elapsed_ms: u64,
    heartbeat_count: usize,
) -> Value {
    json!({
        "mode": "codex_exec",
        "status": "running",
        "assistant_run_id": task_context.assistant_run_id.to_string(),
        "capability": task_context.capability.clone(),
        "host_kind": decision.host_kind.clone(),
        "profile": decision.profile.safe_summary(),
        "command_plan": command_plan.safe_summary(),
        "elapsed_ms": elapsed_ms,
        "heartbeat_count": heartbeat_count,
        "raw_prompt_exposed": false,
        "stdout_exposed": false,
        "stderr_exposed": false,
        "secrets_exposed": false,
    })
}

async fn run_codex_exec(
    command_plan: &CodexCommandPlan,
    task_context: &CodexHostTaskContext,
    decision: &CodexHostExecutionDecision,
    runtime_config: &CodexHostRuntimeConfig,
) -> Result<serde_json::Value> {
    if let Some(workspace_path) = command_plan.workspace_path.as_ref() {
        std::fs::create_dir_all(workspace_path).map_err(|error| {
            anyhow!(
                "failed to prepare Codex Host task workspace {}: {error}",
                workspace_path.display()
            )
        })?;
        materialize_fixed_task_bundle(workspace_path, task_context, decision)?;
    }
    let mut command = Command::new(&command_plan.program);
    command.args(command_plan.process_args());
    if let Some(workspace_path) = command_plan.workspace_path.as_ref() {
        command.current_dir(workspace_path);
    }
    command.kill_on_drop(true);
    let output = tokio::time::timeout(
        Duration::from_millis(runtime_config.task_timeout_ms()),
        command.output(),
    )
    .await
    .map_err(|_| {
        anyhow!(
            "Codex Host command timed out after {}ms; stdout_chars=0; stderr_chars=0",
            runtime_config.task_timeout_ms()
        )
    })?
    .map_err(|error| anyhow!("failed to launch Codex Host command: {error}"))?;
    let process_output = CodexProcessOutput {
        exit_code: output.status.code(),
        stdout_excerpt: safe_log_excerpt(&output.stdout, runtime_config.stdout_limit_bytes()),
        stderr_excerpt: safe_log_excerpt(&output.stderr, runtime_config.stderr_limit_bytes()),
    };

    if !output.status.success() {
        return Err(anyhow!(
            "Codex Host command failed: kind=non_zero_exit exit_code={:?}; stdout_chars={}; stderr_chars={}",
            process_output.exit_code,
            process_output.stdout_excerpt.chars().count(),
            process_output.stderr_excerpt.chars().count()
        ));
    }

    let fixed_task_output = task_context
        .fixed_task
        .as_ref()
        .map(|fixed_task| {
            extract_fixed_task_output_from_stdout(&output.stdout, fixed_task.template_id.as_str())
        })
        .transpose()?;

    Ok(codex_exec_output(
        command_plan,
        task_context,
        decision,
        process_output,
        fixed_task_output,
    ))
}

fn codex_exec_output(
    command_plan: &CodexCommandPlan,
    task_context: &CodexHostTaskContext,
    decision: &CodexHostExecutionDecision,
    process_output: CodexProcessOutput,
    fixed_task_output: Option<Value>,
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
        fixed_task_output,
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

        let output = codex_exec_output(
            &command_plan,
            &task_context,
            &decision,
            process_output,
            None,
        );

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
    fn codex_exec_output_keeps_fixed_task_output_without_raw_stdout() {
        let mut task_context = CodexHostTaskContext {
            assistant_run_id: AssistantRunId::new(),
            capability: "static_page_image2_data_publish".to_string(),
            task: Some("Run fixed static-page package".to_string()),
            local_thread_id: None,
            task_memory_isolated: true,
            task_memory_space_id: Some("codex-host-task:static-page".to_string()),
            fixed_task: Some(
                contracts::CodexHostFixedTaskTemplateContextView::static_page_image2_data_publish_example(),
            ),
        };
        task_context.capability = task_context
            .fixed_task
            .as_ref()
            .expect("fixed task")
            .template_id
            .as_str()
            .to_string();
        let command_plan = CodexCommandPlan {
            program: "codex".to_string(),
            args_without_prompt: vec!["exec".to_string(), "--ephemeral".to_string()],
            prompt: "Run fixed static-page package".to_string(),
            sandbox: "workspace-write".to_string(),
            workspace_path: Some(std::path::PathBuf::from(
                "D:/codex-host/tasks/codex-host-static-page",
            )),
            workspace_label: Some("codex-host-static-page".to_string()),
        };
        let decision = CodexHostExecutionDecision {
            mode: CodexHostExecutionMode::CodexExec,
            profile: codex_host_agent::CodexHostProfile {
                id: "cloudflare-fixed".to_string(),
                kind: "codex-native".to_string(),
                model: Some("gpt-5.3-codex".to_string()),
                provider_id: None,
                base_url: None,
                env_key: None,
                wire_api: None,
                allowed_capabilities: vec!["static_page_image2_data_publish".to_string()],
            },
            host_kind: "cloudflare_codex".to_string(),
            command_plan: Some(command_plan.clone()),
        };
        let fixed_task_output = json!({
            "template_id": "static_page_image2_data_publish",
            "status": "success",
            "artifact": {
                "public_url": "https://v3.elepcloud.com/generated-artifacts/static-page/index.html"
            },
            "validation_report": {
                "source_row_count": 1
            }
        });
        let process_output = CodexProcessOutput {
            exit_code: Some(0),
            stdout_excerpt: "raw public url and internal notes".to_string(),
            stderr_excerpt: "stderr internals".to_string(),
        };

        let output = codex_exec_output(
            &command_plan,
            &task_context,
            &decision,
            process_output,
            Some(fixed_task_output.clone()),
        );
        let serialized = output.to_string();

        assert_eq!(output["fixed_task_output"], fixed_task_output);
        assert_eq!(output["process"]["stdout_excerpt"], json!(""));
        assert_eq!(output["process"]["stderr_excerpt"], json!(""));
        assert!(!serialized.contains("raw public url and internal notes"));
        assert!(!serialized.contains("stderr internals"));
    }

    #[tokio::test]
    async fn codex_exec_nonzero_exit_returns_bounded_error() {
        let (task_context, command_plan, decision) = codex_exec_test_context(false);
        let runtime_config = CodexHostRuntimeConfig {
            task_timeout_ms: 10_000,
            heartbeat_ms: 1_000,
            stdout_limit_bytes: 80,
            stderr_limit_bytes: 80,
        };

        let error = run_codex_exec(&command_plan, &task_context, &decision, &runtime_config)
            .await
            .expect_err("non-zero command should fail");
        let message = error.to_string();

        assert!(message.contains("kind=non_zero_exit"));
        assert!(message.contains("stdout_chars="));
        assert!(message.contains("stderr_chars="));
        assert!(!message.contains("secret"));
        assert!(!message.contains("api_key"));
    }

    #[tokio::test]
    async fn codex_exec_timeout_returns_bounded_error() {
        let (task_context, command_plan, decision) = codex_exec_test_context(true);
        let runtime_config = CodexHostRuntimeConfig {
            task_timeout_ms: 50,
            heartbeat_ms: 1_000,
            stdout_limit_bytes: 80,
            stderr_limit_bytes: 80,
        };

        let error = run_codex_exec(&command_plan, &task_context, &decision, &runtime_config)
            .await
            .expect_err("timeout command should fail");
        let message = error.to_string();

        assert!(message.contains("timed out"));
        assert!(message.contains("stdout_chars=0"));
        assert!(message.contains("stderr_chars=0"));
        assert!(!message.contains("timeout-secret"));
    }

    #[test]
    fn codex_exec_heartbeat_payload_is_bounded() {
        let (task_context, command_plan, decision) = codex_exec_test_context(false);

        let payload = codex_exec_heartbeat_payload(&command_plan, &task_context, &decision, 123, 2);
        let serialized = payload.to_string();

        assert_eq!(payload["status"], json!("running"));
        assert_eq!(payload["raw_prompt_exposed"], json!(false));
        assert_eq!(payload["stdout_exposed"], json!(false));
        assert_eq!(payload["stderr_exposed"], json!(false));
        assert_eq!(payload["heartbeat_count"], json!(2));
        assert!(!serialized.contains("inspect secret prompt"));
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

    fn codex_exec_test_context(
        slow: bool,
    ) -> (
        CodexHostTaskContext,
        CodexCommandPlan,
        CodexHostExecutionDecision,
    ) {
        let task_context = CodexHostTaskContext {
            assistant_run_id: AssistantRunId::new(),
            capability: "inspect_project".to_string(),
            task: Some("inspect secret prompt".to_string()),
            local_thread_id: Some("thread-a".to_string()),
            task_memory_isolated: true,
            task_memory_space_id: Some("codex-host-task:test".to_string()),
            fixed_task: None,
        };
        let command_plan = codex_exec_test_command_plan(slow);
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
        (task_context, command_plan, decision)
    }

    #[cfg(windows)]
    fn codex_exec_test_command_plan(slow: bool) -> CodexCommandPlan {
        let script = if slow {
            "Start-Sleep -Milliseconds 1000; Write-Output 'timeout-secret'"
        } else {
            "Write-Output 'ok'; Write-Output 'api_key=secret'; Write-Error 'stderr secret'; exit 7"
        };
        CodexCommandPlan {
            program: "powershell".to_string(),
            args_without_prompt: vec![
                "-NoProfile".to_string(),
                "-Command".to_string(),
                script.to_string(),
            ],
            prompt: "inspect secret prompt".to_string(),
            sandbox: "read-only".to_string(),
            workspace_path: None,
            workspace_label: Some("test-workspace".to_string()),
        }
    }

    #[cfg(not(windows))]
    fn codex_exec_test_command_plan(slow: bool) -> CodexCommandPlan {
        let script = if slow {
            "sleep 1; printf '%s\n' 'timeout-secret'"
        } else {
            "printf '%s\n' 'ok'; printf '%s\n' 'api_key=secret'; printf '%s\n' 'stderr secret' 1>&2; exit 7"
        };
        CodexCommandPlan {
            program: "sh".to_string(),
            args_without_prompt: vec!["-c".to_string(), script.to_string()],
            prompt: "inspect secret prompt".to_string(),
            sandbox: "read-only".to_string(),
            workspace_path: None,
            workspace_label: Some("test-workspace".to_string()),
        }
    }
}
