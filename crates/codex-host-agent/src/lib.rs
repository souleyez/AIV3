use anyhow::{anyhow, Result};
use contracts::{
    CodexHostCommandPlanSummaryView, CodexHostFixedTaskTemplateContextView,
    CodexHostFixedTaskTemplateIdView, CodexHostProcessOutputSummaryView, CodexHostTaskOutputView,
    CodexHostTaskProfileSummaryView, HtmlArtifactManifestView,
};
use domain_model::{AssistantRunId, WorkflowExecution, WorkflowKind};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use uuid::Uuid;

const DEFAULT_PROFILE_ID: &str = "default-dry-run";
const DEFAULT_PROFILE_KIND: &str = "dry-run";
const DEFAULT_HOST_KIND: &str = "developer_workstation";
const DEFAULT_COMPAT_PROVIDER_ID: &str = "minimax";
const DEFAULT_COMPAT_PROVIDER_ENV_KEY: &str = "MINIMAX_API_KEY";
const DEFAULT_COMPAT_PROVIDER_WIRE_API: &str = "responses";
const STATIC_PAGE_IMAGE2_DATA_PUBLISH: &str = "static_page_image2_data_publish";
const ANSWER_QUALITY_AUTOFIX: &str = "answer_quality_autofix";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexHostTaskContext {
    pub assistant_run_id: AssistantRunId,
    pub capability: String,
    pub task: Option<String>,
    pub local_thread_id: Option<String>,
    pub task_memory_isolated: bool,
    pub task_memory_space_id: Option<String>,
    pub fixed_task: Option<CodexHostFixedTaskTemplateContextView>,
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

        Ok(Self {
            assistant_run_id,
            capability,
            task,
            local_thread_id,
            task_memory_isolated,
            task_memory_space_id,
            fixed_task,
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
        _ => vec![json!({"title": "处理任务", "detail": "Codex Host 返回结构化任务结果。"})],
    }
}

fn codex_host_report_risks(mode: &str) -> Vec<Value> {
    match mode {
        "codex_exec" => vec![json!({
            "title": "执行输出需复核",
            "detail": "报告只展示脱敏摘要；代码变更和产物仍需由 V3 工作流或人工复核。"
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
}

impl CodexHostExecutionMode {
    pub fn from_raw(raw: &str) -> Result<Self> {
        match raw.trim() {
            "" | "dry_run" | "dry-run" => Ok(Self::DryRun),
            "plan_only" | "plan-only" => Ok(Self::PlanOnly),
            "codex_exec" | "codex-exec" => Ok(Self::CodexExec),
            other => Err(anyhow!(
                "unsupported CODEX_HOST_AGENT_EXECUTION_MODE={other}; expected dry_run, plan_only, or codex_exec"
            )),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DryRun => "dry_run",
            Self::PlanOnly => "plan_only",
            Self::CodexExec => "codex_exec",
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
        self.validate_fixed_task_policy(context)?;
        let command_plan = match self.mode {
            CodexHostExecutionMode::DryRun => None,
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

        Ok(CodexHostExecutionDecision {
            mode: self.mode.clone(),
            profile: self.profile.clone(),
            host_kind: self.host_kind.clone(),
            command_plan,
        })
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
                "fixed template {} is blocked on host kind {}; use windows_jump, mac_host, or cloudflare_codex",
                fixed_task.template_id.as_str(),
                self.host_kind
            ));
        }
        match &fixed_task.template_id {
            CodexHostFixedTaskTemplateIdView::StaticPageImage2DataPublish => {
                validate_static_page_fixed_task(fixed_task)?;
                if self.mode != CodexHostExecutionMode::DryRun && self.task_workspace_root.is_none()
                {
                    return Err(anyhow!(
                        "static_page_image2_data_publish requires CODEX_HOST_AGENT_TASK_WORKSPACE_ROOT for non-dry-run execution"
                    ));
                }
            }
            CodexHostFixedTaskTemplateIdView::AnswerQualityAutofix => {
                validate_answer_quality_fixed_task(fixed_task)?;
                if self.mode != CodexHostExecutionMode::DryRun && self.task_workspace_root.is_none()
                {
                    return Err(anyhow!(
                        "answer_quality_autofix requires CODEX_HOST_AGENT_TASK_WORKSPACE_ROOT for non-dry-run execution"
                    ));
                }
            }
        }
        Ok(())
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
                "codex_exec mode is blocked on host kind {other}; use windows_jump, mac_host, or cloudflare_codex"
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
        let workspace_configured = command_plan
            .and_then(|plan| plan.workspace_path.as_ref())
            .is_some();
        if !workspace_configured {
            return Err(anyhow!(
                "codex_exec mode requires CODEX_HOST_AGENT_TASK_WORKSPACE_ROOT for an isolated task workspace"
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
        _ => None,
    }
}

fn approved_remote_host_kind(host_kind: &str) -> bool {
    matches!(host_kind, "windows_jump" | "mac_host" | "cloudflare_codex")
}

fn validate_static_page_fixed_task(
    fixed_task: &CodexHostFixedTaskTemplateContextView,
) -> Result<()> {
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
    Ok(())
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
    let prompt = context
        .task
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| fixed_task_prompt(context.fixed_task.as_ref()).ok())
        .ok_or_else(|| anyhow!("Codex Host task text is required for non-dry-run planning"))?
        .to_string();
    let sandbox = if context.capability == "propose_patch" || context.fixed_task.is_some() {
        "workspace-write"
    } else {
        "read-only"
    }
    .to_string();
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

fn fixed_task_prompt(fixed_task: Option<&CodexHostFixedTaskTemplateContextView>) -> Result<String> {
    let fixed_task = fixed_task.ok_or_else(|| anyhow!("fixed task package missing"))?;
    let package = serde_json::to_string_pretty(fixed_task)
        .map_err(|error| anyhow!("failed to serialize fixed task package: {error}"))?;
    Ok(format!(
        "Run the V3 fixed Cloudflare Codex task template `{}`. Use only this server-owned package and return the configured output schema.\n\n{}",
        fixed_task.template_id.as_str(),
        package
    ))
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
                "task_memory_space_id": "codex-host-task:run-a"
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
    fn safe_log_excerpt_redacts_secret_lines_and_truncates() {
        let raw = b"ok\nAuthorization: Bearer abc\napi_key=xyz\nnormal line after secret\n";

        let excerpt = safe_log_excerpt(raw, 40);

        assert!(excerpt.contains("ok"));
        assert!(excerpt.contains("[redacted-log-line]"));
        assert!(!excerpt.contains("abc"));
        assert!(!excerpt.contains("xyz"));
        assert!(excerpt.chars().count() <= 40);
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

    fn test_context(capability: &str, task: Option<&str>) -> CodexHostTaskContext {
        CodexHostTaskContext {
            assistant_run_id: AssistantRunId::new(),
            capability: capability.to_string(),
            task: task.map(ToOwned::to_owned),
            local_thread_id: Some("thread-a".to_string()),
            task_memory_isolated: true,
            task_memory_space_id: Some("codex-host-task:test".to_string()),
            fixed_task: None,
        }
    }
}
