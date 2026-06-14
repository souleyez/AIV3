use contracts::ExternalBotMessageView;
use serde_json::Value;

use crate::external_requested_skills_support::{
    external_requested_skill_argument_string, external_requested_skill_mode,
};
use crate::static_page_template_reference_support::infer_static_page_template_reference_id;

pub(crate) fn external_channel_static_page_template_reference_id(
    message: &ExternalBotMessageView,
    prompt: &str,
) -> Option<String> {
    if let Some(value) = message
        .template
        .as_ref()
        .and_then(|template| template.template_reference_id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(value.to_string());
    }

    for skill in message
        .requested_skills
        .iter()
        .filter(|skill| external_requested_skill_mode(skill) != "disabled")
    {
        if let Some(value) = external_requested_skill_argument_string(
            skill,
            &[
                "template_reference_id",
                "templateReferenceId",
                "static_page_template",
                "staticPageTemplate",
            ],
        ) {
            return Some(value);
        }
    }

    infer_static_page_template_reference_id(prompt).map(str::to_string)
}

pub(crate) fn external_static_page_template_reference_label(reference: &Value) -> Option<String> {
    ["label", "name", "title", "templateId", "template_id", "id"]
        .iter()
        .find_map(|key| {
            reference
                .get(*key)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
        })
}

pub(crate) fn external_channel_static_page_pipeline_reply_text(
    codex_auto_publish_enabled: bool,
    template_reference: Option<&Value>,
) -> String {
    let task_clause = if codex_auto_publish_enabled {
        "已创建静态页草稿并提交 Image2 可视化队列；可视化无需客户确认，生成后会继续进入固定 Cloudflare Codex 发布链路。"
    } else {
        "已创建静态页草稿并提交 Image2 可视化队列；固定发布链路当前未启用或未加入 allowlist。"
    };
    if let Some(label) = template_reference.and_then(external_static_page_template_reference_label)
    {
        format!(
            "已收到模板参考：将以「{label}」作为页面结构、版式风格和字段组织参考；事实内容仍以本会话已授权资料和检索证据为准。{task_clause}"
        )
    } else {
        format!("已收到静态页制作需求。{task_clause}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_message(value: Value) -> ExternalBotMessageView {
        serde_json::from_value(value).expect("external bot message")
    }

    #[test]
    fn template_reference_id_prefers_explicit_message_template() {
        let message = sample_message(json!({
            "platform": "generic_chat",
            "tenant_external_id": "tenant-ext-001",
            "bot_external_id": "bot-v3",
            "conversation_external_id": "conv-001",
            "sender_external_id": "user-ext-001",
            "message_external_id": "msg-001",
            "message_type": "text",
            "text": "生成经营分析报告和 KPI 图表",
            "artifact_template": {
                "template_reference_id": " data-report-custom "
            },
            "requested_skills": [{
                "skill_id": "static_page",
                "arguments": {"templateReferenceId": "dashboard-page"}
            }],
            "idempotency_key": "generic:tenant-ext-001:msg-001",
            "received_at": "2026-06-15T00:00:00Z"
        }));

        assert_eq!(
            external_channel_static_page_template_reference_id(
                &message,
                "生成经营分析报告和 KPI 图表"
            ),
            Some("data-report-custom".to_string())
        );
    }

    #[test]
    fn template_reference_id_uses_enabled_requested_skill_argument() {
        let message = sample_message(json!({
            "platform": "generic_chat",
            "tenant_external_id": "tenant-ext-001",
            "bot_external_id": "bot-v3",
            "conversation_external_id": "conv-001",
            "sender_external_id": "user-ext-001",
            "message_external_id": "msg-001",
            "message_type": "text",
            "text": "按模板生成页面",
            "requested_skills": [
                {
                    "skill_id": "static_page",
                    "mode": "disabled",
                    "arguments": {"templateReferenceId": "disabled-template"}
                },
                {
                    "skill_id": "static_page",
                    "arguments": {"staticPageTemplate": "enabled-template"}
                }
            ],
            "idempotency_key": "generic:tenant-ext-001:msg-001",
            "received_at": "2026-06-15T00:00:00Z"
        }));

        assert_eq!(
            external_channel_static_page_template_reference_id(&message, "按模板生成页面"),
            Some("enabled-template".to_string())
        );
    }

    #[test]
    fn template_reference_id_falls_back_to_prompt_intent() {
        let message = sample_message(json!({
            "platform": "generic_chat",
            "tenant_external_id": "tenant-ext-001",
            "bot_external_id": "bot-v3",
            "conversation_external_id": "conv-001",
            "sender_external_id": "user-ext-001",
            "message_external_id": "msg-001",
            "message_type": "text",
            "text": "生成经营分析报告和 KPI 图表",
            "idempotency_key": "generic:tenant-ext-001:msg-001",
            "received_at": "2026-06-15T00:00:00Z"
        }));

        assert_eq!(
            external_channel_static_page_template_reference_id(
                &message,
                "生成经营分析报告和 KPI 图表"
            ),
            Some("data-report".to_string())
        );
    }

    #[test]
    fn template_reference_label_uses_first_non_empty_display_field() {
        let reference = json!({
            "label": " ",
            "name": "经营月报模板",
            "id": "template-001"
        });

        assert_eq!(
            external_static_page_template_reference_label(&reference),
            Some("经营月报模板".to_string())
        );
    }

    #[test]
    fn pipeline_reply_text_mentions_template_and_publish_path() {
        let text = external_channel_static_page_pipeline_reply_text(
            true,
            Some(&json!({"label": "文档模板：客户周报模板"})),
        );

        assert!(text.contains("已收到模板参考"));
        assert!(text.contains("文档模板：客户周报模板"));
        assert!(text.contains("事实内容仍以本会话已授权资料和检索证据为准"));
        assert!(text.contains("继续进入固定 Cloudflare Codex 发布链路"));
    }

    #[test]
    fn pipeline_reply_text_without_template_reports_disabled_publish_path() {
        let text = external_channel_static_page_pipeline_reply_text(false, None);

        assert!(text.contains("已收到静态页制作需求"));
        assert!(text.contains("固定发布链路当前未启用或未加入 allowlist"));
    }
}
