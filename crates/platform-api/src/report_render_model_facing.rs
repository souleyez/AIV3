use contracts::{
    ModelFacingCapabilityClassView, ModelFacingEvidenceStateView, ModelFacingNextActionView,
    ReportRenderOutputStatusView, ReportRenderOutputView, WorkflowModelFacingSummaryView,
};

use crate::model_facing_handoff::collect_service_handoff_signals;
use crate::model_facing_policy::build_model_facing_summary;
use crate::report_render_output_asset::{
    report_render_output_asset_kind, report_render_output_has_asset_path,
};

pub(crate) fn derive_report_render_output_model_facing_summary(
    output: &ReportRenderOutputView,
) -> WorkflowModelFacingSummaryView {
    let capability_class = ModelFacingCapabilityClassView::ReportGenerationAndEditing;
    let evidence_state = infer_report_render_output_model_facing_evidence_state(output);
    let allowed_next_actions =
        infer_report_render_output_model_facing_next_actions(output, &evidence_state);
    let mut signals = collect_report_render_output_model_facing_signals(output);
    if let Some(handoff) = output.service_handoff.as_ref() {
        signals.extend(collect_service_handoff_signals(handoff));
    }
    let mut summary = build_model_facing_summary(
        capability_class,
        evidence_state,
        allowed_next_actions,
        signals,
    );
    if let Some(handoff) = output.service_handoff.as_ref() {
        summary.service_lane = handoff.service_lane.clone();
        summary.report_entry_state = handoff.report_entry_state.clone();
    }
    summary
}

fn infer_report_render_output_model_facing_evidence_state(
    output: &ReportRenderOutputView,
) -> ModelFacingEvidenceStateView {
    match output.status {
        ReportRenderOutputStatusView::Rendered => ModelFacingEvidenceStateView::Mixed,
        ReportRenderOutputStatusView::Failed => ModelFacingEvidenceStateView::Degraded,
    }
}

fn infer_report_render_output_model_facing_next_actions(
    output: &ReportRenderOutputView,
    evidence_state: &ModelFacingEvidenceStateView,
) -> Vec<ModelFacingNextActionView> {
    if *evidence_state == ModelFacingEvidenceStateView::Degraded {
        return vec![ModelFacingNextActionView::RetryExecution];
    }

    let mut actions = Vec::new();
    if report_render_output_has_asset_path(output) {
        actions.push(ModelFacingNextActionView::PublishReport);
    }
    actions
}

fn collect_report_render_output_model_facing_signals(
    output: &ReportRenderOutputView,
) -> Vec<String> {
    let mut signals = vec![
        "workflow_kind=report_render".to_string(),
        format!("report_render_status={:?}", output.status),
        format!("surface={}", output.surface.as_str()),
        format!(
            "has_asset_path={}",
            report_render_output_has_asset_path(output)
        ),
    ];
    if let Some(kind) = report_render_output_asset_kind(output) {
        signals.push(format!("asset_kind={kind}"));
    }
    signals
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
    use domain_model::{
        DatasetId, PublishedSurface, ReportPlanAstVersionId, ReportPlanId, ReportRenderOutputId,
        WorkflowExecutionId,
    };
    use serde_json::json;

    fn report_render_output(status: ReportRenderOutputStatusView) -> ReportRenderOutputView {
        ReportRenderOutputView {
            id: ReportRenderOutputId::new(),
            execution_id: WorkflowExecutionId::new(),
            plan_id: ReportPlanId::new(),
            dataset_id: DatasetId::new(),
            ast_version_id: ReportPlanAstVersionId::new(),
            surface: PublishedSurface::Pc,
            status,
            asset_manifest: json!({ "kind": "html_report", "path": " reports/demo/pc.html " }),
            service_handoff: None,
            model_facing: None,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn report_render_summary_publishes_rendered_asset() {
        let model_facing = derive_report_render_output_model_facing_summary(&report_render_output(
            ReportRenderOutputStatusView::Rendered,
        ));

        assert_eq!(
            model_facing.capability_class,
            ModelFacingCapabilityClassView::ReportGenerationAndEditing
        );
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
            Some(ModelFacingNextActionView::PublishReport)
        );
        assert_eq!(
            model_facing.recommended_tool_key,
            Some("report.publish".to_string())
        );
        assert!(model_facing
            .signals
            .iter()
            .any(|signal| signal == "has_asset_path=true"));
        assert!(model_facing
            .signals
            .iter()
            .any(|signal| signal == "asset_kind=html_report"));
    }

    #[test]
    fn report_render_summary_marks_failed_output_as_degraded_without_extra_failed_signal() {
        let mut output = report_render_output(ReportRenderOutputStatusView::Failed);
        output.asset_manifest = json!({ "kind": "placeholder_asset" });

        let model_facing = derive_report_render_output_model_facing_summary(&output);

        assert_eq!(
            model_facing.evidence_state,
            ModelFacingEvidenceStateView::Degraded
        );
        assert_eq!(
            model_facing.continuation_state,
            ModelFacingContinuationStateView::RetryRequired
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
    fn report_render_summary_extends_service_handoff_signals_and_lane() {
        let report_plan_id = ReportPlanId::new();
        let mut output = report_render_output(ReportRenderOutputStatusView::Rendered);
        output.plan_id = report_plan_id;
        output.service_handoff = Some(ManifestServiceHandoffView {
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

        let model_facing = derive_report_render_output_model_facing_summary(&output);

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
