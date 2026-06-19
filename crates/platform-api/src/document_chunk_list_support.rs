use contracts::DocumentChunkView;
use domain_model::{DocumentChunk, DocumentId, SecretBindingId, UserId};

use crate::{
    document_view_support::to_document_chunk_view, load_visible_document_for_user_with_local_scope,
    ApiError, AppState,
};

pub(crate) async fn list_document_chunk_views_for_user(
    state: &AppState,
    document_id: DocumentId,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
    local_thread_id: Option<&str>,
) -> std::result::Result<Vec<DocumentChunkView>, ApiError> {
    load_visible_document_for_user_with_local_scope(
        state,
        document_id,
        active_secret_binding_ids,
        current_user_id,
        local_thread_id,
    )
    .await?;

    let chunks = state
        .storage
        .document_chunks()
        .list_by_document(state.tenant_id, document_id)
        .await
        .map_err(ApiError::from_storage)?;

    Ok(document_chunk_views(chunks))
}

fn document_chunk_views(chunks: Vec<DocumentChunk>) -> Vec<DocumentChunkView> {
    chunks.into_iter().map(to_document_chunk_view).collect()
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use domain_model::{DatasetId, DocumentChunkId, DocumentChunkState, TenantId};
    use serde_json::json;
    use std::collections::BTreeMap;

    use super::*;

    fn document_chunk(document_id: DocumentId, index: i32, content: &str) -> DocumentChunk {
        DocumentChunk {
            id: DocumentChunkId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            document_id,
            chunk_index: index,
            content: content.to_string(),
            token_count: 8,
            state: DocumentChunkState::Extracted,
            metadata: BTreeMap::from_iter([("source".to_string(), json!("test"))]),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn document_chunk_views_preserve_order_and_chunk_indexes() {
        let document_id = DocumentId::new();
        let first = document_chunk(document_id, 1, "first");
        let first_id = first.id;
        let second = document_chunk(document_id, 2, "second");
        let second_id = second.id;

        let views = document_chunk_views(vec![first, second]);

        assert_eq!(views.len(), 2);
        assert_eq!(views[0].id, first_id);
        assert_eq!(views[0].chunk_index, 1);
        assert_eq!(views[1].id, second_id);
        assert_eq!(views[1].chunk_index, 2);
    }
}
