use contracts::ExternalChannelEventResponse;
use serde_json::{json, Map, Value};

use crate::assistant_run_text_support::truncate_assistant_supply_text;
use crate::external_action_dispatch_transport_support::external_action_response_request_id;
use crate::external_channel_support::{
    external_action_dispatch_auth_configured, external_action_dispatch_auth_from_config,
    external_action_dispatch_auth_mode, external_channel_outbound_reply_dispatch_auth_from_config,
    external_channel_outbound_reply_dispatch_url_from_config,
    external_channel_reply_specific_dispatch_auth_from_config,
};
use crate::external_integration_summary::action_response_summary;

pub(crate) const EXTERNAL_CHANNEL_OUTBOUND_REPLY_SCHEMA_V1: &str =
    "v3.external_channel.outbound_reply.v1";
pub(crate) const EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_AUDIT_SCHEMA_V1: &str =
    "v3.external_channel.outbound_reply.dispatch_audit.v1";
pub(crate) const EXTERNAL_CHANNEL_OUTBOUND_REPLY_EVENT_TYPE_ASSISTANT_REPLY: &str =
    "assistant_reply";
pub(crate) const EXTERNAL_CHANNEL_OUTBOUND_REPLY_TRIGGER_ASYNC_RESULT_COMPLETED: &str =
    "async_result_completed";
pub(crate) const EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_EVENT_NAME_PREFIX: &str =
    "assistant_run.external_channel_outbound_reply_dispatch_";
pub(crate) const EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_BLOCKED_EVENT_NAME: &str =
    "assistant_run.external_channel_outbound_reply_dispatch_blocked";
pub(crate) const EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_FAILED_EVENT_NAME: &str =
    "assistant_run.external_channel_outbound_reply_dispatch_failed";
pub(crate) const EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_DISPATCHED_EVENT_NAME: &str =
    "assistant_run.external_channel_outbound_reply_dispatch_dispatched";
pub(crate) const EXTERNAL_CHANNEL_OUTBOUND_REPLY_STATIC_PAGE_PUBLISH_COMPLETED_SOURCE_EVENT_NAME:
    &str = "assistant_run.external_channel_static_page_publish_completed";
pub(crate) const EXTERNAL_CHANNEL_OUTBOUND_REPLY_STATIC_PAGE_PUBLISH_FAILED_SOURCE_EVENT_NAME:
    &str = "assistant_run.external_channel_static_page_publish_failed";
pub(crate) const EXTERNAL_CHANNEL_OUTBOUND_REPLY_FIRST_TURN_FOLLOWUP_ACTION_SOURCE_EVENT_NAME:
    &str = "assistant_run.external_channel_first_turn_followup_action_reply";
pub(crate) const EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_STATUS_BLOCKED: &str = "dispatch_blocked";
pub(crate) const EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_STATUS_FAILED: &str = "dispatch_failed";
pub(crate) const EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_STATUS_DISPATCHED: &str = "dispatched";
pub(crate) const EXTERNAL_CHANNEL_OUTBOUND_REPLY_REASON_CONNECTION_MISSING: &str =
    "connection_missing";
pub(crate) const EXTERNAL_CHANNEL_OUTBOUND_REPLY_REASON_CONNECTION_DISABLED: &str =
    "connection_disabled";
pub(crate) const EXTERNAL_CHANNEL_OUTBOUND_REPLY_REASON_ENDPOINT_MISSING: &str =
    "reply_dispatch_endpoint_missing";
pub(crate) const EXTERNAL_CHANNEL_OUTBOUND_REPLY_REASON_ENDPOINT_INVALID: &str =
    "reply_dispatch_endpoint_invalid";
pub(crate) const EXTERNAL_CHANNEL_OUTBOUND_REPLY_REASON_AUTH_MISSING: &str =
    "reply_dispatch_auth_missing";
pub(crate) const EXTERNAL_CHANNEL_OUTBOUND_REPLY_REASON_REQUEST_FAILED: &str = "request_failed";
pub(crate) const EXTERNAL_CHANNEL_OUTBOUND_REPLY_AUTH_SOURCE_REPLY_SPECIFIC: &str =
    "reply_specific";
pub(crate) const EXTERNAL_CHANNEL_OUTBOUND_REPLY_AUTH_SOURCE_ACTION_DISPATCH_FALLBACK: &str =
    "action_dispatch_fallback";
pub(crate) const EXTERNAL_CHANNEL_OUTBOUND_REPLY_AUTH_SOURCE_NONE: &str = "none";
pub(crate) const EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_TIMEOUT_SECS: u64 = 12;

pub(crate) fn external_channel_outbound_reply_dispatch_event_names() -> [&'static str; 3] {
    [
        EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_BLOCKED_EVENT_NAME,
        EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_FAILED_EVENT_NAME,
        EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_DISPATCHED_EVENT_NAME,
    ]
}

pub(crate) fn external_channel_outbound_reply_terminal_source_event_names() -> [&'static str; 2] {
    [
        EXTERNAL_CHANNEL_OUTBOUND_REPLY_STATIC_PAGE_PUBLISH_COMPLETED_SOURCE_EVENT_NAME,
        EXTERNAL_CHANNEL_OUTBOUND_REPLY_STATIC_PAGE_PUBLISH_FAILED_SOURCE_EVENT_NAME,
    ]
}

pub(crate) fn external_channel_outbound_reply_is_terminal_source_event(event_name: &str) -> bool {
    external_channel_outbound_reply_terminal_source_event_names().contains(&event_name)
}

pub(crate) fn external_channel_outbound_reply_dispatch_status_from_event_name(
    event_name: &str,
) -> Option<&'static str> {
    match event_name {
        EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_BLOCKED_EVENT_NAME => {
            Some(EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_STATUS_BLOCKED)
        }
        EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_FAILED_EVENT_NAME => {
            Some(EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_STATUS_FAILED)
        }
        EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_DISPATCHED_EVENT_NAME => {
            Some(EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_STATUS_DISPATCHED)
        }
        _ => None,
    }
}

pub(crate) fn external_channel_outbound_reply_dispatch_ready(
    endpoint_configured: bool,
    auth_configured: bool,
) -> bool {
    endpoint_configured && auth_configured
}

pub(crate) fn external_channel_outbound_reply_dispatch_auth_source(
    reply_auth_configured: bool,
    action_auth_fallback_available: bool,
) -> &'static str {
    if reply_auth_configured {
        EXTERNAL_CHANNEL_OUTBOUND_REPLY_AUTH_SOURCE_REPLY_SPECIFIC
    } else if action_auth_fallback_available {
        EXTERNAL_CHANNEL_OUTBOUND_REPLY_AUTH_SOURCE_ACTION_DISPATCH_FALLBACK
    } else {
        EXTERNAL_CHANNEL_OUTBOUND_REPLY_AUTH_SOURCE_NONE
    }
}

pub(crate) fn external_channel_outbound_reply_dispatch_endpoint_configured(
    dispatch_url: Option<&str>,
) -> bool {
    dispatch_url.is_some()
}

pub(crate) fn external_channel_outbound_reply_dispatch_endpoint_host(
    dispatch_url: Option<&str>,
) -> Option<String> {
    dispatch_url
        .and_then(|value| reqwest::Url::parse(value).ok())
        .and_then(|url| url.host_str().map(str::to_string))
}

pub(crate) fn external_channel_outbound_reply_text_present(text: Option<&str>) -> bool {
    text.map(str::trim).is_some_and(|value| !value.is_empty())
}

pub(crate) fn external_channel_outbound_reply_text_preview(text: Option<&str>) -> String {
    text.map(|value| truncate_assistant_supply_text(value, 240))
        .unwrap_or_default()
}

pub(crate) fn external_channel_outbound_reply_dispatch_http_status_success(
    http_status: u16,
) -> bool {
    (200..300).contains(&http_status)
}

pub(crate) fn external_channel_outbound_reply_dispatch_event_name_for_http_status(
    http_status: u16,
) -> &'static str {
    if external_channel_outbound_reply_dispatch_http_status_success(http_status) {
        EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_DISPATCHED_EVENT_NAME
    } else {
        EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_FAILED_EVENT_NAME
    }
}

pub(crate) fn external_channel_outbound_reply_dispatch_status_for_http_status(
    http_status: u16,
) -> &'static str {
    if external_channel_outbound_reply_dispatch_http_status_success(http_status) {
        EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_STATUS_DISPATCHED
    } else {
        EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_STATUS_FAILED
    }
}

pub(crate) fn external_channel_outbound_reply_http_dispatch_result(
    endpoint_host: Option<&str>,
    auth_mode: &str,
    http_status: u16,
    response_summary: Value,
    external_request_id: Option<String>,
) -> Value {
    json!({
        "status": external_channel_outbound_reply_dispatch_status_for_http_status(http_status),
        "endpoint_configured": true,
        "endpoint_host": endpoint_host,
        "auth_mode": auth_mode,
        "http_status": http_status,
        "response_summary": response_summary,
        "external_request_id": external_request_id,
    })
}

pub(crate) fn external_channel_outbound_reply_blocked_dispatch_result(
    reason: &str,
    endpoint_configured: Option<bool>,
    endpoint_host: Option<&str>,
    auth_mode: Option<&str>,
) -> Value {
    let mut dispatch = Map::new();
    dispatch.insert(
        "status".to_string(),
        json!(EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_STATUS_BLOCKED),
    );
    if let Some(endpoint_configured) = endpoint_configured {
        dispatch.insert(
            "endpoint_configured".to_string(),
            json!(endpoint_configured),
        );
    }
    if let Some(endpoint_host) = endpoint_host {
        dispatch.insert("endpoint_host".to_string(), json!(endpoint_host));
    }
    if let Some(auth_mode) = auth_mode {
        dispatch.insert("auth_mode".to_string(), json!(auth_mode));
    }
    dispatch.insert("reason".to_string(), json!(reason));
    Value::Object(dispatch)
}

pub(crate) fn external_channel_outbound_reply_header_blocked_dispatch_result(
    reason: &str,
    endpoint_host: Option<&str>,
    auth_mode: &str,
) -> Value {
    external_channel_outbound_reply_blocked_dispatch_result(
        reason,
        Some(true),
        endpoint_host,
        Some(auth_mode),
    )
}

pub(crate) fn external_channel_outbound_reply_request_failed_dispatch_result(
    endpoint_host: Option<&str>,
    auth_mode: &str,
    request_error_kind: &str,
) -> Value {
    json!({
        "status": EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_STATUS_FAILED,
        "endpoint_configured": true,
        "endpoint_host": endpoint_host,
        "auth_mode": auth_mode,
        "reason": EXTERNAL_CHANNEL_OUTBOUND_REPLY_REASON_REQUEST_FAILED,
        "request_error_kind": request_error_kind,
    })
}

pub(crate) fn external_channel_outbound_reply_dispatch_client(
) -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(
            EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_TIMEOUT_SECS,
        ))
        .build()
}

pub(crate) fn external_channel_outbound_reply_dispatch_summary(config: &Value) -> Value {
    let reply_auth = external_channel_reply_specific_dispatch_auth_from_config(config);
    let action_auth = external_action_dispatch_auth_from_config(config);
    let effective_auth = external_channel_outbound_reply_dispatch_auth_from_config(config);
    let dispatch_url = external_channel_outbound_reply_dispatch_url_from_config(config);
    let endpoint_configured =
        external_channel_outbound_reply_dispatch_endpoint_configured(dispatch_url.as_deref());
    let endpoint_host =
        external_channel_outbound_reply_dispatch_endpoint_host(dispatch_url.as_deref());
    let reply_auth_configured = external_action_dispatch_auth_configured(&reply_auth);
    let action_auth_fallback_available = external_action_dispatch_auth_configured(&action_auth);
    let auth_configured = external_action_dispatch_auth_configured(&effective_auth);
    let auth_source = external_channel_outbound_reply_dispatch_auth_source(
        reply_auth_configured,
        action_auth_fallback_available,
    );
    json!({
        "endpoint_configured": endpoint_configured,
        "endpoint_host": endpoint_host,
        "auth_configured": auth_configured,
        "auth_mode": external_action_dispatch_auth_mode(&effective_auth),
        "auth_source": auth_source,
        "action_auth_fallback_available": action_auth_fallback_available,
        "ready": external_channel_outbound_reply_dispatch_ready(endpoint_configured, auth_configured),
    })
}

pub(crate) fn external_channel_outbound_reply_dispatch_payload(
    response: &ExternalChannelEventResponse,
    source_event_name: &str,
) -> Value {
    json!({
        "schema": EXTERNAL_CHANNEL_OUTBOUND_REPLY_SCHEMA_V1,
        "event_type": EXTERNAL_CHANNEL_OUTBOUND_REPLY_EVENT_TYPE_ASSISTANT_REPLY,
        "trigger": EXTERNAL_CHANNEL_OUTBOUND_REPLY_TRIGGER_ASYNC_RESULT_COMPLETED,
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

pub(crate) fn external_channel_outbound_reply_dispatch_body(
    payload: &Value,
) -> serde_json::Result<Vec<u8>> {
    serde_json::to_vec(payload)
}

pub(crate) fn external_channel_outbound_reply_response_summary(
    response_text: &str,
) -> (Value, Option<String>) {
    let response_json = serde_json::from_str::<Value>(response_text).ok();
    let response_summary = action_response_summary(response_json.as_ref(), response_text);
    let external_request_id = response_json
        .as_ref()
        .and_then(external_action_response_request_id);
    (response_summary, external_request_id)
}

pub(crate) fn external_channel_outbound_reply_dispatch_audit_payload(
    source_event_name: &str,
    source_event_hash: &str,
    connection_id: &str,
    response: &ExternalChannelEventResponse,
    dispatch: Value,
) -> Value {
    json!({
        "schema": EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_AUDIT_SCHEMA_V1,
        "source_event_name": source_event_name,
        "source_event_hash": source_event_hash,
        "channel_connection_id": connection_id,
        "assistant_run_id": response.assistant_run_id,
        "conversation_external_id": response.reply.target_conversation_external_id,
        "reply_type": response.reply.reply_type,
        "task_status": response.reply.task_status,
        "artifact_links": response.reply.artifact_links,
        "text_present": external_channel_outbound_reply_text_present(response.reply.text.as_deref()),
        "text_preview": external_channel_outbound_reply_text_preview(response.reply.text.as_deref()),
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
        assert_eq!(
            summary["auth_source"],
            json!(EXTERNAL_CHANNEL_OUTBOUND_REPLY_AUTH_SOURCE_REPLY_SPECIFIC)
        );
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
        assert_eq!(
            summary["auth_source"],
            json!(EXTERNAL_CHANNEL_OUTBOUND_REPLY_AUTH_SOURCE_ACTION_DISPATCH_FALLBACK)
        );
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
        assert_eq!(
            summary["auth_source"],
            json!(EXTERNAL_CHANNEL_OUTBOUND_REPLY_AUTH_SOURCE_NONE)
        );
        assert_eq!(summary["ready"], json!(false));
    }

    #[test]
    fn outbound_reply_dispatch_ready_requires_endpoint_and_auth() {
        assert!(external_channel_outbound_reply_dispatch_ready(true, true));
        assert!(!external_channel_outbound_reply_dispatch_ready(true, false));
        assert!(!external_channel_outbound_reply_dispatch_ready(false, true));
        assert!(!external_channel_outbound_reply_dispatch_ready(
            false, false
        ));
    }

    #[test]
    fn outbound_reply_dispatch_auth_source_prefers_reply_specific_then_action_fallback() {
        assert_eq!(
            external_channel_outbound_reply_dispatch_auth_source(true, true),
            EXTERNAL_CHANNEL_OUTBOUND_REPLY_AUTH_SOURCE_REPLY_SPECIFIC
        );
        assert_eq!(
            external_channel_outbound_reply_dispatch_auth_source(true, false),
            EXTERNAL_CHANNEL_OUTBOUND_REPLY_AUTH_SOURCE_REPLY_SPECIFIC
        );
        assert_eq!(
            external_channel_outbound_reply_dispatch_auth_source(false, true),
            EXTERNAL_CHANNEL_OUTBOUND_REPLY_AUTH_SOURCE_ACTION_DISPATCH_FALLBACK
        );
        assert_eq!(
            external_channel_outbound_reply_dispatch_auth_source(false, false),
            EXTERNAL_CHANNEL_OUTBOUND_REPLY_AUTH_SOURCE_NONE
        );
    }

    #[test]
    fn outbound_reply_dispatch_endpoint_configured_tracks_url_presence() {
        assert!(external_channel_outbound_reply_dispatch_endpoint_configured(Some("[redacted]")));
        assert!(
            external_channel_outbound_reply_dispatch_endpoint_configured(Some(
                "https://replies.example.com/assistant"
            ))
        );
        assert!(!external_channel_outbound_reply_dispatch_endpoint_configured(None));
    }

    #[test]
    fn outbound_reply_dispatch_endpoint_host_extracts_parseable_hosts() {
        assert_eq!(
            external_channel_outbound_reply_dispatch_endpoint_host(Some(
                "https://replies.example.com/assistant"
            )),
            Some("replies.example.com".to_string())
        );
        assert_eq!(
            external_channel_outbound_reply_dispatch_endpoint_host(Some(
                "http://127.0.0.1:3000/replies"
            )),
            Some("127.0.0.1".to_string())
        );
        assert_eq!(
            external_channel_outbound_reply_dispatch_endpoint_host(Some("[redacted]")),
            None
        );
        assert_eq!(
            external_channel_outbound_reply_dispatch_endpoint_host(None),
            None
        );
    }

    #[test]
    fn outbound_reply_text_helpers_preserve_present_flag_and_preview_budget() {
        assert!(!external_channel_outbound_reply_text_present(None));
        assert!(!external_channel_outbound_reply_text_present(Some("   ")));
        assert!(external_channel_outbound_reply_text_present(Some(
            "报表已生成"
        )));
        assert_eq!(external_channel_outbound_reply_text_preview(None), "");

        let long_text = format!("{}{}", "经营风险 ".repeat(80), "结尾");
        let preview = external_channel_outbound_reply_text_preview(Some(&long_text));
        assert!(preview.chars().count() <= 240);
        assert_ne!(preview, long_text);
    }

    #[test]
    fn outbound_reply_dispatch_http_status_success_accepts_only_2xx() {
        assert!(!external_channel_outbound_reply_dispatch_http_status_success(199));
        assert!(external_channel_outbound_reply_dispatch_http_status_success(200));
        assert!(external_channel_outbound_reply_dispatch_http_status_success(204));
        assert!(external_channel_outbound_reply_dispatch_http_status_success(299));
        assert!(!external_channel_outbound_reply_dispatch_http_status_success(300));
        assert!(!external_channel_outbound_reply_dispatch_http_status_success(500));
    }

    #[test]
    fn outbound_reply_http_dispatch_result_maps_event_status_and_payload() {
        assert_eq!(
            external_channel_outbound_reply_dispatch_event_name_for_http_status(204),
            EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_DISPATCHED_EVENT_NAME
        );
        assert_eq!(
            external_channel_outbound_reply_dispatch_event_name_for_http_status(500),
            EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_FAILED_EVENT_NAME
        );
        assert_eq!(
            external_channel_outbound_reply_dispatch_status_for_http_status(299),
            EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_STATUS_DISPATCHED
        );
        assert_eq!(
            external_channel_outbound_reply_dispatch_status_for_http_status(300),
            EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_STATUS_FAILED
        );

        let dispatched = external_channel_outbound_reply_http_dispatch_result(
            Some("reply.example.com"),
            "signature",
            200,
            json!({"accepted": true}),
            Some("reply-req-001".to_string()),
        );
        assert_eq!(
            dispatched["status"],
            json!(EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_STATUS_DISPATCHED)
        );
        assert_eq!(dispatched["endpoint_configured"], json!(true));
        assert_eq!(dispatched["endpoint_host"], json!("reply.example.com"));
        assert_eq!(dispatched["auth_mode"], json!("signature"));
        assert_eq!(dispatched["http_status"], json!(200));
        assert_eq!(dispatched["response_summary"], json!({"accepted": true}));
        assert_eq!(dispatched["external_request_id"], json!("reply-req-001"));

        let failed = external_channel_outbound_reply_http_dispatch_result(
            None,
            "bearer",
            500,
            json!({"body_preview": "failed"}),
            None,
        );
        assert_eq!(
            failed["status"],
            json!(EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_STATUS_FAILED)
        );
        assert_eq!(failed["endpoint_host"], Value::Null);
        assert_eq!(failed["auth_mode"], json!("bearer"));
        assert_eq!(failed["http_status"], json!(500));
        assert_eq!(failed["external_request_id"], Value::Null);
    }

    #[test]
    fn outbound_reply_blocked_dispatch_result_preserves_optional_fields() {
        let connection_missing = external_channel_outbound_reply_blocked_dispatch_result(
            EXTERNAL_CHANNEL_OUTBOUND_REPLY_REASON_CONNECTION_MISSING,
            None,
            None,
            None,
        );
        assert_eq!(
            connection_missing["status"],
            json!(EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_STATUS_BLOCKED)
        );
        assert_eq!(
            connection_missing["reason"],
            json!(EXTERNAL_CHANNEL_OUTBOUND_REPLY_REASON_CONNECTION_MISSING)
        );
        assert!(connection_missing.get("endpoint_configured").is_none());
        assert!(connection_missing.get("endpoint_host").is_none());
        assert!(connection_missing.get("auth_mode").is_none());

        let endpoint_missing = external_channel_outbound_reply_blocked_dispatch_result(
            EXTERNAL_CHANNEL_OUTBOUND_REPLY_REASON_ENDPOINT_MISSING,
            Some(false),
            None,
            None,
        );
        assert_eq!(endpoint_missing["endpoint_configured"], json!(false));
        assert!(endpoint_missing.get("endpoint_host").is_none());

        let auth_missing = external_channel_outbound_reply_blocked_dispatch_result(
            EXTERNAL_CHANNEL_OUTBOUND_REPLY_REASON_AUTH_MISSING,
            Some(true),
            Some("reply.example.com"),
            Some("none"),
        );
        assert_eq!(auth_missing["endpoint_configured"], json!(true));
        assert_eq!(auth_missing["endpoint_host"], json!("reply.example.com"));
        assert_eq!(auth_missing["auth_mode"], json!("none"));
        assert_eq!(
            auth_missing["reason"],
            json!(EXTERNAL_CHANNEL_OUTBOUND_REPLY_REASON_AUTH_MISSING)
        );
    }

    #[test]
    fn outbound_reply_header_blocked_dispatch_result_preserves_header_error_context() {
        let blocked = external_channel_outbound_reply_header_blocked_dispatch_result(
            "invalid_header_value:x-v3-signature",
            Some("reply.example.com"),
            "signature",
        );

        assert_eq!(
            blocked["status"],
            json!(EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_STATUS_BLOCKED)
        );
        assert_eq!(blocked["endpoint_configured"], json!(true));
        assert_eq!(blocked["endpoint_host"], json!("reply.example.com"));
        assert_eq!(blocked["auth_mode"], json!("signature"));
        assert_eq!(
            blocked["reason"],
            json!("invalid_header_value:x-v3-signature")
        );
    }

    #[test]
    fn outbound_reply_request_failed_dispatch_result_preserves_transport_error_summary() {
        let failed = external_channel_outbound_reply_request_failed_dispatch_result(
            Some("reply.example.com"),
            "signature",
            "timeout",
        );
        assert_eq!(
            failed["status"],
            json!(EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_STATUS_FAILED)
        );
        assert_eq!(failed["endpoint_configured"], json!(true));
        assert_eq!(failed["endpoint_host"], json!("reply.example.com"));
        assert_eq!(failed["auth_mode"], json!("signature"));
        assert_eq!(
            failed["reason"],
            json!(EXTERNAL_CHANNEL_OUTBOUND_REPLY_REASON_REQUEST_FAILED)
        );
        assert_eq!(failed["request_error_kind"], json!("timeout"));

        let failed_without_host =
            external_channel_outbound_reply_request_failed_dispatch_result(None, "bearer", "body");
        assert_eq!(failed_without_host["endpoint_host"], Value::Null);
        assert_eq!(failed_without_host["auth_mode"], json!("bearer"));
        assert_eq!(failed_without_host["request_error_kind"], json!("body"));
    }

    #[test]
    fn outbound_reply_dispatch_client_uses_fixed_timeout_budget() {
        assert_eq!(EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_TIMEOUT_SECS, 12);
        let _client = external_channel_outbound_reply_dispatch_client()
            .expect("outbound reply dispatch client should build");
    }

    #[test]
    fn outbound_reply_dispatch_status_from_event_name_maps_known_events() {
        assert_eq!(
            external_channel_outbound_reply_dispatch_event_names(),
            [
                EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_BLOCKED_EVENT_NAME,
                EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_FAILED_EVENT_NAME,
                EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_DISPATCHED_EVENT_NAME,
            ]
        );
        assert_eq!(
            external_channel_outbound_reply_dispatch_status_from_event_name(
                EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_BLOCKED_EVENT_NAME
            ),
            Some(EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_STATUS_BLOCKED)
        );
        assert_eq!(
            external_channel_outbound_reply_dispatch_status_from_event_name(
                EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_FAILED_EVENT_NAME
            ),
            Some(EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_STATUS_FAILED)
        );
        assert_eq!(
            external_channel_outbound_reply_dispatch_status_from_event_name(
                EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_DISPATCHED_EVENT_NAME
            ),
            Some(EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_STATUS_DISPATCHED)
        );
        assert_eq!(
            external_channel_outbound_reply_dispatch_status_from_event_name(
                "assistant_run.external_channel_static_page_publish_completed"
            ),
            None
        );
    }

    #[test]
    fn outbound_reply_terminal_source_event_check_accepts_only_publish_terminal_events() {
        assert_eq!(
            external_channel_outbound_reply_terminal_source_event_names(),
            [
                EXTERNAL_CHANNEL_OUTBOUND_REPLY_STATIC_PAGE_PUBLISH_COMPLETED_SOURCE_EVENT_NAME,
                EXTERNAL_CHANNEL_OUTBOUND_REPLY_STATIC_PAGE_PUBLISH_FAILED_SOURCE_EVENT_NAME,
            ]
        );
        assert!(external_channel_outbound_reply_is_terminal_source_event(
            EXTERNAL_CHANNEL_OUTBOUND_REPLY_STATIC_PAGE_PUBLISH_COMPLETED_SOURCE_EVENT_NAME
        ));
        assert!(external_channel_outbound_reply_is_terminal_source_event(
            EXTERNAL_CHANNEL_OUTBOUND_REPLY_STATIC_PAGE_PUBLISH_FAILED_SOURCE_EVENT_NAME
        ));
        assert!(!external_channel_outbound_reply_is_terminal_source_event(
            EXTERNAL_CHANNEL_OUTBOUND_REPLY_FIRST_TURN_FOLLOWUP_ACTION_SOURCE_EVENT_NAME
        ));
        assert!(!external_channel_outbound_reply_is_terminal_source_event(
            EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_DISPATCHED_EVENT_NAME
        ));
    }

    #[test]
    fn outbound_reply_dispatch_payload_preserves_public_reply_shape() {
        let response = sample_response_with_text(Some("报表已生成。".to_string()));
        let payload = external_channel_outbound_reply_dispatch_payload(
            &response,
            EXTERNAL_CHANNEL_OUTBOUND_REPLY_STATIC_PAGE_PUBLISH_COMPLETED_SOURCE_EVENT_NAME,
        );

        assert_eq!(
            payload["schema"],
            json!(EXTERNAL_CHANNEL_OUTBOUND_REPLY_SCHEMA_V1)
        );
        assert_eq!(
            payload["event_type"],
            json!(EXTERNAL_CHANNEL_OUTBOUND_REPLY_EVENT_TYPE_ASSISTANT_REPLY)
        );
        assert_eq!(
            payload["trigger"],
            json!(EXTERNAL_CHANNEL_OUTBOUND_REPLY_TRIGGER_ASYNC_RESULT_COMPLETED)
        );
        assert_eq!(
            payload["source_event_name"],
            json!(EXTERNAL_CHANNEL_OUTBOUND_REPLY_STATIC_PAGE_PUBLISH_COMPLETED_SOURCE_EVENT_NAME)
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
    fn outbound_reply_dispatch_body_serializes_payload_without_changing_shape() {
        let response = sample_response_with_text(Some("报表已生成。".to_string()));
        let payload = external_channel_outbound_reply_dispatch_payload(
            &response,
            EXTERNAL_CHANNEL_OUTBOUND_REPLY_STATIC_PAGE_PUBLISH_COMPLETED_SOURCE_EVENT_NAME,
        );

        let body =
            external_channel_outbound_reply_dispatch_body(&payload).expect("payload serializes");
        let parsed: Value = serde_json::from_slice(&body).expect("body parses as JSON");

        assert_eq!(parsed, payload);
    }

    #[test]
    fn outbound_reply_response_summary_extracts_safe_summary_and_request_id() {
        let (summary, external_request_id) = external_channel_outbound_reply_response_summary(
            r#"{
                "status": "accepted",
                "externalRequestId": "reply-req-001",
                "message": "ok",
                "secret": "do-not-copy"
            }"#,
        );

        assert_eq!(external_request_id.as_deref(), Some("reply-req-001"));
        assert_eq!(summary["json"]["externalRequestId"], json!("reply-req-001"));
        assert_eq!(summary["json"]["message_present"], json!(true));
        assert!(summary.to_string().contains("accepted"));
        assert!(!summary.to_string().contains("do-not-copy"));

        let (text_summary, missing_request_id) =
            external_channel_outbound_reply_response_summary("plain response body");
        assert!(missing_request_id.is_none());
        assert_eq!(text_summary["text_chars"], json!(19));
        assert_eq!(text_summary["body_redacted"], json!(true));
        assert!(!text_summary.to_string().contains("plain response body"));
    }

    #[test]
    fn outbound_reply_dispatch_audit_payload_redacts_text_to_preview() {
        let long_text = format!("{}{}", "经营风险 ".repeat(80), "结尾");
        let response = sample_response_with_text(Some(long_text.clone()));
        let audit = external_channel_outbound_reply_dispatch_audit_payload(
            EXTERNAL_CHANNEL_OUTBOUND_REPLY_STATIC_PAGE_PUBLISH_COMPLETED_SOURCE_EVENT_NAME,
            "event-hash-001",
            "generic-chat-main",
            &response,
            json!({
                "status": EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_STATUS_DISPATCHED,
                "http_status": 200,
                "external_request_id": "reply-req-001",
            }),
        );

        assert_eq!(
            audit["schema"],
            json!(EXTERNAL_CHANNEL_OUTBOUND_REPLY_DISPATCH_AUDIT_SCHEMA_V1)
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
