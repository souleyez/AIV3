use contracts::ExternalBotMessageView;
use serde_json::json;

use crate::{ApiError, EXTERNAL_CHANNEL_DEFAULT_PROMPT_LIMIT};

pub(crate) fn validate_and_normalize_external_answer_policy(
    message: &mut ExternalBotMessageView,
) -> std::result::Result<(), ApiError> {
    if let Some(default_prompt) = message.default_prompt.take() {
        let normalized = default_prompt.trim().to_string();
        if normalized.is_empty() {
            message.default_prompt = None;
        } else if normalized.chars().count() > EXTERNAL_CHANNEL_DEFAULT_PROMPT_LIMIT
            || normalized
                .chars()
                .any(|ch| ch.is_control() && !matches!(ch, '\n' | '\r' | '\t'))
        {
            return Err(external_answer_policy_bad_request(
                "invalid_default_prompt",
                format!(
                    "default_prompt must be printable text within {EXTERNAL_CHANNEL_DEFAULT_PROMPT_LIMIT} characters"
                ),
            ));
        } else if external_answer_policy_text_contains_credential_hint(&normalized) {
            return Err(external_answer_policy_bad_request(
                "default_prompt_contains_credential_hint",
                "default_prompt must not include bearer tokens, cookies, api keys, or secret values",
            ));
        } else {
            message.default_prompt = Some(normalized);
        }
    }

    if let Some(output_format) = message.output_format.take() {
        let normalized = output_format.trim();
        if normalized.is_empty() {
            message.output_format = None;
        } else if let Some(format) = normalize_external_output_format(normalized) {
            message.output_format = Some(format.to_string());
        } else {
            return Err(external_answer_policy_bad_request(
                "invalid_output_format",
                "output_format must be one of rich_text, image_text, markdown_table, or json",
            ));
        }
    }

    if let Some(render_mode) = message.render_mode.take() {
        let normalized = render_mode.trim();
        if normalized.is_empty() {
            message.render_mode = None;
        } else if let Some(mode) = normalize_external_render_mode(normalized) {
            message.render_mode = Some(mode.to_string());
        } else {
            return Err(external_answer_policy_bad_request(
                "invalid_render_mode",
                "render_mode must be normal or artifact",
            ));
        }
    }

    Ok(())
}

fn external_answer_policy_text_contains_credential_hint(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "authorization:",
        "bearer ",
        "cookie:",
        "set-cookie:",
        "api_key=",
        "apikey=",
        "access_token=",
        "secret=",
        "password=",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn normalize_external_output_format(value: &str) -> Option<&'static str> {
    let raw = value.trim();
    match raw {
        "富文本" | "普通富文本" | "聊天富文本" => return Some("rich_text"),
        "图文排版" | "图文" | "图文混排" => return Some("image_text"),
        "MD表格" | "Markdown表格" | "表格" => return Some("markdown_table"),
        "JSON" | "Json" | "json" => return Some("json"),
        _ => {}
    }
    let compact = raw
        .chars()
        .filter(|ch| !matches!(ch, '-' | '_' | ' ' | '\t' | '\n' | '\r'))
        .flat_map(|ch| ch.to_lowercase())
        .collect::<String>();
    match compact.as_str() {
        "richtext" | "chat" | "markdown" | "copyablerichtext" => Some("rich_text"),
        "imagetext" | "imagetextlayout" | "richmedia" | "html" | "graphiclayout" => {
            Some("image_text")
        }
        "mdtable" | "markdowntable" => Some("markdown_table"),
        "json" => Some("json"),
        _ => None,
    }
}

fn normalize_external_render_mode(value: &str) -> Option<&'static str> {
    let compact = value
        .trim()
        .chars()
        .filter(|ch| !matches!(ch, '-' | '_' | ' ' | '\t' | '\n' | '\r'))
        .flat_map(|ch| ch.to_lowercase())
        .collect::<String>();
    match compact.as_str() {
        "" => None,
        "normal" | "chat" | "text" | "plain" => Some("normal"),
        "artifact" | "html" | "staticpage" | "page" | "download" => Some("artifact"),
        _ => None,
    }
}

fn external_answer_policy_bad_request(reason: &str, message: impl Into<String>) -> ApiError {
    ApiError::bad_request_with_details(
        "external_channel_answer_policy_invalid",
        message.into(),
        json!({
            "reason": reason,
            "schema": {
                "default_prompt": "optional printable text",
                "output_format": "rich_text | image_text | markdown_table | json",
                "render_mode": "normal | artifact"
            },
            "accepted_output_format_aliases": ["富文本", "图文排版", "MD表格", "JSON"]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_message() -> ExternalBotMessageView {
        serde_json::from_value(json!({
            "platform": "generic_chat",
            "tenant_external_id": "tenant-ext-001",
            "bot_external_id": "bot-v3",
            "conversation_external_id": "conv-001",
            "sender_external_id": "user-ext-001",
            "message_external_id": "msg-001",
            "message_type": "text",
            "text": "生成一份报表",
            "idempotency_key": "generic:tenant-ext-001:msg-001",
            "received_at": "2026-06-14T00:00:00Z"
        }))
        .expect("sample message")
    }

    #[test]
    fn normalizes_external_answer_policy_aliases() {
        let mut message = sample_message();
        message.default_prompt = Some("  正式一点输出  ".to_string());
        message.output_format = Some("MD表格".to_string());
        message.render_mode = Some("Static Page".to_string());

        validate_and_normalize_external_answer_policy(&mut message).expect("valid policy");

        assert_eq!(message.default_prompt.as_deref(), Some("正式一点输出"));
        assert_eq!(message.output_format.as_deref(), Some("markdown_table"));
        assert_eq!(message.render_mode.as_deref(), Some("artifact"));
    }

    #[test]
    fn clears_empty_answer_policy_fields() {
        let mut message = sample_message();
        message.default_prompt = Some("  ".to_string());
        message.output_format = Some("\t".to_string());
        message.render_mode = Some("\n".to_string());

        validate_and_normalize_external_answer_policy(&mut message).expect("empty fields clear");

        assert!(message.default_prompt.is_none());
        assert!(message.output_format.is_none());
        assert!(message.render_mode.is_none());
    }

    #[test]
    fn rejects_external_answer_policy_credential_hints() {
        let mut message = sample_message();
        message.default_prompt = Some("Authorization: Bearer token".to_string());

        assert!(validate_and_normalize_external_answer_policy(&mut message).is_err());
    }
}
