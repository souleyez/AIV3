use axum::http::HeaderMap;
use domain_model::User;

use crate::{
    client_artifact_contract_support::client_artifact_upload_bearer_token,
    client_config_session_support::{verify_client_session_from_headers, V3ClientSessionClaims},
    current_auth_session,
    external_channel_auth_support::external_channel_constant_time_str_eq,
    ApiError, AppState,
};

const V3_CLIENT_ARTIFACT_UPLOAD_TOKEN_ENV: &str = "V3_CLIENT_ARTIFACT_TOKEN";

pub(crate) enum ClientArtifactAuthorization {
    User(User),
    GlobalUploadToken,
    ClientSession(V3ClientSessionClaims),
}

impl ClientArtifactAuthorization {
    pub(crate) fn user(&self) -> Option<&User> {
        match self {
            Self::User(user) => Some(user),
            Self::GlobalUploadToken | Self::ClientSession(_) => None,
        }
    }

    pub(crate) fn client_session(&self) -> Option<&V3ClientSessionClaims> {
        match self {
            Self::ClientSession(claims) => Some(claims),
            Self::User(_) | Self::GlobalUploadToken => None,
        }
    }
}

pub(crate) async fn require_client_artifact_upload_authorization(
    state: &AppState,
    headers: &HeaderMap,
) -> std::result::Result<ClientArtifactAuthorization, ApiError> {
    if let Some((user, _session)) = current_auth_session(state, headers).await? {
        return Ok(ClientArtifactAuthorization::User(user));
    }
    if let Some(claims) = verify_client_session_from_headers(headers, chrono::Utc::now())? {
        return Ok(ClientArtifactAuthorization::ClientSession(claims));
    }
    let Some(expected) = client_artifact_upload_token_from_env() else {
        return Err(ApiError::unauthorized(
            "client_artifact_token_required",
            "client artifact upload requires a V3 session or upload token".to_string(),
        ));
    };
    let Some(actual) = client_artifact_upload_bearer_token(headers) else {
        return Err(ApiError::unauthorized(
            "client_artifact_token_required",
            "client artifact upload token is required".to_string(),
        ));
    };
    if !external_channel_constant_time_str_eq(actual, &expected) {
        return Err(ApiError::unauthorized(
            "invalid_client_artifact_token",
            "client artifact upload token is invalid".to_string(),
        ));
    }
    Ok(ClientArtifactAuthorization::GlobalUploadToken)
}

pub(crate) async fn require_client_artifact_management_authorization(
    state: &AppState,
    headers: &HeaderMap,
) -> std::result::Result<(), ApiError> {
    match require_client_artifact_upload_authorization(state, headers).await? {
        ClientArtifactAuthorization::ClientSession(_) => Err(ApiError::forbidden(
            "client_session_upload_only",
            "client session may upload artifacts but cannot list or read them".to_string(),
        )),
        ClientArtifactAuthorization::User(_) | ClientArtifactAuthorization::GlobalUploadToken => {
            Ok(())
        }
    }
}

fn client_artifact_upload_token_from_env() -> Option<String> {
    std::env::var(V3_CLIENT_ARTIFACT_UPLOAD_TOKEN_ENV)
        .ok()
        .and_then(trim_optional_string)
}

fn trim_optional_string(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_artifact_auth_support_trims_env_token_values() {
        assert_eq!(
            trim_optional_string(" token-1 ".to_string()).as_deref(),
            Some("token-1")
        );
        assert!(trim_optional_string("  ".to_string()).is_none());
    }
}
