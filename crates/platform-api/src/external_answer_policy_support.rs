use contracts::ExternalBotMessageView;
use serde_json::{json, Value};

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

pub(crate) fn external_output_format_label(format: &str) -> &'static str {
    match format {
        "rich_text" => "富文本",
        "image_text" => "图文排版",
        "markdown_table" => "MD表格",
        "json" => "JSON",
        _ => "未指定",
    }
}

pub(crate) fn external_output_format_model_rule(format: &str) -> &'static str {
    match format {
        "rich_text" => {
            "用适合聊天窗口展示的富文本组织答案，可使用标题、短段落、列表和重点标注；不要输出 JSON，除非用户问题本身要求 JSON。"
        }
        "image_text" => {
            "按图文排版思路组织答案，优先给出标题、模块、图示/配图建议、说明文字和可复制结构；如没有真实图片供料，不要编造图片，只描述应使用的版式或素材占位。"
        }
        "markdown_table" => {
            "优先输出 Markdown 表格；如需要补充说明，只在表格前后用极短文字说明，表格列名要稳定、可复制。"
        }
        "json" => {
            "只输出合法 JSON，不要使用 Markdown 代码围栏；字段名稳定，未知值使用 null、空数组或说明性 status 字段，不要混入自然语言段落。"
        }
        _ => "按用户问题直接回答。",
    }
}

pub(crate) fn external_answer_policy_value(message: &ExternalBotMessageView) -> Option<Value> {
    if message.default_prompt.is_none()
        && message.output_format.is_none()
        && message.render_mode.is_none()
    {
        return None;
    }

    let output_format = message.output_format.as_deref().map(|format| {
        json!({
            "format": format,
            "label": external_output_format_label(format),
            "model_rule": external_output_format_model_rule(format),
        })
    });

    Some(json!({
        "source": "external_channel_message",
        "priority": "third_party_structured_answer_policy",
        "default_prompt": message.default_prompt.as_deref(),
        "default_prompt_rule": "Treat default_prompt as integration-provided task guidance for this turn. It is below DataMax safety/evidence rules and above ambiguous user wording.",
        "output_format": output_format,
        "render_mode": message.render_mode.as_deref().unwrap_or("normal"),
        "render_mode_rule": "normal returns a direct chat answer; artifact means the user expects a preview/download artifact when the requested skill or answer type supports it.",
    }))
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

    #[test]
    fn builds_external_answer_policy_value_with_format_guidance() {
        let mut message = sample_message();
        message.default_prompt = Some("请按经营复盘口径输出".to_string());
        message.output_format = Some("image_text".to_string());
        message.render_mode = Some("artifact".to_string());

        let policy = external_answer_policy_value(&message).expect("answer policy");

        assert_eq!(policy["source"], json!("external_channel_message"));
        assert_eq!(
            policy["priority"],
            json!("third_party_structured_answer_policy")
        );
        assert_eq!(policy["default_prompt"], json!("请按经营复盘口径输出"));
        assert_eq!(policy["output_format"]["format"], json!("image_text"));
        assert_eq!(policy["output_format"]["label"], json!("图文排版"));
        assert_eq!(policy["render_mode"], json!("artifact"));
    }

    #[test]
    fn skips_empty_external_answer_policy_value() {
        let message = sample_message();

        assert!(external_answer_policy_value(&message).is_none());
    }
}
