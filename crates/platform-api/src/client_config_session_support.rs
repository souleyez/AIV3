use axum::http::HeaderMap;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::{DateTime, Duration, TimeZone, Utc};
use contracts::{
    ClientConfigPackageView, ResolveClientConfigSessionRequest, ResolveClientConfigSessionResponse,
    ResolvedClientConfigPackageView, V3ClientArtifactManifestView, V3_CLIENT_CONFIG_SCHEMA,
};
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::env;
use uuid::Uuid;

use crate::{
    external_channel_auth_support::{
        external_channel_authorization_bearer_token, external_channel_constant_time_str_eq,
    },
    ApiError,
};

const CLIENT_CONFIG_BRIDGE_TOKEN_ENV: &str = "V3_CLIENT_CONFIG_BRIDGE_TOKEN";
const CLIENT_SESSION_SIGNING_KEY_ENV: &str = "V3_CLIENT_SESSION_SIGNING_KEY";
const DEFAULT_CLIENT_SESSION_TTL_SECONDS: i64 = 60 * 60;
const MIN_CLIENT_SESSION_TTL_SECONDS: i64 = 5 * 60;
const MAX_CLIENT_SESSION_TTL_SECONDS: i64 = 60 * 60;
const CLIENT_SESSION_TOKEN_PREFIX: &str = "v3cs_";
const MAX_CLIENT_SESSION_TOKEN_BYTES: usize = 4 * 1024;

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct V3ClientSessionClaims {
    pub version: u8,
    pub package_id: String,
    pub tenant_id: String,
    pub user_id: String,
    pub client_id: String,
    #[serde(default)]
    pub dataset_ids: Vec<String>,
    #[serde(default)]
    pub asset_library_ids: Vec<String>,
    pub expires_at_unix: i64,
    pub nonce: String,
}

pub(crate) fn require_client_config_bridge_authorization(
    headers: &HeaderMap,
) -> std::result::Result<(), ApiError> {
    let expected = required_secret(CLIENT_CONFIG_BRIDGE_TOKEN_ENV)?;
    let actual = external_channel_authorization_bearer_token(headers).ok_or_else(|| {
        ApiError::unauthorized(
            "client_config_bridge_token_required",
            "client config bridge authorization is required".to_string(),
        )
    })?;
    if !external_channel_constant_time_str_eq(actual, &expected) {
        return Err(ApiError::unauthorized(
            "invalid_client_config_bridge_token",
            "client config bridge authorization is invalid".to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn resolve_client_config_session(
    package: &ClientConfigPackageView,
    request: &ResolveClientConfigSessionRequest,
    now: DateTime<Utc>,
) -> std::result::Result<ResolveClientConfigSessionResponse, ApiError> {
    let signing_key = required_secret(CLIENT_SESSION_SIGNING_KEY_ENV)?;
    resolve_client_config_session_with_secret(package, request, now, &signing_key)
}

fn resolve_client_config_session_with_secret(
    package: &ClientConfigPackageView,
    request: &ResolveClientConfigSessionRequest,
    now: DateTime<Utc>,
    signing_key: &str,
) -> std::result::Result<ResolveClientConfigSessionResponse, ApiError> {
    validate_resolve_request(package, request, now)?;
    let requested_ttl = request
        .expires_in_seconds
        .unwrap_or(DEFAULT_CLIENT_SESSION_TTL_SECONDS)
        .clamp(
            MIN_CLIENT_SESSION_TTL_SECONDS,
            MAX_CLIENT_SESSION_TTL_SECONDS,
        );
    let requested_expiry = now + Duration::seconds(requested_ttl);
    let session_expires_at = package
        .expires_at
        .map(|package_expiry| package_expiry.min(requested_expiry))
        .unwrap_or(requested_expiry);
    if session_expires_at <= now {
        return Err(ApiError::forbidden(
            "client_config_package_expired",
            "client config package is expired".to_string(),
        ));
    }
    let claims = V3ClientSessionClaims {
        version: 1,
        package_id: package.package_id.clone(),
        tenant_id: package.tenant_id.clone(),
        user_id: package.user_id.clone(),
        client_id: request.client_id.trim().to_string(),
        dataset_ids: package.dataset_ids.clone(),
        asset_library_ids: package.asset_library_ids.clone(),
        expires_at_unix: session_expires_at.timestamp(),
        nonce: Uuid::new_v4().simple().to_string(),
    };
    let session_token = issue_client_session_token(&claims, signing_key)?;
    if session_token.len() > MAX_CLIENT_SESSION_TOKEN_BYTES {
        return Err(ApiError::bad_request(
            "client_config_scope_too_large",
            "client config scope is too large for a native client session".to_string(),
        ));
    }
    Ok(ResolveClientConfigSessionResponse {
        package_id: package.package_id.clone(),
        config_package: ResolvedClientConfigPackageView {
            schema: V3_CLIENT_CONFIG_SCHEMA.to_string(),
            tenant_id: package.tenant_id.clone(),
            user_id: package.user_id.clone(),
            client_id: claims.client_id.clone(),
            v3_base_url: package.v3_base_url.clone(),
            dataset_ids: package.dataset_ids.clone(),
            asset_library_ids: package.asset_library_ids.clone(),
            skill_packs: package.skill_packs.clone(),
            artifact_upload: package.artifact_upload.clone(),
            expires_at: session_expires_at,
        },
        session_token,
        session_expires_at,
    })
}

pub(crate) fn verify_client_session_from_headers(
    headers: &HeaderMap,
    now: DateTime<Utc>,
) -> std::result::Result<Option<V3ClientSessionClaims>, ApiError> {
    let Some(token) = external_channel_authorization_bearer_token(headers) else {
        return Ok(None);
    };
    if !token.starts_with(CLIENT_SESSION_TOKEN_PREFIX) {
        return Ok(None);
    }
    let signing_key = required_secret(CLIENT_SESSION_SIGNING_KEY_ENV)?;
    verify_client_session_token(token, &signing_key, now).map(Some)
}

pub(crate) fn validate_client_session_manifest_scope(
    claims: &V3ClientSessionClaims,
    manifest: &V3ClientArtifactManifestView,
) -> std::result::Result<(), ApiError> {
    for (field, actual, expected) in [
        (
            "tenant_id",
            manifest.tenant_id.as_str(),
            claims.tenant_id.as_str(),
        ),
        (
            "user_id",
            manifest.user_id.as_str(),
            claims.user_id.as_str(),
        ),
        (
            "client_id",
            manifest.client_id.as_str(),
            claims.client_id.as_str(),
        ),
    ] {
        if actual.trim() != expected.trim() {
            return Err(ApiError::forbidden(
                "client_session_identity_mismatch",
                format!("artifact {field} is outside the client session"),
            ));
        }
    }
    require_subset("dataset_ids", &manifest.dataset_ids, &claims.dataset_ids)?;
    require_subset(
        "asset_library_ids",
        &manifest.asset_library_ids,
        &claims.asset_library_ids,
    )?;
    Ok(())
}

fn validate_resolve_request(
    package: &ClientConfigPackageView,
    request: &ResolveClientConfigSessionRequest,
    now: DateTime<Utc>,
) -> std::result::Result<(), ApiError> {
    for (field, actual, expected) in [
        (
            "tenant_id",
            request.tenant_id.as_str(),
            package.tenant_id.as_str(),
        ),
        (
            "user_id",
            request.user_id.as_str(),
            package.user_id.as_str(),
        ),
    ] {
        if actual.trim().is_empty() || actual.trim() != expected.trim() {
            return Err(ApiError::forbidden(
                "client_config_binding_mismatch",
                format!("{field} does not match the V3 config package"),
            ));
        }
    }
    let client_id = request.client_id.trim();
    if client_id.is_empty() || client_id.len() > 128 {
        return Err(ApiError::bad_request(
            "invalid_client_id",
            "client_id is required and must not exceed 128 characters".to_string(),
        ));
    }
    if package
        .expires_at
        .is_some_and(|expires_at| expires_at <= now)
    {
        return Err(ApiError::forbidden(
            "client_config_package_expired",
            "client config package is expired".to_string(),
        ));
    }
    Ok(())
}

fn required_secret(name: &str) -> std::result::Result<String, ApiError> {
    let value = env::var(name).unwrap_or_default().trim().to_string();
    if value.len() < 32 {
        return Err(ApiError::service_unavailable(
            "client_session_not_configured",
            format!("{name} is not configured"),
        ));
    }
    Ok(value)
}

fn issue_client_session_token(
    claims: &V3ClientSessionClaims,
    secret: &str,
) -> std::result::Result<String, ApiError> {
    let payload = serde_json::to_vec(claims)
        .map_err(|error| ApiError::internal("client_session_encode_failed", error.to_string()))?;
    let payload = URL_SAFE_NO_PAD.encode(payload);
    let signature = sign_client_session_payload(&payload, secret);
    Ok(format!(
        "{CLIENT_SESSION_TOKEN_PREFIX}{payload}.{signature}"
    ))
}

fn verify_client_session_token(
    token: &str,
    secret: &str,
    now: DateTime<Utc>,
) -> std::result::Result<V3ClientSessionClaims, ApiError> {
    let encoded = token
        .strip_prefix(CLIENT_SESSION_TOKEN_PREFIX)
        .ok_or_else(invalid_client_session_error)?;
    let (payload, signature) = encoded
        .split_once('.')
        .ok_or_else(invalid_client_session_error)?;
    if payload.len() > 32 * 1024 || signature.len() > 128 {
        return Err(invalid_client_session_error());
    }
    let signature = URL_SAFE_NO_PAD
        .decode(signature)
        .map_err(|_| invalid_client_session_error())?;
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(payload.as_bytes());
    mac.verify_slice(&signature)
        .map_err(|_| invalid_client_session_error())?;
    let payload = URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|_| invalid_client_session_error())?;
    let claims: V3ClientSessionClaims =
        serde_json::from_slice(&payload).map_err(|_| invalid_client_session_error())?;
    if claims.version != 1
        || claims.package_id.trim().is_empty()
        || claims.tenant_id.trim().is_empty()
        || claims.user_id.trim().is_empty()
        || claims.client_id.trim().is_empty()
        || claims.nonce.trim().is_empty()
    {
        return Err(invalid_client_session_error());
    }
    let expires_at = Utc
        .timestamp_opt(claims.expires_at_unix, 0)
        .single()
        .ok_or_else(invalid_client_session_error)?;
    if expires_at <= now {
        return Err(ApiError::unauthorized(
            "client_session_expired",
            "client session is expired".to_string(),
        ));
    }
    Ok(claims)
}

fn sign_client_session_payload(payload: &str, secret: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(payload.as_bytes());
    URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
}

fn invalid_client_session_error() -> ApiError {
    ApiError::unauthorized(
        "invalid_client_session",
        "client session is invalid".to_string(),
    )
}

fn require_subset(
    field: &str,
    requested: &[String],
    allowed: &[String],
) -> std::result::Result<(), ApiError> {
    for value in requested {
        let value = value.trim();
        if !allowed.iter().any(|allowed| allowed.trim() == value) {
            return Err(ApiError::forbidden(
                "client_session_scope_denied",
                format!("{field} value is outside the client session"),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use contracts::V3ClientArtifactUploadConfigView;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, 11, 8, 0, 0).single().unwrap()
    }

    fn claims() -> V3ClientSessionClaims {
        V3ClientSessionClaims {
            version: 1,
            package_id: "v3cp_1".to_string(),
            tenant_id: "tenant-1".to_string(),
            user_id: "user-1".to_string(),
            client_id: "client-1".to_string(),
            dataset_ids: vec!["dataset-1".to_string()],
            asset_library_ids: vec!["asset-1".to_string()],
            expires_at_unix: (now() + Duration::hours(1)).timestamp(),
            nonce: "nonce-1".to_string(),
        }
    }

    fn manifest() -> V3ClientArtifactManifestView {
        V3ClientArtifactManifestView {
            schema: "v3.client_artifact_manifest.v1".to_string(),
            source: "enterprise-codex-client".to_string(),
            tenant_id: "tenant-1".to_string(),
            user_id: "user-1".to_string(),
            client_id: "client-1".to_string(),
            task_id: "task-1".to_string(),
            title: "artifact".to_string(),
            artifact_type: "html".to_string(),
            dataset_ids: vec!["dataset-1".to_string()],
            asset_library_ids: vec!["asset-1".to_string()],
            files: Vec::new(),
            evidence_refs: Vec::new(),
            created_at: now(),
            metadata: serde_json::json!({}),
        }
    }

    #[test]
    fn client_session_token_round_trips_and_rejects_tampering() {
        let secret = "test-client-session-signing-key-32-bytes";
        let token = issue_client_session_token(&claims(), secret).unwrap();
        let decoded = verify_client_session_token(&token, secret, now()).unwrap();
        assert_eq!(decoded, claims());

        let mut tampered = token.into_bytes();
        let last = tampered.len() - 1;
        tampered[last] = if tampered[last] == b'a' { b'b' } else { b'a' };
        let tampered = String::from_utf8(tampered).unwrap();
        assert!(verify_client_session_token(&tampered, secret, now()).is_err());
    }

    #[test]
    fn client_session_token_rejects_expiry() {
        let secret = "test-client-session-signing-key-32-bytes";
        let token = issue_client_session_token(&claims(), secret).unwrap();
        let error = verify_client_session_token(&token, secret, now() + Duration::hours(2))
            .expect_err("expired session must fail");
        assert_eq!(error.payload.code, "client_session_expired");
    }

    #[test]
    fn client_session_scope_rejects_identity_and_dataset_changes() {
        validate_client_session_manifest_scope(&claims(), &manifest()).unwrap();
        let mut outside = manifest();
        outside.dataset_ids = vec!["dataset-2".to_string()];
        assert_eq!(
            validate_client_session_manifest_scope(&claims(), &outside)
                .unwrap_err()
                .payload
                .code,
            "client_session_scope_denied"
        );
        outside = manifest();
        outside.client_id = "client-2".to_string();
        assert_eq!(
            validate_client_session_manifest_scope(&claims(), &outside)
                .unwrap_err()
                .payload
                .code,
            "client_session_identity_mismatch"
        );
    }

    #[test]
    fn resolved_config_has_only_v3_owned_fields() {
        let package = ClientConfigPackageView {
            package_id: "v3cp_1".to_string(),
            tenant_id: "tenant-1".to_string(),
            user_id: "user-1".to_string(),
            client_id: "template-client".to_string(),
            v3_base_url: "https://v3.elepcloud.com".to_string(),
            asset_library_ids: vec!["asset-1".to_string()],
            dataset_ids: vec!["dataset-1".to_string()],
            skill_packs: vec!["reporting".to_string()],
            artifact_upload: V3ClientArtifactUploadConfigView {
                mode: "session_token".to_string(),
                endpoint: "/v1/client-artifacts".to_string(),
            },
            codex_control: Some(Default::default()),
            expires_at: None,
            created_at: now(),
            config_package: serde_json::json!({"provider_key": "must-not-escape"}),
        };
        let request = ResolveClientConfigSessionRequest {
            tenant_id: "tenant-1".to_string(),
            user_id: "user-1".to_string(),
            client_id: "terminal-1".to_string(),
            expires_in_seconds: Some(3600),
        };
        let resolved = resolve_client_config_session_with_secret(
            &package,
            &request,
            now(),
            "test-client-session-signing-key-32-bytes",
        )
        .unwrap();
        let encoded = serde_json::to_value(&resolved.config_package).unwrap();
        assert!(encoded.get("codex_control").is_none());
        assert!(encoded.get("provider_key").is_none());
        assert_eq!(encoded["client_id"], "terminal-1");
    }

    #[test]
    fn client_session_rejects_scope_that_exceeds_native_credential_limit() {
        let mut package = ClientConfigPackageView {
            package_id: "v3cp_large".to_string(),
            tenant_id: "tenant-1".to_string(),
            user_id: "user-1".to_string(),
            client_id: "template-client".to_string(),
            v3_base_url: "https://v3.elepcloud.com".to_string(),
            asset_library_ids: Vec::new(),
            dataset_ids: Vec::new(),
            skill_packs: Vec::new(),
            artifact_upload: V3ClientArtifactUploadConfigView {
                mode: "session_token".to_string(),
                endpoint: "/v1/client-artifacts".to_string(),
            },
            codex_control: None,
            expires_at: None,
            created_at: now(),
            config_package: serde_json::json!({}),
        };
        package.dataset_ids = (0..128)
            .map(|index| format!("dataset-{index:03}-{}", "x".repeat(80)))
            .collect();
        let error = resolve_client_config_session_with_secret(
            &package,
            &ResolveClientConfigSessionRequest {
                tenant_id: "tenant-1".to_string(),
                user_id: "user-1".to_string(),
                client_id: "terminal-1".to_string(),
                expires_in_seconds: Some(3600),
            },
            now(),
            "test-client-session-signing-key-32-bytes",
        )
        .expect_err("oversized scoped session must fail before delivery");
        assert_eq!(error.payload.code, "client_config_scope_too_large");
    }
}
