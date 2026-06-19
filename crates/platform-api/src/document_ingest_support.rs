use chrono::Utc;
use contracts::{CreateDocumentIngestResponse, WorkflowExecutionView};
use domain_model::{Document, DocumentId, DocumentLifecycle, SecretBindingId, UserId};
use serde_json::json;
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path as StdPath, PathBuf},
};
use storage::NewDocument;
use workflow_engine::WorkflowSignal;
use zip::ZipArchive;

use crate::{
    apply_workflow_signal, build_initial_upload_ingest_event,
    build_initial_upload_ingest_execution,
    external_document_object_support::{external_document_object_root, safe_external_path_segment},
    load_visible_document_for_user_with_local_scope,
    record_local_document_content_fingerprint_if_available, resolve_platform_local_object_path,
    to_document_summaries_with_dataset_ids, to_document_summary,
    zip_ingest_support::{
        infer_zip_child_content_type, safe_zip_entry_output_name, zip_entry_extension_supported,
        zip_entry_should_skip, zip_entry_title, zip_ingest_env_u64, zip_ingest_env_usize,
    },
    ApiError, AppState,
};

pub(crate) async fn create_document_ingest_for_user(
    state: &AppState,
    document_id: DocumentId,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
    local_thread_id: Option<&str>,
) -> std::result::Result<CreateDocumentIngestResponse, ApiError> {
    let document = load_visible_document_for_user_with_local_scope(
        state,
        document_id,
        active_secret_binding_ids,
        current_user_id,
        local_thread_id,
    )
    .await?;
    sync_document_asset_profile_before_ingest(state, &document).await?;

    if document_is_zip_archive(&document) {
        return create_zip_archive_child_ingests(state, &document).await;
    }

    let execution = build_initial_upload_ingest_execution(state, &document)?;
    let initial_event = build_initial_upload_ingest_event(&execution, &document);
    state
        .storage
        .workflow_executions()
        .create_with_initial_event(&execution, &initial_event)
        .await
        .map_err(ApiError::from_storage)?;
    let started = apply_workflow_signal(state, execution.id, WorkflowSignal::Start).await?;

    Ok(create_document_ingest_response(document, started.execution))
}

async fn sync_document_asset_profile_before_ingest(
    state: &AppState,
    document: &Document,
) -> std::result::Result<(), ApiError> {
    let existing_chunks = state
        .storage
        .document_chunks()
        .list_by_document(state.tenant_id, document.id)
        .await
        .map_err(ApiError::from_storage)?;
    if let Err(error) = state
        .storage
        .asset_items()
        .sync_document_asset_profile(state.tenant_id, document, &existing_chunks)
        .await
    {
        tracing::warn!(
            error = ?error,
            document_id = %document.id,
            dataset_id = %document.dataset_id,
            "document asset profile sync failed before ingest workflow; ingest will continue"
        );
    }
    Ok(())
}

fn create_document_ingest_response(
    document: Document,
    workflow_execution: WorkflowExecutionView,
) -> CreateDocumentIngestResponse {
    CreateDocumentIngestResponse {
        document: to_document_summary(document),
        workflow_execution,
        child_documents: Vec::new(),
        child_documents_camel: Vec::new(),
        child_workflow_executions: Vec::new(),
        child_workflow_executions_camel: Vec::new(),
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ExpandedZipEntry {
    pub(crate) entry_name: String,
    pub(crate) title: String,
    pub(crate) object_key: String,
    pub(crate) content_type: String,
    pub(crate) size_bytes: u64,
}

pub(crate) async fn create_zip_archive_child_ingests(
    state: &AppState,
    parent_document: &Document,
) -> std::result::Result<CreateDocumentIngestResponse, ApiError> {
    let entries = expand_zip_document_to_local_files(parent_document)?;
    if entries.is_empty() {
        return Err(ApiError::bad_request(
            "zip_archive_empty",
            "zip archive did not contain supported files to ingest".to_string(),
        ));
    }

    let mut child_documents = Vec::new();
    let mut child_workflow_executions = Vec::new();
    for (index, entry) in entries.into_iter().enumerate() {
        let child_document = state
            .storage
            .documents()
            .create(
                state.tenant_id,
                NewDocument {
                    dataset_id: parent_document.dataset_id,
                    title: entry.title.clone(),
                    object_key: entry.object_key.clone(),
                    content_type: entry.content_type.clone(),
                    secret_binding_ids: parent_document.secret_binding_ids.clone(),
                    owner_user_id: parent_document.owner_user_id,
                    metadata: json!({
                        "zip_parent": {
                            "document_id": parent_document.id,
                            "title": parent_document.title,
                            "object_key": parent_document.object_key,
                        },
                        "zip_entry": {
                            "name": entry.entry_name,
                            "index": index,
                            "size_bytes": entry.size_bytes,
                        },
                        "parse_state": {
                            "stage": "queued",
                            "user_blocking": false,
                        }
                    }),
                },
            )
            .await
            .map_err(ApiError::from_storage)?;
        let child_document = record_local_document_content_fingerprint_if_available(
            state,
            child_document,
            Utc::now(),
        )
        .await;
        if let Err(error) = state
            .storage
            .asset_items()
            .sync_document_asset_profile(state.tenant_id, &child_document, &[])
            .await
        {
            tracing::warn!(
                error = ?error,
                document_id = %child_document.id,
                dataset_id = %child_document.dataset_id,
                "zip child document asset profile sync failed; child ingest will continue"
            );
        }

        let execution = build_initial_upload_ingest_execution(state, &child_document)?;
        let initial_event = build_initial_upload_ingest_event(&execution, &child_document);
        state
            .storage
            .workflow_executions()
            .create_with_initial_event(&execution, &initial_event)
            .await
            .map_err(ApiError::from_storage)?;
        let started = apply_workflow_signal(state, execution.id, WorkflowSignal::Start).await?;
        child_workflow_executions.push(started.execution);
        child_documents.push(child_document);
    }

    let child_document_views =
        to_document_summaries_with_dataset_ids(state, child_documents, None).await?;
    let parent_metadata = json!({
        "parse_status": "zip_expanded",
        "ingest": {
            "processor": "zip_expander",
            "parse_method": "zip-child-documents",
            "parse_status": "zip_expanded",
            "child_document_count": child_document_views.len(),
            "child_document_ids": child_document_views
                .iter()
                .map(|document| document.id.to_string())
                .collect::<Vec<_>>(),
            "extracted_at": Utc::now(),
        }
    });
    let updated_parent = state
        .storage
        .documents()
        .update_state(
            state.tenant_id,
            parent_document.id,
            DocumentLifecycle::Indexed,
            Some(&parent_document.title),
            &parent_metadata,
            Utc::now(),
        )
        .await
        .map_err(ApiError::from_storage)?;
    if let Err(error) = state
        .storage
        .asset_items()
        .sync_document_asset_profile(state.tenant_id, &updated_parent, &[])
        .await
    {
        tracing::warn!(
            error = ?error,
            document_id = %updated_parent.id,
            dataset_id = %updated_parent.dataset_id,
            "zip parent document asset profile sync failed; response will continue"
        );
    }

    Ok(CreateDocumentIngestResponse {
        document: to_document_summary(updated_parent),
        workflow_execution: child_workflow_executions.first().cloned().ok_or_else(|| {
            ApiError::internal(
                "zip_archive_ingest_missing_child_workflow",
                "zip archive expansion did not create child workflows".to_string(),
            )
        })?,
        child_documents: child_document_views.clone(),
        child_documents_camel: child_document_views,
        child_workflow_executions: child_workflow_executions.clone(),
        child_workflow_executions_camel: child_workflow_executions,
    })
}

pub(crate) fn document_is_zip_archive(document: &Document) -> bool {
    let content_type = document
        .content_type
        .split(';')
        .next()
        .unwrap_or(&document.content_type)
        .trim()
        .to_ascii_lowercase();
    matches!(
        content_type.as_str(),
        "application/zip" | "application/x-zip-compressed" | "multipart/x-zip"
    ) || document.title.to_ascii_lowercase().ends_with(".zip")
        || document.object_key.to_ascii_lowercase().ends_with(".zip")
}

pub(crate) fn expand_zip_document_to_local_files(
    document: &Document,
) -> std::result::Result<Vec<ExpandedZipEntry>, ApiError> {
    let path = resolve_platform_local_object_path(&document.object_key).ok_or_else(|| {
        ApiError::bad_request(
            "zip_archive_object_not_found",
            "zip archive object file was not found on this server".to_string(),
        )
    })?;
    let file = File::open(&path).map_err(|error| {
        ApiError::bad_request(
            "zip_archive_open_failed",
            format!("failed to open zip archive: {error}"),
        )
    })?;
    let mut archive = ZipArchive::new(file).map_err(|error| {
        ApiError::bad_request(
            "zip_archive_invalid",
            format!("uploaded file is not a readable zip archive: {error}"),
        )
    })?;

    let max_entries = zip_ingest_env_usize("ZIP_INGEST_MAX_ENTRIES", 80).clamp(1, 500);
    let max_entry_bytes = zip_ingest_env_u64("ZIP_INGEST_MAX_ENTRY_BYTES", 80 * 1024 * 1024).max(1);
    let max_total_bytes =
        zip_ingest_env_u64("ZIP_INGEST_MAX_TOTAL_BYTES", 300 * 1024 * 1024).max(1);
    let output_root = zip_child_output_root(&path, document)?;
    fs::create_dir_all(&output_root).map_err(|error| {
        ApiError::internal(
            "zip_archive_expand_failed",
            format!("failed to create zip extraction directory: {error}"),
        )
    })?;

    let mut entries = Vec::new();
    let mut total_bytes = 0u64;
    for index in 0..archive.len() {
        if entries.len() >= max_entries {
            break;
        }
        let file = archive.by_index(index).map_err(|error| {
            ApiError::bad_request(
                "zip_archive_read_failed",
                format!("failed to read zip entry {index}: {error}"),
            )
        })?;
        if file.is_dir() {
            continue;
        }
        let Some(enclosed_name) = file.enclosed_name().map(PathBuf::from) else {
            continue;
        };
        let entry_name = enclosed_name.to_string_lossy().replace('\\', "/");
        if zip_entry_should_skip(&entry_name) {
            continue;
        }
        let extension = enclosed_name
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| format!(".{}", value.to_ascii_lowercase()))
            .unwrap_or_default();
        if !zip_entry_extension_supported(&extension) {
            continue;
        }
        let entry_size = file.size();
        if entry_size > max_entry_bytes {
            continue;
        }
        total_bytes = total_bytes.saturating_add(entry_size);
        if total_bytes > max_total_bytes {
            return Err(ApiError::bad_request(
                "zip_archive_too_large",
                format!("zip expanded content exceeds {} bytes", max_total_bytes),
            ));
        }
        let safe_name = safe_zip_entry_output_name(entries.len(), &entry_name, &extension);
        let output_path = output_root.join(safe_name);
        let mut output = File::create(&output_path).map_err(|error| {
            ApiError::internal(
                "zip_archive_expand_failed",
                format!("failed to create extracted zip entry: {error}"),
            )
        })?;
        let copied =
            std::io::copy(&mut file.take(max_entry_bytes + 1), &mut output).map_err(|error| {
                ApiError::internal(
                    "zip_archive_expand_failed",
                    format!("failed to write extracted zip entry: {error}"),
                )
            })?;
        output.flush().map_err(|error| {
            ApiError::internal(
                "zip_archive_expand_failed",
                format!("failed to flush extracted zip entry: {error}"),
            )
        })?;
        if copied > max_entry_bytes {
            let _ = fs::remove_file(&output_path);
            continue;
        }
        entries.push(ExpandedZipEntry {
            entry_name: entry_name.clone(),
            title: zip_entry_title(&entry_name),
            object_key: output_path.to_string_lossy().to_string(),
            content_type: infer_zip_child_content_type(&extension).to_string(),
            size_bytes: copied,
        });
    }

    Ok(entries)
}

fn zip_child_output_root(
    zip_path: &StdPath,
    document: &Document,
) -> std::result::Result<PathBuf, ApiError> {
    let base = zip_path
        .parent()
        .map(StdPath::to_path_buf)
        .unwrap_or_else(|| {
            external_document_object_root().unwrap_or_else(|_| std::env::temp_dir())
        });
    Ok(base
        .join("_zip_extracted")
        .join(safe_external_path_segment(&document.id.to_string())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{
        DatasetId, DocumentLifecycle, TenantId, WorkflowExecutionId, WorkflowKind, WorkflowStatus,
    };
    use std::collections::BTreeMap;
    use uuid::Uuid;

    fn document(title: &str) -> Document {
        Document {
            id: DocumentId(Uuid::from_u128(1)),
            tenant_id: TenantId(Uuid::from_u128(2)),
            dataset_id: DatasetId(Uuid::from_u128(3)),
            owner_user_id: None,
            title: title.to_string(),
            object_key: "documents/manual-ingest.md".to_string(),
            content_type: "text/markdown".to_string(),
            lifecycle: DocumentLifecycle::Received,
            secret_binding_ids: Vec::new(),
            metadata: BTreeMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn workflow_execution() -> WorkflowExecutionView {
        WorkflowExecutionView {
            id: WorkflowExecutionId(Uuid::from_u128(4)),
            kind: WorkflowKind::UploadIngest,
            stage: "ingest".to_string(),
            status: WorkflowStatus::Running,
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn create_document_ingest_response_keeps_non_zip_child_aliases_empty() {
        let response =
            create_document_ingest_response(document("Manual ingest"), workflow_execution());

        assert_eq!(response.document.title, "Manual ingest");
        assert!(response.child_documents.is_empty());
        assert!(response.child_documents_camel.is_empty());
        assert!(response.child_workflow_executions.is_empty());
        assert!(response.child_workflow_executions_camel.is_empty());
    }
}
