use contracts::{
    ModelFacingCapabilityClassView, ModelFacingContinuationStateView, ModelFacingEvidenceStateView,
    ModelFacingNextActionView, ModelFacingReportEntryStateView, ModelFacingServiceLaneView,
    WorkflowModelFacingSummaryView,
};

use crate::model_facing_format::{
    default_tool_key_for_model_facing_next_action, default_tool_keys_for_model_facing_next_actions,
};

pub(crate) fn build_model_facing_summary(
    capability_class: ModelFacingCapabilityClassView,
    evidence_state: ModelFacingEvidenceStateView,
    mut allowed_next_actions: Vec<ModelFacingNextActionView>,
    signals: Vec<String>,
) -> WorkflowModelFacingSummaryView {
    allowed_next_actions.dedup();
    let service_lane = infer_model_facing_service_lane(&capability_class);
    let report_entry_state = infer_model_facing_report_entry_state(&capability_class);
    let continuation_state =
        infer_model_facing_continuation_state(&evidence_state, &allowed_next_actions);
    let recommended_next_action =
        infer_model_facing_recommended_next_action(&continuation_state, &allowed_next_actions);
    let recommended_tool_key = recommended_next_action
        .as_ref()
        .and_then(default_tool_key_for_model_facing_next_action)
        .map(str::to_string);
    let allowed_tool_keys = default_tool_keys_for_model_facing_next_actions(&allowed_next_actions);

    WorkflowModelFacingSummaryView {
        capability_class,
        service_lane,
        report_entry_state,
        evidence_state,
        continuation_state,
        recommended_next_action,
        allowed_next_actions,
        recommended_tool_key,
        allowed_tool_keys,
        signals,
    }
}

pub(crate) fn degraded_model_facing_summary(
    capability_class: ModelFacingCapabilityClassView,
    mut signals: Vec<String>,
) -> WorkflowModelFacingSummaryView {
    if !signals
        .iter()
        .any(|signal| signal == "execution_status=failed")
    {
        signals.push("execution_status=failed".to_string());
    }
    build_model_facing_summary(
        capability_class,
        ModelFacingEvidenceStateView::Degraded,
        vec![ModelFacingNextActionView::RetryExecution],
        signals,
    )
}

fn infer_model_facing_service_lane(
    capability_class: &ModelFacingCapabilityClassView,
) -> ModelFacingServiceLaneView {
    match capability_class {
        ModelFacingCapabilityClassView::DatasetDirectoryAwareness
        | ModelFacingCapabilityClassView::EvidenceRetrieval
        | ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis => {
            ModelFacingServiceLaneView::MaterialService
        }
        ModelFacingCapabilityClassView::ReportPlanning
        | ModelFacingCapabilityClassView::ReportGenerationAndEditing => {
            ModelFacingServiceLaneView::ReportService
        }
        ModelFacingCapabilityClassView::ControlledPlatformAction => {
            ModelFacingServiceLaneView::ControlledPlatformAction
        }
    }
}

fn infer_model_facing_report_entry_state(
    capability_class: &ModelFacingCapabilityClassView,
) -> ModelFacingReportEntryStateView {
    match capability_class {
        ModelFacingCapabilityClassView::ReportPlanning
        | ModelFacingCapabilityClassView::ReportGenerationAndEditing => {
            ModelFacingReportEntryStateView::Confirmed
        }
        ModelFacingCapabilityClassView::DatasetDirectoryAwareness
        | ModelFacingCapabilityClassView::EvidenceRetrieval
        | ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis
        | ModelFacingCapabilityClassView::ControlledPlatformAction => {
            ModelFacingReportEntryStateView::NotApplicable
        }
    }
}

fn infer_model_facing_continuation_state(
    evidence_state: &ModelFacingEvidenceStateView,
    allowed_next_actions: &[ModelFacingNextActionView],
) -> ModelFacingContinuationStateView {
    if *evidence_state == ModelFacingEvidenceStateView::Degraded {
        return ModelFacingContinuationStateView::RetryRequired;
    }
    if allowed_next_actions
        .iter()
        .any(|action| *action == ModelFacingNextActionView::RequestReportEntryConfirmation)
    {
        return ModelFacingContinuationStateView::NeedsUserConfirmation;
    }
    if allowed_next_actions.iter().any(|action| {
        matches!(
            action,
            ModelFacingNextActionView::WaitForToolLoop
                | ModelFacingNextActionView::FinalizeArtifactCommit
        )
    }) {
        return ModelFacingContinuationStateView::WaitingForRuntime;
    }
    if allowed_next_actions
        .iter()
        .any(|action| *action == ModelFacingNextActionView::AnswerDirectly)
        || allowed_next_actions.is_empty()
    {
        return ModelFacingContinuationStateView::ReadyToAnswer;
    }

    ModelFacingContinuationStateView::NeedsPlatformContinuation
}

fn infer_model_facing_recommended_next_action(
    continuation_state: &ModelFacingContinuationStateView,
    allowed_next_actions: &[ModelFacingNextActionView],
) -> Option<ModelFacingNextActionView> {
    match continuation_state {
        ModelFacingContinuationStateView::RetryRequired => allowed_next_actions
            .iter()
            .find(|action| **action == ModelFacingNextActionView::RetryExecution)
            .cloned()
            .or_else(|| allowed_next_actions.first().cloned()),
        ModelFacingContinuationStateView::NeedsUserConfirmation => allowed_next_actions
            .iter()
            .find(|action| **action == ModelFacingNextActionView::RequestReportEntryConfirmation)
            .cloned()
            .or_else(|| allowed_next_actions.first().cloned()),
        ModelFacingContinuationStateView::WaitingForRuntime => allowed_next_actions
            .iter()
            .find(|action| **action == ModelFacingNextActionView::WaitForToolLoop)
            .cloned()
            .or_else(|| {
                allowed_next_actions
                    .iter()
                    .find(|action| **action == ModelFacingNextActionView::FinalizeArtifactCommit)
                    .cloned()
            })
            .or_else(|| allowed_next_actions.first().cloned()),
        ModelFacingContinuationStateView::ReadyToAnswer => allowed_next_actions
            .iter()
            .find(|action| **action == ModelFacingNextActionView::AnswerDirectly)
            .cloned()
            .or_else(|| allowed_next_actions.first().cloned()),
        ModelFacingContinuationStateView::NeedsPlatformContinuation => allowed_next_actions
            .iter()
            .find(|action| {
                !matches!(
                    action,
                    ModelFacingNextActionView::AnswerDirectly
                        | ModelFacingNextActionView::WaitForToolLoop
                        | ModelFacingNextActionView::FinalizeArtifactCommit
                        | ModelFacingNextActionView::RetryExecution
                )
            })
            .cloned()
            .or_else(|| allowed_next_actions.first().cloned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_model_facing_summary_sets_lane_state_and_tool_keys() {
        let summary = build_model_facing_summary(
            ModelFacingCapabilityClassView::ReportGenerationAndEditing,
            ModelFacingEvidenceStateView::Mixed,
            vec![
                ModelFacingNextActionView::GenerateReportOutput,
                ModelFacingNextActionView::GenerateReportOutput,
                ModelFacingNextActionView::PublishReport,
            ],
            vec!["workflow_kind=report_render_workflow".to_string()],
        );

        assert_eq!(
            summary.service_lane,
            ModelFacingServiceLaneView::ReportService
        );
        assert_eq!(
            summary.report_entry_state,
            ModelFacingReportEntryStateView::Confirmed,
        );
        assert_eq!(
            summary.continuation_state,
            ModelFacingContinuationStateView::NeedsPlatformContinuation,
        );
        assert_eq!(
            summary.recommended_next_action,
            Some(ModelFacingNextActionView::GenerateReportOutput),
        );
        assert_eq!(
            summary.recommended_tool_key,
            Some("report.render".to_string())
        );
        assert_eq!(
            summary.allowed_tool_keys,
            vec!["report.render".to_string(), "report.publish".to_string()],
        );
        assert_eq!(
            summary.allowed_next_actions,
            vec![
                ModelFacingNextActionView::GenerateReportOutput,
                ModelFacingNextActionView::PublishReport,
            ],
        );
    }

    #[test]
    fn model_facing_policy_prioritizes_user_confirmation_runtime_waits_and_retry() {
        let confirm = build_model_facing_summary(
            ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis,
            ModelFacingEvidenceStateView::Mixed,
            vec![
                ModelFacingNextActionView::AnswerDirectly,
                ModelFacingNextActionView::RequestReportEntryConfirmation,
            ],
            Vec::new(),
        );
        assert_eq!(
            confirm.continuation_state,
            ModelFacingContinuationStateView::NeedsUserConfirmation,
        );
        assert_eq!(
            confirm.recommended_next_action,
            Some(ModelFacingNextActionView::RequestReportEntryConfirmation),
        );

        let waiting = build_model_facing_summary(
            ModelFacingCapabilityClassView::ControlledPlatformAction,
            ModelFacingEvidenceStateView::SupplyOnly,
            vec![
                ModelFacingNextActionView::FinalizeArtifactCommit,
                ModelFacingNextActionView::WaitForToolLoop,
            ],
            Vec::new(),
        );
        assert_eq!(
            waiting.continuation_state,
            ModelFacingContinuationStateView::WaitingForRuntime,
        );
        assert_eq!(
            waiting.recommended_next_action,
            Some(ModelFacingNextActionView::WaitForToolLoop),
        );

        let failed = degraded_model_facing_summary(
            ModelFacingCapabilityClassView::EvidenceRetrieval,
            vec!["execution_status=failed".to_string()],
        );
        assert_eq!(
            failed.continuation_state,
            ModelFacingContinuationStateView::RetryRequired,
        );
        assert_eq!(
            failed.recommended_next_action,
            Some(ModelFacingNextActionView::RetryExecution),
        );
        assert_eq!(failed.signals, vec!["execution_status=failed".to_string()],);
    }
}
