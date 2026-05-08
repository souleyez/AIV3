use anyhow::{anyhow, Result};
use contracts::{
    CodexHostCommandPlanSummaryView, CodexHostProcessOutputSummaryView, CodexHostTaskOutputView,
    CodexHostTaskProfileSummaryView,
};
use domain_model::{AssistantRunId, WorkflowExecution, WorkflowKind};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

const DEFAULT_PROFILE_ID: &str = "default-dry-run";
const DEFAULT_PROFILE_KIND: &str = "dry-run";
const DEFAULT_HOST_KIND: &str = "developer_workstation";
const DEFAULT_COMPAT_PROVIDER_ID: &str = "minimax";
const DEFAULT_COMPAT_PROVIDER_ENV_KEY: &str = "MINIMAX_API_KEY";
const DEFAULT_COMPAT_PROVIDER_WIRE_API: &str = "responses";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexHostTaskContext {
    pub assistant_run_id: AssistantRunId,
    pub capability: String,
    pub task: Option<String>,
    pub local_thread_id: Option<String>,
    pub task_memory_isolated: bool,
    pub task_memory_space_id: Option<String>,
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

        Ok(Self {
            assistant_run_id,
            capability,
            task,
            local_thread_id,
            task_memory_isolated,
            task_memory_space_id,
        })
    }

    pub fn dry_run_output(&self) -> Value {
        json!(CodexHostTaskOutputView {
            mode: "dry_run".to_string(),
            codex_invoked: false,
            status: "completed".to_string(),
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
        })
    }

    pub fn planned_output(&self, decision: &CodexHostExecutionDecision) -> Value {
        json!(CodexHostTaskOutputView {
            mode: decision.mode.as_str().to_string(),
            codex_invoked: false,
            status: "planned".to_string(),
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
        })
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
            stdout_excerpt: self.stdout_excerpt.clone(),
            stderr_excerpt: self.stderr_excerpt.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexHostAgentPolicy {
    pub mode: CodexHostExecutionMode,
    pub profile: CodexHostProfile,
    pub host_kind: String,
    pub allow_real_codex_exec: bool,
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
        let command_plan = match self.mode {
            CodexHostExecutionMode::DryRun => None,
            CodexHostExecutionMode::PlanOnly | CodexHostExecutionMode::CodexExec => {
                Some(build_codex_command_plan(context, &self.profile)?)
            }
        };
        if self.mode == CodexHostExecutionMode::CodexExec {
            self.validate_real_exec_guard()?;
        }

        Ok(CodexHostExecutionDecision {
            mode: self.mode.clone(),
            profile: self.profile.clone(),
            command_plan,
        })
    }

    fn validate_real_exec_guard(&self) -> Result<()> {
        if !self.allow_real_codex_exec {
            return Err(anyhow!(
                "CODEX_HOST_AGENT_ALLOW_REAL_CODEX_EXEC must be true before codex_exec mode can launch Codex"
            ));
        }
        match self.host_kind.as_str() {
            "windows_jump" | "mac_host" => {}
            other => {
                return Err(anyhow!(
                    "codex_exec mode is blocked on host kind {other}; use windows_jump or mac_host"
                ));
            }
        }
        match self.profile.kind.as_str() {
            "codex-native" | "codex-compatible-shim" => Ok(()),
            other => Err(anyhow!(
                "profile kind {other} cannot be used for real Codex execution"
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexHostExecutionDecision {
    pub mode: CodexHostExecutionMode,
    pub profile: CodexHostProfile,
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
) -> Result<CodexCommandPlan> {
    let prompt = context
        .task
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("Codex Host task text is required for non-dry-run planning"))?
        .to_string();
    let sandbox = if context.capability == "propose_patch" {
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
    Ok(CodexCommandPlan {
        program: "codex".to_string(),
        args_without_prompt,
        prompt,
        sandbox,
    })
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
        assert_eq!(context.dry_run_output()["codex_invoked"], json!(false));
        assert_eq!(
            context.dry_run_output()["task_memory_space_id"],
            json!("codex-host-task:run-a")
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
        };

        let decision = policy.prepare(&context).expect("decision");

        assert_eq!(decision.mode, CodexHostExecutionMode::DryRun);
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
        };

        let decision = policy.prepare(&context).expect("decision");
        let plan = decision.command_plan.expect("command plan");
        let summary = plan.safe_summary();

        assert_eq!(summary.program, "codex");
        assert_eq!(summary.sandbox, "read-only");
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
        };

        let error = policy.prepare(&context).expect_err("should reject");

        assert!(error.to_string().contains("not allowed"));
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
        };

        let error = policy
            .prepare(&context)
            .expect_err("should reject local host");

        assert!(error.to_string().contains("developer_workstation"));
    }

    #[test]
    fn codex_exec_guard_allows_jump_host_profile() {
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
        };

        let decision = policy.prepare(&context).expect("decision");

        assert_eq!(decision.mode, CodexHostExecutionMode::CodexExec);
        assert!(decision.command_plan.is_some());
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

    fn test_context(capability: &str, task: Option<&str>) -> CodexHostTaskContext {
        CodexHostTaskContext {
            assistant_run_id: AssistantRunId::new(),
            capability: capability.to_string(),
            task: task.map(ToOwned::to_owned),
            local_thread_id: Some("thread-a".to_string()),
            task_memory_isolated: true,
            task_memory_space_id: Some("codex-host-task:test".to_string()),
        }
    }
}
