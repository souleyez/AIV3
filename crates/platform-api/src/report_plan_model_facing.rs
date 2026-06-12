use contracts::{
    ModelFacingCapabilityClassView, ModelFacingEvidenceStateView, ModelFacingNextActionView,
    ReportPlanStatusView, ReportPlanSummary, WorkflowModelFacingSummaryView,
};

use crate::model_facing_handoff::collect_service_handoff_signals;
use crate::model_facing_policy::build_model_facing_summary;

pub(crate) fn derive_report_plan_model_facing_summary(
    plan: &ReportPlanSummary,
) -> WorkflowModelFacingSummaryView {
    let capability_class = ModelFacingCapabilityClassView::ReportPlanning;
    let evidence_state = infer_report_plan_model_facing_evidence_state(plan);
    let allowed_next_actions = infer_report_plan_model_facing_next_actions(plan, &evidence_state);
    let mut signals = collect_report_plan_model_facing_signals(plan);
    if let Some(handoff) = plan.service_handoff.as_ref() {
        signals.extend(collect_service_handoff_signals(handoff));
    }
    let mut summary = build_model_facing_summary(
        capability_class,
        evidence_state,
        allowed_next_actions,
        signals,
    );
    if let Some(handoff) = plan.service_handoff.as_ref() {
        summary.service_lane = handoff.service_lane.clone();
        summary.report_entry_state = handoff.report_entry_state.clone();
    }
    summary
}

fn infer_report_plan_model_facing_evidence_state(
    plan: &ReportPlanSummary,
) -> ModelFacingEvidenceStateView {
    let has_current_ast_version = plan.current_ast_version_id.is_some();
    match plan.status {
        ReportPlanStatusView::Draft => ModelFacingEvidenceStateView::CatalogMemory,
        ReportPlanStatusView::Planned
        | ReportPlanStatusView::Rendered
        | ReportPlanStatusView::Published => {
            if has_current_ast_version {
                ModelFacingEvidenceStateView::Mixed
            } else {
                ModelFacingEvidenceStateView::Degraded
            }
        }
    }
}

fn infer_report_plan_model_facing_next_actions(
    plan: &ReportPlanSummary,
    evidence_state: &ModelFacingEvidenceStateView,
) -> Vec<ModelFacingNextActionView> {
    if *evidence_state == ModelFacingEvidenceStateView::Degraded {
        return vec![ModelFacingNextActionView::RetryExecution];
    }

    if plan.current_ast_version_id.is_some()
        && matches!(
            plan.status,
            ReportPlanStatusView::Planned
                | ReportPlanStatusView::Rendered
                | ReportPlanStatusView::Published
        )
    {
        return vec![ModelFacingNextActionView::GenerateReportOutput];
    }

    vec![ModelFacingNextActionView::ContinueReportPlanning]
}

fn collect_report_plan_model_facing_signals(plan: &ReportPlanSummary) -> Vec<String> {
    vec![
        "workflow_kind=report_plan".to_string(),
        format!("report_plan_status={:?}", plan.status),
        format!(
            "has_current_ast_version={}",
            plan.current_ast_version_id.is_some()
        ),
        format!("theme_key={}", plan.theme_key),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use contracts::{
        ChatSessionReportEntryResolutionView, ManifestServiceHandoffSourceView,
        ManifestServiceHandoffView, ModelFacingContinuationStateView,
        ModelFacingReportEntryStateView, ModelFacingServiceLaneView,
    };
    use domain_model::{DatasetId, ReportPlanAstVersionId, ReportPlanId};

    fn report_plan(status: ReportPlanStatusView) -> ReportPlanSummary {
        ReportPlanSummary {
            id: ReportPlanId::new(),
            dataset_id: DatasetId::new(),
            title: "Quarterly Report".to_string(),
            objective: "Summarize product and market signals".to_string(),
            status,
            theme_key: "executive-default".to_string(),
            current_ast_version_id: None,
            service_handoff: None,
            model_facing: None,
        }
    }

    #[test]
    fn report_plan_summary_marks_missing_ast_as_degraded() {
        let model_facing =
            derive_report_plan_model_facing_summary(&report_plan(ReportPlanStatusView::Planned));

        assert_eq!(
            model_facing.capability_class,
            ModelFacingCapabilityClassView::ReportPlanning
        );
        assert_eq!(
            model_facing.evidence_state,
            ModelFacingEvidenceStateView::Degraded
        );
        assert_eq!(
            model_facing.continuation_state,
            ModelFacingContinuationStateView::RetryRequired
        );
        assert_eq!(
            model_facing.recommended_next_action,
            Some(ModelFacingNextActionView::RetryExecution)
        );
        assert_eq!(
            model_facing.allowed_next_actions,
            vec![ModelFacingNextActionView::RetryExecution],
        );
        assert!(!model_facing
            .signals
            .iter()
            .any(|signal| signal == "execution_status=failed"));
    }

    #[test]
    fn report_plan_summary_uses_ast_to_continue_report_generation() {
        let mut plan = report_plan(ReportPlanStatusView::Planned);
        plan.current_ast_version_id = Some(ReportPlanAstVersionId::new());

        let model_facing = derive_report_plan_model_facing_summary(&plan);

        assert_eq!(
            model_facing.evidence_state,
            ModelFacingEvidenceStateView::Mixed
        );
        assert_eq!(
            model_facing.continuation_state,
            ModelFacingContinuationStateView::NeedsPlatformContinuation
        );
        assert_eq!(
            model_facing.recommended_next_action,
            Some(ModelFacingNextActionView::GenerateReportOutput)
        );
        assert_eq!(
            model_facing.recommended_tool_key,
            Some("report.render".to_string())
        );
        assert!(model_facing
            .signals
            .iter()
            .any(|signal| signal == "has_current_ast_version=true"));
    }

    #[test]
    fn report_plan_summary_extends_service_handoff_signals_and_lane() {
        let report_plan_id = ReportPlanId::new();
        let mut plan = report_plan(ReportPlanStatusView::Planned);
        plan.id = report_plan_id;
        plan.current_ast_version_id = Some(ReportPlanAstVersionId::new());
        plan.service_handoff = Some(ManifestServiceHandoffView {
            source: ManifestServiceHandoffSourceView::ChatSessionReportEntry,
            service_lane: ModelFacingServiceLaneView::ReportService,
            report_entry_state: ModelFacingReportEntryStateView::Confirmed,
            requested_at: Some(Utc::now()),
            resolved_at: Some(Utc::now()),
            resolved_action: Some(ChatSessionReportEntryResolutionView::EnterReportService),
            suggested_title: Some("Quarterly Report".to_string()),
            suggested_objective: Some("Summarize product and market signals".to_string()),
            confirmed_report_plan_id: Some(report_plan_id),
        });

        let model_facing = derive_report_plan_model_facing_summary(&plan);

        assert_eq!(
            model_facing.service_lane,
            ModelFacingServiceLaneView::ReportService
        );
        assert_eq!(
            model_facing.report_entry_state,
            ModelFacingReportEntryStateView::Confirmed
        );
        assert!(model_facing
            .signals
            .iter()
            .any(|signal| signal == "service_handoff_source=chat_session_report_entry"));
        assert!(model_facing
            .signals
            .iter()
            .any(|signal| *signal == format!("confirmed_report_plan_id={report_plan_id}")));
    }
}
