use anyhow::{anyhow, Result};
use domain_model::StaticPageImageJobId;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

pub const DEFAULT_ORCHESTRATOR_BASE_URL: &str = "http://127.0.0.1:3003";
pub const DEFAULT_ORCHESTRATOR_RUNTIME_TARGET: &str = "cloudflare";
pub const STATIC_PAGE_ORCHESTRATOR_KIND: &str = "static-page-visual";
pub const STATIC_PAGE_ORCHESTRATOR_SOURCE: &str = "ai-data-platform-static-pages";
pub const STATIC_PAGE_ORCHESTRATOR_USER_AGENT: &str = "AIDataPlatformV3StaticPageWorker/1.0";
pub const DEFAULT_STATIC_PAGE_IMAGE_MODEL: &str = "gpt-image-2";
pub const DEFAULT_STATIC_PAGE_IMAGE_SIZE: &str = "1536x1024";
pub const DEFAULT_STATIC_PAGE_IMAGE_QUALITY: &str = "high";
pub const DEFAULT_STATIC_PAGE_VISUAL_CONTEXT_LIMIT_CHARS: usize = 5_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexOrchestratorConfig {
    pub base_url: String,
    pub access_key: String,
    pub runtime_target_id: String,
    pub project_id: Option<String>,
    pub image_model: String,
    pub image_size: String,
    pub image_quality: String,
    pub include_artifact_data: bool,
}

impl CodexOrchestratorConfig {
    pub fn from_env() -> Result<Self> {
        let access_key = orchestrator_access_key_from_env()?;

        Ok(Self {
            base_url: std::env::var("CODEX_ORCHESTRATOR_BASE_URL")
                .unwrap_or_else(|_| DEFAULT_ORCHESTRATOR_BASE_URL.to_string()),
            access_key,
            runtime_target_id: optional_env("CODEX_ORCHESTRATOR_RUNTIME_TARGET")
                .or_else(|| optional_env("CODEX_ORCHESTRATOR_RUNTIME_TARGET_ID"))
                .unwrap_or_else(|| DEFAULT_ORCHESTRATOR_RUNTIME_TARGET.to_string()),
            project_id: optional_env("CODEX_ORCHESTRATOR_PROJECT_ID"),
            image_model: std::env::var("CODEX_ORCHESTRATOR_IMAGE_MODEL")
                .unwrap_or_else(|_| DEFAULT_STATIC_PAGE_IMAGE_MODEL.to_string()),
            image_size: std::env::var("CODEX_ORCHESTRATOR_IMAGE_SIZE")
                .unwrap_or_else(|_| DEFAULT_STATIC_PAGE_IMAGE_SIZE.to_string()),
            image_quality: std::env::var("CODEX_ORCHESTRATOR_IMAGE_QUALITY")
                .unwrap_or_else(|_| DEFAULT_STATIC_PAGE_IMAGE_QUALITY.to_string()),
            include_artifact_data: bool_env("CODEX_ORCHESTRATOR_INCLUDE_ARTIFACT_DATA"),
        })
    }

    pub fn endpoint(&self, path: &str) -> String {
        format!(
            "{}/api/codex/orchestrator/v1{}",
            self.base_url.trim_end_matches('/'),
            path
        )
    }
}

fn orchestrator_access_key_from_env() -> Result<String> {
    if let Some(value) = optional_env("CODEX_ORCHESTRATOR_ACCESS_KEY") {
        return Ok(value);
    }
    read_orchestrator_key_file()?.ok_or_else(|| {
        anyhow!("CODEX_ORCHESTRATOR_ACCESS_KEY or CODEX_ORCHESTRATOR_KEY_FILE is required")
    })
}

fn read_orchestrator_key_file() -> Result<Option<String>> {
    let Some(path) = optional_env("CODEX_ORCHESTRATOR_KEY_FILE") else {
        return Ok(None);
    };
    let raw = std::fs::read_to_string(&path)
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

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StaticPageVisualTaskRequest {
    pub runtime_target_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    pub kind: String,
    pub source: String,
    pub prompt: String,
    pub metadata: Value,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct OrchestratorTaskEnvelope {
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub task: Option<OrchestratorTaskView>,
    #[serde(default)]
    pub error: Option<OrchestratorErrorView>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OrchestratorTaskView {
    pub id: String,
    pub status: String,
    #[serde(default)]
    pub queue_position: Option<i32>,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error: Option<OrchestratorErrorView>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct OrchestratorErrorView {
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub retryable: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaticPageVisualArtifact {
    pub asset_key: String,
    pub name: Option<String>,
    pub mime_type: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
}

pub fn build_static_page_visual_task_request(
    config: &CodexOrchestratorConfig,
    job_id: StaticPageImageJobId,
    image_prompt_payload: &Value,
) -> StaticPageVisualTaskRequest {
    StaticPageVisualTaskRequest {
        runtime_target_id: config.runtime_target_id.clone(),
        project_id: config.project_id.clone(),
        kind: STATIC_PAGE_ORCHESTRATOR_KIND.to_string(),
        source: STATIC_PAGE_ORCHESTRATOR_SOURCE.to_string(),
        prompt: build_static_page_visual_prompt(image_prompt_payload),
        metadata: json!({
            "requestId": format!("static-page-image-{job_id}"),
            "output": "image-artifact",
            "model": config.image_model,
            "size": config.image_size,
            "quality": config.image_quality,
            "staticPageImageJobId": job_id,
        }),
    }
}

pub fn build_static_page_visual_prompt(image_prompt_payload: &Value) -> String {
    let payload = bounded_static_page_visual_context(image_prompt_payload);
    if let Some(prompt_text) = static_page_visual_prompt_text(image_prompt_payload) {
        return format!(
            "请用 GPT Image 2 生成一张 1536x1024 的中文企业静态页视觉草稿图。\n\
             要求：画面像客户汇报页或经营分析页，不要浏览器边框，不要后台管理系统，不要出现可编辑控件。\n\
             重点：以用户确认的生图文案为准；这一步只做视觉效果图，不要把结构化 JSON 当成固定模块规划。后续系统会读图并接入真实数据制作 HTML。完成后必须返回一张图片 artifact。\n\
             用户确认的生图文案：\n{prompt_text}\n\n\
             结构化上下文 JSON：\n```json\n{payload}\n```"
        );
    }
    format!(
        "请用 GPT Image 2 生成一张 1536x1024 的中文企业静态页视觉草稿图。\n\
         要求：画面像客户汇报页或经营分析页，不要浏览器边框，不要后台管理系统，不要出现可编辑控件。\n\
         重点：保留模块层级、中文标题、图表类型、数据关系和商务汇报质感。完成后必须返回一张图片 artifact。\n\
         静态页规划 JSON：\n```json\n{payload}\n```"
    )
}

fn bounded_static_page_visual_context(image_prompt_payload: &Value) -> String {
    let payload = serde_json::to_string_pretty(image_prompt_payload)
        .unwrap_or_else(|_| image_prompt_payload.to_string());
    let limit = env_usize(
        "STATIC_PAGE_VISUAL_CONTEXT_LIMIT_CHARS",
        DEFAULT_STATIC_PAGE_VISUAL_CONTEXT_LIMIT_CHARS,
    )
    .clamp(1_000, 20_000);
    if payload.chars().count() <= limit {
        return payload;
    }
    let mut bounded = payload.chars().take(limit).collect::<String>();
    bounded.push_str("\n...TRUNCATED_FOR_IMAGE2_VISUAL_CONTRACT...");
    bounded
}

fn static_page_visual_prompt_text(image_prompt_payload: &Value) -> Option<&str> {
    image_prompt_payload
        .get("promptText")
        .or_else(|| image_prompt_payload.get("prompt_text"))
        .or_else(|| image_prompt_payload.get("prompt"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub fn submit_static_page_visual_task(
    client: &Client,
    config: &CodexOrchestratorConfig,
    job_id: StaticPageImageJobId,
    image_prompt_payload: &Value,
) -> Result<OrchestratorTaskView> {
    let body = build_static_page_visual_task_request(config, job_id, image_prompt_payload);
    let response = client
        .post(config.endpoint("/tasks"))
        .bearer_auth(&config.access_key)
        .header("Idempotency-Key", format!("static-page-image-{job_id}"))
        .header("X-Client-Name", STATIC_PAGE_ORCHESTRATOR_SOURCE)
        .header("User-Agent", STATIC_PAGE_ORCHESTRATOR_USER_AGENT)
        .json(&body)
        .send()?;
    parse_orchestrator_response(response)
}

pub fn poll_static_page_visual_task(
    client: &Client,
    config: &CodexOrchestratorConfig,
    task_id: &str,
) -> Result<OrchestratorTaskView> {
    let query = if config.include_artifact_data {
        "?includeArtifactData=1"
    } else {
        ""
    };
    let response = client
        .get(config.endpoint(&format!("/tasks/{task_id}{query}")))
        .bearer_auth(&config.access_key)
        .header("X-Client-Name", STATIC_PAGE_ORCHESTRATOR_SOURCE)
        .header("User-Agent", STATIC_PAGE_ORCHESTRATOR_USER_AGENT)
        .send()?;
    parse_orchestrator_response(response)
}

pub fn parse_orchestrator_response(
    response: reqwest::blocking::Response,
) -> Result<OrchestratorTaskView> {
    let status = response.status();
    let body = response.text()?;
    let envelope = serde_json::from_str::<OrchestratorTaskEnvelope>(&body).map_err(|error| {
        anyhow!(
            "orchestrator response JSON decode failed: status={} body_chars={} body_excerpt=\"{}\": {}",
            status,
            body.chars().count(),
            orchestrator_response_excerpt(&body, 300),
            error
        )
    })?;
    if !status.is_success() || !envelope.ok {
        return Err(anyhow!(orchestrator_error_message(&envelope)
            .unwrap_or_else(|| format!(
                "orchestrator request failed with HTTP {status}"
            ))));
    }
    envelope
        .task
        .ok_or_else(|| anyhow!("orchestrator response did not include task"))
}

fn orchestrator_response_excerpt(body: &str, max_chars: usize) -> String {
    body.trim()
        .chars()
        .take(max_chars)
        .collect::<String>()
        .escape_debug()
        .to_string()
}

pub fn extract_first_image_artifact(
    task: &OrchestratorTaskView,
) -> Result<StaticPageVisualArtifact> {
    let result = task
        .result
        .as_ref()
        .ok_or_else(|| anyhow!("orchestrator task completed without result"))?;
    if result.get("artifactStatus").and_then(Value::as_str) == Some("missing_required") {
        return Err(anyhow!(
            "orchestrator completed without the required image artifact"
        ));
    }

    let artifacts = result
        .get("artifacts")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("orchestrator result has no artifacts array"))?;
    let artifact = artifacts
        .iter()
        .find(|item| {
            item.get("type").and_then(Value::as_str) == Some("image")
                || item
                    .get("mimeType")
                    .and_then(Value::as_str)
                    .map(|value| value.starts_with("image/"))
                    .unwrap_or(false)
        })
        .ok_or_else(|| anyhow!("orchestrator result has no image artifact"))?;

    let mime_type = artifact
        .get("mimeType")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let asset_key = artifact
        .get("url")
        .and_then(Value::as_str)
        .or_else(|| artifact.get("downloadUrl").and_then(Value::as_str))
        .or_else(|| artifact.get("dataUrl").and_then(Value::as_str))
        .map(ToOwned::to_owned)
        .or_else(|| {
            artifact
                .get("base64")
                .and_then(Value::as_str)
                .map(|base64| {
                    format!(
                        "data:{};base64,{base64}",
                        mime_type.as_deref().unwrap_or("image/png")
                    )
                })
        })
        .ok_or_else(|| {
            anyhow!("image artifact does not include url, downloadUrl, dataUrl, or base64")
        })?;

    Ok(StaticPageVisualArtifact {
        asset_key,
        name: artifact
            .get("name")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        mime_type,
        width: artifact.get("width").and_then(Value::as_i64),
        height: artifact.get("height").and_then(Value::as_i64),
    })
}

pub fn normalize_artifact_asset_key(asset_key: &str, orchestrator_base_url: &str) -> String {
    let trimmed = asset_key.trim();
    if trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("data:image/")
        || trimmed.starts_with("blob:")
    {
        return trimmed.to_string();
    }

    let base = orchestrator_base_url.trim_end_matches('/');
    if trimmed.starts_with('/') {
        return format!("{base}{trimmed}");
    }

    format!("{base}/{trimmed}")
}

pub fn merge_orchestrator_state(image_prompt_payload: &Value, state: Value) -> Value {
    let mut object = image_prompt_payload
        .as_object()
        .cloned()
        .unwrap_or_default();
    object.insert("orchestrator".to_string(), state);
    Value::Object(object)
}

pub fn context_uuid(value: &Value, key: &str) -> Result<Uuid> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("workflow execution context missing {key}"))
        .and_then(|raw| Uuid::parse_str(raw).map_err(Into::into))
}

pub fn task_failure_message(task: &OrchestratorTaskView) -> String {
    task.error
        .as_ref()
        .and_then(|error| error.message.clone().or_else(|| error.code.clone()))
        .unwrap_or_else(|| format!("orchestrator task ended with status {}", task.status))
}

fn orchestrator_error_message(envelope: &OrchestratorTaskEnvelope) -> Option<String> {
    envelope
        .error
        .as_ref()
        .and_then(|error| error.message.clone().or_else(|| error.code.clone()))
        .or_else(|| envelope.message.clone())
        .or_else(|| envelope.status.clone())
}

fn optional_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn bool_env(key: &str) -> bool {
    std::env::var(key)
        .ok()
        .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct EnvRestore {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvRestore {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, previous }
        }

        fn remove(key: &'static str) -> Self {
            let previous = std::env::var(key).ok();
            std::env::remove_var(key);
            Self { key, previous }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            if let Some(value) = self.previous.as_ref() {
                std::env::set_var(self.key, value);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }

    fn config() -> CodexOrchestratorConfig {
        CodexOrchestratorConfig {
            base_url: "http://127.0.0.1:3003/".to_string(),
            access_key: "test-access-key".to_string(),
            runtime_target_id: "cloudflare".to_string(),
            project_id: None,
            image_model: "gpt-image-2".to_string(),
            image_size: "1536x1024".to_string(),
            image_quality: "high".to_string(),
            include_artifact_data: false,
        }
    }

    #[test]
    fn from_env_reads_orchestrator_key_file_when_access_key_is_not_set() {
        let _lock = ENV_LOCK.lock().expect("env lock");
        let _access = EnvRestore::remove("CODEX_ORCHESTRATOR_ACCESS_KEY");
        let path =
            std::env::temp_dir().join(format!("static-page-worker-key-{}.txt", Uuid::new_v4()));
        std::fs::write(
            &path,
            json!({"keys": {"rawKey": "file-secret"}}).to_string(),
        )
        .expect("write key file");
        let _key_file = EnvRestore::set(
            "CODEX_ORCHESTRATOR_KEY_FILE",
            path.to_str().expect("utf-8 temp path"),
        );

        let config = CodexOrchestratorConfig::from_env().expect("config from key file");

        assert_eq!(config.access_key, "file-secret");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn reads_plain_orchestrator_key_file_text() {
        assert_eq!(
            orchestrator_access_key_from_file_text("# comment\n plain-secret \n"),
            Some("plain-secret".to_string())
        );
    }

    #[test]
    fn builds_static_page_visual_task_request_for_cloudflare_codex() {
        let job_id =
            StaticPageImageJobId(Uuid::parse_str("00000000-0000-0000-0000-000000000123").unwrap());
        let request = build_static_page_visual_task_request(
            &config(),
            job_id,
            &json!({
                "title": "经营分析静态页",
                "modules": [{ "title": "核心判断", "visualization": "kpi" }]
            }),
        );

        assert_eq!(request.runtime_target_id, "cloudflare");
        assert_eq!(request.project_id, None);
        assert_eq!(request.kind, "static-page-visual");
        assert_eq!(request.source, "ai-data-platform-static-pages");
        assert_eq!(request.metadata["output"], json!("image-artifact"));
        assert_eq!(request.metadata["model"], json!("gpt-image-2"));
        assert!(request.prompt.contains("经营分析静态页"));
        assert!(request.prompt.contains("必须返回一张图片 artifact"));
    }

    #[test]
    fn omits_project_id_from_default_static_page_visual_request_body() {
        let job_id =
            StaticPageImageJobId(Uuid::parse_str("00000000-0000-0000-0000-000000000123").unwrap());
        let request = build_static_page_visual_task_request(
            &config(),
            job_id,
            &json!({ "title": "经营分析静态页" }),
        );

        let body = serde_json::to_value(request).expect("serializes request");
        assert_eq!(body.get("projectId"), None);
        assert_eq!(body["runtimeTargetId"], json!("cloudflare"));
    }

    #[test]
    fn keeps_project_id_when_explicitly_configured() {
        let mut config = config();
        config.project_id = Some("explicit-project".to_string());
        let job_id =
            StaticPageImageJobId(Uuid::parse_str("00000000-0000-0000-0000-000000000123").unwrap());
        let request = build_static_page_visual_task_request(
            &config,
            job_id,
            &json!({ "title": "经营分析静态页" }),
        );

        let body = serde_json::to_value(request).expect("serializes request");
        assert_eq!(body["projectId"], json!("explicit-project"));
    }

    #[test]
    fn visual_prompt_prefers_confirmed_prompt_text() {
        let prompt = build_static_page_visual_prompt(&json!({
            "promptText": "用户确认：生成智能家居项目经营分析图，不要编辑框。",
            "title": "旧标题"
        }));

        assert!(prompt.contains("用户确认的生图文案"));
        assert!(prompt.contains("用户确认：生成智能家居项目经营分析图，不要编辑框。"));
        assert!(prompt.contains("结构化上下文 JSON"));
    }

    #[test]
    fn visual_prompt_bounds_large_structured_context() {
        let prompt = build_static_page_visual_prompt(&json!({
            "promptText": "用户确认：生成V3演示页。",
            "modules": (0..300)
                .map(|index| json!({
                    "id": format!("module-{index}"),
                    "title": format!("模块 {index}"),
                    "content": "这是一段用于撑大视觉上下文的说明文字。".repeat(20),
                }))
                .collect::<Vec<_>>()
        }));

        assert!(prompt.contains("用户确认：生成V3演示页。"));
        assert!(prompt.contains("TRUNCATED_FOR_IMAGE2_VISUAL_CONTRACT"));
        assert!(prompt.chars().count() < 8_000);
    }

    #[test]
    fn extracts_first_downloadable_image_artifact() {
        let task = OrchestratorTaskView {
            id: "task_1".to_string(),
            status: "completed".to_string(),
            queue_position: None,
            result: Some(json!({
                "type": "image",
                "artifacts": [{
                    "type": "image",
                    "name": "static-page-draft.png",
                    "mimeType": "image/png",
                    "downloadUrl": "https://example.com/static-page-draft.png",
                    "width": 1536,
                    "height": 1024
                }]
            })),
            error: None,
        };

        let artifact = extract_first_image_artifact(&task).expect("artifact");
        assert_eq!(
            artifact.asset_key,
            "https://example.com/static-page-draft.png"
        );
        assert_eq!(artifact.mime_type.as_deref(), Some("image/png"));
        assert_eq!(artifact.width, Some(1536));
    }

    #[test]
    fn rejects_missing_required_image_artifact() {
        let task = OrchestratorTaskView {
            id: "task_1".to_string(),
            status: "completed".to_string(),
            queue_position: None,
            result: Some(json!({
                "artifactStatus": "missing_required",
                "artifacts": []
            })),
            error: None,
        };

        let error = extract_first_image_artifact(&task).expect_err("missing artifact");
        assert!(error.to_string().contains("required image artifact"));
    }

    #[test]
    fn normalizes_relative_artifact_urls_against_orchestrator_base() {
        assert_eq!(
            normalize_artifact_asset_key(
                "/api/codex/artifacts/artifact_1?download=1",
                "http://127.0.0.1:3003/"
            ),
            "http://127.0.0.1:3003/api/codex/artifacts/artifact_1?download=1"
        );
        assert_eq!(
            normalize_artifact_asset_key(
                "api/codex/artifacts/artifact_2?download=1",
                "http://127.0.0.1:3003"
            ),
            "http://127.0.0.1:3003/api/codex/artifacts/artifact_2?download=1"
        );
        assert_eq!(
            normalize_artifact_asset_key("data:image/png;base64,abc", "http://127.0.0.1:3003"),
            "data:image/png;base64,abc"
        );
    }

    #[test]
    fn endpoint_uses_canonical_orchestrator_path() {
        assert_eq!(
            config().endpoint("/tasks"),
            "http://127.0.0.1:3003/api/codex/orchestrator/v1/tasks"
        );
    }
}
