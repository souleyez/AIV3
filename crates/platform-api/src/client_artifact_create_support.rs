use axum::http::HeaderMap;
use contracts::{ClientArtifactView, CreateClientArtifactResponse, V3ClientArtifactManifestView};
use domain_model::UserId;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    client_artifact_contract_support::{
        validate_client_artifact_files_match_manifest, validate_client_artifact_manifest,
        UploadedClientArtifactFile,
    },
    client_artifact_scope_support::validate_v3_client_scope_refs,
    client_artifact_storage_support::{
        client_artifact_database_file_bytes_limit_from_env, client_artifact_object_root_from_env,
        prepare_client_artifact_file_storage,
    },
    client_artifact_view_support::load_client_artifact_view,
    client_config_session_support::{
        validate_client_session_manifest_scope, V3ClientSessionClaims,
    },
    sha256_hex, ApiError, AppState,
};

pub(crate) fn new_client_artifact_id() -> String {
    format!("v3ca_{}", Uuid::new_v4().simple())
}

pub(crate) fn client_artifact_file_sha256_hex(bytes: &[u8]) -> String {
    sha256_hex([bytes])
}

pub(crate) async fn validate_client_artifact_create_request(
    state: &AppState,
    headers: &HeaderMap,
    current_user_id: Option<UserId>,
    client_session: Option<&V3ClientSessionClaims>,
    manifest: &V3ClientArtifactManifestView,
    files: &[UploadedClientArtifactFile],
) -> std::result::Result<(), ApiError> {
    validate_client_artifact_manifest(manifest)?;
    if let Some(claims) = client_session {
        validate_client_session_manifest_scope(claims, manifest)?;
    } else {
        validate_v3_client_scope_refs(
            state,
            headers,
            current_user_id,
            &manifest.dataset_ids,
            &manifest.asset_library_ids,
        )
        .await?;
    }
    validate_client_artifact_files_match_manifest(manifest, files)
}

pub(crate) async fn insert_client_artifact_with_files(
    state: &AppState,
    artifact_id: &str,
    owner_user_id: Option<UserId>,
    manifest: &V3ClientArtifactManifestView,
    files: &[UploadedClientArtifactFile],
) -> std::result::Result<(), ApiError> {
    let manifest_value = serde_json::to_value(manifest)
        .map_err(|error| ApiError::internal("manifest_encode_failed", error.to_string()))?;
    let mut tx = state
        .storage
        .pool()
        .begin()
        .await
        .map_err(|error| ApiError::from_storage(error.into()))?;
    let row = sqlx::query(
        r#"
        insert into v3_client_artifacts (
            artifact_id, tenant_id, owner_user_id, tenant_ref, user_ref,
            client_id, task_id, title, artifact_type, status, manifest,
            dataset_ids, asset_library_ids
        )
        values ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'received', $10, $11, $12)
        returning id
        "#,
    )
    .bind(artifact_id)
    .bind(state.tenant_id.0)
    .bind(owner_user_id.map(|user_id| user_id.0))
    .bind(manifest.tenant_id.trim())
    .bind(manifest.user_id.trim())
    .bind(manifest.client_id.trim())
    .bind(manifest.task_id.trim())
    .bind(manifest.title.trim())
    .bind(manifest.artifact_type.trim())
    .bind(&manifest_value)
    .bind(&manifest.dataset_ids)
    .bind(&manifest.asset_library_ids)
    .fetch_one(&mut *tx)
    .await
    .map_err(|error| ApiError::from_storage(error.into()))?;
    let artifact_db_id = row.get::<Uuid, _>("id");
    let client_artifact_object_root = client_artifact_object_root_from_env();
    let client_artifact_database_file_bytes_limit =
        client_artifact_database_file_bytes_limit_from_env();

    for (index, descriptor) in manifest.files.iter().enumerate() {
        let file = &files[index];
        let sha256 = client_artifact_file_sha256_hex(file.bytes.as_ref());
        let storage = prepare_client_artifact_file_storage(
            client_artifact_object_root.as_deref(),
            client_artifact_database_file_bytes_limit,
            state.tenant_id.0,
            artifact_id,
            index as i32,
            &sha256,
            file.bytes.as_ref(),
        )?;
        sqlx::query(
            r#"
            insert into v3_client_artifact_files (
                artifact_id, file_index, filename, content_type, role,
                size_bytes, sha256, storage_kind, object_locator, bytes
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
        .bind(artifact_db_id)
        .bind(index as i32)
        .bind(descriptor.filename.trim())
        .bind(descriptor.content_type.trim())
        .bind(descriptor.role.trim())
        .bind(file.bytes.len() as i64)
        .bind(sha256)
        .bind(storage.storage_kind)
        .bind(storage.object_locator)
        .bind(storage.database_bytes)
        .execute(&mut *tx)
        .await
        .map_err(|error| ApiError::from_storage(error.into()))?;
    }

    tx.commit()
        .await
        .map_err(|error| ApiError::from_storage(error.into()))?;
    Ok(())
}

pub(crate) async fn create_client_artifact_from_upload(
    state: &AppState,
    headers: &HeaderMap,
    current_user_id: Option<UserId>,
    client_session: Option<&V3ClientSessionClaims>,
    manifest: &V3ClientArtifactManifestView,
    files: &[UploadedClientArtifactFile],
) -> std::result::Result<ClientArtifactView, ApiError> {
    validate_client_artifact_create_request(
        state,
        headers,
        current_user_id,
        client_session,
        manifest,
        files,
    )
    .await?;
    let artifact_id = new_client_artifact_id();
    insert_client_artifact_with_files(state, &artifact_id, current_user_id, manifest, files)
        .await?;
    load_client_artifact_view(state, &artifact_id).await
}

pub(crate) async fn create_client_artifact_from_upload_response(
    state: &AppState,
    headers: &HeaderMap,
    current_user_id: Option<UserId>,
    client_session: Option<&V3ClientSessionClaims>,
    manifest: &V3ClientArtifactManifestView,
    files: &[UploadedClientArtifactFile],
) -> std::result::Result<CreateClientArtifactResponse, ApiError> {
    let artifact = create_client_artifact_from_upload(
        state,
        headers,
        current_user_id,
        client_session,
        manifest,
        files,
    )
    .await?;
    Ok(create_client_artifact_response(artifact))
}

fn create_client_artifact_response(artifact: ClientArtifactView) -> CreateClientArtifactResponse {
    CreateClientArtifactResponse { artifact }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;

    #[test]
    fn client_artifact_create_support_generates_prefixed_ids() {
        let artifact_id = new_client_artifact_id();

        assert!(artifact_id.starts_with("v3ca_"));
        assert_eq!(artifact_id.len(), "v3ca_".len() + 32);
        assert!(artifact_id["v3ca_".len()..]
            .chars()
            .all(|ch| ch.is_ascii_hexdigit()));
    }

    #[test]
    fn client_artifact_create_support_hashes_file_bytes() {
        assert_eq!(
            client_artifact_file_sha256_hex(b"client artifact"),
            "ab9439bd568e9a2bcdc16cc45c8a18b24012d7ce7c02e151e2a4ddf0354c44eb"
        );
    }

    #[test]
    fn client_artifact_create_support_builds_create_response() {
        let now = Utc::now();
        let artifact = ClientArtifactView {
            artifact_id: "v3ca_1".to_string(),
            tenant_id: "tenant-1".to_string(),
            user_id: "user-1".to_string(),
            client_id: "client-1".to_string(),
            task_id: "task-1".to_string(),
            title: "客户产物".to_string(),
            artifact_type: "html".to_string(),
            status: "received".to_string(),
            dataset_ids: vec!["dataset-1".to_string()],
            asset_library_ids: Vec::new(),
            files: Vec::new(),
            manifest: json!({"kind": "client_artifact"}),
            created_at: now,
            updated_at: now,
        };

        let response = create_client_artifact_response(artifact.clone());

        assert_eq!(response.artifact.artifact_id, artifact.artifact_id);
        assert_eq!(response.artifact.status, "received");
    }
}
