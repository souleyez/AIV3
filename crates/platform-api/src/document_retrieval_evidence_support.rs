use contracts::RetrievalEvidenceView;
use domain_model::{DocumentId, RetrievalEvidence, SecretBindingId, UserId};

use crate::{
    load_visible_document_for_user_with_local_scope,
    retrieval_evidence_ranking_support::sort_retrieval_evidences_by_relevance,
    retrieval_evidence_view_support::to_retrieval_evidence_view, ApiError, AppState,
};

pub(crate) async fn list_document_retrieval_evidence_views_for_user(
    state: &AppState,
    document_id: DocumentId,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
    local_thread_id: Option<&str>,
) -> std::result::Result<Vec<RetrievalEvidenceView>, ApiError> {
    load_visible_document_for_user_with_local_scope(
        state,
        document_id,
        active_secret_binding_ids,
        current_user_id,
        local_thread_id,
    )
    .await?;

    let evidences = state
        .storage
        .retrieval_evidences()
        .list_by_document_or_canonical(state.tenant_id, document_id)
        .await
        .map_err(ApiError::from_storage)?;

    Ok(sorted_retrieval_evidence_views(evidences))
}

fn sorted_retrieval_evidence_views(
    mut evidences: Vec<RetrievalEvidence>,
) -> Vec<RetrievalEvidenceView> {
    sort_retrieval_evidences_by_relevance(&mut evidences);
    evidences
        .into_iter()
        .map(to_retrieval_evidence_view)
        .collect()
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use domain_model::{
        DatasetId, DocumentChunkId, DocumentId, RetrievalEvidenceId, TenantId, WorkflowExecutionId,
    };
    use serde_json::json;

    use super::*;

    fn retrieval_evidence(
        document_id: DocumentId,
        summary: &str,
        recall_score: f64,
    ) -> RetrievalEvidence {
        RetrievalEvidence {
            id: RetrievalEvidenceId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            execution_id: WorkflowExecutionId::new(),
            document_id,
            document_chunk_id: DocumentChunkId::new(),
            chunk_index: 0,
            source_locator: format!("documents/demo.md#{summary}"),
            content_excerpt: summary.to_string(),
            summary: summary.to_string(),
            payload_filter_key: "dataset/demo".to_string(),
            embedding_model: "local-lexical-v1".to_string(),
            recall_score,
            evidence_manifest: json!({}),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn sorted_retrieval_evidence_views_keep_existing_relevance_ordering() {
        let document_id = DocumentId::new();
        let low = retrieval_evidence(document_id, "low", 0.2);
        let low_id = low.id;
        let high = retrieval_evidence(document_id, "high", 0.9);
        let high_id = high.id;

        let views = sorted_retrieval_evidence_views(vec![low, high]);

        assert_eq!(views.len(), 2);
        assert_eq!(views[0].id, high_id);
        assert_eq!(views[1].id, low_id);
    }
}
