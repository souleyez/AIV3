use contracts::{
    CreateClientConfigPackageRequest, V3ClientArtifactUploadConfigView, V3_CLIENT_CONFIG_SCHEMA,
};
use serde_json::{json, Value};

pub(crate) fn default_client_artifact_upload_config() -> V3ClientArtifactUploadConfigView {
    V3ClientArtifactUploadConfigView {
        mode: "session_token".to_string(),
        endpoint: "/v1/client-artifacts".to_string(),
    }
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
        "expires_at": request.expires_at,
    });
    if !request.metadata.is_null() {
        payload["metadata"] = request.metadata.clone();
    }
    payload
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
        );

        assert!(payload.get("metadata").is_none());
    }
}
