use serde_json::{json, Value};

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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
}
