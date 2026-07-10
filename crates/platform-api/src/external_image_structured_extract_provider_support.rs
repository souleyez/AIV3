use std::time::Instant;

use serde_json::{json, Value};

use crate::{
    assistant_run_text_support::truncate_assistant_supply_text,
    external_image_structured_extract_runtime_support::{
        external_image_structured_extract_chat_response_text,
        external_image_structured_extract_failure_runtime, ExternalImageStructuredExtractFailure,
        ExternalImageStructuredExtractRuntimeConfig,
    },
};

const EXTERNAL_IMAGE_STRUCTURED_EXTRACT_SYSTEM_PROMPT: &str = "你是业务截图表格结构化抽取器。只输出 JSON，不要输出解释。必须先识别表头，再按图片从上到下逐行抽取订单/充值/支付记录。无法确定的字段填 null，不要编造，不要复用历史结果。";

pub(crate) fn external_image_structured_extract_request_payload(
    config: &ExternalImageStructuredExtractRuntimeConfig,
    image_url: &str,
    prompt_text: String,
) -> Value {
    external_image_structured_extract_request_payload_with_system(
        config,
        image_url,
        EXTERNAL_IMAGE_STRUCTURED_EXTRACT_SYSTEM_PROMPT,
        prompt_text,
    )
}

pub(crate) fn external_image_structured_extract_request_payload_with_system(
    config: &ExternalImageStructuredExtractRuntimeConfig,
    image_url: &str,
    system_prompt: &str,
    prompt_text: String,
) -> Value {
    let mut payload = json!({
        "model": config.model,
        "messages": [
            {
                "role": "system",
                "content": system_prompt
            },
            {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": prompt_text
                    },
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": image_url,
                            "detail": "high"
                        }
                    }
                ]
            }
        ],
        "response_format": { "type": "json_object" },
        "temperature": 0,
        "max_tokens": config.max_tokens
    });
    if let Some(effort) = config.reasoning_effort.as_deref() {
        payload["reasoning"] = json!({ "effort": effort });
    }
    payload
}

pub(crate) async fn external_image_structured_extract_call_provider(
    client: &reqwest::Client,
    config: &ExternalImageStructuredExtractRuntimeConfig,
    image_url: &str,
    prompt_text: String,
    started_at: Instant,
) -> std::result::Result<Value, ExternalImageStructuredExtractFailure> {
    let request_payload =
        external_image_structured_extract_request_payload(config, image_url, prompt_text);
    external_image_structured_extract_submit_provider_request(
        client,
        config,
        request_payload,
        started_at,
    )
    .await
}

pub(crate) async fn external_image_structured_extract_call_provider_with_system(
    client: &reqwest::Client,
    config: &ExternalImageStructuredExtractRuntimeConfig,
    image_url: &str,
    system_prompt: &str,
    prompt_text: String,
    started_at: Instant,
) -> std::result::Result<Value, ExternalImageStructuredExtractFailure> {
    let request_payload = external_image_structured_extract_request_payload_with_system(
        config,
        image_url,
        system_prompt,
        prompt_text,
    );
    external_image_structured_extract_submit_provider_request(
        client,
        config,
        request_payload,
        started_at,
    )
    .await
}

async fn external_image_structured_extract_submit_provider_request(
    client: &reqwest::Client,
    config: &ExternalImageStructuredExtractRuntimeConfig,
    request_payload: Value,
    started_at: Instant,
) -> std::result::Result<Value, ExternalImageStructuredExtractFailure> {
    let response = client
        .post(&config.endpoint_url)
        .bearer_auth(&config.api_key)
        .json(&request_payload)
        .send()
        .await
        .map_err(|error| ExternalImageStructuredExtractFailure {
            reason: format!("image_extract_request_failed:{error}"),
            runtime: external_image_structured_extract_failure_runtime(
                config,
                "request_failed",
                &error.to_string(),
                started_at.elapsed().as_millis() as u64,
            ),
        })?;
    let status = response.status();
    let response_text =
        response
            .text()
            .await
            .map_err(|error| ExternalImageStructuredExtractFailure {
                reason: format!("image_extract_response_read_failed:{error}"),
                runtime: external_image_structured_extract_failure_runtime(
                    config,
                    "response_read_failed",
                    &error.to_string(),
                    started_at.elapsed().as_millis() as u64,
                ),
            })?;
    if !status.is_success() {
        return Err(ExternalImageStructuredExtractFailure {
            reason: format!("image_extract_provider_status:{}", status.as_u16()),
            runtime: external_image_structured_extract_failure_runtime(
                config,
                "provider_status_error",
                &truncate_assistant_supply_text(&response_text, 800),
                started_at.elapsed().as_millis() as u64,
            ),
        });
    }
    let response_json = serde_json::from_str::<Value>(&response_text).map_err(|error| {
        ExternalImageStructuredExtractFailure {
            reason: format!("image_extract_provider_json_invalid:{error}"),
            runtime: external_image_structured_extract_failure_runtime(
                config,
                "provider_json_invalid",
                &error.to_string(),
                started_at.elapsed().as_millis() as u64,
            ),
        }
    })?;
    let Some(content) = external_image_structured_extract_chat_response_text(&response_json) else {
        return Err(ExternalImageStructuredExtractFailure {
            reason: "image_extract_provider_content_missing".to_string(),
            runtime: external_image_structured_extract_failure_runtime(
                config,
                "provider_content_missing",
                &truncate_assistant_supply_text(&response_text, 800),
                started_at.elapsed().as_millis() as u64,
            ),
        });
    };
    parse_json_object_from_model_text(&content).ok_or_else(|| {
        ExternalImageStructuredExtractFailure {
            reason: "image_extract_output_json_missing".to_string(),
            runtime: external_image_structured_extract_failure_runtime(
                config,
                "output_json_missing",
                &truncate_assistant_supply_text(&content, 800),
                started_at.elapsed().as_millis() as u64,
            ),
        }
    })
}

pub(crate) fn parse_json_object_from_model_text(text: &str) -> Option<Value> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        return Some(value);
    }
    let unfenced = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .and_then(|value| value.strip_suffix("```"))
        .map(str::trim)
        .unwrap_or(trimmed);
    if let Ok(value) = serde_json::from_str::<Value>(unfenced) {
        return Some(value);
    }
    let start = unfenced.find('{')?;
    let end = unfenced.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str::<Value>(&unfenced[start..=end]).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config(reasoning_effort: Option<&str>) -> ExternalImageStructuredExtractRuntimeConfig {
        ExternalImageStructuredExtractRuntimeConfig {
            endpoint_url: "https://model.example.test/v1/chat/completions".to_string(),
            api_key: "test-key".to_string(),
            model: "gpt-5.5".to_string(),
            timeout_ms: 60_000,
            max_tokens: 4_096,
            reasoning_effort: reasoning_effort.map(str::to_string),
            retry_enabled: true,
            provider_label: "test-provider".to_string(),
        }
    }

    #[test]
    fn request_payload_keeps_json_mode_high_detail_and_reasoning() {
        let payload = external_image_structured_extract_request_payload(
            &test_config(Some("medium")),
            "https://third.example.com/order.png",
            "请按订单字段抽取".to_string(),
        );

        assert_eq!(payload["model"], json!("gpt-5.5"));
        assert_eq!(payload["messages"][0]["role"], json!("system"));
        assert!(payload["messages"][0]["content"]
            .as_str()
            .unwrap_or_default()
            .contains("只输出 JSON"));
        assert_eq!(
            payload["messages"][1]["content"][0]["text"],
            json!("请按订单字段抽取")
        );
        assert_eq!(
            payload["messages"][1]["content"][1]["image_url"]["detail"],
            json!("high")
        );
        assert_eq!(payload["response_format"]["type"], json!("json_object"));
        assert_eq!(payload["temperature"], json!(0));
        assert_eq!(payload["max_tokens"], json!(4096));
        assert_eq!(payload["reasoning"]["effort"], json!("medium"));
    }

    #[test]
    fn request_payload_omits_reasoning_when_not_configured() {
        let payload = external_image_structured_extract_request_payload_with_system(
            &test_config(None),
            "https://third.example.com/order.png",
            "custom system",
            "custom prompt".to_string(),
        );

        assert_eq!(payload["messages"][0]["content"], json!("custom system"));
        assert_eq!(
            payload["messages"][1]["content"][0]["text"],
            json!("custom prompt")
        );
        assert!(payload.get("reasoning").is_none());
    }

    #[test]
    fn parse_json_object_from_model_text_accepts_plain_fenced_and_wrapped_json() {
        assert_eq!(
            parse_json_object_from_model_text(r#"{"records":[{"order_no":"A1"}]}"#),
            Some(json!({"records":[{"order_no":"A1"}]}))
        );
        assert_eq!(
            parse_json_object_from_model_text("```json\n{\"records\":[]}\n```"),
            Some(json!({"records":[]}))
        );
        assert_eq!(
            parse_json_object_from_model_text("说明文字 {\"records\":[1]} 结束"),
            Some(json!({"records":[1]}))
        );
        assert!(parse_json_object_from_model_text("no json here").is_none());
    }
}
