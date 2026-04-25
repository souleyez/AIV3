use anyhow::{anyhow, Context, Result};
use prompt_registry::InMemoryPromptRegistry;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt::{self, Debug, Display};
use std::sync::Arc;
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

#[derive(Clone, Debug)]
pub struct OpenAiCompatibleLlmProviderConfig {
    pub api_base_url: String,
    pub api_path: String,
    pub api_key: Option<String>,
}

pub fn render_runtime_manifest(runtime: &LlmRuntimeMetadata) -> Value {
    json!({
        "mode": runtime.mode.as_str(),
        "provider": runtime.provider.as_str(),
        "model": runtime.model.as_str(),
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
        let runtime = LlmRuntimeMetadata {
            mode: LlmRuntimeMode::Provider,
            provider: self.provider_name.clone(),
            model: request.model.clone(),
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
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn placeholder_provider_echoes_input_and_runtime_metadata() {
        let provider = PlaceholderLlmProvider::new("placeholder")
            .with_prompt_registry(bootstrap_default_prompt_registry());
        let response = provider
            .complete(&LlmRequest {
                model: "placeholder-model-v1".to_string(),
                system_prompt_key: Some(CHAT_SESSION_PLACEHOLDER_PROMPT_KEY.to_string()),
                input: "hello world".to_string(),
            })
            .expect("placeholder provider should succeed");

        assert_eq!(response.output_text, "hello world");
        assert_eq!(response.runtime.mode, LlmRuntimeMode::Placeholder);
        assert_eq!(response.runtime.provider, "placeholder");
        assert_eq!(response.runtime.model, "placeholder-model-v1");
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
                system_prompt_key: Some(CHAT_SESSION_PLACEHOLDER_PROMPT_KEY.to_string()),
                input: "hello world".to_string(),
            })
            .expect("scripted provider should succeed");

        assert_eq!(response.output_text, "provider reply");
        assert_eq!(response.runtime.mode, LlmRuntimeMode::Provider);
        assert_eq!(response.runtime.provider, "openai");
        assert_eq!(response.runtime.model, "gpt-5.4");
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
    fn render_runtime_manifest_serializes_runtime_metadata() {
        let manifest = render_runtime_manifest(&LlmRuntimeMetadata {
            mode: LlmRuntimeMode::Provider,
            provider: "openai".to_string(),
            model: "gpt-5.4".to_string(),
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
}
