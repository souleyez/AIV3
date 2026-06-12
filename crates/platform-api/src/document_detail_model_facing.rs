use contracts::{DocumentDetailView, WorkflowModelFacingSummaryView};
use serde_json::Value;

use crate::document_model_facing_support::{
    document_detail_failed_retrieval_evidence_count, format_document_lifecycle_view,
};
use crate::model_facing_policy::{build_model_facing_summary, degraded_model_facing_summary};
use crate::{
    document_chunk_value_noun_terms, document_chunk_value_section_title_hints, push_string_hint,
};

pub(crate) fn derive_document_detail_model_facing_summary(
    detail: &DocumentDetailView,
) -> WorkflowModelFacingSummaryView {
    let capability_class =
        contracts::ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis;
    let signals = collect_document_detail_model_facing_signals(detail);
    let evidence_state = infer_document_detail_model_facing_evidence_state(detail);

    if evidence_state == contracts::ModelFacingEvidenceStateView::Degraded {
        return degraded_model_facing_summary(capability_class, signals);
    }

    build_model_facing_summary(
        capability_class,
        evidence_state,
        vec![contracts::ModelFacingNextActionView::AnswerDirectly],
        signals,
    )
}

fn infer_document_detail_model_facing_evidence_state(
    detail: &DocumentDetailView,
) -> contracts::ModelFacingEvidenceStateView {
    if detail.document.lifecycle == contracts::DocumentLifecycleView::Failed
        || document_detail_failed_retrieval_evidence_count(detail) > 0
    {
        return contracts::ModelFacingEvidenceStateView::Degraded;
    }

    if !detail.retrieval_evidences.is_empty()
        || detail.document.lifecycle == contracts::DocumentLifecycleView::Indexed
    {
        return contracts::ModelFacingEvidenceStateView::LiveDetail;
    }
    if !detail.chunks.is_empty() {
        return contracts::ModelFacingEvidenceStateView::SupplyOnly;
    }

    contracts::ModelFacingEvidenceStateView::CatalogMemory
}

fn collect_document_detail_model_facing_signals(detail: &DocumentDetailView) -> Vec<String> {
    let section_title_hints = collect_document_detail_section_title_hints(detail);
    let noun_terms = collect_document_detail_noun_terms(detail);
    let mut signals = vec![
        "workflow_kind=document_detail".to_string(),
        "document_focus=single_document".to_string(),
        format!(
            "document_lifecycle={}",
            format_document_lifecycle_view(detail.document.lifecycle.clone())
        ),
        format!("chunk_count={}", detail.chunks.len()),
        format!(
            "indexed_chunk_count={}",
            detail
                .chunks
                .iter()
                .filter(|chunk| chunk.state == contracts::DocumentChunkStateView::Indexed)
                .count()
        ),
        format!(
            "retrieval_evidence_count={}",
            detail.retrieval_evidences.len()
        ),
        format!(
            "failed_retrieval_evidence_count={}",
            document_detail_failed_retrieval_evidence_count(detail)
        ),
        format!("section_title_hint_count={}", section_title_hints.len()),
        format!("noun_term_hint_count={}", noun_terms.len()),
    ];
    if !section_title_hints.is_empty() {
        signals.push("rag_signal=section_title_hints".to_string());
        signals.push(format!(
            "section_title_hints={}",
            section_title_hints
                .iter()
                .take(6)
                .cloned()
                .collect::<Vec<_>>()
                .join("|")
        ));
    }
    if !noun_terms.is_empty() {
        signals.push("rag_signal=noun_term_hints".to_string());
        signals.push(format!(
            "noun_terms={}",
            noun_terms
                .iter()
                .take(10)
                .cloned()
                .collect::<Vec<_>>()
                .join("|")
        ));
    }
    if let Some(parse_quality_status) = detail.parse_state.parse_quality_status.as_deref() {
        signals.push(format!("parse_quality_status={parse_quality_status}"));
    }
    if let Some(parse_quality_summary) = detail.parse_state.parse_quality_summary.as_ref() {
        push_parse_quality_signal(
            &mut signals,
            parse_quality_summary,
            "parse_method",
            &["parse_method"],
        );
        push_parse_quality_signal(
            &mut signals,
            parse_quality_summary,
            "parse_candidate_selected_method",
            &["candidate_selection", "selected_method"],
        );
        push_parse_quality_signal(
            &mut signals,
            parse_quality_summary,
            "parse_vlm_rescue_selected",
            &["vlm_rescue", "selected"],
        );
        push_parse_quality_signal(
            &mut signals,
            parse_quality_summary,
            "auto_reparse_status",
            &["auto_reparse", "status"],
        );
    }
    signals
}

fn push_parse_quality_signal(
    signals: &mut Vec<String>,
    parse_quality_summary: &Value,
    signal_key: &str,
    path: &[&str],
) {
    let mut value = parse_quality_summary;
    for key in path {
        let Some(next) = value.get(*key) else {
            return;
        };
        value = next;
    }
    if let Some(text) = value.as_str() {
        signals.push(format!("{signal_key}={text}"));
    }
}

fn collect_document_detail_section_title_hints(detail: &DocumentDetailView) -> Vec<String> {
    let mut hints = Vec::new();
    for chunk in &detail.chunks {
        for hint in document_chunk_value_section_title_hints(&chunk.metadata, &chunk.content, 6) {
            push_string_hint(&mut hints, hint);
        }
        if hints.len() >= 24 {
            break;
        }
    }
    hints.truncate(24);
    hints
}

fn collect_document_detail_noun_terms(detail: &DocumentDetailView) -> Vec<String> {
    let mut terms = Vec::new();
    for chunk in &detail.chunks {
        for term in document_chunk_value_noun_terms(&chunk.metadata) {
            push_string_hint(&mut terms, term);
        }
        if terms.len() >= 40 {
            break;
        }
    }
    terms.truncate(40);
    terms
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use contracts::{DocumentChunkView, DocumentParseStatusView, DocumentSummary};
    use domain_model::{DatasetId, DocumentChunkId, DocumentId};
    use serde_json::json;

    fn document_summary(lifecycle: contracts::DocumentLifecycleView) -> DocumentSummary {
        let now = Utc::now();
        DocumentSummary {
            id: DocumentId::new(),
            dataset_id: DatasetId::new(),
            dataset_ids: Vec::new(),
            dataset_ids_camel: Vec::new(),
            title: "Document detail fixture".to_string(),
            object_key: "documents/detail-fixture.md".to_string(),
            content_type: "text/markdown".to_string(),
            lifecycle,
            parse_status: "indexed".to_string(),
            parse_status_camel: "indexed".to_string(),
            parse_quality_status: None,
            parse_quality_status_camel: None,
            secret_binding_ids: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn document_detail_summary_marks_failed_lifecycle_as_degraded() {
        let detail = DocumentDetailView {
            document: document_summary(contracts::DocumentLifecycleView::Failed),
            chunks: Vec::new(),
            retrieval_evidences: Vec::new(),
            parse_state: DocumentParseStatusView {
                lifecycle: contracts::DocumentLifecycleView::Failed,
                ..Default::default()
            },
            model_facing: None,
        };

        let summary = derive_document_detail_model_facing_summary(&detail);

        assert_eq!(
            summary.capability_class,
            contracts::ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis
        );
        assert_eq!(
            summary.evidence_state,
            contracts::ModelFacingEvidenceStateView::Degraded
        );
        assert_eq!(
            summary.continuation_state,
            contracts::ModelFacingContinuationStateView::RetryRequired
        );
        assert_eq!(
            summary.recommended_next_action,
            Some(contracts::ModelFacingNextActionView::RetryExecution)
        );
        assert!(summary
            .signals
            .iter()
            .any(|signal| signal == "document_lifecycle=failed"));
    }

    #[test]
    fn document_detail_summary_keeps_hint_and_parse_quality_signals() {
        let now = Utc::now();
        let detail = DocumentDetailView {
            document: document_summary(contracts::DocumentLifecycleView::Indexed),
            chunks: vec![DocumentChunkView {
                id: DocumentChunkId::new(),
                document_id: DocumentId::new(),
                chunk_index: 0,
                token_count: 12,
                state: contracts::DocumentChunkStateView::Indexed,
                content: "## Care workflow\nMedication check steps".to_string(),
                metadata: json!({
                    "section_title_hints": ["Care workflow"],
                    "noun_terms": ["medication", "check"],
                }),
                created_at: now,
                updated_at: now,
            }],
            retrieval_evidences: Vec::new(),
            parse_state: DocumentParseStatusView {
                parse_quality_status: Some("ok".to_string()),
                parse_quality_status_camel: Some("ok".to_string()),
                parse_quality_summary: Some(json!({
                    "parse_method": "text",
                    "candidate_selection": { "selected_method": "native" },
                    "vlm_rescue": { "selected": "false" },
                    "auto_reparse": { "status": "not_required" }
                })),
                lifecycle: contracts::DocumentLifecycleView::Indexed,
                chunk_count: 1,
                retrieval_evidence_count: 0,
                ..Default::default()
            },
            model_facing: None,
        };

        let summary = derive_document_detail_model_facing_summary(&detail);

        assert_eq!(
            summary.evidence_state,
            contracts::ModelFacingEvidenceStateView::LiveDetail
        );
        for expected in [
            "rag_signal=section_title_hints",
            "section_title_hints=Care workflow",
            "rag_signal=noun_term_hints",
            "noun_terms=medication|check",
            "parse_quality_status=ok",
            "parse_method=text",
            "parse_candidate_selected_method=native",
            "parse_vlm_rescue_selected=false",
            "auto_reparse_status=not_required",
        ] {
            assert!(summary.signals.iter().any(|signal| signal == expected));
        }
    }
}
