use contracts::{
    DatasetOutputView, ManifestServiceHandoffView, ManifestToolCallStatusView,
    ModelFacingCapabilityClassView, ModelFacingEvidenceStateView, ModelFacingNextActionView,
    ModelFacingReportEntryStateView, WorkflowModelFacingSummaryView,
};
use domain_model::WorkflowKind;

use crate::model_facing_document_focus::{
    format_model_facing_document_focus, infer_model_facing_document_focus, ModelFacingDocumentFocus,
};
use crate::model_facing_handoff::{
    collect_service_handoff_signals, infer_service_handoff_capability_class,
    infer_service_handoff_next_actions,
};
use crate::model_facing_policy::build_model_facing_summary;

pub(crate) fn derive_dataset_output_model_facing_summary(
    output: &DatasetOutputView,
) -> WorkflowModelFacingSummaryView {
    let base_capability_class = infer_dataset_output_model_facing_capability_class(output);
    let evidence_state = infer_dataset_output_model_facing_evidence_state(output);
    let mut signals = collect_dataset_output_model_facing_signals(output);

    if let Some(handoff) = dataset_output_service_handoff(output) {
        signals.extend(collect_service_handoff_signals(handoff));
        match handoff.report_entry_state {
            ModelFacingReportEntryStateView::ConfirmationRequired => {
                let mut summary = build_model_facing_summary(
                    base_capability_class,
                    evidence_state,
                    vec![ModelFacingNextActionView::RequestReportEntryConfirmation],
                    signals,
                );
                summary.service_lane = handoff.service_lane.clone();
                summary.report_entry_state = handoff.report_entry_state.clone();
                return summary;
            }
            ModelFacingReportEntryStateView::Confirmed => {
                let capability_class =
                    infer_service_handoff_capability_class(handoff, &base_capability_class);
                let allowed_next_actions = infer_service_handoff_next_actions(
                    handoff,
                    &base_capability_class,
                    &evidence_state,
                );
                let mut summary = build_model_facing_summary(
                    capability_class,
                    evidence_state,
                    allowed_next_actions,
                    signals,
                );
                summary.service_lane = handoff.service_lane.clone();
                summary.report_entry_state = handoff.report_entry_state.clone();
                return summary;
            }
            ModelFacingReportEntryStateView::NotApplicable => {}
        }
    }

    let allowed_next_actions = infer_dataset_output_model_facing_next_actions(
        output,
        &base_capability_class,
        &evidence_state,
    );
    build_model_facing_summary(
        base_capability_class,
        evidence_state,
        allowed_next_actions,
        signals,
    )
}

fn infer_dataset_output_model_facing_capability_class(
    output: &DatasetOutputView,
) -> ModelFacingCapabilityClassView {
    let has_answer_content = dataset_output_has_answer_content(output);
    let retrieval_evidence_count = output.retrieval_evidence_ids.len();

    if !has_answer_content && retrieval_evidence_count == 0 && output.memory_directory_id.is_some()
    {
        return ModelFacingCapabilityClassView::DatasetDirectoryAwareness;
    }
    if !has_answer_content && retrieval_evidence_count > 0 {
        return ModelFacingCapabilityClassView::EvidenceRetrieval;
    }

    ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis
}

fn infer_dataset_output_model_facing_evidence_state(
    output: &DatasetOutputView,
) -> ModelFacingEvidenceStateView {
    let has_memory_directory = output.memory_directory_id.is_some();
    let retrieval_evidence_count = output.retrieval_evidence_ids.len();
    let has_answer_content = dataset_output_has_answer_content(output);
    let document_focus = dataset_output_document_focus(output);

    if output
        .tool_executions
        .iter()
        .any(|execution| execution.status == ManifestToolCallStatusView::Failed)
    {
        return ModelFacingEvidenceStateView::Degraded;
    }
    if retrieval_evidence_count > 0 {
        if document_focus == ModelFacingDocumentFocus::SingleDocument && has_answer_content {
            return ModelFacingEvidenceStateView::LiveDetail;
        }
        if document_focus == ModelFacingDocumentFocus::MultiDocument
            || (has_memory_directory && has_answer_content)
        {
            return ModelFacingEvidenceStateView::Mixed;
        }
        return ModelFacingEvidenceStateView::SupplyOnly;
    }
    if has_memory_directory {
        return ModelFacingEvidenceStateView::CatalogMemory;
    }

    ModelFacingEvidenceStateView::CatalogMemory
}

fn infer_dataset_output_model_facing_next_actions(
    output: &DatasetOutputView,
    capability_class: &ModelFacingCapabilityClassView,
    evidence_state: &ModelFacingEvidenceStateView,
) -> Vec<ModelFacingNextActionView> {
    if *evidence_state == ModelFacingEvidenceStateView::Degraded {
        return vec![ModelFacingNextActionView::RetryExecution];
    }

    let document_focus = dataset_output_document_focus(output);
    let mut actions = Vec::new();
    match capability_class {
        ModelFacingCapabilityClassView::DatasetDirectoryAwareness => {
            actions.push(ModelFacingNextActionView::RefreshDirectory);
            actions.push(ModelFacingNextActionView::AnswerDirectly);
        }
        ModelFacingCapabilityClassView::EvidenceRetrieval => {
            if document_focus == ModelFacingDocumentFocus::MultiDocument {
                actions.push(ModelFacingNextActionView::CompareDocuments);
            }
            actions.push(ModelFacingNextActionView::ReadDocumentDetail);
        }
        ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis => {
            if document_focus == ModelFacingDocumentFocus::MultiDocument {
                actions.push(ModelFacingNextActionView::CompareDocuments);
            }
            if !output.retrieval_evidence_ids.is_empty() {
                actions.push(ModelFacingNextActionView::ReadDocumentDetail);
            }
            actions.push(ModelFacingNextActionView::AnswerDirectly);
        }
        ModelFacingCapabilityClassView::ReportPlanning => {
            actions.push(ModelFacingNextActionView::ContinueReportPlanning);
        }
        ModelFacingCapabilityClassView::ReportGenerationAndEditing => {
            actions.push(ModelFacingNextActionView::GenerateReportOutput);
        }
        ModelFacingCapabilityClassView::ControlledPlatformAction => {
            actions.push(ModelFacingNextActionView::RetryExecution);
        }
    }
    actions
}

fn collect_dataset_output_model_facing_signals(output: &DatasetOutputView) -> Vec<String> {
    let document_focus = dataset_output_document_focus(output);
    let distinct_document_count = dataset_output_distinct_document_count(output);
    let indexed_document_count = output
        .output_manifest_view
        .as_ref()
        .map(|manifest| manifest.indexed_document_count)
        .unwrap_or(0);
    vec![
        format!("workflow_kind={}", WorkflowKind::DatasetOutput.as_str()),
        format!(
            "retrieval_evidence_count={}",
            output.retrieval_evidence_ids.len()
        ),
        format!(
            "answer_content_present={}",
            dataset_output_has_answer_content(output)
        ),
        format!(
            "document_focus={}",
            format_model_facing_document_focus(document_focus)
        ),
        format!("distinct_document_count={distinct_document_count}"),
        format!("indexed_document_count={indexed_document_count}"),
        format!(
            "has_memory_directory={}",
            output.memory_directory_id.is_some()
        ),
        format!("tool_execution_count={}", output.tool_executions.len()),
    ]
}

fn dataset_output_service_handoff(
    output: &DatasetOutputView,
) -> Option<&ManifestServiceHandoffView> {
    output
        .output_manifest_view
        .as_ref()
        .and_then(|manifest| manifest.service_handoff.as_ref())
}

fn dataset_output_has_answer_content(output: &DatasetOutputView) -> bool {
    !output.output_text.trim().is_empty()
        || output
            .output_manifest_view
            .as_ref()
            .and_then(|manifest| manifest.output.as_ref())
            .map(|content| {
                content
                    .sections
                    .iter()
                    .any(|section| !section.content.trim().is_empty())
            })
            .unwrap_or(false)
}

pub(crate) fn dataset_output_distinct_document_count(output: &DatasetOutputView) -> usize {
    output
        .retrieval_evidences
        .iter()
        .map(|evidence| evidence.document_id)
        .collect::<std::collections::BTreeSet<_>>()
        .len()
}

fn dataset_output_indexed_document_count(output: &DatasetOutputView) -> usize {
    output
        .output_manifest_view
        .as_ref()
        .map(|manifest| manifest.indexed_document_count)
        .unwrap_or(0)
}

fn dataset_output_document_focus(output: &DatasetOutputView) -> ModelFacingDocumentFocus {
    infer_model_facing_document_focus(
        dataset_output_distinct_document_count(output),
        dataset_output_indexed_document_count(output),
    )
}
