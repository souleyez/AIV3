use serde_json::{json, Map, Value};

use crate::external_channel_outbound_reply_dispatch_support::external_channel_outbound_reply_dispatch_summary;
use crate::external_channel_support::{
    external_action_dispatch_auth_from_config, external_action_dispatch_auth_mode,
    external_action_dispatch_url_from_config, external_channel_inbound_bearer_token_from_config,
    external_channel_temporary_access_summary,
};
use crate::external_database_source_config_support::external_database_source_config_summary;
use crate::external_integration_summary::has_redacted_value;

pub(crate) fn external_integration_config_summary(config: &Value) -> Value {
    let dispatch_auth = external_action_dispatch_auth_from_config(config);
    let inbound_auth_configured =
        external_channel_inbound_bearer_token_from_config(config).is_some();
    json!({
        "key_count": config.as_object().map(Map::len).unwrap_or(0),
        "redacted_value_present": has_redacted_value(config),
        "inbound_auth_mode": if inbound_auth_configured { "bearer" } else { "none" },
        "inbound_auth_configured": inbound_auth_configured,
        "temporary_access": external_channel_temporary_access_summary(config),
        "dispatch_endpoint_configured": external_action_dispatch_url_from_config(config, "external_business_action.invoke").is_some()
            || external_action_dispatch_url_from_config(config, "external_artifact.publish").is_some(),
        "dispatch_auth_mode": external_action_dispatch_auth_mode(&dispatch_auth),
        "platform_callback_token_configured": external_config_string(
            config,
            &[
                "token",
                "callback_token",
                "callbackToken",
                "verification_token",
                "verificationToken",
            ],
        )
        .is_some(),
        "outbound_reply_dispatch": external_channel_outbound_reply_dispatch_summary(config),
        "database_source": external_database_source_config_summary(config),
    })
}

fn external_config_string(config: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        config
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty() && !value.starts_with("[redacted"))
            .map(ToString::to_string)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn integration_config_summary_does_not_expose_secret_values() {
        let summary = external_integration_config_summary(&json!({
            "action_dispatch_url": "https://api.example.com/actions",
            "dispatch_bearer_token": "dispatch-token",
            "reply_dispatch_url": "https://api.example.com/replies",
            "reply_dispatch_signing_secret": "reply-secret",
            "token": "callback-token",
            "inbound_bearer_token": "inbound-token",
            "temporary_access": {
                "enabled": true,
                "ttl_seconds": 3600
            },
            "nested": {
                "signing_secret": "hidden"
            }
        }));
        let summary_text = summary.to_string();

        assert_eq!(summary["dispatch_endpoint_configured"], json!(true));
        assert_eq!(summary["dispatch_auth_mode"], json!("bearer"));
        assert_eq!(summary["inbound_auth_mode"], json!("bearer"));
        assert_eq!(summary["inbound_auth_configured"], json!(true));
        assert_eq!(
            summary["outbound_reply_dispatch"]["endpoint_configured"],
            json!(true)
        );
        assert_eq!(
            summary["outbound_reply_dispatch"]["auth_mode"],
            json!("signature_and_bearer")
        );
        assert_eq!(
            summary["outbound_reply_dispatch"]["auth_source"],
            json!("reply_specific")
        );
        assert_eq!(summary["outbound_reply_dispatch"]["ready"], json!(true));
        assert_eq!(summary["platform_callback_token_configured"], json!(true));
        assert_eq!(summary["redacted_value_present"], json!(false));
        assert!(!summary_text.contains("dispatch-token"));
        assert!(!summary_text.contains("reply-secret"));
        assert!(!summary_text.contains("callback-token"));
        assert!(!summary_text.contains("inbound-token"));
        assert!(!summary_text.contains("hidden"));
    }

    #[test]
    fn integration_config_summary_treats_redacted_or_empty_callback_tokens_as_absent() {
        let summary = external_integration_config_summary(&json!({
            "token": "[redacted]",
            "callback_token": "  ",
            "verificationToken": "\t",
            "inbound_bearer_token": "[redacted]",
        }));

        assert_eq!(summary["redacted_value_present"], json!(true));
        assert_eq!(summary["platform_callback_token_configured"], json!(false));
        assert_eq!(summary["inbound_auth_mode"], json!("none"));
        assert_eq!(summary["inbound_auth_configured"], json!(false));
    }

    #[test]
    fn integration_config_summary_embeds_database_source_and_reply_dispatch_summaries() {
        let summary = external_integration_config_summary(&json!({
            "reply_dispatch_url": "https://api.example.com/replies",
            "dispatch_bearer_token": "dispatch-token",
            "databaseSource": {
                "connection_env": "THIRD_PARTY_HY_SQL_DATABASE_URL",
                "database": "hy_sql",
                "default_dataset_id": "018f0000-0000-7000-9000-000000000001",
                "tables": [{
                    "table": "bi_contract_warning",
                    "id_column": "id",
                    "content_columns": ["brand_name"]
                }]
            }
        }));

        assert_eq!(
            summary["outbound_reply_dispatch"]["auth_source"],
            json!("action_dispatch_fallback")
        );
        assert_eq!(summary["outbound_reply_dispatch"]["ready"], json!(true));
        assert_eq!(summary["database_source"]["kind"], json!("mysql"));
        assert_eq!(summary["database_source"]["database"], json!("hy_sql"));
        assert_eq!(
            summary["database_source"]["tables"][0],
            json!("bi_contract_warning")
        );
        assert!(!summary.to_string().contains("dispatch-token"));
    }
}
