#[cfg(test)]
use contracts::{
    ChatSessionReportEntryResolutionView, ManifestServiceHandoffSourceView,
    ModelFacingReportEntryStateView,
};
use contracts::{
    ManifestServiceHandoffView, ModelFacingCapabilityClassView, ModelFacingEvidenceStateView,
    ModelFacingNextActionView, ModelFacingServiceLaneView,
};

use crate::model_facing_format::{
    format_chat_session_report_entry_resolution, format_manifest_service_handoff_source,
    format_model_facing_report_entry_state, format_model_facing_service_lane,
};

pub(crate) fn collect_service_handoff_signals(handoff: &ManifestServiceHandoffView) -> Vec<String> {
    let mut signals = vec![
        format!(
            "service_handoff_source={}",
            format_manifest_service_handoff_source(&handoff.source)
        ),
        format!(
            "service_handoff_lane={}",
            format_model_facing_service_lane(&handoff.service_lane)
        ),
        format!(
            "report_entry_state={}",
            format_model_facing_report_entry_state(&handoff.report_entry_state)
        ),
        format!(
            "service_handoff_suggested_title_present={}",
            handoff.suggested_title.is_some()
        ),
        format!(
            "service_handoff_suggested_objective_present={}",
            handoff.suggested_objective.is_some()
        ),
    ];
    if let Some(resolved_action) = handoff.resolved_action.as_ref() {
        signals.push(format!(
            "report_entry_resolved_action={}",
            format_chat_session_report_entry_resolution(resolved_action)
        ));
    }
    if let Some(report_plan_id) = handoff.confirmed_report_plan_id {
        signals.push(format!("confirmed_report_plan_id={report_plan_id}"));
    }
    signals
}

pub(crate) fn infer_service_handoff_capability_class(
    handoff: &ManifestServiceHandoffView,
    fallback: &ModelFacingCapabilityClassView,
) -> ModelFacingCapabilityClassView {
    match handoff.service_lane {
        ModelFacingServiceLaneView::ReportService => ModelFacingCapabilityClassView::ReportPlanning,
        ModelFacingServiceLaneView::ControlledPlatformAction => {
            ModelFacingCapabilityClassView::ControlledPlatformAction
        }
        ModelFacingServiceLaneView::MaterialService => fallback.clone(),
    }
}

pub(crate) fn infer_service_handoff_next_actions(
    handoff: &ManifestServiceHandoffView,
    fallback: &ModelFacingCapabilityClassView,
    evidence_state: &ModelFacingEvidenceStateView,
) -> Vec<ModelFacingNextActionView> {
    if *evidence_state == ModelFacingEvidenceStateView::Degraded {
        return vec![ModelFacingNextActionView::RetryExecution];
    }

    match infer_service_handoff_capability_class(handoff, fallback) {
        ModelFacingCapabilityClassView::ReportPlanning => {
            vec![ModelFacingNextActionView::ContinueReportPlanning]
        }
        ModelFacingCapabilityClassView::ReportGenerationAndEditing => {
            vec![ModelFacingNextActionView::GenerateReportOutput]
        }
        ModelFacingCapabilityClassView::ControlledPlatformAction => {
            vec![ModelFacingNextActionView::RetryExecution]
        }
        ModelFacingCapabilityClassView::DatasetDirectoryAwareness
        | ModelFacingCapabilityClassView::EvidenceRetrieval
        | ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis => {
            vec![ModelFacingNextActionView::AnswerDirectly]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handoff(
        service_lane: ModelFacingServiceLaneView,
        report_entry_state: ModelFacingReportEntryStateView,
    ) -> ManifestServiceHandoffView {
        ManifestServiceHandoffView {
            source: ManifestServiceHandoffSourceView::ChatSessionReportEntry,
            service_lane,
            report_entry_state,
            requested_at: None,
            resolved_at: None,
            resolved_action: Some(ChatSessionReportEntryResolutionView::EnterReportService),
            suggested_title: Some("Monthly operating report".to_string()),
            suggested_objective: None,
            confirmed_report_plan_id: None,
        }
    }

    #[test]
    fn service_handoff_signals_keep_protocol_fields_and_order() {
        let signals = collect_service_handoff_signals(&handoff(
            ModelFacingServiceLaneView::ReportService,
            ModelFacingReportEntryStateView::Confirmed,
        ));

        assert_eq!(
            signals,
            vec![
                "service_handoff_source=chat_session_report_entry".to_string(),
                "service_handoff_lane=report_service".to_string(),
                "report_entry_state=confirmed".to_string(),
                "service_handoff_suggested_title_present=true".to_string(),
                "service_handoff_suggested_objective_present=false".to_string(),
                "report_entry_resolved_action=enter_report_service".to_string(),
            ],
        );
    }

    #[test]
    fn service_handoff_capability_class_prefers_lane_then_fallback() {
        let fallback = ModelFacingCapabilityClassView::EvidenceRetrieval;
        assert_eq!(
            infer_service_handoff_capability_class(
                &handoff(
                    ModelFacingServiceLaneView::ReportService,
                    ModelFacingReportEntryStateView::Confirmed,
                ),
                &fallback,
            ),
            ModelFacingCapabilityClassView::ReportPlanning,
        );
        assert_eq!(
            infer_service_handoff_capability_class(
                &handoff(
                    ModelFacingServiceLaneView::ControlledPlatformAction,
                    ModelFacingReportEntryStateView::Confirmed,
                ),
                &fallback,
            ),
            ModelFacingCapabilityClassView::ControlledPlatformAction,
        );
        assert_eq!(
            infer_service_handoff_capability_class(
                &handoff(
                    ModelFacingServiceLaneView::MaterialService,
                    ModelFacingReportEntryStateView::NotApplicable,
                ),
                &fallback,
            ),
            ModelFacingCapabilityClassView::EvidenceRetrieval,
        );
    }

    #[test]
    fn service_handoff_next_actions_keep_degraded_and_lane_semantics() {
        let fallback = ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis;
        assert_eq!(
            infer_service_handoff_next_actions(
                &handoff(
                    ModelFacingServiceLaneView::ReportService,
                    ModelFacingReportEntryStateView::Confirmed,
                ),
                &fallback,
                &ModelFacingEvidenceStateView::Mixed,
            ),
            vec![ModelFacingNextActionView::ContinueReportPlanning],
        );
        assert_eq!(
            infer_service_handoff_next_actions(
                &handoff(
                    ModelFacingServiceLaneView::MaterialService,
                    ModelFacingReportEntryStateView::NotApplicable,
                ),
                &fallback,
                &ModelFacingEvidenceStateView::SupplyOnly,
            ),
            vec![ModelFacingNextActionView::AnswerDirectly],
        );
        assert_eq!(
            infer_service_handoff_next_actions(
                &handoff(
                    ModelFacingServiceLaneView::ReportService,
                    ModelFacingReportEntryStateView::Confirmed,
                ),
                &fallback,
                &ModelFacingEvidenceStateView::Degraded,
            ),
            vec![ModelFacingNextActionView::RetryExecution],
        );
    }
}
