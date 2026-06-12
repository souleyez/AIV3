use contracts::{
    DocumentMediaDetailView, ModelFacingCapabilityClassView, ModelFacingEvidenceStateView,
    ModelFacingNextActionView, WorkflowModelFacingSummaryView,
};

use crate::model_facing_policy::{build_model_facing_summary, degraded_model_facing_summary};

pub(crate) fn derive_document_media_detail_model_facing_summary(
    detail: &DocumentMediaDetailView,
) -> WorkflowModelFacingSummaryView {
    let evidence_state = if detail.parse_status == "failed" {
        ModelFacingEvidenceStateView::Degraded
    } else if !detail.transcript_segments.is_empty()
        || !detail.scenes.is_empty()
        || !detail.keyframe_ocr_snippets.is_empty()
    {
        ModelFacingEvidenceStateView::LiveDetail
    } else if detail.parse_status == "partial" || detail.parse_status == "unknown" {
        ModelFacingEvidenceStateView::CatalogMemory
    } else {
        ModelFacingEvidenceStateView::SupplyOnly
    };
    let signals = collect_document_media_detail_model_facing_signals(detail);
    if evidence_state == ModelFacingEvidenceStateView::Degraded {
        return degraded_model_facing_summary(
            ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis,
            signals,
        );
    }
    build_model_facing_summary(
        ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis,
        evidence_state,
        vec![ModelFacingNextActionView::AnswerDirectly],
        signals,
    )
}

fn collect_document_media_detail_model_facing_signals(
    detail: &DocumentMediaDetailView,
) -> Vec<String> {
    vec![
        "workflow_kind=document_media_detail".to_string(),
        "document_focus=single_document".to_string(),
        format!("media_kind={}", detail.media_kind),
        format!("parse_status={}", detail.parse_status),
        format!(
            "transcript_segment_count={}",
            detail.transcript_segments.len()
        ),
        format!("scene_count={}", detail.scenes.len()),
        format!(
            "keyframe_ocr_snippet_count={}",
            detail.keyframe_ocr_snippets.len()
        ),
        format!(
            "supported_provider_capability_count={}",
            detail
                .provider_evidence
                .iter()
                .filter(|evidence| evidence.supported)
                .count()
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use contracts::{
        DocumentLifecycleView, DocumentMediaDetailView, DocumentSummary, MediaOcrSnippetView,
        MediaProviderEvidenceView, MediaSceneView, MediaTranscriptSegmentView,
        ModelFacingContinuationStateView,
    };
    use domain_model::{DatasetId, DocumentId};
    use serde_json::json;

    fn document_media_detail(parse_status: &str) -> DocumentMediaDetailView {
        let now = Utc::now();
        DocumentMediaDetailView {
            document: DocumentSummary {
                id: DocumentId::new(),
                dataset_id: DatasetId::new(),
                dataset_ids: Vec::new(),
                dataset_ids_camel: Vec::new(),
                title: "Synthetic media".to_string(),
                object_key: "synthetic/media.fixture".to_string(),
                content_type: "video/mp4".to_string(),
                lifecycle: DocumentLifecycleView::Indexed,
                parse_status: parse_status.to_string(),
                parse_status_camel: parse_status.to_string(),
                parse_quality_status: None,
                parse_quality_status_camel: None,
                secret_binding_ids: Vec::new(),
                created_at: now,
                updated_at: now,
            },
            media_kind: "video".to_string(),
            parse_status: parse_status.to_string(),
            transcript_segments: Vec::new(),
            scenes: Vec::new(),
            keyframe_ocr_snippets: Vec::new(),
            provider_evidence: Vec::new(),
            raw_media_metadata: json!({ "fixture": true }),
            model_facing: None,
        }
    }

    #[test]
    fn media_detail_summary_maps_failed_and_empty_statuses_to_existing_states() {
        let failed =
            derive_document_media_detail_model_facing_summary(&document_media_detail("failed"));
        assert_eq!(
            failed.evidence_state,
            ModelFacingEvidenceStateView::Degraded
        );
        assert_eq!(
            failed.continuation_state,
            ModelFacingContinuationStateView::RetryRequired,
        );
        assert_eq!(
            failed.allowed_next_actions,
            vec![ModelFacingNextActionView::RetryExecution],
        );

        let partial =
            derive_document_media_detail_model_facing_summary(&document_media_detail("partial"));
        assert_eq!(
            partial.evidence_state,
            ModelFacingEvidenceStateView::CatalogMemory,
        );
        assert_eq!(
            partial.allowed_next_actions,
            vec![ModelFacingNextActionView::AnswerDirectly],
        );

        let completed =
            derive_document_media_detail_model_facing_summary(&document_media_detail("completed"));
        assert_eq!(
            completed.evidence_state,
            ModelFacingEvidenceStateView::SupplyOnly,
        );
    }

    #[test]
    fn media_detail_summary_marks_any_media_evidence_as_live_detail_and_counts_signals() {
        let mut detail = document_media_detail("completed");
        detail.transcript_segments.push(MediaTranscriptSegmentView {
            start_seconds: Some(0.0),
            end_seconds: Some(1.0),
            text: "synthetic transcript".to_string(),
            source: "fixture".to_string(),
            language: Some("en".to_string()),
            confidence: Some(0.9),
        });
        detail.scenes.push(MediaSceneView {
            start_seconds: Some(0.0),
            end_seconds: Some(1.0),
            representative_seconds: Some(0.5),
            summary: "synthetic scene".to_string(),
            source: "fixture".to_string(),
        });
        detail.keyframe_ocr_snippets.push(MediaOcrSnippetView {
            timestamp_seconds: Some(0.5),
            text: "synthetic ocr".to_string(),
            source: "fixture".to_string(),
        });
        detail.provider_evidence.push(MediaProviderEvidenceView {
            provider: "fixture-provider".to_string(),
            capability: "vision".to_string(),
            status: "ready".to_string(),
            supported: true,
            detail: "synthetic".to_string(),
            endpoint: None,
            model: "fixture-model".to_string(),
        });
        detail.provider_evidence.push(MediaProviderEvidenceView {
            provider: "fixture-provider".to_string(),
            capability: "transcript".to_string(),
            status: "unsupported".to_string(),
            supported: false,
            detail: "synthetic".to_string(),
            endpoint: None,
            model: "fixture-model".to_string(),
        });

        let summary = derive_document_media_detail_model_facing_summary(&detail);

        assert_eq!(
            summary.evidence_state,
            ModelFacingEvidenceStateView::LiveDetail,
        );
        assert!(summary
            .signals
            .iter()
            .any(|signal| signal == "transcript_segment_count=1"));
        assert!(summary
            .signals
            .iter()
            .any(|signal| signal == "scene_count=1"));
        assert!(summary
            .signals
            .iter()
            .any(|signal| signal == "keyframe_ocr_snippet_count=1"));
        assert!(summary
            .signals
            .iter()
            .any(|signal| signal == "supported_provider_capability_count=1"));
    }
}
