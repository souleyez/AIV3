#[cfg(test)]
use chrono::Utc;
use contracts::{ExternalBotMessageView, ExternalRequestedSkillView};
#[cfg(test)]
use contracts::{ExternalChannelPlatformView, ExternalMessageTypeView};
use serde_json::{json, Value};

pub(crate) fn external_channel_static_page_recipient_delivery(
    message: &ExternalBotMessageView,
    prompt: &str,
) -> Value {
    let explicit_mapping = external_requested_skills_permission_mapping(&message.requested_skills);
    let role_scope_candidates = external_channel_static_page_role_scope_candidates(prompt);
    let mapping_status = if explicit_mapping.is_some() {
        "provided_for_auto_configuration"
    } else if !message.mention_external_user_ids.is_empty() && !role_scope_candidates.is_empty() {
        "needs_user_role_scope_mapping"
    } else if !role_scope_candidates.is_empty() {
        "role_requirements_detected"
    } else {
        "needs_user_role_mapping"
    };
    json!({
        "enabled": true,
        "editable_after_publish": true,
        "can_create_recipient_specific_links": true,
        "recipient_link_policy": "create_separate_static_page_link_per_role_or_store_scope",
        "current_delivery_mode": "base_link_first_then_recipient_specific_adjustment",
        "mapping_status": mapping_status,
        "permission_review_status": mapping_status,
        "operator_external_user_id": message.sender_external_id,
        "target_external_user_ids": message.mention_external_user_ids,
        "role_scope_candidates": role_scope_candidates,
        "provided_mapping": explicit_mapping.unwrap_or(Value::Null),
        "default_page_scope": "summary_view_until_user_role_store_mapping_is_confirmed",
        "operator_hint": "页面链接可先交付；如需分别发送给总部、分店店总或指定门店人员，请继续提供用户-角色-门店映射，DataMax 可基于当前页面继续生成对应权限口径的单独链接。",
        "mapping_input_hint": {
            "users": "external_user_id -> role",
            "scopes": "role -> store_ids/region_ids/brand_ids",
            "examples": [
                {"external_user_id": "user-hq-001", "role": "headquarters", "scope": "all_stores"},
                {"external_user_id": "user-store-001", "role": "store_manager", "store_scope": ["南京新百店"]}
            ]
        }
    })
}

fn external_requested_skills_permission_mapping(
    skills: &[ExternalRequestedSkillView],
) -> Option<Value> {
    for skill in skills {
        let Some(arguments) = skill.arguments.as_ref().and_then(Value::as_object) else {
            continue;
        };
        for key in [
            "user_role_mappings",
            "userRoleMappings",
            "recipient_permissions",
            "recipientPermissions",
            "permission_mappings",
            "permissionMappings",
            "user_permission_scope",
            "userPermissionScope",
        ] {
            if let Some(value) = arguments.get(key).filter(|value| !value.is_null()) {
                return Some(value.clone());
            }
        }
    }
    None
}

fn external_channel_static_page_role_scope_candidates(prompt: &str) -> Vec<Value> {
    let normalized = prompt.to_ascii_lowercase();
    let mut candidates = Vec::new();
    let has_headquarters = prompt.contains("总部")
        || prompt.contains("管理层")
        || normalized.contains("headquarters")
        || normalized.contains("hq");
    if has_headquarters {
        candidates.push(json!({
            "role": "headquarters",
            "label": "总部管理层",
            "default_scope": "all_stores",
            "page_focus": ["经营健康度", "区域/门店排行", "风险机会池", "全量汇总"]
        }));
    }
    let has_store_manager = prompt.contains("分店")
        || prompt.contains("店总")
        || prompt.contains("门店")
        || prompt.contains("店长")
        || normalized.contains("store_manager")
        || normalized.contains("store manager");
    if has_store_manager {
        candidates.push(json!({
            "role": "store_manager",
            "label": "分店店总",
            "default_scope": "assigned_store_only",
            "page_focus": ["本店经营问题", "品牌明细", "行动清单", "本店风险预警"]
        }));
    }
    candidates
}

pub(crate) fn external_channel_recipient_delivery_from_payload(payload: &Value) -> Value {
    [
        payload.get("recipient_delivery"),
        payload.pointer("/source_refs/recipient_delivery"),
        payload.pointer("/requirements/recipient_delivery"),
        payload.pointer("/fixed_task/requirements/recipient_delivery"),
    ]
    .into_iter()
    .flatten()
    .find(|value| !value.is_null())
    .cloned()
    .unwrap_or(Value::Null)
}

pub(crate) fn external_channel_permission_review_status_from_payload(payload: &Value) -> Value {
    if let Some(value) = payload
        .get("permission_review_status")
        .filter(|value| !value.is_null())
    {
        return value.clone();
    }
    external_channel_recipient_delivery_from_payload(payload)
        .get("permission_review_status")
        .cloned()
        .unwrap_or(Value::Null)
}

pub(crate) fn external_channel_editable_after_publish_from_payload(payload: &Value) -> Value {
    if let Some(value) = payload
        .get("editable_after_publish")
        .filter(|value| !value.is_null())
    {
        return value.clone();
    }
    external_channel_recipient_delivery_from_payload(payload)
        .get("editable_after_publish")
        .cloned()
        .unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn requested_skill(arguments: Value) -> ExternalRequestedSkillView {
        ExternalRequestedSkillView {
            skill_id: "static_page".to_string(),
            version: None,
            mode: None,
            arguments: Some(arguments),
        }
    }

    fn message_with(
        skills: Vec<ExternalRequestedSkillView>,
        mentions: Vec<String>,
    ) -> ExternalBotMessageView {
        ExternalBotMessageView {
            platform: ExternalChannelPlatformView::GenericChat,
            tenant_external_id: "tenant".to_string(),
            bot_external_id: "bot".to_string(),
            conversation_external_id: "conversation".to_string(),
            thread_external_id: None,
            sender_external_id: "operator".to_string(),
            message_external_id: "message".to_string(),
            message_type: ExternalMessageTypeView::Text,
            text: Some("生成报表".to_string()),
            default_prompt: None,
            output_format: None,
            render_mode: None,
            artifact_type: None,
            template: None,
            mention_external_user_ids: mentions,
            attachment_refs: Vec::new(),
            business_datasource_ids: Vec::new(),
            available_document_external_ids: Vec::new(),
            available_document_source_id: None,
            dataset_external_id: None,
            dataset_external_ids: Vec::new(),
            requested_skills: skills,
            idempotency_key: "idem".to_string(),
            received_at: Utc::now(),
        }
    }

    #[test]
    fn static_page_recipient_delivery_uses_explicit_permission_mapping() {
        let mapping = json!({"user-hq-001": {"role": "headquarters", "scope": "all_stores"}});
        let message = message_with(
            vec![requested_skill(json!({
                "permissionMappings": mapping.clone()
            }))],
            Vec::new(),
        );

        let delivery = external_channel_static_page_recipient_delivery(
            &message,
            "请给总部和分店店总分别生成经营报表",
        );

        assert_eq!(
            delivery["mapping_status"],
            json!("provided_for_auto_configuration")
        );
        assert_eq!(delivery["provided_mapping"], mapping);
        assert_eq!(
            delivery["role_scope_candidates"][0]["role"],
            json!("headquarters")
        );
        assert_eq!(
            delivery["role_scope_candidates"][1]["role"],
            json!("store_manager")
        );
    }

    #[test]
    fn static_page_recipient_delivery_detects_role_mapping_needed() {
        let message = message_with(Vec::new(), vec!["user-store-001".to_string()]);

        let delivery =
            external_channel_static_page_recipient_delivery(&message, "门店店长查看本店风险");

        assert_eq!(
            delivery["mapping_status"],
            json!("needs_user_role_scope_mapping")
        );
        assert_eq!(
            delivery["target_external_user_ids"],
            json!(["user-store-001"])
        );
        assert_eq!(
            delivery["role_scope_candidates"][0]["role"],
            json!("store_manager")
        );
    }

    #[test]
    fn recipient_delivery_payload_helpers_keep_existing_precedence() {
        let payload = json!({
            "permission_review_status": "direct_status",
            "editable_after_publish": false,
            "source_refs": {
                "recipient_delivery": {
                    "permission_review_status": "source_status",
                    "editable_after_publish": true
                }
            },
            "requirements": {
                "recipient_delivery": {
                    "permission_review_status": "requirements_status"
                }
            }
        });

        assert_eq!(
            external_channel_recipient_delivery_from_payload(&payload),
            json!({
                "permission_review_status": "source_status",
                "editable_after_publish": true
            })
        );
        assert_eq!(
            external_channel_permission_review_status_from_payload(&payload),
            json!("direct_status")
        );
        assert_eq!(
            external_channel_editable_after_publish_from_payload(&payload),
            json!(false)
        );
    }

    #[test]
    fn recipient_delivery_payload_helpers_fall_back_to_nested_values() {
        let payload = json!({
            "fixed_task": {
                "requirements": {
                    "recipient_delivery": {
                        "permission_review_status": "fixed_status",
                        "editable_after_publish": true
                    }
                }
            }
        });

        assert_eq!(
            external_channel_permission_review_status_from_payload(&payload),
            json!("fixed_status")
        );
        assert_eq!(
            external_channel_editable_after_publish_from_payload(&payload),
            json!(true)
        );
    }
}
