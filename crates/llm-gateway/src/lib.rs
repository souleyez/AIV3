use anyhow::{anyhow, Context, Result};
use contracts::{
    ProviderShimCostHintsView, ProviderShimHealthView, ProviderShimObservabilitySnapshotView,
    ProviderShimProfileSnapshotView, ProviderShimRateLimitHintsView,
    ProviderShimRedactionPolicyView, ProviderShimUsageEventView, ProviderShimUsageSummaryView,
};
use prompt_registry::InMemoryPromptRegistry;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt::{self, Debug, Display};
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LlmRuntimeMode {
    Placeholder,
    Provider,
}

impl LlmRuntimeMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Placeholder => "placeholder",
            Self::Provider => "provider",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LlmFinishReason {
    Stop,
    ToolCalls,
    Length,
    ContentFilter,
    Error,
    Other(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmProviderFailureKind {
    RequestFailed,
    RequestTimeout,
    HttpStatus,
    ResponseBodyReadFailed,
    InvalidJson,
    InvalidResponse,
    FinishReasonError,
}

impl LlmProviderFailureKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RequestFailed => "request_failed",
            Self::RequestTimeout => "request_timeout",
            Self::HttpStatus => "http_status",
            Self::ResponseBodyReadFailed => "response_body_read_failed",
            Self::InvalidJson => "invalid_json",
            Self::InvalidResponse => "invalid_response",
            Self::FinishReasonError => "finish_reason_error",
        }
    }

    pub fn from_wire_value(value: &str) -> Option<Self> {
        match value {
            "request_failed" => Some(Self::RequestFailed),
            "request_timeout" => Some(Self::RequestTimeout),
            "http_status" => Some(Self::HttpStatus),
            "response_body_read_failed" => Some(Self::ResponseBodyReadFailed),
            "invalid_json" => Some(Self::InvalidJson),
            "invalid_response" => Some(Self::InvalidResponse),
            "finish_reason_error" => Some(Self::FinishReasonError),
            _ => None,
        }
    }

    pub fn provider_responded(&self) -> bool {
        !matches!(self, Self::RequestFailed | Self::RequestTimeout)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmProviderFailure {
    pub kind: LlmProviderFailureKind,
    pub message: String,
}

#[derive(Clone, Debug)]
pub struct LlmProviderError {
    runtime: LlmRuntimeMetadata,
    message: String,
}

impl LlmProviderError {
    pub fn new(runtime: LlmRuntimeMetadata, message: impl Into<String>) -> Self {
        Self {
            runtime,
            message: message.into(),
        }
    }

    pub fn runtime(&self) -> &LlmRuntimeMetadata {
        &self.runtime
    }
}

impl Display for LlmProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for LlmProviderError {}

impl LlmFinishReason {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Stop => "stop",
            Self::ToolCalls => "tool_calls",
            Self::Length => "length",
            Self::ContentFilter => "content_filter",
            Self::Error => "error",
            Self::Other(value) => value.as_str(),
        }
    }

    pub fn from_wire_value(value: &str) -> Self {
        match value {
            "stop" => Self::Stop,
            "tool_calls" => Self::ToolCalls,
            "length" => Self::Length,
            "content_filter" => Self::ContentFilter,
            "error" => Self::Error,
            _ => Self::Other(value.to_string()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct LlmRequest {
    pub model: String,
    pub lane: Option<String>,
    pub system_prompt_key: Option<String>,
    pub input: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmToolCallStatus {
    Requested,
    Completed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmToolCall {
    pub call_id: Option<String>,
    pub tool_name: String,
    pub status: LlmToolCallStatus,
    pub arguments: Option<Value>,
    pub result: Option<Value>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmTokenUsage {
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub total_tokens: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LlmRuntimeMetadata {
    pub mode: LlmRuntimeMode,
    pub provider: String,
    pub model: String,
    pub lane: Option<String>,
    pub request_id: Option<String>,
    pub finish_reason: Option<LlmFinishReason>,
    pub provider_failure: Option<LlmProviderFailure>,
    pub latency_ms: Option<u64>,
    pub usage: Option<LlmTokenUsage>,
    pub system_prompt_key: Option<String>,
    pub system_prompt_version: Option<String>,
    pub tool_trace_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LlmResponse {
    pub output_text: String,
    pub runtime: LlmRuntimeMetadata,
    pub tool_calls: Vec<LlmToolCall>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LlmRuntimeSelection {
    pub mode: String,
    pub provider: String,
    pub model: String,
    pub lane: String,
}

#[derive(Clone, Debug)]
pub struct OpenAiCompatibleLlmProviderConfig {
    pub api_base_url: String,
    pub api_path: String,
    pub api_key: Option<String>,
}

#[derive(Clone, Debug)]
pub struct OpenClawLlmProviderConfig {
    pub gateway_base_url: String,
    pub token: Option<String>,
    pub agent_id: Option<String>,
    pub model: Option<String>,
    pub model_override: Option<String>,
    pub prefer_responses: bool,
    pub timeout_ms: u64,
}

pub const MODEL_LANE_ASSISTANT_CHAT: &str = "assistant_chat";
pub const MODEL_LANE_ASSISTANT_REACT_JSON: &str = "assistant_react_json";
pub const MODEL_LANE_STATIC_PAGE_INTENT: &str = "static_page_intent";
pub const MODEL_LANE_STATIC_PAGE_IMAGE_PROMPT: &str = "static_page_image_prompt";
pub const MODEL_LANE_CHAT_SESSION: &str = "chat_session";
pub const MODEL_LANE_DATASET_OUTPUT: &str = "dataset_output";
pub const MODEL_LANE_DOCUMENT_VLM: &str = "document_vlm";
pub const MODEL_LANE_AUDIO_TRANSCRIPT: &str = "audio_transcript";
pub const MODEL_LANE_VIDEO_SCENE_SUMMARY: &str = "video_scene_summary";
pub const MODEL_LANE_REPORT_PLANNING: &str = "report_planning";
pub const MODEL_LANE_CODEX_TASK_SUMMARY: &str = "codex_task_summary";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelProfileWireApi {
    ChatCompletions,
    Responses,
    CodexCompatibleShim,
}

impl ModelProfileWireApi {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ChatCompletions => "chat_completions",
            Self::Responses => "responses",
            Self::CodexCompatibleShim => "codex_compatible_shim",
        }
    }

    pub fn from_env_value(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "chat" | "chat_completions" | "openai_chat_completions" => Some(Self::ChatCompletions),
            "responses" | "response" | "openai_responses" => Some(Self::Responses),
            "codex_compatible_shim"
            | "codex-compatible-shim"
            | "codex_shim"
            | "responses_shim"
            | "provider_shim" => Some(Self::CodexCompatibleShim),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ModelCapabilityManifest {
    pub chat: bool,
    pub reasoning: bool,
    pub vision: bool,
    pub audio: bool,
    pub video: bool,
    pub json_mode: bool,
    pub tool_calling: bool,
    pub image_prompt: bool,
    pub static_page: bool,
    pub codex_compatible: bool,
    pub extra: Vec<String>,
}

impl ModelCapabilityManifest {
    pub fn from_names(names: &[String]) -> Self {
        let mut manifest = Self::default();
        for raw_name in names {
            let name = raw_name.trim();
            if name.is_empty() {
                continue;
            }
            match name.to_ascii_lowercase().as_str() {
                "chat" | "conversation" => manifest.chat = true,
                "reasoning" | "think" | "thinking" => manifest.reasoning = true,
                "vision" | "document" | "vlm" => manifest.vision = true,
                "audio" | "transcript" => manifest.audio = true,
                "video" | "scene" => manifest.video = true,
                "json" | "json_mode" | "structured_output" => manifest.json_mode = true,
                "tool" | "tools" | "tool_calling" | "tool_control" => manifest.tool_calling = true,
                "image_prompt" | "visual_prompt" => manifest.image_prompt = true,
                "static_page" | "static_page_plan" | "static_page_edit" => {
                    manifest.static_page = true
                }
                "codex" | "codex_compatible" | "codex_executor" => manifest.codex_compatible = true,
                _ => {
                    if !manifest.extra.iter().any(|item| item == name) {
                        manifest.extra.push(name.to_string());
                    }
                }
            }
        }
        manifest
    }

    pub fn names(&self) -> Vec<String> {
        let mut names = Vec::new();
        if self.chat {
            names.push("chat".to_string());
        }
        if self.reasoning {
            names.push("reasoning".to_string());
        }
        if self.vision {
            names.push("vision".to_string());
        }
        if self.audio {
            names.push("audio".to_string());
        }
        if self.video {
            names.push("video".to_string());
        }
        if self.json_mode {
            names.push("json_mode".to_string());
        }
        if self.tool_calling {
            names.push("tool_calling".to_string());
        }
        if self.image_prompt {
            names.push("image_prompt".to_string());
        }
        if self.static_page {
            names.push("static_page".to_string());
        }
        if self.codex_compatible {
            names.push("codex_compatible".to_string());
        }
        names.extend(self.extra.iter().cloned());
        names
    }

    pub fn has(&self, capability: &str) -> bool {
        self.names()
            .iter()
            .any(|name| name.eq_ignore_ascii_case(capability))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ModelRateLimitHints {
    pub requests_per_minute: Option<u32>,
    pub tokens_per_minute: Option<u32>,
    pub concurrent_requests: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ModelCostHints {
    pub input_microusd_per_million_tokens: Option<u64>,
    pub output_microusd_per_million_tokens: Option<u64>,
    pub currency: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRedactionPolicy {
    pub redact_provider_errors: bool,
    pub redact_request_payloads: bool,
    pub redact_response_payloads: bool,
    pub max_error_chars: usize,
}

impl Default for ModelRedactionPolicy {
    fn default() -> Self {
        Self {
            redact_provider_errors: true,
            redact_request_payloads: true,
            redact_response_payloads: true,
            max_error_chars: PROVIDER_ERROR_MAX_CHARS,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelProviderProfile {
    pub profile_id: String,
    pub provider_id: String,
    pub model_id: String,
    pub base_url: Option<String>,
    pub api_path: Option<String>,
    pub wire_api: ModelProfileWireApi,
    pub auth_env_key_name: Option<String>,
    pub capabilities: ModelCapabilityManifest,
    pub timeout_ms: Option<u64>,
    pub rate_limit: ModelRateLimitHints,
    pub cost: ModelCostHints,
    pub redaction: ModelRedactionPolicy,
}

impl ModelProviderProfile {
    pub fn new(
        profile_id: impl Into<String>,
        provider_id: impl Into<String>,
        model_id: impl Into<String>,
    ) -> Self {
        Self {
            profile_id: profile_id.into(),
            provider_id: provider_id.into(),
            model_id: model_id.into(),
            base_url: None,
            api_path: None,
            wire_api: ModelProfileWireApi::ChatCompletions,
            auth_env_key_name: None,
            capabilities: ModelCapabilityManifest::default(),
            timeout_ms: None,
            rate_limit: ModelRateLimitHints::default(),
            cost: ModelCostHints::default(),
            redaction: ModelRedactionPolicy::default(),
        }
    }

    pub fn from_env(env_prefix: &str) -> Result<Self> {
        let provider_id = required_env_string(env_prefix, "PROVIDER_ID")?;
        let model_id = required_env_string(env_prefix, "MODEL_ID")?;
        let profile_id = optional_env_string(env_prefix, "PROFILE_ID")
            .unwrap_or_else(|| format!("{provider_id}:{model_id}"));
        let mut profile = Self::new(profile_id, provider_id, model_id);
        profile.base_url = optional_env_string(env_prefix, "BASE_URL");
        profile.api_path = optional_env_string(env_prefix, "API_PATH");
        profile.auth_env_key_name = optional_env_string(env_prefix, "AUTH_ENV_KEY");
        profile.timeout_ms = optional_env_u64(env_prefix, "TIMEOUT_MS")?;
        if let Some(wire_api) = optional_env_string(env_prefix, "WIRE_API") {
            profile.wire_api = ModelProfileWireApi::from_env_value(&wire_api).ok_or_else(|| {
                anyhow!(
                    "invalid {env_prefix}_WIRE_API value {wire_api}; expected chat_completions, responses, or codex_compatible_shim"
                )
            })?;
        }
        if let Some(capabilities) = optional_env_string(env_prefix, "CAPABILITIES") {
            let capability_names = split_csv_env(&capabilities);
            profile.capabilities = ModelCapabilityManifest::from_names(&capability_names);
        }
        profile.rate_limit = ModelRateLimitHints {
            requests_per_minute: optional_env_u32(env_prefix, "RATE_LIMIT_RPM")?,
            tokens_per_minute: optional_env_u32(env_prefix, "RATE_LIMIT_TPM")?,
            concurrent_requests: optional_env_u32(env_prefix, "RATE_LIMIT_CONCURRENCY")?,
        };
        profile.cost = ModelCostHints {
            input_microusd_per_million_tokens: optional_env_u64(
                env_prefix,
                "COST_INPUT_MICROUSD_PER_MILLION_TOKENS",
            )?,
            output_microusd_per_million_tokens: optional_env_u64(
                env_prefix,
                "COST_OUTPUT_MICROUSD_PER_MILLION_TOKENS",
            )?,
            currency: optional_env_string(env_prefix, "COST_CURRENCY"),
        };
        profile.redaction = ModelRedactionPolicy {
            redact_provider_errors: optional_env_bool(env_prefix, "REDACT_PROVIDER_ERRORS", true),
            redact_request_payloads: optional_env_bool(env_prefix, "REDACT_REQUEST_PAYLOADS", true),
            redact_response_payloads: optional_env_bool(
                env_prefix,
                "REDACT_RESPONSE_PAYLOADS",
                true,
            ),
            max_error_chars: optional_env_usize(env_prefix, "MAX_ERROR_CHARS")?
                .unwrap_or(PROVIDER_ERROR_MAX_CHARS),
        };
        Ok(profile)
    }

    pub fn to_model_route(&self, lane: &str, priority: i32) -> ModelRoute {
        ModelRoute {
            lane: lane.to_string(),
            provider: self.provider_id.clone(),
            model: self.model_id.clone(),
            capability_class: self.capabilities.names(),
            priority,
            fallback_route: None,
        }
    }

    pub fn runtime_selection_for_lane(&self, lane: &str) -> LlmRuntimeSelection {
        LlmRuntimeSelection {
            mode: "provider".to_string(),
            provider: self.provider_id.clone(),
            model: self.model_id.clone(),
            lane: lane.to_string(),
        }
    }

    pub fn public_manifest(&self) -> Value {
        let auth_env_key_name = self
            .auth_env_key_name
            .as_deref()
            .and_then(safe_env_key_name);
        let auth_configured = self
            .auth_env_key_name
            .as_deref()
            .and_then(|key| std::env::var(key).ok())
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false);

        json!({
            "profile_id": self.profile_id.as_str(),
            "provider_id": self.provider_id.as_str(),
            "model_id": self.model_id.as_str(),
            "wire_api": self.wire_api.as_str(),
            "base_url": self.base_url.as_deref().map(redact_provider_error),
            "api_path": self.api_path.as_deref(),
            "auth": {
                "env_key_name": auth_env_key_name,
                "configured": auth_configured,
            },
            "capabilities": self.capabilities.names(),
            "capability_flags": &self.capabilities,
            "timeout_ms": self.timeout_ms,
            "rate_limit": &self.rate_limit,
            "cost": &self.cost,
            "redaction": &self.redaction,
        })
    }

    pub fn provider_shim_profile_snapshot(&self) -> ProviderShimProfileSnapshotView {
        let auth_env_key_name = self
            .auth_env_key_name
            .as_deref()
            .and_then(safe_env_key_name);
        let auth_configured = self
            .auth_env_key_name
            .as_deref()
            .and_then(|key| std::env::var(key).ok())
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false);

        ProviderShimProfileSnapshotView {
            profile_id: self.profile_id.clone(),
            provider_id: self.provider_id.clone(),
            model_id: self.model_id.clone(),
            wire_api: self.wire_api.as_str().to_string(),
            endpoint_scope: provider_endpoint_scope(self.base_url.as_deref()),
            base_url_configured: self.base_url.is_some(),
            api_path: self.api_path.clone(),
            auth_env_key_name,
            auth_configured,
            timeout_ms: self.timeout_ms,
            capabilities: self.capabilities.names(),
            rate_limit: ProviderShimRateLimitHintsView {
                requests_per_minute: self.rate_limit.requests_per_minute,
                tokens_per_minute: self.rate_limit.tokens_per_minute,
                concurrent_requests: self.rate_limit.concurrent_requests,
            },
            cost: ProviderShimCostHintsView {
                input_microusd_per_million_tokens: self.cost.input_microusd_per_million_tokens,
                output_microusd_per_million_tokens: self.cost.output_microusd_per_million_tokens,
                currency: self.cost.currency.clone(),
            },
            redaction: ProviderShimRedactionPolicyView {
                redact_provider_errors: self.redaction.redact_provider_errors,
                redact_request_payloads: self.redaction.redact_request_payloads,
                redact_response_payloads: self.redaction.redact_response_payloads,
                max_error_chars: self.redaction.max_error_chars,
            },
        }
    }

    pub fn provider_shim_observability_snapshot(
        &self,
        health: ProviderShimHealthView,
    ) -> ProviderShimObservabilitySnapshotView {
        ProviderShimObservabilitySnapshotView::new(health, self.provider_shim_profile_snapshot())
    }
}

pub fn provider_shim_usage_event_from_runtime(
    runtime: &LlmRuntimeMetadata,
    assistant_run_id: Option<&str>,
    workflow_execution_id: Option<&str>,
) -> ProviderShimUsageEventView {
    let provider_failure = runtime.provider_failure.as_ref();
    ProviderShimUsageEventView {
        request_id: runtime.request_id.clone(),
        assistant_run_id: assistant_run_id.map(ToOwned::to_owned),
        workflow_execution_id: workflow_execution_id.map(ToOwned::to_owned),
        status: if provider_failure.is_some() {
            "failed".to_string()
        } else {
            "responded".to_string()
        },
        input_tokens: runtime
            .usage
            .as_ref()
            .map(|usage| usage.input_tokens as u64),
        output_tokens: runtime
            .usage
            .as_ref()
            .map(|usage| usage.output_tokens as u64),
        total_tokens: runtime
            .usage
            .as_ref()
            .map(|usage| usage.total_tokens as u64),
        latency_ms: runtime.latency_ms,
        provider_failure_kind: provider_failure.map(|failure| failure.kind.as_str().to_string()),
        provider_failure_message: provider_failure.map(|failure| failure.message.clone()),
        recorded_at: None,
    }
}

pub fn provider_shim_usage_summary_from_events(
    events: &[ProviderShimUsageEventView],
) -> ProviderShimUsageSummaryView {
    ProviderShimUsageSummaryView {
        request_count: events.len() as u64,
        failed_request_count: events
            .iter()
            .filter(|event| event.status == "failed")
            .count() as u64,
        input_tokens: events.iter().filter_map(|event| event.input_tokens).sum(),
        output_tokens: events.iter().filter_map(|event| event.output_tokens).sum(),
        total_tokens: events.iter().filter_map(|event| event.total_tokens).sum(),
        last_request_id: events
            .iter()
            .rev()
            .find_map(|event| event.request_id.clone()),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelRoute {
    pub lane: String,
    pub provider: String,
    pub model: String,
    pub capability_class: Vec<String>,
    pub priority: i32,
    pub fallback_route: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct ModelRouteRegistry {
    routes: Vec<ModelRoute>,
}

impl ModelRouteRegistry {
    pub fn empty() -> Self {
        Self { routes: Vec::new() }
    }

    pub fn from_routes(routes: Vec<ModelRoute>) -> Self {
        Self { routes }
    }

    pub fn from_env_with_defaults(default_lane: &str) -> Self {
        let mut registry = Self::from_routes(default_model_routes());
        registry.apply_env_overrides(default_lane);
        registry
    }

    pub fn select(&self, lane: &str) -> Option<ModelRoute> {
        self.routes
            .iter()
            .filter(|route| route.lane == lane)
            .max_by_key(|route| route.priority)
            .cloned()
    }

    pub fn routes(&self) -> &[ModelRoute] {
        &self.routes
    }

    fn apply_env_overrides(&mut self, default_lane: &str) {
        let mut lanes: Vec<String> = self.routes.iter().map(|route| route.lane.clone()).collect();
        if !default_lane.trim().is_empty() && !lanes.iter().any(|lane| lane == default_lane) {
            lanes.push(default_lane.trim().to_string());
        }

        for lane in lanes {
            if let Some(route) = model_route_from_env(&lane) {
                self.routes.retain(|existing| existing.lane != lane);
                self.routes.push(route);
            }
        }
    }
}

pub fn resolve_runtime_selection_from_env(
    env_prefix: &str,
    lane: &str,
    default_model: &str,
) -> LlmRuntimeSelection {
    let mut mode = std::env::var(format!("{env_prefix}_RUNTIME_MODE"))
        .unwrap_or_else(|_| "placeholder".to_string());
    let mut provider =
        std::env::var(format!("{env_prefix}_RUNTIME_PROVIDER")).unwrap_or_else(|_| mode.clone());
    let mut model = std::env::var(format!("{env_prefix}_RUNTIME_MODEL"))
        .unwrap_or_else(|_| default_model.to_string());

    let registry = ModelRouteRegistry::from_env_with_defaults(lane);
    if let Some(route) = registry.select(lane).filter(|route| route.priority > 0) {
        provider = route.provider;
        model = route.model;
        mode = if provider == "placeholder" {
            "placeholder".to_string()
        } else {
            "provider".to_string()
        };
    }

    LlmRuntimeSelection {
        mode,
        provider,
        model,
        lane: lane.to_string(),
    }
}

fn default_model_routes() -> Vec<ModelRoute> {
    vec![
        default_model_route(
            MODEL_LANE_ASSISTANT_CHAT,
            "placeholder",
            "assistant-chat-placeholder",
            &["text", "conversation"],
        ),
        default_model_route(
            MODEL_LANE_ASSISTANT_REACT_JSON,
            "placeholder",
            "assistant-react-json-placeholder",
            &["text", "json", "tool_control"],
        ),
        default_model_route(
            MODEL_LANE_STATIC_PAGE_INTENT,
            "placeholder",
            "static-page-intent-placeholder",
            &["text", "json", "static_page"],
        ),
        default_model_route(
            MODEL_LANE_STATIC_PAGE_IMAGE_PROMPT,
            "placeholder",
            "static-page-image-prompt-placeholder",
            &["text", "visual_prompt"],
        ),
        default_model_route(
            MODEL_LANE_CHAT_SESSION,
            "placeholder",
            "chat-session-placeholder",
            &["text", "conversation", "rag"],
        ),
        default_model_route(
            MODEL_LANE_DATASET_OUTPUT,
            "placeholder",
            "dataset-output-placeholder",
            &["text", "dataset_output"],
        ),
        default_model_route(
            MODEL_LANE_DOCUMENT_VLM,
            "placeholder",
            "document-vlm-placeholder",
            &["vision", "document"],
        ),
        default_model_route(
            MODEL_LANE_AUDIO_TRANSCRIPT,
            "placeholder",
            "audio-transcript-placeholder",
            &["audio", "transcript"],
        ),
        default_model_route(
            MODEL_LANE_VIDEO_SCENE_SUMMARY,
            "placeholder",
            "video-scene-summary-placeholder",
            &["video", "vision", "summary"],
        ),
        default_model_route(
            MODEL_LANE_REPORT_PLANNING,
            "placeholder",
            "report-planning-placeholder",
            &["text", "json", "report"],
        ),
        default_model_route(
            MODEL_LANE_CODEX_TASK_SUMMARY,
            "placeholder",
            "codex-task-summary-placeholder",
            &["text", "codex_task"],
        ),
    ]
}

fn default_model_route(
    lane: &str,
    provider: &str,
    model: &str,
    capability_class: &[&str],
) -> ModelRoute {
    ModelRoute {
        lane: lane.to_string(),
        provider: provider.to_string(),
        model: model.to_string(),
        capability_class: capability_class
            .iter()
            .map(|capability| capability.to_string())
            .collect(),
        priority: 0,
        fallback_route: None,
    }
}

fn model_route_from_env(lane: &str) -> Option<ModelRoute> {
    let env_prefix = model_route_env_prefix(lane);
    let provider = std::env::var(format!("{env_prefix}_PROVIDER")).ok()?;
    let model = std::env::var(format!("{env_prefix}_MODEL"))
        .unwrap_or_else(|_| format!("{}-default", provider.trim()));
    let capability_class = std::env::var(format!("{env_prefix}_CAPABILITIES"))
        .ok()
        .map(|value| split_csv_env(&value))
        .unwrap_or_default();
    let priority = std::env::var(format!("{env_prefix}_PRIORITY"))
        .ok()
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(100);
    let fallback_route = std::env::var(format!("{env_prefix}_FALLBACK"))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    Some(ModelRoute {
        lane: lane.to_string(),
        provider: provider.trim().to_string(),
        model: model.trim().to_string(),
        capability_class,
        priority,
        fallback_route,
    })
}

fn model_route_env_prefix(lane: &str) -> String {
    let normalized_lane: String = lane
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect();
    format!("LLM_GATEWAY_ROUTE_{normalized_lane}")
}

fn split_csv_env(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn required_env_string(env_prefix: &str, suffix: &str) -> Result<String> {
    optional_env_string(env_prefix, suffix)
        .ok_or_else(|| anyhow!("{env_prefix}_{suffix} is required"))
}

fn optional_env_string(env_prefix: &str, suffix: &str) -> Option<String> {
    std::env::var(format!("{env_prefix}_{suffix}"))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn optional_env_u32(env_prefix: &str, suffix: &str) -> Result<Option<u32>> {
    optional_env_string(env_prefix, suffix)
        .map(|value| {
            value
                .parse::<u32>()
                .map_err(|error| anyhow!("invalid {env_prefix}_{suffix} value {value}: {error}"))
        })
        .transpose()
}

fn optional_env_u64(env_prefix: &str, suffix: &str) -> Result<Option<u64>> {
    optional_env_string(env_prefix, suffix)
        .map(|value| {
            value
                .parse::<u64>()
                .map_err(|error| anyhow!("invalid {env_prefix}_{suffix} value {value}: {error}"))
        })
        .transpose()
}

fn optional_env_usize(env_prefix: &str, suffix: &str) -> Result<Option<usize>> {
    optional_env_string(env_prefix, suffix)
        .map(|value| {
            value
                .parse::<usize>()
                .map_err(|error| anyhow!("invalid {env_prefix}_{suffix} value {value}: {error}"))
        })
        .transpose()
}

fn optional_env_bool(env_prefix: &str, suffix: &str, default_value: bool) -> bool {
    std::env::var(format!("{env_prefix}_{suffix}"))
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default_value)
}

fn safe_env_key_name(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        Some(value.to_string())
    } else {
        Some(REDACTED_VALUE.to_string())
    }
}

fn provider_endpoint_scope(base_url: Option<&str>) -> String {
    let Some(base_url) = base_url.map(str::trim).filter(|value| !value.is_empty()) else {
        return "unconfigured".to_string();
    };
    let lower = base_url.to_ascii_lowercase();
    if lower.starts_with("http://127.0.0.1")
        || lower.starts_with("https://127.0.0.1")
        || lower.starts_with("http://localhost")
        || lower.starts_with("https://localhost")
        || lower.starts_with("http://[::1]")
        || lower.starts_with("https://[::1]")
        || lower.starts_with("unix:")
    {
        "local_private".to_string()
    } else if lower.starts_with("http://") || lower.starts_with("https://") {
        "remote".to_string()
    } else {
        "unknown".to_string()
    }
}

const OPENCLAW_CORRECTIVE_RETRY_INSTRUCTIONS: [&str; 2] = [
    "直接回答用户当前问题。不要自我介绍，不要谈内部状态，不要说自己刚启动、没有名字、没有记忆，也不要让用户给你起名。",
    "只输出最终答案。不要泄露工具调用、函数参数、JSON 工具轨迹或内部执行文本。",
];
const PROVIDER_ERROR_MAX_CHARS: usize = 900;
const REDACTED_VALUE: &str = "[redacted]";
const REDACTED_PATH: &str = "[redacted-path]";

pub fn redact_provider_error(message: &str) -> String {
    let redacted = redact_bearer_tokens(message);
    let redacted = [
        "authorization",
        "api_key",
        "api-key",
        "apikey",
        "x-api-key",
        "access_token",
        "refresh_token",
        "id_token",
        "cookie",
        "set-cookie",
        "secret",
        "password",
    ]
    .iter()
    .fold(redacted, |current, key| {
        redact_sensitive_key_values(&current, key)
    });
    let redacted = redact_secret_paths(&redacted);
    truncate_provider_error(&redacted)
}

fn redact_bearer_tokens(input: &str) -> String {
    let lower = input.to_ascii_lowercase();
    let mut output = String::new();
    let mut cursor = 0;
    let mut search_from = 0;

    while let Some(relative_index) = lower[search_from..].find("bearer") {
        let bearer_start = search_from + relative_index;
        let mut value_start = bearer_start + "bearer".len();
        let bytes = input.as_bytes();
        while value_start < bytes.len() && bytes[value_start].is_ascii_whitespace() {
            value_start += 1;
        }
        if value_start >= bytes.len() {
            break;
        }
        let mut value_end = value_start;
        while value_end < bytes.len() && !is_secret_value_delimiter(bytes[value_end]) {
            value_end += 1;
        }
        if value_end > value_start {
            output.push_str(&input[cursor..value_start]);
            output.push_str(REDACTED_VALUE);
            cursor = value_end;
        }
        search_from = value_end.max(bearer_start + "bearer".len());
    }

    output.push_str(&input[cursor..]);
    output
}

fn redact_sensitive_key_values(input: &str, key: &str) -> String {
    let lower = input.to_ascii_lowercase();
    let key = key.to_ascii_lowercase();
    let bytes = input.as_bytes();
    let mut output = String::new();
    let mut cursor = 0;
    let mut search_from = 0;

    while let Some(relative_index) = lower[search_from..].find(&key) {
        let key_start = search_from + relative_index;
        let mut separator_index = key_start + key.len();
        while separator_index < bytes.len()
            && matches!(bytes[separator_index], b' ' | b'\t' | b'\'' | b'"')
        {
            separator_index += 1;
        }
        if separator_index >= bytes.len() || !matches!(bytes[separator_index], b':' | b'=') {
            search_from = key_start + key.len();
            continue;
        }

        let mut value_start = separator_index + 1;
        while value_start < bytes.len() && bytes[value_start].is_ascii_whitespace() {
            value_start += 1;
        }
        let quote = if value_start < bytes.len() && matches!(bytes[value_start], b'\'' | b'"') {
            let quote = bytes[value_start];
            value_start += 1;
            Some(quote)
        } else {
            None
        };
        let mut value_end = value_start;
        while value_end < bytes.len() {
            let byte = bytes[value_end];
            if quote.is_some_and(|quote| byte == quote)
                || (quote.is_none() && is_secret_value_delimiter(byte))
            {
                break;
            }
            value_end += 1;
        }

        if value_end > value_start {
            output.push_str(&input[cursor..value_start]);
            output.push_str(REDACTED_VALUE);
            cursor = value_end;
        }
        search_from = value_end.max(key_start + key.len());
    }

    output.push_str(&input[cursor..]);
    output
}

fn redact_secret_paths(input: &str) -> String {
    [r"C:\Users\", "/Users/", "/home/"]
        .iter()
        .fold(input.to_string(), |current, prefix| {
            redact_value_with_prefix(&current, prefix, REDACTED_PATH)
        })
}

fn redact_value_with_prefix(input: &str, prefix: &str, replacement: &str) -> String {
    let mut output = String::new();
    let mut cursor = 0;
    let mut search_from = 0;

    while let Some(relative_index) = input[search_from..].find(prefix) {
        let value_start = search_from + relative_index;
        let bytes = input.as_bytes();
        let mut value_end = value_start;
        while value_end < bytes.len() && !is_secret_path_delimiter(bytes[value_end]) {
            value_end += 1;
        }
        output.push_str(&input[cursor..value_start]);
        output.push_str(replacement);
        cursor = value_end;
        search_from = value_end;
    }

    output.push_str(&input[cursor..]);
    output
}

fn is_secret_value_delimiter(byte: u8) -> bool {
    byte.is_ascii_whitespace() || matches!(byte, b',' | b';' | b'}' | b']' | b'&')
}

fn is_secret_path_delimiter(byte: u8) -> bool {
    byte.is_ascii_whitespace() || matches!(byte, b',' | b';' | b'}' | b']' | b'"' | b'\'')
}

fn truncate_provider_error(input: &str) -> String {
    if input.chars().count() <= PROVIDER_ERROR_MAX_CHARS {
        return input.to_string();
    }

    let mut truncated = input
        .chars()
        .take(PROVIDER_ERROR_MAX_CHARS)
        .collect::<String>();
    truncated.push_str("... [truncated]");
    truncated
}

pub fn render_runtime_manifest(runtime: &LlmRuntimeMetadata) -> Value {
    json!({
        "mode": runtime.mode.as_str(),
        "provider": runtime.provider.as_str(),
        "model": runtime.model.as_str(),
        "lane": runtime.lane.as_deref(),
        "request_id": runtime.request_id.as_deref(),
        "finish_reason": runtime.finish_reason.as_ref().map(LlmFinishReason::as_str),
        "provider_failure": runtime.provider_failure.as_ref().map(|failure| json!({
            "kind": failure.kind.as_str(),
            "message": failure.message,
        })),
        "latency_ms": runtime.latency_ms,
        "usage": runtime.usage.as_ref().map(|usage| json!({
            "input_tokens": usage.input_tokens,
            "output_tokens": usage.output_tokens,
            "total_tokens": usage.total_tokens,
        })),
        "system_prompt_key": runtime.system_prompt_key.as_deref(),
        "system_prompt_version": runtime.system_prompt_version.as_deref(),
        "tool_trace_count": runtime.tool_trace_count,
    })
}

pub trait LlmProvider: Debug + Send + Sync {
    fn name(&self) -> &str;
    fn complete(&self, request: &LlmRequest) -> Result<LlmResponse>;
}

pub fn build_provider_from_env(
    env_prefix: &str,
    runtime_mode: &str,
    runtime_provider: impl Into<String>,
    prompt_registry: InMemoryPromptRegistry,
) -> Result<Arc<dyn LlmProvider>> {
    let runtime_provider = runtime_provider.into();

    match runtime_mode {
        "placeholder" => Ok(Arc::new(
            PlaceholderLlmProvider::new(runtime_provider).with_prompt_registry(prompt_registry),
        )),
        "provider" => {
            if runtime_provider == "openclaw" {
                let provider = build_openclaw_provider_from_env(runtime_provider, prompt_registry)?;
                return Ok(Arc::new(provider));
            }

            if let Ok(api_base_url) = std::env::var(format!("{env_prefix}_RUNTIME_BASE_URL")) {
                let api_path = std::env::var(format!("{env_prefix}_RUNTIME_API_PATH"))
                    .unwrap_or_else(|_| "/v1/chat/completions".to_string());
                let api_key = std::env::var(format!("{env_prefix}_RUNTIME_API_KEY")).ok();
                let provider = OpenAiCompatibleLlmProvider::new(
                    runtime_provider,
                    OpenAiCompatibleLlmProviderConfig {
                        api_base_url,
                        api_path,
                        api_key,
                    },
                )?
                .with_prompt_registry(prompt_registry);
                return Ok(Arc::new(provider));
            }

            Ok(Arc::new(build_scripted_provider_from_env(
                env_prefix,
                runtime_provider,
                prompt_registry,
            )?))
        }
        other => Err(anyhow!(
            "unsupported {env_prefix}_RUNTIME_MODE {other}; expected placeholder or provider"
        )),
    }
}

#[derive(Clone, Debug)]
pub struct PlaceholderLlmProvider {
    provider_name: String,
    prompt_registry: InMemoryPromptRegistry,
}

impl Default for PlaceholderLlmProvider {
    fn default() -> Self {
        Self::new("placeholder")
    }
}

impl PlaceholderLlmProvider {
    pub fn new(provider_name: impl Into<String>) -> Self {
        Self {
            provider_name: provider_name.into(),
            prompt_registry: InMemoryPromptRegistry::default(),
        }
    }

    pub fn with_prompt_registry(mut self, prompt_registry: InMemoryPromptRegistry) -> Self {
        self.prompt_registry = prompt_registry;
        self
    }
}

impl LlmProvider for PlaceholderLlmProvider {
    fn name(&self) -> &str {
        &self.provider_name
    }

    fn complete(&self, request: &LlmRequest) -> Result<LlmResponse> {
        let started_at = Instant::now();
        let tool_calls = Vec::new();
        let output_text = request.input.clone();
        let usage = LlmTokenUsage {
            input_tokens: estimate_token_count(&request.input),
            output_tokens: estimate_token_count(&output_text),
            total_tokens: estimate_token_count(&request.input) + estimate_token_count(&output_text),
        };
        let runtime = LlmRuntimeMetadata {
            mode: LlmRuntimeMode::Placeholder,
            provider: self.provider_name.clone(),
            model: request.model.clone(),
            lane: request.lane.clone(),
            request_id: Some(Uuid::new_v4().to_string()),
            finish_reason: Some(LlmFinishReason::Stop),
            provider_failure: None,
            latency_ms: Some(started_at.elapsed().as_millis() as u64),
            usage: Some(usage),
            system_prompt_key: request.system_prompt_key.clone(),
            system_prompt_version: resolve_prompt_version(&self.prompt_registry, request),
            tool_trace_count: tool_calls.len(),
        };

        Ok(LlmResponse {
            output_text,
            runtime,
            tool_calls,
        })
    }
}

#[derive(Clone, Debug)]
pub struct ScriptedLlmProvider {
    provider_name: String,
    prompt_registry: InMemoryPromptRegistry,
    response_text: Option<String>,
    request_id: Option<String>,
    finish_reason: LlmFinishReason,
    latency_ms: u64,
    usage: Option<LlmTokenUsage>,
    tool_calls: Vec<LlmToolCall>,
}

impl ScriptedLlmProvider {
    pub fn new(provider_name: impl Into<String>) -> Self {
        Self {
            provider_name: provider_name.into(),
            prompt_registry: InMemoryPromptRegistry::default(),
            response_text: None,
            request_id: None,
            finish_reason: LlmFinishReason::Stop,
            latency_ms: 0,
            usage: None,
            tool_calls: Vec::new(),
        }
    }

    pub fn with_prompt_registry(mut self, prompt_registry: InMemoryPromptRegistry) -> Self {
        self.prompt_registry = prompt_registry;
        self
    }

    pub fn with_response_text(mut self, response_text: impl Into<String>) -> Self {
        self.response_text = Some(response_text.into());
        self
    }

    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    pub fn with_finish_reason(mut self, finish_reason: LlmFinishReason) -> Self {
        self.finish_reason = finish_reason;
        self
    }

    pub fn with_latency_ms(mut self, latency_ms: u64) -> Self {
        self.latency_ms = latency_ms;
        self
    }

    pub fn with_usage(mut self, usage: LlmTokenUsage) -> Self {
        self.usage = Some(usage);
        self
    }

    pub fn with_tool_calls(mut self, tool_calls: Vec<LlmToolCall>) -> Self {
        self.tool_calls = tool_calls;
        self
    }
}

impl LlmProvider for ScriptedLlmProvider {
    fn name(&self) -> &str {
        &self.provider_name
    }

    fn complete(&self, request: &LlmRequest) -> Result<LlmResponse> {
        let output_text = self
            .response_text
            .clone()
            .unwrap_or_else(|| request.input.clone());
        let tool_calls = self.tool_calls.clone();
        let usage = self
            .usage
            .clone()
            .unwrap_or_else(|| estimate_usage(&request.input, &output_text));
        let runtime = LlmRuntimeMetadata {
            mode: LlmRuntimeMode::Provider,
            provider: self.provider_name.clone(),
            model: request.model.clone(),
            lane: request.lane.clone(),
            request_id: Some(
                self.request_id
                    .clone()
                    .unwrap_or_else(|| Uuid::new_v4().to_string()),
            ),
            finish_reason: Some(self.finish_reason.clone()),
            provider_failure: provider_failure_for_finish_reason(
                &self.provider_name,
                Some(&self.finish_reason),
            ),
            latency_ms: Some(self.latency_ms),
            usage: Some(usage),
            system_prompt_key: request.system_prompt_key.clone(),
            system_prompt_version: resolve_prompt_version(&self.prompt_registry, request),
            tool_trace_count: tool_calls.len(),
        };

        Ok(LlmResponse {
            output_text,
            runtime,
            tool_calls,
        })
    }
}

#[derive(Clone, Debug)]
pub struct OpenAiCompatibleLlmProvider {
    provider_name: String,
    config: OpenAiCompatibleLlmProviderConfig,
    prompt_registry: InMemoryPromptRegistry,
    client: Client,
}

impl OpenAiCompatibleLlmProvider {
    pub fn new(
        provider_name: impl Into<String>,
        config: OpenAiCompatibleLlmProviderConfig,
    ) -> Result<Self> {
        Ok(Self {
            provider_name: provider_name.into(),
            config,
            prompt_registry: InMemoryPromptRegistry::default(),
            client: Client::builder()
                .build()
                .context("failed to build OpenAI-compatible HTTP client")?,
        })
    }

    pub fn with_prompt_registry(mut self, prompt_registry: InMemoryPromptRegistry) -> Self {
        self.prompt_registry = prompt_registry;
        self
    }
}

impl LlmProvider for OpenAiCompatibleLlmProvider {
    fn name(&self) -> &str {
        &self.provider_name
    }

    fn complete(&self, request: &LlmRequest) -> Result<LlmResponse> {
        let started_at = Instant::now();
        let endpoint = join_url(&self.config.api_base_url, &self.config.api_path);
        let system_prompt = request
            .system_prompt_key
            .as_deref()
            .and_then(|key| self.prompt_registry.active(key))
            .map(|prompt| prompt.body.clone());

        let mut messages = Vec::new();
        if let Some(system_prompt) = system_prompt {
            messages.push(json!({
                "role": "system",
                "content": system_prompt,
            }));
        }
        messages.push(json!({
            "role": "user",
            "content": request.input,
        }));

        let body = json!({
            "model": request.model,
            "messages": messages,
        });

        let mut http_request = self.client.post(&endpoint).json(&body);
        if let Some(api_key) = self.config.api_key.as_deref() {
            http_request = http_request.bearer_auth(api_key);
        }

        let response = match http_request.send() {
            Ok(response) => response,
            Err(error) => {
                let kind = if error.is_timeout() {
                    LlmProviderFailureKind::RequestTimeout
                } else {
                    LlmProviderFailureKind::RequestFailed
                };
                let message = format!(
                    "{} request to {endpoint} failed: {error}",
                    self.provider_name
                );
                return Err(self
                    .provider_error(request, kind, message, started_at)
                    .into());
            }
        };
        let status = response.status();
        let response_body = match response.text() {
            Ok(body) => body,
            Err(error) => {
                let message = format!("{} response body read failed: {error}", self.provider_name);
                return Err(self
                    .provider_error(
                        request,
                        LlmProviderFailureKind::ResponseBodyReadFailed,
                        message,
                        started_at,
                    )
                    .into());
            }
        };

        if !status.is_success() {
            let message = format!(
                "{} returned HTTP {} with body {}",
                self.provider_name,
                status.as_u16(),
                response_body
            );
            return Err(self
                .provider_error(
                    request,
                    LlmProviderFailureKind::HttpStatus,
                    message,
                    started_at,
                )
                .into());
        }

        let value = match serde_json::from_str::<Value>(&response_body) {
            Ok(value) => value,
            Err(error) => {
                let message = format!(
                    "{} returned invalid JSON payload from {endpoint}: {}",
                    self.provider_name, response_body
                );
                return Err(self
                    .provider_error(
                        request,
                        LlmProviderFailureKind::InvalidJson,
                        format!("{message} ({error})"),
                        started_at,
                    )
                    .into());
            }
        };
        let Some(choice) = value
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
        else {
            let message = format!("{} response missing choices[0]", self.provider_name);
            return Err(self
                .provider_error(
                    request,
                    LlmProviderFailureKind::InvalidResponse,
                    message,
                    started_at,
                )
                .into());
        };
        let Some(output_text) = extract_chat_completion_text(choice) else {
            let message = format!(
                "{} response missing assistant message content",
                self.provider_name
            );
            return Err(self
                .provider_error(
                    request,
                    LlmProviderFailureKind::InvalidResponse,
                    message,
                    started_at,
                )
                .into());
        };
        let output_text = normalize_provider_output_text(&output_text);
        let tool_calls = match extract_chat_completion_tool_calls(choice) {
            Ok(tool_calls) => tool_calls,
            Err(error) => {
                let message = format!(
                    "{} response has invalid tool calls: {error}",
                    self.provider_name
                );
                return Err(self
                    .provider_error(
                        request,
                        LlmProviderFailureKind::InvalidResponse,
                        message,
                        started_at,
                    )
                    .into());
            }
        };
        let finish_reason = choice
            .get("finish_reason")
            .and_then(Value::as_str)
            .map(LlmFinishReason::from_wire_value);
        let runtime = LlmRuntimeMetadata {
            mode: LlmRuntimeMode::Provider,
            provider: self.provider_name.clone(),
            model: request.model.clone(),
            lane: request.lane.clone(),
            request_id: value.get("id").and_then(Value::as_str).map(str::to_string),
            finish_reason: finish_reason.clone(),
            provider_failure: provider_failure_for_finish_reason(
                &self.provider_name,
                finish_reason.as_ref(),
            ),
            latency_ms: Some(started_at.elapsed().as_millis() as u64),
            usage: parse_usage(value.get("usage")),
            system_prompt_key: request.system_prompt_key.clone(),
            system_prompt_version: resolve_prompt_version(&self.prompt_registry, request),
            tool_trace_count: tool_calls.len(),
        };

        Ok(LlmResponse {
            output_text,
            runtime,
            tool_calls,
        })
    }
}

impl OpenAiCompatibleLlmProvider {
    fn provider_error(
        &self,
        request: &LlmRequest,
        kind: LlmProviderFailureKind,
        message: String,
        started_at: Instant,
    ) -> LlmProviderError {
        let message = redact_provider_error(&message);
        let runtime = LlmRuntimeMetadata {
            mode: LlmRuntimeMode::Provider,
            provider: self.provider_name.clone(),
            model: request.model.clone(),
            lane: request.lane.clone(),
            request_id: None,
            finish_reason: Some(LlmFinishReason::Error),
            provider_failure: Some(LlmProviderFailure {
                kind,
                message: message.clone(),
            }),
            latency_ms: Some(started_at.elapsed().as_millis() as u64),
            usage: None,
            system_prompt_key: request.system_prompt_key.clone(),
            system_prompt_version: resolve_prompt_version(&self.prompt_registry, request),
            tool_trace_count: 0,
        };

        LlmProviderError::new(runtime, message)
    }
}

#[derive(Clone, Debug)]
pub struct OpenClawLlmProvider {
    provider_name: String,
    config: OpenClawLlmProviderConfig,
    prompt_registry: InMemoryPromptRegistry,
    client: Client,
}

impl OpenClawLlmProvider {
    pub fn new(
        provider_name: impl Into<String>,
        config: OpenClawLlmProviderConfig,
    ) -> Result<Self> {
        Ok(Self {
            provider_name: provider_name.into(),
            client: Client::builder()
                .timeout(Duration::from_millis(config.timeout_ms.max(1)))
                .build()
                .context("failed to build OpenClaw HTTP client")?,
            config,
            prompt_registry: InMemoryPromptRegistry::default(),
        })
    }

    pub fn with_prompt_registry(mut self, prompt_registry: InMemoryPromptRegistry) -> Self {
        self.prompt_registry = prompt_registry;
        self
    }

    fn complete_responses(&self, request: &LlmRequest) -> Result<LlmResponse> {
        self.complete_responses_with_instruction(request, None)
    }

    fn complete_responses_with_instruction(
        &self,
        request: &LlmRequest,
        additional_instruction: Option<&str>,
    ) -> Result<LlmResponse> {
        let started_at = Instant::now();
        let endpoint = join_url(&self.config.gateway_base_url, "/v1/responses");
        let system_prompt = self.resolve_system_prompt(request);
        let model = self.resolve_request_model(request);
        let mut body = json!({
            "model": model,
            "user": "v3",
            "input": request.input,
            "temperature": 0.2,
            "reasoning": {
                "effort": "medium",
                "summary": "auto",
            },
        });
        let instructions =
            merge_openclaw_instructions(system_prompt.as_deref(), additional_instruction);
        if let Some(instructions) = instructions {
            body["instructions"] = json!(instructions);
        }

        let value = self.send_json(request, &endpoint, &body, started_at)?;
        let Some(output_text) = extract_responses_output_text(&value) else {
            let message = format!("{} response missing output text", self.provider_name);
            return Err(self
                .provider_error(
                    request,
                    LlmProviderFailureKind::InvalidResponse,
                    message,
                    started_at,
                )
                .into());
        };
        let output_text = normalize_provider_output_text(&output_text);
        let runtime = LlmRuntimeMetadata {
            mode: LlmRuntimeMode::Provider,
            provider: self.provider_name.clone(),
            model,
            lane: request.lane.clone(),
            request_id: value.get("id").and_then(Value::as_str).map(str::to_string),
            finish_reason: Some(LlmFinishReason::Stop),
            provider_failure: None,
            latency_ms: Some(started_at.elapsed().as_millis() as u64),
            usage: parse_usage(value.get("usage")),
            system_prompt_key: request.system_prompt_key.clone(),
            system_prompt_version: resolve_prompt_version(&self.prompt_registry, request),
            tool_trace_count: 0,
        };

        Ok(LlmResponse {
            output_text,
            runtime,
            tool_calls: Vec::new(),
        })
    }

    fn complete_chat(&self, request: &LlmRequest) -> Result<LlmResponse> {
        self.complete_chat_with_instruction(request, None)
    }

    fn complete_chat_with_instruction(
        &self,
        request: &LlmRequest,
        additional_instruction: Option<&str>,
    ) -> Result<LlmResponse> {
        let started_at = Instant::now();
        let endpoint = join_url(&self.config.gateway_base_url, "/v1/chat/completions");
        let system_prompt = self.resolve_system_prompt(request);
        let model = self.resolve_request_model(request);
        let mut messages = Vec::new();
        if let Some(system_prompt) =
            merge_openclaw_instructions(system_prompt.as_deref(), additional_instruction)
        {
            messages.push(json!({
                "role": "system",
                "content": system_prompt,
            }));
        }
        messages.push(json!({
            "role": "user",
            "content": request.input,
        }));
        let body = json!({
            "model": model,
            "user": "v3",
            "temperature": 0.2,
            "messages": messages,
        });

        let value = self.send_json(request, &endpoint, &body, started_at)?;
        let Some(choice) = value
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
        else {
            let message = format!("{} response missing choices[0]", self.provider_name);
            return Err(self
                .provider_error(
                    request,
                    LlmProviderFailureKind::InvalidResponse,
                    message,
                    started_at,
                )
                .into());
        };
        let Some(output_text) = extract_chat_completion_text(choice) else {
            let message = format!(
                "{} response missing assistant message content",
                self.provider_name
            );
            return Err(self
                .provider_error(
                    request,
                    LlmProviderFailureKind::InvalidResponse,
                    message,
                    started_at,
                )
                .into());
        };
        let output_text = normalize_provider_output_text(&output_text);
        let tool_calls = extract_chat_completion_tool_calls(choice).map_err(|error| {
            self.provider_error(
                request,
                LlmProviderFailureKind::InvalidResponse,
                format!(
                    "{} response has invalid tool calls: {error}",
                    self.provider_name
                ),
                started_at,
            )
        })?;
        let finish_reason = choice
            .get("finish_reason")
            .and_then(Value::as_str)
            .map(LlmFinishReason::from_wire_value);
        let runtime = LlmRuntimeMetadata {
            mode: LlmRuntimeMode::Provider,
            provider: self.provider_name.clone(),
            model,
            lane: request.lane.clone(),
            request_id: value.get("id").and_then(Value::as_str).map(str::to_string),
            finish_reason: finish_reason.clone(),
            provider_failure: provider_failure_for_finish_reason(
                &self.provider_name,
                finish_reason.as_ref(),
            ),
            latency_ms: Some(started_at.elapsed().as_millis() as u64),
            usage: parse_usage(value.get("usage")),
            system_prompt_key: request.system_prompt_key.clone(),
            system_prompt_version: resolve_prompt_version(&self.prompt_registry, request),
            tool_trace_count: tool_calls.len(),
        };

        Ok(LlmResponse {
            output_text,
            runtime,
            tool_calls,
        })
    }

    fn resolve_system_prompt(&self, request: &LlmRequest) -> Option<String> {
        request
            .system_prompt_key
            .as_deref()
            .and_then(|key| self.prompt_registry.active(key))
            .map(|prompt| prompt.body.clone())
    }

    fn resolve_request_model(&self, request: &LlmRequest) -> String {
        self.config
            .model
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(request.model.as_str())
            .to_string()
    }

    fn retry_bad_responses_output(
        &self,
        request: &LlmRequest,
        first_response: LlmResponse,
    ) -> LlmResponse {
        if !looks_like_openclaw_retryable_bad_output(&first_response.output_text) {
            return first_response;
        }

        for instruction in OPENCLAW_CORRECTIVE_RETRY_INSTRUCTIONS {
            if let Ok(response) =
                self.complete_responses_with_instruction(request, Some(instruction))
            {
                if !looks_like_openclaw_retryable_bad_output(&response.output_text) {
                    return response;
                }
            }
        }

        first_response
    }

    fn retry_bad_chat_output(
        &self,
        request: &LlmRequest,
        first_response: LlmResponse,
    ) -> LlmResponse {
        if !looks_like_openclaw_retryable_bad_output(&first_response.output_text) {
            return first_response;
        }

        for instruction in OPENCLAW_CORRECTIVE_RETRY_INSTRUCTIONS {
            if let Ok(response) = self.complete_chat_with_instruction(request, Some(instruction)) {
                if !looks_like_openclaw_retryable_bad_output(&response.output_text) {
                    return response;
                }
            }
        }

        first_response
    }

    fn send_json(
        &self,
        request: &LlmRequest,
        endpoint: &str,
        body: &Value,
        started_at: Instant,
    ) -> Result<Value, LlmProviderError> {
        let mut http_request = self.client.post(endpoint).json(body);
        if let Some(token) = self.config.token.as_deref() {
            http_request = http_request.bearer_auth(token);
        }
        if let Some(agent_id) = self.config.agent_id.as_deref() {
            http_request = http_request.header("x-openclaw-agent-id", agent_id);
        }
        if let Some(model_override) = self.config.model_override.as_deref() {
            http_request = http_request.header("x-openclaw-model", model_override);
        }

        let response = match http_request.send() {
            Ok(response) => response,
            Err(error) => {
                let kind = if error.is_timeout() {
                    LlmProviderFailureKind::RequestTimeout
                } else {
                    LlmProviderFailureKind::RequestFailed
                };
                let message = format!(
                    "{} request to {endpoint} failed: {error}",
                    self.provider_name
                );
                return Err(self.provider_error(request, kind, message, started_at));
            }
        };
        let status = response.status();
        let response_body = match response.text() {
            Ok(body) => body,
            Err(error) => {
                let message = format!("{} response body read failed: {error}", self.provider_name);
                return Err(self.provider_error(
                    request,
                    LlmProviderFailureKind::ResponseBodyReadFailed,
                    message,
                    started_at,
                ));
            }
        };
        if !status.is_success() {
            let message = format!(
                "{} returned HTTP {} with body {}",
                self.provider_name,
                status.as_u16(),
                response_body
            );
            return Err(self.provider_error(
                request,
                LlmProviderFailureKind::HttpStatus,
                message,
                started_at,
            ));
        }

        serde_json::from_str::<Value>(&response_body).map_err(|error| {
            let message = format!(
                "{} returned invalid JSON payload from {endpoint}: {} ({error})",
                self.provider_name, response_body
            );
            self.provider_error(
                request,
                LlmProviderFailureKind::InvalidJson,
                message,
                started_at,
            )
        })
    }

    fn provider_error(
        &self,
        request: &LlmRequest,
        kind: LlmProviderFailureKind,
        message: String,
        started_at: Instant,
    ) -> LlmProviderError {
        let message = redact_provider_error(&message);
        let runtime = LlmRuntimeMetadata {
            mode: LlmRuntimeMode::Provider,
            provider: self.provider_name.clone(),
            model: request.model.clone(),
            lane: request.lane.clone(),
            request_id: None,
            finish_reason: Some(LlmFinishReason::Error),
            provider_failure: Some(LlmProviderFailure {
                kind,
                message: message.clone(),
            }),
            latency_ms: Some(started_at.elapsed().as_millis() as u64),
            usage: None,
            system_prompt_key: request.system_prompt_key.clone(),
            system_prompt_version: resolve_prompt_version(&self.prompt_registry, request),
            tool_trace_count: 0,
        };

        LlmProviderError::new(runtime, message)
    }
}

impl LlmProvider for OpenClawLlmProvider {
    fn name(&self) -> &str {
        &self.provider_name
    }

    fn complete(&self, request: &LlmRequest) -> Result<LlmResponse> {
        if self.config.prefer_responses {
            if let Ok(response) = self.complete_responses(request) {
                return Ok(self.retry_bad_responses_output(request, response));
            }
        }

        let response = self.complete_chat(request)?;
        Ok(self.retry_bad_chat_output(request, response))
    }
}

fn build_scripted_provider_from_env(
    env_prefix: &str,
    runtime_provider: String,
    prompt_registry: InMemoryPromptRegistry,
) -> Result<ScriptedLlmProvider> {
    let mut provider =
        ScriptedLlmProvider::new(runtime_provider).with_prompt_registry(prompt_registry);

    if let Ok(output_text) = std::env::var(format!("{env_prefix}_RUNTIME_OUTPUT_TEXT")) {
        provider = provider.with_response_text(output_text);
    }
    if let Ok(request_id) = std::env::var(format!("{env_prefix}_RUNTIME_REQUEST_ID")) {
        provider = provider.with_request_id(request_id);
    }
    if let Ok(finish_reason) = std::env::var(format!("{env_prefix}_RUNTIME_FINISH_REASON")) {
        provider = provider.with_finish_reason(LlmFinishReason::from_wire_value(&finish_reason));
    }
    if let Ok(latency_ms) = std::env::var(format!("{env_prefix}_RUNTIME_LATENCY_MS")) {
        provider = provider.with_latency_ms(latency_ms.parse::<u64>().map_err(|error| {
            anyhow!("invalid {env_prefix}_RUNTIME_LATENCY_MS value {latency_ms}: {error}")
        })?);
    }
    if let Ok(usage_json) = std::env::var(format!("{env_prefix}_RUNTIME_USAGE_JSON")) {
        let usage = serde_json::from_str::<LlmTokenUsage>(&usage_json).map_err(|error| {
            anyhow!("invalid {env_prefix}_RUNTIME_USAGE_JSON payload {usage_json}: {error}")
        })?;
        provider = provider.with_usage(usage);
    }
    if let Ok(tool_trace_json) = std::env::var(format!("{env_prefix}_RUNTIME_TOOL_TRACE_JSON")) {
        let tool_calls = serde_json::from_str::<Vec<LlmToolCall>>(&tool_trace_json).map_err(
            |error| {
                anyhow!(
                    "invalid {env_prefix}_RUNTIME_TOOL_TRACE_JSON payload {tool_trace_json}: {error}"
                )
            },
        )?;
        provider = provider.with_tool_calls(tool_calls);
    }

    Ok(provider)
}

fn build_openclaw_provider_from_env(
    runtime_provider: String,
    prompt_registry: InMemoryPromptRegistry,
) -> Result<OpenClawLlmProvider> {
    if !env_flag("OPENCLAW_EXTENSION_ENABLED", false) {
        return Err(anyhow!(
            "OPENCLAW_EXTENSION_ENABLED must be true to use runtime provider openclaw"
        ));
    }

    let gateway_base_url = std::env::var("OPENCLAW_GATEWAY_BASE_URL")
        .or_else(|_| std::env::var("OPENCLAW_GATEWAY_URL"))
        .map_err(|_| {
            anyhow!(
                "OPENCLAW_GATEWAY_BASE_URL is required when runtime provider openclaw is selected"
            )
        })?;
    let token = std::env::var("OPENCLAW_GATEWAY_TOKEN").ok();
    if token.as_deref().map(str::trim).unwrap_or("").is_empty()
        && !env_flag("OPENCLAW_GATEWAY_TOKEN_OPTIONAL", false)
    {
        return Err(anyhow!(
            "OPENCLAW_GATEWAY_TOKEN is required unless OPENCLAW_GATEWAY_TOKEN_OPTIONAL=true"
        ));
    }
    let timeout_ms = std::env::var("OPENCLAW_TIMEOUT_MS")
        .ok()
        .map(|value| {
            value
                .parse::<u64>()
                .map_err(|error| anyhow!("invalid OPENCLAW_TIMEOUT_MS value {value}: {error}"))
        })
        .transpose()?
        .unwrap_or(60_000);

    OpenClawLlmProvider::new(
        runtime_provider,
        OpenClawLlmProviderConfig {
            gateway_base_url,
            token,
            agent_id: std::env::var("OPENCLAW_AGENT_ID").ok(),
            model: std::env::var("OPENCLAW_MODEL").ok(),
            model_override: std::env::var("OPENCLAW_MODEL_OVERRIDE").ok(),
            prefer_responses: env_flag("OPENCLAW_PREFER_RESPONSES", true),
            timeout_ms,
        },
    )
    .map(|provider| provider.with_prompt_registry(prompt_registry))
}

fn resolve_prompt_version(
    prompt_registry: &InMemoryPromptRegistry,
    request: &LlmRequest,
) -> Option<String> {
    request.system_prompt_key.as_deref().and_then(|key| {
        prompt_registry
            .active(key)
            .map(|prompt| prompt.active_version.clone())
    })
}

fn env_flag(key: &str, default_value: bool) -> bool {
    std::env::var(key)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default_value)
}

fn merge_openclaw_instructions(
    system_prompt: Option<&str>,
    additional_instruction: Option<&str>,
) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(system_prompt) = system_prompt
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        parts.push(system_prompt.to_string());
    }
    if let Some(additional_instruction) = additional_instruction
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        parts.push(additional_instruction.to_string());
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n\n"))
    }
}

fn join_url(base: &str, path: &str) -> String {
    format!(
        "{}{}",
        base.trim_end_matches('/'),
        if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{path}")
        }
    )
}

fn parse_usage(value: Option<&Value>) -> Option<LlmTokenUsage> {
    let usage = value?.as_object()?;

    Some(LlmTokenUsage {
        input_tokens: usage
            .get("prompt_tokens")
            .or_else(|| usage.get("input_tokens"))?
            .as_u64()? as usize,
        output_tokens: usage
            .get("completion_tokens")
            .or_else(|| usage.get("output_tokens"))?
            .as_u64()? as usize,
        total_tokens: usage
            .get("total_tokens")
            .or_else(|| usage.get("input_tokens"))?
            .as_u64()? as usize,
    })
}

fn extract_chat_completion_text(choice: &Value) -> Option<String> {
    let message = choice.get("message")?.as_object()?;
    let content = message.get("content")?;

    if let Some(value) = content.as_str() {
        return Some(value.to_string());
    }

    let content = content.as_array()?;
    let mut parts = Vec::new();
    for item in content {
        let item = item.as_object()?;
        if item.get("type").and_then(Value::as_str) == Some("text") {
            parts.push(item.get("text")?.as_str()?.to_string());
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

fn normalize_provider_output_text(value: &str) -> String {
    strip_leading_reasoning_blocks(value)
}

fn strip_leading_reasoning_blocks(value: &str) -> String {
    let mut remaining = value;
    let mut removed = false;

    loop {
        let trimmed = remaining.trim_start();
        let lower = trimmed.to_ascii_lowercase();
        let Some(after_open_tag) = lower.strip_prefix("<think>").map(|_| "<think>".len()) else {
            break;
        };
        let Some(close_tag_offset) = lower[after_open_tag..].find("</think>") else {
            break;
        };
        let close_tag_end = after_open_tag + close_tag_offset + "</think>".len();
        remaining = &trimmed[close_tag_end..];
        removed = true;
    }

    if removed {
        remaining.trim_start().to_string()
    } else {
        value.to_string()
    }
}

fn extract_chat_completion_tool_calls(choice: &Value) -> Result<Vec<LlmToolCall>> {
    let Some(tool_calls) = choice
        .get("message")
        .and_then(|message| message.get("tool_calls"))
        .and_then(Value::as_array)
    else {
        return Ok(Vec::new());
    };

    tool_calls
        .iter()
        .map(|tool_call| {
            let function = tool_call
                .get("function")
                .and_then(Value::as_object)
                .ok_or_else(|| anyhow!("tool_call.function missing from provider response"))?;
            let arguments = function
                .get("arguments")
                .and_then(Value::as_str)
                .map(parse_tool_arguments)
                .transpose()?;

            Ok(LlmToolCall {
                call_id: tool_call
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                tool_name: function
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow!("tool_call.function.name missing"))?
                    .to_string(),
                status: LlmToolCallStatus::Requested,
                arguments,
                result: None,
            })
        })
        .collect()
}

fn looks_like_openclaw_retryable_bad_output(content: &str) -> bool {
    looks_like_openclaw_onboarding_drift(content)
        || looks_like_openclaw_leaked_tool_call_content(content)
        || looks_like_openclaw_native_tool_failure(content)
}

fn looks_like_openclaw_onboarding_drift(content: &str) -> bool {
    let content = content.trim().to_ascii_lowercase();
    if content.is_empty() {
        return false;
    }

    (content.contains("刚启动")
        || content.contains("第一次")
        || content.contains("first time")
        || content.contains("just started")
        || content.contains("no memory"))
        && (content.contains("起名")
            || content.contains("名字")
            || content.contains("name me")
            || content.contains("give me a name")
            || content.contains("没有名字"))
}

fn looks_like_openclaw_leaked_tool_call_content(content: &str) -> bool {
    let trimmed = content.trim();
    let lower = trimmed.to_ascii_lowercase();

    lower.contains("<tool_call")
        || lower.contains("</tool_call")
        || lower.contains("function_call")
        || lower.contains("\"tool_calls\"")
        || lower.contains("\"tool_call\"")
        || (trimmed.starts_with('{')
            && (lower.contains("\"arguments\"")
                || lower.contains("\"function\"")
                || lower.contains("\"tool\"")))
}

fn looks_like_openclaw_native_tool_failure(content: &str) -> bool {
    let lower = content.trim().to_ascii_lowercase();
    lower.contains("cannot read properties of undefined")
        || lower.contains("tool failed")
        || lower.contains("search failed")
        || lower.contains("web search")
            && (lower.contains("failed") || lower.contains("unavailable"))
        || lower.contains("无法访问外部搜索")
        || lower.contains("无法联网搜索")
        || lower.contains("工具调用失败")
}

fn extract_responses_output_text(value: &Value) -> Option<String> {
    if let Some(text) = value.get("output_text").and_then(Value::as_str) {
        let text = text.trim();
        if !text.is_empty() {
            return Some(text.to_string());
        }
    }

    let mut parts = Vec::new();
    for output in value.get("output")?.as_array()? {
        if let Some(text) = output.get("text").and_then(Value::as_str) {
            if !text.trim().is_empty() {
                parts.push(text.to_string());
            }
        }
        let Some(content) = output.get("content").and_then(Value::as_array) else {
            continue;
        };
        for item in content {
            if let Some(text) = item
                .get("text")
                .or_else(|| item.get("output_text"))
                .and_then(Value::as_str)
            {
                if !text.trim().is_empty() {
                    parts.push(text.to_string());
                }
            }
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

fn parse_tool_arguments(value: &str) -> Result<Value> {
    match serde_json::from_str(value) {
        Ok(parsed) => Ok(parsed),
        Err(_) => Ok(Value::String(value.to_string())),
    }
}

fn estimate_usage(input: &str, output: &str) -> LlmTokenUsage {
    let input_tokens = estimate_token_count(input);
    let output_tokens = estimate_token_count(output);

    LlmTokenUsage {
        input_tokens,
        output_tokens,
        total_tokens: input_tokens + output_tokens,
    }
}

fn estimate_token_count(value: &str) -> usize {
    value.split_whitespace().count()
}

fn provider_failure_for_finish_reason(
    provider_name: &str,
    finish_reason: Option<&LlmFinishReason>,
) -> Option<LlmProviderFailure> {
    if !matches!(finish_reason, Some(LlmFinishReason::Error)) {
        return None;
    }

    Some(LlmProviderFailure {
        kind: LlmProviderFailureKind::FinishReasonError,
        message: format!("{provider_name} returned finish_reason=error"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use prompt_registry::{bootstrap_default_prompt_registry, CHAT_SESSION_PLACEHOLDER_PROMPT_KEY};
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Mutex, OnceLock};
    use std::thread;

    #[test]
    fn model_route_registry_selects_lane_default() {
        let _guard = model_route_env_lock().lock().expect("model route env lock");
        clear_model_route_env(MODEL_LANE_ASSISTANT_CHAT);
        let registry = ModelRouteRegistry::from_env_with_defaults(MODEL_LANE_ASSISTANT_CHAT);
        let route = registry
            .select(MODEL_LANE_ASSISTANT_CHAT)
            .expect("assistant chat route");

        assert_eq!(route.lane, MODEL_LANE_ASSISTANT_CHAT);
        assert!(!route.provider.is_empty());
        assert!(!route.model.is_empty());
        assert!(route.capability_class.contains(&"text".to_string()));
    }

    #[test]
    fn model_route_registry_rejects_unknown_lane_without_default() {
        let registry = ModelRouteRegistry::empty();

        assert!(registry.select("missing").is_none());
    }

    #[test]
    fn model_route_registry_can_route_lane_to_minimax_from_env() {
        let _guard = model_route_env_lock().lock().expect("model route env lock");
        clear_model_route_env(MODEL_LANE_DOCUMENT_VLM);
        std::env::set_var(
            "LLM_GATEWAY_ROUTE_DOCUMENT_VLM_PROVIDER",
            "minimax_openai_compatible",
        );
        std::env::set_var("LLM_GATEWAY_ROUTE_DOCUMENT_VLM_MODEL", "abab7.0-chat");
        std::env::set_var(
            "LLM_GATEWAY_ROUTE_DOCUMENT_VLM_CAPABILITIES",
            "vision,document,json",
        );
        std::env::set_var("LLM_GATEWAY_ROUTE_DOCUMENT_VLM_PRIORITY", "120");

        let registry = ModelRouteRegistry::from_env_with_defaults(MODEL_LANE_DOCUMENT_VLM);
        let route = registry
            .select(MODEL_LANE_DOCUMENT_VLM)
            .expect("document vlm route");

        clear_model_route_env(MODEL_LANE_DOCUMENT_VLM);
        assert_eq!(route.provider, "minimax_openai_compatible");
        assert_eq!(route.model, "abab7.0-chat");
        assert_eq!(
            route.capability_class,
            vec![
                "vision".to_string(),
                "document".to_string(),
                "json".to_string()
            ]
        );
        assert_eq!(route.priority, 120);
    }

    #[test]
    fn runtime_selection_prefers_lane_route_over_legacy_runtime_env() {
        let _guard = model_route_env_lock().lock().expect("model route env lock");
        clear_model_route_env(MODEL_LANE_ASSISTANT_CHAT);
        std::env::set_var("TEST_ASSISTANT_RUNTIME_MODE", "placeholder");
        std::env::set_var("TEST_ASSISTANT_RUNTIME_PROVIDER", "placeholder");
        std::env::set_var("TEST_ASSISTANT_RUNTIME_MODEL", "legacy-placeholder");
        std::env::set_var(
            "LLM_GATEWAY_ROUTE_ASSISTANT_CHAT_PROVIDER",
            "minimax_openai_compatible",
        );
        std::env::set_var("LLM_GATEWAY_ROUTE_ASSISTANT_CHAT_MODEL", "MiniMax-M2.7");

        let selection = resolve_runtime_selection_from_env(
            "TEST_ASSISTANT",
            MODEL_LANE_ASSISTANT_CHAT,
            "default-model",
        );

        clear_model_route_env(MODEL_LANE_ASSISTANT_CHAT);
        std::env::remove_var("TEST_ASSISTANT_RUNTIME_MODE");
        std::env::remove_var("TEST_ASSISTANT_RUNTIME_PROVIDER");
        std::env::remove_var("TEST_ASSISTANT_RUNTIME_MODEL");
        assert_eq!(selection.mode, "provider");
        assert_eq!(selection.provider, "minimax_openai_compatible");
        assert_eq!(selection.model, "MiniMax-M2.7");
        assert_eq!(selection.lane, MODEL_LANE_ASSISTANT_CHAT);
    }

    #[test]
    fn model_provider_profile_from_env_builds_redacted_gpt_manifest() {
        let _guard = model_route_env_lock()
            .lock()
            .expect("model profile env lock");
        clear_model_profile_env("TEST_GPT_PROFILE");
        std::env::remove_var("OPENAI_API_KEY");
        std::env::set_var("OPENAI_API_KEY", "sk-openai-secret");
        std::env::set_var("TEST_GPT_PROFILE_PROFILE_ID", "gpt-main");
        std::env::set_var("TEST_GPT_PROFILE_PROVIDER_ID", "openai");
        std::env::set_var("TEST_GPT_PROFILE_MODEL_ID", "gpt-5.4");
        std::env::set_var("TEST_GPT_PROFILE_BASE_URL", "https://api.openai.example/v1");
        std::env::set_var("TEST_GPT_PROFILE_API_PATH", "/v1/chat/completions");
        std::env::set_var("TEST_GPT_PROFILE_WIRE_API", "chat_completions");
        std::env::set_var("TEST_GPT_PROFILE_AUTH_ENV_KEY", "OPENAI_API_KEY");
        std::env::set_var(
            "TEST_GPT_PROFILE_CAPABILITIES",
            "chat,reasoning,json,tool_calling",
        );
        std::env::set_var("TEST_GPT_PROFILE_TIMEOUT_MS", "45000");
        std::env::set_var("TEST_GPT_PROFILE_RATE_LIMIT_RPM", "120");
        std::env::set_var(
            "TEST_GPT_PROFILE_COST_INPUT_MICROUSD_PER_MILLION_TOKENS",
            "2500000",
        );
        std::env::set_var(
            "TEST_GPT_PROFILE_COST_OUTPUT_MICROUSD_PER_MILLION_TOKENS",
            "10000000",
        );
        std::env::set_var("TEST_GPT_PROFILE_COST_CURRENCY", "USD");

        let profile = ModelProviderProfile::from_env("TEST_GPT_PROFILE")
            .expect("profile should parse from env");
        let manifest = profile.public_manifest();
        let serialized = manifest.to_string();
        let selection = profile.runtime_selection_for_lane(MODEL_LANE_ASSISTANT_CHAT);

        clear_model_profile_env("TEST_GPT_PROFILE");
        std::env::remove_var("OPENAI_API_KEY");
        assert_eq!(profile.profile_id, "gpt-main");
        assert_eq!(profile.wire_api, ModelProfileWireApi::ChatCompletions);
        assert_eq!(selection.provider, "openai");
        assert_eq!(selection.model, "gpt-5.4");
        assert_eq!(manifest["auth"]["env_key_name"], json!("OPENAI_API_KEY"));
        assert_eq!(manifest["auth"]["configured"], json!(true));
        assert_eq!(manifest["capability_flags"]["chat"], json!(true));
        assert_eq!(manifest["capability_flags"]["reasoning"], json!(true));
        assert_eq!(manifest["capability_flags"]["json_mode"], json!(true));
        assert_eq!(manifest["capability_flags"]["tool_calling"], json!(true));
        assert_eq!(manifest["timeout_ms"], json!(45000));
        assert_eq!(manifest["rate_limit"]["requests_per_minute"], json!(120));
        assert_eq!(
            manifest["cost"]["output_microusd_per_million_tokens"],
            json!(10000000)
        );
        assert!(!serialized.contains("sk-openai-secret"));
    }

    #[test]
    fn model_provider_profile_redacts_secret_like_minimax_shim_metadata() {
        let _guard = model_route_env_lock()
            .lock()
            .expect("model profile env lock");
        clear_model_profile_env("TEST_MINIMAX_PROFILE");
        std::env::set_var("TEST_MINIMAX_PROFILE_PROVIDER_ID", "minimax");
        std::env::set_var("TEST_MINIMAX_PROFILE_MODEL_ID", "MiniMax-M2.7");
        std::env::set_var(
            "TEST_MINIMAX_PROFILE_BASE_URL",
            "http://127.0.0.1:8999/v1?api_key=sk-base-secret",
        );
        std::env::set_var("TEST_MINIMAX_PROFILE_WIRE_API", "codex-compatible-shim");
        std::env::set_var(
            "TEST_MINIMAX_PROFILE_AUTH_ENV_KEY",
            "sk-minimax-direct-secret",
        );
        std::env::set_var(
            "TEST_MINIMAX_PROFILE_CAPABILITIES",
            "chat,vision,audio,video,json,tools,image_prompt,static_page,codex_compatible",
        );

        let profile = ModelProviderProfile::from_env("TEST_MINIMAX_PROFILE")
            .expect("minimax shim profile should parse");
        let manifest = profile.public_manifest();
        let serialized = manifest.to_string();
        let route = profile.to_model_route(MODEL_LANE_STATIC_PAGE_INTENT, 200);

        clear_model_profile_env("TEST_MINIMAX_PROFILE");
        assert_eq!(profile.wire_api, ModelProfileWireApi::CodexCompatibleShim);
        assert_eq!(manifest["auth"]["env_key_name"], json!(REDACTED_VALUE));
        assert_eq!(manifest["auth"]["configured"], json!(false));
        assert_eq!(manifest["capability_flags"]["vision"], json!(true));
        assert_eq!(manifest["capability_flags"]["audio"], json!(true));
        assert_eq!(manifest["capability_flags"]["video"], json!(true));
        assert_eq!(
            manifest["capability_flags"]["codex_compatible"],
            json!(true)
        );
        assert_eq!(route.provider, "minimax");
        assert_eq!(route.model, "MiniMax-M2.7");
        assert!(route
            .capability_class
            .contains(&"codex_compatible".to_string()));
        assert!(!serialized.contains("sk-base-secret"));
        assert!(!serialized.contains("sk-minimax-direct-secret"));
        assert!(serialized.contains(REDACTED_VALUE));
    }

    #[test]
    fn model_provider_profile_builds_safe_provider_shim_observability_snapshot() {
        let _guard = model_route_env_lock()
            .lock()
            .expect("model profile env lock");
        clear_model_profile_env("TEST_SHIM_OBSERVABILITY_PROFILE");
        std::env::set_var("MINIMAX_API_KEY", "sk-minimax-secret-value");
        std::env::set_var(
            "TEST_SHIM_OBSERVABILITY_PROFILE_PROFILE_ID",
            "minimax-private-experiment",
        );
        std::env::set_var("TEST_SHIM_OBSERVABILITY_PROFILE_PROVIDER_ID", "minimax");
        std::env::set_var("TEST_SHIM_OBSERVABILITY_PROFILE_MODEL_ID", "MiniMax-M2.7");
        std::env::set_var(
            "TEST_SHIM_OBSERVABILITY_PROFILE_BASE_URL",
            "http://127.0.0.1:8999/v1?api_key=sk-base-secret",
        );
        std::env::set_var("TEST_SHIM_OBSERVABILITY_PROFILE_API_PATH", "/v1/responses");
        std::env::set_var(
            "TEST_SHIM_OBSERVABILITY_PROFILE_WIRE_API",
            "codex_compatible_shim",
        );
        std::env::set_var(
            "TEST_SHIM_OBSERVABILITY_PROFILE_AUTH_ENV_KEY",
            "MINIMAX_API_KEY",
        );
        std::env::set_var(
            "TEST_SHIM_OBSERVABILITY_PROFILE_CAPABILITIES",
            "chat,json,tool_calling,codex_compatible",
        );
        std::env::set_var("TEST_SHIM_OBSERVABILITY_PROFILE_RATE_LIMIT_RPM", "30");

        let profile = ModelProviderProfile::from_env("TEST_SHIM_OBSERVABILITY_PROFILE")
            .expect("shim profile should parse");
        let snapshot = profile.provider_shim_observability_snapshot(ProviderShimHealthView {
            status: contracts::ProviderShimHealthStatusView::Healthy,
            process_reachable: true,
            upstream_reachable: true,
            checked_at: None,
            message: Some("ready".to_string()),
        });
        let serialized = serde_json::to_string(&snapshot).expect("snapshot should serialize");

        clear_model_profile_env("TEST_SHIM_OBSERVABILITY_PROFILE");
        std::env::remove_var("MINIMAX_API_KEY");
        assert_eq!(snapshot.schema_version, 1);
        assert_eq!(snapshot.profile.endpoint_scope, "local_private");
        assert_eq!(
            snapshot.profile.auth_env_key_name.as_deref(),
            Some("MINIMAX_API_KEY")
        );
        assert!(snapshot.profile.auth_configured);
        assert!(snapshot
            .profile
            .capabilities
            .contains(&"codex_compatible".to_string()));
        assert_eq!(snapshot.profile.rate_limit.requests_per_minute, Some(30));
        assert!(!serialized.contains("sk-minimax-secret-value"));
        assert!(!serialized.contains("sk-base-secret"));
        assert!(!serialized.contains("127.0.0.1:8999"));
        assert!(!serialized.contains("api_key"));
    }

    #[test]
    fn provider_runtime_metadata_builds_provider_shim_usage_events() {
        let responded_runtime = LlmRuntimeMetadata {
            mode: LlmRuntimeMode::Provider,
            provider: "minimax".to_string(),
            model: "MiniMax-M2.7".to_string(),
            lane: Some(MODEL_LANE_ASSISTANT_CHAT.to_string()),
            request_id: Some("req-ok".to_string()),
            finish_reason: Some(LlmFinishReason::Stop),
            provider_failure: None,
            latency_ms: Some(123),
            usage: Some(LlmTokenUsage {
                input_tokens: 100,
                output_tokens: 40,
                total_tokens: 140,
            }),
            system_prompt_key: None,
            system_prompt_version: None,
            tool_trace_count: 0,
        };
        let failed_runtime = LlmRuntimeMetadata {
            provider_failure: Some(LlmProviderFailure {
                kind: LlmProviderFailureKind::HttpStatus,
                message: "upstream returned HTTP 429".to_string(),
            }),
            request_id: Some("req-failed".to_string()),
            usage: None,
            ..responded_runtime.clone()
        };

        let ok_event = provider_shim_usage_event_from_runtime(
            &responded_runtime,
            Some("assistant-run-1"),
            None,
        );
        let failed_event =
            provider_shim_usage_event_from_runtime(&failed_runtime, None, Some("workflow-1"));
        let summary =
            provider_shim_usage_summary_from_events(&[ok_event.clone(), failed_event.clone()]);

        assert_eq!(ok_event.status, "responded");
        assert_eq!(
            ok_event.assistant_run_id.as_deref(),
            Some("assistant-run-1")
        );
        assert_eq!(ok_event.total_tokens, Some(140));
        assert_eq!(failed_event.status, "failed");
        assert_eq!(
            failed_event.provider_failure_kind.as_deref(),
            Some("http_status")
        );
        assert_eq!(
            failed_event.workflow_execution_id.as_deref(),
            Some("workflow-1")
        );
        assert_eq!(summary.request_count, 2);
        assert_eq!(summary.failed_request_count, 1);
        assert_eq!(summary.input_tokens, 100);
        assert_eq!(summary.output_tokens, 40);
        assert_eq!(summary.total_tokens, 140);
        assert_eq!(summary.last_request_id.as_deref(), Some("req-failed"));
    }

    #[test]
    fn provider_error_redacts_bearer_tokens() {
        let redacted =
            redact_provider_error("Authorization: Bearer secret-token-123 request failed");

        assert!(!redacted.contains("secret-token-123"));
        assert!(redacted.contains(REDACTED_VALUE));
    }

    #[test]
    fn provider_error_redacts_api_keys_cookies_and_user_paths() {
        let redacted = redact_provider_error(
            r#"body {"api_key":"sk-minimax-secret","access_token":"access-secret"} Cookie: session=secret; path C:\Users\soulzyn\.codex\secrets.env"#,
        );

        assert!(!redacted.contains("sk-minimax-secret"));
        assert!(!redacted.contains("access-secret"));
        assert!(!redacted.contains("session=secret"));
        assert!(!redacted.contains(r"C:\Users\soulzyn"));
        assert!(redacted.contains(REDACTED_VALUE));
        assert!(redacted.contains(REDACTED_PATH));
    }

    #[test]
    fn provider_error_truncates_large_body() {
        let redacted = redact_provider_error(&"x".repeat(10_000));

        assert!(redacted.len() < 1_000);
        assert!(redacted.ends_with("... [truncated]"));
    }

    #[test]
    fn placeholder_provider_echoes_input_and_runtime_metadata() {
        let provider = PlaceholderLlmProvider::new("placeholder")
            .with_prompt_registry(bootstrap_default_prompt_registry());
        let response = provider
            .complete(&LlmRequest {
                model: "placeholder-model-v1".to_string(),
                lane: Some(MODEL_LANE_CHAT_SESSION.to_string()),
                system_prompt_key: Some(CHAT_SESSION_PLACEHOLDER_PROMPT_KEY.to_string()),
                input: "hello world".to_string(),
            })
            .expect("placeholder provider should succeed");

        assert_eq!(response.output_text, "hello world");
        assert_eq!(response.runtime.mode, LlmRuntimeMode::Placeholder);
        assert_eq!(response.runtime.provider, "placeholder");
        assert_eq!(response.runtime.model, "placeholder-model-v1");
        assert_eq!(
            response.runtime.lane.as_deref(),
            Some(MODEL_LANE_CHAT_SESSION)
        );
        assert!(response
            .runtime
            .request_id
            .as_deref()
            .and_then(|value| Uuid::parse_str(value).ok())
            .is_some());
        assert_eq!(response.runtime.finish_reason, Some(LlmFinishReason::Stop));
        assert!(response.runtime.latency_ms.is_some());
        assert_eq!(
            response.runtime.usage,
            Some(LlmTokenUsage {
                input_tokens: 2,
                output_tokens: 2,
                total_tokens: 4,
            })
        );
        assert_eq!(
            response.runtime.system_prompt_key.as_deref(),
            Some(CHAT_SESSION_PLACEHOLDER_PROMPT_KEY)
        );
        assert_eq!(
            response.runtime.system_prompt_version.as_deref(),
            Some("v1")
        );
        assert_eq!(response.runtime.tool_trace_count, 0);
        assert!(response.tool_calls.is_empty());
    }

    #[test]
    fn scripted_provider_returns_provider_mode_runtime_and_tool_calls() {
        let provider = ScriptedLlmProvider::new("openai")
            .with_prompt_registry(bootstrap_default_prompt_registry())
            .with_response_text("provider reply")
            .with_request_id("req_provider_123")
            .with_finish_reason(LlmFinishReason::ToolCalls)
            .with_latency_ms(321)
            .with_tool_calls(vec![LlmToolCall {
                call_id: Some("call_123".to_string()),
                tool_name: "weather.lookup".to_string(),
                status: LlmToolCallStatus::Completed,
                arguments: Some(serde_json::json!({ "city": "Shanghai" })),
                result: Some(serde_json::json!({ "summary": "sunny" })),
            }]);
        let response = provider
            .complete(&LlmRequest {
                model: "gpt-5.4".to_string(),
                lane: Some(MODEL_LANE_CHAT_SESSION.to_string()),
                system_prompt_key: Some(CHAT_SESSION_PLACEHOLDER_PROMPT_KEY.to_string()),
                input: "hello world".to_string(),
            })
            .expect("scripted provider should succeed");

        assert_eq!(response.output_text, "provider reply");
        assert_eq!(response.runtime.mode, LlmRuntimeMode::Provider);
        assert_eq!(response.runtime.provider, "openai");
        assert_eq!(response.runtime.model, "gpt-5.4");
        assert_eq!(
            response.runtime.lane.as_deref(),
            Some(MODEL_LANE_CHAT_SESSION)
        );
        assert_eq!(
            response.runtime.request_id.as_deref(),
            Some("req_provider_123")
        );
        assert_eq!(
            response.runtime.finish_reason,
            Some(LlmFinishReason::ToolCalls)
        );
        assert_eq!(response.runtime.latency_ms, Some(321));
        assert_eq!(
            response.runtime.usage,
            Some(LlmTokenUsage {
                input_tokens: 2,
                output_tokens: 2,
                total_tokens: 4,
            })
        );
        assert_eq!(response.runtime.tool_trace_count, 1);
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].tool_name, "weather.lookup");
    }

    #[test]
    fn openai_compatible_provider_parses_chat_completion_response() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let addr = listener.local_addr().expect("addr");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut request_bytes = Vec::new();
            let mut buffer = [0_u8; 1024];
            let mut expected_len = None;
            loop {
                let read = stream.read(&mut buffer).expect("read");
                if read == 0 {
                    break;
                }
                request_bytes.extend_from_slice(&buffer[..read]);
                let request = String::from_utf8_lossy(&request_bytes);
                if expected_len.is_none() {
                    expected_len = request.split("\r\n").find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        if name.eq_ignore_ascii_case("content-length") {
                            value.trim().parse::<usize>().ok()
                        } else {
                            None
                        }
                    });
                }
                if let Some(headers_end) = request.find("\r\n\r\n") {
                    let body_len = request_bytes.len() - (headers_end + 4);
                    if body_len >= expected_len.unwrap_or(0) {
                        break;
                    }
                }
            }
            let request = String::from_utf8_lossy(&request_bytes);
            assert!(request.contains("POST /v1/chat/completions HTTP/1.1"));
            assert!(
                request.contains("Authorization: Bearer test-key")
                    || request.contains("authorization: Bearer test-key")
            );
            assert!(request.contains("Placeholder chat session system prompt"));
            assert!(request.contains("\"model\":\"gpt-5.4-mini\""));
            let body = json!({
                "id": "chatcmpl_live_123",
                "choices": [{
                    "finish_reason": "tool_calls",
                    "message": {
                        "content": "live provider reply",
                        "tool_calls": [{
                            "id": "call_live_123",
                            "function": {
                                "name": "weather.lookup",
                                "arguments": "{\"city\":\"Shanghai\"}"
                            }
                        }]
                    }
                }],
                "usage": {
                    "prompt_tokens": 11,
                    "completion_tokens": 13,
                    "total_tokens": 24
                }
            })
            .to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).expect("write");
        });

        let provider = OpenAiCompatibleLlmProvider::new(
            "openai",
            OpenAiCompatibleLlmProviderConfig {
                api_base_url: format!("http://{addr}"),
                api_path: "/v1/chat/completions".to_string(),
                api_key: Some("test-key".to_string()),
            },
        )
        .expect("provider")
        .with_prompt_registry(bootstrap_default_prompt_registry());
        let response = provider
            .complete(&LlmRequest {
                model: "gpt-5.4-mini".to_string(),
                lane: Some(MODEL_LANE_CHAT_SESSION.to_string()),
                system_prompt_key: Some(CHAT_SESSION_PLACEHOLDER_PROMPT_KEY.to_string()),
                input: "What is the weather?".to_string(),
            })
            .expect("live provider should succeed");

        server.join().expect("server join");

        assert_eq!(response.output_text, "live provider reply");
        assert_eq!(response.runtime.mode, LlmRuntimeMode::Provider);
        assert_eq!(response.runtime.provider, "openai");
        assert_eq!(
            response.runtime.request_id.as_deref(),
            Some("chatcmpl_live_123")
        );
        assert_eq!(
            response.runtime.lane.as_deref(),
            Some(MODEL_LANE_CHAT_SESSION)
        );
        assert_eq!(
            response.runtime.finish_reason,
            Some(LlmFinishReason::ToolCalls)
        );
        assert_eq!(
            response.runtime.usage,
            Some(LlmTokenUsage {
                input_tokens: 11,
                output_tokens: 13,
                total_tokens: 24,
            })
        );
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].status, LlmToolCallStatus::Requested);
        assert_eq!(response.tool_calls[0].tool_name, "weather.lookup");
        assert_eq!(
            response.tool_calls[0].arguments,
            Some(json!({ "city": "Shanghai" }))
        );
    }

    #[test]
    fn chat_completion_tool_call_extraction_rejects_incomplete_tool_request() {
        let choice = json!({
            "message": {
                "tool_calls": [{
                    "id": "call_missing_name",
                    "type": "function",
                    "function": {
                        "arguments": "{\"city\":\"Chengdu\"}"
                    }
                }]
            }
        });

        let error = extract_chat_completion_tool_calls(&choice)
            .expect_err("missing function name should be a protocol error");

        assert!(error
            .to_string()
            .contains("tool_call.function.name missing"));
    }

    #[test]
    fn openai_compatible_provider_strips_leading_reasoning_blocks() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let addr = listener.local_addr().expect("addr");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let _request = read_http_request(&mut stream);
            write_http_json_response(
                &mut stream,
                200,
                r#"{
                    "id": "chatcmpl_minimax_123",
                    "choices": [{
                        "finish_reason": "stop",
                        "message": {
                            "content": "<think>private reasoning must not enter the user answer</think>\n\nMINIMAX_SMOKE_OK"
                        }
                    }],
                    "usage": {
                        "prompt_tokens": 4,
                        "completion_tokens": 9,
                        "total_tokens": 13
                    }
                }"#,
            );
        });

        let provider = OpenAiCompatibleLlmProvider::new(
            "minimax_openai_compatible",
            OpenAiCompatibleLlmProviderConfig {
                api_base_url: format!("http://{addr}"),
                api_path: "/v1/chat/completions".to_string(),
                api_key: Some("test-key".to_string()),
            },
        )
        .expect("provider");
        let response = provider
            .complete(&LlmRequest {
                model: "MiniMax-M2.7".to_string(),
                lane: Some(MODEL_LANE_ASSISTANT_CHAT.to_string()),
                system_prompt_key: None,
                input: "Reply exactly MINIMAX_SMOKE_OK".to_string(),
            })
            .expect("minimax-compatible provider should succeed");

        server.join().expect("server join");
        assert_eq!(response.output_text, "MINIMAX_SMOKE_OK");
        assert_eq!(
            response.runtime.request_id.as_deref(),
            Some("chatcmpl_minimax_123")
        );
    }

    #[test]
    fn output_normalization_only_strips_leading_reasoning_blocks() {
        assert_eq!(
            normalize_provider_output_text("<think>draft</think>\n\nFinal answer"),
            "Final answer"
        );
        assert_eq!(
            normalize_provider_output_text("Keep inline <think>literal</think> text"),
            "Keep inline <think>literal</think> text"
        );
        assert_eq!(
            normalize_provider_output_text("<think>first</think>\n<think>second</think>\nAnswer"),
            "Answer"
        );
    }

    #[test]
    fn openai_compatible_provider_redacts_http_error_body() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let addr = listener.local_addr().expect("addr");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let _request = read_http_request(&mut stream);
            write_http_json_response(
                &mut stream,
                401,
                r#"{"error":"bad key","api_key":"sk-live-secret","cookie":"sid=secret"}"#,
            );
        });

        let provider = OpenAiCompatibleLlmProvider::new(
            "minimax_openai_compatible",
            OpenAiCompatibleLlmProviderConfig {
                api_base_url: format!("http://{addr}"),
                api_path: "/v1/chat/completions".to_string(),
                api_key: Some("client-secret-key".to_string()),
            },
        )
        .expect("provider");
        let error = provider
            .complete(&LlmRequest {
                model: "abab7.0-chat".to_string(),
                lane: Some(MODEL_LANE_DOCUMENT_VLM.to_string()),
                system_prompt_key: None,
                input: "Summarize document".to_string(),
            })
            .expect_err("HTTP failure should be reported");

        server.join().expect("server join");
        let message = error.to_string();
        let provider_error = error
            .downcast_ref::<LlmProviderError>()
            .expect("provider error");
        let failure_message = provider_error
            .runtime()
            .provider_failure
            .as_ref()
            .map(|failure| failure.message.as_str())
            .expect("provider failure");
        assert!(!message.contains("sk-live-secret"));
        assert!(!message.contains("sid=secret"));
        assert!(!failure_message.contains("sk-live-secret"));
        assert!(!failure_message.contains("sid=secret"));
        assert_eq!(
            provider_error.runtime().lane.as_deref(),
            Some(MODEL_LANE_DOCUMENT_VLM)
        );
    }

    #[test]
    fn openclaw_provider_builds_from_global_env_when_enabled() {
        let _guard = openclaw_env_lock().lock().expect("openclaw env lock");
        clear_openclaw_env();
        std::env::set_var("OPENCLAW_EXTENSION_ENABLED", "true");
        std::env::set_var("OPENCLAW_GATEWAY_BASE_URL", "http://127.0.0.1:65530");
        std::env::set_var("OPENCLAW_GATEWAY_TOKEN", "test-openclaw-token");

        let provider = build_provider_from_env(
            "ASSISTANT_RUN",
            "provider",
            "openclaw",
            bootstrap_default_prompt_registry(),
        )
        .expect("openclaw provider should build from global env");

        assert_eq!(provider.name(), "openclaw");
        clear_openclaw_env();
    }

    #[test]
    fn openclaw_provider_requires_base_url_and_token_by_default() {
        let _guard = openclaw_env_lock().lock().expect("openclaw env lock");
        clear_openclaw_env();
        std::env::set_var("OPENCLAW_EXTENSION_ENABLED", "true");

        let missing_base = build_provider_from_env(
            "ASSISTANT_RUN",
            "provider",
            "openclaw",
            bootstrap_default_prompt_registry(),
        )
        .expect_err("missing base url should fail");
        assert!(missing_base
            .to_string()
            .contains("OPENCLAW_GATEWAY_BASE_URL"));

        std::env::set_var("OPENCLAW_GATEWAY_BASE_URL", "http://127.0.0.1:65530");
        let missing_token = build_provider_from_env(
            "ASSISTANT_RUN",
            "provider",
            "openclaw",
            bootstrap_default_prompt_registry(),
        )
        .expect_err("missing token should fail by default");
        assert!(missing_token.to_string().contains("OPENCLAW_GATEWAY_TOKEN"));

        std::env::set_var("OPENCLAW_GATEWAY_TOKEN_OPTIONAL", "true");
        let provider = build_provider_from_env(
            "ASSISTANT_RUN",
            "provider",
            "openclaw",
            bootstrap_default_prompt_registry(),
        )
        .expect("token optional should allow provider construction");
        assert_eq!(provider.name(), "openclaw");
        clear_openclaw_env();
    }

    #[test]
    fn openclaw_provider_prefers_responses_endpoint_and_parses_output_text() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let addr = listener.local_addr().expect("addr");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let request = read_http_request(&mut stream);
            assert!(request.contains("POST /v1/responses HTTP/1.1"));
            assert!(
                request.contains("Authorization: Bearer test-openclaw-token")
                    || request.contains("authorization: Bearer test-openclaw-token")
            );
            assert!(
                request.contains("x-openclaw-agent-id: agent-test")
                    || request.contains("X-Openclaw-Agent-Id: agent-test")
                    || request.contains("X-OpenClaw-Agent-Id: agent-test")
            );
            assert!(
                request.contains("x-openclaw-model: override-model")
                    || request.contains("X-Openclaw-Model: override-model")
                    || request.contains("X-OpenClaw-Model: override-model")
            );
            assert!(request.contains("\"model\":\"openclaw-model\""));
            assert!(request.contains("\"input\":\"Explain revenue\""));
            let body = json!({
                "id": "resp_openclaw_1",
                "output": [{
                    "type": "message",
                    "content": [{
                        "type": "output_text",
                        "text": "OpenClaw answer"
                    }]
                }],
                "usage": {
                    "input_tokens": 10,
                    "output_tokens": 4,
                    "total_tokens": 14
                }
            })
            .to_string();
            write_http_json_response(&mut stream, 200, &body);
        });

        let provider = OpenClawLlmProvider::new(
            "openclaw",
            OpenClawLlmProviderConfig {
                gateway_base_url: format!("http://{addr}"),
                token: Some("test-openclaw-token".to_string()),
                agent_id: Some("agent-test".to_string()),
                model: None,
                model_override: Some("override-model".to_string()),
                prefer_responses: true,
                timeout_ms: 60_000,
            },
        )
        .expect("provider");
        let response = provider
            .complete(&LlmRequest {
                model: "openclaw-model".to_string(),
                lane: Some(MODEL_LANE_ASSISTANT_CHAT.to_string()),
                system_prompt_key: None,
                input: "Explain revenue".to_string(),
            })
            .expect("openclaw responses call should succeed");

        server.join().expect("server join");
        assert_eq!(response.output_text, "OpenClaw answer");
        assert_eq!(response.runtime.mode, LlmRuntimeMode::Provider);
        assert_eq!(response.runtime.provider, "openclaw");
        assert_eq!(response.runtime.model, "openclaw-model");
        assert_eq!(
            response.runtime.lane.as_deref(),
            Some(MODEL_LANE_ASSISTANT_CHAT)
        );
        assert_eq!(
            response.runtime.request_id.as_deref(),
            Some("resp_openclaw_1")
        );
        assert_eq!(response.runtime.finish_reason, Some(LlmFinishReason::Stop));
        assert_eq!(
            response.runtime.usage,
            Some(LlmTokenUsage {
                input_tokens: 10,
                output_tokens: 4,
                total_tokens: 14,
            })
        );
    }

    #[test]
    fn openclaw_provider_falls_back_to_chat_completions() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let addr = listener.local_addr().expect("addr");
        let server = thread::spawn(move || {
            let (mut responses_stream, _) = listener.accept().expect("accept responses");
            let responses_request = read_http_request(&mut responses_stream);
            assert!(responses_request.contains("POST /v1/responses HTTP/1.1"));
            write_http_json_response(
                &mut responses_stream,
                404,
                "{\"error\":\"responses unavailable\"}",
            );

            let (mut chat_stream, _) = listener.accept().expect("accept chat");
            let chat_request = read_http_request(&mut chat_stream);
            assert!(chat_request.contains("POST /v1/chat/completions HTTP/1.1"));
            assert!(chat_request.contains("\"messages\""));
            let body = json!({
                "id": "chatcmpl_openclaw_1",
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": "Fallback answer"
                    },
                    "finish_reason": "stop"
                }],
                "usage": {
                    "prompt_tokens": 8,
                    "completion_tokens": 3,
                    "total_tokens": 11
                }
            })
            .to_string();
            write_http_json_response(&mut chat_stream, 200, &body);
        });

        let provider = OpenClawLlmProvider::new(
            "openclaw",
            OpenClawLlmProviderConfig {
                gateway_base_url: format!("http://{addr}"),
                token: Some("test-openclaw-token".to_string()),
                agent_id: None,
                model: None,
                model_override: None,
                prefer_responses: true,
                timeout_ms: 60_000,
            },
        )
        .expect("provider");
        let response = provider
            .complete(&LlmRequest {
                model: "openclaw-model".to_string(),
                lane: Some(MODEL_LANE_ASSISTANT_CHAT.to_string()),
                system_prompt_key: None,
                input: "Fallback please".to_string(),
            })
            .expect("openclaw chat fallback should succeed");

        server.join().expect("server join");
        assert_eq!(response.output_text, "Fallback answer");
        assert_eq!(response.runtime.provider, "openclaw");
        assert_eq!(
            response.runtime.lane.as_deref(),
            Some(MODEL_LANE_ASSISTANT_CHAT)
        );
        assert_eq!(
            response.runtime.request_id.as_deref(),
            Some("chatcmpl_openclaw_1")
        );
        assert_eq!(response.runtime.finish_reason, Some(LlmFinishReason::Stop));
        assert_eq!(
            response.runtime.usage,
            Some(LlmTokenUsage {
                input_tokens: 8,
                output_tokens: 3,
                total_tokens: 11,
            })
        );
    }

    #[test]
    fn openclaw_retry_detectors_match_bad_outputs() {
        assert!(looks_like_openclaw_onboarding_drift(
            "我是刚启动的智能助手，还没有名字，你可以给我起名。"
        ));
        assert!(looks_like_openclaw_leaked_tool_call_content(
            "{\"tool_calls\":[{\"function\":{\"name\":\"search\"}}]}"
        ));
        assert!(looks_like_openclaw_native_tool_failure(
            "Cannot read properties of undefined (reading 'input')"
        ));
        assert!(!looks_like_openclaw_retryable_bad_output(
            "这里是根据当前数据整理出的收入结论。"
        ));
    }

    #[test]
    fn openclaw_provider_retries_on_onboarding_drift() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let addr = listener.local_addr().expect("addr");
        let server = thread::spawn(move || {
            let (mut first_stream, _) = listener.accept().expect("accept first");
            let first_request = read_http_request(&mut first_stream);
            assert!(first_request.contains("POST /v1/responses HTTP/1.1"));
            let first_body = json!({
                "id": "resp_openclaw_bad",
                "output_text": "我是刚启动的智能助手，还没有名字，你可以给我起名。"
            })
            .to_string();
            write_http_json_response(&mut first_stream, 200, &first_body);

            let (mut retry_stream, _) = listener.accept().expect("accept retry");
            let retry_request = read_http_request(&mut retry_stream);
            assert!(retry_request.contains("POST /v1/responses HTTP/1.1"));
            assert!(retry_request.contains("直接回答用户当前问题"));
            let retry_body = json!({
                "id": "resp_openclaw_retry",
                "output_text": "这是重试后的有效回答。"
            })
            .to_string();
            write_http_json_response(&mut retry_stream, 200, &retry_body);
        });

        let provider = OpenClawLlmProvider::new(
            "openclaw",
            OpenClawLlmProviderConfig {
                gateway_base_url: format!("http://{addr}"),
                token: Some("test-openclaw-token".to_string()),
                agent_id: None,
                model: None,
                model_override: None,
                prefer_responses: true,
                timeout_ms: 60_000,
            },
        )
        .expect("provider");
        let response = provider
            .complete(&LlmRequest {
                model: "openclaw-model".to_string(),
                lane: Some(MODEL_LANE_ASSISTANT_CHAT.to_string()),
                system_prompt_key: None,
                input: "回答当前经营问题".to_string(),
            })
            .expect("retry should recover bad openclaw output");

        server.join().expect("server join");
        assert_eq!(response.output_text, "这是重试后的有效回答。");
        assert_eq!(
            response.runtime.request_id.as_deref(),
            Some("resp_openclaw_retry")
        );
    }

    #[test]
    fn render_runtime_manifest_serializes_runtime_metadata() {
        let manifest = render_runtime_manifest(&LlmRuntimeMetadata {
            mode: LlmRuntimeMode::Provider,
            provider: "openai".to_string(),
            model: "gpt-5.4".to_string(),
            lane: Some(MODEL_LANE_ASSISTANT_REACT_JSON.to_string()),
            request_id: Some("req_runtime_manifest".to_string()),
            finish_reason: Some(LlmFinishReason::ToolCalls),
            provider_failure: None,
            latency_ms: Some(123),
            usage: Some(LlmTokenUsage {
                input_tokens: 7,
                output_tokens: 5,
                total_tokens: 12,
            }),
            system_prompt_key: Some(CHAT_SESSION_PLACEHOLDER_PROMPT_KEY.to_string()),
            system_prompt_version: Some("v1".to_string()),
            tool_trace_count: 2,
        });

        assert_eq!(manifest["mode"], json!("provider"));
        assert_eq!(manifest["provider"], json!("openai"));
        assert_eq!(manifest["model"], json!("gpt-5.4"));
        assert_eq!(manifest["lane"], json!(MODEL_LANE_ASSISTANT_REACT_JSON));
        assert_eq!(manifest["request_id"], json!("req_runtime_manifest"));
        assert_eq!(manifest["finish_reason"], json!("tool_calls"));
        assert!(manifest["provider_failure"].is_null());
        assert_eq!(manifest["latency_ms"], json!(123));
        assert_eq!(manifest["usage"]["total_tokens"], json!(12));
        assert_eq!(
            manifest["system_prompt_key"],
            json!(CHAT_SESSION_PLACEHOLDER_PROMPT_KEY)
        );
        assert_eq!(manifest["system_prompt_version"], json!("v1"));
        assert_eq!(manifest["tool_trace_count"], json!(2));
    }

    fn openclaw_env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn model_route_env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn clear_model_route_env(lane: &str) {
        let prefix = model_route_env_prefix(lane);
        for suffix in ["PROVIDER", "MODEL", "CAPABILITIES", "PRIORITY", "FALLBACK"] {
            std::env::remove_var(format!("{prefix}_{suffix}"));
        }
    }

    fn clear_model_profile_env(prefix: &str) {
        for suffix in [
            "PROFILE_ID",
            "PROVIDER_ID",
            "MODEL_ID",
            "BASE_URL",
            "API_PATH",
            "WIRE_API",
            "AUTH_ENV_KEY",
            "CAPABILITIES",
            "TIMEOUT_MS",
            "RATE_LIMIT_RPM",
            "RATE_LIMIT_TPM",
            "RATE_LIMIT_CONCURRENCY",
            "COST_INPUT_MICROUSD_PER_MILLION_TOKENS",
            "COST_OUTPUT_MICROUSD_PER_MILLION_TOKENS",
            "COST_CURRENCY",
            "REDACT_PROVIDER_ERRORS",
            "REDACT_REQUEST_PAYLOADS",
            "REDACT_RESPONSE_PAYLOADS",
            "MAX_ERROR_CHARS",
        ] {
            std::env::remove_var(format!("{prefix}_{suffix}"));
        }
    }

    fn clear_openclaw_env() {
        for key in [
            "OPENCLAW_EXTENSION_ENABLED",
            "OPENCLAW_GATEWAY_BASE_URL",
            "OPENCLAW_GATEWAY_TOKEN",
            "OPENCLAW_GATEWAY_TOKEN_OPTIONAL",
            "OPENCLAW_AGENT_ID",
            "OPENCLAW_MODEL",
            "OPENCLAW_MODEL_OVERRIDE",
            "OPENCLAW_PREFER_RESPONSES",
            "OPENCLAW_TIMEOUT_MS",
        ] {
            std::env::remove_var(key);
        }
    }

    fn read_http_request(stream: &mut TcpStream) -> String {
        let mut request_bytes = Vec::new();
        let mut buffer = [0_u8; 1024];
        let mut expected_len = None;
        loop {
            let read = stream.read(&mut buffer).expect("read");
            if read == 0 {
                break;
            }
            request_bytes.extend_from_slice(&buffer[..read]);
            let request = String::from_utf8_lossy(&request_bytes);
            if expected_len.is_none() {
                expected_len = request.split("\r\n").find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    if name.eq_ignore_ascii_case("content-length") {
                        value.trim().parse::<usize>().ok()
                    } else {
                        None
                    }
                });
            }
            if let Some(headers_end) = request.find("\r\n\r\n") {
                let body_len = request_bytes.len() - (headers_end + 4);
                if body_len >= expected_len.unwrap_or(0) {
                    break;
                }
            }
        }
        String::from_utf8_lossy(&request_bytes).to_string()
    }

    fn write_http_json_response(stream: &mut TcpStream, status: u16, body: &str) {
        let reason = match status {
            200 => "OK",
            404 => "Not Found",
            500 => "Internal Server Error",
            _ => "OK",
        };
        let response = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).expect("write");
    }
}
