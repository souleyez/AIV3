use contracts::MemoryDirectoryView;
use domain_model::{DatasetId, MemoryDirectory, SecretBindingId, UserId};

use crate::{
    filter_visible_memory_directories_for_user, load_visible_dataset_for_user,
    memory_directory_view_support::to_memory_directory_view, ApiError, AppState,
};

pub(crate) async fn list_memory_directory_views_for_user(
    state: &AppState,
    dataset_id: DatasetId,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
) -> std::result::Result<Vec<MemoryDirectoryView>, ApiError> {
    load_visible_dataset_for_user(
        state,
        dataset_id,
        active_secret_binding_ids,
        current_user_id,
    )
    .await?;

    let directories = state
        .storage
        .memory_directories()
        .list_by_dataset(state.tenant_id, dataset_id)
        .await
        .map_err(ApiError::from_storage)?;
    let directories =
        filter_visible_memory_directories_for_user(state, dataset_id, directories, current_user_id)
            .await?;

    Ok(memory_directory_views(directories))
}

fn memory_directory_views(directories: Vec<MemoryDirectory>) -> Vec<MemoryDirectoryView> {
    directories
        .into_iter()
        .map(to_memory_directory_view)
        .collect()
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use domain_model::{DocumentId, MemoryDirectoryId, TenantId, WorkflowExecutionId};
    use serde_json::json;

    use super::*;

    fn memory_directory(dataset_id: DatasetId, version_no: i32) -> MemoryDirectory {
        MemoryDirectory {
            id: MemoryDirectoryId::new(),
            tenant_id: TenantId::new(),
            dataset_id,
            execution_id: WorkflowExecutionId::new(),
            owner_user_id: None,
            source_document_ids: vec![DocumentId::new()],
            version_no,
            directory_nodes: version_no,
            refreshed_chunks: version_no * 10,
            directory_manifest: json!({
                "schema_version": "0.1.0",
                "generator": "memory-worker",
                "dataset_id": dataset_id,
                "version_no": version_no,
                "root": {
                    "kind": "dataset",
                    "title": "Dataset Memory",
                    "scope": "dataset",
                    "version_no": version_no,
                    "children": []
                }
            }),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn memory_directory_views_preserve_order_and_versions() {
        let dataset_id = DatasetId::new();
        let first = memory_directory(dataset_id, 1);
        let first_id = first.id;
        let second = memory_directory(dataset_id, 2);
        let second_id = second.id;

        let views = memory_directory_views(vec![first, second]);

        assert_eq!(views.len(), 2);
        assert_eq!(views[0].id, first_id);
        assert_eq!(views[0].version_no, 1);
        assert_eq!(views[1].id, second_id);
        assert_eq!(views[1].version_no, 2);
    }
}
