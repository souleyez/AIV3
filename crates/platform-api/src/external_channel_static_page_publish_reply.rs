use contracts::ExternalBotReplyView;
use serde_json::Value;

use crate::{
    codex_host_fixed_task_public_artifact_url_allowed,
    external_channel_editable_after_publish_from_payload,
    external_channel_permission_review_status_from_payload,
    external_channel_recipient_delivery_from_payload,
    external_channel_static_page_card_with_template_payload,
    external_channel_static_page_published_reply,
    external_channel_task_status_reply_for_conversation,
};

pub(crate) fn external_channel_static_page_publish_completed_reply_from_event_payload(
    payload: &Value,
) -> Option<ExternalBotReplyView> {
    let public_url = payload
        .get("public_url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| codex_host_fixed_task_public_artifact_url_allowed(value))?;
    let conversation_external_id = payload
        .get("conversation_external_id")
        .and_then(Value::as_str)
        .or_else(|| {
            payload
                .pointer("/source_refs/conversation_external_id")
                .and_then(Value::as_str)
        })?
        .trim();
    if conversation_external_id.is_empty() {
        return None;
    }
    Some(external_channel_static_page_published_reply(
        conversation_external_id,
        public_url,
        payload,
    ))
}

pub(crate) fn external_channel_static_page_publish_failed_reply_from_event_payload(
    payload: &Value,
    fallback_conversation_external_id: &str,
) -> ExternalBotReplyView {
    let conversation_external_id = payload
        .get("conversation_external_id")
        .and_then(Value::as_str)
        .or_else(|| {
            payload
                .pointer("/source_refs/conversation_external_id")
                .and_then(Value::as_str)
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback_conversation_external_id);
    external_channel_task_status_reply_for_conversation(
        conversation_external_id,
        "static_page_publish_failed",
        Some(
            "DataMax 已完成页面规划，但最终静态页暂未完成，系统已记录原因，可继续补充数据、重试或转人工处理。"
                .to_string(),
        ),
        Some(external_channel_static_page_card_with_template_payload(
            serde_json::json!({
                "type": "v3_static_page_image2_publish_status",
                "status": "static_page_publish_failed",
                "template_id": payload
                    .get("template_id")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!("static_page_image2_data_publish")),
                "draft_id": payload.get("draft_id").cloned().unwrap_or(Value::Null),
                "image_job_id": payload.get("image_job_id").cloned().unwrap_or(Value::Null),
                "codex_host_workflow_execution_id": payload
                    .get("codex_host_workflow_execution_id")
                    .cloned()
                    .unwrap_or(Value::Null),
                "publish_mode": payload
                    .get("publish_mode")
                    .cloned()
                    .unwrap_or(Value::Null),
                "retryable": payload
                    .get("retryable")
                    .cloned()
                    .unwrap_or(Value::Bool(false)),
                "error": payload.get("error").cloned().unwrap_or(Value::Null),
                "output": payload.get("output").cloned().unwrap_or(Value::Null),
                "validation": payload.get("validation").cloned().unwrap_or(Value::Null),
                "validation_summary": payload
                    .get("validation_summary")
                    .cloned()
                    .unwrap_or(Value::Null),
                "status_url": payload.get("status_url").cloned().unwrap_or(Value::Null),
                "status_method": payload
                    .get("status_method")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!("GET")),
                "recipient_delivery": external_channel_recipient_delivery_from_payload(payload),
                "permission_review_status": external_channel_permission_review_status_from_payload(
                    payload
                ),
                "editable_after_publish": external_channel_editable_after_publish_from_payload(
                    payload
                ),
                "poll_after_seconds": Value::Null,
            }),
            payload,
        )),
        Vec::new(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::ExternalBotReplyTypeView;
    use serde_json::json;

    fn public_url() -> &'static str {
        "https://v3.elepcloud.com/generated-artifacts/database-static-pages/final/index.html"
    }

    #[test]
    fn publish_completed_reply_reads_direct_conversation_id() {
        let payload = json!({
            "conversation_external_id": "room-direct",
            "public_url": public_url()
        });

        let reply =
            external_channel_static_page_publish_completed_reply_from_event_payload(&payload)
                .expect("completed payload should become artifact reply");

        assert_eq!(reply.target_conversation_external_id, "room-direct");
        assert_eq!(reply.artifact_links, vec![public_url().to_string()]);
    }

    #[test]
    fn publish_completed_reply_reads_source_refs_conversation_id() {
        let payload = json!({
            "source_refs": {
                "conversation_external_id": "room-source-ref"
            },
            "public_url": public_url()
        });

        let reply =
            external_channel_static_page_publish_completed_reply_from_event_payload(&payload)
                .expect("completed payload should use source refs conversation id");

        assert_eq!(reply.target_conversation_external_id, "room-source-ref");
        assert_eq!(reply.artifact_links, vec![public_url().to_string()]);
    }

    #[test]
    fn publish_completed_reply_rejects_missing_scope_or_invalid_url() {
        assert!(
            external_channel_static_page_publish_completed_reply_from_event_payload(&json!({
                "conversation_external_id": "room-1",
                "public_url": "https://example.com/not-allowed/index.html"
            }))
            .is_none()
        );
        assert!(
            external_channel_static_page_publish_completed_reply_from_event_payload(&json!({
                "conversation_external_id": "",
                "public_url": public_url()
            }))
            .is_none()
        );
    }

    #[test]
    fn publish_failed_reply_prefers_payload_conversation_id() {
        let payload = json!({
            "conversation_external_id": "room-direct",
            "template_id": "template-custom",
            "draft_id": "draft-1",
            "retryable": true,
            "status_method": "POST",
            "error": {"code": "publish_timeout"},
            "template_reference_id": "generated-static-page:template-002"
        });

        let reply = external_channel_static_page_publish_failed_reply_from_event_payload(
            &payload,
            "fallback-room",
        );

        assert_eq!(reply.reply_type, ExternalBotReplyTypeView::TaskStatus);
        assert_eq!(reply.target_conversation_external_id, "room-direct");
        assert_eq!(reply.task_status.as_deref(), Some("processing"));
        assert!(reply.artifact_links.is_empty());
        let card = reply.card.expect("failed reply should include status card");
        assert_eq!(card["status"], json!("static_page_publish_failed"));
        assert_eq!(card["template_id"], json!("template-custom"));
        assert_eq!(card["draft_id"], json!("draft-1"));
        assert_eq!(card["retryable"], json!(true));
        assert_eq!(card["status_method"], json!("POST"));
        assert_eq!(card["error"]["code"], json!("publish_timeout"));
        assert_eq!(
            card["template_reference_id"],
            json!("generated-static-page:template-002")
        );
    }

    #[test]
    fn publish_failed_reply_uses_source_refs_or_fallback_conversation_id() {
        let from_source_refs = external_channel_static_page_publish_failed_reply_from_event_payload(
            &json!({
                "source_refs": {
                    "conversation_external_id": "room-source-ref"
                }
            }),
            "fallback-room",
        );
        assert_eq!(
            from_source_refs.target_conversation_external_id,
            "room-source-ref"
        );

        let from_fallback = external_channel_static_page_publish_failed_reply_from_event_payload(
            &json!({
                "conversation_external_id": "",
                "status_url": "https://v3.elepcloud.com/api/status/static-page"
            }),
            "fallback-room",
        );
        assert_eq!(
            from_fallback.target_conversation_external_id,
            "fallback-room"
        );
        let card = from_fallback
            .card
            .expect("failed reply should include status card");
        assert_eq!(
            card["template_id"],
            json!("static_page_image2_data_publish")
        );
        assert_eq!(card["status_method"], json!("GET"));
    }
}
