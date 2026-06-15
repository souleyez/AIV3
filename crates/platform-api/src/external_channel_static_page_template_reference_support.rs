use contracts::ExternalBotMessageView;
use serde_json::{json, Value};

use crate::external_requested_skills_support::{
    external_requested_skill_argument_string, external_requested_skill_mode,
};
use crate::static_page_template_reference_support::{
    infer_static_page_template_reference_id, normalize_static_page_template_reference_id,
    static_page_template_reference_id_from_payload,
    static_page_template_reference_id_from_source_refs,
};

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

pub(crate) fn external_static_page_template_reference_id(reference: &Value) -> Option<&str> {
    ["templateId", "template_id", "id"].iter().find_map(|key| {
        normalize_static_page_template_reference_id(reference.get(*key).and_then(Value::as_str))
    })
}

pub(crate) fn external_channel_static_page_template_reference_from_payload(
    payload: &Value,
) -> Value {
    payload
        .get("template_reference")
        .or_else(|| payload.get("templateReference"))
        .cloned()
        .or_else(|| {
            payload
                .get("source_refs")
                .and_then(|source_refs| {
                    source_refs
                        .get("template_reference")
                        .or_else(|| source_refs.get("templateReference"))
                })
                .cloned()
        })
        .or_else(|| {
            payload
                .get("template_references")
                .or_else(|| payload.get("templateReferences"))
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .cloned()
        })
        .or_else(|| {
            payload
                .get("source_refs")
                .and_then(|source_refs| {
                    source_refs
                        .get("template_references")
                        .or_else(|| source_refs.get("templateReferences"))
                })
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .cloned()
        })
        .unwrap_or(Value::Null)
}

pub(crate) fn external_channel_static_page_template_reference_id_from_payload(
    payload: &Value,
) -> Value {
    if let Some(value) = static_page_template_reference_id_from_payload(payload).or_else(|| {
        payload
            .get("source_refs")
            .and_then(static_page_template_reference_id_from_source_refs)
    }) {
        return Value::String(value.to_string());
    }
    let reference = external_channel_static_page_template_reference_from_payload(payload);
    external_static_page_template_reference_id(&reference)
        .map(|value| Value::String(value.to_string()))
        .unwrap_or(Value::Null)
}

pub(crate) fn external_channel_static_page_payload_or_source_refs_value(
    payload: &Value,
    key: &str,
) -> Value {
    payload
        .get(key)
        .cloned()
        .or_else(|| {
            payload
                .get("source_refs")
                .and_then(|source_refs| source_refs.get(key))
                .cloned()
        })
        .unwrap_or(Value::Null)
}

pub(crate) fn external_channel_static_page_relaxed_template_match_from_payload(
    payload: &Value,
) -> Value {
    external_channel_static_page_payload_or_source_refs_value(payload, "relaxed_template_match")
}

pub(crate) fn external_channel_static_page_template_match_policy_from_payload(
    payload: &Value,
) -> Value {
    let explicit = payload
        .get("template_match_policy")
        .or_else(|| payload.get("templateMatchPolicy"))
        .cloned()
        .or_else(|| {
            payload
                .get("source_refs")
                .and_then(|source_refs| {
                    source_refs
                        .get("template_match_policy")
                        .or_else(|| source_refs.get("templateMatchPolicy"))
                })
                .cloned()
        });
    if let Some(value) = explicit {
        return value;
    }
    let relaxed_match = external_channel_static_page_relaxed_template_match_from_payload(payload);
    if !relaxed_match.is_null() {
        return json!("dataset_overlap");
    }
    if payload.get("status").and_then(Value::as_str) == Some("static_page_stable_artifact_reused") {
        return json!("exact_dataset_artifact_key");
    }
    if !external_channel_static_page_template_reference_id_from_payload(payload).is_null() {
        return json!("explicit_or_inferred_template");
    }
    json!("none")
}

pub(crate) fn external_channel_static_page_style_reuse_policy_from_payload(
    payload: &Value,
) -> Value {
    external_channel_static_page_payload_or_source_refs_value(payload, "style_reuse_policy")
        .as_str()
        .map(|value| json!(value))
        .or_else(|| {
            payload
                .pointer("/artifact_stability/style_reuse_policy")
                .cloned()
        })
        .or_else(|| {
            payload
                .pointer("/source_refs/artifact_stability/style_reuse_policy")
                .cloned()
        })
        .unwrap_or_else(|| json!("reuse_style_unless_explicit_redesign"))
}

pub(crate) fn external_channel_static_page_data_refresh_policy_from_payload(
    payload: &Value,
) -> Value {
    external_channel_static_page_payload_or_source_refs_value(payload, "data_refresh_policy")
        .as_str()
        .map(|value| json!(value))
        .or_else(|| {
            payload
                .pointer("/artifact_stability/data_refresh_policy")
                .cloned()
        })
        .or_else(|| {
            payload
                .pointer("/source_refs/artifact_stability/data_refresh_policy")
                .cloned()
        })
        .unwrap_or_else(|| json!("refresh_data_files_from_dataset_sources"))
}

pub(crate) fn external_channel_static_page_default_template_scope_from_payload(
    payload: &Value,
) -> Value {
    external_channel_static_page_payload_or_source_refs_value(payload, "default_template_scope")
        .as_str()
        .map(|value| json!(value))
        .or_else(|| {
            payload
                .pointer("/artifact_stability/default_template_scope")
                .cloned()
        })
        .or_else(|| {
            payload
                .pointer("/source_refs/artifact_stability/default_template_scope")
                .cloned()
        })
        .unwrap_or_else(|| json!("dataset_combination"))
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
    fn template_reference_from_payload_prefers_direct_and_source_refs_values() {
        let direct_payload = json!({
            "templateReference": {"id": "direct-template"},
            "source_refs": {
                "template_reference": {"id": "source-template"}
            }
        });

        assert_eq!(
            external_channel_static_page_template_reference_from_payload(&direct_payload),
            json!({"id": "direct-template"})
        );

        let source_refs_payload = json!({
            "source_refs": {
                "templateReferences": [{"id": "source-array-template"}]
            }
        });

        assert_eq!(
            external_channel_static_page_template_reference_from_payload(&source_refs_payload),
            json!({"id": "source-array-template"})
        );
    }

    #[test]
    fn template_reference_id_from_payload_uses_direct_source_refs_and_reference_objects() {
        assert_eq!(
            external_channel_static_page_template_reference_id_from_payload(&json!({
                "templateReferenceId": " direct-template "
            })),
            json!("direct-template")
        );
        assert_eq!(
            external_channel_static_page_template_reference_id_from_payload(&json!({
                "source_refs": {
                    "template_reference_id": " source-template "
                }
            })),
            json!("source-template")
        );
        assert_eq!(
            external_channel_static_page_template_reference_id_from_payload(&json!({
                "template_reference": {
                    "template_id": " object-template "
                }
            })),
            json!("object-template")
        );
        assert_eq!(
            external_channel_static_page_template_reference_id_from_payload(&json!({})),
            Value::Null
        );
    }

    #[test]
    fn template_match_policy_from_payload_preserves_existing_fallback_order() {
        assert_eq!(
            external_channel_static_page_template_match_policy_from_payload(&json!({
                "templateMatchPolicy": "strict"
            })),
            json!("strict")
        );
        assert_eq!(
            external_channel_static_page_template_match_policy_from_payload(&json!({
                "source_refs": {
                    "template_match_policy": "source-strict"
                }
            })),
            json!("source-strict")
        );
        assert_eq!(
            external_channel_static_page_template_match_policy_from_payload(&json!({
                "relaxed_template_match": true
            })),
            json!("dataset_overlap")
        );
        assert_eq!(
            external_channel_static_page_template_match_policy_from_payload(&json!({
                "status": "static_page_stable_artifact_reused"
            })),
            json!("exact_dataset_artifact_key")
        );
        assert_eq!(
            external_channel_static_page_template_match_policy_from_payload(&json!({
                "template_reference": {"id": "template-001"}
            })),
            json!("explicit_or_inferred_template")
        );
        assert_eq!(
            external_channel_static_page_template_match_policy_from_payload(&json!({})),
            json!("none")
        );
    }

    #[test]
    fn reuse_refresh_and_scope_policies_preserve_payload_source_refs_and_defaults() {
        let payload = json!({
            "source_refs": {
                "style_reuse_policy": "source-style",
                "artifact_stability": {
                    "data_refresh_policy": "source-stability-refresh",
                    "default_template_scope": "source-stability-scope"
                }
            }
        });

        assert_eq!(
            external_channel_static_page_style_reuse_policy_from_payload(&payload),
            json!("source-style")
        );
        assert_eq!(
            external_channel_static_page_data_refresh_policy_from_payload(&payload),
            json!("source-stability-refresh")
        );
        assert_eq!(
            external_channel_static_page_default_template_scope_from_payload(&payload),
            json!("source-stability-scope")
        );

        assert_eq!(
            external_channel_static_page_style_reuse_policy_from_payload(&json!({})),
            json!("reuse_style_unless_explicit_redesign")
        );
        assert_eq!(
            external_channel_static_page_data_refresh_policy_from_payload(&json!({})),
            json!("refresh_data_files_from_dataset_sources")
        );
        assert_eq!(
            external_channel_static_page_default_template_scope_from_payload(&json!({})),
            json!("dataset_combination")
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
