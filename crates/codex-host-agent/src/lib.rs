use anyhow::{anyhow, Result};
use contracts::{
    CodexHostCommandPlanSummaryView, CodexHostFixedTaskTemplateContextView,
    CodexHostFixedTaskTemplateIdView, CodexHostProcessOutputSummaryView, CodexHostTaskOutputView,
    CodexHostTaskProfileSummaryView, HtmlArtifactManifestView,
};
use domain_model::{AssistantRunId, WorkflowExecution, WorkflowKind};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::{Component, Path, PathBuf};
use uuid::Uuid;

const DEFAULT_PROFILE_ID: &str = "default-dry-run";
const DEFAULT_PROFILE_KIND: &str = "dry-run";
const DEFAULT_HOST_KIND: &str = "developer_workstation";
const DEFAULT_COMPAT_PROVIDER_ID: &str = "minimax";
const DEFAULT_COMPAT_PROVIDER_ENV_KEY: &str = "MINIMAX_API_KEY";
const DEFAULT_COMPAT_PROVIDER_WIRE_API: &str = "responses";
const ENV_MODEL_REASONING_EFFORT: &str = "CODEX_HOST_AGENT_MODEL_REASONING_EFFORT";
const ENV_STATIC_PAGE_REASONING_EFFORT: &str = "CODEX_HOST_AGENT_STATIC_PAGE_REASONING_EFFORT";
const STATIC_PAGE_IMAGE2_DATA_PUBLISH: &str = "static_page_image2_data_publish";
const ANSWER_QUALITY_AUTOFIX: &str = "answer_quality_autofix";
const DATA_INGESTION_ANALYSIS: &str = "data_ingestion_analysis";
const PROPOSE_PATCH: &str = "propose_patch";
const CUSTOMER_COMPLEX_REQUEST: &str = "customer_complex_request";
const CUSTOMER_ARTIFACT_REQUEST: &str = "customer_artifact_request";
const GENERATED_STATIC_PAGE_EDIT: &str = "generated_static_page_edit";
const GENERATED_STATIC_PAGE_PUBLISH: &str = "generated_static_page_publish";
const V3_PRODUCT_CHANGE_REQUEST: &str = "v3_product_change_request";
const DEFAULT_TASK_TIMEOUT_MS: u64 = 1_800_000;
const DEFAULT_HEARTBEAT_MS: u64 = 15_000;
const DEFAULT_STDOUT_LIMIT_BYTES: usize = 200_000;
const DEFAULT_STDERR_LIMIT_BYTES: usize = 100_000;
const DEFAULT_TASK_WORKSPACE_RETENTION_HOURS: u64 = 168;
const DEFAULT_GENERATED_ARTIFACTS_ROOT: &str = "/srv/aiv3/shared/objects/generated-artifacts";
const V3_GENERATED_ARTIFACTS_URL_PREFIX: &str = "https://v3.elepcloud.com/generated-artifacts/";
const V3_SERVER_REPO_ROOT: &str = "/srv/aiv3/repo";
const CUSTOMER_RESULT_OUTPUT_SCHEMA_PATH: &str = "schemas/customer-result-summary.schema.json";
const ENV_CUSTOMER_RESULT_OUTPUT_SCHEMA_ENABLED: &str =
    "CODEX_HOST_AGENT_CUSTOMER_RESULT_OUTPUT_SCHEMA_ENABLED";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexHostTaskContext {
    pub assistant_run_id: AssistantRunId,
    pub capability: String,
    pub task: Option<String>,
    pub local_thread_id: Option<String>,
    pub task_memory_isolated: bool,
    pub task_memory_space_id: Option<String>,
    pub fixed_task: Option<CodexHostFixedTaskTemplateContextView>,
    pub workspace_seed: Option<Value>,
}

impl CodexHostTaskContext {
    pub fn from_execution(execution: &WorkflowExecution) -> Result<Self> {
        if execution.kind != WorkflowKind::CodexHostTask {
            return Err(anyhow!(
                "workflow execution {} has unexpected kind {}",
                execution.id,
                execution.kind.as_str()
            ));
        }
        let assistant_run_id = context_string(&execution.context, "assistant_run_id")?
            .and_then(|value| Uuid::parse_str(&value).ok())
            .map(AssistantRunId)
            .ok_or_else(|| {
                anyhow!(
                    "workflow execution {} missing assistant_run_id",
                    execution.id
                )
            })?;
        let capability = context_string(&execution.context, "capability")?
            .ok_or_else(|| anyhow!("workflow execution {} missing capability", execution.id))?;
        let task = context_string(&execution.context, "task")?;
        let local_thread_id = context_string(&execution.context, "local_thread_id")?;
        let task_memory_isolated = execution
            .context
            .get("task_memory_policy")
            .and_then(|policy| policy.get("isolated"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let task_memory_space_id = context_string(&execution.context, "task_memory_space_id")?
            .or_else(|| {
                execution
                    .context
                    .get("task_memory_policy")
                    .and_then(|policy| policy.get("memory_space_id"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
            });
        let fixed_task = execution
            .context
            .get("fixed_task")
            .filter(|value| !value.is_null())
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|error| anyhow!("fixed_task context is invalid: {error}"))?;
        let workspace_seed = execution
            .context
            .get("workspace_seed")
            .filter(|value| !value.is_null())
            .cloned();

        Ok(Self {
            assistant_run_id,
            capability,
            task,
            local_thread_id,
            task_memory_isolated,
            task_memory_space_id,
            fixed_task,
            workspace_seed,
        })
    }

    pub fn dry_run_output(&self) -> Value {
        let html_artifacts = vec![self.html_report_artifact("dry_run", "completed", None, None)];
        json!(CodexHostTaskOutputView {
            mode: "dry_run".to_string(),
            codex_invoked: false,
            status: "completed".to_string(),
            host_kind: None,
            assistant_run_id: self.assistant_run_id.to_string(),
            capability: self.capability.clone(),
            profile: None,
            command_plan: None,
            process: None,
            task_chars: self
                .task
                .as_ref()
                .map(|task| task.chars().count())
                .unwrap_or(0),
            local_thread_id: self.local_thread_id.clone(),
            task_memory_isolated: self.task_memory_isolated,
            task_memory_space_id: self.task_memory_space_id.clone(),
            html_artifacts,
            fixed_task_output: None,
        })
    }

    pub fn planned_output(&self, decision: &CodexHostExecutionDecision) -> Value {
        let html_artifacts = vec![self.html_report_artifact(
            decision.mode.as_str(),
            "planned",
            Some(decision),
            None,
        )];
        json!(CodexHostTaskOutputView {
            mode: decision.mode.as_str().to_string(),
            codex_invoked: false,
            status: "planned".to_string(),
            host_kind: Some(decision.host_kind.clone()),
            assistant_run_id: self.assistant_run_id.to_string(),
            capability: self.capability.clone(),
            profile: Some(decision.profile.safe_summary()),
            command_plan: decision
                .command_plan
                .as_ref()
                .map(CodexCommandPlan::safe_summary),
            process: None,
            task_chars: self
                .task
                .as_ref()
                .map(|task| task.chars().count())
                .unwrap_or(0),
            local_thread_id: self.local_thread_id.clone(),
            task_memory_isolated: self.task_memory_isolated,
            task_memory_space_id: self.task_memory_space_id.clone(),
            html_artifacts,
            fixed_task_output: None,
        })
    }

    pub fn html_report_artifact(
        &self,
        mode: &str,
        status: &str,
        decision: Option<&CodexHostExecutionDecision>,
        process_output: Option<&CodexProcessOutput>,
    ) -> HtmlArtifactManifestView {
        let command_plan = decision
            .and_then(|decision| decision.command_plan.as_ref())
            .map(CodexCommandPlan::safe_summary);
        let profile = decision.map(|decision| decision.profile.safe_summary());
        let process = process_output.map(CodexProcessOutput::safe_summary);
        HtmlArtifactManifestView::codex_execution_report(
            &self.assistant_run_id.to_string(),
            "Codex Host 执行报告",
            format!("{mode} {status}"),
            json!({
                "mode": mode,
                "status": status,
                "hostKind": decision.map(|decision| decision.host_kind.clone()),
                "capability": self.capability.clone(),
                "fixedTask": self.fixed_task_report_payload(),
                "summary": codex_host_report_summary(mode, status, &self.capability),
                "workspaceLabel": command_plan
                    .as_ref()
                    .and_then(|plan| plan.workspace_label.clone())
                    .unwrap_or_else(|| "未配置".to_string()),
                "sandbox": command_plan
                    .as_ref()
                    .map(|plan| plan.sandbox.clone())
                    .unwrap_or_else(|| "dry-run".to_string()),
                "promptChars": self.task.as_ref().map(|task| task.chars().count()).unwrap_or(0),
                "profile": profile.as_ref().map(|profile| json!({
                    "id": profile.id.clone(),
                    "kind": profile.kind.clone(),
                    "model": profile.model.clone(),
                    "providerId": profile.provider_id.clone(),
                    "baseUrlConfigured": profile.base_url_configured,
                    "wireApi": profile.wire_api.clone(),
                    "allowedCapabilities": profile.allowed_capabilities.clone(),
                })),
                "process": process.as_ref().map(|process| json!({
                    "exitCode": process.exit_code,
                    "stdoutChars": process.stdout_chars,
                    "stderrChars": process.stderr_chars,
                })),
                "steps": codex_host_report_steps(mode),
                "risks": codex_host_report_risks(mode),
            }),
        )
    }

    fn fixed_task_report_payload(&self) -> Option<Value> {
        let fixed_task = self.fixed_task.as_ref()?;
        Some(json!({
            "templateId": fixed_task.template_id.as_str(),
            "version": fixed_task.version,
            "humanReviewPolicy": fixed_task.human_review_policy.clone(),
            "publishMode": fixed_task.policies.get("publish_mode").cloned(),
            "allowedWriteFileCount": fixed_task
                .allowed_write_scope
                .as_ref()
                .map(|scope| scope.files.len())
                .unwrap_or(0),
        }))
    }
}

fn codex_host_report_summary(mode: &str, status: &str, capability: &str) -> String {
    match mode {
        "dry_run" => format!(
            "Codex Host 已完成 dry-run，没有启动本机 Codex。能力：{capability}，状态：{status}。"
        ),
        "plan_only" => format!(
            "Codex Host 已生成计划摘要，命令和提示词保持脱敏。能力：{capability}，状态：{status}。"
        ),
        "codex_exec" => {
            format!("Codex Host 已在允许的远端主机执行 Codex。能力：{capability}，状态：{status}。")
        }
        "cloudflare_orchestrator" => {
            format!(
                "Codex Host 已投递到 Cloudflare Codex 编排器。能力：{capability}，状态：{status}。"
            )
        }
        _ => format!("Codex Host 任务已处理。能力：{capability}，状态：{status}。"),
    }
}

fn codex_host_report_steps(mode: &str) -> Vec<Value> {
    match mode {
        "dry_run" => vec![
            json!({"title": "读取任务上下文", "detail": "解析 assistant_run、能力、线程和隔离记忆空间。"}),
            json!({"title": "保持安全空跑", "detail": "不构造真实命令，不启动本机 Codex。"}),
        ],
        "plan_only" => vec![
            json!({"title": "校验 profile", "detail": "确认能力在 profile allowlist 内。"}),
            json!({"title": "生成命令计划", "detail": "只返回脱敏命令摘要和任务工作区标签。"}),
        ],
        "codex_exec" => vec![
            json!({"title": "校验远端主机", "detail": "必须满足 host kind、profile kind、allow flag 和任务工作区要求。"}),
            json!({"title": "执行 Codex", "detail": "在任务隔离工作区启动 Codex，并仅保留脱敏日志摘要。"}),
        ],
        "cloudflare_orchestrator" => vec![
            json!({"title": "校验 Cloudflare Codex", "detail": "确认 host kind、profile allowlist 和编排器配置。"}),
            json!({"title": "远端投递", "detail": "通过 Codex Web orchestrator 投递任务并轮询完成结果。"}),
        ],
        _ => vec![json!({"title": "处理任务", "detail": "Codex Host 返回结构化任务结果。"})],
    }
}

fn codex_host_report_risks(mode: &str) -> Vec<Value> {
    match mode {
        "codex_exec" | "cloudflare_orchestrator" => vec![json!({
            "title": "执行输出需复核",
            "detail": "报告只展示脱敏摘要；代码变更和产物仍需由 DataMax 工作流或人工复核。"
        })],
        _ => vec![json!({
            "title": "尚未真实执行",
            "detail": "dry-run/plan-only 只验证任务与命令计划，不代表远端 Codex 已完成实际工作。"
        })],
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CodexHostExecutionMode {
    DryRun,
    PlanOnly,
    CodexExec,
    CloudflareOrchestrator,
}

impl CodexHostExecutionMode {
    pub fn from_raw(raw: &str) -> Result<Self> {
        match raw.trim() {
            "" | "dry_run" | "dry-run" => Ok(Self::DryRun),
            "plan_only" | "plan-only" => Ok(Self::PlanOnly),
            "codex_exec" | "codex-exec" => Ok(Self::CodexExec),
            "cloudflare_orchestrator"
            | "cloudflare-orchestrator"
            | "cloudflare_codex"
            | "cloudflare-codex" => Ok(Self::CloudflareOrchestrator),
            other => Err(anyhow!(
                "unsupported CODEX_HOST_AGENT_EXECUTION_MODE={other}; expected dry_run, plan_only, codex_exec, or cloudflare_orchestrator"
            )),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DryRun => "dry_run",
            Self::PlanOnly => "plan_only",
            Self::CodexExec => "codex_exec",
            Self::CloudflareOrchestrator => "cloudflare_orchestrator",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexHostProfile {
    pub id: String,
    pub kind: String,
    pub model: Option<String>,
    pub provider_id: Option<String>,
    pub base_url: Option<String>,
    pub env_key: Option<String>,
    pub wire_api: Option<String>,
    pub allowed_capabilities: Vec<String>,
}

impl CodexHostProfile {
    pub fn allows_capability(&self, capability: &str) -> bool {
        if self.kind == "dry-run" && self.allowed_capabilities.is_empty() {
            return true;
        }
        self.allowed_capabilities
            .iter()
            .any(|allowed| allowed == capability)
    }

    pub fn safe_summary(&self) -> CodexHostTaskProfileSummaryView {
        CodexHostTaskProfileSummaryView {
            id: self.id.clone(),
            kind: self.kind.clone(),
            model: self.model.clone(),
            provider_id: self.provider_id.clone(),
            base_url_configured: self.base_url.is_some(),
            env_key: self.env_key.clone(),
            wire_api: self.wire_api.clone(),
            allowed_capabilities: self.allowed_capabilities.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexCommandPlan {
    pub program: String,
    pub args_without_prompt: Vec<String>,
    pub prompt: String,
    pub sandbox: String,
    pub workspace_path: Option<PathBuf>,
    pub workspace_label: Option<String>,
}

impl CodexCommandPlan {
    pub fn process_args(&self) -> Vec<String> {
        let mut args = self.args_without_prompt.clone();
        args.push(self.prompt.clone());
        args
    }

    pub fn safe_summary(&self) -> CodexHostCommandPlanSummaryView {
        CodexHostCommandPlanSummaryView {
            program: self.program.clone(),
            args_without_prompt: self.args_without_prompt.clone(),
            prompt_chars: self.prompt.chars().count(),
            sandbox: self.sandbox.clone(),
            workspace_configured: self.workspace_path.is_some(),
            workspace_label: self.workspace_label.clone(),
            prompt_redacted: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexProcessOutput {
    pub exit_code: Option<i32>,
    pub stdout_excerpt: String,
    pub stderr_excerpt: String,
}

impl CodexProcessOutput {
    pub fn safe_summary(&self) -> CodexHostProcessOutputSummaryView {
        CodexHostProcessOutputSummaryView {
            exit_code: self.exit_code,
            stdout_chars: self.stdout_excerpt.chars().count(),
            stderr_chars: self.stderr_excerpt.chars().count(),
            stdout_excerpt: String::new(),
            stderr_excerpt: String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexHostRuntimeConfig {
    pub task_timeout_ms: u64,
    pub heartbeat_ms: u64,
    pub stdout_limit_bytes: usize,
    pub stderr_limit_bytes: usize,
    pub task_workspace_retention_hours: u64,
}

impl CodexHostRuntimeConfig {
    pub fn from_env() -> Self {
        Self {
            task_timeout_ms: env_u64("CODEX_HOST_AGENT_TASK_TIMEOUT_MS", DEFAULT_TASK_TIMEOUT_MS),
            heartbeat_ms: env_u64("CODEX_HOST_AGENT_HEARTBEAT_MS", DEFAULT_HEARTBEAT_MS),
            stdout_limit_bytes: env_usize(
                "CODEX_HOST_AGENT_STDOUT_LIMIT_BYTES",
                DEFAULT_STDOUT_LIMIT_BYTES,
            ),
            stderr_limit_bytes: env_usize(
                "CODEX_HOST_AGENT_STDERR_LIMIT_BYTES",
                DEFAULT_STDERR_LIMIT_BYTES,
            ),
            task_workspace_retention_hours: env_u64(
                "CODEX_HOST_AGENT_TASK_WORKSPACE_RETENTION_HOURS",
                DEFAULT_TASK_WORKSPACE_RETENTION_HOURS,
            ),
        }
    }

    pub fn task_timeout_ms(&self) -> u64 {
        self.task_timeout_ms.max(1)
    }

    pub fn heartbeat_ms(&self) -> u64 {
        self.heartbeat_ms.max(1)
    }

    pub fn stdout_limit_bytes(&self) -> usize {
        self.stdout_limit_bytes.max(1)
    }

    pub fn stderr_limit_bytes(&self) -> usize {
        self.stderr_limit_bytes.max(1)
    }

    pub fn task_workspace_retention_hours(&self) -> u64 {
        self.task_workspace_retention_hours.max(1)
    }

    pub fn workspace_retention_policy(&self) -> CodexHostWorkspaceRetentionPolicy {
        CodexHostWorkspaceRetentionPolicy::new(self.task_workspace_retention_hours())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexHostWorkspaceRetentionPolicy {
    pub retention_hours: u64,
    pub cleanup_requires_operator: bool,
    pub backup_before_delete: bool,
    pub normal_cleanup_mode: String,
    pub local_cleanup_helper: String,
    pub server_cleanup_note: String,
}

impl CodexHostWorkspaceRetentionPolicy {
    pub fn new(retention_hours: u64) -> Self {
        Self {
            retention_hours: retention_hours.max(1),
            cleanup_requires_operator: true,
            backup_before_delete: true,
            normal_cleanup_mode: "operator_explicit".to_string(),
            local_cleanup_helper: "Safe-RemoveToBackup.ps1".to_string(),
            server_cleanup_note:
                "Archive workspace contents before deletion; never run cleanup from normal polling."
                    .to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexHostAgentPolicy {
    pub mode: CodexHostExecutionMode,
    pub profile: CodexHostProfile,
    pub host_kind: String,
    pub allow_real_codex_exec: bool,
    pub task_workspace_root: Option<PathBuf>,
}

impl CodexHostAgentPolicy {
    pub fn from_env() -> Result<Self> {
        let mode = CodexHostExecutionMode::from_raw(
            &std::env::var("CODEX_HOST_AGENT_EXECUTION_MODE")
                .unwrap_or_else(|_| "dry_run".to_string()),
        )?;
        let profile = CodexHostProfile {
            id: env_or_default("CODEX_HOST_AGENT_PROFILE_ID", DEFAULT_PROFILE_ID),
            kind: env_or_default("CODEX_HOST_AGENT_PROFILE_KIND", DEFAULT_PROFILE_KIND),
            model: std::env::var("CODEX_HOST_AGENT_PROFILE_MODEL")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            provider_id: std::env::var("CODEX_HOST_AGENT_PROFILE_PROVIDER_ID")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            base_url: std::env::var("CODEX_HOST_AGENT_PROFILE_BASE_URL")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            env_key: std::env::var("CODEX_HOST_AGENT_PROFILE_ENV_KEY")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            wire_api: std::env::var("CODEX_HOST_AGENT_PROFILE_WIRE_API")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            allowed_capabilities: split_env_list("CODEX_HOST_AGENT_PROFILE_ALLOWED_CAPABILITIES"),
        };
        Ok(Self {
            mode,
            profile,
            host_kind: env_or_default("CODEX_HOST_AGENT_HOST_KIND", DEFAULT_HOST_KIND),
            allow_real_codex_exec: env_bool("CODEX_HOST_AGENT_ALLOW_REAL_CODEX_EXEC", false),
            task_workspace_root: std::env::var("CODEX_HOST_AGENT_TASK_WORKSPACE_ROOT")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .map(PathBuf::from),
        })
    }

    pub fn prepare(&self, context: &CodexHostTaskContext) -> Result<CodexHostExecutionDecision> {
        if !self.profile.allows_capability(&context.capability) {
            return Err(anyhow!(
                "capability {} is not allowed by Codex Host profile {}",
                context.capability,
                self.profile.id
            ));
        }
        self.validate_capability_execution_scope(context)?;
        self.validate_fixed_task_policy(context)?;
        let command_plan = match self.mode {
            CodexHostExecutionMode::DryRun | CodexHostExecutionMode::CloudflareOrchestrator => None,
            CodexHostExecutionMode::PlanOnly | CodexHostExecutionMode::CodexExec => {
                Some(build_codex_command_plan(
                    context,
                    &self.profile,
                    self.task_workspace_root.as_deref(),
                )?)
            }
        };
        if self.mode == CodexHostExecutionMode::CodexExec {
            self.validate_real_exec_guard(command_plan.as_ref())?;
        }
        if self.mode == CodexHostExecutionMode::CloudflareOrchestrator {
            self.validate_cloudflare_orchestrator_guard()?;
        }

        Ok(CodexHostExecutionDecision {
            mode: self.mode.clone(),
            profile: self.profile.clone(),
            host_kind: self.host_kind.clone(),
            command_plan,
        })
    }

    fn validate_capability_execution_scope(&self, context: &CodexHostTaskContext) -> Result<()> {
        if context.capability == V3_PRODUCT_CHANGE_REQUEST {
            return Err(anyhow!(
                "capability {V3_PRODUCT_CHANGE_REQUEST} is blocked for Codex Host execution; V3 product changes require operator review"
            ));
        }
        Ok(())
    }

    fn validate_fixed_task_policy(&self, context: &CodexHostTaskContext) -> Result<()> {
        let requires_fixed_template = fixed_template_capability(&context.capability).is_some();
        if !requires_fixed_template && context.fixed_task.is_none() {
            return Ok(());
        }
        let fixed_task = context.fixed_task.as_ref().ok_or_else(|| {
            anyhow!(
                "capability {} requires a fixed_task template package",
                context.capability
            )
        })?;
        if fixed_task.template_id.as_str() != context.capability {
            return Err(anyhow!(
                "fixed_task template_id {} must match capability {}",
                fixed_task.template_id.as_str(),
                context.capability
            ));
        }
        if self.mode != CodexHostExecutionMode::DryRun
            && !approved_remote_host_kind(&self.host_kind)
        {
            return Err(anyhow!(
                "fixed template {} is blocked on host kind {}; use windows_jump, mac_host, linux_host, aiv3_server, or cloudflare_codex",
                fixed_task.template_id.as_str(),
                self.host_kind
            ));
        }
        match &fixed_task.template_id {
            CodexHostFixedTaskTemplateIdView::StaticPageImage2DataPublish => {
                validate_static_page_fixed_task(fixed_task)?;
                if self.requires_task_workspace_root() && self.task_workspace_root.is_none() {
                    return Err(anyhow!(
                        "static_page_image2_data_publish requires CODEX_HOST_AGENT_TASK_WORKSPACE_ROOT for non-dry-run execution"
                    ));
                }
            }
            CodexHostFixedTaskTemplateIdView::AnswerQualityAutofix => {
                validate_answer_quality_fixed_task(fixed_task)?;
                if self.requires_task_workspace_root() && self.task_workspace_root.is_none() {
                    return Err(anyhow!(
                        "answer_quality_autofix requires CODEX_HOST_AGENT_TASK_WORKSPACE_ROOT for non-dry-run execution"
                    ));
                }
            }
            CodexHostFixedTaskTemplateIdView::DataIngestionAnalysis => {
                validate_data_ingestion_fixed_task(fixed_task)?;
                if self.requires_task_workspace_root() && self.task_workspace_root.is_none() {
                    return Err(anyhow!(
                        "data_ingestion_analysis requires CODEX_HOST_AGENT_TASK_WORKSPACE_ROOT for non-dry-run execution"
                    ));
                }
            }
        }
        Ok(())
    }

    fn requires_task_workspace_root(&self) -> bool {
        matches!(
            self.mode,
            CodexHostExecutionMode::PlanOnly | CodexHostExecutionMode::CodexExec
        )
    }

    fn validate_real_exec_guard(&self, command_plan: Option<&CodexCommandPlan>) -> Result<()> {
        if !self.allow_real_codex_exec {
            return Err(anyhow!(
                "CODEX_HOST_AGENT_ALLOW_REAL_CODEX_EXEC must be true before codex_exec mode can launch Codex"
            ));
        }
        if approved_remote_host_kind(&self.host_kind) {
        } else {
            let other = self.host_kind.as_str();
            return Err(anyhow!(
                "codex_exec mode is blocked on host kind {other}; use windows_jump, mac_host, linux_host, aiv3_server, or cloudflare_codex"
            ));
        }
        match self.profile.kind.as_str() {
            "codex-native" | "codex-compatible-shim" => {}
            other => {
                return Err(anyhow!(
                    "profile kind {other} cannot be used for real Codex execution"
                ));
            }
        }
        let task_workspace_root = self.task_workspace_root.as_ref().ok_or_else(|| {
            anyhow!(
                "codex_exec mode requires CODEX_HOST_AGENT_TASK_WORKSPACE_ROOT for an isolated task workspace"
            )
        })?;
        let workspace_path = command_plan
            .and_then(|plan| plan.workspace_path.as_ref())
            .ok_or_else(|| {
                anyhow!(
                    "codex_exec mode requires CODEX_HOST_AGENT_TASK_WORKSPACE_ROOT for an isolated task workspace"
                )
            })?;
        validate_task_workspace_scope(task_workspace_root, workspace_path)?;
        Ok(())
    }

    fn validate_cloudflare_orchestrator_guard(&self) -> Result<()> {
        if self.host_kind != "cloudflare_codex" {
            return Err(anyhow!(
                "cloudflare_orchestrator mode requires CODEX_HOST_AGENT_HOST_KIND=cloudflare_codex"
            ));
        }
        Ok(())
    }
}

fn fixed_template_capability(capability: &str) -> Option<CodexHostFixedTaskTemplateIdView> {
    match capability {
        STATIC_PAGE_IMAGE2_DATA_PUBLISH => {
            Some(CodexHostFixedTaskTemplateIdView::StaticPageImage2DataPublish)
        }
        ANSWER_QUALITY_AUTOFIX => Some(CodexHostFixedTaskTemplateIdView::AnswerQualityAutofix),
        DATA_INGESTION_ANALYSIS => Some(CodexHostFixedTaskTemplateIdView::DataIngestionAnalysis),
        _ => None,
    }
}

fn approved_remote_host_kind(host_kind: &str) -> bool {
    matches!(
        host_kind,
        "windows_jump" | "mac_host" | "linux_host" | "aiv3_server" | "cloudflare_codex"
    )
}

fn validate_task_workspace_scope(task_workspace_root: &Path, workspace_path: &Path) -> Result<()> {
    if path_contains_parent_component(task_workspace_root)
        || path_contains_parent_component(workspace_path)
    {
        return Err(anyhow!(
            "Codex Host task workspace paths must not contain parent-directory components"
        ));
    }
    if path_is_under_v3_product_repo(task_workspace_root)
        || path_is_under_v3_product_repo(workspace_path)
    {
        return Err(anyhow!(
            "Codex Host task workspace must not point at the V3 product repository"
        ));
    }
    if !workspace_path.starts_with(task_workspace_root) {
        return Err(anyhow!(
            "Codex Host task workspace must stay under CODEX_HOST_AGENT_TASK_WORKSPACE_ROOT"
        ));
    }
    Ok(())
}

fn path_contains_parent_component(path: &Path) -> bool {
    path.components()
        .any(|component| matches!(component, Component::ParentDir))
}

fn path_is_under_v3_product_repo(path: &Path) -> bool {
    let normalized = normalized_path_for_policy(path);
    normalized == V3_SERVER_REPO_ROOT || normalized.starts_with(&format!("{V3_SERVER_REPO_ROOT}/"))
}

fn normalized_path_for_policy(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_string()
}

fn validate_static_page_fixed_task(
    fixed_task: &CodexHostFixedTaskTemplateContextView,
) -> Result<()> {
    if fixed_task.template_id != CodexHostFixedTaskTemplateIdView::StaticPageImage2DataPublish {
        return Err(anyhow!(
            "static_page_image2_data_publish requires template_id=static_page_image2_data_publish"
        ));
    }
    let publish_mode = fixed_task
        .policies
        .get("publish_mode")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if publish_mode != "new_generated_artifact_only" {
        return Err(anyhow!(
            "static_page_image2_data_publish requires publish_mode=new_generated_artifact_only"
        ));
    }
    require_non_empty_value_string(
        &fixed_task.image2,
        "image_job_id",
        "static_page_image2_data_publish requires image2.image_job_id",
    )?;
    let preview_asset_ready = non_empty_value_string(&fixed_task.image2, "preview_asset_key")
        .is_some_and(|value| !value.is_empty());
    let visual_contract_ready = fixed_task
        .image2
        .get("visual_contract_status")
        .and_then(Value::as_str)
        .is_some_and(|value| value == "preview_ready");
    if !visual_contract_ready && !preview_asset_ready {
        return Err(anyhow!(
            "static_page_image2_data_publish requires visual_contract_status=preview_ready or a non-empty preview_asset_key"
        ));
    }
    if fixed_task
        .image2
        .get("human_confirmation_required")
        .and_then(Value::as_bool)
        != Some(false)
    {
        return Err(anyhow!(
            "static_page_image2_data_publish requires image2.human_confirmation_required=false"
        ));
    }
    if fixed_task
        .policies
        .get("effect_image_confirmation_required")
        .and_then(Value::as_bool)
        != Some(false)
    {
        return Err(anyhow!(
            "static_page_image2_data_publish requires effect_image_confirmation_required=false"
        ));
    }
    if fixed_task
        .policies
        .get("continue_to_publish_after_effect_image")
        .and_then(Value::as_bool)
        != Some(true)
    {
        return Err(anyhow!(
            "static_page_image2_data_publish requires continue_to_publish_after_effect_image=true"
        ));
    }
    if !static_page_dataset_scope_has_selected_source(&fixed_task.dataset_scope) {
        return Err(anyhow!(
            "static_page_image2_data_publish requires at least one selected dataset, document, or database source"
        ));
    }
    validate_static_page_dynamic_page_contract(&fixed_task.requirements)?;
    for (key, description) in [
        ("snapshot_aggregation", "snapshot aggregation policy"),
        ("trend_aggregation", "trend aggregation policy"),
        ("unit_rendering", "unit rendering policy"),
        ("detail_table_policy", "detail table policy"),
    ] {
        require_non_empty_value_string(
            &fixed_task.policies,
            key,
            &format!("static_page_image2_data_publish requires {description}"),
        )?;
    }
    Ok(())
}

fn validate_static_page_dynamic_page_contract(requirements: &Value) -> Result<()> {
    let contract = requirements
        .get("dynamic_page_contract")
        .ok_or_else(|| anyhow!("static_page_image2_data_publish requires dynamic_page_contract"))?;
    if contract
        .get("time_selector_required")
        .and_then(Value::as_bool)
        != Some(true)
    {
        return Err(anyhow!(
            "static_page_image2_data_publish requires time_selector_required=true"
        ));
    }
    if contract
        .get("time_range_selector_required")
        .and_then(Value::as_bool)
        != Some(true)
    {
        return Err(anyhow!(
            "static_page_image2_data_publish requires time_range_selector_required=true"
        ));
    }
    let report_time_range = contract
        .get("report_time_range")
        .ok_or_else(|| anyhow!("static_page_image2_data_publish requires report_time_range"))?;
    if report_time_range.get("required").and_then(Value::as_bool) != Some(true) {
        return Err(anyhow!(
            "static_page_image2_data_publish requires report_time_range.required=true"
        ));
    }
    if report_time_range
        .get("default_granularity")
        .and_then(Value::as_str)
        != Some("month")
    {
        return Err(anyhow!(
            "static_page_image2_data_publish requires report_time_range.default_granularity=month"
        ));
    }
    Ok(())
}

fn non_empty_value_string<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn require_non_empty_value_string<'a>(
    value: &'a Value,
    key: &str,
    message: &str,
) -> Result<&'a str> {
    non_empty_value_string(value, key).ok_or_else(|| anyhow!(message.to_string()))
}

fn static_page_dataset_scope_has_selected_source(scope: &Value) -> bool {
    [
        "dataset_ids",
        "database_source_ids",
        "selected_document_ids",
    ]
    .iter()
    .any(|key| {
        scope
            .get(key)
            .and_then(Value::as_array)
            .is_some_and(|values| {
                values.iter().any(|value| {
                    value
                        .as_str()
                        .map(str::trim)
                        .is_some_and(|value| !value.is_empty())
                })
            })
    })
}

fn data_ingestion_scope_has_selected_source(scope: &Value) -> bool {
    [
        "dataset_ids",
        "database_source_ids",
        "selected_document_ids",
        "uploaded_file_ids",
        "source_ids",
        "table_ids",
    ]
    .iter()
    .any(|key| {
        scope
            .get(key)
            .and_then(Value::as_array)
            .is_some_and(|values| {
                values.iter().any(|value| {
                    value
                        .as_str()
                        .map(str::trim)
                        .is_some_and(|value| !value.is_empty())
                })
            })
    })
}

fn validate_answer_quality_fixed_task(
    fixed_task: &CodexHostFixedTaskTemplateContextView,
) -> Result<()> {
    let scope = fixed_task
        .allowed_write_scope
        .as_ref()
        .ok_or_else(|| anyhow!("answer_quality_autofix requires an allowed_write_scope"))?;
    for file in &scope.files {
        if !answer_quality_file_scope_allowed(file) {
            return Err(anyhow!(
                "answer_quality_autofix file scope {file} is not allowlisted"
            ));
        }
    }
    if scope.files.is_empty() {
        return Err(anyhow!(
            "answer_quality_autofix requires at least one allowlisted file scope"
        ));
    }
    Ok(())
}

fn answer_quality_file_scope_allowed(file: &str) -> bool {
    matches!(
        file,
        "crates/platform-api/src/lib.rs"
            | "fixtures/document-quality/**"
            | "scripts/run-document-quality-smoke.ps1"
            | "scripts/run-v3-quality-gate-smoke.ps1"
            | "docs/validation/**"
    )
}

fn validate_data_ingestion_fixed_task(
    fixed_task: &CodexHostFixedTaskTemplateContextView,
) -> Result<()> {
    if fixed_task.template_id != CodexHostFixedTaskTemplateIdView::DataIngestionAnalysis {
        return Err(anyhow!(
            "data_ingestion_analysis requires template_id=data_ingestion_analysis"
        ));
    }
    if !data_ingestion_scope_has_selected_source(&fixed_task.dataset_scope) {
        return Err(anyhow!(
            "data_ingestion_analysis requires at least one selected dataset, document, file, table, or database source"
        ));
    }
    let mode = fixed_task
        .policies
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if mode != "read_only_analysis_or_staging_spec" {
        return Err(anyhow!(
            "data_ingestion_analysis requires mode=read_only_analysis_or_staging_spec"
        ));
    }
    let credential_policy = fixed_task
        .policies
        .get("credential_policy")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if credential_policy != "do_not_request_or_emit_credentials" {
        return Err(anyhow!(
            "data_ingestion_analysis requires credential_policy=do_not_request_or_emit_credentials"
        ));
    }
    let production_write_policy = fixed_task
        .policies
        .get("production_write_policy")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if production_write_policy != "needs_human_confirmation" {
        return Err(anyhow!(
            "data_ingestion_analysis requires production_write_policy=needs_human_confirmation"
        ));
    }
    if fixed_task
        .policies
        .get("public_api_change_allowed")
        .and_then(Value::as_bool)
        != Some(false)
    {
        return Err(anyhow!(
            "data_ingestion_analysis requires public_api_change_allowed=false"
        ));
    }
    if fixed_task
        .policies
        .get("schema_change_allowed_without_confirmation")
        .and_then(Value::as_bool)
        != Some(false)
    {
        return Err(anyhow!(
            "data_ingestion_analysis requires schema_change_allowed_without_confirmation=false"
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexHostExecutionDecision {
    pub mode: CodexHostExecutionMode,
    pub profile: CodexHostProfile,
    pub host_kind: String,
    pub command_plan: Option<CodexCommandPlan>,
}

fn context_string(value: &Value, key: &str) -> Result<Option<String>> {
    match value.get(key) {
        Some(Value::String(raw)) => {
            Ok(Some(raw.trim().to_string()).filter(|value| !value.is_empty()))
        }
        Some(Value::Null) | None => Ok(None),
        Some(other) => Err(anyhow!("context field {key} must be a string, got {other}")),
    }
}

fn build_codex_command_plan(
    context: &CodexHostTaskContext,
    profile: &CodexHostProfile,
    task_workspace_root: Option<&Path>,
) -> Result<CodexCommandPlan> {
    let prompt = if context.fixed_task.is_some() {
        fixed_task_prompt(context.fixed_task.as_ref())?
    } else {
        let task = context
            .task
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .ok_or_else(|| anyhow!("Codex Host task text is required for non-dry-run planning"))?;
        customer_web_codex_prompt(context.capability.as_str(), &task)
    };
    let sandbox = codex_sandbox_for_context(context).to_string();
    let mut args_without_prompt = vec![
        "-a".to_string(),
        "never".to_string(),
        "exec".to_string(),
        "--skip-git-repo-check".to_string(),
        "--ephemeral".to_string(),
        "--sandbox".to_string(),
        sandbox.clone(),
    ];
    append_provider_config_args(&mut args_without_prompt, profile)?;
    append_reasoning_effort_config_args(&mut args_without_prompt, context)?;
    if let Some(output_schema_path) =
        customer_web_codex_output_schema_path(context.capability.as_str())
    {
        args_without_prompt.push("--output-schema".to_string());
        args_without_prompt.push(output_schema_path.to_string());
    }
    if let Some(model) = profile
        .model
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        args_without_prompt.push("--model".to_string());
        args_without_prompt.push(model.to_string());
    }
    let workspace_label = task_workspace_label(context);
    let workspace_path = task_workspace_root.map(|root| root.join(&workspace_label));
    Ok(CodexCommandPlan {
        program: "codex".to_string(),
        args_without_prompt,
        prompt,
        sandbox,
        workspace_path,
        workspace_label: Some(workspace_label),
    })
}

fn codex_sandbox_for_context(context: &CodexHostTaskContext) -> &'static str {
    match context.capability.as_str() {
        PROPOSE_PATCH
        | CUSTOMER_ARTIFACT_REQUEST
        | GENERATED_STATIC_PAGE_EDIT
        | GENERATED_STATIC_PAGE_PUBLISH => "workspace-write",
        CUSTOMER_COMPLEX_REQUEST | V3_PRODUCT_CHANGE_REQUEST => "read-only",
        _ if context.fixed_task.is_some() => "workspace-write",
        _ => "read-only",
    }
}

fn customer_web_codex_capability(capability: &str) -> bool {
    matches!(
        capability,
        CUSTOMER_COMPLEX_REQUEST
            | CUSTOMER_ARTIFACT_REQUEST
            | GENERATED_STATIC_PAGE_EDIT
            | GENERATED_STATIC_PAGE_PUBLISH
    )
}

fn customer_result_output_schema_enabled() -> bool {
    std::env::var(ENV_CUSTOMER_RESULT_OUTPUT_SCHEMA_ENABLED)
        .ok()
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            !matches!(normalized.as_str(), "0" | "false" | "no" | "off")
        })
        .unwrap_or(true)
}

fn customer_web_codex_output_schema_path(capability: &str) -> Option<&'static str> {
    if customer_web_codex_capability(capability) && customer_result_output_schema_enabled() {
        Some(CUSTOMER_RESULT_OUTPUT_SCHEMA_PATH)
    } else {
        None
    }
}

fn customer_web_codex_prompt(capability: &str, task: &str) -> String {
    match capability {
        CUSTOMER_COMPLEX_REQUEST => format!(
            "Run this DataMax Web Codex customer task.\n\
\n\
Scope:\n\
- Treat this as a customer using Codex from the V3 web UI.\n\
- Use read-only analysis. Do not modify files, repositories, V3 product source, services, migrations, auth, public APIs, provider config, deployment state, commits, or system files.\n\
- Do not expose credentials, raw customer documents, raw prompt text, provider logs, stdout, stderr, local absolute paths, database URLs, cookies, or tokens.\n\
- Give a useful customer-facing result, not just an execution log.\n\
\n\
Final output contract:\n\
- At the end, print exactly one JSON object containing `customer_result_summary`.\n\
- Do not wrap the final JSON in Markdown fences.\n\
- Keep every user-visible string concise and free of secrets or absolute paths.\n\
- Shape: {{\"customer_result_summary\":{{\"schema\":\"v3.customer_codex_result_summary\",\"schema_version\":1,\"status\":\"completed|needs_human|failed\",\"title\":\"string\",\"summary\":\"string\",\"findings\":[\"string\"],\"recommended_next_actions\":[\"string\"],\"warnings\":[\"string\"],\"artifact_intent\":false,\"safety\":{{\"raw_logs_exposed\":false,\"credentials_exposed\":false,\"absolute_paths_exposed\":false,\"prompt_exposed\":false}}}}}}\n\
\n\
Customer request:\n\
{task}"
        ),
        CUSTOMER_ARTIFACT_REQUEST | GENERATED_STATIC_PAGE_EDIT | GENERATED_STATIC_PAGE_PUBLISH => {
            format!(
                "Run this DataMax Web Codex customer artifact task.\n\
\n\
Scope:\n\
- Treat this as a customer using Codex from the V3 web UI.\n\
- Work only inside the current isolated task workspace.\n\
- Do not modify V3 product source, services, migrations, auth, public APIs, provider config, deployment state, commits, system files, or stable generated-artifact URLs.\n\
- Put customer-facing files under workspace-relative paths such as `artifacts/` or `generated-artifacts/final/`.\n\
- Write `customer-artifact-manifest.json`, `artifacts/manifest.json`, or `generated-artifacts/manifest.json` with workspace-relative artifact paths.\n\
- Manifest paths must not reference secrets, `.env`, `.git`, `node_modules`, absolute paths, or parent-directory escapes.\n\
- Do not expose credentials, raw customer documents, raw prompt text, provider logs, stdout, stderr, local absolute paths, database URLs, cookies, or tokens.\n\
\n\
Final output contract:\n\
- After writing files and the manifest, print exactly one JSON object containing `customer_result_summary`.\n\
- Do not wrap the final JSON in Markdown fences.\n\
- Shape: {{\"customer_result_summary\":{{\"schema\":\"v3.customer_codex_result_summary\",\"schema_version\":1,\"status\":\"completed|needs_human|failed\",\"title\":\"string\",\"summary\":\"string\",\"findings\":[\"string\"],\"recommended_next_actions\":[\"string\"],\"warnings\":[\"string\"],\"artifact_intent\":true,\"safety\":{{\"raw_logs_exposed\":false,\"credentials_exposed\":false,\"absolute_paths_exposed\":false,\"prompt_exposed\":false}}}}}}\n\
\n\
Customer request:\n\
{task}"
            )
        }
        _ => task.to_string(),
    }
}

fn fixed_task_prompt(fixed_task: Option<&CodexHostFixedTaskTemplateContextView>) -> Result<String> {
    let fixed_task = fixed_task.ok_or_else(|| anyhow!("fixed task package missing"))?;
    let template_id = fixed_task.template_id.as_str();
    let mut prompt = format!(
        "Run the DataMax fixed Cloudflare Codex task template `{}`. Use the files in the current task workspace. Read `task.json` and `schemas/output.schema.json`. Return exactly one final JSON object matching the configured output schema. Do not wrap the final JSON in Markdown fences. Do not modify files outside the workspace except through explicitly allowed generated-artifacts paths or allowlisted patch files. If the task cannot satisfy the no-confirm policy, return status `needs_human` with a bounded human_review_reason.\n\nExpected output schema:\n{}",
        template_id,
        fixed_task_output_schema_hint(template_id),
    );
    if template_id == STATIC_PAGE_IMAGE2_DATA_PUBLISH {
        prompt.push_str(
            "\n\nStatic-page rules:\n- The GPT-Image-2 preview is the mandatory visual contract. Build the website from that image's layout, hierarchy, density, color, and module composition.\n- If `task.json.requirements.existing_artifact.local_index_path` is present, treat it as the current published page to revise: preserve its style and module structure unless the user explicitly requests redesign, repair the requested data binding or content issue, and publish a new generated artifact instead of overwriting the old URL.\n- If `task.json.image2.local_preview_path` or `visual_contract_local_path` is present, use that local preview file as the visual contract before writing HTML.\n- Do not return a simplified renderer page, demo-only placeholder, or visual-contract fallback as success.\n- Write a complete artifact directory under the task workspace, normally `generated-artifacts/<artifact-id>/`, containing `index.html`, `data.json`, `data-snapshot.json`, and `manifest.json`.\n- `index.html` must load local `data.json`, preserve time controls, primary partition controls, manual refresh, and auto refresh/change detection so database-backed data can be replaced without rewriting the page.\n- Every report page must expose a time-range selector. Operating reports default to a monthly view; when the user does not specify a range, use the latest available month while keeping custom range/month controls.\n- Bind real DataMax dataset/database/document evidence from `task.json`. If selected evidence is thin or partially insufficient, first use every supplied dataset/database/document summary and available sample; then still publish a useful page with visible data-gap notes and `validation_report.warnings`. Do not return `needs_human` or `failed` solely because sample rows, optional dimensions, or some modules are incomplete.",
        );
    }
    Ok(prompt)
}

pub fn fixed_task_output_schema_hint(template_id: &str) -> &'static str {
    match template_id {
        STATIC_PAGE_IMAGE2_DATA_PUBLISH => {
            r#"{"template_id":"static_page_image2_data_publish","status":"success|needs_human|failed","artifact":{"local_path":"string","public_url":"https://v3.elepcloud.com/generated-artifacts/...","manifest_path":"string","data_url":"https://v3.elepcloud.com/generated-artifacts/.../data.json|null","data_snapshot_url":"https://v3.elepcloud.com/generated-artifacts/.../data-snapshot.json|null","data_json":{},"html":"string|null"},"dynamic_page_contract":{"data_file":"data.json","source_snapshot_file":"data-snapshot.json","time_selector":true,"primary_partition_selector":true,"manual_refresh":true,"auto_refresh":true,"database_refresh_ready":true},"validation_report":{"visual_contract_used":true,"snapshot_policy":"string","latest_snapshot":"string|null","source_row_count":0,"current_state_row_count":0,"detail_row_count":0,"unit_policy":"string","warnings":["string"]},"source_summary":["string"],"human_review_reason":"string|null"}"#
        }
        ANSWER_QUALITY_AUTOFIX => {
            r#"{"template_id":"answer_quality_autofix","status":"patch_ready|needs_human|not_system_defect|failed","failure_type":"missing_source|parse_quality|retrieval_supply|answer_policy|not_reproducible|unsafe_or_out_of_scope","root_cause":"string","changed_files":["string"],"tests_added":["string"],"test_commands":["string"],"risk_level":"low|medium|high","rollback_notes":"string","human_review_reason":"string|null"}"#
        }
        DATA_INGESTION_ANALYSIS => {
            r#"{"template_id":"data_ingestion_analysis","status":"analysis_ready|staging_spec_ready|needs_human|failed","source_summary":["string"],"data_quality_report":{"row_count":0,"warnings":["string"],"quality_notes":["string"]},"mapping_plan":{"fields":[{"source":"string","target":"string","confidence":"high|medium|low","notes":"string"}]},"staging_spec":{"target":"string","steps":["string"]},"validation_checks":["string"],"recommended_next_actions":["string"],"production_write_requested":false,"credential_request_detected":false,"public_api_change_requested":false,"schema_change_requested":false,"human_review_reason":"string|null"}"#
        }
        _ => {
            r#"{"template_id":"string","status":"success|needs_human|failed","human_review_reason":"string|null"}"#
        }
    }
}

pub fn extract_fixed_task_output_from_stdout(
    raw_stdout: &[u8],
    template_id: &str,
) -> Result<Value> {
    let stdout = String::from_utf8_lossy(raw_stdout);
    for (start, ch) in stdout.char_indices().rev() {
        if ch != '{' {
            continue;
        }
        let candidate = &stdout[start..];
        let mut deserializer = serde_json::Deserializer::from_str(candidate);
        let Ok(value) = Value::deserialize(&mut deserializer) else {
            continue;
        };
        if let Some(output) = extract_fixed_task_output_value(&value, template_id) {
            return Ok(output.clone());
        }
    }
    Err(anyhow!(
        "fixed task output for template {template_id} was not found in Codex stdout"
    ))
}

fn extract_fixed_task_output_value<'a>(value: &'a Value, template_id: &str) -> Option<&'a Value> {
    if value.get("template_id").and_then(Value::as_str) == Some(template_id) {
        return Some(value);
    }
    for field in [
        "fixed_task_output",
        "fixedTaskOutput",
        "template_output",
        "templateOutput",
        "result",
        "output",
    ] {
        if let Some(nested) = value.get(field) {
            if let Some(output) = extract_fixed_task_output_value(nested, template_id) {
                return Some(output);
            }
        }
    }
    None
}

pub fn materialize_fixed_task_bundle(
    workspace_path: &Path,
    context: &CodexHostTaskContext,
    decision: &CodexHostExecutionDecision,
    retention_policy: &CodexHostWorkspaceRetentionPolicy,
) -> Result<()> {
    let Some(fixed_task) = context.fixed_task.as_ref() else {
        return Ok(());
    };
    fs::create_dir_all(workspace_path).map_err(|error| {
        anyhow!(
            "failed to create Codex Host task workspace {}: {error}",
            workspace_path.display()
        )
    })?;
    fs::create_dir_all(workspace_path.join("schemas")).map_err(|error| {
        anyhow!(
            "failed to create Codex Host schema directory {}: {error}",
            workspace_path.join("schemas").display()
        )
    })?;
    fs::create_dir_all(workspace_path.join("evidence")).map_err(|error| {
        anyhow!(
            "failed to create Codex Host evidence directory {}: {error}",
            workspace_path.join("evidence").display()
        )
    })?;

    let mut task_json = json!(fixed_task);
    materialize_static_page_image2_preview_asset(workspace_path, &mut task_json)?;
    materialize_static_page_existing_artifact(workspace_path, &mut task_json)?;
    write_json_file(&workspace_path.join("task.json"), &task_json)?;
    let schema: Value = serde_json::from_str(fixed_task_output_schema_hint(
        fixed_task.template_id.as_str(),
    ))
    .map_err(|error| anyhow!("fixed task output schema hint is invalid JSON: {error}"))?;
    write_json_file(&workspace_path.join("schemas/output.schema.json"), &schema)?;
    write_json_file(
        &workspace_path.join("evidence/summary.json"),
        &json!({
            "template_id": fixed_task.template_id.as_str(),
            "dataset_scope": fixed_task.dataset_scope.clone(),
            "evidence_summary": fixed_task.evidence_summary.clone(),
            "trace_summary": fixed_task.trace_summary.clone(),
        }),
    )?;
    write_json_file(
        &workspace_path.join("runtime.json"),
        &json!({
            "assistant_run_id": context.assistant_run_id.to_string(),
            "capability": context.capability.clone(),
            "local_thread_id": context.local_thread_id.clone(),
            "task_memory_isolated": context.task_memory_isolated,
            "task_memory_space_id": context.task_memory_space_id.clone(),
            "host_kind": decision.host_kind.clone(),
            "profile": decision.profile.safe_summary(),
            "workspace_label": decision
                .command_plan
                .as_ref()
                .and_then(|plan| plan.workspace_label.clone()),
            "retention_policy": retention_policy,
            "raw_prompt_exposed": false,
            "secrets_exposed": false,
        }),
    )?;
    fs::write(
        workspace_path.join("README.md"),
        fixed_task_bundle_readme(fixed_task.template_id.as_str()),
    )
    .map_err(|error| {
        anyhow!(
            "failed to write Codex Host bundle README {}: {error}",
            workspace_path.join("README.md").display()
        )
    })?;
    Ok(())
}

pub fn materialize_customer_web_codex_output_schema(
    workspace_path: &Path,
    context: &CodexHostTaskContext,
) -> Result<()> {
    let Some(output_schema_path) =
        customer_web_codex_output_schema_path(context.capability.as_str())
    else {
        return Ok(());
    };
    fs::create_dir_all(workspace_path.join("schemas")).map_err(|error| {
        anyhow!(
            "failed to create Codex Host schema directory {}: {error}",
            workspace_path.join("schemas").display()
        )
    })?;
    write_json_file(
        &workspace_path.join(output_schema_path),
        &customer_web_codex_result_output_schema(context.capability.as_str()),
    )
}

fn customer_web_codex_result_output_schema(capability: &str) -> Value {
    let artifact_intent_default = capability != CUSTOMER_COMPLEX_REQUEST;
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "DataMax Customer Web Codex Result",
        "type": "object",
        "additionalProperties": false,
        "required": ["customer_result_summary"],
        "properties": {
            "customer_result_summary": {
                "type": "object",
                "additionalProperties": false,
                "required": [
                    "schema",
                    "schema_version",
                    "status",
                    "title",
                    "summary",
                    "findings",
                    "recommended_next_actions",
                    "warnings",
                    "artifact_intent",
                    "safety"
                ],
                "properties": {
                    "schema": {
                        "const": "v3.customer_codex_result_summary"
                    },
                    "schema_version": {
                        "const": 1
                    },
                    "status": {
                        "type": "string",
                        "enum": ["completed", "needs_human", "failed"]
                    },
                    "title": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 80
                    },
                    "summary": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 280
                    },
                    "findings": {
                        "type": "array",
                        "maxItems": 6,
                        "items": {
                            "type": "string",
                            "minLength": 1,
                            "maxLength": 180
                        }
                    },
                    "recommended_next_actions": {
                        "type": "array",
                        "maxItems": 5,
                        "items": {
                            "type": "string",
                            "minLength": 1,
                            "maxLength": 180
                        }
                    },
                    "warnings": {
                        "type": "array",
                        "maxItems": 4,
                        "items": {
                            "type": "string",
                            "minLength": 1,
                            "maxLength": 160
                        }
                    },
                    "artifact_intent": {
                        "type": "boolean",
                        "default": artifact_intent_default
                    },
                    "safety": {
                        "type": "object",
                        "additionalProperties": false,
                        "required": [
                            "raw_logs_exposed",
                            "credentials_exposed",
                            "absolute_paths_exposed",
                            "prompt_exposed"
                        ],
                        "properties": {
                            "raw_logs_exposed": { "const": false },
                            "credentials_exposed": { "const": false },
                            "absolute_paths_exposed": { "const": false },
                            "prompt_exposed": { "const": false }
                        }
                    }
                }
            }
        }
    })
}

pub fn materialize_workspace_seed(
    workspace_path: &Path,
    context: &CodexHostTaskContext,
    decision: &CodexHostExecutionDecision,
    retention_policy: &CodexHostWorkspaceRetentionPolicy,
) -> Result<()> {
    let Some(seed) = context.workspace_seed.as_ref() else {
        return Ok(());
    };
    if !matches!(
        context.capability.as_str(),
        CUSTOMER_ARTIFACT_REQUEST | GENERATED_STATIC_PAGE_EDIT | GENERATED_STATIC_PAGE_PUBLISH
    ) {
        return Ok(());
    }
    fs::create_dir_all(workspace_path).map_err(|error| {
        anyhow!(
            "failed to create Codex Host seeded workspace {}: {error}",
            workspace_path.display()
        )
    })?;
    write_json_file(&workspace_path.join("workspace-seed.json"), seed)?;
    write_json_file(
        &workspace_path.join("task.json"),
        &json!({
            "schema": "v3.customer_codex_workspace_task",
            "version": 1,
            "assistant_run_id": context.assistant_run_id.to_string(),
            "capability": context.capability.clone(),
            "local_thread_id": context.local_thread_id.clone(),
            "task_memory_space_id": context.task_memory_space_id.clone(),
            "workspace_seed": seed,
            "output_manifest": {
                "preferred_path": "customer-artifact-manifest.json",
                "compatible_paths": ["artifacts/manifest.json", "generated-artifacts/manifest.json"],
                "artifact_paths_must_be_workspace_relative": true
            },
            "safety": {
                "v3_product_repo_write_allowed": false,
                "deployment_change_allowed": false,
                "stable_url_overwrite_allowed": false
            }
        }),
    )?;
    write_json_file(
        &workspace_path.join("runtime.json"),
        &json!({
            "assistant_run_id": context.assistant_run_id.to_string(),
            "capability": context.capability.clone(),
            "local_thread_id": context.local_thread_id.clone(),
            "task_memory_isolated": context.task_memory_isolated,
            "task_memory_space_id": context.task_memory_space_id.clone(),
            "host_kind": decision.host_kind.clone(),
            "profile": decision.profile.safe_summary(),
            "workspace_label": decision
                .command_plan
                .as_ref()
                .and_then(|plan| plan.workspace_label.clone()),
            "retention_policy": retention_policy,
            "raw_prompt_exposed": false,
            "secrets_exposed": false,
        }),
    )?;
    fs::write(
        workspace_path.join("README.md"),
        customer_workspace_seed_readme(context.capability.as_str()),
    )
    .map_err(|error| {
        anyhow!(
            "failed to write Codex Host seeded workspace README {}: {error}",
            workspace_path.join("README.md").display()
        )
    })?;

    if context.capability == GENERATED_STATIC_PAGE_EDIT {
        materialize_generated_static_page_edit_seed(workspace_path, seed)?;
    }
    Ok(())
}

fn materialize_generated_static_page_edit_seed(workspace_path: &Path, seed: &Value) -> Result<()> {
    let Some(public_url) = generated_static_page_edit_seed_public_url(seed) else {
        return Ok(());
    };
    let mut task_json = json!({
        "template_id": STATIC_PAGE_IMAGE2_DATA_PUBLISH,
        "requirements": {
            "existing_artifact": {
                "public_url": public_url,
                "index_url": public_url,
                "revision_requested": true,
                "source": "assistant_run_workspace_seed"
            }
        }
    });
    materialize_static_page_existing_artifact(workspace_path, &mut task_json)?;
    write_json_file(
        &workspace_path.join("existing-artifact-seed.json"),
        task_json
            .pointer("/requirements/existing_artifact")
            .unwrap_or(&Value::Null),
    )?;
    Ok(())
}

fn generated_static_page_edit_seed_public_url(seed: &Value) -> Option<&str> {
    if seed.get("schema").and_then(Value::as_str) != Some("v3.codex_host_workspace_seed") {
        return None;
    }
    if seed.get("version").and_then(Value::as_i64) != Some(1) {
        return None;
    }
    if seed.get("kind").and_then(Value::as_str) != Some("generated_static_page_edit") {
        return None;
    }
    seed.pointer("/existing_artifact/public_url")
        .or_else(|| seed.pointer("/current_artifact/publicUrl"))
        .or_else(|| seed.pointer("/current_artifact/public_url"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| value.starts_with(V3_GENERATED_ARTIFACTS_URL_PREFIX))
}

fn customer_workspace_seed_readme(capability: &str) -> String {
    format!(
        "# DataMax Customer Codex Workspace\n\nCapability: `{capability}`\n\nRead `task.json` and `workspace-seed.json` before editing. For generated static page edits, the current page is copied under `existing-artifact/` when available.\n\nRules:\n\n- Work only inside this task workspace.\n- Do not modify V3 product source, services, migrations, auth, public APIs, provider config, deployment state, commits, or system files.\n- Do not overwrite a stable generated-artifact URL.\n- Put final customer files under a workspace-relative directory such as `generated-artifacts/final/` or `artifacts/`.\n- Write `customer-artifact-manifest.json` at the workspace root, or `artifacts/manifest.json`, or `generated-artifacts/manifest.json`.\n- Manifest file paths must be workspace-relative and must not reference secrets, `.env`, `.git`, `node_modules`, absolute paths, or parent-directory escapes.\n"
    )
}

fn write_json_file(path: &Path, value: &Value) -> Result<()> {
    let content = serde_json::to_string_pretty(value)
        .map_err(|error| anyhow!("failed to serialize {}: {error}", path.display()))?;
    fs::write(path, content).map_err(|error| anyhow!("failed to write {}: {error}", path.display()))
}

fn materialize_static_page_image2_preview_asset(
    workspace_path: &Path,
    task_json: &mut Value,
) -> Result<()> {
    if task_json.get("template_id").and_then(Value::as_str) != Some(STATIC_PAGE_IMAGE2_DATA_PUBLISH)
    {
        return Ok(());
    }
    let Some(preview_url) = static_page_image2_preview_url(task_json) else {
        return Ok(());
    };
    let preview_url = preview_url.to_string();
    let Some(relative_asset_path) = v3_generated_artifact_relative_path(&preview_url) else {
        return Ok(());
    };
    let source_path = generated_artifacts_root().join(&relative_asset_path);
    if !source_path.is_file() {
        return Ok(());
    }

    let preview_dir = workspace_path.join("image2");
    fs::create_dir_all(&preview_dir).map_err(|error| {
        anyhow!(
            "failed to create static page image2 directory {}: {error}",
            preview_dir.display()
        )
    })?;
    let extension = source_path
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("png");
    let local_name = format!("preview.{extension}");
    let local_path = preview_dir.join(&local_name);
    fs::copy(&source_path, &local_path).map_err(|error| {
        anyhow!(
            "failed to copy static page image2 preview {} to {}: {error}",
            source_path.display(),
            local_path.display()
        )
    })?;

    let local_rel = format!("image2/{local_name}");
    if let Some(image2) = task_json.get_mut("image2").and_then(Value::as_object_mut) {
        image2.insert(
            "local_preview_path".to_string(),
            Value::String(local_rel.clone()),
        );
        image2.insert(
            "visual_contract_local_path".to_string(),
            Value::String(local_rel.clone()),
        );
        image2.insert(
            "visual_contract_materialized".to_string(),
            Value::Bool(true),
        );
    }
    write_json_file(
        &preview_dir.join("manifest.json"),
        &json!({
            "kind": "static_page_image2_preview_asset",
            "source_url": preview_url,
            "source_path": source_path.display().to_string(),
            "local_preview_path": local_rel,
            "materialized": true,
        }),
    )?;
    Ok(())
}

fn materialize_static_page_existing_artifact(
    workspace_path: &Path,
    task_json: &mut Value,
) -> Result<()> {
    if task_json.get("template_id").and_then(Value::as_str) != Some(STATIC_PAGE_IMAGE2_DATA_PUBLISH)
    {
        return Ok(());
    }
    let Some(public_url) = task_json
        .pointer("/requirements/existing_artifact/public_url")
        .or_else(|| task_json.pointer("/requirements/existing_artifact/index_url"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| value.starts_with(V3_GENERATED_ARTIFACTS_URL_PREFIX))
        .map(str::to_string)
    else {
        return Ok(());
    };
    let Some(relative_artifact_path) = v3_generated_artifact_relative_path(&public_url) else {
        return Ok(());
    };
    let source_path = generated_artifacts_root().join(&relative_artifact_path);
    let source_index = if source_path.is_file() {
        source_path
    } else {
        source_path.join("index.html")
    };
    if !source_index.is_file() {
        return Ok(());
    }
    let Some(source_dir) = source_index.parent() else {
        return Ok(());
    };
    let target_dir = workspace_path.join("existing-artifact");
    let mut copy_summary = ExistingArtifactCopySummary::default();
    copy_existing_artifact_tree(source_dir, &target_dir, &mut copy_summary)?;
    let local_index_path = "existing-artifact/index.html";
    let data_path = target_dir.join("data.json");
    let snapshot_path = target_dir.join("data-snapshot.json");
    if let Some(existing_artifact) = task_json
        .pointer_mut("/requirements/existing_artifact")
        .and_then(Value::as_object_mut)
    {
        existing_artifact.insert("materialized".to_string(), Value::Bool(true));
        existing_artifact.insert(
            "local_dir".to_string(),
            Value::String("existing-artifact".to_string()),
        );
        existing_artifact.insert(
            "local_index_path".to_string(),
            Value::String(local_index_path.to_string()),
        );
        if data_path.is_file() {
            existing_artifact.insert(
                "local_data_path".to_string(),
                Value::String("existing-artifact/data.json".to_string()),
            );
        }
        if snapshot_path.is_file() {
            existing_artifact.insert(
                "local_data_snapshot_path".to_string(),
                Value::String("existing-artifact/data-snapshot.json".to_string()),
            );
        }
        existing_artifact.insert(
            "materialized_file_count".to_string(),
            Value::from(copy_summary.file_count as u64),
        );
        existing_artifact.insert(
            "materialized_total_bytes".to_string(),
            Value::from(copy_summary.total_bytes),
        );
    }
    write_json_file(
        &target_dir.join("source-manifest.json"),
        &json!({
            "kind": "static_page_existing_artifact",
            "source_url": public_url,
            "local_index_path": local_index_path,
            "file_count": copy_summary.file_count,
            "total_bytes": copy_summary.total_bytes,
            "materialized": true,
        }),
    )?;
    Ok(())
}

#[derive(Default)]
struct ExistingArtifactCopySummary {
    file_count: usize,
    total_bytes: u64,
}

fn copy_existing_artifact_tree(
    source_dir: &Path,
    target_dir: &Path,
    summary: &mut ExistingArtifactCopySummary,
) -> Result<()> {
    const MAX_EXISTING_ARTIFACT_FILES: usize = 500;
    const MAX_EXISTING_ARTIFACT_BYTES: u64 = 100 * 1024 * 1024;
    fs::create_dir_all(target_dir).map_err(|error| {
        anyhow!(
            "failed to create existing artifact directory {}: {error}",
            target_dir.display()
        )
    })?;
    for entry in fs::read_dir(source_dir).map_err(|error| {
        anyhow!(
            "failed to read existing artifact {}: {error}",
            source_dir.display()
        )
    })? {
        let entry = entry.map_err(|error| {
            anyhow!(
                "failed to read existing artifact entry {}: {error}",
                source_dir.display()
            )
        })?;
        let file_type = entry.file_type().map_err(|error| {
            anyhow!(
                "failed to inspect existing artifact entry {}: {error}",
                entry.path().display()
            )
        })?;
        if file_type.is_symlink() {
            continue;
        }
        let target_path = target_dir.join(entry.file_name());
        if file_type.is_dir() {
            copy_existing_artifact_tree(&entry.path(), &target_path, summary)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let size = entry
            .metadata()
            .map_err(|error| {
                anyhow!(
                    "failed to inspect existing artifact file {}: {error}",
                    entry.path().display()
                )
            })?
            .len();
        if summary.file_count >= MAX_EXISTING_ARTIFACT_FILES
            || summary.total_bytes.saturating_add(size) > MAX_EXISTING_ARTIFACT_BYTES
        {
            return Err(anyhow!(
                "existing artifact copy limit exceeded at {}",
                entry.path().display()
            ));
        }
        fs::copy(entry.path(), &target_path).map_err(|error| {
            anyhow!(
                "failed to copy existing artifact {} to {}: {error}",
                entry.path().display(),
                target_path.display()
            )
        })?;
        summary.file_count += 1;
        summary.total_bytes = summary.total_bytes.saturating_add(size);
    }
    Ok(())
}

fn static_page_image2_preview_url(task_json: &Value) -> Option<&str> {
    let image2 = task_json.get("image2")?;
    [
        "/preview_asset_key",
        "/render_asset_url",
        "/asset_provenance/persistedPreviewAssetKey",
        "/asset_provenance/renderAssetUrl",
    ]
    .into_iter()
    .filter_map(|pointer| image2.pointer(pointer).and_then(Value::as_str))
    .map(str::trim)
    .find(|value| value.starts_with(V3_GENERATED_ARTIFACTS_URL_PREFIX))
}

fn v3_generated_artifact_relative_path(url: &str) -> Option<PathBuf> {
    let raw = url
        .strip_prefix(V3_GENERATED_ARTIFACTS_URL_PREFIX)?
        .split(['?', '#'])
        .next()?
        .trim_matches('/');
    if raw.is_empty() {
        return None;
    }
    let path = PathBuf::from(raw);
    if path
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return None;
    }
    Some(path)
}

fn generated_artifacts_root() -> PathBuf {
    std::env::var("CODEX_HOST_AGENT_GENERATED_ARTIFACTS_ROOT")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_GENERATED_ARTIFACTS_ROOT))
}

fn fixed_task_bundle_readme(template_id: &str) -> String {
    format!(
        "# DataMax Fixed Codex Task\n\nTemplate: `{template_id}`\n\nRead `task.json` for the DataMax-owned fixed task package and `schemas/output.schema.json` for the required final JSON shape.\n\nRules:\n\n- Return exactly one final JSON object.\n- Do not wrap the final JSON in Markdown fences.\n- Do not emit credentials, database URLs, provider logs, raw customer documents, or raw stdout/stderr.\n- Do not change public API, auth, request/response fields, database schema, deployment config, or stable customer URLs unless the fixed task output returns `needs_human`.\n- `runtime.json` contains the workspace retention policy. Cleanup is operator-explicit and backup-first; normal polling must not delete this workspace.\n"
    )
}

fn task_workspace_label(context: &CodexHostTaskContext) -> String {
    let source = context
        .task_memory_space_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("codex-host-task-{}", context.assistant_run_id));
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

fn append_provider_config_args(args: &mut Vec<String>, profile: &CodexHostProfile) -> Result<()> {
    if profile.kind != "codex-compatible-shim" {
        return Ok(());
    }
    let provider_id = profile
        .provider_id
        .as_deref()
        .unwrap_or(DEFAULT_COMPAT_PROVIDER_ID);
    let base_url = profile
        .base_url
        .as_deref()
        .ok_or_else(|| anyhow!("codex-compatible-shim profile requires base_url"))?;
    let env_key = profile
        .env_key
        .as_deref()
        .unwrap_or(DEFAULT_COMPAT_PROVIDER_ENV_KEY);
    validate_provider_env_key_name(env_key)?;
    let wire_api = profile
        .wire_api
        .as_deref()
        .unwrap_or(DEFAULT_COMPAT_PROVIDER_WIRE_API);

    push_config_arg(args, "model_provider", provider_id);
    push_config_arg(
        args,
        &format!("model_providers.{provider_id}.name"),
        provider_id,
    );
    push_config_arg(
        args,
        &format!("model_providers.{provider_id}.base_url"),
        base_url,
    );
    push_config_arg(
        args,
        &format!("model_providers.{provider_id}.env_key"),
        env_key,
    );
    push_config_arg(
        args,
        &format!("model_providers.{provider_id}.wire_api"),
        wire_api,
    );
    args.push("-c".to_string());
    args.push(format!(
        "model_providers.{provider_id}.requires_openai_auth=false"
    ));
    Ok(())
}

fn validate_provider_env_key_name(env_key: &str) -> Result<()> {
    let trimmed = env_key.trim();
    let mut chars = trimmed.chars();
    let Some(first) = chars.next() else {
        return Err(anyhow!(
            "codex-compatible-shim profile env_key must name an environment variable"
        ));
    };
    let valid_first = first.is_ascii_alphabetic() || first == '_';
    let valid_rest = chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_');
    if !valid_first || !valid_rest || trimmed.len() > 128 {
        return Err(anyhow!(
            "codex-compatible-shim profile env_key must be an environment variable name, not a secret value"
        ));
    }
    Ok(())
}

fn append_reasoning_effort_config_args(
    args: &mut Vec<String>,
    context: &CodexHostTaskContext,
) -> Result<()> {
    let configured = if context.capability == STATIC_PAGE_IMAGE2_DATA_PUBLISH {
        std::env::var(ENV_STATIC_PAGE_REASONING_EFFORT)
            .or_else(|_| std::env::var(ENV_MODEL_REASONING_EFFORT))
            .ok()
    } else {
        std::env::var(ENV_MODEL_REASONING_EFFORT).ok()
    };
    let Some(effort) = configured
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
    else {
        return Ok(());
    };
    if !matches!(
        effort.as_str(),
        "none" | "minimal" | "low" | "medium" | "high" | "xhigh"
    ) {
        return Err(anyhow!(
            "unsupported Codex reasoning effort {effort}; expected none, minimal, low, medium, high, or xhigh"
        ));
    }
    push_config_arg(args, "model_reasoning_effort", &effort);
    Ok(())
}

fn push_config_arg(args: &mut Vec<String>, key: &str, value: &str) {
    args.push("-c".to_string());
    args.push(format!("{key}={}", toml_string(value)));
}

fn toml_string(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    format!("\"{escaped}\"")
}

fn env_or_default(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn env_bool(key: &str, default: bool) -> bool {
    std::env::var(key)
        .ok()
        .map(|raw| {
            matches!(
                raw.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|raw| raw.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn split_env_list(key: &str) -> Vec<String> {
    std::env::var(key)
        .ok()
        .map(|raw| {
            raw.split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

pub fn safe_log_excerpt(raw: &[u8], max_chars: usize) -> String {
    let text = String::from_utf8_lossy(raw);
    let mut redacted = String::new();
    for line in text.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("authorization")
            || lower.contains("bearer ")
            || lower.contains("api_key")
            || lower.contains("apikey")
            || lower.contains("access_token")
            || lower.contains("cookie")
            || lower.contains("secret")
            || lower.contains("\\.codex")
            || lower.contains("/.codex")
        {
            redacted.push_str("[redacted-log-line]\n");
        } else {
            redacted.push_str(line);
            redacted.push('\n');
        }
    }
    redacted.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{TenantId, WorkflowExecutionId, WorkflowStatus};
    use std::sync::{Mutex, OnceLock};

    #[test]
    fn task_context_parses_safe_execution_fields() {
        let assistant_run_id = AssistantRunId::new();
        let execution = WorkflowExecution {
            id: WorkflowExecutionId::new(),
            tenant_id: TenantId::new(),
            dataset_id: None,
            report_plan_id: None,
            kind: WorkflowKind::CodexHostTask,
            version: "0.1.0".to_string(),
            stage: "run_codex_host_task".to_string(),
            status: WorkflowStatus::Running,
            attempt: 0,
            context: json!({
                "assistant_run_id": assistant_run_id.to_string(),
                "capability": "inspect_project",
                "task": "Summarize repository shape",
                "local_thread_id": "thread-a",
                "task_memory_policy": {
                    "kind": "task",
                    "isolated": true,
                    "memory_space_id": "codex-host-task:run-a"
                },
                "task_memory_space_id": "codex-host-task:run-a",
                "workspace_seed": {
                    "schema": "v3.codex_host_workspace_seed",
                    "version": 1,
                    "kind": "generated_static_page_edit"
                }
            }),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let context = CodexHostTaskContext::from_execution(&execution).expect("context");
        assert_eq!(context.assistant_run_id, assistant_run_id);
        assert_eq!(context.capability, "inspect_project");
        assert_eq!(context.task.as_deref(), Some("Summarize repository shape"));
        assert!(context.task_memory_isolated);
        assert_eq!(
            context.task_memory_space_id.as_deref(),
            Some("codex-host-task:run-a")
        );
        assert_eq!(
            context
                .workspace_seed
                .as_ref()
                .and_then(|seed| seed.get("kind")),
            Some(&json!("generated_static_page_edit"))
        );
        let dry_run_output = context.dry_run_output();
        assert_eq!(dry_run_output["codex_invoked"], json!(false));
        assert_eq!(
            dry_run_output["task_memory_space_id"],
            json!("codex-host-task:run-a")
        );
        assert_eq!(
            dry_run_output["html_artifacts"][0]["template_id"],
            json!("codex_execution_report")
        );
        assert_eq!(
            dry_run_output["html_artifacts"][0]["payload"]["mode"],
            json!("dry_run")
        );
    }

    #[test]
    fn dry_run_profile_allows_policy_without_command_plan() {
        let context = test_context("inspect_project", Some("Summarize repository shape"));
        let policy = CodexHostAgentPolicy {
            mode: CodexHostExecutionMode::DryRun,
            profile: CodexHostProfile {
                id: "default-dry-run".to_string(),
                kind: "dry-run".to_string(),
                model: None,
                provider_id: None,
                base_url: None,
                env_key: None,
                wire_api: None,
                allowed_capabilities: Vec::new(),
            },
            host_kind: "developer_workstation".to_string(),
            allow_real_codex_exec: false,
            task_workspace_root: None,
        };

        let decision = policy.prepare(&context).expect("decision");

        assert_eq!(decision.mode, CodexHostExecutionMode::DryRun);
        assert_eq!(decision.host_kind, "developer_workstation");
        assert!(decision.command_plan.is_none());
    }

    #[test]
    fn plan_only_builds_redacted_command_summary() {
        let context = test_context("inspect_project", Some("Read the repo and summarize it"));
        let policy = CodexHostAgentPolicy {
            mode: CodexHostExecutionMode::PlanOnly,
            profile: CodexHostProfile {
                id: "readonly".to_string(),
                kind: "codex-native".to_string(),
                model: Some("gpt-5.3-codex".to_string()),
                provider_id: None,
                base_url: None,
                env_key: None,
                wire_api: None,
                allowed_capabilities: vec!["inspect_project".to_string()],
            },
            host_kind: "developer_workstation".to_string(),
            allow_real_codex_exec: false,
            task_workspace_root: Some(PathBuf::from("D:/codex-host/tasks")),
        };

        let decision = policy.prepare(&context).expect("decision");
        let planned_output = context.planned_output(&decision);
        let plan = decision.command_plan.expect("command plan");
        let summary = plan.safe_summary();

        assert_eq!(
            planned_output["html_artifacts"][0]["payload"]["workspaceLabel"],
            json!("codex-host-task-test")
        );
        assert_eq!(planned_output["host_kind"], json!("developer_workstation"));
        assert_eq!(
            planned_output["html_artifacts"][0]["payload"]["sandbox"],
            json!("read-only")
        );
        assert_eq!(summary.program, "codex");
        assert_eq!(summary.sandbox, "read-only");
        assert!(summary.workspace_configured);
        assert_eq!(
            summary.workspace_label.as_deref(),
            Some("codex-host-task-test")
        );
        assert_eq!(
            plan.workspace_path.as_deref(),
            Some(Path::new("D:/codex-host/tasks/codex-host-task-test"))
        );
        assert!(summary.prompt_redacted);
        assert_eq!(summary.prompt_chars, "Read the repo and summarize it".len());
        assert_eq!(plan.args_without_prompt.last().unwrap(), "gpt-5.3-codex");
        assert!(plan.args_without_prompt.contains(&"-a".to_string()));
        assert!(plan
            .args_without_prompt
            .contains(&"--skip-git-repo-check".to_string()));
        assert!(!plan
            .args_without_prompt
            .contains(&"--ask-for-approval".to_string()));
        assert_eq!(
            plan.process_args().last().unwrap(),
            "Read the repo and summarize it"
        );
    }

    #[test]
    fn plan_only_rejects_unprofiled_capability() {
        let context = test_context("secret_read", Some("Read local secrets"));
        let policy = CodexHostAgentPolicy {
            mode: CodexHostExecutionMode::PlanOnly,
            profile: CodexHostProfile {
                id: "readonly".to_string(),
                kind: "codex-native".to_string(),
                model: None,
                provider_id: None,
                base_url: None,
                env_key: None,
                wire_api: None,
                allowed_capabilities: vec!["inspect_project".to_string()],
            },
            host_kind: "developer_workstation".to_string(),
            allow_real_codex_exec: false,
            task_workspace_root: None,
        };

        let error = policy.prepare(&context).expect_err("should reject");

        assert!(error.to_string().contains("not allowed"));
    }

    #[test]
    fn customer_complex_request_uses_read_only_sandbox() {
        let context = test_context(
            CUSTOMER_COMPLEX_REQUEST,
            Some("Analyze this complex customer request and return an action plan."),
        );
        let policy = plan_only_policy_for_capability(CUSTOMER_COMPLEX_REQUEST);

        let decision = policy.prepare(&context).expect("decision");
        let plan = decision.command_plan.expect("command plan");

        assert_eq!(plan.sandbox, "read-only");
    }

    #[test]
    fn customer_complex_request_prompt_requires_safe_result_summary() {
        let _lock = test_env_lock().lock().expect("env lock");
        let _schema_enabled =
            TestEnvVarRestore::set(ENV_CUSTOMER_RESULT_OUTPUT_SCHEMA_ENABLED, "true");
        let context = test_context(
            CUSTOMER_COMPLEX_REQUEST,
            Some("Analyze this complex customer request and return an action plan."),
        );
        let policy = plan_only_policy_for_capability(CUSTOMER_COMPLEX_REQUEST);

        let decision = policy.prepare(&context).expect("decision");
        let plan = decision.command_plan.expect("command plan");

        assert!(plan.prompt.contains("DataMax Web Codex customer task"));
        assert!(plan.prompt.contains("Use read-only analysis"));
        assert!(plan.prompt.contains("customer_result_summary"));
        assert!(plan.prompt.contains("raw_logs_exposed"));
        assert!(plan
            .prompt
            .contains("Analyze this complex customer request"));
        assert!(!plan.prompt.contains("propose_patch"));
        assert!(plan
            .args_without_prompt
            .windows(2)
            .any(|window| window[0] == "--output-schema"
                && window[1] == CUSTOMER_RESULT_OUTPUT_SCHEMA_PATH));
    }

    #[test]
    fn customer_artifact_request_uses_workspace_write_sandbox() {
        let _lock = test_env_lock().lock().expect("env lock");
        let _schema_enabled =
            TestEnvVarRestore::set(ENV_CUSTOMER_RESULT_OUTPUT_SCHEMA_ENABLED, "true");
        let context = test_context(
            CUSTOMER_ARTIFACT_REQUEST,
            Some("Create customer-facing artifacts in the isolated task workspace."),
        );
        let policy = plan_only_policy_for_capability(CUSTOMER_ARTIFACT_REQUEST);

        let decision = policy.prepare(&context).expect("decision");
        let plan = decision.command_plan.expect("command plan");

        assert_eq!(plan.sandbox, "workspace-write");
        assert_eq!(
            plan.workspace_path.as_deref(),
            Some(Path::new("D:/codex-host/tasks/codex-host-task-test"))
        );
        assert!(plan
            .prompt
            .contains("DataMax Web Codex customer artifact task"));
        assert!(plan.prompt.contains("customer-artifact-manifest.json"));
        assert!(plan.prompt.contains("customer_result_summary"));
        assert!(plan
            .args_without_prompt
            .windows(2)
            .any(|window| window[0] == "--output-schema"
                && window[1] == CUSTOMER_RESULT_OUTPUT_SCHEMA_PATH));
    }

    #[test]
    fn generated_static_page_edit_uses_workspace_write_sandbox() {
        let context = test_context(
            GENERATED_STATIC_PAGE_EDIT,
            Some("Edit the generated static page in the task workspace."),
        );
        let policy = plan_only_policy_for_capability(GENERATED_STATIC_PAGE_EDIT);

        let decision = policy.prepare(&context).expect("decision");
        let plan = decision.command_plan.expect("command plan");

        assert_eq!(plan.sandbox, "workspace-write");
        assert_eq!(
            plan.workspace_path.as_deref(),
            Some(Path::new("D:/codex-host/tasks/codex-host-task-test"))
        );
    }

    #[test]
    fn generated_static_page_publish_uses_workspace_write_sandbox() {
        let context = test_context(
            GENERATED_STATIC_PAGE_PUBLISH,
            Some("Publish a new generated static page artifact from the workspace."),
        );
        let policy = plan_only_policy_for_capability(GENERATED_STATIC_PAGE_PUBLISH);

        let decision = policy.prepare(&context).expect("decision");
        let plan = decision.command_plan.expect("command plan");

        assert_eq!(plan.sandbox, "workspace-write");
    }

    #[test]
    fn v3_product_change_request_is_blocked_even_when_profile_allows_it() {
        let context = test_context(
            V3_PRODUCT_CHANGE_REQUEST,
            Some("Change the V3 product login behavior."),
        );
        let policy = plan_only_policy_for_capability(V3_PRODUCT_CHANGE_REQUEST);

        let error = policy
            .prepare(&context)
            .expect_err("should require operator review");

        assert!(error.to_string().contains("operator review"));
    }

    #[test]
    fn plan_only_allows_static_page_image2_data_publish_template() {
        let mut context = test_context(
            STATIC_PAGE_IMAGE2_DATA_PUBLISH,
            Some("Run the fixed static-page template package."),
        );
        context.fixed_task =
            Some(CodexHostFixedTaskTemplateContextView::static_page_image2_data_publish_example());
        let policy = fixed_task_policy(
            CodexHostExecutionMode::PlanOnly,
            STATIC_PAGE_IMAGE2_DATA_PUBLISH,
        );

        let decision = policy.prepare(&context).expect("decision");
        let output = context.planned_output(&decision);

        assert_eq!(decision.mode, CodexHostExecutionMode::PlanOnly);
        assert_eq!(decision.host_kind, "cloudflare_codex");
        assert_eq!(
            decision
                .command_plan
                .as_ref()
                .expect("command plan")
                .sandbox,
            "workspace-write"
        );
        assert_eq!(
            output["html_artifacts"][0]["payload"]["fixedTask"]["templateId"],
            json!(STATIC_PAGE_IMAGE2_DATA_PUBLISH)
        );
    }

    #[test]
    fn plan_only_allows_answer_quality_autofix_template() {
        let mut context = test_context(
            ANSWER_QUALITY_AUTOFIX,
            Some("Run the fixed answer-quality autofix template package."),
        );
        context.fixed_task =
            Some(CodexHostFixedTaskTemplateContextView::answer_quality_autofix_example());
        let policy = fixed_task_policy(CodexHostExecutionMode::PlanOnly, ANSWER_QUALITY_AUTOFIX);

        let decision = policy.prepare(&context).expect("decision");

        assert_eq!(decision.host_kind, "cloudflare_codex");
        assert_eq!(
            decision
                .command_plan
                .as_ref()
                .expect("command plan")
                .sandbox,
            "workspace-write"
        );
    }

    #[test]
    fn plan_only_allows_data_ingestion_analysis_template() {
        let mut context = test_context(
            DATA_INGESTION_ANALYSIS,
            Some("Run the fixed data-ingestion analysis template package."),
        );
        context.fixed_task =
            Some(CodexHostFixedTaskTemplateContextView::data_ingestion_analysis_example());
        let policy = fixed_task_policy(CodexHostExecutionMode::PlanOnly, DATA_INGESTION_ANALYSIS);

        let decision = policy.prepare(&context).expect("decision");
        let plan = decision.command_plan.expect("command plan");

        assert_eq!(decision.host_kind, "cloudflare_codex");
        assert_eq!(plan.sandbox, "workspace-write");
        assert!(plan.prompt.contains("data_ingestion_analysis"));
    }

    #[test]
    fn data_ingestion_template_requires_selected_source_scope() {
        let mut fixed_task =
            CodexHostFixedTaskTemplateContextView::data_ingestion_analysis_example();
        fixed_task.dataset_scope = json!({
            "tenant_id": "tenant-1",
            "dataset_ids": [],
            "database_source_ids": [],
            "selected_document_ids": [],
            "uploaded_file_ids": []
        });
        let mut context = test_context(
            DATA_INGESTION_ANALYSIS,
            Some("Run the fixed data-ingestion analysis template package."),
        );
        context.fixed_task = Some(fixed_task);
        let policy = fixed_task_policy(CodexHostExecutionMode::PlanOnly, DATA_INGESTION_ANALYSIS);

        let error = policy.prepare(&context).expect_err("should reject");

        assert!(error
            .to_string()
            .contains("selected dataset, document, file, table, or database source"));
    }

    #[test]
    fn data_ingestion_template_requires_read_only_analysis_policies() {
        for (policy_key, bad_value, expected) in [
            (
                "mode",
                json!("production_import"),
                "mode=read_only_analysis_or_staging_spec",
            ),
            (
                "credential_policy",
                json!("request_credentials_if_missing"),
                "credential_policy=do_not_request_or_emit_credentials",
            ),
            (
                "production_write_policy",
                json!("allow_without_confirmation"),
                "production_write_policy=needs_human_confirmation",
            ),
            (
                "public_api_change_allowed",
                json!(true),
                "public_api_change_allowed=false",
            ),
            (
                "schema_change_allowed_without_confirmation",
                json!(true),
                "schema_change_allowed_without_confirmation=false",
            ),
        ] {
            let mut fixed_task =
                CodexHostFixedTaskTemplateContextView::data_ingestion_analysis_example();
            fixed_task.policies[policy_key] = bad_value;
            let mut context = test_context(
                DATA_INGESTION_ANALYSIS,
                Some("Run the fixed data-ingestion analysis template package."),
            );
            context.fixed_task = Some(fixed_task);
            let policy =
                fixed_task_policy(CodexHostExecutionMode::PlanOnly, DATA_INGESTION_ANALYSIS);

            let error = policy.prepare(&context).expect_err("should reject");

            assert!(
                error.to_string().contains(expected),
                "bad {policy_key} should mention {expected}, got {error}"
            );
        }
    }

    #[test]
    fn data_ingestion_template_rejects_unapproved_host_kind_for_plan_only() {
        let mut context = test_context(
            DATA_INGESTION_ANALYSIS,
            Some("Run the fixed data-ingestion analysis template package."),
        );
        context.fixed_task =
            Some(CodexHostFixedTaskTemplateContextView::data_ingestion_analysis_example());
        let mut policy =
            fixed_task_policy(CodexHostExecutionMode::PlanOnly, DATA_INGESTION_ANALYSIS);
        policy.host_kind = "developer_workstation".to_string();

        let error = policy.prepare(&context).expect_err("should reject");

        assert!(error.to_string().contains("blocked on host kind"));
    }

    #[test]
    fn codex_exec_rejects_untemplated_write_capability() {
        let context = test_context(STATIC_PAGE_IMAGE2_DATA_PUBLISH, Some("Publish a page"));
        let policy = fixed_task_policy(
            CodexHostExecutionMode::CodexExec,
            STATIC_PAGE_IMAGE2_DATA_PUBLISH,
        );

        let error = policy.prepare(&context).expect_err("should reject");

        assert!(error.to_string().contains("requires a fixed_task"));
    }

    #[test]
    fn static_page_template_requires_generated_artifact_publish_mode() {
        let mut fixed_task =
            CodexHostFixedTaskTemplateContextView::static_page_image2_data_publish_example();
        fixed_task.policies = json!({
            "publish_mode": "overwrite_existing_artifact"
        });
        let mut context = test_context(
            STATIC_PAGE_IMAGE2_DATA_PUBLISH,
            Some("Run the fixed static-page template package."),
        );
        context.fixed_task = Some(fixed_task);
        let policy = fixed_task_policy(
            CodexHostExecutionMode::PlanOnly,
            STATIC_PAGE_IMAGE2_DATA_PUBLISH,
        );

        let error = policy.prepare(&context).expect_err("should reject");

        assert!(error
            .to_string()
            .contains("publish_mode=new_generated_artifact_only"));
    }

    #[test]
    fn static_page_template_requires_preview_ready_or_preview_asset_key() {
        let mut fixed_task =
            CodexHostFixedTaskTemplateContextView::static_page_image2_data_publish_example();
        fixed_task.image2["visual_contract_status"] = json!("queued");
        fixed_task.image2["preview_asset_key"] = Value::Null;
        let mut context = test_context(
            STATIC_PAGE_IMAGE2_DATA_PUBLISH,
            Some("Run the fixed static-page template package."),
        );
        context.fixed_task = Some(fixed_task);
        let policy = fixed_task_policy(
            CodexHostExecutionMode::PlanOnly,
            STATIC_PAGE_IMAGE2_DATA_PUBLISH,
        );

        let error = policy.prepare(&context).expect_err("should reject");

        assert!(error
            .to_string()
            .contains("visual_contract_status=preview_ready"));
    }

    #[test]
    fn static_page_template_requires_dataset_or_database_scope() {
        let mut fixed_task =
            CodexHostFixedTaskTemplateContextView::static_page_image2_data_publish_example();
        fixed_task.dataset_scope = json!({
            "tenant_id": "tenant-1",
            "dataset_ids": [],
            "database_source_ids": [],
            "selected_document_ids": []
        });
        let mut context = test_context(
            STATIC_PAGE_IMAGE2_DATA_PUBLISH,
            Some("Run the fixed static-page template package."),
        );
        context.fixed_task = Some(fixed_task);
        let policy = fixed_task_policy(
            CodexHostExecutionMode::PlanOnly,
            STATIC_PAGE_IMAGE2_DATA_PUBLISH,
        );

        let error = policy.prepare(&context).expect_err("should reject");

        assert!(error
            .to_string()
            .contains("selected dataset, document, or database source"));
    }

    #[test]
    fn static_page_template_requires_monthly_time_range_contract() {
        let mut fixed_task =
            CodexHostFixedTaskTemplateContextView::static_page_image2_data_publish_example();
        fixed_task.requirements["dynamic_page_contract"]["report_time_range"]
            ["default_granularity"] = json!("day");
        let mut context = test_context(
            STATIC_PAGE_IMAGE2_DATA_PUBLISH,
            Some("Run the fixed static-page template package."),
        );
        context.fixed_task = Some(fixed_task);
        let policy = fixed_task_policy(
            CodexHostExecutionMode::PlanOnly,
            STATIC_PAGE_IMAGE2_DATA_PUBLISH,
        );

        let error = policy.prepare(&context).expect_err("should reject");

        assert!(error
            .to_string()
            .contains("report_time_range.default_granularity=month"));
    }

    #[test]
    fn static_page_template_requires_snapshot_unit_and_detail_policies() {
        for (policy_key, expected) in [
            ("snapshot_aggregation", "snapshot aggregation policy"),
            ("trend_aggregation", "trend aggregation policy"),
            ("unit_rendering", "unit rendering policy"),
            ("detail_table_policy", "detail table policy"),
        ] {
            let mut fixed_task =
                CodexHostFixedTaskTemplateContextView::static_page_image2_data_publish_example();
            fixed_task.policies[policy_key] = Value::Null;
            let mut context = test_context(
                STATIC_PAGE_IMAGE2_DATA_PUBLISH,
                Some("Run the fixed static-page template package."),
            );
            context.fixed_task = Some(fixed_task);
            let policy = fixed_task_policy(
                CodexHostExecutionMode::PlanOnly,
                STATIC_PAGE_IMAGE2_DATA_PUBLISH,
            );

            let error = policy.prepare(&context).expect_err("should reject");

            assert!(
                error.to_string().contains(expected),
                "missing {policy_key} should mention {expected}, got {error}"
            );
        }
    }

    #[test]
    fn static_page_template_rejects_effect_image_confirmation_required_true() {
        let mut fixed_task =
            CodexHostFixedTaskTemplateContextView::static_page_image2_data_publish_example();
        fixed_task.policies["effect_image_confirmation_required"] = json!(true);
        let mut context = test_context(
            STATIC_PAGE_IMAGE2_DATA_PUBLISH,
            Some("Run the fixed static-page template package."),
        );
        context.fixed_task = Some(fixed_task);
        let policy = fixed_task_policy(
            CodexHostExecutionMode::PlanOnly,
            STATIC_PAGE_IMAGE2_DATA_PUBLISH,
        );

        let error = policy.prepare(&context).expect_err("should reject");

        assert!(error
            .to_string()
            .contains("effect_image_confirmation_required=false"));
    }

    #[test]
    fn answer_quality_template_rejects_non_allowlisted_file_scope() {
        let mut fixed_task =
            CodexHostFixedTaskTemplateContextView::answer_quality_autofix_example();
        fixed_task
            .allowed_write_scope
            .as_mut()
            .expect("scope")
            .files
            .push("apps/web/app/globals.css".to_string());
        let mut context = test_context(
            ANSWER_QUALITY_AUTOFIX,
            Some("Run the fixed answer-quality autofix template package."),
        );
        context.fixed_task = Some(fixed_task);
        let policy = fixed_task_policy(CodexHostExecutionMode::PlanOnly, ANSWER_QUALITY_AUTOFIX);

        let error = policy.prepare(&context).expect_err("should reject");

        assert!(error.to_string().contains("not allowlisted"));
    }

    #[test]
    fn codex_exec_requires_real_host_guard() {
        let context = test_context("inspect_project", Some("Read the repo"));
        let policy = CodexHostAgentPolicy {
            mode: CodexHostExecutionMode::CodexExec,
            profile: CodexHostProfile {
                id: "readonly".to_string(),
                kind: "codex-native".to_string(),
                model: None,
                provider_id: None,
                base_url: None,
                env_key: None,
                wire_api: None,
                allowed_capabilities: vec!["inspect_project".to_string()],
            },
            host_kind: "developer_workstation".to_string(),
            allow_real_codex_exec: true,
            task_workspace_root: Some(PathBuf::from("D:/codex-host/tasks")),
        };

        let error = policy
            .prepare(&context)
            .expect_err("should reject local host");

        assert!(error.to_string().contains("developer_workstation"));
    }

    #[test]
    fn codex_exec_requires_task_workspace_root() {
        let context = test_context("inspect_project", Some("Read the repo"));
        let policy = CodexHostAgentPolicy {
            mode: CodexHostExecutionMode::CodexExec,
            profile: CodexHostProfile {
                id: "readonly".to_string(),
                kind: "codex-native".to_string(),
                model: None,
                provider_id: None,
                base_url: None,
                env_key: None,
                wire_api: None,
                allowed_capabilities: vec!["inspect_project".to_string()],
            },
            host_kind: "windows_jump".to_string(),
            allow_real_codex_exec: true,
            task_workspace_root: None,
        };

        let error = policy
            .prepare(&context)
            .expect_err("should require task workspace root");

        assert!(error.to_string().contains("TASK_WORKSPACE_ROOT"));
    }

    #[test]
    fn codex_exec_guard_allows_jump_host_profile_with_task_workspace() {
        let context = test_context("inspect_project", Some("Read the repo"));
        let policy = CodexHostAgentPolicy {
            mode: CodexHostExecutionMode::CodexExec,
            profile: CodexHostProfile {
                id: "readonly".to_string(),
                kind: "codex-native".to_string(),
                model: None,
                provider_id: None,
                base_url: None,
                env_key: None,
                wire_api: None,
                allowed_capabilities: vec!["inspect_project".to_string()],
            },
            host_kind: "windows_jump".to_string(),
            allow_real_codex_exec: true,
            task_workspace_root: Some(PathBuf::from("D:/codex-host/tasks")),
        };

        let decision = policy.prepare(&context).expect("decision");

        assert_eq!(decision.mode, CodexHostExecutionMode::CodexExec);
        assert_eq!(decision.host_kind, "windows_jump");
        let plan = decision.command_plan.expect("command plan");
        assert_eq!(
            plan.workspace_path.as_deref(),
            Some(Path::new("D:/codex-host/tasks/codex-host-task-test"))
        );
    }

    #[test]
    fn codex_exec_rejects_v3_product_repo_workspace_root() {
        let context = test_context(GENERATED_STATIC_PAGE_EDIT, Some("Edit generated page"));
        let policy = CodexHostAgentPolicy {
            mode: CodexHostExecutionMode::CodexExec,
            profile: compatible_shim_test_profile(GENERATED_STATIC_PAGE_EDIT),
            host_kind: "aiv3_server".to_string(),
            allow_real_codex_exec: true,
            task_workspace_root: Some(PathBuf::from(V3_SERVER_REPO_ROOT)),
        };

        let error = policy
            .prepare(&context)
            .expect_err("should reject product repo workspace");

        assert!(error.to_string().contains("V3 product repository"));
    }

    #[test]
    fn codex_exec_rejects_parent_directory_workspace_root() {
        let context = test_context(GENERATED_STATIC_PAGE_EDIT, Some("Edit generated page"));
        let policy = CodexHostAgentPolicy {
            mode: CodexHostExecutionMode::CodexExec,
            profile: compatible_shim_test_profile(GENERATED_STATIC_PAGE_EDIT),
            host_kind: "aiv3_server".to_string(),
            allow_real_codex_exec: true,
            task_workspace_root: Some(PathBuf::from("/srv/aiv3/repo/../codex-workspaces")),
        };

        let error = policy
            .prepare(&context)
            .expect_err("should reject parent directory workspace");

        assert!(error.to_string().contains("parent-directory"));
    }

    #[test]
    fn compatible_shim_builds_private_provider_overrides() {
        let context = test_context("inspect_project", Some("Read the repo"));
        let policy = CodexHostAgentPolicy {
            mode: CodexHostExecutionMode::PlanOnly,
            profile: CodexHostProfile {
                id: "minimax-private".to_string(),
                kind: "codex-compatible-shim".to_string(),
                model: Some("MiniMax-M2.7".to_string()),
                provider_id: Some("minimax".to_string()),
                base_url: Some("https://api.minimaxi.com/v1".to_string()),
                env_key: Some("MINIMAX_API_KEY".to_string()),
                wire_api: Some("responses".to_string()),
                allowed_capabilities: vec!["inspect_project".to_string()],
            },
            host_kind: "windows_jump".to_string(),
            allow_real_codex_exec: false,
            task_workspace_root: None,
        };

        let decision = policy.prepare(&context).expect("decision");
        let plan = decision.command_plan.expect("command plan");
        let args = plan.args_without_prompt.join(" ");

        assert!(args.contains("model_provider=\"minimax\""));
        assert!(args.contains("model_providers.minimax.base_url=\"https://api.minimaxi.com/v1\""));
        assert!(args.contains("model_providers.minimax.env_key=\"MINIMAX_API_KEY\""));
        assert!(args.contains("model_providers.minimax.wire_api=\"responses\""));
        assert!(args.contains("MiniMax-M2.7"));
    }

    #[test]
    fn rightcode_shim_profile_uses_named_rightcode_env_key() {
        let context = test_context("inspect_project", Some("Read the repo"));
        let profile = compatible_shim_test_profile("inspect_project");

        let plan =
            build_codex_command_plan(&context, &profile, Some(Path::new("D:/codex-host/tasks")))
                .expect("command plan");
        let args = plan.args_without_prompt.join(" ");

        assert!(args.contains("model_provider=\"rightcode\""));
        assert!(args.contains("model_providers.rightcode.env_key=\"RIGHTCODE_API_KEY_MAIN\""));
        assert!(!args.contains("OPENAI_API_KEY"));
    }

    #[test]
    fn compatible_shim_rejects_secret_like_env_key() {
        let context = test_context("inspect_project", Some("Read the repo"));
        let mut profile = compatible_shim_test_profile("inspect_project");
        profile.env_key = Some("sk-rightcode-secret-value".to_string());

        let error =
            build_codex_command_plan(&context, &profile, Some(Path::new("D:/codex-host/tasks")))
                .expect_err("should reject secret-like env key");

        assert!(error.to_string().contains("environment variable name"));
    }

    #[test]
    fn static_page_reasoning_effort_overrides_codex_global_config() {
        let _lock = test_env_lock().lock().expect("env lock");
        let _static_effort = TestEnvVarRestore::set(ENV_STATIC_PAGE_REASONING_EFFORT, "medium");
        let _general_effort = TestEnvVarRestore::unset(ENV_MODEL_REASONING_EFFORT);
        let context = test_context(STATIC_PAGE_IMAGE2_DATA_PUBLISH, Some("Publish page"));
        let profile = compatible_shim_test_profile(STATIC_PAGE_IMAGE2_DATA_PUBLISH);

        let plan =
            build_codex_command_plan(&context, &profile, Some(Path::new("D:/codex-host/tasks")))
                .expect("command plan");
        let args = plan.args_without_prompt.join(" ");

        assert!(args.contains("model_reasoning_effort=\"medium\""));
    }

    #[test]
    fn general_reasoning_effort_applies_to_non_static_page_task() {
        let _lock = test_env_lock().lock().expect("env lock");
        let _static_effort = TestEnvVarRestore::unset(ENV_STATIC_PAGE_REASONING_EFFORT);
        let _general_effort = TestEnvVarRestore::set(ENV_MODEL_REASONING_EFFORT, "low");
        let context = test_context("inspect_project", Some("Read the repo"));
        let profile = compatible_shim_test_profile("inspect_project");

        let plan =
            build_codex_command_plan(&context, &profile, Some(Path::new("D:/codex-host/tasks")))
                .expect("command plan");
        let args = plan.args_without_prompt.join(" ");

        assert!(args.contains("model_reasoning_effort=\"low\""));
    }

    #[test]
    fn reasoning_effort_rejects_unknown_value() {
        let _lock = test_env_lock().lock().expect("env lock");
        let _static_effort = TestEnvVarRestore::set(ENV_STATIC_PAGE_REASONING_EFFORT, "turbo");
        let _general_effort = TestEnvVarRestore::unset(ENV_MODEL_REASONING_EFFORT);
        let context = test_context(STATIC_PAGE_IMAGE2_DATA_PUBLISH, Some("Publish page"));
        let profile = compatible_shim_test_profile(STATIC_PAGE_IMAGE2_DATA_PUBLISH);

        let error =
            build_codex_command_plan(&context, &profile, Some(Path::new("D:/codex-host/tasks")))
                .expect_err("should reject unknown effort");

        assert!(error
            .to_string()
            .contains("unsupported Codex reasoning effort"));
    }

    #[test]
    fn safe_log_excerpt_redacts_secret_lines_and_truncates() {
        let raw = b"ok\nAuthorization: Bearer abc\napi_key=xyz\nnormal line after secret\n";

        let excerpt = safe_log_excerpt(raw, 40);

        assert!(excerpt.contains("ok"));
        assert!(excerpt.contains("[redacted-log-line]"));
        assert!(!excerpt.contains("abc"));
        assert!(!excerpt.contains("xyz"));
        assert!(excerpt.chars().count() <= 40);
    }

    #[test]
    fn runtime_config_reads_bounded_env_values() {
        let _lock = test_env_lock().lock().expect("env lock");
        let _timeout = TestEnvVarRestore::set("CODEX_HOST_AGENT_TASK_TIMEOUT_MS", "42");
        let _heartbeat = TestEnvVarRestore::set("CODEX_HOST_AGENT_HEARTBEAT_MS", "invalid");
        let _stdout = TestEnvVarRestore::set("CODEX_HOST_AGENT_STDOUT_LIMIT_BYTES", "12");
        let _stderr = TestEnvVarRestore::set("CODEX_HOST_AGENT_STDERR_LIMIT_BYTES", "0");
        let _retention =
            TestEnvVarRestore::set("CODEX_HOST_AGENT_TASK_WORKSPACE_RETENTION_HOURS", "336");

        let config = CodexHostRuntimeConfig::from_env();

        assert_eq!(config.task_timeout_ms(), 42);
        assert_eq!(config.heartbeat_ms(), DEFAULT_HEARTBEAT_MS);
        assert_eq!(config.stdout_limit_bytes(), 12);
        assert_eq!(config.stderr_limit_bytes(), DEFAULT_STDERR_LIMIT_BYTES);
        assert_eq!(config.task_workspace_retention_hours(), 336);
    }

    #[test]
    fn runtime_config_default_task_timeout_allows_slow_cloudflare_tasks() {
        let _lock = test_env_lock().lock().expect("env lock");
        let _timeout = TestEnvVarRestore::unset("CODEX_HOST_AGENT_TASK_TIMEOUT_MS");
        let _retention =
            TestEnvVarRestore::unset("CODEX_HOST_AGENT_TASK_WORKSPACE_RETENTION_HOURS");

        let config = CodexHostRuntimeConfig::from_env();

        assert_eq!(config.task_timeout_ms(), 1_800_000);
        assert_eq!(
            config.task_workspace_retention_hours(),
            DEFAULT_TASK_WORKSPACE_RETENTION_HOURS
        );
    }

    #[test]
    fn codex_exec_extracts_static_page_fixed_task_output_from_stdout() {
        let stdout = br#"
thinking...
{
  "template_id": "static_page_image2_data_publish",
  "status": "success",
  "artifact": {
    "local_path": "/srv/aiv3/shared/objects/generated-artifacts/database-static-pages/run/page",
    "public_url": "https://v3.elepcloud.com/generated-artifacts/database-static-pages/run/page/index.html",
    "manifest_path": "/srv/aiv3/shared/objects/generated-artifacts/database-static-pages/run/page/manifest.json"
  },
  "validation_report": {
    "snapshot_policy": "latest_snapshot_for_state_modules",
    "latest_snapshot": "2026-05-10",
    "source_row_count": 862,
    "current_state_row_count": 851,
    "detail_row_count": 851,
    "unit_policy": "raw_value_checked_then_wan_or_yi",
    "warnings": []
  },
  "source_summary": ["2026-05-10 snapshot"],
  "human_review_reason": null
}
"#;

        let output = extract_fixed_task_output_from_stdout(stdout, STATIC_PAGE_IMAGE2_DATA_PUBLISH)
            .expect("fixed output");

        assert_eq!(
            output["template_id"],
            json!(STATIC_PAGE_IMAGE2_DATA_PUBLISH)
        );
        assert_eq!(output["status"], json!("success"));
        assert_eq!(output["validation_report"]["detail_row_count"], json!(851));
    }

    #[test]
    fn codex_exec_extracts_answer_quality_fixed_task_output_from_stdout() {
        let stdout = br#"
summary text before final output
{"fixed_task_output":{"template_id":"answer_quality_autofix","status":"patch_ready","failure_type":"retrieval_supply","root_cause":"table rows were not supplied","changed_files":["crates/platform-api/src/lib.rs"],"tests_added":["assistant_run_answer_quality_regression"],"test_commands":["cargo test -p platform-api assistant_run_answer_quality --lib"],"risk_level":"low","rollback_notes":"revert answer quality helper change","human_review_reason":null}}
"#;

        let output = extract_fixed_task_output_from_stdout(stdout, ANSWER_QUALITY_AUTOFIX)
            .expect("fixed output");

        assert_eq!(output["template_id"], json!(ANSWER_QUALITY_AUTOFIX));
        assert_eq!(output["status"], json!("patch_ready"));
        assert_eq!(
            output["changed_files"],
            json!(["crates/platform-api/src/lib.rs"])
        );
    }

    #[test]
    fn codex_exec_rejects_missing_fixed_task_output_for_fixed_template() {
        let error = extract_fixed_task_output_from_stdout(
            b"Codex completed but did not print JSON.",
            STATIC_PAGE_IMAGE2_DATA_PUBLISH,
        )
        .expect_err("missing output should fail");

        assert!(error.to_string().contains("fixed task output"));
    }

    #[test]
    fn customer_web_codex_output_schema_materializes_for_customer_capabilities() {
        let _lock = test_env_lock().lock().expect("env lock");
        let _schema_enabled =
            TestEnvVarRestore::set(ENV_CUSTOMER_RESULT_OUTPUT_SCHEMA_ENABLED, "true");
        let context = test_context(
            CUSTOMER_COMPLEX_REQUEST,
            Some("Analyze this complex customer request."),
        );
        let workspace = std::env::temp_dir().join(format!(
            "v3-codex-host-customer-schema-test-{}",
            Uuid::new_v4()
        ));

        materialize_customer_web_codex_output_schema(&workspace, &context)
            .expect("schema should be materialized");

        let schema_path = workspace.join(CUSTOMER_RESULT_OUTPUT_SCHEMA_PATH);
        let schema_text = fs::read_to_string(&schema_path).expect("schema should be readable");
        let schema: Value = serde_json::from_str(&schema_text).expect("schema should be json");

        assert_eq!(
            schema["properties"]["customer_result_summary"]["properties"]["schema"]["const"],
            json!("v3.customer_codex_result_summary")
        );
        assert_eq!(
            schema["properties"]["customer_result_summary"]["properties"]["safety"]["properties"]
                ["raw_logs_exposed"]["const"],
            json!(false)
        );

        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn customer_web_codex_output_schema_skips_non_customer_capabilities() {
        let _lock = test_env_lock().lock().expect("env lock");
        let _schema_enabled =
            TestEnvVarRestore::set(ENV_CUSTOMER_RESULT_OUTPUT_SCHEMA_ENABLED, "true");
        let context = test_context("inspect_project", Some("Read the repo."));
        let workspace = std::env::temp_dir().join(format!(
            "v3-codex-host-non-customer-schema-test-{}",
            Uuid::new_v4()
        ));

        materialize_customer_web_codex_output_schema(&workspace, &context)
            .expect("non customer should skip schema");

        assert!(!workspace.join(CUSTOMER_RESULT_OUTPUT_SCHEMA_PATH).exists());

        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn fixed_task_bundle_materializes_workspace_files() {
        let mut context = test_context(
            STATIC_PAGE_IMAGE2_DATA_PUBLISH,
            Some("Run the fixed static-page template package."),
        );
        context.fixed_task =
            Some(CodexHostFixedTaskTemplateContextView::static_page_image2_data_publish_example());
        let policy = fixed_task_policy(
            CodexHostExecutionMode::PlanOnly,
            STATIC_PAGE_IMAGE2_DATA_PUBLISH,
        );
        let decision = policy.prepare(&context).expect("decision");
        let workspace =
            std::env::temp_dir().join(format!("v3-codex-host-bundle-test-{}", Uuid::new_v4()));

        materialize_fixed_task_bundle(
            &workspace,
            &context,
            &decision,
            &CodexHostWorkspaceRetentionPolicy::new(336),
        )
        .expect("bundle should materialize");

        let task = fs::read_to_string(workspace.join("task.json")).expect("task.json");
        let readme = fs::read_to_string(workspace.join("README.md")).expect("README.md");
        let schema =
            fs::read_to_string(workspace.join("schemas/output.schema.json")).expect("schema");
        let evidence =
            fs::read_to_string(workspace.join("evidence/summary.json")).expect("evidence");
        let runtime = fs::read_to_string(workspace.join("runtime.json")).expect("runtime");

        assert!(task.contains("\"template_id\": \"static_page_image2_data_publish\""));
        assert!(readme.contains("Read `task.json`"));
        assert!(schema.contains("\"template_id\""));
        assert!(evidence.contains("\"dataset_scope\""));
        assert!(runtime.contains("\"raw_prompt_exposed\": false"));
        assert!(runtime.contains("\"retention_hours\": 336"));
        assert!(runtime.contains("\"cleanup_requires_operator\": true"));
        assert!(runtime.contains("\"backup_before_delete\": true"));
        assert!(runtime.contains("Safe-RemoveToBackup.ps1"));
        assert!(!runtime.contains("api_key"));
        assert!(!readme.contains("DATABASE_URL"));
    }

    #[test]
    fn fixed_task_bundle_materializes_static_page_image2_preview_file() {
        let _lock = test_env_lock().lock().expect("env lock");
        let root = std::env::temp_dir().join(format!("v3-codex-host-artifacts-{}", Uuid::new_v4()));
        let source = root.join("static-page-previews/job-1/preview.png");
        fs::create_dir_all(source.parent().expect("source parent")).expect("source dir");
        fs::write(&source, b"fake image bytes").expect("source image");
        let _root = TestEnvVarRestore::set(
            "CODEX_HOST_AGENT_GENERATED_ARTIFACTS_ROOT",
            root.to_str().expect("utf-8 temp root"),
        );

        let mut fixed_task =
            CodexHostFixedTaskTemplateContextView::static_page_image2_data_publish_example();
        fixed_task.image2["preview_asset_key"] = json!(
            "https://v3.elepcloud.com/generated-artifacts/static-page-previews/job-1/preview.png"
        );
        fixed_task.image2["render_asset_url"] = fixed_task.image2["preview_asset_key"].clone();
        let mut context = test_context(
            STATIC_PAGE_IMAGE2_DATA_PUBLISH,
            Some("Run the fixed static-page template package."),
        );
        context.fixed_task = Some(fixed_task);
        let policy = fixed_task_policy(
            CodexHostExecutionMode::PlanOnly,
            STATIC_PAGE_IMAGE2_DATA_PUBLISH,
        );
        let decision = policy.prepare(&context).expect("decision");
        let workspace =
            std::env::temp_dir().join(format!("v3-codex-host-bundle-test-{}", Uuid::new_v4()));

        materialize_fixed_task_bundle(
            &workspace,
            &context,
            &decision,
            &CodexHostWorkspaceRetentionPolicy::new(336),
        )
        .expect("bundle should materialize");

        let local_preview = workspace.join("image2/preview.png");
        let task = fs::read_to_string(workspace.join("task.json")).expect("task.json");
        let manifest =
            fs::read_to_string(workspace.join("image2/manifest.json")).expect("manifest");

        assert_eq!(
            fs::read(&local_preview).expect("local preview"),
            b"fake image bytes"
        );
        assert!(task.contains("\"local_preview_path\": \"image2/preview.png\""));
        assert!(task.contains("\"visual_contract_materialized\": true"));
        assert!(manifest.contains("\"materialized\": true"));
    }

    #[test]
    fn fixed_task_bundle_materializes_existing_static_page_artifact() {
        let _lock = test_env_lock().lock().expect("env lock");
        let root = std::env::temp_dir().join(format!("v3-codex-host-artifacts-{}", Uuid::new_v4()));
        let source_dir = root.join("database-static-pages/xinbai/report");
        fs::create_dir_all(source_dir.join("assets")).expect("source dir");
        fs::write(source_dir.join("index.html"), b"<html>old report</html>").expect("index");
        fs::write(source_dir.join("data.json"), br#"{"kpi":1}"#).expect("data");
        fs::write(source_dir.join("data-snapshot.json"), br#"{"snapshot":1}"#).expect("snapshot");
        fs::write(source_dir.join("assets/chart.css"), b".chart{}").expect("asset");
        let _root = TestEnvVarRestore::set(
            "CODEX_HOST_AGENT_GENERATED_ARTIFACTS_ROOT",
            root.to_str().expect("utf-8 temp root"),
        );

        let mut fixed_task =
            CodexHostFixedTaskTemplateContextView::static_page_image2_data_publish_example();
        fixed_task.requirements["existing_artifact"] = json!({
            "kind": "v3_generated_static_page",
            "public_url": "https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai/report/index.html",
            "revision_requested": true,
            "publish_mode": "new_generated_artifact_only"
        });
        let mut context = test_context(
            STATIC_PAGE_IMAGE2_DATA_PUBLISH,
            Some("Run the fixed static-page template package."),
        );
        context.fixed_task = Some(fixed_task);
        let policy = fixed_task_policy(
            CodexHostExecutionMode::PlanOnly,
            STATIC_PAGE_IMAGE2_DATA_PUBLISH,
        );
        let decision = policy.prepare(&context).expect("decision");
        let workspace =
            std::env::temp_dir().join(format!("v3-codex-host-bundle-test-{}", Uuid::new_v4()));

        materialize_fixed_task_bundle(
            &workspace,
            &context,
            &decision,
            &CodexHostWorkspaceRetentionPolicy::new(336),
        )
        .expect("bundle should materialize");

        assert_eq!(
            fs::read(workspace.join("existing-artifact/index.html")).expect("local index"),
            b"<html>old report</html>"
        );
        assert_eq!(
            fs::read(workspace.join("existing-artifact/data.json")).expect("local data"),
            br#"{"kpi":1}"#
        );
        assert_eq!(
            fs::read(workspace.join("existing-artifact/assets/chart.css")).expect("local asset"),
            b".chart{}"
        );
        let task: Value = serde_json::from_str(
            &fs::read_to_string(workspace.join("task.json")).expect("task.json"),
        )
        .expect("task json");
        assert_eq!(
            task["requirements"]["existing_artifact"]["local_index_path"],
            json!("existing-artifact/index.html")
        );
        assert_eq!(
            task["requirements"]["existing_artifact"]["local_data_path"],
            json!("existing-artifact/data.json")
        );
        assert_eq!(
            task["requirements"]["existing_artifact"]["local_data_snapshot_path"],
            json!("existing-artifact/data-snapshot.json")
        );
        assert_eq!(
            task["requirements"]["existing_artifact"]["materialized"],
            json!(true)
        );
    }

    #[test]
    fn customer_workspace_seed_materializes_generated_static_page_edit_artifact() {
        let _lock = test_env_lock().lock().expect("env lock");
        let root = std::env::temp_dir().join(format!("v3-codex-host-artifacts-{}", Uuid::new_v4()));
        let source_dir = root.join("database-static-pages/xinbai/current");
        fs::create_dir_all(source_dir.join("assets")).expect("source dir");
        fs::write(
            source_dir.join("index.html"),
            b"<html>current report</html>",
        )
        .expect("index");
        fs::write(source_dir.join("data.json"), br#"{"kpi":2}"#).expect("data");
        fs::write(source_dir.join("data-snapshot.json"), br#"{"snapshot":2}"#).expect("snapshot");
        fs::write(source_dir.join("assets/chart.css"), b".chart{color:red}").expect("asset");
        let _root = TestEnvVarRestore::set(
            "CODEX_HOST_AGENT_GENERATED_ARTIFACTS_ROOT",
            root.to_str().expect("utf-8 temp root"),
        );
        let public_url = "https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai/current/index.html";
        let mut context = test_context(
            GENERATED_STATIC_PAGE_EDIT,
            Some("Revise the current static page artifact."),
        );
        context.workspace_seed = Some(json!({
            "schema": "v3.codex_host_workspace_seed",
            "version": 1,
            "kind": "generated_static_page_edit",
            "existing_artifact": {
                "public_url": public_url,
                "revision_requested": true
            },
            "current_artifact": {
                "type": "static_page_draft",
                "publicUrl": public_url
            }
        }));
        let policy =
            fixed_task_policy(CodexHostExecutionMode::PlanOnly, GENERATED_STATIC_PAGE_EDIT);
        let decision = policy.prepare(&context).expect("decision");
        let workspace =
            std::env::temp_dir().join(format!("v3-codex-host-seed-test-{}", Uuid::new_v4()));

        materialize_workspace_seed(
            &workspace,
            &context,
            &decision,
            &CodexHostWorkspaceRetentionPolicy::new(168),
        )
        .expect("seed should materialize");

        assert_eq!(
            fs::read(workspace.join("existing-artifact/index.html")).expect("local index"),
            b"<html>current report</html>"
        );
        assert_eq!(
            fs::read(workspace.join("existing-artifact/data.json")).expect("local data"),
            br#"{"kpi":2}"#
        );
        assert_eq!(
            fs::read(workspace.join("existing-artifact/assets/chart.css")).expect("local asset"),
            b".chart{color:red}"
        );
        let seed: Value = serde_json::from_str(
            &fs::read_to_string(workspace.join("workspace-seed.json")).expect("seed json"),
        )
        .expect("seed parses");
        assert_eq!(seed["kind"], json!("generated_static_page_edit"));
        let task: Value = serde_json::from_str(
            &fs::read_to_string(workspace.join("task.json")).expect("task json"),
        )
        .expect("task parses");
        assert_eq!(
            task["output_manifest"]["preferred_path"],
            json!("customer-artifact-manifest.json")
        );
        let existing_seed: Value = serde_json::from_str(
            &fs::read_to_string(workspace.join("existing-artifact-seed.json"))
                .expect("existing artifact seed json"),
        )
        .expect("existing artifact seed parses");
        assert_eq!(
            existing_seed["local_index_path"],
            json!("existing-artifact/index.html")
        );
        let readme = fs::read_to_string(workspace.join("README.md")).expect("readme");
        assert!(readme.contains("existing-artifact/"));
        assert!(readme.contains("customer-artifact-manifest.json"));
        assert!(!readme.contains("DATABASE_URL"));
    }

    #[test]
    fn fixed_task_prompt_points_to_workspace_bundle() {
        let mut context = test_context(
            ANSWER_QUALITY_AUTOFIX,
            Some("Run the fixed answer-quality autofix template package."),
        );
        context.fixed_task =
            Some(CodexHostFixedTaskTemplateContextView::answer_quality_autofix_example());
        let policy = fixed_task_policy(CodexHostExecutionMode::PlanOnly, ANSWER_QUALITY_AUTOFIX);

        let decision = policy.prepare(&context).expect("decision");
        let plan = decision.command_plan.expect("command plan");

        assert!(plan.prompt.contains("Read `task.json`"));
        assert!(plan.prompt.contains("schemas/output.schema.json"));
        assert!(plan.prompt.contains("Return exactly one final JSON object"));
        assert!(!plan.prompt.contains("\"allowed_write_scope\""));
    }

    fn fixed_task_policy(mode: CodexHostExecutionMode, capability: &str) -> CodexHostAgentPolicy {
        CodexHostAgentPolicy {
            mode,
            profile: CodexHostProfile {
                id: "cloudflare-fixed".to_string(),
                kind: "codex-native".to_string(),
                model: Some("gpt-5.3-codex".to_string()),
                provider_id: None,
                base_url: None,
                env_key: None,
                wire_api: None,
                allowed_capabilities: vec![capability.to_string()],
            },
            host_kind: "cloudflare_codex".to_string(),
            allow_real_codex_exec: true,
            task_workspace_root: Some(PathBuf::from("D:/codex-host/tasks")),
        }
    }

    fn plan_only_policy_for_capability(capability: &str) -> CodexHostAgentPolicy {
        CodexHostAgentPolicy {
            mode: CodexHostExecutionMode::PlanOnly,
            profile: CodexHostProfile {
                id: "rightcode-plan-only".to_string(),
                kind: "codex-compatible-shim".to_string(),
                model: Some("gpt-5.5".to_string()),
                provider_id: Some("rightcode".to_string()),
                base_url: Some("https://right.codes/codex/v1".to_string()),
                env_key: Some("RIGHTCODE_API_KEY_MAIN".to_string()),
                wire_api: Some("responses".to_string()),
                allowed_capabilities: vec![capability.to_string()],
            },
            host_kind: "aiv3_server".to_string(),
            allow_real_codex_exec: false,
            task_workspace_root: Some(PathBuf::from("D:/codex-host/tasks")),
        }
    }

    fn compatible_shim_test_profile(capability: &str) -> CodexHostProfile {
        CodexHostProfile {
            id: "rightcode-gpt-5-5".to_string(),
            kind: "codex-compatible-shim".to_string(),
            model: Some("gpt-5.5".to_string()),
            provider_id: Some("rightcode".to_string()),
            base_url: Some("https://right.codes/codex/v1".to_string()),
            env_key: Some("RIGHTCODE_API_KEY_MAIN".to_string()),
            wire_api: Some("responses".to_string()),
            allowed_capabilities: vec![capability.to_string()],
        }
    }

    fn test_context(capability: &str, task: Option<&str>) -> CodexHostTaskContext {
        CodexHostTaskContext {
            assistant_run_id: AssistantRunId::new(),
            capability: capability.to_string(),
            task: task.map(ToOwned::to_owned),
            local_thread_id: Some("thread-a".to_string()),
            task_memory_isolated: true,
            task_memory_space_id: Some("codex-host-task:test".to_string()),
            fixed_task: None,
            workspace_seed: None,
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
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, previous }
        }

        fn unset(key: &'static str) -> Self {
            let previous = std::env::var(key).ok();
            std::env::remove_var(key);
            Self { key, previous }
        }
    }

    impl Drop for TestEnvVarRestore {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }
}
