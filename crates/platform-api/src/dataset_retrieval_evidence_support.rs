use contracts::RetrievalEvidenceView;
use domain_model::{DatasetId, RetrievalEvidence, SecretBindingId, UserId};

use crate::{
    filter_retrieval_evidences_for_visible_documents, load_visible_dataset_for_user,
    retrieval_evidence_ranking_support::sort_retrieval_evidences_by_relevance,
    retrieval_evidence_view_support::to_retrieval_evidence_view, ApiError, AppState,
};

const DATASET_RETRIEVAL_EVIDENCE_LIST_LIMIT: i64 = 100;

pub(crate) async fn list_dataset_retrieval_evidence_views_for_user(
    state: &AppState,
    dataset_id: DatasetId,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
) -> std::result::Result<Vec<RetrievalEvidenceView>, ApiError> {
    load_visible_dataset_for_user(
        state,
        dataset_id,
        active_secret_binding_ids,
        current_user_id,
    )
    .await?;

    let evidences = state
        .storage
        .retrieval_evidences()
        .list_latest_by_dataset_scope(
            state.tenant_id,
            dataset_id,
            DATASET_RETRIEVAL_EVIDENCE_LIST_LIMIT,
        )
        .await
        .map_err(ApiError::from_storage)?;
    let evidences = filter_retrieval_evidences_for_visible_documents(
        state,
        dataset_id,
        evidences,
        current_user_id,
    )
    .await?;

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

    fn retrieval_evidence(summary: &str, recall_score: f64) -> RetrievalEvidence {
        RetrievalEvidence {
            id: RetrievalEvidenceId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            execution_id: WorkflowExecutionId::new(),
            document_id: DocumentId::new(),
            document_chunk_id: DocumentChunkId::new(),
            chunk_index: 0,
            source_locator: "documents/demo.md#chunk=0".to_string(),
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
        let low = retrieval_evidence("low", 0.2);
        let high = retrieval_evidence("high", 0.9);
        let high_id = high.id;
        let low_id = low.id;

        let views = sorted_retrieval_evidence_views(vec![low, high]);

        assert_eq!(views.len(), 2);
        assert_eq!(views[0].id, high_id);
        assert_eq!(views[1].id, low_id);
    }
}
