use contracts::ClientArtifactView;
use domain_model::UserId;
use serde_json::Value;
use sqlx::Row;
use uuid::Uuid;

use crate::client_artifact_storage_support::{
    read_client_artifact_file_storage, ClientArtifactFileStorageRecord,
};
use crate::{
    client_artifact_error_support::{
        client_artifact_file_not_found_error, client_artifact_not_found_error,
    },
    client_artifact_publish_support::{
        client_artifact_file_record_view, select_client_artifact_public_html_candidate,
        ClientArtifactPublicHtmlCandidate,
    },
    ApiError, AppState,
};

pub(crate) struct ClientArtifactDownloadFile {
    pub(crate) filename: String,
    pub(crate) content_type: String,
    pub(crate) bytes: Vec<u8>,
}

pub(crate) struct ClientArtifactPreviewFile {
    pub(crate) owner_user_id: Option<UserId>,
    pub(crate) status: String,
    pub(crate) title: String,
    pub(crate) filename: String,
    pub(crate) content_type: String,
    pub(crate) role: String,
    pub(crate) size_bytes: i64,
    pub(crate) bytes: Vec<u8>,
}

pub(crate) struct ClientArtifactPublicHtmlRecord {
    pub(crate) artifact_db_id: Uuid,
    pub(crate) owner_user_id: Option<UserId>,
    pub(crate) status: String,
    pub(crate) title: String,
    pub(crate) manifest: Value,
    pub(crate) dataset_ids: Vec<String>,
    pub(crate) asset_library_ids: Vec<String>,
}

pub(crate) struct ClientArtifactPublishRecord {
    pub(crate) owner_user_id: Option<UserId>,
    pub(crate) manifest: Value,
}

pub(crate) fn client_artifacts_list_limit(limit: Option<i64>) -> i64 {
    limit.unwrap_or(50).clamp(1, 100)
}

pub(crate) async fn list_client_artifact_views(
    state: &AppState,
    limit: i64,
) -> std::result::Result<Vec<ClientArtifactView>, ApiError> {
    let rows = sqlx::query(
        r#"
        select artifact_id
        from v3_client_artifacts
        where tenant_id = $1
        order by created_at desc
        limit $2
        "#,
    )
    .bind(state.tenant_id.0)
    .bind(limit)
    .fetch_all(state.storage.pool())
    .await
    .map_err(|error| ApiError::from_storage(error.into()))?;
    let mut artifacts = Vec::with_capacity(rows.len());
    for row in rows {
        let artifact_id: String = row.get("artifact_id");
        artifacts.push(load_client_artifact_view(state, &artifact_id).await?);
    }
    Ok(artifacts)
}

pub(crate) async fn list_client_artifact_views_for_query_limit(
    state: &AppState,
    limit: Option<i64>,
) -> std::result::Result<Vec<ClientArtifactView>, ApiError> {
    list_client_artifact_views(state, client_artifacts_list_limit(limit)).await
}

pub(crate) async fn get_client_artifact_view_for_request(
    state: &AppState,
    artifact_id: &str,
) -> std::result::Result<ClientArtifactView, ApiError> {
    load_client_artifact_view(state, artifact_id).await
}

pub(crate) async fn load_client_artifact_download_file(
    state: &AppState,
    artifact_id: &str,
    file_index: i32,
) -> std::result::Result<ClientArtifactDownloadFile, ApiError> {
    let row = sqlx::query(
        r#"
        select f.filename, f.content_type, f.storage_kind, f.object_locator, f.bytes
        from v3_client_artifact_files f
        join v3_client_artifacts a on a.id = f.artifact_id
        where a.tenant_id = $1 and a.artifact_id = $2 and f.file_index = $3
        "#,
    )
    .bind(state.tenant_id.0)
    .bind(artifact_id.trim())
    .bind(file_index)
    .fetch_optional(state.storage.pool())
    .await
    .map_err(|error| ApiError::from_storage(error.into()))?
    .ok_or_else(|| client_artifact_file_not_found_error(artifact_id, file_index))?;
    let bytes = read_client_artifact_file_storage(&ClientArtifactFileStorageRecord {
        storage_kind: row.get("storage_kind"),
        object_locator: row.get("object_locator"),
        database_bytes: row.get("bytes"),
    })?;
    Ok(ClientArtifactDownloadFile {
        filename: row.get("filename"),
        content_type: row.get("content_type"),
        bytes,
    })
}

pub(crate) async fn load_client_artifact_preview_file(
    state: &AppState,
    artifact_id: &str,
    file_index: i32,
) -> std::result::Result<ClientArtifactPreviewFile, ApiError> {
    let row = sqlx::query(
        r#"
        select a.owner_user_id, a.status, a.title,
               f.filename, f.content_type, f.role, f.size_bytes,
               f.storage_kind, f.object_locator, f.bytes
        from v3_client_artifact_files f
        join v3_client_artifacts a on a.id = f.artifact_id
        where a.tenant_id = $1 and a.artifact_id = $2 and f.file_index = $3
        "#,
    )
    .bind(state.tenant_id.0)
    .bind(artifact_id.trim())
    .bind(file_index)
    .fetch_optional(state.storage.pool())
    .await
    .map_err(|error| ApiError::from_storage(error.into()))?
    .ok_or_else(|| client_artifact_file_not_found_error(artifact_id, file_index))?;
    let bytes = read_client_artifact_file_storage(&ClientArtifactFileStorageRecord {
        storage_kind: row.get("storage_kind"),
        object_locator: row.get("object_locator"),
        database_bytes: row.get("bytes"),
    })?;
    Ok(ClientArtifactPreviewFile {
        owner_user_id: row.get::<Option<Uuid>, _>("owner_user_id").map(UserId),
        status: row.get("status"),
        title: row.get("title"),
        filename: row.get("filename"),
        content_type: row.get("content_type"),
        role: row.get("role"),
        size_bytes: row.get("size_bytes"),
        bytes,
    })
}

pub(crate) async fn load_client_artifact_public_html_candidate_file(
    state: &AppState,
    artifact_db_id: Uuid,
) -> std::result::Result<ClientArtifactPublicHtmlCandidate, ApiError> {
    let file_rows = sqlx::query(
        r#"
        select file_index, filename, content_type, role, size_bytes,
               storage_kind, object_locator, bytes
        from v3_client_artifact_files
        where artifact_id = $1
        order by case when role = 'primary_html' then 0 else 1 end, file_index asc
        "#,
    )
    .bind(artifact_db_id)
    .fetch_all(state.storage.pool())
    .await
    .map_err(|error| ApiError::from_storage(error.into()))?;
    select_client_artifact_public_html_candidate(
        file_rows
            .into_iter()
            .map(|row| ClientArtifactPublicHtmlCandidate {
                file_index: row.get("file_index"),
                filename: row.get("filename"),
                content_type: row.get("content_type"),
                role: row.get("role"),
                size_bytes: row.get("size_bytes"),
                storage_kind: row.get("storage_kind"),
                object_locator: row.get("object_locator"),
                database_bytes: row.get("bytes"),
            })
            .collect(),
    )
}

pub(crate) async fn load_client_artifact_public_html_record(
    state: &AppState,
    artifact_id: &str,
) -> std::result::Result<ClientArtifactPublicHtmlRecord, ApiError> {
    let row = sqlx::query(
        r#"
        select id, owner_user_id, status, title, manifest, dataset_ids, asset_library_ids
        from v3_client_artifacts
        where tenant_id = $1 and artifact_id = $2
        "#,
    )
    .bind(state.tenant_id.0)
    .bind(artifact_id)
    .fetch_optional(state.storage.pool())
    .await
    .map_err(|error| ApiError::from_storage(error.into()))?
    .ok_or_else(|| client_artifact_not_found_error(artifact_id))?;
    Ok(ClientArtifactPublicHtmlRecord {
        artifact_db_id: row.get("id"),
        owner_user_id: row.get::<Option<Uuid>, _>("owner_user_id").map(UserId),
        status: row.get("status"),
        title: row.get("title"),
        manifest: row.get("manifest"),
        dataset_ids: row.get("dataset_ids"),
        asset_library_ids: row.get("asset_library_ids"),
    })
}

pub(crate) async fn load_client_artifact_publish_record(
    state: &AppState,
    artifact_id: &str,
) -> std::result::Result<ClientArtifactPublishRecord, ApiError> {
    let row = sqlx::query(
        r#"
        select owner_user_id, manifest
        from v3_client_artifacts
        where tenant_id = $1 and artifact_id = $2
        "#,
    )
    .bind(state.tenant_id.0)
    .bind(artifact_id)
    .fetch_optional(state.storage.pool())
    .await
    .map_err(|error| ApiError::from_storage(error.into()))?
    .ok_or_else(|| client_artifact_not_found_error(artifact_id))?;
    Ok(ClientArtifactPublishRecord {
        owner_user_id: row.get::<Option<Uuid>, _>("owner_user_id").map(UserId),
        manifest: row.get("manifest"),
    })
}

pub(crate) async fn load_client_artifact_view(
    state: &AppState,
    artifact_id: &str,
) -> std::result::Result<ClientArtifactView, ApiError> {
    let row = sqlx::query(
        r#"
        select id, artifact_id, tenant_ref, user_ref, client_id, task_id, title,
               artifact_type, status, manifest, dataset_ids, asset_library_ids,
               created_at, updated_at
        from v3_client_artifacts
        where tenant_id = $1 and artifact_id = $2
        "#,
    )
    .bind(state.tenant_id.0)
    .bind(artifact_id)
    .fetch_optional(state.storage.pool())
    .await
    .map_err(|error| ApiError::from_storage(error.into()))?
    .ok_or_else(|| client_artifact_not_found_error(artifact_id))?;
    let artifact_db_id = row.get::<Uuid, _>("id");
    let file_rows = sqlx::query(
        r#"
        select file_index, filename, content_type, role, size_bytes, sha256
        from v3_client_artifact_files
        where artifact_id = $1
        order by file_index asc
        "#,
    )
    .bind(artifact_db_id)
    .fetch_all(state.storage.pool())
    .await
    .map_err(|error| ApiError::from_storage(error.into()))?;
    let artifact_status: String = row.get("status");
    let manifest_value: Value = row.get("manifest");
    let files = file_rows
        .into_iter()
        .map(|row| {
            let file_index = row.get::<i32, _>("file_index");
            client_artifact_file_record_view(
                artifact_id,
                &artifact_status,
                &manifest_value,
                file_index,
                row.get("filename"),
                row.get("content_type"),
                row.get("role"),
                row.get("size_bytes"),
                row.get("sha256"),
            )
        })
        .collect();

    Ok(ClientArtifactView {
        artifact_id: row.get("artifact_id"),
        tenant_id: row.get("tenant_ref"),
        user_id: row.get("user_ref"),
        client_id: row.get("client_id"),
        task_id: row.get("task_id"),
        title: row.get("title"),
        artifact_type: row.get("artifact_type"),
        status: artifact_status,
        dataset_ids: row.get("dataset_ids"),
        asset_library_ids: row.get("asset_library_ids"),
        files,
        manifest: manifest_value,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_artifact_view_support_clamps_list_limit() {
        assert_eq!(client_artifacts_list_limit(None), 50);
        assert_eq!(client_artifacts_list_limit(Some(-10)), 1);
        assert_eq!(client_artifacts_list_limit(Some(0)), 1);
        assert_eq!(client_artifacts_list_limit(Some(5)), 5);
        assert_eq!(client_artifacts_list_limit(Some(500)), 100);
    }

    #[test]
    fn client_artifact_view_support_keeps_query_limit_bounds() {
        let limits = [None, Some(-1), Some(1), Some(100), Some(101)];
        let effective = limits
            .into_iter()
            .map(client_artifacts_list_limit)
            .collect::<Vec<_>>();

        assert_eq!(effective, vec![50, 1, 1, 100, 100]);
    }
}
