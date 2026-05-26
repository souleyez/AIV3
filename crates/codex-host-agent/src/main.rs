use anyhow::{anyhow, Result};
use chrono::Utc;
use codex_host_agent::{
    extract_fixed_task_output_from_stdout, fixed_task_output_schema_hint,
    materialize_fixed_task_bundle, safe_log_excerpt, CodexCommandPlan, CodexHostAgentPolicy,
    CodexHostExecutionDecision, CodexHostExecutionMode, CodexHostRuntimeConfig,
    CodexHostTaskContext, CodexProcessOutput,
};
use contracts::CodexHostTaskOutputView;
use domain_model::{AssistantRunId, WorkflowKind};
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

async fn run_cloudflare_orchestrator_with_heartbeat(
    storage: &PgStorage,
    tenant_id: domain_model::TenantId,
    execution_id: domain_model::WorkflowExecutionId,
    task_context: &CodexHostTaskContext,
    decision: &CodexHostExecutionDecision,
    runtime_config: &CodexHostRuntimeConfig,
) -> Result<serde_json::Value> {
    let started_at = Instant::now();
    let mut heartbeat_interval =
        tokio::time::interval(Duration::from_millis(runtime_config.heartbeat_ms()));
    let mut heartbeat_count = 0usize;
    let exec = run_cloudflare_orchestrator(task_context, decision, execution_id, runtime_config);
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

async fn run_cloudflare_orchestrator(
    task_context: &CodexHostTaskContext,
    decision: &CodexHostExecutionDecision,
    execution_id: domain_model::WorkflowExecutionId,
    runtime_config: &CodexHostRuntimeConfig,
) -> Result<serde_json::Value> {
    let config = CloudflareOrchestratorConfig::from_env()?;
    let prompt = build_cloudflare_orchestrator_prompt(task_context)?;
    let submitted =
        submit_cloudflare_orchestrator_task(&config, task_context, execution_id, &prompt).await?;
    let task_id = submitted
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Cloudflare Codex response missing task.id"))?
        .to_string();
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
    let full = serde_json::to_string_pretty(fixed_task)
        .map_err(|error| anyhow!("failed to serialize fixed task package: {error}"))?;
    if full.chars().count() <= 9_000 {
        return Ok(full);
    }
    let bounded = bounded_orchestrator_prompt_value(&json!(fixed_task), 0);
    serde_json::to_string_pretty(&bounded)
        .map_err(|error| anyhow!("failed to serialize bounded fixed task package: {error}"))
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
        return Err(anyhow!(
            "Cloudflare Codex submit failed: status={} body_chars={}",
            status,
            text.chars().count()
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
                "Cloudflare Codex task timed out after {}ms",
                timeout_ms.max(1)
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
            return Err(anyhow!(
                "Cloudflare Codex poll failed: status={} body_chars={}",
                status_code,
                text.chars().count()
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
                return Err(anyhow!("Cloudflare Codex task ended with status={code}"));
            }
            _ => {
                tokio::time::sleep(Duration::from_millis(config.poll_interval_ms.max(250))).await;
            }
        }
    }
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
    public_url.starts_with("https://v3.elepcloud.com/generated-artifacts/")
        || public_url.starts_with("/generated-artifacts/")
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
