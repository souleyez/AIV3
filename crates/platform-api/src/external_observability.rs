use axum::http::HeaderMap;
use sha2::{Digest, Sha256};

use crate::ApiError;

pub(crate) const EXTERNAL_OBSERVABILITY_ACCESS_HEADER: &str =
    "x-ai-data-platform-external-observability-key";

fn configured_access_key() -> Option<String> {
    std::env::var("EXTERNAL_OBSERVABILITY_ACCESS_KEY")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            std::env::var("EXTERNAL_INTEGRATIONS_OBSERVATION_KEY")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
}

pub(crate) fn access_allowed(headers: &HeaderMap) -> bool {
    let Some(expected) = configured_access_key() else {
        return true;
    };
    let Some(actual) = headers
        .get(EXTERNAL_OBSERVABILITY_ACCESS_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return false;
    };
    Sha256::digest(expected.as_bytes()) == Sha256::digest(actual.as_bytes())
}

pub(crate) fn require_external_integration_management_access(
    headers: &HeaderMap,
) -> std::result::Result<(), ApiError> {
    if access_allowed(headers) {
        return Ok(());
    }
    Err(ApiError::unauthorized(
        "external_observability_access_required",
        "external integration management requires an observability access key".to_string(),
    ))
}
