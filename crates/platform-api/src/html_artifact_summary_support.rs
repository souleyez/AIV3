use serde::Serialize;
use serde_json::Value;

use crate::ApiError;

pub(crate) fn html_artifact_safe_summary_text(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let lowered = trimmed.to_ascii_lowercase();
    let unsafe_patterns = [
        "<script",
        "<iframe",
        "<object",
        "<embed",
        "<link",
        "<meta",
        "<form",
        "javascript:",
        "data:text/html",
        "srcdoc",
        "src=",
        "href=",
        "http://",
        "https://",
        "api_key",
        "api-key",
        "access_token",
        "access-token",
        "authorization",
        "bearer ",
        "cookie",
        "secret",
        "onerror=",
        "onclick=",
        "onload=",
    ];
    if unsafe_patterns
        .iter()
        .any(|pattern| lowered.contains(pattern))
    {
        return "[已移除敏感或不安全内容]".to_string();
    }
    let mut output = trimmed.chars().take(max_chars).collect::<String>();
    if trimmed.chars().count() > max_chars {
        output.push('…');
    }
    output
}

pub(crate) fn html_artifact_serialized_variant<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| "unknown".to_string())
}

pub(crate) fn html_artifact_action_intent_prompt(
    payload: &Value,
) -> std::result::Result<String, ApiError> {
    let object = payload.as_object().ok_or_else(|| {
        ApiError::bad_request(
            "html_artifact_invalid_action_intent",
            "action_intent payload must be an object".to_string(),
        )
    })?;
    for key in ["prompt", "instruction", "message", "text", "note"] {
        if let Some(prompt) = object
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            if prompt.chars().count() > 2000 {
                return Err(ApiError::bad_request(
                    "html_artifact_invalid_action_intent",
                    "action_intent prompt is too long".to_string(),
                ));
            }
            return Ok(prompt.to_string());
        }
    }
    Err(ApiError::bad_request(
        "html_artifact_invalid_action_intent",
        "action_intent payload requires prompt, instruction, message, text, or note".to_string(),
    ))
}

pub(crate) fn html_artifact_patch_operations(
    payload: &Value,
) -> std::result::Result<&[Value], ApiError> {
    payload
        .as_object()
        .and_then(|object| object.get("operations").or_else(|| object.get("patch")))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| {
            ApiError::bad_request(
                "html_artifact_invalid_patch",
                "patch payload.operations must be an array".to_string(),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn safe_summary_text_trims_truncates_and_blocks_unsafe_content() {
        assert_eq!(
            html_artifact_safe_summary_text("  客户经营月报  ", 20),
            "客户经营月报"
        );
        assert_eq!(html_artifact_safe_summary_text("abcdef", 3), "abc…");
        assert_eq!(
            html_artifact_safe_summary_text("请打开 https://example.com/token", 80),
            "[已移除敏感或不安全内容]"
        );
        assert_eq!(html_artifact_safe_summary_text("   ", 20), "");
    }

    #[test]
    fn serialized_variant_keeps_string_enum_and_unknown_fallback() {
        #[derive(Serialize)]
        #[serde(rename_all = "snake_case")]
        enum TestVariant {
            StaticPage,
        }

        #[derive(Serialize)]
        struct NotAString {
            value: &'static str,
        }

        assert_eq!(
            html_artifact_serialized_variant(&TestVariant::StaticPage),
            "static_page"
        );
        assert_eq!(
            html_artifact_serialized_variant(&NotAString { value: "x" }),
            "unknown"
        );
    }

    #[test]
    fn action_intent_prompt_requires_real_instruction() {
        let prompt = html_artifact_action_intent_prompt(&json!({
            "action": "apply_static_page_intent",
            "prompt": "把趋势模块改成折线图"
        }))
        .expect("prompt should be accepted");
        assert_eq!(prompt, "把趋势模块改成折线图");

        let error = html_artifact_action_intent_prompt(&json!({"action": "submit"}))
            .expect_err("empty submit actions cannot mutate product state");
        assert_eq!(error.payload.code, "html_artifact_invalid_action_intent");
    }

    #[test]
    fn patch_operations_accepts_operations_or_patch_array() {
        let operations_payload = json!({
            "operations": [{"op": "replace"}]
        });
        let operations = html_artifact_patch_operations(&operations_payload)
            .expect("operations should be accepted");
        assert_eq!(operations.len(), 1);

        let patch_payload = json!({
            "patch": [{"op": "remove"}]
        });
        let patch =
            html_artifact_patch_operations(&patch_payload).expect("patch alias should be accepted");
        assert_eq!(patch.len(), 1);

        let error = html_artifact_patch_operations(&json!({"operations": {}}))
            .expect_err("non-array patch must be rejected");
        assert_eq!(error.payload.code, "html_artifact_invalid_patch");
    }
}
