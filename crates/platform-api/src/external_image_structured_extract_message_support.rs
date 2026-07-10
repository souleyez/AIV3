use contracts::{ExternalAttachmentRefView, ExternalBotMessageView, ExternalMessageTypeView};
use serde_json::{json, Value};

use crate::{sha256_hex, text_normalization::non_empty_trimmed_string};

pub(crate) fn external_attachment_ref_is_image(attachment: &ExternalAttachmentRefView) -> bool {
    if attachment
        .content_type
        .as_deref()
        .map(|content_type| {
            content_type
                .trim()
                .to_ascii_lowercase()
                .starts_with("image/")
        })
        .unwrap_or(false)
    {
        return true;
    }
    let Some(filename) = attachment
        .filename
        .as_deref()
        .map(|value| value.trim().to_ascii_lowercase())
    else {
        return false;
    };
    [".png", ".jpg", ".jpeg", ".webp", ".gif", ".bmp"]
        .iter()
        .any(|suffix| filename.ends_with(suffix))
}

pub(crate) fn external_image_structured_extract_first_image_url(
    message: &ExternalBotMessageView,
) -> Option<String> {
    message
        .attachment_refs
        .iter()
        .filter(|attachment| external_attachment_ref_is_image(attachment))
        .chain(message.attachment_refs.iter())
        .find_map(|attachment| {
            attachment
                .download_url_redacted
                .as_deref()
                .and_then(non_empty_trimmed_string)
        })
        .or_else(|| {
            message
                .text
                .as_deref()
                .and_then(non_empty_trimmed_string)
                .filter(|value| {
                    value.starts_with("https://")
                        || value.starts_with("http://")
                        || value.starts_with("data:image/")
                })
        })
}

pub(crate) fn external_image_structured_extract_attachment_summaries(
    message: &ExternalBotMessageView,
) -> Value {
    Value::Array(
        message
            .attachment_refs
            .iter()
            .map(|attachment| {
                json!({
                    "attachment_external_id": attachment.attachment_external_id,
                    "filename": attachment.filename,
                    "content_type": attachment.content_type,
                    "size_bytes": attachment.size_bytes,
                    "download_url_present": attachment
                        .download_url_redacted
                        .as_ref()
                        .map(|value| !value.trim().is_empty())
                        .unwrap_or(false),
                })
            })
            .collect(),
    )
}

pub(crate) fn external_image_structured_extract_schema_from_message(
    message: &ExternalBotMessageView,
) -> Value {
    let skill_schema = message
        .requested_skills
        .iter()
        .find(|skill| {
            if skill.mode.as_deref() == Some("disabled") {
                return false;
            }
            let skill_id = skill.skill_id.trim().to_ascii_lowercase();
            matches!(
                skill_id.as_str(),
                "order_screenshot_extract"
                    | "order_image_extract"
                    | "recharge_order_extract"
                    | "image_structured_extract"
                    | "screenshot_table_extract"
                    | "business_screenshot_extract"
            )
        })
        .and_then(|skill| skill.arguments.as_ref())
        .and_then(|arguments| {
            arguments
                .get("schema")
                .or_else(|| arguments.get("fields"))
                .cloned()
        });
    skill_schema.unwrap_or_else(|| {
        json!({
            "record_type": "recharge_order",
            "fields": [
                { "name": "recharge_amount", "type": "number", "source_label": "充值额度" },
                { "name": "recharge_amount_raw", "type": "string", "source_label": "充值额度原文" },
                { "name": "pay_amount", "type": "number", "source_label": "支付金额" },
                { "name": "pay_amount_raw", "type": "string", "source_label": "支付金额原文" },
                { "name": "payment_method", "type": "enum", "source_label": "支付方式" },
                { "name": "payment_method_label", "type": "string", "source_label": "支付方式原文" },
                { "name": "order_no", "type": "string", "source_label": "订单号" },
                { "name": "status", "type": "enum", "source_label": "状态" },
                { "name": "status_label", "type": "string", "source_label": "状态原文" },
                { "name": "created_at", "type": "string", "source_label": "创建时间" }
            ]
        })
    })
}

pub(crate) fn external_image_structured_extract_output_format_is_json(
    message: &ExternalBotMessageView,
) -> bool {
    message.output_format.as_deref() == Some("json")
}

pub(crate) fn external_image_structured_extract_id(
    connection_id: &str,
    message: &ExternalBotMessageView,
) -> String {
    let attachment_key = message
        .attachment_refs
        .iter()
        .map(|attachment| attachment.attachment_external_id.as_str())
        .collect::<Vec<_>>()
        .join("|");
    let hash = sha256_hex([
        connection_id.as_bytes(),
        b":",
        message.idempotency_key.as_bytes(),
        b":",
        attachment_key.as_bytes(),
    ]);
    format!("img-extract-{}", &hash[..24])
}

pub(crate) fn external_channel_message_requests_image_structured_extract(
    message: &ExternalBotMessageView,
    prompt: &str,
) -> bool {
    if external_channel_message_has_requested_image_extract_skill(message) {
        return true;
    }
    if !matches!(message.message_type, ExternalMessageTypeView::Image)
        || !external_channel_message_has_image_attachment(message)
    {
        return false;
    }
    if external_channel_message_is_artifact_generation_request(message, prompt) {
        return false;
    }
    true
}

fn external_channel_message_has_requested_image_extract_skill(
    message: &ExternalBotMessageView,
) -> bool {
    message.requested_skills.iter().any(|skill| {
        if skill.mode.as_deref() == Some("disabled") {
            return false;
        }
        let skill_id = skill.skill_id.trim().to_ascii_lowercase();
        matches!(
            skill_id.as_str(),
            "order_screenshot_extract"
                | "order_image_extract"
                | "recharge_order_extract"
                | "image_structured_extract"
                | "screenshot_table_extract"
                | "business_screenshot_extract"
        )
    })
}

fn external_channel_message_is_artifact_generation_request(
    message: &ExternalBotMessageView,
    prompt: &str,
) -> bool {
    if message
        .render_mode
        .as_deref()
        .map(|value| value == "artifact")
        .unwrap_or(false)
        || message.artifact_type.is_some()
        || message.template.is_some()
    {
        return true;
    }
    let lower = prompt.to_ascii_lowercase();
    let artifact_markers = [
        "static_page",
        "html",
        "artifact",
        "dashboard",
        "生成页面",
        "生成报表",
        "可视化",
        "静态页",
        "效果图",
    ];
    artifact_markers.iter().any(|marker| lower.contains(marker))
}

pub(crate) fn external_channel_message_has_image_attachment(
    message: &ExternalBotMessageView,
) -> bool {
    message
        .attachment_refs
        .iter()
        .any(external_attachment_ref_is_image)
        || matches!(message.message_type, ExternalMessageTypeView::Image)
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::{
        ExternalChannelPlatformView, ExternalMessageTypeView, ExternalRequestedSkillView,
    };

    fn attachment(
        external_id: &str,
        filename: Option<&str>,
        content_type: Option<&str>,
        url: Option<&str>,
    ) -> ExternalAttachmentRefView {
        ExternalAttachmentRefView {
            attachment_external_id: external_id.to_string(),
            filename: filename.map(ToOwned::to_owned),
            content_type: content_type.map(ToOwned::to_owned),
            size_bytes: Some(128),
            download_url_redacted: url.map(ToOwned::to_owned),
        }
    }

    fn message_with_attachments(
        text: Option<&str>,
        attachments: Vec<ExternalAttachmentRefView>,
    ) -> ExternalBotMessageView {
        ExternalBotMessageView {
            platform: ExternalChannelPlatformView::GenericChat,
            tenant_external_id: "tenant-ext".to_string(),
            bot_external_id: "bot-v3".to_string(),
            conversation_external_id: "conv-1".to_string(),
            thread_external_id: None,
            sender_external_id: "user-1".to_string(),
            message_external_id: "msg-1".to_string(),
            message_type: ExternalMessageTypeView::Image,
            text: text.map(ToOwned::to_owned),
            default_prompt: None,
            output_format: None,
            render_mode: None,
            artifact_type: None,
            template: None,
            mention_external_user_ids: Vec::new(),
            attachment_refs: attachments,
            business_datasource_ids: Vec::new(),
            available_document_external_ids: Vec::new(),
            available_document_source_id: None,
            dataset_external_id: None,
            dataset_external_ids: Vec::new(),
            requested_skills: Vec::new(),
            idempotency_key: "idem-1".to_string(),
            received_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn attachment_is_image_by_content_type_or_filename_suffix() {
        assert!(external_attachment_ref_is_image(&attachment(
            "img-1",
            Some("file.bin"),
            Some(" image/png "),
            None
        )));
        assert!(external_attachment_ref_is_image(&attachment(
            "img-2",
            Some("LOOKBOOK.WEBP"),
            Some("application/octet-stream"),
            None
        )));
        assert!(!external_attachment_ref_is_image(&attachment(
            "doc-1",
            Some("report.pdf"),
            Some("application/pdf"),
            None
        )));
    }

    #[test]
    fn first_image_url_prefers_image_attachment_then_any_attachment_then_text_url() {
        let message = message_with_attachments(
            Some("https://example.com/from-text.png"),
            vec![
                attachment(
                    "doc-1",
                    Some("report.pdf"),
                    Some("application/pdf"),
                    Some(" https://example.com/report.pdf "),
                ),
                attachment(
                    "img-1",
                    Some("order.png"),
                    Some("image/png"),
                    Some(" https://example.com/order.png "),
                ),
            ],
        );
        assert_eq!(
            external_image_structured_extract_first_image_url(&message),
            Some("https://example.com/order.png".to_string())
        );

        let fallback_attachment = message_with_attachments(
            Some("https://example.com/from-text.png"),
            vec![attachment(
                "doc-1",
                Some("report.pdf"),
                Some("application/pdf"),
                Some(" https://example.com/report.pdf "),
            )],
        );
        assert_eq!(
            external_image_structured_extract_first_image_url(&fallback_attachment),
            Some("https://example.com/report.pdf".to_string())
        );

        let text_only = message_with_attachments(Some(" data:image/png;base64,abc "), Vec::new());
        assert_eq!(
            external_image_structured_extract_first_image_url(&text_only),
            Some("data:image/png;base64,abc".to_string())
        );
    }

    #[test]
    fn attachment_summaries_expose_shape_without_raw_url() {
        let message = message_with_attachments(
            None,
            vec![
                attachment(
                    "img-1",
                    Some("order.png"),
                    Some("image/png"),
                    Some(" https://example.com/order.png "),
                ),
                attachment("img-2", Some("empty.png"), Some("image/png"), Some("   ")),
            ],
        );
        assert_eq!(
            external_image_structured_extract_attachment_summaries(&message),
            json!([
                {
                    "attachment_external_id": "img-1",
                    "filename": "order.png",
                    "content_type": "image/png",
                    "size_bytes": 128,
                    "download_url_present": true
                },
                {
                    "attachment_external_id": "img-2",
                    "filename": "empty.png",
                    "content_type": "image/png",
                    "size_bytes": 128,
                    "download_url_present": false
                }
            ])
        );
    }

    #[test]
    fn schema_prefers_enabled_skill_arguments_then_default_order_schema() {
        let mut message = message_with_attachments(None, Vec::new());
        message.requested_skills = vec![
            ExternalRequestedSkillView {
                skill_id: "order_screenshot_extract".to_string(),
                version: Some("v1".to_string()),
                mode: Some("disabled".to_string()),
                arguments: Some(json!({ "schema": { "record_type": "disabled" } })),
            },
            ExternalRequestedSkillView {
                skill_id: " image_structured_extract ".to_string(),
                version: Some("v1".to_string()),
                mode: Some("required".to_string()),
                arguments: Some(json!({ "fields": [{ "name": "order_no" }] })),
            },
        ];
        assert_eq!(
            external_image_structured_extract_schema_from_message(&message),
            json!([{ "name": "order_no" }])
        );

        let default_schema = external_image_structured_extract_schema_from_message(
            &message_with_attachments(None, Vec::new()),
        );
        assert_eq!(default_schema["record_type"], json!("recharge_order"));
        assert_eq!(
            default_schema["fields"][0]["name"],
            json!("recharge_amount")
        );
    }

    #[test]
    fn output_format_and_extract_id_are_stable_for_message_and_attachments() {
        let mut message = message_with_attachments(
            None,
            vec![attachment(
                "img-1",
                Some("order.png"),
                Some("image/png"),
                Some("https://example.com/order.png"),
            )],
        );
        message.output_format = Some("json".to_string());
        assert!(external_image_structured_extract_output_format_is_json(
            &message
        ));

        let id = external_image_structured_extract_id("generic-chat-main", &message);
        assert!(id.starts_with("img-extract-"));
        assert_eq!(
            id,
            external_image_structured_extract_id("generic-chat-main", &message)
        );

        let mut changed = message.clone();
        changed.attachment_refs[0].attachment_external_id = "img-2".to_string();
        assert_ne!(
            id,
            external_image_structured_extract_id("generic-chat-main", &changed)
        );
    }

    #[test]
    fn image_structured_extract_intent_respects_skill_image_and_artifact_guards() {
        let mut explicit_skill = message_with_attachments(None, Vec::new());
        explicit_skill.message_type = ExternalMessageTypeView::Text;
        explicit_skill.requested_skills = vec![ExternalRequestedSkillView {
            skill_id: " order_screenshot_extract ".to_string(),
            version: Some("v1".to_string()),
            mode: Some("required".to_string()),
            arguments: None,
        }];
        assert!(external_channel_message_requests_image_structured_extract(
            &explicit_skill,
            "普通问题"
        ));

        let image_message = message_with_attachments(
            Some("请识别这张截图"),
            vec![attachment(
                "img-1",
                Some("order.png"),
                Some("image/png"),
                Some("https://example.com/order.png"),
            )],
        );
        assert!(external_channel_message_requests_image_structured_extract(
            &image_message,
            "请识别这张截图"
        ));

        let mut artifact_message = image_message.clone();
        artifact_message.render_mode = Some("artifact".to_string());
        assert!(!external_channel_message_requests_image_structured_extract(
            &artifact_message,
            "请生成报表"
        ));

        let mut text_message = message_with_attachments(Some("普通文本"), Vec::new());
        text_message.message_type = ExternalMessageTypeView::Text;
        assert!(!external_channel_message_requests_image_structured_extract(
            &text_message,
            "普通文本"
        ));
    }
}
