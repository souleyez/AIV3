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
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use storage::{NewAssistantRunEvent, NewWorkflowTask, PgStorage, DEFAULT_LOCAL_DATABASE_URL};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::Semaphore;
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
const DEFAULT_ORCHESTRATOR_USER_AGENT: &str = "v3-codex-host-agent/1.0";
const DEFAULT_ORCHESTRATOR_KIND: &str = "code-task";
const DEFAULT_ORCHESTRATOR_RETRY_DELAY_MS: u64 = 15_000;
const ORCHESTRATOR_FIXED_TASK_PROMPT_LIMIT_CHARS: usize = 6_500;
const STATIC_PAGE_IMAGE2_DATA_PUBLISH: &str = "static_page_image2_data_publish";
const DEFAULT_CODEX_HOST_CONCURRENCY: usize = 1;
const MAX_CODEX_HOST_CONCURRENCY: usize = 8;

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
    let worker_concurrency = parse_codex_host_concurrency(
        std::env::var("CODEX_HOST_CLOUDFLARE_CONCURRENCY")
            .ok()
            .or_else(|| std::env::var("CODEX_HOST_AGENT_CONCURRENCY").ok())
            .as_deref(),
    );
    let execution_policy = CodexHostAgentPolicy::from_env()?;
    let runtime_config = CodexHostRuntimeConfig::from_env();

    let storage = PgStorage::connect_with_configured_max_connections(
        &database_url,
        "CODEX_HOST_DATABASE_MAX_CONNECTIONS",
    )
    .await?;
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
        worker_concurrency,
        execution_mode = %execution_policy.mode.as_str(),
        profile_id = %execution_policy.profile.id,
        profile_kind = %execution_policy.profile.kind,
        host_kind = %execution_policy.host_kind,
        task_timeout_ms = runtime_config.task_timeout_ms(),
        heartbeat_ms = runtime_config.heartbeat_ms(),
        stdout_limit_bytes = runtime_config.stdout_limit_bytes(),
        stderr_limit_bytes = runtime_config.stderr_limit_bytes(),
        task_workspace_retention_hours = runtime_config.task_workspace_retention_hours(),
        "codex-host-agent polling started"
    );

    let task_permits = Arc::new(Semaphore::new(worker_concurrency));
    loop {
        let permit = match Arc::clone(&task_permits).acquire_owned().await {
            Ok(permit) => permit,
            Err(error) => {
                tracing::error!(error = ?error, "codex host agent semaphore closed");
                wait_for_next_task_signal(&mut task_waker, poll_interval).await;
                continue;
            }
        };
        match storage
            .workflow_tasks()
            .claim_next_available(&queue, Some(&task_key), Utc::now())
            .await
        {
            Ok(Some(task)) => {
                let storage = storage.clone();
                let workflow_catalog = workflow_catalog.clone();
                let event_bus = event_bus.clone();
                let execution_policy = execution_policy.clone();
                let runtime_config = runtime_config.clone();
                tokio::spawn(async move {
                    let _permit = permit;
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
                });
            }
            Ok(None) => {
                drop(permit);
                wait_for_next_task_signal(&mut task_waker, poll_interval).await;
            }
            Err(error) => {
                drop(permit);
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
    if execution.kind != WorkflowKind::CodexHostTask {
        return Err(anyhow!(
            "workflow execution {} has unexpected kind {}",
            execution.id,
            execution.kind.as_str()
        ));
    }
    let task_context = CodexHostTaskContext::from_execution(&execution)?;
    if execution.status == domain_model::WorkflowStatus::Cancelled {
        append_assistant_event(
            storage,
            task.tenant_id,
            task_context.assistant_run_id,
            "codex_host_task.cancelled",
            codex_host_task_cancelled_payload(
                execution_policy.mode.as_str(),
                &task_context,
                &task,
                "workflow_cancelled_before_start",
                None,
            ),
        )
        .await?;
        storage
            .workflow_tasks()
            .mark_failed(task.id, "workflow_cancelled_before_start", Utc::now())
            .await?;
        return Ok(());
    }

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
        maybe_record_external_data_ingestion_analysis_result_from_task_output(
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
        if let Some(cancel_reason) = codex_host_cancelled_error_reason(&error_message) {
            append_assistant_event(
                storage,
                task.tenant_id,
                task_context.assistant_run_id,
                "codex_host_task.cancelled",
                codex_host_task_cancelled_payload(
                    execution_policy.mode.as_str(),
                    &task_context,
                    &task,
                    cancel_reason,
                    Some(&error_message),
                ),
            )
            .await?;
            storage
                .workflow_tasks()
                .mark_failed(task.id, cancel_reason, Utc::now())
                .await?;
            tracing::info!(
                task_id = %task.id,
                execution_id = %task.execution_id,
                reason = cancel_reason,
                "codex host task cancelled"
            );
            return Ok(());
        }
        if should_requeue_cloudflare_orchestrator_poll(
            &execution_policy.mode,
            &error_message,
            &task,
        ) {
            let now = Utc::now();
            let retry_delay_ms = cloudflare_orchestrator_retry_delay_ms();
            let available_at =
                now + chrono::Duration::milliseconds(retry_delay_ms.min(i64::MAX as u64) as i64);
            let retry_reason = cloudflare_orchestrator_retry_reason(&error_message);
            let logical_queue = cloudflare_orchestrator_logical_queue(&task_context);
            let logical_task_key = cloudflare_orchestrator_poll_logical_task_key(&task_context);
            let current_payload = latest_workflow_task_payload(storage, &task).await?;
            let updated_payload = workflow_task_payload_with_cloudflare_orchestrator_poll(
                &current_payload,
                retry_reason,
                logical_queue,
                logical_task_key,
                task.attempt,
                retry_delay_ms,
                available_at,
                now,
                Some(&error_message),
            );
            let remote_task_id = cloudflare_orchestrator_task_id_from_payload(&updated_payload);
            storage
                .workflow_tasks()
                .update_payload(task.id, &updated_payload, now)
                .await?;
            append_assistant_event(
                storage,
                task.tenant_id,
                task_context.assistant_run_id,
                "codex_host_task.poll_retry",
                json!({
                    "mode": "cloudflare_orchestrator",
                    "status": "processing",
                    "reason": retry_reason,
                    "retryable": true,
                    "attempt": task.attempt,
                    "max_attempts": task.max_attempts,
                    "retry_delay_ms": retry_delay_ms,
                    "available_at": available_at.to_rfc3339(),
                    "task_id": remote_task_id,
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
                reason = retry_reason,
                "Cloudflare Codex task requeued for non-blocking continued polling"
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

async fn latest_workflow_task_payload(
    storage: &PgStorage,
    task: &domain_model::WorkflowTask,
) -> Result<Value> {
    let latest = storage
        .workflow_tasks()
        .list_by_execution(task.execution_id)
        .await?
        .into_iter()
        .find(|candidate| candidate.id == task.id)
        .map(|candidate| candidate.payload)
        .unwrap_or_else(|| task.payload.clone());
    Ok(latest)
}

fn codex_host_cancelled_error_reason(error_message: &str) -> Option<&'static str> {
    if error_message.contains("Codex Host command cancelled before completion")
        || error_message.contains("Cloudflare Codex task cancelled before completion")
    {
        Some("workflow_cancelled_during_execution")
    } else {
        None
    }
}

fn codex_host_task_cancelled_payload(
    mode: &str,
    task_context: &CodexHostTaskContext,
    task: &domain_model::WorkflowTask,
    reason: &str,
    error_message: Option<&str>,
) -> Value {
    json!({
        "mode": mode,
        "status": "cancelled",
        "reason": reason,
        "assistant_run_id": task_context.assistant_run_id.to_string(),
        "capability": task_context.capability.clone(),
        "template_id": task_context
            .fixed_task
            .as_ref()
            .map(|fixed_task| fixed_task.template_id.as_str())
            .unwrap_or(task_context.capability.as_str()),
        "workflow_execution_id": task.execution_id.to_string(),
        "workflow_task_id": task.id.to_string(),
        "attempt": task.attempt,
        "max_attempts": task.max_attempts,
        "retryable": false,
        "error": error_message
            .map(|message| safe_response_excerpt(message, 240))
            .unwrap_or_default(),
        "raw_prompt_exposed": false,
        "stdout_exposed": false,
        "stderr_exposed": false,
        "secrets_exposed": false,
    })
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
        .filter(|value| {
            generated_static_page_public_url_claim_is_host_published(fixed_task_output, value)
        })
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

async fn maybe_record_external_data_ingestion_analysis_result_from_task_output(
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
    if fixed_task.template_id.as_str() != "data_ingestion_analysis" {
        return Ok(());
    }
    let Some(fixed_task_output) = task_output
        .get("fixed_task_output")
        .filter(|value| value.is_object())
    else {
        return Ok(());
    };

    let event_name = data_ingestion_analysis_assistant_event_name(fixed_task_output);
    let existing_events = storage
        .assistant_runs()
        .list_events(tenant_id, assistant_run_id)
        .await?;
    if existing_events.iter().any(|event| {
        matches!(
            event.event_name.as_str(),
            "assistant_run.data_ingestion_analysis_completed"
                | "assistant_run.data_ingestion_analysis_needs_human"
                | "assistant_run.data_ingestion_analysis_failed"
        ) && event
            .payload
            .get("codex_host_workflow_execution_id")
            .and_then(Value::as_str)
            == Some(workflow_execution_id.to_string().as_str())
    }) {
        return Ok(());
    }

    append_assistant_event(
        storage,
        tenant_id,
        assistant_run_id,
        event_name,
        external_data_ingestion_analysis_payload_from_fixed_task_output(
            workflow_execution_id,
            fixed_task_output,
        ),
    )
    .await?;
    Ok(())
}

fn data_ingestion_analysis_assistant_event_name(output: &Value) -> &'static str {
    match output
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("failed")
    {
        "analysis_ready" | "staging_spec_ready" => {
            "assistant_run.data_ingestion_analysis_completed"
        }
        "needs_human" => "assistant_run.data_ingestion_analysis_needs_human",
        _ => "assistant_run.data_ingestion_analysis_failed",
    }
}

fn external_data_ingestion_analysis_payload_from_fixed_task_output(
    workflow_execution_id: WorkflowExecutionId,
    output: &Value,
) -> Value {
    let staging_spec = output.get("staging_spec").cloned().unwrap_or(Value::Null);
    let staging_plan_available = !staging_spec.is_null();
    json!({
        "template_id": "data_ingestion_analysis",
        "status": output.get("status").cloned().unwrap_or(Value::String("failed".to_string())),
        "codex_host_workflow_execution_id": workflow_execution_id.to_string(),
        "result_summary": external_data_ingestion_result_summary(output),
        "staging_plan": if staging_plan_available {
            json!({
                "type": "v3_data_ingestion_staging_plan",
                "plan_id": format!("staging-plan-{}", workflow_execution_id),
                "codex_host_workflow_execution_id": workflow_execution_id.to_string(),
                "template_id": "data_ingestion_analysis",
                "staging_spec": bounded_public_value(&staging_spec, 8),
                "production_write_allowed": false,
                "requires_human_confirmation": true,
            })
        } else {
            Value::Null
        },
        "staging_plan_available": staging_plan_available,
        "staging_spec_available": staging_plan_available,
        "human_review_required": output.get("status").and_then(Value::as_str) != Some("analysis_ready"),
        "production_write_allowed": false,
        "raw_credentials_exposed": false,
        "raw_table_dump_exposed": false,
        "validation": {
            "status": output.get("status").cloned().unwrap_or(Value::String("failed".to_string())),
            "auto_apply_allowed": false,
        },
    })
}

fn external_data_ingestion_result_summary(output: &Value) -> Value {
    let data_quality_report = output.get("data_quality_report").unwrap_or(&Value::Null);
    let mapping_plan = output.get("mapping_plan").unwrap_or(&Value::Null);
    let staging_spec = output.get("staging_spec").unwrap_or(&Value::Null);
    json!({
        "status": output.get("status").cloned().unwrap_or(Value::String("failed".to_string())),
        "source_summary": safe_public_string_array(output.get("source_summary"), 6, 260),
        "data_quality_report": {
            "row_count": data_quality_report.get("row_count").cloned().unwrap_or(Value::Null),
            "warnings": safe_public_string_array(data_quality_report.get("warnings"), 8, 220),
            "quality_notes": safe_public_string_array(data_quality_report.get("quality_notes"), 8, 220),
        },
        "mapping_plan_summary": {
            "field_count": mapping_plan
                .get("fields")
                .and_then(Value::as_array)
                .map(|items| items.len())
                .unwrap_or(0),
            "available": !mapping_plan.is_null(),
        },
        "staging_spec_summary": {
            "available": !staging_spec.is_null(),
            "target": staging_spec.get("target").cloned().unwrap_or(Value::Null),
            "step_count": staging_spec
                .get("steps")
                .and_then(Value::as_array)
                .map(|items| items.len())
                .unwrap_or(0),
            "write_policy": "requires_human_confirmation",
        },
        "staging_spec_available": !staging_spec.is_null(),
        "validation_checks": safe_public_string_array(output.get("validation_checks"), 12, 220),
        "recommended_next_actions": safe_public_string_array(output.get("recommended_next_actions"), 8, 260),
        "human_review_reason": output
            .get("human_review_reason")
            .and_then(Value::as_str)
            .map(|value| safe_public_text(value, 360)),
        "production_write_allowed": false,
        "raw_credentials_exposed": false,
        "raw_table_dump_exposed": false,
    })
}

fn safe_public_string_array(
    value: Option<&Value>,
    max_items: usize,
    max_chars: usize,
) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(|value| safe_public_text(value, max_chars))
                .filter(|value| !value.is_empty())
                .take(max_items)
                .collect()
        })
        .unwrap_or_default()
}

fn safe_public_text(value: &str, max_chars: usize) -> String {
    let excerpt = safe_log_excerpt(value.as_bytes(), max_chars);
    excerpt.trim().to_string()
}

fn bounded_public_value(value: &Value, max_depth: usize) -> Value {
    if max_depth == 0 {
        return Value::String("[truncated]".to_string());
    }
    match value {
        Value::Array(items) => Value::Array(
            items
                .iter()
                .take(40)
                .map(|item| bounded_public_value(item, max_depth - 1))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.iter()
                .take(40)
                .map(|(key, value)| (key.clone(), bounded_public_value(value, max_depth - 1)))
                .collect(),
        ),
        Value::String(text) => Value::String(safe_public_text(text, 500)),
        _ => value.clone(),
    }
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
    let exec = run_codex_exec(
        execution_id,
        command_plan,
        task_context,
        decision,
        runtime_config,
    );
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
        && cloudflare_orchestrator_requeueable_error(error_message)
        && (cloudflare_orchestrator_pending_progress_error(error_message)
            || task.attempt < task.max_attempts)
}

fn cloudflare_orchestrator_pending_progress_error(error_message: &str) -> bool {
    let message = error_message.to_ascii_lowercase();
    message.contains("cloudflare codex task submitted and pending")
        || message.contains("cloudflare codex task still running")
}

fn cloudflare_orchestrator_requeueable_error(error_message: &str) -> bool {
    let message = error_message.to_ascii_lowercase();
    if cloudflare_orchestrator_waf_blocked_error(&message)
        || cloudflare_orchestrator_auth_failed_error(&message)
    {
        return false;
    }
    cloudflare_orchestrator_pending_progress_error(error_message)
        || message.contains("cloudflare codex task timed out after")
        || message.contains("failed to poll cloudflare codex task")
        || message.contains("cloudflare codex poll response is invalid json")
        || message.contains("cloudflare codex poll response missing task")
        || message.contains("cloudflare codex poll failed: status=500")
        || message.contains("cloudflare codex poll failed: status=502")
        || message.contains("cloudflare codex poll failed: status=503")
        || message.contains("cloudflare codex poll failed: status=504")
}

fn cloudflare_orchestrator_retry_reason(error_message: &str) -> &'static str {
    let message = error_message.to_ascii_lowercase();
    if cloudflare_orchestrator_waf_blocked_error(&message) {
        "cloudflare_orchestrator_waf_blocked"
    } else if cloudflare_orchestrator_auth_failed_error(&message) {
        "cloudflare_orchestrator_auth_failed"
    } else if message.contains("cloudflare codex task submitted and pending") {
        "cloudflare_orchestrator_submitted"
    } else if message.contains("cloudflare codex task still running") {
        "cloudflare_orchestrator_pending"
    } else if message.contains("cloudflare codex task timed out after") {
        "cloudflare_orchestrator_poll_timeout"
    } else {
        "cloudflare_orchestrator_poll_transient"
    }
}

fn cloudflare_orchestrator_waf_blocked_error(lowercase_message: &str) -> bool {
    (lowercase_message.contains("status=403")
        || lowercase_message.contains("status: 403")
        || lowercase_message.contains("http 403"))
        && (lowercase_message.contains("1010")
            || lowercase_message.contains("browser_signature")
            || lowercase_message.contains("browser signature")
            || lowercase_message.contains("challenge")
            || lowercase_message.contains("attention required")
            || lowercase_message.contains("cf-error")
            || lowercase_message.contains("cloudflare ray"))
}

fn cloudflare_orchestrator_auth_failed_error(lowercase_message: &str) -> bool {
    (lowercase_message.contains("status=401")
        || lowercase_message.contains("status: 401")
        || lowercase_message.contains("http 401")
        || lowercase_message.contains("status=403")
        || lowercase_message.contains("status: 403")
        || lowercase_message.contains("http 403"))
        && !cloudflare_orchestrator_waf_blocked_error(lowercase_message)
}

fn cloudflare_orchestrator_retry_delay_ms() -> u64 {
    env_u64(
        "CODEX_ORCHESTRATOR_RETRY_DELAY_MS",
        DEFAULT_ORCHESTRATOR_RETRY_DELAY_MS,
    )
}

fn cloudflare_orchestrator_logical_queue(task_context: &CodexHostTaskContext) -> &'static str {
    if task_context.capability == STATIC_PAGE_IMAGE2_DATA_PUBLISH {
        "static_page_publish"
    } else {
        "codex_fixed_task"
    }
}

fn cloudflare_orchestrator_poll_logical_task_key(
    task_context: &CodexHostTaskContext,
) -> &'static str {
    if task_context.capability == STATIC_PAGE_IMAGE2_DATA_PUBLISH {
        "poll_static_page_publish"
    } else {
        "poll_codex_fixed_task"
    }
}

#[derive(Clone, Debug)]
struct CloudflareOrchestratorConfig {
    base_url: String,
    api_path: String,
    access_key: String,
    runtime_target_id: String,
    project_id: Option<String>,
    source: String,
    user_agent: String,
    kind: String,
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
            user_agent: env_or_default(
                "CODEX_ORCHESTRATOR_USER_AGENT",
                DEFAULT_ORCHESTRATOR_USER_AGENT,
            ),
            kind: env_or_default("CODEX_ORCHESTRATOR_KIND", DEFAULT_ORCHESTRATOR_KIND),
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

fn cloudflare_orchestrator_service_headers(
    config: &CloudflareOrchestratorConfig,
    content_type: Option<&'static str>,
    idempotency_key: Option<String>,
) -> Vec<(&'static str, String)> {
    let mut headers = vec![
        ("Authorization", format!("Bearer {}", config.access_key)),
        ("X-Client-Name", config.source.clone()),
        ("User-Agent", config.user_agent.clone()),
    ];
    if let Some(content_type) = content_type {
        headers.push(("Content-Type", content_type.to_string()));
    }
    if let Some(idempotency_key) = idempotency_key {
        headers.push(("Idempotency-Key", idempotency_key));
    }
    headers
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
    logical_queue: &str,
    next_logical_task_key: &str,
) -> Value {
    let mut root = payload.as_object().cloned().unwrap_or_default();
    root.insert(
        "logical_queue".to_string(),
        Value::String(logical_queue.to_string()),
    );
    root.insert(
        "logical_task_key".to_string(),
        Value::String(next_logical_task_key.to_string()),
    );
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

fn workflow_task_payload_with_cloudflare_orchestrator_poll(
    payload: &Value,
    reason: &str,
    logical_queue: &str,
    logical_task_key: &str,
    poll_attempt: u32,
    retry_delay_ms: u64,
    next_poll_at: chrono::DateTime<Utc>,
    last_poll_at: chrono::DateTime<Utc>,
    error_message: Option<&str>,
) -> Value {
    let mut root = payload.as_object().cloned().unwrap_or_default();
    root.insert(
        "logical_queue".to_string(),
        Value::String(logical_queue.to_string()),
    );
    root.insert(
        "logical_task_key".to_string(),
        Value::String(logical_task_key.to_string()),
    );
    let mut orchestrator = root
        .get("cloudflare_orchestrator")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    orchestrator.insert("status".to_string(), Value::String(reason.to_string()));
    orchestrator.insert(
        "last_poll_at".to_string(),
        Value::String(last_poll_at.to_rfc3339()),
    );
    orchestrator.insert(
        "poll_attempt".to_string(),
        Value::Number(serde_json::Number::from(u64::from(poll_attempt))),
    );
    orchestrator.insert(
        "retry_delay_ms".to_string(),
        Value::Number(serde_json::Number::from(retry_delay_ms)),
    );
    orchestrator.insert(
        "next_poll_at".to_string(),
        Value::String(next_poll_at.to_rfc3339()),
    );
    if let Some(error_message) = error_message {
        orchestrator.insert(
            "last_error".to_string(),
            Value::String(safe_response_excerpt(error_message, 300)),
        );
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
    _runtime_config: &CodexHostRuntimeConfig,
) -> Result<serde_json::Value> {
    let config = CloudflareOrchestratorConfig::from_env()?;
    let (task_id, submitted_this_claim) = if let Some(task_id) =
        cloudflare_orchestrator_task_id_from_payload(workflow_task_payload)
    {
        (task_id, false)
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
            cloudflare_orchestrator_logical_queue(task_context),
            cloudflare_orchestrator_poll_logical_task_key(task_context),
        );
        storage
            .workflow_tasks()
            .update_payload(workflow_task_id, &updated_payload, Utc::now())
            .await?;
        (task_id, true)
    };
    if submitted_this_claim {
        return Err(anyhow!(
            "Cloudflare Codex task submitted and pending; task_id={task_id}"
        ));
    }
    let polled = poll_cloudflare_orchestrator_task_once(&config, &task_id).await?;
    let status = polled
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    if status != "completed" {
        if matches!(status, "failed" | "cancelled") {
            let code = polled
                .pointer("/error/code")
                .and_then(Value::as_str)
                .unwrap_or("cloudflare_codex_task_failed");
            let excerpt = safe_response_excerpt(&polled.to_string(), 500);
            return Err(anyhow!(
                "Cloudflare Codex task ended with status={code}; task_excerpt=\"{}\"",
                excerpt
            ));
        }
        return Err(anyhow!(
            "Cloudflare Codex task still running; task_id={task_id} status={status}"
        ));
    }
    let fixed_task_output = task_context
        .fixed_task
        .as_ref()
        .map(|fixed_task| {
            let text = cloudflare_orchestrator_result_text(&polled);
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
                "\n\nStatic-page rules:\n- The GPT-Image-2 preview is the mandatory visual contract. Build the website from that image's layout, hierarchy, density, color, and module composition.\n- Do not return a simplified renderer page, demo-only placeholder, or visual-contract fallback as success.\n- Cloudflare runtime cannot write V3 server files directly. If you cannot produce an approved V3 `artifact.public_url`, return `artifact.html` as a complete standalone HTML document plus `artifact.data_json`; the V3 host-agent will publish it under `/generated-artifacts/` and replace it with `artifact.public_url`.\n- The final HTML must load local `data.json`, preserve time controls, primary partition controls, manual refresh, and auto refresh/change detection so database-backed data can be replaced without rewriting the page.\n- Bind real V3 dataset/database/document evidence from the task package; if selected data is unavailable or insufficient, return `needs_human` or `failed` instead of publishing a fallback page.",
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
            "render_asset_url": fixed_task.image2.get("render_asset_url").cloned().unwrap_or(Value::Null),
            "asset_provenance": compact_static_page_image2_asset_provenance_for_orchestrator(
                fixed_task.image2.get("asset_provenance"),
            ),
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
            "render_asset_url": fixed_task.image2.get("render_asset_url").cloned().unwrap_or(Value::Null),
            "asset_provenance": compact_static_page_image2_asset_provenance_for_orchestrator(
                fixed_task.image2.get("asset_provenance"),
            ),
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

fn compact_static_page_image2_asset_provenance_for_orchestrator(value: Option<&Value>) -> Value {
    let Some(value) = value else {
        return Value::Null;
    };
    json!({
        "schema": value.get("schema").and_then(Value::as_str).map(|item| truncate_chars(item, 80)).unwrap_or_else(|| "v3.static_page_preview_asset_provenance".to_string()),
        "schemaVersion": value.get("schemaVersion").or_else(|| value.get("schema_version")).and_then(Value::as_u64).map(Value::from).unwrap_or(Value::Null),
        "renderAssetUrl": value.get("renderAssetUrl").or_else(|| value.get("render_asset_url")).and_then(Value::as_str).map(safe_static_page_asset_ref_for_orchestrator).unwrap_or(Value::Null),
        "renderAssetPolicy": value.get("renderAssetPolicy").or_else(|| value.get("render_asset_policy")).and_then(Value::as_str).map(|item| Value::String(truncate_chars(item, 120))).unwrap_or(Value::Null),
        "sourceAssetKind": value.get("sourceAssetKind").or_else(|| value.get("source_asset_kind")).and_then(Value::as_str).map(|item| Value::String(truncate_chars(item, 80))).unwrap_or(Value::Null),
        "sourceAssetRef": value.get("sourceAssetRef").or_else(|| value.get("source_asset_ref")).and_then(Value::as_str).map(safe_static_page_asset_ref_for_orchestrator).unwrap_or(Value::Null),
        "sourceAssetRefRedacted": value.get("sourceAssetRefRedacted").or_else(|| value.get("source_asset_ref_redacted")).and_then(Value::as_bool).unwrap_or(true),
        "sourceAssetHadQuery": value.get("sourceAssetHadQuery").or_else(|| value.get("source_asset_had_query")).and_then(Value::as_bool).unwrap_or(false),
        "persisted": value.get("persisted").and_then(Value::as_bool).unwrap_or(false),
        "persistedPreviewAssetKey": value.get("persistedPreviewAssetKey").or_else(|| value.get("persisted_preview_asset_key")).and_then(Value::as_str).map(safe_static_page_asset_ref_for_orchestrator).unwrap_or(Value::Null),
        "storageStatus": value.get("storageStatus").or_else(|| value.get("storage_status")).and_then(Value::as_str).map(|item| Value::String(truncate_chars(item, 80))).unwrap_or(Value::Null),
        "byteSize": value.get("byteSize").or_else(|| value.get("byte_size")).and_then(Value::as_u64).map(Value::from).unwrap_or(Value::Null),
        "mimeType": value.get("mimeType").or_else(|| value.get("mime_type")).and_then(Value::as_str).map(|item| Value::String(truncate_chars(item, 80))).unwrap_or(Value::Null),
        "width": value.get("width").and_then(Value::as_i64).map(Value::from).unwrap_or(Value::Null),
        "height": value.get("height").and_then(Value::as_i64).map(Value::from).unwrap_or(Value::Null),
    })
}

fn safe_static_page_asset_ref_for_orchestrator(value: &str) -> Value {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.starts_with("data:image/") || trimmed.starts_with("blob:") {
        return Value::Null;
    }
    let without_fragment = trimmed.split('#').next().unwrap_or(trimmed);
    let without_query = without_fragment
        .split('?')
        .next()
        .unwrap_or(without_fragment);
    Value::String(truncate_chars(without_query, 500))
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
    let headers = cloudflare_orchestrator_service_headers(
        config,
        Some("application/json"),
        Some(format!(
            "v3-codex-host:{execution_id}:{}",
            task_context.capability
        )),
    );
    let (status, text) =
        curl_orchestrator_json("POST", &config.endpoint("/tasks"), Some(&body), &headers)
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

async fn poll_cloudflare_orchestrator_task_once(
    config: &CloudflareOrchestratorConfig,
    task_id: &str,
) -> Result<Value> {
    let headers = cloudflare_orchestrator_service_headers(config, None, None);
    let (status_code, text) = curl_orchestrator_json(
        "GET",
        &config.endpoint(&format!("/tasks/{task_id}")),
        None,
        &headers,
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
    value
        .get("task")
        .cloned()
        .ok_or_else(|| anyhow!("Cloudflare Codex poll response missing task"))
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
    let static_page_image2_task = task_context.fixed_task.as_ref().is_some_and(|fixed_task| {
        fixed_task.template_id.as_str() == "static_page_image2_data_publish"
    });
    if !static_page_image2_task {
        return Ok(output);
    }
    if output.get("template_id").and_then(Value::as_str) == Some("static_page_image2_data_publish")
        && output.get("status").and_then(Value::as_str) == Some("success")
    {
        let public_url = output
            .pointer("/artifact/public_url")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if generated_static_page_public_url_claim_is_host_published(&output, public_url) {
            let mut output = output;
            strip_static_page_inline_artifact_payload(&mut output)?;
            return Ok(output);
        }
        if let Some(output) = publish_workspace_static_page_artifact_if_available(
            &output,
            task_context,
            execution_id,
            orchestrator_task_id,
        )? {
            return Ok(output);
        }
        if let Some(html) = extract_static_page_html_from_fixed_output(&output) {
            let data_json = extract_static_page_data_json_from_fixed_output(&output);
            return publish_cloudflare_static_page_html(
                output,
                &html,
                data_json,
                task_context,
                execution_id,
                orchestrator_task_id,
            );
        }
        return Err(anyhow!(
            "static_page_image2_data_publish success output did not include a host-published artifact, task-workspace artifact, or complete artifact.html"
        ));
    }
    Ok(output)
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

fn extract_static_page_data_json_from_fixed_output(output: &Value) -> Option<Value> {
    for pointer in [
        "/artifact/data_json",
        "/artifact/data",
        "/artifact/data_snapshot",
        "/data_json",
        "/data",
        "/data_snapshot",
    ] {
        if let Some(data) = output
            .pointer(pointer)
            .and_then(normalize_static_page_data_json)
        {
            return Some(data);
        }
    }
    None
}

fn normalize_static_page_data_json(value: &Value) -> Option<Value> {
    match value {
        Value::Array(_) | Value::Object(_) => Some(value.clone()),
        Value::String(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return None;
            }
            serde_json::from_str::<Value>(trimmed)
                .ok()
                .filter(|value| value.is_array() || value.is_object())
        }
        _ => None,
    }
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

fn publish_workspace_static_page_artifact_if_available(
    output: &Value,
    task_context: &CodexHostTaskContext,
    execution_id: domain_model::WorkflowExecutionId,
    orchestrator_task_id: &str,
) -> Result<Option<Value>> {
    let public_url = output
        .pointer("/artifact/public_url")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    if !generated_artifact_url_allowed(public_url) {
        return Ok(None);
    }

    let Some(source_index_path) = output
        .pointer("/artifact/local_path")
        .and_then(Value::as_str)
        .and_then(|value| resolve_task_workspace_artifact_path(value, task_context))
    else {
        return Ok(None);
    };
    if !source_index_path.is_absolute() || !source_index_path.is_file() {
        return Ok(None);
    }
    if !local_artifact_path_is_under_task_workspace(&source_index_path)? {
        return Ok(None);
    }

    let Some(source_dir) = source_index_path.parent() else {
        return Ok(None);
    };
    let run_segment = safe_path_segment(&task_context.assistant_run_id.to_string());
    let execution_segment = safe_path_segment(&execution_id.to_string());
    let task_segment = safe_path_segment(orchestrator_task_id);
    let relative_dir = format!(
        "database-static-pages/codex-host/{run_segment}/{execution_segment}-{task_segment}"
    );
    let artifact_dir = generated_artifact_root()?.join(&relative_dir);
    copy_generated_artifact_dir(source_dir, &artifact_dir)?;

    let index_file_name = source_index_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("index.html");
    let index_path = artifact_dir.join(index_file_name);
    if !index_path.is_file() {
        return Err(anyhow!(
            "Codex Host workspace artifact copy did not produce {}",
            index_path.display()
        ));
    }
    validate_static_page_image2_dynamic_artifact(&index_path, &artifact_dir)?;

    let mut republished = output.clone();
    let object = republished
        .as_object_mut()
        .ok_or_else(|| anyhow!("fixed task output must be a JSON object"))?;
    let artifact = object
        .entry("artifact".to_string())
        .or_insert_with(|| json!({}));
    if !artifact.is_object() {
        *artifact = json!({});
    }
    if let Some(artifact_object) = artifact.as_object_mut() {
        artifact_object.insert(
            "local_path".to_string(),
            Value::String(index_path.display().to_string()),
        );
        artifact_object.insert(
            "public_url".to_string(),
            Value::String(generated_artifact_public_file_url(
                &relative_dir,
                index_file_name,
            )),
        );

        let manifest_path = artifact_dir.join("manifest.json");
        if manifest_path.is_file() {
            artifact_object.insert(
                "manifest_path".to_string(),
                Value::String(manifest_path.display().to_string()),
            );
        }
        let data_path = artifact_dir.join("data.json");
        if data_path.is_file() {
            artifact_object.insert(
                "data_path".to_string(),
                Value::String(data_path.display().to_string()),
            );
            artifact_object.insert(
                "data_url".to_string(),
                Value::String(generated_artifact_public_file_url(
                    &relative_dir,
                    "data.json",
                )),
            );
        }
        let data_snapshot_path = artifact_dir.join("data-snapshot.json");
        if data_snapshot_path.is_file() {
            artifact_object.insert(
                "data_snapshot_path".to_string(),
                Value::String(data_snapshot_path.display().to_string()),
            );
            artifact_object.insert(
                "data_snapshot_url".to_string(),
                Value::String(generated_artifact_public_file_url(
                    &relative_dir,
                    "data-snapshot.json",
                )),
            );
        }
    }
    strip_static_page_inline_artifact_payload(&mut republished)?;
    Ok(Some(republished))
}

fn resolve_task_workspace_artifact_path(
    raw_path: &str,
    task_context: &CodexHostTaskContext,
) -> Option<PathBuf> {
    let trimmed = raw_path.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(relative) = trimmed.strip_prefix("/workspace/") {
        return task_workspace_path(task_context).map(|workspace| workspace.join(relative));
    }
    if let Some(relative) = trimmed.strip_prefix("workspace/") {
        return task_workspace_path(task_context).map(|workspace| workspace.join(relative));
    }
    let path = PathBuf::from(trimmed);
    if path.is_absolute() {
        return Some(path);
    }
    task_workspace_path(task_context).map(|workspace| workspace.join(path))
}

fn task_workspace_path(task_context: &CodexHostTaskContext) -> Option<PathBuf> {
    let root = std::env::var("CODEX_HOST_AGENT_TASK_WORKSPACE_ROOT")
        .ok()
        .map(|value| PathBuf::from(value.trim()))
        .filter(|value| value.is_absolute())?;
    Some(root.join(task_workspace_label(task_context)))
}

fn task_workspace_label(task_context: &CodexHostTaskContext) -> String {
    let source = task_context
        .task_memory_space_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("codex-host-task-{}", task_context.assistant_run_id));
    let safe = source
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
        "codex-host-task".to_string()
    } else {
        safe
    }
}

fn local_artifact_path_is_under_task_workspace(path: &Path) -> Result<bool> {
    let Some(root) = std::env::var("CODEX_HOST_AGENT_TASK_WORKSPACE_ROOT")
        .ok()
        .map(|value| PathBuf::from(value.trim()))
        .filter(|value| value.is_absolute())
    else {
        return Ok(false);
    };
    let canonical_root = match fs::canonicalize(&root) {
        Ok(root) => root,
        Err(_) => return Ok(false),
    };
    let canonical_path = match fs::canonicalize(path) {
        Ok(path) => path,
        Err(_) => return Ok(false),
    };
    Ok(canonical_path.starts_with(canonical_root))
}

fn copy_generated_artifact_dir(source_dir: &Path, target_dir: &Path) -> Result<()> {
    fs::create_dir_all(target_dir).map_err(|error| {
        anyhow!(
            "failed to create generated artifact target dir {}: {error}",
            target_dir.display()
        )
    })?;
    for entry in fs::read_dir(source_dir).map_err(|error| {
        anyhow!(
            "failed to read generated artifact source dir {}: {error}",
            source_dir.display()
        )
    })? {
        let entry = entry.map_err(|error| {
            anyhow!(
                "failed to read generated artifact source entry {}: {error}",
                source_dir.display()
            )
        })?;
        let file_type = entry.file_type().map_err(|error| {
            anyhow!(
                "failed to read generated artifact entry type {}: {error}",
                entry.path().display()
            )
        })?;
        let target_path = target_dir.join(entry.file_name());
        if file_type.is_dir() {
            copy_generated_artifact_dir(&entry.path(), &target_path)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), &target_path).map_err(|error| {
                anyhow!(
                    "failed to copy generated artifact {} to {}: {error}",
                    entry.path().display(),
                    target_path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn validate_static_page_image2_dynamic_artifact(
    index_path: &Path,
    artifact_dir: &Path,
) -> Result<()> {
    if !artifact_dir.join("data.json").is_file() {
        return Err(anyhow!(
            "static_page_image2_data_publish artifact must include data.json for dynamic database-backed refresh"
        ));
    }
    if !artifact_dir.join("data-snapshot.json").is_file() {
        return Err(anyhow!(
            "static_page_image2_data_publish artifact must include data-snapshot.json for time/snapshot refresh"
        ));
    }
    let html = fs::read_to_string(index_path).map_err(|error| {
        anyhow!(
            "failed to read generated static-page HTML {}: {error}",
            index_path.display()
        )
    })?;
    let html_lower = html.to_ascii_lowercase();
    if !html_lower.contains("data.json") {
        return Err(anyhow!(
            "static_page_image2_data_publish HTML must load local data.json"
        ));
    }
    let has_time_or_snapshot_signal = [
        "snapshotversion",
        "updatedat",
        "time",
        "date",
        "txdate",
        "时间",
        "日期",
        "快照",
    ]
    .iter()
    .any(|term| html_lower.contains(term));
    if !has_time_or_snapshot_signal {
        return Err(anyhow!(
            "static_page_image2_data_publish HTML must expose time or snapshot controls/signals"
        ));
    }
    Ok(())
}

fn publish_cloudflare_static_page_html(
    mut output: Value,
    html: &str,
    data_json: Option<Value>,
    task_context: &CodexHostTaskContext,
    execution_id: domain_model::WorkflowExecutionId,
    orchestrator_task_id: &str,
) -> Result<Value> {
    if data_json.is_none() {
        return Err(anyhow!(
            "static_page_image2_data_publish artifact.html success requires artifact.data_json for dynamic database-backed refresh"
        ));
    }
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
    let (data_path, data_url, data_snapshot_url) = if let Some(data_json) = data_json.as_ref() {
        let data_path = artifact_dir.join("data.json");
        let data_snapshot_path = artifact_dir.join("data-snapshot.json");
        let data_bytes = serde_json::to_vec_pretty(data_json)
            .map_err(|error| anyhow!("failed to serialize generated artifact data: {error}"))?;
        fs::write(&data_path, &data_bytes).map_err(|error| {
            anyhow!(
                "failed to write Cloudflare Codex generated static-page data {}: {error}",
                data_path.display()
            )
        })?;
        fs::write(&data_snapshot_path, &data_bytes).map_err(|error| {
            anyhow!(
                "failed to write Cloudflare Codex generated static-page data snapshot {}: {error}",
                data_snapshot_path.display()
            )
        })?;
        (
            Some(data_path),
            Some(generated_artifact_public_file_url(
                &relative_dir,
                "data.json",
            )),
            Some(generated_artifact_public_file_url(
                &relative_dir,
                "data-snapshot.json",
            )),
        )
    } else {
        (None, None, None)
    };
    validate_static_page_image2_dynamic_artifact(&index_path, &artifact_dir)?;
    let manifest = json!({
        "kind": "v3_codex_host_generated_static_page",
        "version": 1,
        "assistant_run_id": task_context.assistant_run_id.to_string(),
        "workflow_execution_id": execution_id.to_string(),
        "orchestrator_task_id": orchestrator_task_id,
        "capability": task_context.capability.clone(),
        "public_url": public_url.clone(),
        "data_url": data_url.clone(),
        "data_snapshot_url": data_snapshot_url.clone(),
        "dynamic_page_contract": static_page_dynamic_page_contract(),
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
        artifact_object.insert(
            "local_path".to_string(),
            Value::String(index_path.display().to_string()),
        );
        artifact_object.insert("public_url".to_string(), Value::String(public_url));
        artifact_object.insert(
            "manifest_path".to_string(),
            Value::String(manifest_path.display().to_string()),
        );
        if let (Some(data_path), Some(data_url)) = (data_path.as_ref(), data_url.as_ref()) {
            artifact_object.insert(
                "data_path".to_string(),
                Value::String(data_path.display().to_string()),
            );
            artifact_object.insert("data_url".to_string(), Value::String(data_url.clone()));
        }
        if let Some(data_snapshot_url) = data_snapshot_url.as_ref() {
            artifact_object.insert(
                "data_snapshot_url".to_string(),
                Value::String(data_snapshot_url.clone()),
            );
        }
    }
    strip_static_page_inline_artifact_payload(&mut output)?;
    Ok(output)
}

fn strip_static_page_inline_artifact_payload(output: &mut Value) -> Result<()> {
    let object = output
        .as_object_mut()
        .ok_or_else(|| anyhow!("fixed task output must be a JSON object"))?;
    if let Some(artifact_object) = object.get_mut("artifact").and_then(Value::as_object_mut) {
        for key in [
            "html",
            "html_text",
            "index_html",
            "data_json",
            "data",
            "data_snapshot",
        ] {
            artifact_object.remove(key);
        }
    }
    for key in ["html", "html_text", "data_json", "data", "data_snapshot"] {
        object.remove(key);
    }
    Ok(())
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
    generated_artifact_public_file_url(relative_dir, "index.html")
}

fn generated_artifact_public_file_url(relative_dir: &str, file_name: &str) -> String {
    format!(
        "{}/{}/{}",
        generated_artifact_public_base_url(),
        relative_dir.trim_matches('/'),
        file_name.trim_matches('/')
    )
}

fn static_page_dynamic_page_contract() -> Value {
    json!({
        "version": 1,
        "data_file": "data.json",
        "source_snapshot_file": "data-snapshot.json",
        "data_role": "client_refresh_snapshot",
        "default_controls": ["time_range", "primary_partition", "manual_refresh", "auto_refresh"],
        "refresh_policy": {
            "mode": "poll_data_json_when_published",
            "interval_seconds": 60,
            "change_detection_fields": ["snapshotVersion", "updatedAt", "snapshot_version", "updated_at"]
        },
        "rendering_policy": "final_html_should_render_stateful_business_modules_from_data_json_when_present"
    })
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

fn generated_static_page_public_url_claim_is_host_published(
    output: &Value,
    public_url: &str,
) -> bool {
    let normalized = public_url.trim();
    if !generated_artifact_url_allowed(normalized) {
        return false;
    }
    let local_path = output
        .pointer("/artifact/local_path")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    let remote_workspace_path =
        local_path.starts_with("/workspace/") || local_path.starts_with("workspace/");
    if remote_workspace_path {
        return false;
    }
    normalized.contains("/generated-artifacts/database-static-pages/")
        || local_path.starts_with("/srv/aiv3/shared/objects/generated-artifacts/")
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

fn parse_codex_host_concurrency(value: Option<&str>) -> usize {
    value
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_CODEX_HOST_CONCURRENCY)
        .min(MAX_CODEX_HOST_CONCURRENCY)
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
    execution_id: domain_model::WorkflowExecutionId,
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
        materialize_fixed_task_bundle(
            workspace_path,
            task_context,
            decision,
            &runtime_config.workspace_retention_policy(),
        )?;
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
            let output = extract_fixed_task_output_from_stdout(
                &output.stdout,
                fixed_task.template_id.as_str(),
            )?;
            normalize_cloudflare_fixed_task_output(
                output,
                task_context,
                execution_id,
                command_plan
                    .workspace_label
                    .as_deref()
                    .unwrap_or("codex_exec_workspace"),
            )
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
    fn parse_codex_host_concurrency_defaults_and_clamps() {
        assert_eq!(parse_codex_host_concurrency(None), 1);
        assert_eq!(parse_codex_host_concurrency(Some("")), 1);
        assert_eq!(parse_codex_host_concurrency(Some("0")), 1);
        assert_eq!(parse_codex_host_concurrency(Some("2")), 2);
        assert_eq!(parse_codex_host_concurrency(Some("99")), 8);
    }

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
            task_workspace_retention_hours: 168,
        };

        let error = run_codex_exec(
            domain_model::WorkflowExecutionId::new(),
            &command_plan,
            &task_context,
            &decision,
            &runtime_config,
        )
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
            task_workspace_retention_hours: 168,
        };

        let error = run_codex_exec(
            domain_model::WorkflowExecutionId::new(),
            &command_plan,
            &task_context,
            &decision,
            &runtime_config,
        )
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
                "html": "<!doctype html><html><body><main><h1>经营分析</h1><label>时间</label><button data-time=\"latest\">最新快照</button><script>fetch('data.json').then(r=>r.json()).then(data=>{document.body.dataset.snapshotVersion=data.snapshotVersion||data.updatedAt||'';});</script></main></body></html>",
                "data_json": {
                    "source": "unit-test",
                    "snapshotVersion": "snapshot-1",
                    "rows": [{"store": "新世界", "value": 1}]
                }
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
        assert!(normalized.pointer("/artifact/data_json").is_none());
        let local_path = normalized
            .pointer("/artifact/local_path")
            .and_then(Value::as_str)
            .expect("local path");
        assert!(std::path::Path::new(local_path).exists());
        let html = std::fs::read_to_string(local_path).expect("html should be readable");
        assert!(html.contains("<!doctype html>"));
        assert!(html.contains("经营分析"));
        let data_path = normalized
            .pointer("/artifact/data_path")
            .and_then(Value::as_str)
            .expect("data path");
        let data = std::fs::read_to_string(data_path).expect("data should be readable");
        assert!(data.contains("snapshot-1"));
        assert_eq!(
            normalized.pointer("/artifact/data_url"),
            Some(&json!(public_url.replace("index.html", "data.json")))
        );
        let manifest_path = normalized
            .pointer("/artifact/manifest_path")
            .and_then(Value::as_str)
            .expect("manifest path");
        let manifest: Value = serde_json::from_str(
            &std::fs::read_to_string(manifest_path).expect("manifest should be readable"),
        )
        .expect("manifest json");
        assert_eq!(
            manifest["dynamic_page_contract"]["data_file"],
            json!("data.json")
        );
        assert_eq!(
            manifest["data_url"],
            json!(public_url.replace("index.html", "data.json"))
        );
    }

    #[test]
    fn cloudflare_static_page_allowed_public_url_strips_inline_payload() {
        let task_context = CodexHostTaskContext {
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
        let output = json!({
            "template_id": "static_page_image2_data_publish",
            "status": "success",
            "artifact": {
                "public_url": "https://v3.elepcloud.com/generated-artifacts/database-static-pages/final/index.html",
                "html": "<main>inline html</main>",
                "data_json": {"source": "inline"}
            },
            "data": {"source": "top-level"},
            "validation_report": {
                "source_row_count": 1
            }
        });

        let normalized = normalize_cloudflare_fixed_task_output(
            output,
            &task_context,
            domain_model::WorkflowExecutionId::new(),
            "task_test",
        )
        .expect("output should normalize");

        assert_eq!(
            normalized.pointer("/artifact/public_url"),
            Some(&json!(
                "https://v3.elepcloud.com/generated-artifacts/database-static-pages/final/index.html"
            ))
        );
        assert!(normalized.pointer("/artifact/html").is_none());
        assert!(normalized.pointer("/artifact/data_json").is_none());
        assert!(normalized.pointer("/data").is_none());
    }

    #[test]
    fn cloudflare_static_page_workspace_public_url_is_republished_by_host_agent() {
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
        let draft_id = task_context
            .fixed_task
            .as_ref()
            .and_then(|fixed_task| fixed_task.draft_id.as_deref())
            .unwrap_or("draft");
        let output = json!({
            "template_id": "static_page_image2_data_publish",
            "status": "success",
            "artifact": {
                "public_url": format!(
                    "https://v3.elepcloud.com/generated-artifacts/{draft_id}/index.html"
                ),
                "local_path": format!("/workspace/generated-artifacts/{draft_id}/index.html"),
                "html": "<!doctype html><html><body><main><h1>Image2 视觉合同页面</h1><label>时间</label><script>fetch('data.json').then(r=>r.json()).then(data=>{document.body.dataset.updatedAt=data.updatedAt||data.snapshotVersion||'';});</script></main></body></html>",
                "data_json": {"source": "cloudflare-inline"}
            },
            "validation_report": {
                "source_row_count": 1
            }
        });

        let normalized = normalize_cloudflare_fixed_task_output(
            output,
            &task_context,
            domain_model::WorkflowExecutionId::new(),
            "task_workspace_url",
        )
        .expect("output should normalize");

        let public_url = normalized
            .pointer("/artifact/public_url")
            .and_then(Value::as_str)
            .expect("public url");
        assert!(public_url.contains("/generated-artifacts/database-static-pages/codex-host/"));
        assert!(public_url.contains(&assistant_run_id.to_string()));
        assert!(!public_url.contains(&format!("/generated-artifacts/{draft_id}/")));
        assert!(normalized.pointer("/artifact/html").is_none());
        let local_path = normalized
            .pointer("/artifact/local_path")
            .and_then(Value::as_str)
            .expect("local path");
        assert!(std::path::Path::new(local_path).exists());
        let html = std::fs::read_to_string(local_path).expect("html should be readable");
        assert!(html.contains("Image2 视觉合同页面"));
    }

    #[test]
    fn codex_exec_workspace_local_static_page_dir_is_republished_by_host_agent() {
        let _lock = test_env_lock().lock().expect("env lock");
        let artifact_root = std::env::temp_dir()
            .join("v3-codex-host-test-artifacts")
            .join(Uuid::new_v4().to_string());
        let workspace_root = std::env::temp_dir()
            .join("v3-codex-host-test-workspaces")
            .join(Uuid::new_v4().to_string());
        let workspace_artifact_dir =
            workspace_root.join("task/generated-artifacts/static-page-demo");
        std::fs::create_dir_all(&workspace_artifact_dir).expect("workspace artifact dir");
        std::fs::write(
            workspace_artifact_dir.join("index.html"),
            "<!doctype html><html><body><h1>Codex workspace page</h1><label>时间</label><script>fetch('data.json').then(r=>r.json()).then(data=>{document.body.dataset.snapshotVersion=data.snapshotVersion||data.updatedAt||'';});</script></body></html>",
        )
        .expect("workspace html");
        std::fs::write(
            workspace_artifact_dir.join("data.json"),
            "{\"ok\":true,\"snapshotVersion\":\"snapshot-1\"}",
        )
        .expect("workspace data");
        std::fs::write(
            workspace_artifact_dir.join("data-snapshot.json"),
            "{\"ok\":true,\"snapshotVersion\":\"snapshot-1\"}",
        )
        .expect("workspace data snapshot");
        std::fs::write(
            workspace_artifact_dir.join("manifest.json"),
            "{\"kind\":\"test\"}",
        )
        .expect("workspace manifest");
        let _root = TestEnvVarRestore::set(
            "V3_GENERATED_ARTIFACT_ROOT",
            artifact_root.display().to_string(),
        );
        let _workspace_root = TestEnvVarRestore::set(
            "CODEX_HOST_AGENT_TASK_WORKSPACE_ROOT",
            workspace_root.display().to_string(),
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
        let output = json!({
            "template_id": "static_page_image2_data_publish",
            "status": "success",
            "artifact": {
                "public_url": "https://v3.elepcloud.com/generated-artifacts/static-page-demo/index.html",
                "local_path": workspace_artifact_dir.join("index.html").display().to_string(),
                "manifest_path": workspace_artifact_dir.join("manifest.json").display().to_string()
            },
            "validation_report": {
                "source_row_count": 1
            }
        });

        let normalized = normalize_cloudflare_fixed_task_output(
            output,
            &task_context,
            domain_model::WorkflowExecutionId::new(),
            "task_workspace_local_path",
        )
        .expect("output should normalize");

        let public_url = normalized
            .pointer("/artifact/public_url")
            .and_then(Value::as_str)
            .expect("public url");
        assert!(public_url.contains("/generated-artifacts/database-static-pages/codex-host/"));
        assert!(public_url.ends_with("/index.html"));
        let local_path = normalized
            .pointer("/artifact/local_path")
            .and_then(Value::as_str)
            .expect("local path");
        assert!(std::path::Path::new(local_path).starts_with(&artifact_root));
        let html = std::fs::read_to_string(local_path).expect("html should be readable");
        assert!(html.contains("Codex workspace page"));
        assert!(normalized.pointer("/artifact/data_url").is_some());
        assert!(normalized.pointer("/artifact/manifest_path").is_some());
    }

    #[test]
    fn codex_exec_relative_workspace_static_page_dir_is_republished_by_host_agent() {
        let _lock = test_env_lock().lock().expect("env lock");
        let artifact_root = std::env::temp_dir()
            .join("v3-codex-host-test-artifacts")
            .join(Uuid::new_v4().to_string());
        let workspace_root = std::env::temp_dir()
            .join("v3-codex-host-test-workspaces")
            .join(Uuid::new_v4().to_string());
        let workspace_label = "codex-host-task-relative-static-page";
        let workspace_artifact_dir = workspace_root
            .join(workspace_label)
            .join("generated-artifacts/static-page-demo");
        std::fs::create_dir_all(&workspace_artifact_dir).expect("workspace artifact dir");
        std::fs::write(
            workspace_artifact_dir.join("index.html"),
            "<!doctype html><html><body><h1>Relative workspace page</h1><label>时间</label><script>fetch('data.json').then(r=>r.json()).then(data=>{document.body.dataset.updatedAt=data.updatedAt||data.snapshotVersion||'';});</script></body></html>",
        )
        .expect("workspace html");
        std::fs::write(
            workspace_artifact_dir.join("data.json"),
            "{\"ok\":true,\"updatedAt\":\"2026-05-31T00:00:00Z\"}",
        )
        .expect("workspace data");
        std::fs::write(
            workspace_artifact_dir.join("data-snapshot.json"),
            "{\"ok\":true,\"updatedAt\":\"2026-05-31T00:00:00Z\"}",
        )
        .expect("workspace data snapshot");
        let _root = TestEnvVarRestore::set(
            "V3_GENERATED_ARTIFACT_ROOT",
            artifact_root.display().to_string(),
        );
        let _workspace_root = TestEnvVarRestore::set(
            "CODEX_HOST_AGENT_TASK_WORKSPACE_ROOT",
            workspace_root.display().to_string(),
        );
        let _base = TestEnvVarRestore::set(
            "V3_GENERATED_ARTIFACT_PUBLIC_BASE_URL",
            "https://v3.elepcloud.com/generated-artifacts",
        );
        let task_context = CodexHostTaskContext {
            assistant_run_id: AssistantRunId::new(),
            capability: "static_page_image2_data_publish".to_string(),
            task: Some("Run fixed static-page package".to_string()),
            local_thread_id: None,
            task_memory_isolated: true,
            task_memory_space_id: Some("codex-host-task:relative-static-page".to_string()),
            fixed_task: Some(
                contracts::CodexHostFixedTaskTemplateContextView::static_page_image2_data_publish_example(),
            ),
        };
        let output = json!({
            "template_id": "static_page_image2_data_publish",
            "status": "success",
            "artifact": {
                "public_url": "https://v3.elepcloud.com/generated-artifacts/static-page-demo/index.html",
                "local_path": "generated-artifacts/static-page-demo/index.html"
            },
            "validation_report": {
                "source_row_count": 1
            }
        });

        let normalized = normalize_cloudflare_fixed_task_output(
            output,
            &task_context,
            domain_model::WorkflowExecutionId::new(),
            "task_workspace_relative_path",
        )
        .expect("output should normalize");

        let public_url = normalized
            .pointer("/artifact/public_url")
            .and_then(Value::as_str)
            .expect("public url");
        assert!(public_url.contains("/generated-artifacts/database-static-pages/codex-host/"));
        let local_path = normalized
            .pointer("/artifact/local_path")
            .and_then(Value::as_str)
            .expect("local path");
        assert!(std::path::Path::new(local_path).starts_with(&artifact_root));
        let html = std::fs::read_to_string(local_path).expect("html should be readable");
        assert!(html.contains("Relative workspace page"));
        assert!(normalized.pointer("/artifact/data_url").is_some());
    }

    #[test]
    fn cloudflare_static_page_partial_visual_result_does_not_publish_contract_fallback() {
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
        let task_context = CodexHostTaskContext {
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
        let output = json!({
            "template_id": "static_page_image2_data_publish",
            "report_title": "候选人对比分析报表",
            "publish_ready": false,
            "evidence_status": "referenced_but_not_available_in_task_payload",
            "render_asset_url": "https://v3.elepcloud.com/generated-artifacts/static-page-previews/image-job/preview.png",
            "focus": ["物联网经验", "技术管理", "项目经历", "匹配建议"]
        });

        let normalized = normalize_cloudflare_fixed_task_output(
            output,
            &task_context,
            domain_model::WorkflowExecutionId::new(),
            "task_partial_visual",
        )
        .expect("partial visual result should normalize");

        assert!(normalized.pointer("/artifact/public_url").is_none());
        assert!(normalized.pointer("/artifact/local_path").is_none());
        assert_eq!(
            normalized["evidence_status"],
            json!("referenced_but_not_available_in_task_payload")
        );
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
        fixed_task.image2["render_asset_url"] = json!(
            "https://v3.elepcloud.com/generated-artifacts/static-page-previews/job/preview.png"
        );
        fixed_task.image2["asset_provenance"] = json!({
            "schema": "v3.static_page_preview_asset_provenance",
            "schemaVersion": 1,
            "renderAssetUrl": "https://v3.elepcloud.com/generated-artifacts/static-page-previews/job/preview.png",
            "renderAssetPolicy": "use_persisted_v3_preview_asset_for_final_html",
            "sourceAssetKind": "remote_url",
            "sourceAssetRef": "https://souleye.cc/artifacts/preview.png?token=secret-token#download",
            "sourceAssetRefRedacted": true,
            "sourceAssetHadQuery": true,
            "persisted": true,
            "persistedPreviewAssetKey": "https://v3.elepcloud.com/generated-artifacts/static-page-previews/job/preview.png",
            "storageStatus": "persisted",
            "byteSize": 1234,
            "mimeType": "image/png",
            "width": 1536,
            "height": 1024
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
        assert_eq!(
            parsed["image2"]["asset_provenance"]["sourceAssetRef"],
            json!("https://souleye.cc/artifacts/preview.png")
        );
        assert_eq!(
            parsed["image2"]["asset_provenance"]["renderAssetPolicy"],
            json!("use_persisted_v3_preview_asset_for_final_html")
        );
        assert!(!prompt_json.contains("secret-token"));
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
    fn cloudflare_orchestrator_service_headers_identify_v3_callers() {
        let config = CloudflareOrchestratorConfig {
            base_url: "https://souleye.cc".to_string(),
            api_path: "/api/codex/orchestrator/v1".to_string(),
            access_key: "service-key".to_string(),
            runtime_target_id: "cloudflare".to_string(),
            project_id: None,
            source: "v3-codex-host-agent".to_string(),
            user_agent: "v3-codex-host-agent/1.0".to_string(),
            kind: "code-task".to_string(),
        };

        let headers = cloudflare_orchestrator_service_headers(
            &config,
            Some("application/json"),
            Some("idempotent-1".to_string()),
        );
        let header_value = |name: &str| {
            headers
                .iter()
                .find(|(candidate, _)| *candidate == name)
                .map(|(_, value)| value.as_str())
        };

        assert_eq!(header_value("Authorization"), Some("Bearer service-key"));
        assert_eq!(header_value("X-Client-Name"), Some("v3-codex-host-agent"));
        assert_eq!(header_value("User-Agent"), Some("v3-codex-host-agent/1.0"));
        assert_eq!(header_value("Content-Type"), Some("application/json"));
        assert_eq!(header_value("Idempotency-Key"), Some("idempotent-1"));
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
            "static_page_publish",
            "poll_static_page_publish",
        );

        assert_eq!(
            cloudflare_orchestrator_task_id_from_payload(&updated).as_deref(),
            Some("task-cloudflare-1")
        );
        assert_eq!(
            updated["cloudflare_orchestrator"]["runtime_target_id"],
            json!("cloudflare")
        );
        assert_eq!(updated["logical_queue"], json!("static_page_publish"));
        assert_eq!(
            updated["logical_task_key"],
            json!("poll_static_page_publish")
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
    fn cloudflare_orchestrator_pending_requeue_does_not_consume_attempt_budget() {
        let task = test_workflow_task(9, 3);

        assert!(should_requeue_cloudflare_orchestrator_poll(
            &CodexHostExecutionMode::CloudflareOrchestrator,
            "Cloudflare Codex task still running; task_id=abc status=running",
            &task
        ));
        assert_eq!(
            cloudflare_orchestrator_retry_reason(
                "Cloudflare Codex task submitted and pending; task_id=abc"
            ),
            "cloudflare_orchestrator_submitted"
        );
        assert_eq!(
            cloudflare_orchestrator_retry_reason(
                "Cloudflare Codex task still running; task_id=abc status=running"
            ),
            "cloudflare_orchestrator_pending"
        );
    }

    #[test]
    fn cloudflare_orchestrator_waf_and_auth_errors_are_operator_failures() {
        let task = test_workflow_task(1, 3);
        let waf_error = "Cloudflare Codex poll failed: status=403 body_excerpt=\"1010 browser_signature_banned\"";
        let auth_error =
            "Cloudflare Codex poll failed: status=401 body_excerpt=\"invalid service key\"";

        assert!(!cloudflare_orchestrator_requeueable_error(waf_error));
        assert!(!cloudflare_orchestrator_requeueable_error(auth_error));
        assert!(!should_requeue_cloudflare_orchestrator_poll(
            &CodexHostExecutionMode::CloudflareOrchestrator,
            waf_error,
            &task
        ));
        assert_eq!(
            cloudflare_orchestrator_retry_reason(waf_error),
            "cloudflare_orchestrator_waf_blocked"
        );
        assert_eq!(
            cloudflare_orchestrator_retry_reason(auth_error),
            "cloudflare_orchestrator_auth_failed"
        );
        assert!(cloudflare_orchestrator_requeueable_error(
            "Cloudflare Codex poll failed: status=500 body_excerpt=\"temporary upstream error\""
        ));
    }

    #[test]
    fn cloudflare_orchestrator_poll_payload_tracks_next_poll_without_prompt() {
        let now = Utc::now();
        let payload = workflow_task_payload_with_cloudflare_orchestrator_task(
            &json!({"execution_id": "exec-1"}),
            "task-cloudflare-1",
            &json!({"status": "running", "runtimeTargetId": "cloudflare"}),
            now,
            "static_page_publish",
            "poll_static_page_publish",
        );
        let next_poll_at = now + chrono::Duration::seconds(15);
        let updated = workflow_task_payload_with_cloudflare_orchestrator_poll(
            &payload,
            "cloudflare_orchestrator_pending",
            "static_page_publish",
            "poll_static_page_publish",
            7,
            15_000,
            next_poll_at,
            now,
            Some("Cloudflare Codex task still running; task_id=task-cloudflare-1 status=running"),
        );

        assert_eq!(
            cloudflare_orchestrator_task_id_from_payload(&updated).as_deref(),
            Some("task-cloudflare-1")
        );
        assert_eq!(
            updated["cloudflare_orchestrator"]["status"],
            json!("cloudflare_orchestrator_pending")
        );
        assert_eq!(updated["logical_queue"], json!("static_page_publish"));
        assert_eq!(
            updated["logical_task_key"],
            json!("poll_static_page_publish")
        );
        assert_eq!(updated["cloudflare_orchestrator"]["poll_attempt"], json!(7));
        assert_eq!(
            updated["cloudflare_orchestrator"]["next_poll_at"],
            json!(next_poll_at.to_rfc3339())
        );
        assert!(updated["cloudflare_orchestrator"].get("prompt").is_none());
    }

    #[test]
    fn codex_host_cancelled_error_reason_detects_long_running_cancellation() {
        assert_eq!(
            codex_host_cancelled_error_reason("Cloudflare Codex task cancelled before completion"),
            Some("workflow_cancelled_during_execution")
        );
        assert_eq!(
            codex_host_cancelled_error_reason("Cloudflare Codex task timed out after 1800000ms"),
            None
        );
    }

    #[test]
    fn codex_host_cancelled_payload_is_safe_and_non_retryable() {
        let mut task = test_workflow_task(2, 3);
        task.payload = json!({"raw": "should-not-leak"});
        let mut task_context = CodexHostTaskContext {
            assistant_run_id: AssistantRunId::new(),
            capability: "data_ingestion_analysis".to_string(),
            task: Some("contains secret prompt".to_string()),
            local_thread_id: None,
            task_memory_isolated: true,
            task_memory_space_id: Some("codex-host-task:test".to_string()),
            fixed_task: Some(
                contracts::CodexHostFixedTaskTemplateContextView::data_ingestion_analysis_example(),
            ),
        };
        task_context.fixed_task.as_mut().unwrap().requirements =
            json!({"database_url": "mysql://secret"});

        let payload = codex_host_task_cancelled_payload(
            "cloudflare_orchestrator",
            &task_context,
            &task,
            "workflow_cancelled_during_execution",
            Some("Authorization: Bearer secret-token\nCloudflare Codex task cancelled before completion"),
        );
        let serialized = payload.to_string();

        assert_eq!(payload["status"], json!("cancelled"));
        assert_eq!(payload["retryable"], json!(false));
        assert_eq!(payload["attempt"], json!(2));
        assert_eq!(payload["template_id"], json!("data_ingestion_analysis"));
        assert!(serialized.contains("[redacted-log-line]"));
        assert!(!serialized.contains("secret-token"));
        assert!(!serialized.contains("database_url"));
        assert!(!serialized.contains("contains secret prompt"));
        assert!(!serialized.contains("should-not-leak"));
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
