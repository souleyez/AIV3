use contracts::DocumentDetailView;
use domain_model::{DocumentId, SecretBindingId, UserId};

use crate::{
    load_latest_upload_ingest_workflow_snapshots, load_visible_document_for_user_with_local_scope,
    to_document_detail_view, ApiError, AppState,
};

pub(crate) async fn load_document_detail_view_for_user(
    state: &AppState,
    document_id: DocumentId,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
) -> std::result::Result<DocumentDetailView, ApiError> {
    load_document_detail_view_for_user_with_local_scope(
        state,
        document_id,
        active_secret_binding_ids,
        current_user_id,
        None,
    )
    .await
}

pub(crate) async fn load_document_detail_view_for_user_with_local_scope(
    state: &AppState,
    document_id: DocumentId,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
    local_thread_id: Option<&str>,
) -> std::result::Result<DocumentDetailView, ApiError> {
    let document = load_visible_document_for_user_with_local_scope(
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
        .list_by_document_or_canonical(state.tenant_id, document_id)
        .await
        .map_err(ApiError::from_storage)?;
    let mut retrieval_evidences = state
        .storage
        .retrieval_evidences()
        .list_by_document_or_canonical(state.tenant_id, document_id)
        .await
        .map_err(ApiError::from_storage)?;
    crate::retrieval_evidence_ranking_support::sort_retrieval_evidences_by_relevance(
        &mut retrieval_evidences,
    );
    let workflow_by_document =
        load_latest_upload_ingest_workflow_snapshots(state, document.dataset_id, &[document.id])
            .await?;
    let workflow = workflow_by_document.get(&document.id).cloned();

    Ok(to_document_detail_view(
        document,
        chunks,
        retrieval_evidences,
        workflow,
    ))
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use domain_model::{
        DatasetId, Document, DocumentChunk, DocumentChunkId, DocumentChunkState, DocumentId,
        DocumentLifecycle, RetrievalEvidence, RetrievalEvidenceId, TenantId, WorkflowExecutionId,
    };
    use serde_json::json;
    use std::collections::BTreeMap;

    use super::*;

    fn document(document_id: DocumentId, dataset_id: DatasetId) -> Document {
        let now = Utc::now();
        Document {
            id: document_id,
            tenant_id: TenantId::new(),
            dataset_id,
            owner_user_id: None,
            title: "Detail fixture".to_string(),
            object_key: "/tmp/detail-fixture.md".to_string(),
            content_type: "text/markdown".to_string(),
            lifecycle: DocumentLifecycle::Indexed,
            secret_binding_ids: Vec::new(),
            metadata: BTreeMap::new(),
            created_at: now,
            updated_at: now,
        }
    }

    fn chunk(document_id: DocumentId, dataset_id: DatasetId) -> DocumentChunk {
        let now = Utc::now();
        DocumentChunk {
            id: DocumentChunkId::new(),
            tenant_id: TenantId::new(),
            dataset_id,
            document_id,
            chunk_index: 3,
            content: "## 摘要\nalpha beta".to_string(),
            token_count: 2,
            state: DocumentChunkState::Extracted,
            metadata: BTreeMap::new(),
            created_at: now,
            updated_at: now,
        }
    }

    fn retrieval_evidence(
        document_id: DocumentId,
        dataset_id: DatasetId,
        summary: &str,
        recall_score: f64,
    ) -> RetrievalEvidence {
        RetrievalEvidence {
            id: RetrievalEvidenceId::new(),
            tenant_id: TenantId::new(),
            dataset_id,
            execution_id: WorkflowExecutionId::new(),
            document_id,
            document_chunk_id: DocumentChunkId::new(),
            chunk_index: 0,
            source_locator: format!("documents/detail.md#{summary}"),
            content_excerpt: summary.to_string(),
            summary: summary.to_string(),
            payload_filter_key: "dataset/detail".to_string(),
            embedding_model: "local-lexical-v1".to_string(),
            recall_score,
            evidence_manifest: json!({}),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn document_detail_view_assembly_keeps_chunks_and_sorted_evidence() {
        let document_id = DocumentId::new();
        let dataset_id = DatasetId::new();
        let detail_document = document(document_id, dataset_id);
        let detail_chunk = chunk(document_id, dataset_id);
        let high = retrieval_evidence(document_id, dataset_id, "high", 0.9);
        let high_id = high.id;
        let low = retrieval_evidence(document_id, dataset_id, "low", 0.2);
        let low_id = low.id;

        let mut evidences = vec![low, high];
        crate::retrieval_evidence_ranking_support::sort_retrieval_evidences_by_relevance(
            &mut evidences,
        );
        let view = to_document_detail_view(detail_document, vec![detail_chunk], evidences, None);

        assert_eq!(view.document.id, document_id);
        assert_eq!(view.chunks.len(), 1);
        assert_eq!(view.chunks[0].chunk_index, 3);
        assert_eq!(view.retrieval_evidences.len(), 2);
        assert_eq!(view.retrieval_evidences[0].id, high_id);
        assert_eq!(view.retrieval_evidences[1].id, low_id);
        assert!(view.model_facing.is_some());
    }
}
