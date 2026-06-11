use super::external_config_string;
use crate::ApiError;
use serde_json::Value;

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
