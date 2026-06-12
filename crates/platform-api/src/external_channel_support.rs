use crate::ApiError;
use chrono::{DateTime, Utc};
use contracts::{ExternalChannelPlatformView, ExternalMessageTypeView};
use serde_json::{json, Value};

use super::external_config_string;

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
}
