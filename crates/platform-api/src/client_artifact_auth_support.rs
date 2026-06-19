use axum::http::HeaderMap;
use domain_model::User;

use crate::{
    client_artifact_contract_support::client_artifact_upload_bearer_token, current_auth_session,
    ApiError, AppState,
};

const V3_CLIENT_ARTIFACT_UPLOAD_TOKEN_ENV: &str = "V3_CLIENT_ARTIFACT_TOKEN";

pub(crate) async fn require_client_artifact_upload_authorization(
    state: &AppState,
    headers: &HeaderMap,
) -> std::result::Result<Option<User>, ApiError> {
    if let Some((user, _session)) = current_auth_session(state, headers).await? {
        return Ok(Some(user));
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
    if actual != expected {
        return Err(ApiError::unauthorized(
            "invalid_client_artifact_token",
            "client artifact upload token is invalid".to_string(),
        ));
    }
    Ok(None)
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
