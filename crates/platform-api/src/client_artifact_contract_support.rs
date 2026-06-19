use axum::{
    body::Bytes,
    extract::Multipart,
    http::{header, HeaderMap},
};
use contracts::{
    V3ClientArtifactManifestView, V3ClientArtifactUploadConfigView,
    V3_CLIENT_ARTIFACT_MANIFEST_SCHEMA, V3_CLIENT_ARTIFACT_SOURCE,
};
use std::collections::BTreeSet;

use crate::{validate_required, ApiError};

pub(crate) const V3_CLIENT_ARTIFACT_MAX_FILES: usize = 16;
pub(crate) const V3_CLIENT_ARTIFACT_MAX_FILE_BYTES: usize = 20 * 1024 * 1024;
pub(crate) const V3_CLIENT_ARTIFACT_MAX_HTML_PREVIEW_BYTES: usize = 5 * 1024 * 1024;

#[derive(Debug)]
pub(crate) struct UploadedClientArtifactFile {
    pub(crate) filename: String,
    pub(crate) bytes: Bytes,
}

pub(crate) async fn parse_client_artifact_multipart(
    mut multipart: Multipart,
) -> std::result::Result<
    (
        V3ClientArtifactManifestView,
        Vec<UploadedClientArtifactFile>,
    ),
    ApiError,
> {
    let mut manifest = None;
    let mut files = Vec::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| ApiError::bad_request("invalid_multipart", error.to_string()))?
    {
        let name = field.name().unwrap_or_default().to_string();
        match name.as_str() {
            "manifest" => {
                let text = field.text().await.map_err(|error| {
                    ApiError::bad_request("invalid_manifest", error.to_string())
                })?;
                manifest = Some(parse_client_artifact_manifest_text(&text)?);
            }
            "files" => {
                if files.len() >= V3_CLIENT_ARTIFACT_MAX_FILES {
                    return Err(ApiError::bad_request(
                        "too_many_artifact_files",
                        format!("at most {V3_CLIENT_ARTIFACT_MAX_FILES} files are allowed"),
                    ));
                }
                let filename = field.file_name().unwrap_or_default().to_string();
                let bytes = field.bytes().await.map_err(|error| {
                    ApiError::bad_request("invalid_artifact_file", error.to_string())
                })?;
                validate_client_artifact_uploaded_file_size(&filename, bytes.len())?;
                files.push(UploadedClientArtifactFile { filename, bytes });
            }
            _ => {}
        }
    }

    let manifest = manifest.ok_or_else(|| {
        ApiError::bad_request(
            "manifest_required",
            "multipart field manifest is required".to_string(),
        )
    })?;
    Ok((manifest, files))
}

pub(crate) fn parse_client_artifact_manifest_text(
    text: &str,
) -> std::result::Result<V3ClientArtifactManifestView, ApiError> {
    serde_json::from_str::<V3ClientArtifactManifestView>(text).map_err(|error| {
        ApiError::bad_request(
            "invalid_manifest",
            format!("manifest must be v3.client_artifact_manifest.v1 JSON: {error}"),
        )
    })
}

pub(crate) fn validate_client_artifact_uploaded_file_size(
    filename: &str,
    byte_len: usize,
) -> std::result::Result<(), ApiError> {
    if byte_len > V3_CLIENT_ARTIFACT_MAX_FILE_BYTES {
        return Err(ApiError::bad_request(
            "artifact_file_too_large",
            format!("artifact file {filename} exceeds {V3_CLIENT_ARTIFACT_MAX_FILE_BYTES} bytes"),
        ));
    }
    Ok(())
}

pub(crate) fn validate_client_artifact_upload_config(
    config: &V3ClientArtifactUploadConfigView,
) -> std::result::Result<(), ApiError> {
    validate_required("artifact_upload.mode", &config.mode)?;
    validate_required("artifact_upload.endpoint", &config.endpoint)?;
    if config.mode != "session_token" {
        return Err(ApiError::bad_request(
            "invalid_artifact_upload_mode",
            "artifact_upload.mode must be session_token".to_string(),
        ));
    }
    if !config.endpoint.starts_with('/') {
        return Err(ApiError::bad_request(
            "invalid_artifact_upload_endpoint",
            "artifact_upload.endpoint must be an absolute path".to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn validate_client_artifact_manifest(
    manifest: &V3ClientArtifactManifestView,
) -> std::result::Result<(), ApiError> {
    if manifest.schema != V3_CLIENT_ARTIFACT_MANIFEST_SCHEMA {
        return Err(ApiError::bad_request(
            "invalid_client_artifact_manifest_schema",
            format!("schema must be {V3_CLIENT_ARTIFACT_MANIFEST_SCHEMA}"),
        ));
    }
    if manifest.source != V3_CLIENT_ARTIFACT_SOURCE {
        return Err(ApiError::bad_request(
            "invalid_client_artifact_source",
            format!("source must be {V3_CLIENT_ARTIFACT_SOURCE}"),
        ));
    }
    for (field, value) in [
        ("tenant_id", manifest.tenant_id.as_str()),
        ("user_id", manifest.user_id.as_str()),
        ("client_id", manifest.client_id.as_str()),
        ("task_id", manifest.task_id.as_str()),
        ("title", manifest.title.as_str()),
        ("artifact_type", manifest.artifact_type.as_str()),
    ] {
        validate_required(field, value)?;
    }
    validate_v3_client_ref_list("dataset_ids", &manifest.dataset_ids)?;
    validate_v3_client_ref_list("asset_library_ids", &manifest.asset_library_ids)?;
    if manifest.files.is_empty() {
        return Err(ApiError::bad_request(
            "artifact_files_required",
            "manifest.files must contain at least one file".to_string(),
        ));
    }
    if manifest.files.len() > V3_CLIENT_ARTIFACT_MAX_FILES {
        return Err(ApiError::bad_request(
            "too_many_artifact_files",
            format!("at most {V3_CLIENT_ARTIFACT_MAX_FILES} files are allowed"),
        ));
    }
    for file in &manifest.files {
        if !safe_client_artifact_filename(&file.filename) {
            return Err(ApiError::bad_request(
                "invalid_artifact_filename",
                format!("artifact filename {} is invalid", file.filename),
            ));
        }
        validate_required("file.content_type", &file.content_type)?;
        validate_required("file.role", &file.role)?;
    }
    for evidence_ref in &manifest.evidence_refs {
        validate_required("evidence_ref.kind", &evidence_ref.kind)?;
        validate_required("evidence_ref.id", &evidence_ref.id)?;
    }
    Ok(())
}

pub(crate) fn validate_v3_client_ref_list(
    field: &'static str,
    values: &[String],
) -> std::result::Result<(), ApiError> {
    let mut seen = BTreeSet::new();
    for value in values {
        let trimmed = value.trim();
        validate_required(field, trimmed)?;
        if !seen.insert(trimmed.to_string()) {
            return Err(ApiError::bad_request(
                "duplicate_client_scope_ref",
                format!("{field} contains duplicate value {trimmed}"),
            ));
        }
    }
    Ok(())
}

pub(crate) fn client_artifact_upload_bearer_token(headers: &HeaderMap) -> Option<&str> {
    let raw = headers.get(header::AUTHORIZATION)?.to_str().ok()?.trim();
    raw.strip_prefix("Bearer ")
        .or_else(|| raw.strip_prefix("bearer "))
        .map(str::trim)
        .filter(|token| !token.is_empty())
}

pub(crate) fn validate_client_artifact_file_index(
    file_index: i32,
) -> std::result::Result<(), ApiError> {
    if file_index < 0 {
        return Err(ApiError::bad_request(
            "invalid_client_artifact_file_index",
            "file_index must be non-negative".to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn validate_client_artifact_files_match_manifest(
    manifest: &V3ClientArtifactManifestView,
    files: &[UploadedClientArtifactFile],
) -> std::result::Result<(), ApiError> {
    if manifest.files.len() != files.len() {
        return Err(ApiError::bad_request(
            "artifact_file_count_mismatch",
            format!(
                "manifest declares {} files but multipart contains {} files",
                manifest.files.len(),
                files.len()
            ),
        ));
    }
    for (index, descriptor) in manifest.files.iter().enumerate() {
        let uploaded = &files[index];
        if uploaded.filename != descriptor.filename {
            return Err(ApiError::bad_request(
                "artifact_file_descriptor_mismatch",
                format!(
                    "multipart file {} is {}, expected {}",
                    index, uploaded.filename, descriptor.filename
                ),
            ));
        }
    }
    Ok(())
}

pub(crate) fn safe_client_artifact_filename(filename: &str) -> bool {
    let filename = filename.trim();
    !filename.is_empty()
        && filename != "."
        && filename != ".."
        && !filename.contains('/')
        && !filename.contains('\\')
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn manifest() -> V3ClientArtifactManifestView {
        serde_json::from_value(json!({
            "schema": V3_CLIENT_ARTIFACT_MANIFEST_SCHEMA,
            "source": V3_CLIENT_ARTIFACT_SOURCE,
            "tenant_id": "tenant-001",
            "user_id": "user-001",
            "client_id": "client-001",
            "task_id": "task-001",
            "asset_library_ids": ["asset-library-001"],
            "dataset_ids": ["dataset-001"],
            "title": "Static page",
            "artifact_type": "static_page",
            "created_at": "2026-06-17T10:00:00Z",
            "files": [{
                "filename": "index.html",
                "content_type": "text/html",
                "role": "primary_html",
                "size_bytes": 12,
                "sha256": null
            }],
            "evidence_refs": [{"kind": "dataset", "id": "dataset-001"}],
            "created_at": "2026-06-17T09:00:00Z",
            "metadata": {}
        }))
        .expect("manifest fixture should parse")
    }

    #[test]
    fn client_artifact_contract_support_accepts_valid_manifest() {
        validate_client_artifact_manifest(&manifest()).expect("manifest should be valid");
    }

    #[test]
    fn client_artifact_contract_support_parses_manifest_text() {
        let text = serde_json::to_string(&manifest()).expect("manifest should serialize");
        let parsed =
            parse_client_artifact_manifest_text(&text).expect("manifest text should parse");

        assert_eq!(parsed.schema, V3_CLIENT_ARTIFACT_MANIFEST_SCHEMA);
        assert_eq!(parsed.source, V3_CLIENT_ARTIFACT_SOURCE);
        assert_eq!(parsed.files[0].filename, "index.html");
    }

    #[test]
    fn client_artifact_contract_support_rejects_bad_manifest_text() {
        let error =
            parse_client_artifact_manifest_text("{").expect_err("invalid JSON should be rejected");

        assert_eq!(error.payload.code, "invalid_manifest");
        assert!(error
            .payload
            .message
            .contains("manifest must be v3.client_artifact_manifest.v1 JSON"));
    }

    #[test]
    fn client_artifact_contract_support_rejects_oversized_upload_file() {
        validate_client_artifact_uploaded_file_size(
            "index.html",
            V3_CLIENT_ARTIFACT_MAX_FILE_BYTES,
        )
        .expect("file at limit should be accepted");

        let error = validate_client_artifact_uploaded_file_size(
            "index.html",
            V3_CLIENT_ARTIFACT_MAX_FILE_BYTES + 1,
        )
        .expect_err("file above limit should fail");

        assert_eq!(error.payload.code, "artifact_file_too_large");
        assert_eq!(
            error.payload.message,
            format!("artifact file index.html exceeds {V3_CLIENT_ARTIFACT_MAX_FILE_BYTES} bytes")
        );
    }

    #[test]
    fn client_artifact_contract_support_rejects_bad_upload_config() {
        let error = validate_client_artifact_upload_config(&V3ClientArtifactUploadConfigView {
            mode: "long_lived_token".to_string(),
            endpoint: "v1/client-artifacts".to_string(),
        })
        .expect_err("non-session mode should fail");

        assert_eq!(error.payload.code, "invalid_artifact_upload_mode");
    }

    #[test]
    fn client_artifact_contract_support_requires_matching_file_order() {
        let files = vec![UploadedClientArtifactFile {
            filename: "report.md".to_string(),
            bytes: Bytes::from_static(b"# report"),
        }];

        let error = validate_client_artifact_files_match_manifest(&manifest(), &files)
            .expect_err("different multipart filename should fail");

        assert_eq!(error.payload.code, "artifact_file_descriptor_mismatch");
    }

    #[test]
    fn client_artifact_contract_support_rejects_path_filenames() {
        assert!(safe_client_artifact_filename("index.html"));
        assert!(!safe_client_artifact_filename("../index.html"));
        assert!(!safe_client_artifact_filename("nested/index.html"));
        assert!(!safe_client_artifact_filename("nested\\index.html"));
    }

    #[test]
    fn client_artifact_contract_support_rejects_empty_scope_refs() {
        let error = validate_v3_client_ref_list("dataset_ids", &["  ".to_string()])
            .expect_err("empty refs should fail");

        assert_eq!(error.payload.code, "validation_error");
    }

    #[test]
    fn client_artifact_contract_support_rejects_duplicate_scope_refs_after_trim() {
        let error = validate_v3_client_ref_list(
            "asset_library_ids",
            &[" asset-1 ".to_string(), "asset-1".to_string()],
        )
        .expect_err("duplicate refs should fail");

        assert_eq!(error.payload.code, "duplicate_client_scope_ref");
    }

    #[test]
    fn client_artifact_contract_support_extracts_upload_bearer_token() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            "Bearer upload-token ".parse().unwrap(),
        );

        assert_eq!(
            client_artifact_upload_bearer_token(&headers),
            Some("upload-token")
        );

        headers.insert(
            header::AUTHORIZATION,
            "bearer upload-token with space".parse().unwrap(),
        );
        assert_eq!(
            client_artifact_upload_bearer_token(&headers),
            Some("upload-token with space")
        );
    }

    #[test]
    fn client_artifact_contract_support_rejects_missing_or_wrong_upload_bearer() {
        assert!(client_artifact_upload_bearer_token(&HeaderMap::new()).is_none());

        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, "Basic token".parse().unwrap());
        assert!(client_artifact_upload_bearer_token(&headers).is_none());

        headers.insert(header::AUTHORIZATION, "Bearer ".parse().unwrap());
        assert!(client_artifact_upload_bearer_token(&headers).is_none());
    }

    #[test]
    fn client_artifact_contract_support_rejects_negative_file_index() {
        validate_client_artifact_file_index(0).expect("zero index should be valid");
        validate_client_artifact_file_index(3).expect("positive index should be valid");

        let error =
            validate_client_artifact_file_index(-1).expect_err("negative file index should fail");

        assert_eq!(error.payload.code, "invalid_client_artifact_file_index");
        assert_eq!(error.payload.message, "file_index must be non-negative");
    }
}
