#[cfg(test)]
use chrono::Utc;
#[cfg(test)]
use contracts::ExternalChannelPlatformView;
use contracts::{ExternalArtifactTemplateView, ExternalBotMessageView, ExternalMessageTypeView};
use domain_model::AssistantRun;
#[cfg(test)]
use domain_model::{AssistantRunId, TenantId};
use serde_json::Value;

use crate::external_bot_message_payload_support::external_string_ids_from_payload_value;
use crate::ExternalChannelConnectionSummary;

pub(crate) fn external_channel_static_page_source_ref_string(
    source_refs: &Value,
    key: &str,
) -> Option<String> {
    source_refs
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub(crate) fn external_channel_static_page_source_refs_string_array(
    source_refs: &Value,
    key: &str,
) -> Vec<String> {
    source_refs
        .get(key)
        .map(|value| external_string_ids_from_payload_value(value.clone()))
        .unwrap_or_default()
}

pub(crate) fn external_channel_static_page_template_from_source_refs(
    source_refs: &Value,
) -> Option<ExternalArtifactTemplateView> {
    source_refs
        .get("artifact_template")
        .or_else(|| source_refs.get("template"))
        .filter(|value| !value.is_null())
        .and_then(|value| serde_json::from_value(value.clone()).ok())
}

pub(crate) fn external_channel_static_page_message_from_source_refs(
    source_refs: &Value,
    connection: &ExternalChannelConnectionSummary,
    run: &AssistantRun,
) -> ExternalBotMessageView {
    ExternalBotMessageView {
        platform: connection.platform.clone(),
        tenant_external_id: external_channel_static_page_source_ref_string(
            source_refs,
            "tenant_external_id",
        )
        .unwrap_or_else(|| "external-tenant".to_string()),
        bot_external_id: external_channel_static_page_source_ref_string(
            source_refs,
            "bot_external_id",
        )
        .unwrap_or_else(|| "v3".to_string()),
        conversation_external_id: external_channel_static_page_source_ref_string(
            source_refs,
            "conversation_external_id",
        )
        .unwrap_or_else(|| {
            run.local_thread_id
                .clone()
                .unwrap_or_else(|| run.id.to_string())
        }),
        thread_external_id: external_channel_static_page_source_ref_string(
            source_refs,
            "thread_external_id",
        ),
        sender_external_id: external_channel_static_page_source_ref_string(
            source_refs,
            "sender_external_id",
        )
        .unwrap_or_else(|| "external-user".to_string()),
        message_external_id: external_channel_static_page_source_ref_string(
            source_refs,
            "message_external_id",
        )
        .unwrap_or_else(|| run.id.to_string()),
        message_type: ExternalMessageTypeView::Text,
        text: Some(run.user_prompt.clone()),
        default_prompt: None,
        output_format: external_channel_static_page_source_ref_string(source_refs, "output_format")
            .or_else(|| Some("image_text".to_string())),
        render_mode: external_channel_static_page_source_ref_string(source_refs, "render_mode")
            .or_else(|| Some("artifact".to_string())),
        artifact_type: external_channel_static_page_source_ref_string(source_refs, "artifact_type")
            .or_else(|| Some("static_page".to_string())),
        template: external_channel_static_page_template_from_source_refs(source_refs),
        mention_external_user_ids: Vec::new(),
        attachment_refs: Vec::new(),
        business_datasource_ids: Vec::new(),
        available_document_external_ids: Vec::new(),
        available_document_source_id: external_channel_static_page_source_ref_string(
            source_refs,
            "available_document_source_id",
        ),
        dataset_external_id: external_channel_static_page_source_ref_string(
            source_refs,
            "dataset_external_id",
        ),
        dataset_external_ids: external_channel_static_page_source_refs_string_array(
            source_refs,
            "dataset_external_ids",
        ),
        requested_skills: Vec::new(),
        idempotency_key: format!("static-page-auto-publish:{}", run.id),
        received_at: run.created_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use uuid::Uuid;

    fn sample_connection() -> ExternalChannelConnectionSummary {
        ExternalChannelConnectionSummary {
            platform: ExternalChannelPlatformView::GenericChat,
            status: "enabled".to_string(),
            config_redacted: json!({}),
        }
    }

    fn sample_run() -> AssistantRun {
        let now = Utc::now();

        AssistantRun {
            id: AssistantRunId(Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap()),
            tenant_id: TenantId(Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap()),
            user_id: None,
            local_thread_id: Some("local-thread-001".to_string()),
            user_prompt: "生成新世界经营月报".to_string(),
            startup_briefing: json!({}),
            selected_scope: json!({}),
            scope_candidates: json!([]),
            context_policy: json!({}),
            evidence_state: json!({}),
            service_lane: "default".to_string(),
            execution_trail: json!([]),
            output_artifacts: json!([]),
            runtime_manifest: json!({}),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn source_ref_string_trims_non_empty_strings() {
        let source_refs = json!({
            "channel_connection_id": "  generic-chat-main  "
        });

        assert_eq!(
            external_channel_static_page_source_ref_string(&source_refs, "channel_connection_id")
                .as_deref(),
            Some("generic-chat-main")
        );
    }

    #[test]
    fn source_ref_string_rejects_blank_missing_and_non_string_values() {
        let source_refs = json!({
            "blank": "   ",
            "number": 42,
            "array": ["x"]
        });

        assert!(external_channel_static_page_source_ref_string(&source_refs, "blank").is_none());
        assert!(external_channel_static_page_source_ref_string(&source_refs, "missing").is_none());
        assert!(external_channel_static_page_source_ref_string(&source_refs, "number").is_none());
        assert!(external_channel_static_page_source_ref_string(&source_refs, "array").is_none());
    }

    #[test]
    fn source_refs_string_array_normalizes_string_and_array_values() {
        let source_refs = json!({
            "single": " doc-one ",
            "many": ["doc-two", "  ", 9, "doc-three"]
        });

        assert_eq!(
            external_channel_static_page_source_refs_string_array(&source_refs, "single"),
            vec!["doc-one"]
        );
        assert_eq!(
            external_channel_static_page_source_refs_string_array(&source_refs, "many"),
            vec!["doc-two", "doc-three"]
        );
    }

    #[test]
    fn source_refs_string_array_returns_empty_for_missing_or_blank_values() {
        let source_refs = json!({
            "blank": "   ",
            "nullish": null
        });

        assert!(
            external_channel_static_page_source_refs_string_array(&source_refs, "blank").is_empty()
        );
        assert!(
            external_channel_static_page_source_refs_string_array(&source_refs, "nullish")
                .is_empty()
        );
        assert!(
            external_channel_static_page_source_refs_string_array(&source_refs, "missing")
                .is_empty()
        );
    }

    #[test]
    fn template_from_source_refs_prefers_artifact_template_and_accepts_template_alias() {
        let source_refs = json!({
            "artifact_template": {
                "sourceId": "source-main",
                "documentExternalId": "tpl-preferred",
                "templateReferenceId": "xinbai-template"
            },
            "template": {
                "documentExternalId": "tpl-fallback"
            }
        });

        let template = external_channel_static_page_template_from_source_refs(&source_refs)
            .expect("artifact_template should parse");

        assert_eq!(template.source_id.as_deref(), Some("source-main"));
        assert_eq!(
            template.document_external_id.as_deref(),
            Some("tpl-preferred")
        );
        assert_eq!(
            template.template_reference_id.as_deref(),
            Some("xinbai-template")
        );

        let alias_template = external_channel_static_page_template_from_source_refs(&json!({
            "template": {
                "documentExternalId": "tpl-alias"
            }
        }))
        .expect("template alias should parse");

        assert_eq!(
            alias_template.document_external_id.as_deref(),
            Some("tpl-alias")
        );

        assert!(
            external_channel_static_page_template_from_source_refs(&json!({
                "artifact_template": null
            }))
            .is_none()
        );
    }

    #[test]
    fn message_from_source_refs_reconstructs_static_page_external_message() {
        let run = sample_run();
        let source_refs = json!({
            "tenant_external_id": " tenant-ext-001 ",
            "bot_external_id": "bot-v3",
            "conversation_external_id": "conv-static-page",
            "thread_external_id": "thread-001",
            "sender_external_id": "user-ext-001",
            "message_external_id": "msg-001",
            "output_format": "rich_text",
            "render_mode": "normal",
            "artifact_type": "static_page",
            "available_document_source_id": "third-party-source-main",
            "dataset_external_id": "dataset-main",
            "dataset_external_ids": ["dataset-main", " ", 3, "dataset-extra"],
            "template": {
                "documentExternalId": "tpl-report"
            }
        });

        let message = external_channel_static_page_message_from_source_refs(
            &source_refs,
            &sample_connection(),
            &run,
        );

        assert_eq!(message.platform, ExternalChannelPlatformView::GenericChat);
        assert_eq!(message.tenant_external_id, "tenant-ext-001");
        assert_eq!(message.bot_external_id, "bot-v3");
        assert_eq!(message.conversation_external_id, "conv-static-page");
        assert_eq!(message.thread_external_id.as_deref(), Some("thread-001"));
        assert_eq!(message.sender_external_id, "user-ext-001");
        assert_eq!(message.message_external_id, "msg-001");
        assert_eq!(message.message_type, ExternalMessageTypeView::Text);
        assert_eq!(message.text.as_deref(), Some("生成新世界经营月报"));
        assert_eq!(message.default_prompt, None);
        assert_eq!(message.output_format.as_deref(), Some("rich_text"));
        assert_eq!(message.render_mode.as_deref(), Some("normal"));
        assert_eq!(message.artifact_type.as_deref(), Some("static_page"));
        assert_eq!(
            message
                .template
                .as_ref()
                .and_then(|template| template.document_external_id.as_deref()),
            Some("tpl-report")
        );
        assert!(message.mention_external_user_ids.is_empty());
        assert!(message.attachment_refs.is_empty());
        assert!(message.business_datasource_ids.is_empty());
        assert!(message.available_document_external_ids.is_empty());
        assert_eq!(
            message.available_document_source_id.as_deref(),
            Some("third-party-source-main")
        );
        assert_eq!(message.dataset_external_id.as_deref(), Some("dataset-main"));
        assert_eq!(
            message.dataset_external_ids,
            vec!["dataset-main".to_string(), "dataset-extra".to_string()]
        );
        assert!(message.requested_skills.is_empty());
        assert_eq!(
            message.idempotency_key,
            "static-page-auto-publish:11111111-1111-1111-1111-111111111111"
        );
        assert_eq!(message.received_at, run.created_at);
    }

    #[test]
    fn message_from_source_refs_uses_run_fallbacks_for_missing_values() {
        let run = sample_run();
        let message = external_channel_static_page_message_from_source_refs(
            &json!({}),
            &sample_connection(),
            &run,
        );

        assert_eq!(message.tenant_external_id, "external-tenant");
        assert_eq!(message.bot_external_id, "v3");
        assert_eq!(message.conversation_external_id, "local-thread-001");
        assert_eq!(message.thread_external_id, None);
        assert_eq!(message.sender_external_id, "external-user");
        assert_eq!(
            message.message_external_id,
            "11111111-1111-1111-1111-111111111111"
        );
        assert_eq!(message.output_format.as_deref(), Some("image_text"));
        assert_eq!(message.render_mode.as_deref(), Some("artifact"));
        assert_eq!(message.artifact_type.as_deref(), Some("static_page"));
        assert_eq!(message.template, None);
        assert_eq!(message.available_document_source_id, None);
        assert_eq!(message.dataset_external_id, None);
        assert!(message.dataset_external_ids.is_empty());
    }
}
