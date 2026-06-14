use chrono::{DateTime, Utc};
use contracts::{
    ExternalIntegrationActionDispatchConfigRequest, ExternalIntegrationReplyDispatchConfigRequest,
};
use serde_json::{json, Value};

use crate::{
    external_channel_support::{
        external_action_dispatch_auth_configured, external_action_dispatch_auth_from_config,
        external_channel_outbound_reply_dispatch_auth_from_config, external_control_reason_present,
        remove_payload_keys, validate_external_action_dispatch_secret,
        validate_external_action_dispatch_url, validate_external_reply_dispatch_secret,
        validate_external_reply_dispatch_url,
    },
    static_page_payload_support::{ensure_json_object, set_payload_value},
    text_normalization::non_empty_trimmed_string,
    ApiError,
};

#[derive(Debug)]
pub(crate) struct PreparedExternalReplyDispatchConfig {
    pub(crate) config_redacted: Value,
    pub(crate) cleared: bool,
}

#[derive(Debug)]
pub(crate) struct PreparedExternalActionDispatchConfig {
    pub(crate) config_redacted: Value,
    pub(crate) cleared: bool,
}

const EXTERNAL_REPLY_DISPATCH_URL_KEYS: &[&str] = &[
    "external_reply_dispatch_url",
    "externalReplyDispatchUrl",
    "reply_dispatch_url",
    "replyDispatchUrl",
    "outbound_reply_url",
    "outboundReplyUrl",
    "assistant_reply_dispatch_url",
    "assistantReplyDispatchUrl",
];

const EXTERNAL_REPLY_DISPATCH_BEARER_TOKEN_KEYS: &[&str] = &[
    "external_reply_bearer_token",
    "externalReplyBearerToken",
    "reply_dispatch_bearer_token",
    "replyDispatchBearerToken",
    "outbound_reply_bearer_token",
    "outboundReplyBearerToken",
];

const EXTERNAL_REPLY_DISPATCH_SIGNING_SECRET_KEYS: &[&str] = &[
    "external_reply_signing_secret",
    "externalReplySigningSecret",
    "reply_dispatch_signing_secret",
    "replyDispatchSigningSecret",
    "outbound_reply_signing_secret",
    "outboundReplySigningSecret",
];

const EXTERNAL_ACTION_DISPATCH_URL_KEYS: &[&str] = &[
    "artifact_action_dispatch_url",
    "artifactActionDispatchUrl",
    "artifact_dispatch_url",
    "artifactDispatchUrl",
    "business_action_dispatch_url",
    "businessActionDispatchUrl",
    "business_dispatch_url",
    "businessDispatchUrl",
    "external_action_dispatch_url",
    "externalActionDispatchUrl",
    "action_dispatch_url",
    "actionDispatchUrl",
];

const EXTERNAL_ACTION_DISPATCH_BEARER_TOKEN_KEYS: &[&str] = &[
    "artifact_action_bearer_token",
    "artifactActionBearerToken",
    "artifact_bearer_token",
    "artifactBearerToken",
    "business_action_bearer_token",
    "businessActionBearerToken",
    "business_bearer_token",
    "businessBearerToken",
    "external_action_bearer_token",
    "externalActionBearerToken",
    "action_bearer_token",
    "actionBearerToken",
    "dispatch_bearer_token",
    "dispatchBearerToken",
];

const EXTERNAL_ACTION_DISPATCH_SIGNING_SECRET_KEYS: &[&str] = &[
    "artifact_action_signing_secret",
    "artifactActionSigningSecret",
    "artifact_signing_secret",
    "artifactSigningSecret",
    "business_action_signing_secret",
    "businessActionSigningSecret",
    "business_signing_secret",
    "businessSigningSecret",
    "external_action_signing_secret",
    "externalActionSigningSecret",
    "action_signing_secret",
    "actionSigningSecret",
    "dispatch_signing_secret",
    "dispatchSigningSecret",
];

pub(crate) fn apply_external_reply_dispatch_config(
    mut config: Value,
    request: &ExternalIntegrationReplyDispatchConfigRequest,
    now: DateTime<Utc>,
) -> std::result::Result<PreparedExternalReplyDispatchConfig, ApiError> {
    ensure_json_object(&mut config);
    let reason_present = external_control_reason_present(request.reason.as_deref());
    if request.clear_reply_dispatch.unwrap_or(false) {
        remove_payload_keys(&mut config, EXTERNAL_REPLY_DISPATCH_URL_KEYS);
        remove_payload_keys(&mut config, EXTERNAL_REPLY_DISPATCH_BEARER_TOKEN_KEYS);
        remove_payload_keys(&mut config, EXTERNAL_REPLY_DISPATCH_SIGNING_SECRET_KEYS);
        set_payload_value(
            &mut config,
            "management_control",
            json!({
                "last_action": "clear_reply_dispatch",
                "reason_present": reason_present,
                "updated_at": now,
                "secret_material_included": false,
            }),
        );
        return Ok(PreparedExternalReplyDispatchConfig {
            config_redacted: config,
            cleared: true,
        });
    }

    let Some(dispatch_url) = request
        .reply_dispatch_url
        .as_deref()
        .and_then(non_empty_trimmed_string)
    else {
        return Err(ApiError::bad_request(
            "external_reply_dispatch_url_required",
            "reply_dispatch_url is required unless clear_reply_dispatch is true".to_string(),
        ));
    };
    validate_external_reply_dispatch_url(&dispatch_url)?;
    let bearer_token = validate_external_reply_dispatch_secret(
        "reply_dispatch_bearer_token",
        request.reply_dispatch_bearer_token.as_deref(),
    )?;
    let signing_secret = validate_external_reply_dispatch_secret(
        "reply_dispatch_signing_secret",
        request.reply_dispatch_signing_secret.as_deref(),
    )?;
    let secret_material_included = bearer_token.is_some() || signing_secret.is_some();

    remove_payload_keys(&mut config, EXTERNAL_REPLY_DISPATCH_URL_KEYS);
    set_payload_value(&mut config, "reply_dispatch_url", json!(dispatch_url));
    if let Some(token) = bearer_token {
        remove_payload_keys(&mut config, EXTERNAL_REPLY_DISPATCH_BEARER_TOKEN_KEYS);
        set_payload_value(&mut config, "reply_dispatch_bearer_token", json!(token));
    }
    if let Some(secret) = signing_secret {
        remove_payload_keys(&mut config, EXTERNAL_REPLY_DISPATCH_SIGNING_SECRET_KEYS);
        set_payload_value(&mut config, "reply_dispatch_signing_secret", json!(secret));
    }

    let auth = external_channel_outbound_reply_dispatch_auth_from_config(&config);
    if !external_action_dispatch_auth_configured(&auth) {
        return Err(ApiError::bad_request(
            "external_reply_dispatch_auth_required",
            "reply_dispatch_bearer_token or reply_dispatch_signing_secret is required before outbound replies can be sent"
                .to_string(),
        ));
    }

    set_payload_value(
        &mut config,
        "management_control",
        json!({
            "last_action": "configure_reply_dispatch",
            "reason_present": reason_present,
            "updated_at": now,
            "secret_material_included": secret_material_included,
        }),
    );
    set_payload_value(&mut config, "reply_dispatch_updated_at", json!(now));

    Ok(PreparedExternalReplyDispatchConfig {
        config_redacted: config,
        cleared: false,
    })
}

pub(crate) fn apply_external_action_dispatch_config(
    mut config: Value,
    request: &ExternalIntegrationActionDispatchConfigRequest,
    now: DateTime<Utc>,
) -> std::result::Result<PreparedExternalActionDispatchConfig, ApiError> {
    ensure_json_object(&mut config);
    let reason_present = external_control_reason_present(request.reason.as_deref());
    if request.clear_action_dispatch.unwrap_or(false) {
        remove_payload_keys(&mut config, EXTERNAL_ACTION_DISPATCH_URL_KEYS);
        remove_payload_keys(&mut config, EXTERNAL_ACTION_DISPATCH_BEARER_TOKEN_KEYS);
        remove_payload_keys(&mut config, EXTERNAL_ACTION_DISPATCH_SIGNING_SECRET_KEYS);
        set_payload_value(
            &mut config,
            "management_control",
            json!({
                "last_action": "clear_action_dispatch",
                "reason_present": reason_present,
                "updated_at": now,
                "secret_material_included": false,
            }),
        );
        return Ok(PreparedExternalActionDispatchConfig {
            config_redacted: config,
            cleared: true,
        });
    }

    let Some(dispatch_url) = request
        .action_dispatch_url
        .as_deref()
        .and_then(non_empty_trimmed_string)
    else {
        return Err(ApiError::bad_request(
            "external_action_dispatch_url_required",
            "action_dispatch_url is required unless clear_action_dispatch is true".to_string(),
        ));
    };
    validate_external_action_dispatch_url(&dispatch_url)?;
    let bearer_token = validate_external_action_dispatch_secret(
        "action_bearer_token",
        request.action_bearer_token.as_deref(),
    )?;
    let signing_secret = validate_external_action_dispatch_secret(
        "action_signing_secret",
        request.action_signing_secret.as_deref(),
    )?;
    let secret_material_included = bearer_token.is_some() || signing_secret.is_some();

    remove_payload_keys(&mut config, EXTERNAL_ACTION_DISPATCH_URL_KEYS);
    set_payload_value(
        &mut config,
        "external_action_dispatch_url",
        json!(dispatch_url),
    );
    if let Some(token) = bearer_token {
        remove_payload_keys(&mut config, EXTERNAL_ACTION_DISPATCH_BEARER_TOKEN_KEYS);
        set_payload_value(&mut config, "dispatch_bearer_token", json!(token));
    }
    if let Some(secret) = signing_secret {
        remove_payload_keys(&mut config, EXTERNAL_ACTION_DISPATCH_SIGNING_SECRET_KEYS);
        set_payload_value(&mut config, "dispatch_signing_secret", json!(secret));
    }

    let auth = external_action_dispatch_auth_from_config(&config);
    if !external_action_dispatch_auth_configured(&auth) {
        return Err(ApiError::bad_request(
            "external_action_dispatch_auth_required",
            "action_bearer_token or action_signing_secret is required before external actions can be dispatched"
                .to_string(),
        ));
    }

    set_payload_value(
        &mut config,
        "management_control",
        json!({
            "last_action": "configure_action_dispatch",
            "reason_present": reason_present,
            "updated_at": now,
            "secret_material_included": secret_material_included,
        }),
    );
    set_payload_value(&mut config, "action_dispatch_updated_at", json!(now));

    Ok(PreparedExternalActionDispatchConfig {
        config_redacted: config,
        cleared: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        external_channel_outbound_reply_dispatch_support::external_channel_outbound_reply_dispatch_summary,
        external_channel_support::{
            external_action_dispatch_auth_from_config, external_action_dispatch_url_from_config,
        },
    };

    #[test]
    fn reply_dispatch_config_preserves_existing_auth_when_url_changes() {
        let now = Utc::now();
        let prepared = apply_external_reply_dispatch_config(
            json!({
                "reply_dispatch_url": "https://old.example.com/replies",
                "reply_dispatch_bearer_token": "existing-token",
                "reply_dispatch_signing_secret": "existing-secret"
            }),
            &ExternalIntegrationReplyDispatchConfigRequest {
                reason: Some("operator_update_url".to_string()),
                reply_dispatch_url: Some("https://new.example.com/replies".to_string()),
                reply_dispatch_bearer_token: None,
                reply_dispatch_signing_secret: None,
                clear_reply_dispatch: None,
            },
            now,
        )
        .expect("reply dispatch config should apply");
        let config = prepared.config_redacted;

        assert!(!prepared.cleared);
        assert_eq!(
            config["reply_dispatch_url"],
            json!("https://new.example.com/replies")
        );
        assert_eq!(
            config["reply_dispatch_bearer_token"],
            json!("existing-token")
        );
        assert_eq!(
            config["reply_dispatch_signing_secret"],
            json!("existing-secret")
        );
        assert_eq!(
            config["management_control"]["last_action"],
            json!("configure_reply_dispatch")
        );
        assert_eq!(
            config["management_control"]["secret_material_included"],
            json!(false)
        );
        assert_eq!(
            external_channel_outbound_reply_dispatch_summary(&config)["ready"],
            json!(true)
        );
    }

    #[test]
    fn reply_dispatch_config_writes_canonical_secret_fields_without_echoing_reason() {
        let now = Utc::now();
        let prepared = apply_external_reply_dispatch_config(
            json!({
                "externalReplyDispatchUrl": "https://old.example.com/replies",
                "externalReplyBearerToken": "old-token",
                "externalReplySigningSecret": "old-secret"
            }),
            &ExternalIntegrationReplyDispatchConfigRequest {
                reason: Some("operator supplied secret material".to_string()),
                reply_dispatch_url: Some("https://new.example.com/replies".to_string()),
                reply_dispatch_bearer_token: Some("new-token".to_string()),
                reply_dispatch_signing_secret: Some("new-secret".to_string()),
                clear_reply_dispatch: None,
            },
            now,
        )
        .expect("reply dispatch config should apply");
        let config = prepared.config_redacted;
        let text = config.to_string();

        assert_eq!(
            config["reply_dispatch_url"],
            json!("https://new.example.com/replies")
        );
        assert_eq!(config["reply_dispatch_bearer_token"], json!("new-token"));
        assert_eq!(config["reply_dispatch_signing_secret"], json!("new-secret"));
        assert!(config.get("externalReplyDispatchUrl").is_none());
        assert!(config.get("externalReplyBearerToken").is_none());
        assert!(config.get("externalReplySigningSecret").is_none());
        assert_eq!(
            config["management_control"]["secret_material_included"],
            json!(true)
        );
        assert!(!text.contains("operator supplied secret material"));
    }

    #[test]
    fn reply_dispatch_config_clear_removes_reply_specific_fields() {
        let now = Utc::now();
        let prepared = apply_external_reply_dispatch_config(
            json!({
                "reply_dispatch_url": "https://old.example.com/replies",
                "reply_dispatch_bearer_token": "existing-token",
                "reply_dispatch_signing_secret": "existing-secret",
                "dispatch_bearer_token": "action-token"
            }),
            &ExternalIntegrationReplyDispatchConfigRequest {
                reason: Some("operator_clear".to_string()),
                reply_dispatch_url: None,
                reply_dispatch_bearer_token: None,
                reply_dispatch_signing_secret: None,
                clear_reply_dispatch: Some(true),
            },
            now,
        )
        .expect("reply dispatch config should clear");
        let config = prepared.config_redacted;

        assert!(prepared.cleared);
        assert!(config.get("reply_dispatch_url").is_none());
        assert!(config.get("reply_dispatch_bearer_token").is_none());
        assert!(config.get("reply_dispatch_signing_secret").is_none());
        assert_eq!(config["dispatch_bearer_token"], json!("action-token"));
        assert_eq!(
            external_channel_outbound_reply_dispatch_summary(&config)["ready"],
            json!(false)
        );
        assert_eq!(
            config["management_control"]["last_action"],
            json!("clear_reply_dispatch")
        );
    }

    #[test]
    fn reply_dispatch_config_requires_auth() {
        let error = apply_external_reply_dispatch_config(
            json!({}),
            &ExternalIntegrationReplyDispatchConfigRequest {
                reason: None,
                reply_dispatch_url: Some("https://new.example.com/replies".to_string()),
                reply_dispatch_bearer_token: None,
                reply_dispatch_signing_secret: None,
                clear_reply_dispatch: None,
            },
            Utc::now(),
        )
        .expect_err("auth should be required");

        assert_eq!(error.payload.code, "external_reply_dispatch_auth_required");
    }

    #[test]
    fn action_dispatch_config_writes_canonical_endpoint_and_credentials() {
        let now = Utc::now();
        let prepared = apply_external_action_dispatch_config(
            json!({
                "actionDispatchUrl": "https://old.example.com/actions",
                "actionBearerToken": "old-token",
                "actionSigningSecret": "old-secret"
            }),
            &ExternalIntegrationActionDispatchConfigRequest {
                reason: Some("configure aigolf callbacks".to_string()),
                action_dispatch_url: Some("https://api.example.com/v3/actions".to_string()),
                action_bearer_token: Some("new-action-token".to_string()),
                action_signing_secret: Some("new-action-secret".to_string()),
                clear_action_dispatch: None,
            },
            now,
        )
        .expect("action dispatch config should apply");
        let config = prepared.config_redacted;
        let auth = external_action_dispatch_auth_from_config(&config);
        let text = config.to_string();

        assert!(!prepared.cleared);
        assert_eq!(
            external_action_dispatch_url_from_config(&config, "external_business_action.invoke")
                .as_deref(),
            Some("https://api.example.com/v3/actions")
        );
        assert_eq!(auth.bearer_token.as_deref(), Some("new-action-token"));
        assert_eq!(auth.signing_secret.as_deref(), Some("new-action-secret"));
        assert!(config.get("actionDispatchUrl").is_none());
        assert!(config.get("actionBearerToken").is_none());
        assert!(config.get("actionSigningSecret").is_none());
        assert_eq!(
            config["management_control"]["last_action"],
            json!("configure_action_dispatch")
        );
        assert_eq!(
            config["management_control"]["secret_material_included"],
            json!(true)
        );
        assert!(!text.contains("configure aigolf callbacks"));
    }

    #[test]
    fn action_dispatch_config_clear_preserves_reply_dispatch_fields() {
        let now = Utc::now();
        let prepared = apply_external_action_dispatch_config(
            json!({
                "external_action_dispatch_url": "https://old.example.com/actions",
                "dispatch_bearer_token": "action-token",
                "dispatch_signing_secret": "action-secret",
                "reply_dispatch_url": "https://reply.example.com/replies",
                "reply_dispatch_bearer_token": "reply-token"
            }),
            &ExternalIntegrationActionDispatchConfigRequest {
                reason: Some("clear action only".to_string()),
                action_dispatch_url: None,
                action_bearer_token: None,
                action_signing_secret: None,
                clear_action_dispatch: Some(true),
            },
            now,
        )
        .expect("action dispatch config should clear");
        let config = prepared.config_redacted;

        assert!(prepared.cleared);
        assert!(config.get("external_action_dispatch_url").is_none());
        assert!(config.get("dispatch_bearer_token").is_none());
        assert!(config.get("dispatch_signing_secret").is_none());
        assert_eq!(
            config["reply_dispatch_url"],
            json!("https://reply.example.com/replies")
        );
        assert_eq!(config["reply_dispatch_bearer_token"], json!("reply-token"));
        assert_eq!(
            config["management_control"]["last_action"],
            json!("clear_action_dispatch")
        );
    }

    #[test]
    fn action_dispatch_config_requires_auth() {
        let error = apply_external_action_dispatch_config(
            json!({}),
            &ExternalIntegrationActionDispatchConfigRequest {
                reason: None,
                action_dispatch_url: Some("https://api.example.com/v3/actions".to_string()),
                action_bearer_token: None,
                action_signing_secret: None,
                clear_action_dispatch: None,
            },
            Utc::now(),
        )
        .expect_err("auth should be required");

        assert_eq!(error.payload.code, "external_action_dispatch_auth_required");
    }
}
