use axum::body::Bytes;
use contracts::{
    V3ClientArtifactManifestView, V3ClientArtifactUploadConfigView,
    V3_CLIENT_ARTIFACT_MANIFEST_SCHEMA, V3_CLIENT_ARTIFACT_SOURCE,
};

use crate::{validate_required, validate_v3_client_ref_list, ApiError};

pub(crate) const V3_CLIENT_ARTIFACT_MAX_FILES: usize = 16;
pub(crate) const V3_CLIENT_ARTIFACT_MAX_FILE_BYTES: usize = 20 * 1024 * 1024;
pub(crate) const V3_CLIENT_ARTIFACT_MAX_HTML_PREVIEW_BYTES: usize = 5 * 1024 * 1024;

#[derive(Debug)]
pub(crate) struct UploadedClientArtifactFile {
    pub(crate) filename: String,
    pub(crate) bytes: Bytes,
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
}
