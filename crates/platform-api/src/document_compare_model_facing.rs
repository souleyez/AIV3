use contracts::{
    CompareDocumentsView, DocumentLifecycleView, ModelFacingCapabilityClassView,
    ModelFacingEvidenceStateView, ModelFacingNextActionView, WorkflowModelFacingSummaryView,
};

use crate::document_model_facing_support::document_detail_failed_retrieval_evidence_count;
use crate::model_facing_document_focus::{
    format_model_facing_document_focus, infer_model_facing_document_focus,
};
use crate::model_facing_policy::{build_model_facing_summary, degraded_model_facing_summary};

pub(crate) fn derive_compare_documents_model_facing_summary(
    comparison: &CompareDocumentsView,
) -> WorkflowModelFacingSummaryView {
    let capability_class = ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis;
    let signals = collect_compare_documents_model_facing_signals(comparison);
    let evidence_state = infer_compare_documents_model_facing_evidence_state(comparison);

    if evidence_state == ModelFacingEvidenceStateView::Degraded {
        return degraded_model_facing_summary(capability_class, signals);
    }

    let mut allowed_next_actions = vec![ModelFacingNextActionView::AnswerDirectly];
    if !comparison.documents.is_empty() {
        allowed_next_actions.insert(0, ModelFacingNextActionView::ReadDocumentDetail);
    }

    build_model_facing_summary(
        capability_class,
        evidence_state,
        allowed_next_actions,
        signals,
    )
}

fn infer_compare_documents_model_facing_evidence_state(
    comparison: &CompareDocumentsView,
) -> ModelFacingEvidenceStateView {
    let failed_document_count = comparison
        .documents
        .iter()
        .filter(|detail| detail.document.lifecycle == DocumentLifecycleView::Failed)
        .count();
    let failed_retrieval_evidence_count = comparison
        .documents
        .iter()
        .map(document_detail_failed_retrieval_evidence_count)
        .sum::<usize>();
    let total_chunk_count = comparison
        .documents
        .iter()
        .map(|detail| detail.chunks.len())
        .sum::<usize>();
    let total_retrieval_evidence_count = comparison
        .documents
        .iter()
        .map(|detail| detail.retrieval_evidences.len())
        .sum::<usize>();

    if failed_document_count > 0 || failed_retrieval_evidence_count > 0 {
        return ModelFacingEvidenceStateView::Degraded;
    }
    if comparison.documents.len() >= 2
        && (total_chunk_count > 0 || total_retrieval_evidence_count > 0)
    {
        return ModelFacingEvidenceStateView::Mixed;
    }
    if total_retrieval_evidence_count > 0 {
        return ModelFacingEvidenceStateView::SupplyOnly;
    }
    if total_chunk_count > 0 {
        return ModelFacingEvidenceStateView::CatalogMemory;
    }

    ModelFacingEvidenceStateView::CatalogMemory
}

fn collect_compare_documents_model_facing_signals(
    comparison: &CompareDocumentsView,
) -> Vec<String> {
    let document_focus = infer_model_facing_document_focus(comparison.documents.len(), 0);
    vec![
        "workflow_kind=document_compare".to_string(),
        format!(
            "document_focus={}",
            format_model_facing_document_focus(document_focus)
        ),
        format!("distinct_document_count={}", comparison.documents.len()),
        format!(
            "indexed_document_count={}",
            comparison
                .documents
                .iter()
                .filter(|detail| detail.document.lifecycle == DocumentLifecycleView::Indexed)
                .count()
        ),
        format!(
            "chunk_count={}",
            comparison
                .documents
                .iter()
                .map(|detail| detail.chunks.len())
                .sum::<usize>()
        ),
        format!(
            "retrieval_evidence_count={}",
            comparison
                .documents
                .iter()
                .map(|detail| detail.retrieval_evidences.len())
                .sum::<usize>()
        ),
        format!(
            "failed_document_count={}",
            comparison
                .documents
                .iter()
                .filter(|detail| detail.document.lifecycle == DocumentLifecycleView::Failed)
                .count()
        ),
        format!(
            "failed_retrieval_evidence_count={}",
            comparison
                .documents
                .iter()
                .map(document_detail_failed_retrieval_evidence_count)
                .sum::<usize>()
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use contracts::{
        DocumentChunkStateView, DocumentChunkView, DocumentDetailView, DocumentParseStatusView,
        DocumentSummary, ModelFacingContinuationStateView, RetrievalEvidenceView,
    };
    use domain_model::{
        DatasetId, DocumentChunkId, DocumentId, RetrievalEvidenceId, WorkflowExecutionId,
    };
    use serde_json::json;

    fn document_detail(
        title: &str,
        lifecycle: DocumentLifecycleView,
        chunk_count: usize,
        retrieval_evidence_count: usize,
    ) -> DocumentDetailView {
        let now = Utc::now();
        let document_id = DocumentId::new();
        DocumentDetailView {
            document: DocumentSummary {
                id: document_id,
                dataset_id: DatasetId::new(),
                dataset_ids: Vec::new(),
                dataset_ids_camel: Vec::new(),
                title: title.to_string(),
                object_key: format!("documents/{title}.md"),
                content_type: "text/markdown".to_string(),
                lifecycle: lifecycle.clone(),
                parse_status: "indexed".to_string(),
                parse_status_camel: "indexed".to_string(),
                parse_quality_status: None,
                parse_quality_status_camel: None,
                secret_binding_ids: Vec::new(),
                created_at: now,
                updated_at: now,
            },
            chunks: (0..chunk_count)
                .map(|index| DocumentChunkView {
                    id: DocumentChunkId::new(),
                    document_id,
                    chunk_index: index as i32,
                    token_count: 8,
                    state: DocumentChunkStateView::Indexed,
                    content: format!("{title} detail {index}"),
                    metadata: json!({}),
                    created_at: now,
                    updated_at: now,
                })
                .collect(),
            retrieval_evidences: (0..retrieval_evidence_count)
                .map(|index| RetrievalEvidenceView {
                    id: RetrievalEvidenceId::new(),
                    dataset_id: DatasetId::new(),
                    document_id,
                    document_chunk_id: DocumentChunkId::new(),
                    execution_id: WorkflowExecutionId::new(),
                    chunk_index: index as i32,
                    source_locator: format!("documents/{title}.md#chunk={index}"),
                    content_excerpt: format!("{title} detail {index}"),
                    summary: title.to_string(),
                    payload_filter_key: format!("dataset/{title}"),
                    embedding_model: "local_lexical".to_string(),
                    recall_score: 0.9,
                    evidence_manifest: json!({}),
                    evidence_manifest_view: None,
                    created_at: now,
                })
                .collect(),
            parse_state: DocumentParseStatusView {
                parse_status: "indexed".to_string(),
                parse_status_camel: "indexed".to_string(),
                model_status: "ready".to_string(),
                model_status_camel: "ready".to_string(),
                lifecycle,
                chunk_count,
                retrieval_evidence_count,
                ..Default::default()
            },
            model_facing: None,
        }
    }

    #[test]
    fn compare_documents_summary_marks_multi_document_detail_as_mixed() {
        let comparison = CompareDocumentsView {
            documents: vec![
                document_detail("doc-a", DocumentLifecycleView::Indexed, 1, 1),
                document_detail("doc-b", DocumentLifecycleView::Indexed, 1, 1),
            ],
            model_facing: None,
        };

        let summary = derive_compare_documents_model_facing_summary(&comparison);

        assert_eq!(
            summary.capability_class,
            ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis
        );
        assert_eq!(summary.evidence_state, ModelFacingEvidenceStateView::Mixed);
        assert_eq!(
            summary.continuation_state,
            ModelFacingContinuationStateView::ReadyToAnswer
        );
        assert_eq!(
            summary.recommended_next_action,
            Some(ModelFacingNextActionView::AnswerDirectly)
        );
        assert!(summary
            .allowed_next_actions
            .contains(&ModelFacingNextActionView::ReadDocumentDetail));
        assert!(summary
            .allowed_tool_keys
            .contains(&"document.read_detail".to_string()));
        assert!(summary
            .signals
            .iter()
            .any(|signal| signal == "document_focus=multi_document"));
        assert!(summary
            .signals
            .iter()
            .any(|signal| signal == "distinct_document_count=2"));
    }

    #[test]
    fn compare_documents_summary_marks_failed_document_as_degraded() {
        let comparison = CompareDocumentsView {
            documents: vec![
                document_detail("doc-a", DocumentLifecycleView::Indexed, 1, 1),
                document_detail("doc-b", DocumentLifecycleView::Failed, 0, 0),
            ],
            model_facing: None,
        };

        let summary = derive_compare_documents_model_facing_summary(&comparison);

        assert_eq!(
            summary.evidence_state,
            ModelFacingEvidenceStateView::Degraded
        );
        assert_eq!(
            summary.continuation_state,
            ModelFacingContinuationStateView::RetryRequired
        );
        assert_eq!(
            summary.allowed_next_actions,
            vec![ModelFacingNextActionView::RetryExecution],
        );
        assert!(summary
            .signals
            .iter()
            .any(|signal| signal == "failed_document_count=1"));
    }
}
