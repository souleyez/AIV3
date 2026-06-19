use domain_model::{DatasetId, DocumentId, MemoryDirectory, UserId};
use std::collections::HashSet;

use crate::{
    memory_directory_scope::memory_directory_matches_visible_scope,
    visible_document_ids_for_dataset, ApiError, AppState,
};

pub(crate) async fn filter_visible_memory_directories_for_user(
    state: &AppState,
    dataset_id: DatasetId,
    directories: Vec<MemoryDirectory>,
    current_user_id: Option<UserId>,
) -> std::result::Result<Vec<MemoryDirectory>, ApiError> {
    let visible_document_ids =
        visible_document_ids_for_dataset(state, dataset_id, current_user_id).await?;
    Ok(filter_memory_directories_by_visible_scope(
        directories,
        &visible_document_ids,
        current_user_id,
    ))
}

pub(crate) async fn visible_memory_directory_for_user(
    state: &AppState,
    directory: MemoryDirectory,
    current_user_id: Option<UserId>,
) -> std::result::Result<Option<MemoryDirectory>, ApiError> {
    let visible_document_ids =
        visible_document_ids_for_dataset(state, directory.dataset_id, current_user_id).await?;
    Ok(
        memory_directory_matches_visible_scope(&directory, &visible_document_ids, current_user_id)
            .then_some(directory),
    )
}

pub(crate) async fn latest_visible_memory_directory_for_user(
    state: &AppState,
    dataset_id: DatasetId,
    current_user_id: Option<UserId>,
) -> std::result::Result<Option<MemoryDirectory>, ApiError> {
    let directories = state
        .storage
        .memory_directories()
        .list_by_dataset(state.tenant_id, dataset_id)
        .await
        .map_err(ApiError::from_storage)?;
    Ok(
        filter_visible_memory_directories_for_user(state, dataset_id, directories, current_user_id)
            .await?
            .into_iter()
            .next(),
    )
}

fn filter_memory_directories_by_visible_scope(
    directories: Vec<MemoryDirectory>,
    visible_document_ids: &HashSet<DocumentId>,
    current_user_id: Option<UserId>,
) -> Vec<MemoryDirectory> {
    directories
        .into_iter()
        .filter(|directory| {
            memory_directory_matches_visible_scope(directory, visible_document_ids, current_user_id)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{MemoryDirectoryId, TenantId, WorkflowExecutionId};
    use serde_json::json;

    fn directory(
        dataset_id: DatasetId,
        owner_user_id: Option<UserId>,
        source_document_ids: Vec<DocumentId>,
        version_no: i32,
    ) -> MemoryDirectory {
        MemoryDirectory {
            id: MemoryDirectoryId::new(),
            tenant_id: TenantId::new(),
            dataset_id,
            execution_id: WorkflowExecutionId::new(),
            owner_user_id,
            source_document_ids,
            version_no,
            directory_nodes: version_no,
            refreshed_chunks: version_no * 10,
            directory_manifest: json!({}),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn filter_memory_directories_by_visible_scope_preserves_input_order_for_visible_entries() {
        let dataset_id = DatasetId::new();
        let owner = UserId::new();
        let allowed_document = DocumentId::new();
        let hidden_document = DocumentId::new();
        let mut visible_document_ids = HashSet::new();
        visible_document_ids.insert(allowed_document);

        let public_visible = directory(dataset_id, None, vec![allowed_document], 1);
        let hidden_document_directory = directory(dataset_id, None, vec![hidden_document], 2);
        let hidden_owner_directory =
            directory(dataset_id, Some(UserId::new()), vec![allowed_document], 3);
        let owned_visible = directory(dataset_id, Some(owner), vec![allowed_document], 4);

        let filtered = filter_memory_directories_by_visible_scope(
            vec![
                public_visible,
                hidden_document_directory,
                hidden_owner_directory,
                owned_visible,
            ],
            &visible_document_ids,
            Some(owner),
        );

        assert_eq!(
            filtered
                .iter()
                .map(|directory| directory.version_no)
                .collect::<Vec<_>>(),
            vec![1, 4]
        );
    }
}
