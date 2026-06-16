use contracts::ExternalChannelEventResponse;
use serde_json::{json, Value};

use crate::assistant_run_text_support::truncate_assistant_supply_text;
use crate::external_channel_support::{
    external_action_dispatch_auth_configured, external_action_dispatch_auth_from_config,
    external_action_dispatch_auth_mode, external_channel_outbound_reply_dispatch_auth_from_config,
    external_channel_outbound_reply_dispatch_url_from_config,
    external_channel_reply_specific_dispatch_auth_from_config,
};

pub(crate) fn external_channel_outbound_reply_dispatch_summary(config: &Value) -> Value {
    let reply_auth = external_channel_reply_specific_dispatch_auth_from_config(config);
    let action_auth = external_action_dispatch_auth_from_config(config);
    let effective_auth = external_channel_outbound_reply_dispatch_auth_from_config(config);
    let dispatch_url = external_channel_outbound_reply_dispatch_url_from_config(config);
    let endpoint_configured = dispatch_url.is_some();
    let endpoint_host = dispatch_url
        .as_deref()
        .and_then(|value| reqwest::Url::parse(value).ok())
        .and_then(|url| url.host_str().map(str::to_string));
    let reply_auth_configured = external_action_dispatch_auth_configured(&reply_auth);
    let action_auth_fallback_available = external_action_dispatch_auth_configured(&action_auth);
    let auth_configured = external_action_dispatch_auth_configured(&effective_auth);
    let auth_source = if reply_auth_configured {
        "reply_specific"
    } else if action_auth_fallback_available {
        "action_dispatch_fallback"
    } else {
        "none"
    };
    json!({
        "endpoint_configured": endpoint_configured,
        "endpoint_host": endpoint_host,
        "auth_configured": auth_configured,
        "auth_mode": external_action_dispatch_auth_mode(&effective_auth),
        "auth_source": auth_source,
        "action_auth_fallback_available": action_auth_fallback_available,
        "ready": endpoint_configured && auth_configured,
    })
}

pub(crate) fn external_channel_outbound_reply_dispatch_payload(
    response: &ExternalChannelEventResponse,
    source_event_name: &str,
) -> Value {
    json!({
        "schema": "v3.external_channel.outbound_reply.v1",
        "event_type": "assistant_reply",
        "trigger": "async_result_completed",
        "source_event_name": source_event_name,
        "assistant_run_id": response.assistant_run_id,
        "idempotency_key": response.idempotency_key,
        "conversation_external_id": response.reply.target_conversation_external_id,
        "reply": response.reply,
        "artifact_links": response.reply.artifact_links,
        "task_status": response.reply.task_status,
        "requires_confirmation": response.reply.requires_confirmation,
    })
}

pub(crate) fn external_channel_outbound_reply_dispatch_audit_payload(
    source_event_name: &str,
    source_event_hash: &str,
    connection_id: &str,
    response: &ExternalChannelEventResponse,
    dispatch: Value,
) -> Value {
    json!({
        "schema": "v3.external_channel.outbound_reply.dispatch_audit.v1",
        "source_event_name": source_event_name,
        "source_event_hash": source_event_hash,
        "channel_connection_id": connection_id,
        "assistant_run_id": response.assistant_run_id,
        "conversation_external_id": response.reply.target_conversation_external_id,
        "reply_type": response.reply.reply_type,
        "task_status": response.reply.task_status,
        "artifact_links": response.reply.artifact_links,
        "text_present": response.reply.text.as_ref().map(|text| !text.trim().is_empty()).unwrap_or(false),
        "text_preview": response.reply.text.as_ref().map(|text| truncate_assistant_supply_text(text, 240)).unwrap_or_default(),
        "dispatch": dispatch,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::{ExternalBotReplyTypeView, ExternalBotReplyView};
    use serde_json::json;

    fn sample_response_with_text(text: Option<String>) -> ExternalChannelEventResponse {
        ExternalChannelEventResponse {
            accepted: true,
            assistant_run_id: None,
            idempotency_key: "outbound:run-1:event-hash".to_string(),
            reply: ExternalBotReplyView {
                target_conversation_external_id: "conv-ext-1".to_string(),
                reply_type: ExternalBotReplyTypeView::ArtifactLink,
                text,
                card: Some(json!({
                    "type": "static_page_result",
                    "title": "新世界百货经营管理月报表",
                })),
                artifact_links: vec![
                    "https://v3.elepcloud.com/generated-artifacts/report/index.html".to_string(),
                ],
                task_status: Some("static_page_published".to_string()),
                requires_confirmation: false,
                action_id: None,
                confirmation_id: None,
            },
        }
    }

    #[test]
    fn outbound_reply_dispatch_summary_prefers_reply_specific_auth() {
        let summary = external_channel_outbound_reply_dispatch_summary(&json!({
            "external_action_dispatch_url": "https://actions.example.com/dispatch",
            "dispatch_bearer_token": "action-token",
            "dispatch_signing_secret": "action-secret",
            "reply_dispatch_url": "https://replies.example.com/assistant",
            "reply_dispatch_bearer_token": "reply-token",
            "reply_dispatch_signing_secret": "reply-secret",
        }));

        assert_eq!(summary["endpoint_configured"], json!(true));
        assert_eq!(summary["endpoint_host"], json!("replies.example.com"));
        assert_eq!(summary["auth_configured"], json!(true));
        assert_eq!(summary["auth_mode"], json!("signature_and_bearer"));
        assert_eq!(summary["auth_source"], json!("reply_specific"));
        assert_eq!(summary["action_auth_fallback_available"], json!(true));
        assert_eq!(summary["ready"], json!(true));
        assert!(!summary.to_string().contains("reply-token"));
        assert!(!summary.to_string().contains("action-secret"));
    }

    #[test]
    fn outbound_reply_dispatch_summary_reports_action_auth_fallback() {
        let summary = external_channel_outbound_reply_dispatch_summary(&json!({
            "reply_dispatch_url": "https://api.example.com/replies",
            "dispatch_bearer_token": "dispatch-token",
        }));

        assert_eq!(summary["endpoint_configured"], json!(true));
        assert_eq!(summary["endpoint_host"], json!("api.example.com"));
        assert_eq!(summary["auth_configured"], json!(true));
        assert_eq!(summary["auth_mode"], json!("bearer"));
        assert_eq!(summary["auth_source"], json!("action_dispatch_fallback"));
        assert_eq!(summary["action_auth_fallback_available"], json!(true));
        assert_eq!(summary["ready"], json!(true));
        assert!(!summary.to_string().contains("dispatch-token"));
    }

    #[test]
    fn outbound_reply_dispatch_summary_reports_unready_without_endpoint_or_auth() {
        let summary = external_channel_outbound_reply_dispatch_summary(&json!({
            "reply_dispatch_url": "[redacted]",
            "reply_dispatch_bearer_token": "[redacted]"
        }));

        assert_eq!(summary["endpoint_configured"], json!(false));
        assert_eq!(summary["endpoint_host"], Value::Null);
        assert_eq!(summary["auth_configured"], json!(false));
        assert_eq!(summary["auth_mode"], json!("none"));
        assert_eq!(summary["auth_source"], json!("none"));
        assert_eq!(summary["ready"], json!(false));
    }

    #[test]
    fn outbound_reply_dispatch_payload_preserves_public_reply_shape() {
        let response = sample_response_with_text(Some("报表已生成。".to_string()));
        let payload = external_channel_outbound_reply_dispatch_payload(
            &response,
            "assistant_run.external_channel_static_page_publish_completed",
        );

        assert_eq!(
            payload["schema"],
            json!("v3.external_channel.outbound_reply.v1")
        );
        assert_eq!(payload["event_type"], json!("assistant_reply"));
        assert_eq!(payload["trigger"], json!("async_result_completed"));
        assert_eq!(
            payload["source_event_name"],
            json!("assistant_run.external_channel_static_page_publish_completed")
        );
        assert_eq!(payload["assistant_run_id"], Value::Null);
        assert_eq!(
            payload["idempotency_key"],
            json!("outbound:run-1:event-hash")
        );
        assert_eq!(payload["conversation_external_id"], json!("conv-ext-1"));
        assert_eq!(payload["reply"]["reply_type"], json!("artifact_link"));
        assert_eq!(
            payload["artifact_links"],
            json!(["https://v3.elepcloud.com/generated-artifacts/report/index.html"])
        );
        assert_eq!(payload["task_status"], json!("static_page_published"));
        assert_eq!(payload["requires_confirmation"], json!(false));
    }

    #[test]
    fn outbound_reply_dispatch_audit_payload_redacts_text_to_preview() {
        let long_text = format!("{}{}", "经营风险 ".repeat(80), "结尾");
        let response = sample_response_with_text(Some(long_text.clone()));
        let audit = external_channel_outbound_reply_dispatch_audit_payload(
            "assistant_run.external_channel_static_page_publish_completed",
            "event-hash-001",
            "generic-chat-main",
            &response,
            json!({
                "status": "dispatched",
                "http_status": 200,
                "external_request_id": "reply-req-001",
            }),
        );

        assert_eq!(
            audit["schema"],
            json!("v3.external_channel.outbound_reply.dispatch_audit.v1")
        );
        assert_eq!(audit["source_event_hash"], json!("event-hash-001"));
        assert_eq!(audit["channel_connection_id"], json!("generic-chat-main"));
        assert_eq!(audit["conversation_external_id"], json!("conv-ext-1"));
        assert_eq!(audit["reply_type"], json!("artifact_link"));
        assert_eq!(audit["task_status"], json!("static_page_published"));
        assert_eq!(audit["text_present"], json!(true));
        assert!(
            audit["text_preview"]
                .as_str()
                .unwrap_or_default()
                .chars()
                .count()
                <= 240
        );
        assert_ne!(audit["text_preview"], json!(long_text));
        assert_eq!(
            audit["dispatch"]["external_request_id"],
            json!("reply-req-001")
        );
    }
}
