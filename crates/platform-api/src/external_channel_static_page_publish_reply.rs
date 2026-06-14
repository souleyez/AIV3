use contracts::ExternalBotReplyView;
use serde_json::Value;

use crate::{
    codex_host_fixed_task_public_artifact_url_allowed, external_channel_static_page_published_reply,
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

#[cfg(test)]
mod tests {
    use super::*;
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
}
