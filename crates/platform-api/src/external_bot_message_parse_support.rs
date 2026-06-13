use contracts::ExternalBotMessageView;
#[cfg(test)]
use contracts::{ExternalChannelPlatformView, ExternalMessageTypeView};
use serde_json::{json, Value};

use crate::{
    external_channel_platform_wire_value, normalize_external_bot_message_payload,
    validate_and_normalize_external_answer_policy,
    validate_and_normalize_external_artifact_request,
    validate_and_normalize_external_requested_skills, validate_external_aigolf_requested_skills,
    ApiError, ExternalChannelConnectionSummary,
};

pub(crate) fn parse_external_bot_message_payload(
    payload: Value,
    connection: &ExternalChannelConnectionSummary,
) -> std::result::Result<ExternalBotMessageView, ApiError> {
    let normalized = normalize_external_bot_message_payload(payload, connection);
    let mut message: ExternalBotMessageView =
        serde_json::from_value(normalized.clone()).map_err(|error| {
            ApiError::bad_request_with_details(
                "external_channel_event_payload_invalid",
                format!(
                "external channel event JSON does not match the expected message schema: {error}"
            ),
                json!({
                    "expected_platform": external_channel_platform_wire_value(&connection.platform),
                    "expected_message_type": "text",
                    "accepted_field_names": [
                        "platform",
                        "tenant_external_id",
                        "bot_external_id",
                        "conversation_external_id",
                        "sender_external_id",
                        "message_external_id",
                        "message_type",
                        "text",
                        "default_prompt",
                        "output_format",
                        "render_mode",
                        "artifact_type",
                        "template",
                        "mention_external_user_ids",
                        "attachment_refs",
                        "business_datasource_ids",
                        "available_document_source_id",
                        "available_document_external_ids",
                        "dataset_external_id",
                        "dataset_external_ids",
                        "requested_skills",
                        "idempotency_key",
                        "received_at"
                    ],
                    "accepted_aliases": [
                        "tenantExternalId",
                        "botExternalId",
                        "conversationExternalId",
                        "senderExternalId",
                        "messageExternalId",
                        "messageType",
                        "defaultPrompt",
                        "systemPrompt",
                        "outputFormat",
                        "answerFormat",
                        "replyFormat",
                        "renderMode",
                        "responseMode",
                        "artifactType",
                        "artifactTemplate",
                        "templateRef",
                        "mentionExternalUserIds",
                        "attachmentRefs",
                        "businessDatasourceIds",
                        "businessDataSourceIds",
                        "business_database_source_ids",
                        "businessDatabaseSourceIds",
                        "databaseSourceIds",
                        "database_source_ids",
                        "availableDocumentSourceId",
                        "availableDocumentExternalIds",
                        "availableDocumentExternalId",
                        "documentExternalId",
                        "documentExternalIds",
                        "datasetExternalId",
                        "availableDatasetExternalId",
                        "datasetExternalIds",
                        "availableDatasetExternalIds",
                        "requestedSkills",
                        "skillRefs",
                        "skill_refs",
                        "idempotencyKey",
                        "receivedAt"
                    ],
                }),
            )
        })?;
    validate_and_normalize_external_artifact_request(&mut message)?;
    validate_and_normalize_external_requested_skills(&mut message.requested_skills)?;
    validate_external_aigolf_requested_skills(&message)?;
    validate_and_normalize_external_answer_policy(&mut message)?;
    Ok(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn generic_chat_connection() -> ExternalChannelConnectionSummary {
        ExternalChannelConnectionSummary {
            platform: ExternalChannelPlatformView::GenericChat,
            status: "enabled".to_string(),
            config_redacted: json!({}),
        }
    }

    #[test]
    fn parses_bot_message_payload_with_artifact_request_and_answer_policy() {
        let message = parse_external_bot_message_payload(
            json!({
                "tenantExternalId": "tenant-ext-001",
                "botExternalId": "bot-v3",
                "conversationExternalId": "chat-static-page",
                "senderExternalId": "user-ext-001",
                "messageExternalId": "msg-static-001",
                "text": "根据这些资料生成经营分析静态页",
                "defaultPrompt": " 请按客户经营报表口径回答 ",
                "outputFormat": "图文排版",
                "artifactType": "static-page",
                "template": {
                    "sourceId": "third-party-source-main",
                    "documentExternalId": "tpl-xinbai-static-page",
                    "revisionExternalId": "v1",
                    "mode": "reference"
                },
                "requestedSkills": [
                    {
                        "skillId": "risk_review",
                        "mode": "REQUIRED"
                    }
                ],
                "datasetExternalIds": ["workspace-xinbai"]
            }),
            &generic_chat_connection(),
        )
        .expect("payload should parse");

        assert_eq!(message.platform, ExternalChannelPlatformView::GenericChat);
        assert_eq!(message.message_type, ExternalMessageTypeView::Text);
        assert_eq!(
            message.default_prompt.as_deref(),
            Some("请按客户经营报表口径回答")
        );
        assert_eq!(message.output_format.as_deref(), Some("image_text"));
        assert_eq!(message.render_mode.as_deref(), Some("artifact"));
        assert_eq!(message.artifact_type.as_deref(), Some("static_page"));
        assert_eq!(
            message.available_document_external_ids,
            vec!["tpl-xinbai-static-page".to_string()]
        );
        assert_eq!(message.requested_skills.len(), 2);
        assert!(message
            .requested_skills
            .iter()
            .any(|skill| skill.skill_id == "risk_review"
                && skill.mode.as_deref() == Some("required")));
        assert!(message
            .requested_skills
            .iter()
            .any(|skill| skill.skill_id == "document_template_skill"));
    }

    #[test]
    fn rejects_payloads_that_do_not_match_bot_message_schema() {
        let result = parse_external_bot_message_payload(
            json!({
                "tenantExternalId": "tenant-ext-001",
                "botExternalId": "bot-v3",
                "conversationExternalId": "chat-static-page",
                "senderExternalId": "user-ext-001",
                "text": "缺少 messageExternalId"
            }),
            &generic_chat_connection(),
        );

        assert!(result.is_err());
    }
}
