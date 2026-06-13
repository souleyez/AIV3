use contracts::{
    HtmlArtifactInteractionModeView, HtmlArtifactManifestView, SubmitHtmlArtifactEventRequest,
};
use serde_json::Value;

use crate::ApiError;

pub(crate) fn validate_html_artifact_event_request(
    artifact: &HtmlArtifactManifestView,
    request: &SubmitHtmlArtifactEventRequest,
) -> std::result::Result<(), ApiError> {
    let event_type = request.event_type.trim();
    match &artifact.interaction_mode {
        HtmlArtifactInteractionModeView::ReadOnly => {
            return Err(ApiError::bad_request(
                "html_artifact_read_only",
                "read-only HTML artifacts cannot submit events".to_string(),
            ));
        }
        HtmlArtifactInteractionModeView::JsonPatch if event_type != "html_artifact.patch" => {
            return Err(ApiError::bad_request(
                "html_artifact_event_type_mismatch",
                "json_patch artifacts may only submit html_artifact.patch".to_string(),
            ));
        }
        HtmlArtifactInteractionModeView::ActionIntent
            if event_type != "html_artifact.action_intent" =>
        {
            return Err(ApiError::bad_request(
                "html_artifact_event_type_mismatch",
                "action_intent artifacts may only submit html_artifact.action_intent".to_string(),
            ));
        }
        _ => {}
    }

    validate_html_artifact_event_payload_is_safe(&request.payload, "$")?;
    if event_type == "html_artifact.patch" {
        validate_html_artifact_patch_payload(&request.payload)?;
    } else {
        validate_html_artifact_action_intent_payload(&request.payload)?;
    }
    Ok(())
}

fn validate_html_artifact_action_intent_payload(
    payload: &Value,
) -> std::result::Result<(), ApiError> {
    let action = payload
        .as_object()
        .and_then(|object| object.get("action"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ApiError::bad_request(
                "html_artifact_invalid_action_intent",
                "action_intent payload.action is required".to_string(),
            )
        })?;
    if action.chars().count() > 120 {
        return Err(ApiError::bad_request(
            "html_artifact_invalid_action_intent",
            "action_intent payload.action is too long".to_string(),
        ));
    }
    Ok(())
}

fn validate_html_artifact_patch_payload(payload: &Value) -> std::result::Result<(), ApiError> {
    let Some(object) = payload.as_object() else {
        return Err(ApiError::bad_request(
            "html_artifact_invalid_patch",
            "patch payload must be an object".to_string(),
        ));
    };
    let operations = object
        .get("operations")
        .or_else(|| object.get("patch"))
        .and_then(Value::as_array)
        .ok_or_else(|| {
            ApiError::bad_request(
                "html_artifact_invalid_patch",
                "patch payload.operations must be an array".to_string(),
            )
        })?;
    if operations.is_empty() || operations.len() > 50 {
        return Err(ApiError::bad_request(
            "html_artifact_invalid_patch",
            "patch operations must contain 1-50 operations".to_string(),
        ));
    }

    for operation in operations {
        let Some(operation_object) = operation.as_object() else {
            return Err(ApiError::bad_request(
                "html_artifact_invalid_patch",
                "each patch operation must be an object".to_string(),
            ));
        };
        let op = operation_object
            .get("op")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default();
        if !matches!(op, "add" | "replace" | "remove" | "move" | "copy" | "test") {
            return Err(ApiError::bad_request(
                "html_artifact_invalid_patch",
                format!("{op} is not an allowed patch operation"),
            ));
        }
        let path = operation_object
            .get("path")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default();
        if !path.starts_with('/') || path.chars().count() > 240 {
            return Err(ApiError::bad_request(
                "html_artifact_invalid_patch",
                "patch operation path must be a JSON pointer under 240 chars".to_string(),
            ));
        }
        if matches!(op, "move" | "copy") {
            let from = operation_object
                .get("from")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default();
            if !from.starts_with('/') || from.chars().count() > 240 {
                return Err(ApiError::bad_request(
                    "html_artifact_invalid_patch",
                    "move/copy patch operation requires a valid from JSON pointer".to_string(),
                ));
            }
        }
    }
    Ok(())
}

fn validate_html_artifact_event_payload_is_safe(
    value: &Value,
    path: &str,
) -> std::result::Result<(), ApiError> {
    match value {
        Value::String(text) => validate_html_artifact_safe_string(text, path),
        Value::Array(entries) => {
            if entries.len() > 100 {
                return Err(ApiError::bad_request(
                    "html_artifact_payload_too_large",
                    format!("{path} contains too many entries"),
                ));
            }
            for (index, entry) in entries.iter().enumerate() {
                validate_html_artifact_event_payload_is_safe(entry, &format!("{path}[{index}]"))?;
            }
            Ok(())
        }
        Value::Object(object) => {
            if object.len() > 80 {
                return Err(ApiError::bad_request(
                    "html_artifact_payload_too_large",
                    format!("{path} contains too many fields"),
                ));
            }
            for (key, child) in object {
                validate_html_artifact_safe_string(key, &format!("{path}.{key}"))?;
                validate_html_artifact_event_payload_is_safe(child, &format!("{path}.{key}"))?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn validate_html_artifact_safe_string(text: &str, path: &str) -> std::result::Result<(), ApiError> {
    if text.chars().count() > 4000 {
        return Err(ApiError::bad_request(
            "html_artifact_payload_too_large",
            format!("{path} is too long"),
        ));
    }
    let lowered = text.to_ascii_lowercase();
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
        "http://",
        "https://",
        "api_key",
        "access_token",
        "authorization",
        "bearer ",
        "cookie",
        "secret",
        "onerror=",
        "onclick=",
        "onload=",
    ];
    if let Some(pattern) = unsafe_patterns
        .iter()
        .find(|pattern| lowered.contains(**pattern))
    {
        return Err(ApiError::bad_request(
            "html_artifact_unsafe_payload",
            format!("{path} contains unsafe content: {pattern}"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::HtmlArtifactInteractionModeView;
    use serde_json::json;

    fn request(event_type: &str, payload: Value) -> SubmitHtmlArtifactEventRequest {
        SubmitHtmlArtifactEventRequest {
            assistant_run_id: Some("run-1".to_string()),
            local_thread_id: None,
            event_type: event_type.to_string(),
            payload,
        }
    }

    fn artifact(mode: HtmlArtifactInteractionModeView) -> HtmlArtifactManifestView {
        let mut artifact = HtmlArtifactManifestView::codex_execution_report(
            "run-1",
            "Codex report",
            "action test",
            json!({}),
        );
        artifact.interaction_mode = mode;
        artifact
    }

    #[test]
    fn event_request_keeps_interaction_mode_and_payload_rules() {
        let read_only_error = validate_html_artifact_event_request(
            &artifact(HtmlArtifactInteractionModeView::ReadOnly),
            &request("html_artifact.action_intent", json!({"action": "submit"})),
        )
        .expect_err("read-only artifacts must reject events");
        assert_eq!(read_only_error.payload.code, "html_artifact_read_only");

        let mismatch_error = validate_html_artifact_event_request(
            &artifact(HtmlArtifactInteractionModeView::ActionIntent),
            &request(
                "html_artifact.patch",
                json!({"operations": [{"op": "replace", "path": "/title"}]}),
            ),
        )
        .expect_err("action-intent artifacts must reject patch events");
        assert_eq!(
            mismatch_error.payload.code,
            "html_artifact_event_type_mismatch"
        );

        validate_html_artifact_event_request(
            &artifact(HtmlArtifactInteractionModeView::ActionIntent),
            &request("html_artifact.action_intent", json!({"action": "submit"})),
        )
        .expect("safe action intent should pass");
    }

    #[test]
    fn action_intent_payload_rejects_missing_or_long_action() {
        let missing = validate_html_artifact_action_intent_payload(&json!({"prompt": "更新"}))
            .expect_err("action must be required");
        assert_eq!(missing.payload.code, "html_artifact_invalid_action_intent");

        let long = validate_html_artifact_action_intent_payload(&json!({
            "action": "x".repeat(121)
        }))
        .expect_err("long action must be rejected");
        assert_eq!(long.payload.code, "html_artifact_invalid_action_intent");
    }

    #[test]
    fn patch_payload_keeps_operation_count_op_path_and_from_rules() {
        validate_html_artifact_patch_payload(&json!({
            "patch": [{"op": "copy", "from": "/modules/0", "path": "/modules/1"}]
        }))
        .expect("patch alias and copy from pointer should pass");

        let empty = validate_html_artifact_patch_payload(&json!({"operations": []}))
            .expect_err("empty operation list must be rejected");
        assert_eq!(empty.payload.code, "html_artifact_invalid_patch");

        let bad_op = validate_html_artifact_patch_payload(&json!({
            "operations": [{"op": "merge", "path": "/title"}]
        }))
        .expect_err("unsupported op must be rejected");
        assert_eq!(bad_op.payload.code, "html_artifact_invalid_patch");

        let bad_from = validate_html_artifact_patch_payload(&json!({
            "operations": [{"op": "move", "from": "modules/0", "path": "/modules/1"}]
        }))
        .expect_err("move must require a JSON pointer from path");
        assert_eq!(bad_from.payload.code, "html_artifact_invalid_patch");
    }

    #[test]
    fn payload_safety_rejects_remote_urls_secrets_and_large_shapes() {
        let unsafe_url = validate_html_artifact_event_payload_is_safe(
            &json!({"note": "https://example.com/leak"}),
            "$",
        )
        .expect_err("remote URLs must be rejected");
        assert_eq!(unsafe_url.payload.code, "html_artifact_unsafe_payload");

        let too_many_entries = validate_html_artifact_event_payload_is_safe(
            &json!((0..101).collect::<Vec<_>>()),
            "$.items",
        )
        .expect_err("large arrays must be rejected");
        assert_eq!(
            too_many_entries.payload.code,
            "html_artifact_payload_too_large"
        );

        let too_long = validate_html_artifact_safe_string(&"x".repeat(4001), "$.note")
            .expect_err("long strings must be rejected");
        assert_eq!(too_long.payload.code, "html_artifact_payload_too_large");
    }
}
