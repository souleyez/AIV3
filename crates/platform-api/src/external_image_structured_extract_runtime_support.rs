use serde_json::{json, Value};

use crate::{
    assistant_run_text_support::truncate_assistant_supply_text,
    text_normalization::non_empty_trimmed_string,
};

#[derive(Clone, Debug)]
pub(crate) struct ExternalImageStructuredExtractRuntimeConfig {
    pub(crate) endpoint_url: String,
    pub(crate) api_key: String,
    pub(crate) model: String,
    pub(crate) timeout_ms: u64,
    pub(crate) max_tokens: u64,
    pub(crate) reasoning_effort: Option<String>,
    pub(crate) retry_enabled: bool,
    pub(crate) provider_label: String,
}

#[derive(Debug)]
pub(crate) struct ExternalImageStructuredExtractFailure {
    pub(crate) reason: String,
    pub(crate) runtime: Value,
}

pub(crate) fn external_image_structured_extract_runtime_config(
) -> Option<ExternalImageStructuredExtractRuntimeConfig> {
    let base_url = external_image_structured_extract_env_value(&[
        "EXTERNAL_IMAGE_STRUCTURED_EXTRACT_BASE_URL",
        "ASSISTANT_RUN_RUNTIME_BASE_URL",
    ])?;
    let api_path = external_image_structured_extract_env_value(&[
        "EXTERNAL_IMAGE_STRUCTURED_EXTRACT_API_PATH",
        "ASSISTANT_RUN_RUNTIME_API_PATH",
    ])
    .unwrap_or_else(|| "/v1/chat/completions".to_string());
    let api_key = external_image_structured_extract_env_value(&[
        "EXTERNAL_IMAGE_STRUCTURED_EXTRACT_API_KEY",
        "ASSISTANT_RUN_RUNTIME_API_KEY",
    ])?;
    let model = external_image_structured_extract_env_value(&[
        "EXTERNAL_IMAGE_STRUCTURED_EXTRACT_MODEL",
        "ASSISTANT_RUN_RUNTIME_MODEL",
    ])
    .unwrap_or_else(|| "gpt-5.5".to_string());
    let timeout_ms = external_image_structured_extract_env_value(&[
        "EXTERNAL_IMAGE_STRUCTURED_EXTRACT_TIMEOUT_MS",
    ])
    .and_then(|value| value.parse::<u64>().ok())
    .filter(|value| *value >= 1_000)
    .unwrap_or(60_000);
    let max_tokens = external_image_structured_extract_env_value(&[
        "EXTERNAL_IMAGE_STRUCTURED_EXTRACT_MAX_TOKENS",
    ])
    .and_then(|value| value.parse::<u64>().ok())
    .filter(|value| (512..=16_000).contains(value))
    .unwrap_or(4_096);
    let reasoning_effort = external_image_structured_extract_env_value(&[
        "EXTERNAL_IMAGE_STRUCTURED_EXTRACT_REASONING_EFFORT",
        "ASSISTANT_RUN_RUNTIME_REASONING_EFFORT",
    ])
    .map(|value| value.trim().to_ascii_lowercase())
    .filter(|value| {
        matches!(
            value.as_str(),
            "none" | "minimal" | "low" | "medium" | "high" | "xhigh"
        )
    });
    let retry_enabled = external_image_structured_extract_env_value(&[
        "EXTERNAL_IMAGE_STRUCTURED_EXTRACT_RETRY_ENABLED",
    ])
    .map(|value| {
        !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "0" | "false" | "off"
        )
    })
    .unwrap_or(true);
    Some(ExternalImageStructuredExtractRuntimeConfig {
        endpoint_url: external_image_structured_extract_join_url(&base_url, &api_path),
        api_key,
        model,
        timeout_ms,
        max_tokens,
        reasoning_effort,
        retry_enabled,
        provider_label: external_image_structured_extract_env_value(&[
            "EXTERNAL_IMAGE_STRUCTURED_EXTRACT_PROVIDER",
            "ASSISTANT_RUN_RUNTIME_PROVIDER",
        ])
        .unwrap_or_else(|| "openai_compatible_vision".to_string()),
    })
}

pub(crate) fn external_image_structured_extract_env_value(keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        std::env::var(key)
            .ok()
            .and_then(|value| non_empty_trimmed_string(&value))
    })
}

pub(crate) fn external_image_structured_extract_join_url(base_url: &str, api_path: &str) -> String {
    if api_path.starts_with("http://") || api_path.starts_with("https://") {
        return api_path.to_string();
    }
    format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        api_path.trim_start_matches('/')
    )
}

pub(crate) fn external_image_structured_extract_failure_runtime(
    config: &ExternalImageStructuredExtractRuntimeConfig,
    kind: &str,
    message: &str,
    latency_ms: u64,
) -> Value {
    json!({
        "mode": "external_image_structured_extract",
        "lane": "external_channel",
        "provider": config.provider_label,
        "model": config.model,
        "latency_ms": latency_ms,
        "provider_failure": {
            "kind": kind,
            "message": truncate_assistant_supply_text(message, 800),
        },
    })
}

pub(crate) fn external_image_structured_extract_chat_response_text(
    response: &Value,
) -> Option<String> {
    response
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(|content| match content {
            Value::String(text) => non_empty_trimmed_string(text),
            Value::Array(parts) => {
                let text = parts
                    .iter()
                    .filter_map(|part| part.get("text").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("\n");
                non_empty_trimmed_string(&text)
            }
            other if other.is_object() || other.is_array() => Some(other.to_string()),
            _ => None,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> ExternalImageStructuredExtractRuntimeConfig {
        ExternalImageStructuredExtractRuntimeConfig {
            endpoint_url: "https://model.example.test/v1/chat/completions".to_string(),
            api_key: "test-key".to_string(),
            model: "gpt-5.5".to_string(),
            timeout_ms: 60_000,
            max_tokens: 4_096,
            reasoning_effort: Some("medium".to_string()),
            retry_enabled: true,
            provider_label: "test-provider".to_string(),
        }
    }

    #[test]
    fn join_url_keeps_absolute_path_or_combines_base_and_path() {
        assert_eq!(
            external_image_structured_extract_join_url(
                "https://model.example.test/base/",
                "/v1/chat/completions"
            ),
            "https://model.example.test/base/v1/chat/completions"
        );
        assert_eq!(
            external_image_structured_extract_join_url(
                "https://model.example.test",
                "https://proxy.example.test/chat"
            ),
            "https://proxy.example.test/chat"
        );
    }

    #[test]
    fn failure_runtime_keeps_provider_model_latency_and_truncates_message() {
        let runtime = external_image_structured_extract_failure_runtime(
            &test_config(),
            "provider_error",
            &"x".repeat(1_200),
            321,
        );

        assert_eq!(runtime["mode"], json!("external_image_structured_extract"));
        assert_eq!(runtime["lane"], json!("external_channel"));
        assert_eq!(runtime["provider"], json!("test-provider"));
        assert_eq!(runtime["model"], json!("gpt-5.5"));
        assert_eq!(runtime["latency_ms"], json!(321));
        assert_eq!(runtime["provider_failure"]["kind"], json!("provider_error"));
        assert!(
            runtime["provider_failure"]["message"]
                .as_str()
                .unwrap_or_default()
                .chars()
                .count()
                <= 800
        );
    }

    #[test]
    fn chat_response_text_extracts_string_parts_or_json_content() {
        assert_eq!(
            external_image_structured_extract_chat_response_text(&json!({
                "choices": [{ "message": { "content": "  {\"ok\":true}  " } }]
            }))
            .as_deref(),
            Some("{\"ok\":true}")
        );
        assert_eq!(
            external_image_structured_extract_chat_response_text(&json!({
                "choices": [{
                    "message": {
                        "content": [
                            { "type": "text", "text": "line one" },
                            { "type": "text", "text": "line two" },
                            { "type": "image_url", "image_url": { "url": "ignored" } }
                        ]
                    }
                }]
            }))
            .as_deref(),
            Some("line one\nline two")
        );
        assert_eq!(
            external_image_structured_extract_chat_response_text(&json!({
                "choices": [{ "message": { "content": { "records": [] } } }]
            }))
            .as_deref(),
            Some("{\"records\":[]}")
        );
        assert!(
            external_image_structured_extract_chat_response_text(&json!({
                "choices": [{ "message": { "content": "   " } }]
            }))
            .is_none()
        );
    }
}
