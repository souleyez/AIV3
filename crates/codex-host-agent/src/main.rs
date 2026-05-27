use anyhow::{anyhow, Result};
use chrono::Utc;
use codex_host_agent::{
    extract_fixed_task_output_from_stdout, fixed_task_output_schema_hint,
    materialize_fixed_task_bundle, safe_log_excerpt, CodexCommandPlan, CodexHostAgentPolicy,
    CodexHostExecutionDecision, CodexHostExecutionMode, CodexHostRuntimeConfig,
    CodexHostTaskContext, CodexProcessOutput,
};
use contracts::CodexHostTaskOutputView;
use domain_model::{AssistantRunId, StaticPageDraftId, WorkflowExecutionId, WorkflowKind};
use event_bus::{
    workflow_execution_transition_subject, workflow_task_enqueued_subject, EventBus, EventEnvelope,
    EventSubscription,
};
use serde_json::{json, Map, Value};
use std::fs;
use std::path::PathBuf;
use std::process::Stdio;
use storage::{NewAssistantRunEvent, NewWorkflowTask, PgStorage, DEFAULT_LOCAL_DATABASE_URL};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::{Duration, Instant};
use uuid::Uuid;
use workflow_engine::{WorkflowCatalog, WorkflowRuntimeState, WorkflowSignal};

const DEFAULT_QUEUE: &str = "codex_host";
const DEFAULT_TASK_KEY: &str = "run_codex_host_task";
const DEFAULT_POLL_INTERVAL_MS: u64 = 1_000;
const DEFAULT_ORCHESTRATOR_BASE_URL: &str = "https://souleye.cc";
const DEFAULT_ORCHESTRATOR_API_PATH: &str = "/api/codex/orchestrator/v1";
const DEFAULT_ORCHESTRATOR_RUNTIME_TARGET: &str = "cloudflare";
const DEFAULT_ORCHESTRATOR_SOURCE: &str = "v3-codex-host-agent";
const DEFAULT_ORCHESTRATOR_KIND: &str = "code-task";
const DEFAULT_ORCHESTRATOR_POLL_INTERVAL_MS: u64 = 5_000;
const DEFAULT_ORCHESTRATOR_RETRY_DELAY_MS: u64 = 15_000;
const ORCHESTRATOR_FIXED_TASK_PROMPT_LIMIT_CHARS: usize = 6_500;

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
            CodexHostExecutionMode::CloudflareOrchestrator => {
                run_cloudflare_orchestrator_with_heartbeat(
                    storage,
                    task.tenant_id,
                    task.execution_id,
                    task.id,
                    &task.payload,
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
            output.clone(),
        )
        .await?;
        maybe_record_external_static_page_publish_completed_from_task_output(
            storage,
            task.tenant_id,
            task_context.assistant_run_id,
            task.execution_id,
            &task_context,
            &output,
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
        if should_requeue_cloudflare_orchestrator_poll(
            &execution_policy.mode,
            &error_message,
            &task,
        ) {
            let now = Utc::now();
            let retry_delay_ms = cloudflare_orchestrator_retry_delay_ms();
            let available_at =
                now + chrono::Duration::milliseconds(retry_delay_ms.min(i64::MAX as u64) as i64);
            append_assistant_event(
                storage,
                task.tenant_id,
                task_context.assistant_run_id,
                "codex_host_task.poll_retry",
                json!({
                    "mode": "cloudflare_orchestrator",
                    "status": "processing",
                    "reason": "cloudflare_orchestrator_poll_timeout",
                    "retryable": true,
                    "attempt": task.attempt,
                    "max_attempts": task.max_attempts,
                    "retry_delay_ms": retry_delay_ms,
                    "available_at": available_at.to_rfc3339(),
                    "error": error_message,
                    "secrets_exposed": false,
                }),
            )
            .await?;
            storage
                .workflow_tasks()
                .requeue_after_transient_error(task.id, &error_message, available_at, now)
                .await?;
            tracing::warn!(
                task_id = %task.id,
                execution_id = %task.execution_id,
                attempt = task.attempt,
                max_attempts = task.max_attempts,
                retry_delay_ms,
                "Cloudflare Codex poll timed out; task requeued for continued polling"
            );
            return Ok(());
        }
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

async fn maybe_record_external_static_page_publish_completed_from_task_output(
    storage: &PgStorage,
    tenant_id: domain_model::TenantId,
    assistant_run_id: AssistantRunId,
    workflow_execution_id: WorkflowExecutionId,
    task_context: &CodexHostTaskContext,
    task_output: &Value,
) -> Result<()> {
    let Some(fixed_task) = task_context.fixed_task.as_ref() else {
        return Ok(());
    };
    if fixed_task.template_id.as_str() != "static_page_image2_data_publish" {
        return Ok(());
    }
    let Some(fixed_task_output) = task_output
        .get("fixed_task_output")
        .filter(|value| value.is_object())
    else {
        return Ok(());
    };
    if fixed_task_output.get("status").and_then(Value::as_str) != Some("success") {
        return Ok(());
    }
    let Some(public_url) = fixed_task_output
        .pointer("/artifact/public_url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| generated_artifact_url_allowed(value))
        .map(str::to_string)
    else {
        return Ok(());
    };

    let existing_events = storage
        .assistant_runs()
        .list_events(tenant_id, assistant_run_id)
        .await?;
    if existing_events.iter().any(|event| {
        event.event_name == "assistant_run.external_channel_static_page_publish_completed"
            && (event
                .payload
                .get("codex_host_workflow_execution_id")
                .and_then(Value::as_str)
                == Some(workflow_execution_id.to_string().as_str())
                || event.payload.get("public_url").and_then(Value::as_str)
                    == Some(public_url.as_str()))
    }) {
        return Ok(());
    }

    let source_refs =
        external_static_page_source_refs_for_fixed_task(storage, tenant_id, fixed_task).await?;
    let completed_payload = external_static_page_completed_payload_from_fixed_task_output(
        workflow_execution_id,
        fixed_task,
        fixed_task_output,
        &public_url,
        source_refs,
    );

    attach_external_static_page_artifact_to_run(
        storage,
        tenant_id,
        assistant_run_id,
        &completed_payload,
    )
    .await?;
    append_assistant_event(
        storage,
        tenant_id,
        assistant_run_id,
        "assistant_run.external_channel_static_page_publish_completed",
        completed_payload,
    )
    .await?;
    Ok(())
}

async fn external_static_page_source_refs_for_fixed_task(
    storage: &PgStorage,
    tenant_id: domain_model::TenantId,
    fixed_task: &contracts::CodexHostFixedTaskTemplateContextView,
) -> Result<Value> {
    if let Some(draft_id) = fixed_task
        .draft_id
        .as_deref()
        .and_then(|value| Uuid::parse_str(value).ok())
        .map(StaticPageDraftId)
    {
        if let Some(draft) = storage
            .static_page_drafts()
            .get_by_id(tenant_id, draft_id)
            .await?
        {
            return Ok(external_static_page_filtered_source_refs(
                &draft.source_refs,
            ));
        }
    }

    let mut refs = Map::new();
    for key in [
        "channel_connection_id",
        "platform",
        "tenant_external_id",
        "bot_external_id",
        "conversation_external_id",
        "thread_external_id",
        "sender_external_id",
        "message_external_id",
        "output_format",
        "render_mode",
    ] {
        if let Some(value) = fixed_task
            .requirements
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            refs.insert(key.to_string(), Value::String(value.to_string()));
        }
    }
    if !refs.contains_key("channel_connection_id") {
        if let Some(value) = fixed_task
            .trace_summary
            .pointer("/external_channel/connection_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            refs.insert(
                "channel_connection_id".to_string(),
                Value::String(value.to_string()),
            );
        }
    }
    for (key, pointer) in [
        (
            "conversation_external_id",
            "/external_channel/conversation_external_id",
        ),
        (
            "message_external_id",
            "/external_channel/message_external_id",
        ),
    ] {
        if refs.contains_key(key) {
            continue;
        }
        if let Some(value) = fixed_task
            .trace_summary
            .pointer(pointer)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            refs.insert(key.to_string(), Value::String(value.to_string()));
        }
    }
    Ok(Value::Object(refs))
}

fn external_static_page_filtered_source_refs(source_refs: &Value) -> Value {
    let mut output = Map::new();
    for key in [
        "channel_connection_id",
        "platform",
        "tenant_external_id",
        "bot_external_id",
        "conversation_external_id",
        "thread_external_id",
        "sender_external_id",
        "message_external_id",
        "output_format",
        "render_mode",
    ] {
        if let Some(value) = source_refs
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            output.insert(key.to_string(), Value::String(value.to_string()));
        }
    }
    Value::Object(output)
}

fn external_static_page_completed_payload_from_fixed_task_output(
    workflow_execution_id: WorkflowExecutionId,
    fixed_task: &contracts::CodexHostFixedTaskTemplateContextView,
    fixed_task_output: &Value,
    public_url: &str,
    source_refs: Value,
) -> Value {
    let validation_summary =
        external_static_page_validation_summary_from_fixed_output(fixed_task_output);
    json!({
        "channel_connection_id": source_refs.get("channel_connection_id").cloned().unwrap_or(Value::Null),
        "platform": source_refs.get("platform").cloned().unwrap_or(Value::Null),
        "conversation_external_id": source_refs
            .get("conversation_external_id")
            .cloned()
            .unwrap_or(Value::Null),
        "message_external_id": source_refs
            .get("message_external_id")
            .cloned()
            .unwrap_or(Value::Null),
        "draft_id": fixed_task.draft_id.clone(),
        "image_job_id": fixed_task
            .image2
            .get("image_job_id")
            .cloned()
            .unwrap_or(Value::Null),
        "codex_host_workflow_execution_id": workflow_execution_id.to_string(),
        "template_id": "static_page_image2_data_publish",
        "publish_mode": fixed_task
            .policies
            .get("publish_mode")
            .and_then(Value::as_str)
            .unwrap_or("new_generated_artifact_only"),
        "public_url": public_url,
        "artifact_links": [public_url],
        "local_path": fixed_task_output
            .pointer("/artifact/local_path")
            .cloned()
            .unwrap_or(Value::Null),
        "manifest_path": fixed_task_output
            .pointer("/artifact/manifest_path")
            .cloned()
            .unwrap_or(Value::Null),
        "validation_summary": validation_summary,
        "source_refs": source_refs,
    })
}

fn external_static_page_validation_summary_from_fixed_output(fixed_task_output: &Value) -> Value {
    let report = fixed_task_output
        .get("validation_report")
        .unwrap_or(&Value::Null);
    json!({
        "status": fixed_task_output.get("status").cloned().unwrap_or(Value::Null),
        "reason": "new_generated_artifact_validated",
        "latest_snapshot": report.get("latest_snapshot").cloned().unwrap_or(Value::Null),
        "source_row_count": report.get("source_row_count").cloned().unwrap_or(Value::Null),
        "current_state_row_count": report
            .get("current_state_row_count")
            .cloned()
            .unwrap_or(Value::Null),
        "detail_row_count": report.get("detail_row_count").cloned().unwrap_or(Value::Null),
        "unit_policy": report.get("unit_policy").cloned().unwrap_or(Value::Null),
        "warnings": report
            .get("warnings")
            .cloned()
            .unwrap_or_else(|| json!([])),
    })
}

async fn attach_external_static_page_artifact_to_run(
    storage: &PgStorage,
    tenant_id: domain_model::TenantId,
    assistant_run_id: AssistantRunId,
    completed_payload: &Value,
) -> Result<()> {
    let Some(run) = storage
        .assistant_runs()
        .get_by_id(tenant_id, assistant_run_id)
        .await?
    else {
        return Ok(());
    };
    let Some(public_url) = completed_payload
        .get("public_url")
        .and_then(Value::as_str)
        .filter(|value| generated_artifact_url_allowed(value))
    else {
        return Ok(());
    };
    let mut output_artifacts = run
        .output_artifacts
        .as_array()
        .cloned()
        .unwrap_or_else(Vec::new);
    if output_artifacts.iter().any(|artifact| {
        artifact.get("type").and_then(Value::as_str)
            == Some("external_channel_static_page_artifact")
            && artifact.get("public_url").and_then(Value::as_str) == Some(public_url)
    }) {
        return Ok(());
    }
    output_artifacts.push(json!({
        "type": "external_channel_static_page_artifact",
        "artifact_type": "static_page",
        "title": "static_page_image2_data_publish",
        "public_url": public_url,
        "download_url": public_url,
        "draft_id": completed_payload.get("draft_id").cloned().unwrap_or(Value::Null),
        "image_job_id": completed_payload.get("image_job_id").cloned().unwrap_or(Value::Null),
        "codex_host_workflow_execution_id": completed_payload
            .get("codex_host_workflow_execution_id")
            .cloned()
            .unwrap_or(Value::Null),
        "validation_summary": completed_payload
            .get("validation_summary")
            .cloned()
            .unwrap_or(Value::Null),
    }));
    storage
        .assistant_runs()
        .attach_output_artifacts(tenant_id, assistant_run_id, &Value::Array(output_artifacts))
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

async fn run_cloudflare_orchestrator_with_heartbeat(
    storage: &PgStorage,
    tenant_id: domain_model::TenantId,
    execution_id: domain_model::WorkflowExecutionId,
    workflow_task_id: domain_model::WorkflowTaskId,
    workflow_task_payload: &Value,
    task_context: &CodexHostTaskContext,
    decision: &CodexHostExecutionDecision,
    runtime_config: &CodexHostRuntimeConfig,
) -> Result<serde_json::Value> {
    let started_at = Instant::now();
    let mut heartbeat_interval =
        tokio::time::interval(Duration::from_millis(runtime_config.heartbeat_ms()));
    let mut heartbeat_count = 0usize;
    let exec = run_cloudflare_orchestrator(
        storage,
        workflow_task_id,
        workflow_task_payload,
        task_context,
        decision,
        execution_id,
        runtime_config,
    );
    tokio::pin!(exec);

    loop {
        tokio::select! {
            result = &mut exec => return result,
            _ = heartbeat_interval.tick() => {
                heartbeat_count += 1;
                if codex_host_execution_cancelled(storage, tenant_id, execution_id).await? {
                    return Err(anyhow!("Cloudflare Codex task cancelled before completion"));
                }
                let payload = cloudflare_orchestrator_heartbeat_payload(
                    task_context,
                    decision,
                    started_at.elapsed().as_millis() as u64,
                    heartbeat_count,
                );
                if let Err(error) = append_assistant_event(
                    storage,
                    tenant_id,
                    task_context.assistant_run_id,
                    "codex_host_task.cloudflare_heartbeat",
                    payload,
                )
                .await
                {
                    tracing::warn!(
                        error = ?error,
                        execution_id = %execution_id,
                        "failed to append Cloudflare Codex heartbeat event"
                    );
                }
            }
        }
    }
}

fn cloudflare_orchestrator_heartbeat_payload(
    task_context: &CodexHostTaskContext,
    decision: &CodexHostExecutionDecision,
    elapsed_ms: u64,
    heartbeat_count: usize,
) -> Value {
    json!({
        "mode": "cloudflare_orchestrator",
        "status": "running",
        "assistant_run_id": task_context.assistant_run_id.to_string(),
        "capability": task_context.capability.clone(),
        "host_kind": decision.host_kind.clone(),
        "profile": decision.profile.safe_summary(),
        "elapsed_ms": elapsed_ms,
        "heartbeat_count": heartbeat_count,
        "raw_prompt_exposed": false,
        "remote_result_exposed": false,
        "secrets_exposed": false,
    })
}

fn should_requeue_cloudflare_orchestrator_poll(
    mode: &CodexHostExecutionMode,
    error_message: &str,
    task: &domain_model::WorkflowTask,
) -> bool {
    matches!(mode, CodexHostExecutionMode::CloudflareOrchestrator)
        && task.attempt < task.max_attempts
        && cloudflare_orchestrator_poll_timeout_error(error_message)
}

fn cloudflare_orchestrator_poll_timeout_error(error_message: &str) -> bool {
    error_message.contains("Cloudflare Codex task timed out after")
}

fn cloudflare_orchestrator_retry_delay_ms() -> u64 {
    env_u64(
        "CODEX_ORCHESTRATOR_RETRY_DELAY_MS",
        DEFAULT_ORCHESTRATOR_RETRY_DELAY_MS,
    )
}

#[derive(Clone, Debug)]
struct CloudflareOrchestratorConfig {
    base_url: String,
    api_path: String,
    access_key: String,
    runtime_target_id: String,
    project_id: Option<String>,
    source: String,
    kind: String,
    poll_interval_ms: u64,
}

impl CloudflareOrchestratorConfig {
    fn from_env() -> Result<Self> {
        let access_key = if let Some(value) = std::env::var("CODEX_ORCHESTRATOR_ACCESS_KEY")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        {
            value
        } else {
            read_orchestrator_key_file()?.ok_or_else(|| {
                anyhow!(
                    "CODEX_ORCHESTRATOR_ACCESS_KEY or CODEX_ORCHESTRATOR_KEY_FILE is required for cloudflare_orchestrator mode"
                )
            })?
        };
        Ok(Self {
            base_url: env_or_default("CODEX_ORCHESTRATOR_BASE_URL", DEFAULT_ORCHESTRATOR_BASE_URL)
                .trim_end_matches('/')
                .to_string(),
            api_path: env_or_default("CODEX_ORCHESTRATOR_API_PATH", DEFAULT_ORCHESTRATOR_API_PATH),
            access_key,
            runtime_target_id: env_or_default(
                "CODEX_ORCHESTRATOR_RUNTIME_TARGET",
                DEFAULT_ORCHESTRATOR_RUNTIME_TARGET,
            ),
            project_id: std::env::var("CODEX_ORCHESTRATOR_PROJECT_ID")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            source: env_or_default("CODEX_ORCHESTRATOR_SOURCE", DEFAULT_ORCHESTRATOR_SOURCE),
            kind: env_or_default("CODEX_ORCHESTRATOR_KIND", DEFAULT_ORCHESTRATOR_KIND),
            poll_interval_ms: env_u64(
                "CODEX_ORCHESTRATOR_POLL_INTERVAL_MS",
                DEFAULT_ORCHESTRATOR_POLL_INTERVAL_MS,
            ),
        })
    }

    fn endpoint(&self, suffix: &str) -> String {
        format!(
            "{}/{}{}",
            self.base_url.trim_end_matches('/'),
            self.api_path.trim_matches('/'),
            suffix
        )
    }
}

fn read_orchestrator_key_file() -> Result<Option<String>> {
    let Some(path) = std::env::var("CODEX_ORCHESTRATOR_KEY_FILE")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
    else {
        return Ok(None);
    };
    let raw = fs::read_to_string(&path)
        .map_err(|error| anyhow!("failed to read CODEX_ORCHESTRATOR_KEY_FILE: {error}"))?;
    Ok(orchestrator_access_key_from_file_text(&raw))
}

fn orchestrator_access_key_from_file_text(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        for pointer in [
            "/keys/rawKey",
            "/keys/accessKey",
            "/rawKey",
            "/accessKey",
            "/access_key",
            "/key",
            "/token",
        ] {
            if let Some(value) = value
                .pointer(pointer)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                return Some(value.to_string());
            }
        }
    }
    trimmed
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
}

fn cloudflare_orchestrator_task_id_from_payload(payload: &Value) -> Option<String> {
    payload
        .pointer("/cloudflare_orchestrator/task_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn workflow_task_payload_with_cloudflare_orchestrator_task(
    payload: &Value,
    orchestrator_task_id: &str,
    submitted_task: &Value,
    submitted_at: chrono::DateTime<Utc>,
) -> Value {
    let mut root = payload.as_object().cloned().unwrap_or_default();
    let mut orchestrator = root
        .get("cloudflare_orchestrator")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    orchestrator.insert(
        "task_id".to_string(),
        Value::String(orchestrator_task_id.to_string()),
    );
    orchestrator.insert(
        "submitted_at".to_string(),
        Value::String(submitted_at.to_rfc3339()),
    );
    for (key, pointer) in [
        ("status", "/status"),
        ("kind", "/kind"),
        ("source", "/source"),
        ("runtime_target_id", "/runtimeTargetId"),
    ] {
        if let Some(value) = submitted_task.pointer(pointer) {
            orchestrator.insert(key.to_string(), value.clone());
        }
    }
    root.insert(
        "cloudflare_orchestrator".to_string(),
        Value::Object(orchestrator),
    );
    Value::Object(root)
}

async fn run_cloudflare_orchestrator(
    storage: &PgStorage,
    workflow_task_id: domain_model::WorkflowTaskId,
    workflow_task_payload: &Value,
    task_context: &CodexHostTaskContext,
    decision: &CodexHostExecutionDecision,
    execution_id: domain_model::WorkflowExecutionId,
    runtime_config: &CodexHostRuntimeConfig,
) -> Result<serde_json::Value> {
    let config = CloudflareOrchestratorConfig::from_env()?;
    let task_id = if let Some(task_id) =
        cloudflare_orchestrator_task_id_from_payload(workflow_task_payload)
    {
        task_id
    } else {
        let prompt = build_cloudflare_orchestrator_prompt(task_context)?;
        let submitted =
            submit_cloudflare_orchestrator_task(&config, task_context, execution_id, &prompt)
                .await?;
        let task_id = submitted
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("Cloudflare Codex response missing task.id"))?
            .to_string();
        let updated_payload = workflow_task_payload_with_cloudflare_orchestrator_task(
            workflow_task_payload,
            &task_id,
            &submitted,
            Utc::now(),
        );
        storage
            .workflow_tasks()
            .update_payload(workflow_task_id, &updated_payload, Utc::now())
            .await?;
        task_id
    };
    let completed =
        poll_cloudflare_orchestrator_task(&config, &task_id, runtime_config.task_timeout_ms())
            .await?;
    let fixed_task_output = task_context
        .fixed_task
        .as_ref()
        .map(|fixed_task| {
            let text = cloudflare_orchestrator_result_text(&completed);
            let output = extract_fixed_task_output_from_stdout(
                text.as_bytes(),
                fixed_task.template_id.as_str(),
            )?;
            normalize_cloudflare_fixed_task_output(output, task_context, execution_id, &task_id)
        })
        .transpose()?;
    Ok(cloudflare_orchestrator_output(
        task_context,
        decision,
        &task_id,
        fixed_task_output,
    ))
}

fn build_cloudflare_orchestrator_prompt(task_context: &CodexHostTaskContext) -> Result<String> {
    if let Some(fixed_task) = task_context.fixed_task.as_ref() {
        let task_json = fixed_task_json_for_orchestrator_prompt(fixed_task)?;
        let schema_hint = fixed_task_output_schema_hint(fixed_task.template_id.as_str());
        let mut prompt = format!(
            "Run the V3 fixed Cloudflare Codex task template `{}`.\n\nRules:\n- Return exactly one final JSON object.\n- Do not wrap the final JSON in Markdown fences.\n- Do not expose credentials, provider logs, raw database URLs, or raw customer documents.\n- If the task cannot satisfy the no-confirm policy, return status `needs_human` with a bounded `human_review_reason`.\n\nExpected output schema:\n{}\n\nTask package JSON:\n{}",
            fixed_task.template_id.as_str(),
            schema_hint,
            task_json
        );
        if fixed_task.template_id.as_str() == "static_page_image2_data_publish" {
            prompt.push_str(
                "\n\nCloudflare runtime cannot write V3 server files directly. For this template, if you cannot produce an approved V3 `artifact.public_url`, return `artifact.html` as a complete standalone HTML document. The V3 host-agent will publish it under `/generated-artifacts/` and replace it with `artifact.public_url`.",
            );
        }
        return Ok(prompt);
    }
    task_context
        .task
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("Cloudflare Codex task text is required"))
}

fn fixed_task_json_for_orchestrator_prompt(
    fixed_task: &contracts::CodexHostFixedTaskTemplateContextView,
) -> Result<String> {
    let task_value = if fixed_task.template_id.as_str() == "static_page_image2_data_publish" {
        compact_static_page_image2_fixed_task_for_orchestrator(fixed_task)
    } else {
        json!(fixed_task)
    };
    let full = serde_json::to_string_pretty(&task_value)
        .map_err(|error| anyhow!("failed to serialize fixed task package: {error}"))?;
    if full.chars().count() <= ORCHESTRATOR_FIXED_TASK_PROMPT_LIMIT_CHARS {
        return Ok(full);
    }
    let bounded = bounded_orchestrator_prompt_value(&task_value, 0);
    let bounded_text = serde_json::to_string_pretty(&bounded)
        .map_err(|error| anyhow!("failed to serialize bounded fixed task package: {error}"))?;
    if bounded_text.chars().count() <= ORCHESTRATOR_FIXED_TASK_PROMPT_LIMIT_CHARS {
        return Ok(bounded_text);
    }
    let tight = tightly_bounded_orchestrator_prompt_value(&task_value, 0);
    let tight_text = serde_json::to_string_pretty(&tight).map_err(|error| {
        anyhow!("failed to serialize tightly bounded fixed task package: {error}")
    })?;
    if tight_text.chars().count() <= ORCHESTRATOR_FIXED_TASK_PROMPT_LIMIT_CHARS {
        return Ok(tight_text);
    }
    let minimal = minimal_fixed_task_for_orchestrator(fixed_task);
    serde_json::to_string_pretty(&minimal)
        .map_err(|error| anyhow!("failed to serialize minimal fixed task package: {error}"))
}

fn compact_static_page_image2_fixed_task_for_orchestrator(
    fixed_task: &contracts::CodexHostFixedTaskTemplateContextView,
) -> Value {
    json!({
        "template_id": fixed_task.template_id.as_str(),
        "version": fixed_task.version,
        "assistant_run_id": fixed_task.assistant_run_id,
        "draft_id": fixed_task.draft_id,
        "dataset_scope": bounded_orchestrator_prompt_value(&fixed_task.dataset_scope, 2),
        "requirements": {
            "user_goal": compact_value_string_field(&fixed_task.requirements, "user_goal", 900),
            "project_name": fixed_task.requirements.get("project_name").cloned().unwrap_or(Value::Null),
            "workflow": fixed_task.requirements.get("workflow").cloned().unwrap_or(Value::Null),
            "source": fixed_task.requirements.get("source").cloned().unwrap_or(Value::Null),
            "platform": fixed_task.requirements.get("platform").cloned().unwrap_or(Value::Null),
            "conversation_external_id": fixed_task.requirements.get("conversation_external_id").cloned().unwrap_or(Value::Null),
            "message_external_id": fixed_task.requirements.get("message_external_id").cloned().unwrap_or(Value::Null),
            "artifact_type": fixed_task.requirements.get("artifact_type").cloned().unwrap_or(Value::Null),
            "artifact_template": bounded_orchestrator_prompt_value(
                fixed_task.requirements.get("artifact_template").unwrap_or(&Value::Null),
                2,
            ),
            "output_format": fixed_task.requirements.get("output_format").cloned().unwrap_or(Value::Null),
            "render_mode": fixed_task.requirements.get("render_mode").cloned().unwrap_or(Value::Null),
            "requested_skills": bounded_orchestrator_prompt_value(
                fixed_task.requirements.get("requested_skills").unwrap_or(&Value::Null),
                2,
            ),
            "time_dimension_required": fixed_task.requirements.get("time_dimension_required").cloned().unwrap_or(Value::Null),
            "primary_partition_required": fixed_task.requirements.get("primary_partition_required").cloned().unwrap_or(Value::Null),
            "detail_table_required": fixed_task.requirements.get("detail_table_required").cloned().unwrap_or(Value::Null),
            "evidence_summary": value_excerpt_for_orchestrator(
                fixed_task.requirements.get("evidence_summary"),
                700,
            ),
            "missing_evidence": value_excerpt_for_orchestrator(
                fixed_task.requirements.get("missing_evidence"),
                500,
            ),
        },
        "image2": {
            "prompt_text": compact_value_string_field(&fixed_task.image2, "prompt_text", 1200),
            "image_job_id": fixed_task.image2.get("image_job_id").cloned().unwrap_or(Value::Null),
            "image_job_status": fixed_task.image2.get("image_job_status").cloned().unwrap_or(Value::Null),
            "visual_contract_status": fixed_task.image2.get("visual_contract_status").cloned().unwrap_or(Value::Null),
            "visual_contract_url": fixed_task.image2.get("visual_contract_url").cloned().unwrap_or(Value::Null),
            "preview_asset_key": fixed_task.image2.get("preview_asset_key").cloned().unwrap_or(Value::Null),
            "human_confirmation_required": fixed_task.image2.get("human_confirmation_required").cloned().unwrap_or(Value::Null),
            "image_prompt_payload": compact_image_prompt_payload_for_orchestrator(
                fixed_task.image2.get("image_prompt_payload"),
            ),
        },
        "policies": bounded_orchestrator_prompt_value(&fixed_task.policies, 2),
        "evidence_summary": value_excerpt_for_orchestrator(Some(&fixed_task.evidence_summary), 700),
        "trace_summary": {
            "external_channel": fixed_task
                .trace_summary
                .get("external_channel")
                .cloned()
                .unwrap_or(Value::Null),
            "draft_id": fixed_task.trace_summary.get("draft_id").cloned().unwrap_or(Value::Null),
            "image_job_id": fixed_task.trace_summary.get("image_job_id").cloned().unwrap_or(Value::Null),
        },
        "human_review_policy": fixed_task.human_review_policy,
    })
}

fn compact_image_prompt_payload_for_orchestrator(value: Option<&Value>) -> Value {
    let Some(value) = value else {
        return Value::Null;
    };
    let modules = value
        .get("modules")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .take(5)
                .map(|module| {
                    json!({
                        "id": module.get("id").cloned().unwrap_or(Value::Null),
                        "role": module.get("role").cloned().unwrap_or(Value::Null),
                        "title": module.get("title").cloned().unwrap_or(Value::Null),
                        "layout": module.get("layout").cloned().unwrap_or(Value::Null),
                        "content": compact_value_string_field(module, "content", 180),
                        "visualization": module.get("visualization").cloned().unwrap_or(Value::Null),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({
        "title": value.get("title").cloned().unwrap_or(Value::Null),
        "prompt_text": compact_value_string_field(value, "prompt_text", 500),
        "style_direction": value.get("style_direction").cloned().or_else(|| value.get("styleDirection").cloned()).unwrap_or(Value::Null),
        "module_count": value.get("module_count").cloned().or_else(|| value.get("moduleCount").cloned()).unwrap_or(Value::Null),
        "modules": modules,
        "preview_contract_excerpt": value_excerpt_for_orchestrator(
            value.get("preview_contract_excerpt").or_else(|| value.get("preview_contract")).or_else(|| value.get("previewContract")),
            600,
        ),
        "data_snapshot_excerpt": value_excerpt_for_orchestrator(
            value.get("data_snapshot_excerpt").or_else(|| value.get("data_snapshot")).or_else(|| value.get("dataSnapshot")),
            600,
        ),
    })
}

fn compact_value_string_field(value: &Value, field: &str, max_chars: usize) -> Value {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(|text| Value::String(truncate_chars(text, max_chars)))
        .unwrap_or_else(|| value.get(field).cloned().unwrap_or(Value::Null))
}

fn value_excerpt_for_orchestrator(value: Option<&Value>, max_chars: usize) -> Value {
    let Some(value) = value else {
        return Value::Null;
    };
    let text = serde_json::to_string(value).unwrap_or_else(|_| value.to_string());
    Value::String(truncate_chars(&text, max_chars))
}

fn bounded_orchestrator_prompt_value(value: &Value, depth: usize) -> Value {
    if depth >= 6 {
        return match value {
            Value::String(text) => Value::String(truncate_chars(text, 400)),
            Value::Array(items) => json!({
                "truncated": true,
                "original_len": items.len(),
                "items": items
                    .iter()
                    .take(3)
                    .map(|item| bounded_orchestrator_prompt_value(item, depth + 1))
                    .collect::<Vec<_>>()
            }),
            Value::Object(map) => json!({
                "truncated": true,
                "keys": map.keys().take(12).cloned().collect::<Vec<_>>()
            }),
            other => other.clone(),
        };
    }
    match value {
        Value::String(text) => Value::String(truncate_chars(text, 1_200)),
        Value::Array(items) => {
            let mut bounded = items
                .iter()
                .take(12)
                .map(|item| bounded_orchestrator_prompt_value(item, depth + 1))
                .collect::<Vec<_>>();
            if items.len() > bounded.len() {
                let remaining_items = items.len() - bounded.len();
                bounded.push(json!({
                    "truncated": true,
                    "remaining_items": remaining_items
                }));
            }
            Value::Array(bounded)
        }
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| {
                    (
                        key.clone(),
                        bounded_orchestrator_prompt_value(value, depth + 1),
                    )
                })
                .collect(),
        ),
        other => other.clone(),
    }
}

fn tightly_bounded_orchestrator_prompt_value(value: &Value, depth: usize) -> Value {
    if depth >= 4 {
        return match value {
            Value::String(text) => Value::String(truncate_chars(text, 220)),
            Value::Array(items) => json!({
                "truncated": true,
                "original_len": items.len(),
                "items": items
                    .iter()
                    .take(2)
                    .map(|item| tightly_bounded_orchestrator_prompt_value(item, depth + 1))
                    .collect::<Vec<_>>()
            }),
            Value::Object(map) => json!({
                "truncated": true,
                "keys": map.keys().take(8).cloned().collect::<Vec<_>>()
            }),
            other => other.clone(),
        };
    }
    match value {
        Value::String(text) => Value::String(truncate_chars(text, 500)),
        Value::Array(items) => {
            let mut bounded = items
                .iter()
                .take(5)
                .map(|item| tightly_bounded_orchestrator_prompt_value(item, depth + 1))
                .collect::<Vec<_>>();
            if items.len() > bounded.len() {
                bounded.push(json!({
                    "truncated": true,
                    "remaining_items": items.len() - bounded.len()
                }));
            }
            Value::Array(bounded)
        }
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| {
                    (
                        key.clone(),
                        tightly_bounded_orchestrator_prompt_value(value, depth + 1),
                    )
                })
                .collect(),
        ),
        other => other.clone(),
    }
}

fn minimal_fixed_task_for_orchestrator(
    fixed_task: &contracts::CodexHostFixedTaskTemplateContextView,
) -> Value {
    json!({
        "template_id": fixed_task.template_id.as_str(),
        "version": fixed_task.version,
        "assistant_run_id": fixed_task.assistant_run_id,
        "draft_id": fixed_task.draft_id,
        "requirements": {
            "user_goal": compact_value_string_field(&fixed_task.requirements, "user_goal", 900),
            "project_name": fixed_task.requirements.get("project_name").cloned().unwrap_or(Value::Null),
            "output_format": fixed_task.requirements.get("output_format").cloned().unwrap_or(Value::Null),
            "render_mode": fixed_task.requirements.get("render_mode").cloned().unwrap_or(Value::Null),
            "evidence_summary_excerpt": value_excerpt_for_orchestrator(
                fixed_task.requirements.get("evidence_summary"),
                500,
            ),
        },
        "image2": {
            "prompt_text": compact_value_string_field(&fixed_task.image2, "prompt_text", 1000),
            "image_job_id": fixed_task.image2.get("image_job_id").cloned().unwrap_or(Value::Null),
            "visual_contract_status": fixed_task.image2.get("visual_contract_status").cloned().unwrap_or(Value::Null),
            "preview_asset_key": fixed_task.image2.get("preview_asset_key").cloned().unwrap_or(Value::Null),
            "image_prompt_payload_excerpt": value_excerpt_for_orchestrator(
                fixed_task.image2.get("image_prompt_payload"),
                900,
            ),
        },
        "policies": {
            "publish_mode": fixed_task.policies.get("publish_mode").cloned().unwrap_or(Value::Null),
            "effect_image_confirmation_required": fixed_task.policies.get("effect_image_confirmation_required").cloned().unwrap_or(Value::Null),
            "continue_to_publish_after_effect_image": fixed_task.policies.get("continue_to_publish_after_effect_image").cloned().unwrap_or(Value::Null),
        },
        "human_review_policy": fixed_task.human_review_policy,
    })
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut output = value.chars().take(max_chars).collect::<String>();
    output.push_str("...[truncated]");
    output
}

async fn curl_orchestrator_json(
    method: &str,
    url: &str,
    body: Option<&Value>,
    headers: &[(&str, String)],
) -> Result<(u16, String)> {
    let mut config = String::new();
    config.push_str("silent\nshow-error\nfail-with-body\n");
    config.push_str("location\n");
    config.push_str("write-out = \"\\n%{http_code}\"\n");
    config.push_str(&format!("request = \"{}\"\n", curl_config_escape(method)));
    config.push_str(&format!("url = \"{}\"\n", curl_config_escape(url)));
    for (name, value) in headers {
        config.push_str(&format!(
            "header = \"{}: {}\"\n",
            curl_config_escape(name),
            curl_config_escape(value)
        ));
    }
    if let Some(body) = body {
        let body_json = serde_json::to_string(body)
            .map_err(|error| anyhow!("failed to serialize Cloudflare Codex request: {error}"))?;
        config.push_str(&format!("data = \"{}\"\n", curl_config_escape(&body_json)));
    }

    let mut command = Command::new("curl");
    command
        .arg("--config")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command
        .spawn()
        .map_err(|error| anyhow!("failed to launch curl for Cloudflare Codex request: {error}"))?;
    let Some(mut stdin) = child.stdin.take() else {
        return Err(anyhow!("failed to open curl stdin"));
    };
    stdin
        .write_all(config.as_bytes())
        .await
        .map_err(|error| anyhow!("failed to write curl config: {error}"))?;
    drop(stdin);
    let output = child
        .wait_with_output()
        .await
        .map_err(|error| anyhow!("failed to wait for curl response: {error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr_len = safe_log_excerpt(&output.stderr, 400).chars().count();
    let (body_text, status_text) = stdout
        .rsplit_once('\n')
        .ok_or_else(|| anyhow!("curl response missing status code; stderr_chars={stderr_len}"))?;
    let status = status_text.trim().parse::<u16>().map_err(|error| {
        anyhow!("curl response status code is invalid: {error}; stderr_chars={stderr_len}")
    })?;
    if !output.status.success() && !(200..300).contains(&status) {
        return Ok((status, body_text.to_string()));
    }
    if !output.status.success() {
        return Err(anyhow!(
            "curl request failed despite HTTP status {}; stderr_chars={stderr_len}",
            status
        ));
    }
    Ok((status, body_text.to_string()))
}

fn curl_config_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

async fn submit_cloudflare_orchestrator_task(
    config: &CloudflareOrchestratorConfig,
    task_context: &CodexHostTaskContext,
    execution_id: domain_model::WorkflowExecutionId,
    prompt: &str,
) -> Result<Value> {
    let mut body = json!({
        "runtimeTargetId": config.runtime_target_id,
        "kind": config.kind,
        "source": config.source,
        "prompt": prompt,
        "metadata": {
            "title": format!("V3 {} {}", task_context.capability, task_context.assistant_run_id),
            "assistant_run_id": task_context.assistant_run_id.to_string(),
            "workflow_execution_id": execution_id.to_string(),
            "capability": task_context.capability.clone(),
            "output": "text",
        }
    });
    if let Some(project_id) = config.project_id.as_ref() {
        body["projectId"] = Value::String(project_id.clone());
    }
    let (status, text) = curl_orchestrator_json(
        "POST",
        &config.endpoint("/tasks"),
        Some(&body),
        &[
            ("Authorization", format!("Bearer {}", config.access_key)),
            ("Content-Type", "application/json".to_string()),
            ("X-Client-Name", "v3-codex-host-agent".to_string()),
            (
                "Idempotency-Key",
                format!("v3-codex-host:{execution_id}:{}", task_context.capability),
            ),
        ],
    )
    .await
    .map_err(|error| anyhow!("failed to submit Cloudflare Codex task: {error}"))?;
    if !(200..300).contains(&status) {
        let excerpt = safe_response_excerpt(&text, 300);
        return Err(anyhow!(
            "Cloudflare Codex submit failed: status={} body_chars={} body_excerpt=\"{}\"",
            status,
            text.chars().count(),
            excerpt
        ));
    }
    let value: Value = serde_json::from_str(&text)
        .map_err(|error| anyhow!("Cloudflare Codex submit response is invalid JSON: {error}"))?;
    value
        .get("task")
        .cloned()
        .ok_or_else(|| anyhow!("Cloudflare Codex submit response missing task"))
}

async fn poll_cloudflare_orchestrator_task(
    config: &CloudflareOrchestratorConfig,
    task_id: &str,
    timeout_ms: u64,
) -> Result<Value> {
    let started_at = Instant::now();
    loop {
        if started_at.elapsed() > Duration::from_millis(timeout_ms.max(1)) {
            return Err(anyhow!(
                "Cloudflare Codex task timed out after {}ms; task_id={}",
                timeout_ms.max(1),
                task_id
            ));
        }
        let (status_code, text) = curl_orchestrator_json(
            "GET",
            &config.endpoint(&format!("/tasks/{task_id}")),
            None,
            &[
                ("Authorization", format!("Bearer {}", config.access_key)),
                ("X-Client-Name", "v3-codex-host-agent".to_string()),
            ],
        )
        .await
        .map_err(|error| anyhow!("failed to poll Cloudflare Codex task: {error}"))?;
        if !(200..300).contains(&status_code) {
            let excerpt = safe_response_excerpt(&text, 300);
            return Err(anyhow!(
                "Cloudflare Codex poll failed: status={} body_chars={} body_excerpt=\"{}\"",
                status_code,
                text.chars().count(),
                excerpt
            ));
        }
        let value: Value = serde_json::from_str(&text)
            .map_err(|error| anyhow!("Cloudflare Codex poll response is invalid JSON: {error}"))?;
        let task = value
            .get("task")
            .cloned()
            .ok_or_else(|| anyhow!("Cloudflare Codex poll response missing task"))?;
        match task
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
        {
            "completed" => return Ok(task),
            "failed" | "cancelled" => {
                let code = task
                    .pointer("/error/code")
                    .and_then(Value::as_str)
                    .unwrap_or("cloudflare_codex_task_failed");
                let excerpt = safe_response_excerpt(&task.to_string(), 500);
                return Err(anyhow!(
                    "Cloudflare Codex task ended with status={code}; task_excerpt=\"{}\"",
                    excerpt
                ));
            }
            _ => {
                tokio::time::sleep(Duration::from_millis(config.poll_interval_ms.max(250))).await;
            }
        }
    }
}

fn safe_response_excerpt(text: &str, max_chars: usize) -> String {
    safe_log_excerpt(text.as_bytes(), max_chars)
        .trim()
        .escape_debug()
        .to_string()
}

fn cloudflare_orchestrator_result_text(task: &Value) -> String {
    for pointer in [
        "/result/text",
        "/result/finalMessage",
        "/result/message",
        "/output/text",
        "/finalMessage",
    ] {
        if let Some(text) = task.pointer(pointer).and_then(Value::as_str) {
            if !text.trim().is_empty() {
                return text.to_string();
            }
        }
    }
    task.to_string()
}

fn normalize_cloudflare_fixed_task_output(
    output: Value,
    task_context: &CodexHostTaskContext,
    execution_id: domain_model::WorkflowExecutionId,
    orchestrator_task_id: &str,
) -> Result<Value> {
    if output.get("template_id").and_then(Value::as_str) != Some("static_page_image2_data_publish")
        || output.get("status").and_then(Value::as_str) != Some("success")
    {
        return Ok(output);
    }
    let public_url = output
        .pointer("/artifact/public_url")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if generated_artifact_url_allowed(public_url) {
        return Ok(output);
    }
    let Some(html) = extract_static_page_html_from_fixed_output(&output) else {
        return Ok(output);
    };
    publish_cloudflare_static_page_html(
        output,
        &html,
        task_context,
        execution_id,
        orchestrator_task_id,
    )
}

fn extract_static_page_html_from_fixed_output(output: &Value) -> Option<String> {
    for pointer in [
        "/artifact/html",
        "/artifact/html_text",
        "/artifact/index_html",
        "/html",
        "/html_text",
    ] {
        if let Some(html) = output
            .pointer(pointer)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(standalone_html_document(html));
        }
    }
    None
}

fn standalone_html_document(html: &str) -> String {
    let trimmed = html.trim();
    let lower = trimmed
        .chars()
        .take(256)
        .collect::<String>()
        .to_ascii_lowercase();
    if lower.contains("<!doctype html") || lower.contains("<html") {
        trimmed.to_string()
    } else {
        format!(
            "<!doctype html><html lang=\"zh-CN\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>V3 Generated Artifact</title></head><body>{}</body></html>",
            trimmed
        )
    }
}

fn publish_cloudflare_static_page_html(
    mut output: Value,
    html: &str,
    task_context: &CodexHostTaskContext,
    execution_id: domain_model::WorkflowExecutionId,
    orchestrator_task_id: &str,
) -> Result<Value> {
    let run_segment = safe_path_segment(&task_context.assistant_run_id.to_string());
    let execution_segment = safe_path_segment(&execution_id.to_string());
    let task_segment = safe_path_segment(orchestrator_task_id);
    let relative_dir = format!(
        "database-static-pages/codex-host/{run_segment}/{execution_segment}-{task_segment}"
    );
    let artifact_dir = generated_artifact_root()?.join(&relative_dir);
    fs::create_dir_all(&artifact_dir).map_err(|error| {
        anyhow!(
            "failed to create Cloudflare Codex generated artifact dir {}: {error}",
            artifact_dir.display()
        )
    })?;
    let index_path = artifact_dir.join("index.html");
    fs::write(&index_path, html.as_bytes()).map_err(|error| {
        anyhow!(
            "failed to write Cloudflare Codex generated static-page HTML {}: {error}",
            index_path.display()
        )
    })?;
    let public_url = generated_artifact_public_url(&relative_dir);
    let manifest = json!({
        "kind": "v3_codex_host_generated_static_page",
        "version": 1,
        "assistant_run_id": task_context.assistant_run_id.to_string(),
        "workflow_execution_id": execution_id.to_string(),
        "orchestrator_task_id": orchestrator_task_id,
        "capability": task_context.capability.clone(),
        "public_url": public_url.clone(),
        "created_at": Utc::now(),
    });
    let manifest_path = artifact_dir.join("manifest.json");
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest)
            .map_err(|error| anyhow!("failed to serialize generated artifact manifest: {error}"))?,
    )
    .map_err(|error| {
        anyhow!(
            "failed to write Cloudflare Codex generated artifact manifest {}: {error}",
            manifest_path.display()
        )
    })?;

    let object = output
        .as_object_mut()
        .ok_or_else(|| anyhow!("fixed task output must be a JSON object"))?;
    let artifact = object
        .entry("artifact".to_string())
        .or_insert_with(|| json!({}));
    if !artifact.is_object() {
        *artifact = json!({});
    }
    if let Some(artifact_object) = artifact.as_object_mut() {
        artifact_object.remove("html");
        artifact_object.remove("html_text");
        artifact_object.remove("index_html");
        artifact_object.insert(
            "local_path".to_string(),
            Value::String(index_path.display().to_string()),
        );
        artifact_object.insert("public_url".to_string(), Value::String(public_url));
        artifact_object.insert(
            "manifest_path".to_string(),
            Value::String(manifest_path.display().to_string()),
        );
    }
    object.remove("html");
    object.remove("html_text");
    Ok(output)
}

fn generated_artifact_root() -> Result<PathBuf> {
    let root = std::env::var("V3_GENERATED_ARTIFACT_ROOT")
        .ok()
        .map(|value| PathBuf::from(value.trim()))
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var("PLATFORM_LOCAL_OBJECT_ROOT")
                .ok()
                .map(|value| PathBuf::from(value.trim()))
                .filter(|path| path.is_absolute())
                .map(|path| path.join("generated-artifacts"))
        })
        .unwrap_or_else(|| {
            std::env::temp_dir()
                .join("ai-data-platform-v3-objects")
                .join("generated-artifacts")
        });
    fs::create_dir_all(&root).map_err(|error| {
        anyhow!(
            "failed to create generated artifact root {}: {error}",
            root.display()
        )
    })?;
    Ok(root)
}

fn generated_artifact_public_base_url() -> String {
    std::env::var("V3_GENERATED_ARTIFACT_PUBLIC_BASE_URL")
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "https://v3.elepcloud.com/generated-artifacts".to_string())
}

fn generated_artifact_public_url(relative_dir: &str) -> String {
    format!(
        "{}/{}/index.html",
        generated_artifact_public_base_url(),
        relative_dir.trim_matches('/')
    )
}

fn generated_artifact_url_allowed(public_url: &str) -> bool {
    let normalized = public_url.trim();
    if normalized.contains("/generated-artifacts/pending-")
        || normalized.contains("/generated-artifacts/pending/")
        || normalized.ends_with("/generated-artifacts/pending")
    {
        return false;
    }
    normalized.starts_with("https://v3.elepcloud.com/generated-artifacts/")
        || normalized.starts_with("/generated-artifacts/")
}

fn safe_path_segment(value: &str) -> String {
    let safe = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .chars()
        .take(96)
        .collect::<String>();
    if safe.is_empty() {
        "artifact".to_string()
    } else {
        safe
    }
}

fn env_or_default(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn cloudflare_orchestrator_output(
    task_context: &CodexHostTaskContext,
    decision: &CodexHostExecutionDecision,
    orchestrator_task_id: &str,
    fixed_task_output: Option<Value>,
) -> serde_json::Value {
    let process_output = CodexProcessOutput {
        exit_code: Some(0),
        stdout_excerpt: String::new(),
        stderr_excerpt: String::new(),
    };
    let html_artifacts = vec![task_context.html_report_artifact(
        "cloudflare_orchestrator",
        "completed",
        Some(decision),
        Some(&process_output),
    )];
    let mut output = json!(CodexHostTaskOutputView {
        mode: "cloudflare_orchestrator".to_string(),
        codex_invoked: true,
        status: "completed".to_string(),
        host_kind: Some(decision.host_kind.clone()),
        assistant_run_id: task_context.assistant_run_id.to_string(),
        capability: task_context.capability.clone(),
        profile: Some(decision.profile.safe_summary()),
        command_plan: None,
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
    });
    if let Some(object) = output.as_object_mut() {
        object.insert(
            "orchestrator_task_id".to_string(),
            Value::String(orchestrator_task_id.to_string()),
        );
    }
    output
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
        Some("cloudflare_orchestrator") => "codex_host_task.exec_completed",
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
    use std::sync::{Mutex, OnceLock};
    use uuid::Uuid;

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
    fn cloudflare_static_page_html_output_is_published_to_generated_artifacts() {
        let _lock = test_env_lock().lock().expect("env lock");
        let artifact_root = std::env::temp_dir()
            .join("v3-codex-host-test-artifacts")
            .join(Uuid::new_v4().to_string());
        let _root = TestEnvVarRestore::set(
            "V3_GENERATED_ARTIFACT_ROOT",
            artifact_root.display().to_string(),
        );
        let _base = TestEnvVarRestore::set(
            "V3_GENERATED_ARTIFACT_PUBLIC_BASE_URL",
            "https://v3.elepcloud.com/generated-artifacts",
        );
        let assistant_run_id = AssistantRunId::new();
        let task_context = CodexHostTaskContext {
            assistant_run_id,
            capability: "static_page_image2_data_publish".to_string(),
            task: Some("Run fixed static-page package".to_string()),
            local_thread_id: None,
            task_memory_isolated: true,
            task_memory_space_id: Some("codex-host-task:static-page".to_string()),
            fixed_task: Some(
                contracts::CodexHostFixedTaskTemplateContextView::static_page_image2_data_publish_example(),
            ),
        };
        let execution_id = domain_model::WorkflowExecutionId::new();
        let output = json!({
            "template_id": "static_page_image2_data_publish",
            "status": "success",
            "artifact": {
                "public_url": "https://v3.elepcloud.com/generated-artifacts/pending-host-publication",
                "html": "<main><h1>经营分析</h1></main>"
            },
            "validation_report": {
                "source_row_count": 1
            }
        });

        let normalized = normalize_cloudflare_fixed_task_output(
            output,
            &task_context,
            execution_id,
            "task_test",
        )
        .expect("output should normalize");

        let public_url = normalized
            .pointer("/artifact/public_url")
            .and_then(Value::as_str)
            .expect("public url");
        assert!(public_url.starts_with("https://v3.elepcloud.com/generated-artifacts/"));
        assert!(public_url.contains(&assistant_run_id.to_string()));
        assert!(!public_url.contains("pending-host-agent-publication"));
        assert!(normalized.pointer("/artifact/html").is_none());
        let local_path = normalized
            .pointer("/artifact/local_path")
            .and_then(Value::as_str)
            .expect("local path");
        assert!(std::path::Path::new(local_path).exists());
        let html = std::fs::read_to_string(local_path).expect("html should be readable");
        assert!(html.contains("<!doctype html>"));
        assert!(html.contains("经营分析"));
    }

    #[test]
    fn generated_artifact_url_allowed_rejects_pending_placeholders() {
        assert!(!generated_artifact_url_allowed(
            "https://v3.elepcloud.com/generated-artifacts/pending/draft-1/"
        ));
        assert!(!generated_artifact_url_allowed(
            "https://v3.elepcloud.com/generated-artifacts/pending-draft-1/index.html"
        ));
        assert!(generated_artifact_url_allowed(
            "https://v3.elepcloud.com/generated-artifacts/database-static-pages/codex-host/run/task/index.html"
        ));
    }

    #[test]
    fn external_static_page_completed_payload_keeps_public_summary_only() {
        let mut fixed_task =
            contracts::CodexHostFixedTaskTemplateContextView::static_page_image2_data_publish_example();
        fixed_task.draft_id = Some("draft-123".to_string());
        fixed_task.image2["image_job_id"] = json!("image-job-123");
        fixed_task.policies["publish_mode"] = json!("new_generated_artifact_only");
        let output = json!({
            "template_id": "static_page_image2_data_publish",
            "status": "success",
            "artifact": {
                "local_path": "/srv/aiv3/shared/objects/generated-artifacts/database-static-pages/final/index.html",
                "public_url": "https://v3.elepcloud.com/generated-artifacts/database-static-pages/final/index.html",
                "manifest_path": "/srv/aiv3/shared/objects/generated-artifacts/database-static-pages/final/manifest.json"
            },
            "validation_report": {
                "latest_snapshot": "2026-05-10",
                "source_row_count": 862,
                "current_state_row_count": 851,
                "detail_row_count": 40,
                "unit_policy": "validate_raw_value_then_choose_wan_or_yi",
                "warnings": ["已按最新快照口径输出"]
            }
        });
        let source_refs = json!({
            "channel_connection_id": "generic-chat-main",
            "platform": "generic_chat",
            "conversation_external_id": "conv-1",
            "message_external_id": "msg-1",
            "selected_scope": {"must": "not leak"}
        });
        let workflow_execution_id = WorkflowExecutionId::new();

        let payload = external_static_page_completed_payload_from_fixed_task_output(
            workflow_execution_id,
            &fixed_task,
            &output,
            "https://v3.elepcloud.com/generated-artifacts/database-static-pages/final/index.html",
            external_static_page_filtered_source_refs(&source_refs),
        );

        assert_eq!(
            payload["codex_host_workflow_execution_id"],
            json!(workflow_execution_id.to_string())
        );
        assert_eq!(payload["draft_id"], json!("draft-123"));
        assert_eq!(payload["image_job_id"], json!("image-job-123"));
        assert_eq!(
            payload["source_refs"]["conversation_external_id"],
            json!("conv-1")
        );
        assert_eq!(
            payload["validation_summary"]["source_row_count"],
            json!(862)
        );
        assert!(!payload.to_string().contains("must"));
    }

    #[test]
    fn cloudflare_orchestrator_fixed_task_prompt_is_hard_bounded() {
        let mut fixed_task =
            contracts::CodexHostFixedTaskTemplateContextView::static_page_image2_data_publish_example();
        fixed_task.requirements["user_goal"] = json!("生成经营分析报表。".repeat(2_000));
        fixed_task.requirements["evidence_summary"] = json!({
            "rows": (0..80)
                .map(|index| json!({
                    "name": format!("门店-{index}"),
                    "note": "长证据文本".repeat(200),
                }))
                .collect::<Vec<_>>()
        });
        fixed_task.image2["prompt_text"] = json!("视觉提示词".repeat(2_000));
        fixed_task.image2["image_prompt_payload"] = json!({
            "title": "新世界项目经营分析",
            "modules": (0..30)
                .map(|index| json!({
                    "id": format!("module-{index}"),
                    "role": "metric",
                    "title": format!("指标模块 {index}"),
                    "content": "模块内容".repeat(300),
                    "visualization": {"kind": "bar", "notes": "图表说明".repeat(100)},
                }))
                .collect::<Vec<_>>(),
            "data_snapshot": "数据快照".repeat(2_000),
            "preview_contract": "预览契约".repeat(2_000),
        });
        fixed_task.trace_summary = json!({
            "external_channel": {"conversation_external_id": "conv-1"},
            "execution_trail": "执行轨迹".repeat(2_000),
        });

        let prompt_json =
            fixed_task_json_for_orchestrator_prompt(&fixed_task).expect("prompt should serialize");
        let parsed: Value =
            serde_json::from_str(&prompt_json).expect("prompt json should be valid");

        assert!(prompt_json.chars().count() <= ORCHESTRATOR_FIXED_TASK_PROMPT_LIMIT_CHARS);
        assert_eq!(
            parsed["template_id"],
            json!("static_page_image2_data_publish")
        );
        assert!(prompt_json.contains("preview_asset_key"));
    }

    #[test]
    fn cloudflare_orchestrator_error_excerpt_is_safe_and_bounded() {
        let excerpt = safe_response_excerpt(
            "Authorization: Bearer secret-token\n{\"error\":\"任务内容过长，请简化\"}",
            80,
        );

        assert!(excerpt.contains("[redacted-log-line]"));
        assert!(excerpt.contains("任务内容过长"));
        assert!(!excerpt.contains("secret-token"));
    }

    #[test]
    fn cloudflare_orchestrator_event_name_uses_exec_completed_bucket() {
        assert_eq!(
            codex_host_task_event_name(&json!({"mode": "cloudflare_orchestrator"})),
            "codex_host_task.exec_completed"
        );
    }

    #[test]
    fn cloudflare_orchestrator_key_file_text_accepts_json_or_plain_secret() {
        assert_eq!(
            orchestrator_access_key_from_file_text(
                &json!({"keys": {"rawKey": "json-secret"}}).to_string()
            ),
            Some("json-secret".to_string())
        );
        assert_eq!(
            orchestrator_access_key_from_file_text("# comment\n plain-secret \n"),
            Some("plain-secret".to_string())
        );
    }

    #[test]
    fn cloudflare_orchestrator_task_payload_persists_resume_id_safely() {
        let payload = json!({"execution_id": "exec-1", "kind": "codex_host_task_workflow"});
        let updated = workflow_task_payload_with_cloudflare_orchestrator_task(
            &payload,
            "task-cloudflare-1",
            &json!({
                "status": "running",
                "kind": "static-page-visual",
                "source": "ai-data-platform-static-pages",
                "runtimeTargetId": "cloudflare",
                "prompt": "should-not-be-persisted"
            }),
            Utc::now(),
        );

        assert_eq!(
            cloudflare_orchestrator_task_id_from_payload(&updated).as_deref(),
            Some("task-cloudflare-1")
        );
        assert_eq!(
            updated["cloudflare_orchestrator"]["runtime_target_id"],
            json!("cloudflare")
        );
        assert!(updated["cloudflare_orchestrator"].get("prompt").is_none());
    }

    #[test]
    fn cloudflare_orchestrator_timeout_requeue_requires_attempt_budget() {
        let mut task = test_workflow_task(1, 3);

        assert!(should_requeue_cloudflare_orchestrator_poll(
            &CodexHostExecutionMode::CloudflareOrchestrator,
            "Cloudflare Codex task timed out after 1800000ms; task_id=abc",
            &task
        ));

        task.attempt = 3;
        assert!(!should_requeue_cloudflare_orchestrator_poll(
            &CodexHostExecutionMode::CloudflareOrchestrator,
            "Cloudflare Codex task timed out after 1800000ms; task_id=abc",
            &task
        ));
        task.attempt = 1;
        assert!(!should_requeue_cloudflare_orchestrator_poll(
            &CodexHostExecutionMode::CodexExec,
            "Cloudflare Codex task timed out after 1800000ms; task_id=abc",
            &task
        ));
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
            codex_host_task_event_name(&json!({"mode": "cloudflare_orchestrator"})),
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

    fn test_env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn test_workflow_task(attempt: u32, max_attempts: u32) -> domain_model::WorkflowTask {
        let now = Utc::now();
        domain_model::WorkflowTask {
            id: domain_model::WorkflowTaskId::new(),
            tenant_id: domain_model::TenantId::new(),
            execution_id: domain_model::WorkflowExecutionId::new(),
            queue: "codex_host".to_string(),
            task_key: "run_codex_host_task".to_string(),
            payload: json!({}),
            status: domain_model::WorkflowTaskStatus::Claimed,
            attempt,
            max_attempts,
            available_at: now,
            claimed_at: Some(now),
            finished_at: None,
            error: None,
            created_at: now,
            updated_at: now,
        }
    }

    struct TestEnvVarRestore {
        key: &'static str,
        previous: Option<String>,
    }

    impl TestEnvVarRestore {
        fn set(key: &'static str, value: impl Into<String>) -> Self {
            let previous = std::env::var(key).ok();
            std::env::set_var(key, value.into());
            Self { key, previous }
        }
    }

    impl Drop for TestEnvVarRestore {
        fn drop(&mut self) {
            if let Some(previous) = self.previous.as_ref() {
                std::env::set_var(self.key, previous);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }
}
