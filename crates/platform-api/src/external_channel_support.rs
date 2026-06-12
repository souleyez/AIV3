use crate::ApiError;
use chrono::{DateTime, Utc};
use contracts::{ExternalChannelPlatformView, ExternalMessageTypeView};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use uuid::Uuid;

use super::{
    collect_external_config_string_values, external_config_string, remove_payload_keys,
    set_payload_value, ExternalChannelConnectionSummary,
};
use crate::text_normalization::non_empty_trimmed_string;

#[derive(Clone, Debug, Default)]
pub(crate) struct ExternalActionDispatchAuth {
    pub(crate) bearer_token: Option<String>,
    pub(crate) signing_secret: Option<String>,
}

pub(crate) fn external_action_dispatch_auth_from_config(
    config: &Value,
) -> ExternalActionDispatchAuth {
    ExternalActionDispatchAuth {
        bearer_token: external_config_string(
            config,
            &[
                "external_action_bearer_token",
                "externalActionBearerToken",
                "action_bearer_token",
                "actionBearerToken",
                "dispatch_bearer_token",
                "dispatchBearerToken",
            ],
        ),
        signing_secret: external_config_string(
            config,
            &[
                "external_action_signing_secret",
                "externalActionSigningSecret",
                "action_signing_secret",
                "actionSigningSecret",
                "dispatch_signing_secret",
                "dispatchSigningSecret",
            ],
        ),
    }
}

pub(crate) fn external_channel_reply_specific_dispatch_auth_from_config(
    config: &Value,
) -> ExternalActionDispatchAuth {
    ExternalActionDispatchAuth {
        bearer_token: external_config_string(
            config,
            &[
                "external_reply_bearer_token",
                "externalReplyBearerToken",
                "reply_dispatch_bearer_token",
                "replyDispatchBearerToken",
                "outbound_reply_bearer_token",
                "outboundReplyBearerToken",
            ],
        ),
        signing_secret: external_config_string(
            config,
            &[
                "external_reply_signing_secret",
                "externalReplySigningSecret",
                "reply_dispatch_signing_secret",
                "replyDispatchSigningSecret",
                "outbound_reply_signing_secret",
                "outboundReplySigningSecret",
            ],
        ),
    }
}

pub(crate) fn external_channel_outbound_reply_dispatch_auth_from_config(
    config: &Value,
) -> ExternalActionDispatchAuth {
    let reply_auth = external_channel_reply_specific_dispatch_auth_from_config(config);
    let action_auth = external_action_dispatch_auth_from_config(config);
    ExternalActionDispatchAuth {
        bearer_token: reply_auth.bearer_token.or(action_auth.bearer_token),
        signing_secret: reply_auth.signing_secret.or(action_auth.signing_secret),
    }
}

pub(crate) fn external_action_dispatch_auth_configured(auth: &ExternalActionDispatchAuth) -> bool {
    auth.bearer_token.is_some() || auth.signing_secret.is_some()
}

pub(crate) fn external_control_reason_present(reason: Option<&str>) -> bool {
    reason.map(str::trim).is_some_and(|value| !value.is_empty())
}

pub(crate) fn external_control_integration_kind(channel_count: u64, source_count: u64) -> String {
    match (channel_count > 0, source_count > 0) {
        (true, true) => "mixed".to_string(),
        (true, false) => "channel".to_string(),
        (false, true) => "source".to_string(),
        (false, false) => "unknown".to_string(),
    }
}

pub(crate) fn external_control_config_patch(
    action: &str,
    reason_present: bool,
    now: DateTime<Utc>,
) -> Value {
    json!({
        "management_control": {
            "last_action": action,
            "reason_present": reason_present,
            "updated_at": now,
            "secret_material_included": false,
        }
    })
}

pub(crate) fn validate_external_channel_connection_id(
    value: &str,
) -> std::result::Result<(), ApiError> {
    let trimmed = value.trim();
    if trimmed.len() < 3 || trimmed.len() > 96 {
        return Err(ApiError::bad_request(
            "external_channel_connection_id_invalid",
            "connection_id must be 3-96 characters".to_string(),
        ));
    }
    if !trimmed
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.'))
    {
        return Err(ApiError::bad_request(
            "external_channel_connection_id_invalid",
            "connection_id may only contain ASCII letters, numbers, dot, dash, or underscore"
                .to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn validate_external_channel_platform(value: &str) -> std::result::Result<(), ApiError> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.len() > 64
        || !trimmed
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
    {
        return Err(ApiError::bad_request(
            "external_channel_platform_invalid",
            "platform must be ASCII text within 64 characters".to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn validate_external_database_source_id(
    value: &str,
) -> std::result::Result<String, ApiError> {
    let value = non_empty_trimmed_string(value).ok_or_else(|| {
        ApiError::bad_request(
            "validation_error",
            "source_external_id must not be empty".to_string(),
        )
    })?;
    if value.chars().count() > 128
        || value
            .chars()
            .any(|ch| ch.is_control() || matches!(ch, '/' | '\\' | '?' | '#'))
    {
        return Err(ApiError::bad_request(
            "validation_error",
            "source_external_id must be printable text within 128 characters and must not contain path separators".to_string(),
        ));
    }
    Ok(value)
}

pub(crate) fn validate_external_channel_allowed_database_source_ids(
    values: &[String],
) -> std::result::Result<Vec<String>, ApiError> {
    let mut ids = BTreeSet::new();
    for value in values {
        ids.insert(validate_external_database_source_id(value)?);
    }
    Ok(ids.into_iter().collect())
}

pub(crate) fn external_channel_inbound_bearer_token_from_config(config: &Value) -> Option<String> {
    external_config_string(
        config,
        &[
            "inbound_bearer_token",
            "inboundBearerToken",
            "inbound_token",
            "inboundToken",
            "external_channel_bearer_token",
            "externalChannelBearerToken",
            "callback_bearer_token",
            "callbackBearerToken",
            "generic_chat_inbound_bearer_token",
            "genericChatInboundBearerToken",
        ],
    )
}

pub(crate) fn external_channel_default_source_id_from_config(config: &Value) -> Option<String> {
    external_config_string(
        config,
        &[
            "default_source_id",
            "defaultSourceId",
            "source_id",
            "sourceId",
            "external_source_id",
            "externalSourceId",
        ],
    )
}

pub(crate) fn new_external_channel_inbound_token() -> String {
    format!(
        "v3in_{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    )
}

pub(crate) fn remove_external_channel_inbound_token_keys(config: &mut Value) {
    remove_payload_keys(
        config,
        &[
            "inbound_bearer_token",
            "inboundBearerToken",
            "inbound_token",
            "inboundToken",
            "external_channel_bearer_token",
            "externalChannelBearerToken",
            "callback_bearer_token",
            "callbackBearerToken",
            "generic_chat_inbound_bearer_token",
            "genericChatInboundBearerToken",
            "inbound_bearer_token_expires_at",
            "inboundBearerTokenExpiresAt",
            "inbound_token_rotated_at",
            "inboundTokenRotatedAt",
        ],
    );
}

pub(crate) fn sanitize_cloned_external_channel_config(config: &mut Value) {
    remove_external_channel_inbound_token_keys(config);
    remove_payload_keys(
        config,
        &[
            "reply_dispatch_url",
            "replyDispatchUrl",
            "external_reply_dispatch_url",
            "externalReplyDispatchUrl",
            "outbound_reply_url",
            "outboundReplyUrl",
            "assistant_reply_dispatch_url",
            "assistantReplyDispatchUrl",
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
            "artifact_action_bearer_token",
            "artifactActionBearerToken",
            "artifact_bearer_token",
            "artifactBearerToken",
            "artifact_action_signing_secret",
            "artifactActionSigningSecret",
            "artifact_signing_secret",
            "artifactSigningSecret",
            "business_action_bearer_token",
            "businessActionBearerToken",
            "business_bearer_token",
            "businessBearerToken",
            "business_action_signing_secret",
            "businessActionSigningSecret",
            "business_signing_secret",
            "businessSigningSecret",
            "external_action_bearer_token",
            "externalActionBearerToken",
            "action_bearer_token",
            "actionBearerToken",
            "external_action_signing_secret",
            "externalActionSigningSecret",
            "action_signing_secret",
            "actionSigningSecret",
            "reply_dispatch_bearer_token",
            "replyDispatchBearerToken",
            "external_reply_bearer_token",
            "externalReplyBearerToken",
            "outbound_reply_bearer_token",
            "outboundReplyBearerToken",
            "reply_dispatch_signing_secret",
            "replyDispatchSigningSecret",
            "external_reply_signing_secret",
            "externalReplySigningSecret",
            "outbound_reply_signing_secret",
            "outboundReplySigningSecret",
            "dispatch_bearer_token",
            "dispatchBearerToken",
            "dispatch_signing_secret",
            "dispatchSigningSecret",
            "management_control",
            "temporary_access",
            "channel_management",
        ],
    );
}

pub(crate) fn apply_external_channel_inbound_token_config(
    config: &mut Value,
    token: &str,
    now: DateTime<Utc>,
    temporary: bool,
    expires_at: Option<DateTime<Utc>>,
) {
    set_payload_value(config, "inbound_bearer_token", json!(token));
    set_payload_value(config, "inbound_token_rotated_at", json!(now));
    if let Some(expires_at) = expires_at {
        set_payload_value(config, "inbound_bearer_token_expires_at", json!(expires_at));
    }
    set_payload_value(
        config,
        "temporary_access",
        json!({
            "temporary": temporary,
            "expires_at": expires_at,
            "updated_at": now,
        }),
    );
}

pub(crate) fn external_channel_inbound_token_rotated_at(config: &Value) -> Option<String> {
    external_config_string(
        config,
        &["inbound_token_rotated_at", "inboundTokenRotatedAt"],
    )
}

pub(crate) fn external_channel_inbound_token_expires_at(config: &Value) -> Option<DateTime<Utc>> {
    external_config_string(
        config,
        &[
            "inbound_bearer_token_expires_at",
            "inboundBearerTokenExpiresAt",
        ],
    )
    .and_then(|value| DateTime::parse_from_rfc3339(&value).ok())
    .map(|value| value.with_timezone(&Utc))
}

pub(crate) fn external_channel_inbound_token_expired(config: &Value, now: DateTime<Utc>) -> bool {
    external_channel_inbound_token_expires_at(config).is_some_and(|expires_at| expires_at <= now)
}

pub(crate) fn external_channel_temporary_access_summary(config: &Value) -> Value {
    let temporary_access = config
        .get("temporary_access")
        .or_else(|| config.get("temporaryAccess"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let temporary = temporary_access
        .get("temporary")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    json!({
        "temporary": temporary,
        "expires_at": external_channel_inbound_token_expires_at(config),
        "expired": external_channel_inbound_token_expired(config, Utc::now()),
        "token_rotated_at": external_channel_inbound_token_rotated_at(config),
    })
}

pub(crate) fn ensure_external_channel_database_source_allowed(
    connection: &ExternalChannelConnectionSummary,
    source_id: &str,
) -> std::result::Result<(), ApiError> {
    if external_channel_database_source_allowed(&connection.config_redacted, source_id) {
        return Ok(());
    }
    Err(ApiError::forbidden(
        "database_source_not_allowed",
        "database source is not allowed for this external channel".to_string(),
    ))
}

pub(crate) fn external_channel_database_source_allowed(config: &Value, source_id: &str) -> bool {
    let Some(source_id) = non_empty_trimmed_string(source_id) else {
        return false;
    };
    if external_channel_default_source_id_from_config(config)
        .as_deref()
        .is_some_and(|default_source_id| default_source_id == source_id)
    {
        return true;
    }
    external_channel_allowed_database_source_ids(config)
        .iter()
        .any(|allowed| allowed == &source_id)
}

pub(crate) fn external_channel_allowed_database_source_ids(config: &Value) -> BTreeSet<String> {
    let mut source_ids = BTreeSet::new();
    for key in [
        "allowed_database_source_ids",
        "allowedDatabaseSourceIds",
        "database_source_ids",
        "databaseSourceIds",
        "allowed_source_ids",
        "allowedSourceIds",
    ] {
        collect_external_config_string_values(config.get(key), &mut source_ids);
    }
    if let Some(database_sources) = config
        .get("database_sources")
        .or_else(|| config.get("databaseSources"))
        .and_then(Value::as_array)
    {
        for source in database_sources {
            match source {
                Value::String(value) => {
                    if let Some(value) = non_empty_trimmed_string(value) {
                        source_ids.insert(value);
                    }
                }
                Value::Object(object) => {
                    for key in [
                        "source_external_id",
                        "sourceExternalId",
                        "source_id",
                        "sourceId",
                    ] {
                        if let Some(value) = object
                            .get(key)
                            .and_then(Value::as_str)
                            .and_then(non_empty_trimmed_string)
                        {
                            source_ids.insert(value);
                            break;
                        }
                    }
                }
                _ => {}
            }
        }
    }
    source_ids
}

pub(crate) fn external_action_dispatch_url_from_config(
    config: &Value,
    action_type: &str,
) -> Option<String> {
    let action_specific = if action_type.starts_with("external_artifact.") {
        external_config_string(
            config,
            &[
                "artifact_action_dispatch_url",
                "artifactActionDispatchUrl",
                "artifact_dispatch_url",
                "artifactDispatchUrl",
            ],
        )
    } else {
        external_config_string(
            config,
            &[
                "business_action_dispatch_url",
                "businessActionDispatchUrl",
                "business_dispatch_url",
                "businessDispatchUrl",
            ],
        )
    };
    action_specific.or_else(|| {
        external_config_string(
            config,
            &[
                "external_action_dispatch_url",
                "externalActionDispatchUrl",
                "action_dispatch_url",
                "actionDispatchUrl",
            ],
        )
    })
}

pub(crate) fn external_channel_outbound_reply_dispatch_url_from_config(
    config: &Value,
) -> Option<String> {
    external_config_string(
        config,
        &[
            "external_reply_dispatch_url",
            "externalReplyDispatchUrl",
            "reply_dispatch_url",
            "replyDispatchUrl",
            "outbound_reply_url",
            "outboundReplyUrl",
            "assistant_reply_dispatch_url",
            "assistantReplyDispatchUrl",
        ],
    )
}

pub(crate) fn external_action_dispatch_auth_mode(
    auth: &ExternalActionDispatchAuth,
) -> &'static str {
    match (auth.signing_secret.is_some(), auth.bearer_token.is_some()) {
        (true, true) => "signature_and_bearer",
        (true, false) => "signature",
        (false, true) => "bearer",
        (false, false) => "none",
    }
}

pub(crate) fn external_channel_auth_failed() -> ApiError {
    ApiError::unauthorized(
        "external_channel_auth_failed",
        "external channel bearer token is missing or invalid".to_string(),
    )
}

pub(crate) fn external_channel_platform_wire_value(
    platform: &ExternalChannelPlatformView,
) -> &'static str {
    match platform {
        ExternalChannelPlatformView::Feishu => "feishu",
        ExternalChannelPlatformView::Lark => "lark",
        ExternalChannelPlatformView::WeCom => "we_com",
        ExternalChannelPlatformView::GenericChat => "generic_chat",
        ExternalChannelPlatformView::ThirdParty => "third_party",
    }
}

pub(crate) fn external_channel_platform_from_wire_value(
    value: &str,
) -> Option<ExternalChannelPlatformView> {
    match value {
        "feishu" => Some(ExternalChannelPlatformView::Feishu),
        "lark" => Some(ExternalChannelPlatformView::Lark),
        "we_com" => Some(ExternalChannelPlatformView::WeCom),
        "generic_chat" => Some(ExternalChannelPlatformView::GenericChat),
        "aigolf" => Some(ExternalChannelPlatformView::GenericChat),
        "third_party" => Some(ExternalChannelPlatformView::ThirdParty),
        _ => None,
    }
}

pub(crate) fn external_message_type_wire_value(
    message_type: &ExternalMessageTypeView,
) -> &'static str {
    match message_type {
        ExternalMessageTypeView::Text => "text",
        ExternalMessageTypeView::Image => "image",
        ExternalMessageTypeView::File => "file",
        ExternalMessageTypeView::Audio => "audio",
        ExternalMessageTypeView::Video => "video",
        ExternalMessageTypeView::Card => "card",
        ExternalMessageTypeView::Event => "event",
        ExternalMessageTypeView::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_control_reason_present_trims_without_storing_reason_text() {
        assert!(!external_control_reason_present(None));
        assert!(!external_control_reason_present(Some("   ")));
        assert!(external_control_reason_present(Some(" operator approved ")));
    }

    #[test]
    fn external_control_config_patch_never_stores_operator_reason_text() {
        let now = Utc::now();
        let patch = external_control_config_patch("rotate_secret", true, now);
        let patch_text = patch.to_string();

        assert_eq!(
            patch["management_control"]["last_action"],
            json!("rotate_secret")
        );
        assert_eq!(patch["management_control"]["reason_present"], json!(true));
        assert_eq!(
            patch["management_control"]["secret_material_included"],
            json!(false)
        );
        assert!(!patch_text.contains("operator approved"));
        assert!(!patch_text.contains("secret-token"));
    }

    #[test]
    fn external_control_integration_kind_tracks_mixed_connections() {
        assert_eq!(external_control_integration_kind(1, 0), "channel");
        assert_eq!(external_control_integration_kind(0, 1), "source");
        assert_eq!(external_control_integration_kind(1, 1), "mixed");
        assert_eq!(external_control_integration_kind(0, 0), "unknown");
    }

    #[test]
    fn external_channel_connection_id_validation_preserves_length_and_charset_rules() {
        assert!(validate_external_channel_connection_id(" generic-chat.main_01 ").is_ok());

        let too_short = validate_external_channel_connection_id("ab")
            .expect_err("short ids should be rejected");
        assert_eq!(
            too_short.payload.code,
            "external_channel_connection_id_invalid"
        );
        assert!(too_short.payload.message.contains("3-96 characters"));

        let invalid_charset = validate_external_channel_connection_id("generic/chat")
            .expect_err("slashes should be rejected");
        assert_eq!(
            invalid_charset.payload.code,
            "external_channel_connection_id_invalid"
        );
        assert!(invalid_charset
            .payload
            .message
            .contains("ASCII letters, numbers, dot, dash, or underscore"));
    }

    #[test]
    fn external_channel_platform_validation_preserves_ascii_rules() {
        assert!(validate_external_channel_platform(" generic_chat-1 ").is_ok());

        let empty =
            validate_external_channel_platform("   ").expect_err("empty platform is invalid");
        assert_eq!(empty.payload.code, "external_channel_platform_invalid");

        let invalid_charset = validate_external_channel_platform("generic.chat")
            .expect_err("dot is not allowed for platform");
        assert_eq!(
            invalid_charset.payload.code,
            "external_channel_platform_invalid"
        );
        assert!(invalid_charset
            .payload
            .message
            .contains("within 64 characters"));
    }

    #[test]
    fn external_database_source_id_validation_preserves_trim_length_and_path_rules() {
        assert_eq!(
            validate_external_database_source_id(" source-main ")
                .expect("source id should be valid"),
            "source-main"
        );

        let empty = validate_external_database_source_id("   ").expect_err("empty id is invalid");
        assert_eq!(empty.payload.code, "validation_error");
        assert!(empty.payload.message.contains("must not be empty"));

        let invalid_path =
            validate_external_database_source_id("source/main").expect_err("slash is invalid");
        assert_eq!(invalid_path.payload.code, "validation_error");
        assert!(invalid_path
            .payload
            .message
            .contains("must not contain path separators"));
    }

    #[test]
    fn external_channel_allowed_database_source_ids_collects_aliases_and_database_sources() {
        let config = json!({
            "allowed_database_source_ids": ["db-a", " db-b "],
            "databaseSourceIds": "db-c, db-a",
            "allowedSourceIds": ["db-d"],
            "database_sources": [
                "db-e",
                {"source_external_id": "db-f"},
                {"sourceId": "db-g"},
                {"source_id": "   "}
            ],
            "default_source_id": "db-default"
        });

        let ids = external_channel_allowed_database_source_ids(&config)
            .into_iter()
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            vec![
                "db-a".to_string(),
                "db-b".to_string(),
                "db-c".to_string(),
                "db-d".to_string(),
                "db-e".to_string(),
                "db-f".to_string(),
                "db-g".to_string()
            ]
        );
        assert!(external_channel_database_source_allowed(
            &config,
            " db-default "
        ));
        assert!(external_channel_database_source_allowed(&config, "db-g"));
        assert!(!external_channel_database_source_allowed(
            &config,
            "db-missing"
        ));
    }

    #[test]
    fn external_channel_allowed_database_source_id_validation_dedupes_sorted_ids() {
        let ids = validate_external_channel_allowed_database_source_ids(&[
            " db-b ".to_string(),
            "db-a".to_string(),
            "db-b".to_string(),
        ])
        .expect("source id list should normalize");
        assert_eq!(ids, vec!["db-a".to_string(), "db-b".to_string()]);
    }

    #[test]
    fn external_channel_inbound_token_helpers_preserve_rotation_and_expiry_shape() {
        let now = DateTime::parse_from_rfc3339("2026-06-12T08:00:00Z")
            .expect("valid timestamp")
            .with_timezone(&Utc);
        let expires_at = DateTime::parse_from_rfc3339("2026-06-12T08:30:00Z")
            .expect("valid timestamp")
            .with_timezone(&Utc);
        let mut config = json!({});

        let token = new_external_channel_inbound_token();
        assert!(token.starts_with("v3in_"));
        assert_eq!(token.len(), "v3in_".len() + 64);

        apply_external_channel_inbound_token_config(
            &mut config,
            "v3in_fixed",
            now,
            true,
            Some(expires_at),
        );

        assert_eq!(config["inbound_bearer_token"], json!("v3in_fixed"));
        assert_eq!(
            external_channel_inbound_token_rotated_at(&config),
            Some("2026-06-12T08:00:00Z".to_string())
        );
        assert_eq!(
            external_channel_inbound_token_expires_at(&config),
            Some(expires_at)
        );
        assert!(!external_channel_inbound_token_expired(&config, now));
        assert!(external_channel_inbound_token_expired(
            &config,
            expires_at + chrono::Duration::seconds(1)
        ));

        let summary = external_channel_temporary_access_summary(&config);
        assert_eq!(summary["temporary"], json!(true));
        assert_eq!(summary["token_rotated_at"], json!("2026-06-12T08:00:00Z"));
        assert_eq!(summary["expires_at"], json!(expires_at));
        assert!(summary["expired"].is_boolean());
    }

    #[test]
    fn external_channel_config_sanitizer_preserves_public_fields_and_removes_secret_aliases() {
        let mut config = json!({
            "display_hint": "keep",
            "inbound_bearer_token": "secret",
            "inboundBearerTokenExpiresAt": "2026-06-12T08:00:00Z",
            "reply_dispatch_url": "https://callback.example/reply",
            "external_action_signing_secret": "secret",
            "management_control": {"last_action": "disable"},
            "temporary_access": {"temporary": true},
            "channel_management": {"internal": true}
        });

        sanitize_cloned_external_channel_config(&mut config);

        assert_eq!(config["display_hint"], json!("keep"));
        assert!(config.get("inbound_bearer_token").is_none());
        assert!(config.get("inboundBearerTokenExpiresAt").is_none());
        assert!(config.get("reply_dispatch_url").is_none());
        assert!(config.get("external_action_signing_secret").is_none());
        assert!(config.get("management_control").is_none());
        assert!(config.get("temporary_access").is_none());
        assert!(config.get("channel_management").is_none());
    }
}
