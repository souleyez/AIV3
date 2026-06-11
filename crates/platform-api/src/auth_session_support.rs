use super::{
    auth_email, trim_optional, validate_required, AUTH_SESSION_COOKIE_NAME, AUTH_SESSION_TTL_DAYS,
    DEFAULT_AUTH_SESSION_PEPPER,
};
use crate::ApiError;
use axum::http::{header, HeaderMap, HeaderValue};
use contracts::{AuthAuditEventView, AuthSessionView, AuthUserView};
use domain_model::{AuthAuditEvent, AuthChallengePurpose, User, UserSession, UserSessionId};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub(crate) fn validate_auth_email(email: &str) -> std::result::Result<String, ApiError> {
    validate_required("email", email)?;
    let normalized = auth_email::normalize_email(email);
    let has_one_at = normalized.matches('@').count() == 1;
    if !has_one_at || normalized.starts_with('@') || normalized.ends_with('@') {
        return Err(ApiError::bad_request(
            "invalid_email",
            "email must be a valid address".to_string(),
        ));
    }
    Ok(normalized)
}

pub(crate) fn email_auth_verify_creates_session_for_purpose(
    purpose: &AuthChallengePurpose,
) -> bool {
    matches!(
        purpose,
        AuthChallengePurpose::AccountCreate
            | AuthChallengePurpose::Login
            | AuthChallengePurpose::RecoverKey
    )
}

pub(crate) fn auth_challenge_metadata(device_fingerprint: Option<&str>) -> Value {
    let mut metadata = Map::new();
    if let Some(device) = device_fingerprint.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    }) {
        metadata.insert("device_fingerprint".to_string(), json!(device));
    }
    Value::Object(metadata)
}

pub(crate) fn auth_env(key: &str, fallback: &str) -> String {
    std::env::var(key)
        .ok()
        .and_then(|value| {
            let value = value.trim().to_string();
            (!value.is_empty()).then_some(value)
        })
        .unwrap_or_else(|| fallback.to_string())
}

pub(crate) fn auth_session_token_hash(session_token: &str) -> String {
    let pepper = auth_env("AUTH_SESSION_PEPPER", DEFAULT_AUTH_SESSION_PEPPER);
    let mut hasher = Sha256::new();
    for part in [pepper.as_bytes(), b":", session_token.as_bytes()] {
        hasher.update(part);
    }
    format!("{:x}", hasher.finalize())
}

pub(crate) fn new_auth_session_token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

pub(crate) fn normalize_device_fingerprint(device_fingerprint: Option<String>) -> String {
    trim_optional(device_fingerprint).unwrap_or_else(|| "unknown-device".to_string())
}

pub(crate) fn auth_session_token_from_headers(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    raw.split(';').find_map(|entry| {
        let (name, value) = entry.trim().split_once('=')?;
        (name == AUTH_SESSION_COOKIE_NAME && !value.is_empty()).then(|| value.to_string())
    })
}

pub(crate) fn set_session_cookie_headers(
    session_token: &str,
) -> std::result::Result<HeaderMap, ApiError> {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&format!(
            "{AUTH_SESSION_COOKIE_NAME}={session_token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
            AUTH_SESSION_TTL_DAYS * 24 * 60 * 60
        ))
        .map_err(|error| ApiError::internal("auth_cookie_invalid", error.to_string()))?,
    );
    Ok(headers)
}

pub(crate) fn clear_session_cookie_headers() -> std::result::Result<HeaderMap, ApiError> {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&format!(
            "{AUTH_SESSION_COOKIE_NAME}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"
        ))
        .map_err(|error| ApiError::internal("auth_cookie_invalid", error.to_string()))?,
    );
    Ok(headers)
}

pub(crate) fn to_auth_user_view(user: &User, email_verified: bool) -> AuthUserView {
    AuthUserView {
        id: user.id,
        email: user.email.clone(),
        display_name: user.display_name.clone(),
        email_verified,
    }
}

pub(crate) fn to_auth_session_view(session: &UserSession, email: &str) -> AuthSessionView {
    AuthSessionView {
        id: session.id,
        user_id: session.user_id,
        email: email.to_string(),
        auth_method: session.auth_method.clone(),
        created_at: session.created_at,
        expires_at: session.expires_at,
    }
}

pub(crate) fn to_auth_audit_event_view(
    event: AuthAuditEvent,
    current_session_id: UserSessionId,
) -> AuthAuditEventView {
    AuthAuditEventView {
        id: event.id,
        event_name: event.event_name,
        outcome: event.outcome,
        email: event.email_normalized,
        device_fingerprint: event.device_fingerprint,
        current_session: event.session_id == Some(current_session_id),
        details: safe_auth_audit_details(&event.metadata),
        created_at: event.created_at,
    }
}

fn safe_auth_audit_details(metadata: &Value) -> Value {
    let Some(object) = metadata.as_object() else {
        return json!({});
    };

    let mut details = Map::new();
    for key in [
        "purpose",
        "reason",
        "auth_method",
        "reused",
        "revoked",
        "matched_binding_count",
        "claimed_dataset_count",
        "skipped_owned_dataset_count",
        "attempt_count_before",
    ] {
        if let Some(value) = object.get(key) {
            details.insert(key.to_string(), value.clone());
        }
    }
    Value::Object(details)
}
