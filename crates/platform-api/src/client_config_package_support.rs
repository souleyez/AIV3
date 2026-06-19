use axum::http::HeaderMap;
use contracts::{
    ClientConfigPackageView, CreateClientConfigPackageRequest, CreateClientConfigPackageResponse,
    V3ClientArtifactUploadConfigView, V3CodexControlConfigView, V3_CLIENT_CONFIG_SCHEMA,
};
use domain_model::UserId;
use reqwest::Url;
use serde_json::{json, Value};
use sqlx::Row;
use std::env;
use uuid::Uuid;

use crate::{
    client_artifact_contract_support::{
        validate_client_artifact_upload_config, validate_v3_client_ref_list,
    },
    client_artifact_error_support::client_config_package_not_found_error,
    client_artifact_scope_support::validate_v3_client_scope_refs,
    validate_required, ApiError, AppState,
};

const DEFAULT_CODEX_CONTROL_BASE_URL: &str = "https://ad.goods-editor.com";
const CODEX_CONTROL_BASE_URL_ENV: &str = "V3_CODEX_CONTROL_BASE_URL";
const DEFAULT_CODEX_ACTIVATION_ENDPOINT: &str = "/api/codex/clients/activate";
const DEFAULT_CODEX_HEARTBEAT_ENDPOINT: &str = "/api/codex/clients/heartbeat";
const DEFAULT_CODEX_REVOKE_ENDPOINT: &str = "/api/codex/clients/self-revoke";
const DEFAULT_CODEX_PLANS_ENDPOINT: &str = "/api/codex/quotas/plans";
const DEFAULT_CODEX_QUOTA_SUMMARY_ENDPOINT: &str = "/api/codex/quotas/summary";
const DEFAULT_CODEX_TERMINAL_SUMMARY_ENDPOINT: &str = "/api/codex/clients/terminal-summary";
const DEFAULT_CODEX_BILLING_PATH: &str = "/codex/billing";
const DEFAULT_CODEX_ACTIVATION_TOKEN_ENV: &str = "CODEX_CLIENT_ACTIVATION_TOKEN";
const DEFAULT_CODEX_SESSION_TTL_SECONDS: i64 = 30 * 24 * 60 * 60;

pub(crate) fn default_client_artifact_upload_config() -> V3ClientArtifactUploadConfigView {
    V3ClientArtifactUploadConfigView {
        mode: "session_token".to_string(),
        endpoint: "/v1/client-artifacts".to_string(),
    }
}

fn trim_optional(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

fn default_codex_control_base_url() -> String {
    env::var(CODEX_CONTROL_BASE_URL_ENV)
        .ok()
        .and_then(|value| trim_optional(Some(value)))
        .unwrap_or_else(|| DEFAULT_CODEX_CONTROL_BASE_URL.to_string())
}

fn default_codex_control_billing_url(base_url: &str) -> String {
    format!(
        "{}{}",
        base_url.trim_end_matches('/'),
        DEFAULT_CODEX_BILLING_PATH
    )
}

pub(crate) fn default_codex_control_config(terminal_id: &str) -> V3CodexControlConfigView {
    let base_url = default_codex_control_base_url();
    V3CodexControlConfigView {
        billing_url: Some(default_codex_control_billing_url(&base_url)),
        base_url: Some(base_url),
        activation_endpoint: Some(DEFAULT_CODEX_ACTIVATION_ENDPOINT.to_string()),
        heartbeat_endpoint: Some(DEFAULT_CODEX_HEARTBEAT_ENDPOINT.to_string()),
        revoke_endpoint: Some(DEFAULT_CODEX_REVOKE_ENDPOINT.to_string()),
        plans_endpoint: Some(DEFAULT_CODEX_PLANS_ENDPOINT.to_string()),
        quota_summary_endpoint: Some(DEFAULT_CODEX_QUOTA_SUMMARY_ENDPOINT.to_string()),
        terminal_summary_endpoint: Some(DEFAULT_CODEX_TERMINAL_SUMMARY_ENDPOINT.to_string()),
        activation_token_env: Some(DEFAULT_CODEX_ACTIVATION_TOKEN_ENV.to_string()),
        terminal_id: Some(terminal_id.to_string()),
        terminal_label: None,
        session_ttl_seconds: Some(DEFAULT_CODEX_SESSION_TTL_SECONDS),
    }
}

pub(crate) fn resolve_codex_control_config(
    input: Option<V3CodexControlConfigView>,
    terminal_id: &str,
) -> V3CodexControlConfigView {
    let fallback = default_codex_control_config(terminal_id);
    let input = input.unwrap_or_default();
    let base_url = trim_optional(input.base_url).or(fallback.base_url);
    let billing_url = trim_optional(input.billing_url)
        .or_else(|| base_url.as_deref().map(default_codex_control_billing_url));
    V3CodexControlConfigView {
        base_url,
        activation_endpoint: trim_optional(input.activation_endpoint)
            .or(fallback.activation_endpoint),
        heartbeat_endpoint: trim_optional(input.heartbeat_endpoint).or(fallback.heartbeat_endpoint),
        revoke_endpoint: trim_optional(input.revoke_endpoint).or(fallback.revoke_endpoint),
        plans_endpoint: trim_optional(input.plans_endpoint).or(fallback.plans_endpoint),
        quota_summary_endpoint: trim_optional(input.quota_summary_endpoint)
            .or(fallback.quota_summary_endpoint),
        terminal_summary_endpoint: trim_optional(input.terminal_summary_endpoint)
            .or(fallback.terminal_summary_endpoint),
        billing_url,
        activation_token_env: trim_optional(input.activation_token_env)
            .or(fallback.activation_token_env),
        terminal_id: trim_optional(input.terminal_id).or(fallback.terminal_id),
        terminal_label: trim_optional(input.terminal_label),
        session_ttl_seconds: input.session_ttl_seconds.or(fallback.session_ttl_seconds),
    }
}

pub(crate) fn validate_codex_control_config(
    config: &V3CodexControlConfigView,
) -> std::result::Result<(), ApiError> {
    if let Some(base_url) = config.base_url.as_deref() {
        let parsed = Url::parse(base_url).map_err(|_| {
            ApiError::bad_request(
                "invalid_codex_control_base_url",
                "codex_control.base_url must be an absolute http(s) URL".to_string(),
            )
        })?;
        if parsed.scheme() != "https" && parsed.scheme() != "http" {
            return Err(ApiError::bad_request(
                "invalid_codex_control_base_url",
                "codex_control.base_url must use http or https".to_string(),
            ));
        }
        if parsed.host_str().unwrap_or_default().is_empty() {
            return Err(ApiError::bad_request(
                "invalid_codex_control_base_url",
                "codex_control.base_url must include a host".to_string(),
            ));
        }
    }
    for (field, value) in [
        (
            "codex_control.activation_endpoint",
            config.activation_endpoint.as_deref(),
        ),
        (
            "codex_control.heartbeat_endpoint",
            config.heartbeat_endpoint.as_deref(),
        ),
        (
            "codex_control.revoke_endpoint",
            config.revoke_endpoint.as_deref(),
        ),
        (
            "codex_control.plans_endpoint",
            config.plans_endpoint.as_deref(),
        ),
        (
            "codex_control.quota_summary_endpoint",
            config.quota_summary_endpoint.as_deref(),
        ),
        (
            "codex_control.terminal_summary_endpoint",
            config.terminal_summary_endpoint.as_deref(),
        ),
    ] {
        if let Some(path) = value {
            if !path.starts_with('/') {
                return Err(ApiError::bad_request(
                    "invalid_codex_control_endpoint",
                    format!("{field} must be an absolute path"),
                ));
            }
        }
    }
    if let Some(billing_url) = config.billing_url.as_deref() {
        let parsed = Url::parse(billing_url).map_err(|_| {
            ApiError::bad_request(
                "invalid_codex_control_billing_url",
                "codex_control.billing_url must be an absolute http(s) URL".to_string(),
            )
        })?;
        if parsed.scheme() != "https" && parsed.scheme() != "http" {
            return Err(ApiError::bad_request(
                "invalid_codex_control_billing_url",
                "codex_control.billing_url must use http or https".to_string(),
            ));
        }
        if parsed.host_str().unwrap_or_default().is_empty() {
            return Err(ApiError::bad_request(
                "invalid_codex_control_billing_url",
                "codex_control.billing_url must include a host".to_string(),
            ));
        }
    }
    if matches!(config.session_ttl_seconds, Some(value) if value < 0) {
        return Err(ApiError::bad_request(
            "invalid_codex_control_session_ttl",
            "codex_control.session_ttl_seconds must be non-negative".to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn client_config_ref_or_default(
    explicit_ref: Option<&str>,
    fallback_ref: impl ToString,
) -> String {
    explicit_ref
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| fallback_ref.to_string())
}

pub(crate) fn build_client_config_package_payload(
    request: &CreateClientConfigPackageRequest,
    tenant_ref: &str,
    user_ref: &str,
    client_id: &str,
    artifact_upload: V3ClientArtifactUploadConfigView,
    codex_control: V3CodexControlConfigView,
) -> Value {
    let mut payload = json!({
        "schema": V3_CLIENT_CONFIG_SCHEMA,
        "tenant_id": tenant_ref,
        "user_id": user_ref,
        "client_id": client_id,
        "v3_base_url": request.v3_base_url.trim(),
        "asset_library_ids": request.asset_library_ids,
        "dataset_ids": request.dataset_ids,
        "skill_packs": request.skill_packs,
        "artifact_upload": artifact_upload,
        "codex_control": codex_control,
        "expires_at": request.expires_at,
    });
    if !request.metadata.is_null() {
        payload["metadata"] = request.metadata.clone();
    }
    payload
}

pub(crate) fn client_config_package_artifact_upload_from_payload(
    payload: &Value,
) -> std::result::Result<V3ClientArtifactUploadConfigView, ApiError> {
    serde_json::from_value::<V3ClientArtifactUploadConfigView>(
        payload.get("artifact_upload").cloned().unwrap_or_else(
            || json!({"mode": "session_token", "endpoint": "/v1/client-artifacts"}),
        ),
    )
    .map_err(|error| ApiError::internal("client_config_package_decode_failed", error.to_string()))
}

pub(crate) fn client_config_package_codex_control_from_payload(
    payload: &Value,
) -> std::result::Result<Option<V3CodexControlConfigView>, ApiError> {
    let Some(value) = payload.get("codex_control") else {
        return Ok(None);
    };
    serde_json::from_value::<V3CodexControlConfigView>(value.clone())
        .map(Some)
        .map_err(|error| {
            ApiError::internal("client_config_package_decode_failed", error.to_string())
        })
}

pub(crate) fn client_config_package_string_vec(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) async fn load_client_config_package_view(
    state: &AppState,
    package_id: &str,
) -> std::result::Result<ClientConfigPackageView, ApiError> {
    let row = sqlx::query(
        r#"
        select package_id, tenant_ref, user_ref, client_id, package_payload, expires_at, created_at
        from v3_client_config_packages
        where tenant_id = $1 and package_id = $2
        "#,
    )
    .bind(state.tenant_id.0)
    .bind(package_id)
    .fetch_optional(state.storage.pool())
    .await
    .map_err(|error| ApiError::from_storage(error.into()))?
    .ok_or_else(|| client_config_package_not_found_error(package_id))?;
    let payload = row.get::<Value, _>("package_payload");
    let artifact_upload = client_config_package_artifact_upload_from_payload(&payload)?;
    let codex_control = client_config_package_codex_control_from_payload(&payload)?;

    Ok(ClientConfigPackageView {
        package_id: row.get("package_id"),
        tenant_id: row.get("tenant_ref"),
        user_id: row.get("user_ref"),
        client_id: row.get("client_id"),
        v3_base_url: payload
            .get("v3_base_url")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        asset_library_ids: client_config_package_string_vec(payload.get("asset_library_ids")),
        dataset_ids: client_config_package_string_vec(payload.get("dataset_ids")),
        skill_packs: client_config_package_string_vec(payload.get("skill_packs")),
        artifact_upload,
        codex_control,
        expires_at: row.get("expires_at"),
        created_at: row.get("created_at"),
        config_package: payload,
    })
}

pub(crate) async fn create_client_config_package_and_load_view(
    state: &AppState,
    headers: &HeaderMap,
    current_user_id: UserId,
    request: CreateClientConfigPackageRequest,
) -> std::result::Result<ClientConfigPackageView, ApiError> {
    validate_v3_client_ref_list("dataset_ids", &request.dataset_ids)?;
    validate_v3_client_ref_list("asset_library_ids", &request.asset_library_ids)?;
    validate_v3_client_ref_list("skill_packs", &request.skill_packs)?;
    validate_v3_client_scope_refs(
        state,
        headers,
        Some(current_user_id),
        &request.dataset_ids,
        &request.asset_library_ids,
    )
    .await?;

    let artifact_upload = request
        .artifact_upload
        .clone()
        .unwrap_or_else(default_client_artifact_upload_config);
    validate_client_artifact_upload_config(&artifact_upload)?;

    let tenant_ref = client_config_ref_or_default(request.tenant_id.as_deref(), state.tenant_id);
    let user_ref = client_config_ref_or_default(request.user_id.as_deref(), current_user_id);
    let client_id = request.client_id.trim().to_string();
    let package_id = format!("v3cp_{}", Uuid::new_v4().simple());
    let terminal_id = format!("term_{}", package_id.trim_start_matches("v3cp_"));
    let codex_control = resolve_codex_control_config(request.codex_control.clone(), &terminal_id);
    validate_codex_control_config(&codex_control)?;
    let payload = build_client_config_package_payload(
        &request,
        &tenant_ref,
        &user_ref,
        &client_id,
        artifact_upload,
        codex_control,
    );

    sqlx::query(
        r#"
        insert into v3_client_config_packages (
            package_id, tenant_id, owner_user_id, tenant_ref, user_ref,
            client_id, package_payload, expires_at
        )
        values ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
    )
    .bind(&package_id)
    .bind(state.tenant_id.0)
    .bind(current_user_id.0)
    .bind(payload["tenant_id"].as_str().unwrap_or_default())
    .bind(payload["user_id"].as_str().unwrap_or_default())
    .bind(&client_id)
    .bind(&payload)
    .bind(request.expires_at)
    .execute(state.storage.pool())
    .await
    .map_err(|error| ApiError::from_storage(error.into()))?;

    load_client_config_package_view(state, &package_id).await
}

pub(crate) async fn create_client_config_package_and_load_response(
    state: &AppState,
    headers: &HeaderMap,
    current_user_id: UserId,
    request: CreateClientConfigPackageRequest,
) -> std::result::Result<CreateClientConfigPackageResponse, ApiError> {
    validate_create_client_config_package_request(&request)?;
    let package =
        create_client_config_package_and_load_view(state, headers, current_user_id, request)
            .await?;
    Ok(create_client_config_package_response(package))
}

fn validate_create_client_config_package_request(
    request: &CreateClientConfigPackageRequest,
) -> std::result::Result<(), ApiError> {
    validate_required("client_id", &request.client_id)?;
    validate_required("v3_base_url", &request.v3_base_url)
}

fn create_client_config_package_response(
    package: ClientConfigPackageView,
) -> CreateClientConfigPackageResponse {
    CreateClientConfigPackageResponse { package }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use serde_json::json;

    use super::*;

    fn request() -> CreateClientConfigPackageRequest {
        CreateClientConfigPackageRequest {
            tenant_id: Some(" tenant-ext ".to_string()),
            user_id: None,
            client_id: " client-001 ".to_string(),
            v3_base_url: " https://v3.elepcloud.com ".to_string(),
            asset_library_ids: vec!["asset-1".to_string()],
            dataset_ids: vec!["dataset-1".to_string()],
            skill_packs: vec!["reporting".to_string()],
            artifact_upload: None,
            codex_control: None,
            expires_at: Some(
                Utc.with_ymd_and_hms(2026, 6, 17, 18, 0, 0)
                    .single()
                    .expect("valid timestamp"),
            ),
            metadata: json!({ "purpose": "smoke" }),
        }
    }

    #[test]
    fn client_config_package_support_defaults_artifact_upload() {
        let config = default_client_artifact_upload_config();

        assert_eq!(config.mode, "session_token");
        assert_eq!(config.endpoint, "/v1/client-artifacts");
    }

    #[test]
    fn client_config_package_support_trims_or_falls_back_refs() {
        assert_eq!(
            client_config_ref_or_default(Some(" tenant-ext "), "tenant-default"),
            "tenant-ext"
        );
        assert_eq!(
            client_config_ref_or_default(Some("  "), "tenant-default"),
            "tenant-default"
        );
        assert_eq!(
            client_config_ref_or_default(None, "tenant-default"),
            "tenant-default"
        );
    }

    #[test]
    fn client_config_package_support_builds_stable_payload() {
        let request = request();
        let payload = build_client_config_package_payload(
            &request,
            "tenant-ext",
            "user-default",
            "client-001",
            default_client_artifact_upload_config(),
            default_codex_control_config("term-001"),
        );

        assert_eq!(payload["schema"], V3_CLIENT_CONFIG_SCHEMA);
        assert_eq!(payload["tenant_id"], "tenant-ext");
        assert_eq!(payload["user_id"], "user-default");
        assert_eq!(payload["client_id"], "client-001");
        assert_eq!(payload["v3_base_url"], "https://v3.elepcloud.com");
        assert_eq!(payload["dataset_ids"], json!(["dataset-1"]));
        assert_eq!(payload["asset_library_ids"], json!(["asset-1"]));
        assert_eq!(payload["skill_packs"], json!(["reporting"]));
        assert_eq!(payload["artifact_upload"]["mode"], "session_token");
        assert_eq!(
            payload["codex_control"]["base_url"],
            DEFAULT_CODEX_CONTROL_BASE_URL
        );
        assert_eq!(
            payload["codex_control"]["activation_endpoint"],
            DEFAULT_CODEX_ACTIVATION_ENDPOINT
        );
        assert_eq!(
            payload["codex_control"]["heartbeat_endpoint"],
            DEFAULT_CODEX_HEARTBEAT_ENDPOINT
        );
        assert_eq!(
            payload["codex_control"]["revoke_endpoint"],
            DEFAULT_CODEX_REVOKE_ENDPOINT
        );
        assert_eq!(
            payload["codex_control"]["plans_endpoint"],
            DEFAULT_CODEX_PLANS_ENDPOINT
        );
        assert_eq!(
            payload["codex_control"]["quota_summary_endpoint"],
            DEFAULT_CODEX_QUOTA_SUMMARY_ENDPOINT
        );
        assert_eq!(
            payload["codex_control"]["terminal_summary_endpoint"],
            DEFAULT_CODEX_TERMINAL_SUMMARY_ENDPOINT
        );
        assert_eq!(
            payload["codex_control"]["billing_url"],
            format!("{DEFAULT_CODEX_CONTROL_BASE_URL}{DEFAULT_CODEX_BILLING_PATH}")
        );
        assert_eq!(
            payload["codex_control"]["activation_token_env"],
            DEFAULT_CODEX_ACTIVATION_TOKEN_ENV
        );
        assert_eq!(payload["codex_control"]["terminal_id"], "term-001");
        assert!(payload["codex_control"].get("activation_token").is_none());
        assert_eq!(payload["metadata"]["purpose"], "smoke");
        assert_eq!(payload["expires_at"], "2026-06-17T18:00:00Z");
    }

    #[test]
    fn client_config_package_support_omits_null_metadata() {
        let mut request = request();
        request.metadata = Value::Null;

        let payload = build_client_config_package_payload(
            &request,
            "tenant-ext",
            "user-default",
            "client-001",
            default_client_artifact_upload_config(),
            default_codex_control_config("term-001"),
        );

        assert!(payload.get("metadata").is_none());
    }

    #[test]
    fn client_config_package_support_decodes_artifact_upload_from_payload() {
        let payload = json!({
            "artifact_upload": {
                "mode": "session_token",
                "endpoint": "/v1/client-artifacts"
            }
        });

        let config = client_config_package_artifact_upload_from_payload(&payload)
            .expect("artifact upload should decode");

        assert_eq!(config.mode, "session_token");
        assert_eq!(config.endpoint, "/v1/client-artifacts");
    }

    #[test]
    fn client_config_package_support_defaults_missing_artifact_upload_on_decode() {
        let config = client_config_package_artifact_upload_from_payload(&json!({}))
            .expect("missing artifact upload should default");

        assert_eq!(config, default_client_artifact_upload_config());
    }

    #[test]
    fn client_config_package_support_resolves_codex_control_defaults() {
        let config = resolve_codex_control_config(
            Some(V3CodexControlConfigView {
                base_url: Some(" https://custom.example.com ".to_string()),
                terminal_label: Some(" Finance PC ".to_string()),
                ..V3CodexControlConfigView::default()
            }),
            "term-001",
        );

        assert_eq!(
            config.base_url.as_deref(),
            Some("https://custom.example.com")
        );
        assert_eq!(
            config.activation_endpoint.as_deref(),
            Some(DEFAULT_CODEX_ACTIVATION_ENDPOINT)
        );
        assert_eq!(
            config.activation_token_env.as_deref(),
            Some(DEFAULT_CODEX_ACTIVATION_TOKEN_ENV)
        );
        assert_eq!(
            config.plans_endpoint.as_deref(),
            Some(DEFAULT_CODEX_PLANS_ENDPOINT)
        );
        assert_eq!(
            config.quota_summary_endpoint.as_deref(),
            Some(DEFAULT_CODEX_QUOTA_SUMMARY_ENDPOINT)
        );
        assert_eq!(
            config.terminal_summary_endpoint.as_deref(),
            Some(DEFAULT_CODEX_TERMINAL_SUMMARY_ENDPOINT)
        );
        assert_eq!(
            config.billing_url.as_deref(),
            Some("https://custom.example.com/codex/billing")
        );
        assert_eq!(config.terminal_id.as_deref(), Some("term-001"));
        assert_eq!(config.terminal_label.as_deref(), Some("Finance PC"));
        validate_codex_control_config(&config).expect("default codex control should be valid");
    }

    #[test]
    fn client_config_package_support_decodes_codex_control_from_payload() {
        let payload = json!({
            "codex_control": {
                "base_url": "https://ad.goods-editor.com",
                "activation_endpoint": "/api/codex/clients/activate",
                "heartbeat_endpoint": "/api/codex/clients/heartbeat",
                "revoke_endpoint": "/api/codex/clients/self-revoke",
                "plans_endpoint": "/api/codex/quotas/plans",
                "quota_summary_endpoint": "/api/codex/quotas/summary",
                "terminal_summary_endpoint": "/api/codex/clients/terminal-summary",
                "billing_url": "https://ad.goods-editor.com/codex/billing",
                "activation_token_env": "CODEX_CLIENT_ACTIVATION_TOKEN",
                "terminal_id": "term-001",
                "session_ttl_seconds": 2592000
            }
        });

        let config = client_config_package_codex_control_from_payload(&payload)
            .expect("codex control should decode")
            .expect("codex control should be present");

        assert_eq!(
            config.base_url.as_deref(),
            Some("https://ad.goods-editor.com")
        );
        assert_eq!(
            config.billing_url.as_deref(),
            Some("https://ad.goods-editor.com/codex/billing")
        );
        assert_eq!(config.terminal_id.as_deref(), Some("term-001"));
    }

    #[test]
    fn client_config_package_support_keeps_legacy_payloads_without_codex_control() {
        let config = client_config_package_codex_control_from_payload(&json!({}))
            .expect("missing codex control should not fail");

        assert!(config.is_none());
    }

    #[test]
    fn client_config_package_support_reports_decode_failures() {
        let error =
            client_config_package_artifact_upload_from_payload(&json!({"artifact_upload": "bad"}))
                .expect_err("bad artifact upload should fail");

        assert_eq!(error.payload.code, "client_config_package_decode_failed");
    }

    #[test]
    fn client_config_package_support_reads_string_arrays() {
        assert_eq!(
            client_config_package_string_vec(Some(&json!(["dataset-1", 42, "dataset-2"]))),
            vec!["dataset-1".to_string(), "dataset-2".to_string()]
        );
        assert!(client_config_package_string_vec(Some(&json!("dataset-1"))).is_empty());
        assert!(client_config_package_string_vec(None).is_empty());
    }

    #[test]
    fn client_config_package_support_keeps_create_required_validation() {
        validate_create_client_config_package_request(&request())
            .expect("default request should pass required validation");

        let mut missing_client = request();
        missing_client.client_id = "   ".to_string();
        let client_error = validate_create_client_config_package_request(&missing_client)
            .expect_err("blank client id should fail");
        assert_eq!(client_error.payload.code, "validation_error");
        assert!(client_error.payload.message.contains("client_id"));

        let mut missing_base_url = request();
        missing_base_url.v3_base_url = "   ".to_string();
        let base_url_error = validate_create_client_config_package_request(&missing_base_url)
            .expect_err("blank base url should fail");
        assert_eq!(base_url_error.payload.code, "validation_error");
        assert!(base_url_error.payload.message.contains("v3_base_url"));
    }
}
