use contracts::{
    DocumentLifecycleView, RetrievalEmbeddingStatusView, RetrievalEvidenceView,
    RetrievalRecallStatusView,
};

pub(crate) fn retrieval_evidence_has_failed_state(evidence: &RetrievalEvidenceView) -> bool {
    evidence
        .evidence_manifest_view
        .as_ref()
        .map(|manifest| {
            manifest.embedding.status == RetrievalEmbeddingStatusView::Failed
                || manifest.recall.status == RetrievalRecallStatusView::Failed
        })
        .unwrap_or(false)
}

pub(crate) fn format_document_lifecycle_view(value: DocumentLifecycleView) -> &'static str {
    match value {
        DocumentLifecycleView::Received => "received",
        DocumentLifecycleView::Extracted => "extracted",
        DocumentLifecycleView::Indexed => "indexed",
        DocumentLifecycleView::Failed => "failed",
        DocumentLifecycleView::Archived => "archived",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use contracts::{
        RetrievalEmbeddingManifestView, RetrievalEvidenceLocatorManifestView,
        RetrievalEvidenceManifestView, RetrievalRecallManifestView,
    };
    use domain_model::{
        DatasetId, DocumentChunkId, DocumentId, RetrievalEvidenceId, WorkflowExecutionId,
    };
    use serde_json::json;

    fn retrieval_evidence(
        embedding_status: RetrievalEmbeddingStatusView,
        recall_status: RetrievalRecallStatusView,
    ) -> RetrievalEvidenceView {
        let dataset_id = DatasetId::new();
        let document_id = DocumentId::new();
        let document_chunk_id = DocumentChunkId::new();
        RetrievalEvidenceView {
            id: RetrievalEvidenceId::new(),
            dataset_id,
            document_id,
            document_chunk_id,
            execution_id: WorkflowExecutionId::new(),
            chunk_index: 0,
            source_locator: "fixture://document#0".to_string(),
            content_excerpt: "synthetic excerpt".to_string(),
            summary: "synthetic summary".to_string(),
            payload_filter_key: "fixture".to_string(),
            embedding_model: "fixture-model".to_string(),
            recall_score: 1.0,
            evidence_manifest: json!({ "fixture": true }),
            evidence_manifest_view: Some(RetrievalEvidenceManifestView {
                schema_version: "v1".to_string(),
                generator: "test".to_string(),
                dataset_id,
                document_id,
                document_chunk_id,
                chunk_index: 0,
                indexed_at: Utc::now(),
                embedding: RetrievalEmbeddingManifestView {
                    status: embedding_status,
                    model: "fixture-model".to_string(),
                    token_count: 1,
                },
                recall: RetrievalRecallManifestView {
                    status: recall_status,
                    score: 1.0,
                    rank_hint: 1,
                },
                evidence: RetrievalEvidenceLocatorManifestView {
                    document_chunk_id,
                    payload_filter_key: "fixture".to_string(),
                    source_locator: "fixture://document#0".to_string(),
                },
            }),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn retrieval_evidence_failed_state_uses_embedding_or_recall_status() {
        assert!(!retrieval_evidence_has_failed_state(&retrieval_evidence(
            RetrievalEmbeddingStatusView::Indexed,
            RetrievalRecallStatusView::Ready,
        )));
        assert!(retrieval_evidence_has_failed_state(&retrieval_evidence(
            RetrievalEmbeddingStatusView::Failed,
            RetrievalRecallStatusView::Ready,
        )));
        assert!(retrieval_evidence_has_failed_state(&retrieval_evidence(
            RetrievalEmbeddingStatusView::Indexed,
            RetrievalRecallStatusView::Failed,
        )));

        let mut missing_manifest = retrieval_evidence(
            RetrievalEmbeddingStatusView::Indexed,
            RetrievalRecallStatusView::Ready,
        );
        missing_manifest.evidence_manifest_view = None;
        assert!(!retrieval_evidence_has_failed_state(&missing_manifest));
    }

    #[test]
    fn document_lifecycle_formatter_keeps_protocol_strings() {
        assert_eq!(
            format_document_lifecycle_view(DocumentLifecycleView::Received),
            "received",
        );
        assert_eq!(
            format_document_lifecycle_view(DocumentLifecycleView::Extracted),
            "extracted",
        );
        assert_eq!(
            format_document_lifecycle_view(DocumentLifecycleView::Indexed),
            "indexed",
        );
        assert_eq!(
            format_document_lifecycle_view(DocumentLifecycleView::Failed),
            "failed",
        );
        assert_eq!(
            format_document_lifecycle_view(DocumentLifecycleView::Archived),
            "archived",
        );
    }
}
