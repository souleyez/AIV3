use contracts::RetrievalEvidenceView;
use domain_model::{
    DatasetOutputId, RetrievalEvidence, RetrievalEvidenceId, SecretBindingId, UserId,
};

use crate::{
    filter_retrieval_evidences_for_visible_documents, load_visible_dataset_output_for_user,
    retrieval_evidence_view_support::to_retrieval_evidence_view, ApiError, AppState,
};

pub(crate) async fn list_dataset_output_retrieval_evidence_views_for_user(
    state: &AppState,
    output_id: DatasetOutputId,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
) -> std::result::Result<Vec<RetrievalEvidenceView>, ApiError> {
    let output = load_visible_dataset_output_for_user(
        state,
        output_id,
        active_secret_binding_ids,
        current_user_id,
    )
    .await?;
    let evidences =
        load_dataset_output_retrieval_evidences(state, &output.retrieval_evidence_ids).await?;
    let evidences = filter_retrieval_evidences_for_visible_documents(
        state,
        output.dataset_id,
        evidences,
        current_user_id,
    )
    .await?;

    Ok(retrieval_evidence_views(evidences))
}

async fn load_dataset_output_retrieval_evidences(
    state: &AppState,
    evidence_ids: &[RetrievalEvidenceId],
) -> std::result::Result<Vec<RetrievalEvidence>, ApiError> {
    state
        .storage
        .retrieval_evidences()
        .list_by_ids(state.tenant_id, evidence_ids)
        .await
        .map_err(ApiError::from_storage)
}

fn retrieval_evidence_views(evidences: Vec<RetrievalEvidence>) -> Vec<RetrievalEvidenceView> {
    evidences
        .into_iter()
        .map(to_retrieval_evidence_view)
        .collect()
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use domain_model::{DatasetId, DocumentChunkId, DocumentId, TenantId, WorkflowExecutionId};
    use serde_json::json;

    use super::*;

    fn retrieval_evidence(summary: &str) -> RetrievalEvidence {
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
            recall_score: 0.5,
            evidence_manifest: json!({}),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn retrieval_evidence_views_preserve_order_and_ids() {
        let first = retrieval_evidence("first");
        let first_id = first.id;
        let second = retrieval_evidence("second");
        let second_id = second.id;

        let views = retrieval_evidence_views(vec![first, second]);

        assert_eq!(views.len(), 2);
        assert_eq!(views[0].id, first_id);
        assert_eq!(views[1].id, second_id);
    }
}
