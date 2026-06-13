use crate::report_plan_model_facing::derive_report_plan_model_facing_summary;
use crate::report_render_model_facing::derive_report_render_output_model_facing_summary;
use contracts::{
    PublishedReportVersionView, PublishedReportView, ReportPlanAstVersionView, ReportPlanSummary,
    ReportRenderOutputView,
};
use domain_model::{
    PublishedReport, PublishedReportVersion, ReportPlan, ReportPlanAstVersion, ReportRenderOutput,
};

pub(crate) fn to_report_plan_summary(
    plan: ReportPlan,
    service_handoff: Option<contracts::ManifestServiceHandoffView>,
) -> ReportPlanSummary {
    let mut view = ReportPlanSummary {
        id: plan.id,
        dataset_id: plan.dataset_id,
        title: plan.title,
        objective: plan.objective,
        status: contracts::ReportPlanStatusView::from_domain(plan.status),
        theme_key: plan.theme_key,
        current_ast_version_id: plan.current_ast_version_id,
        service_handoff,
        model_facing: None,
    };
    view.model_facing = Some(derive_report_plan_model_facing_summary(&view));
    view
}

pub(crate) fn to_report_plan_ast_version_view(
    version: ReportPlanAstVersion,
) -> ReportPlanAstVersionView {
    ReportPlanAstVersionView {
        id: version.id,
        plan_id: version.plan_id,
        version_no: version.version_no,
        ast: version.ast,
        created_at: version.created_at,
    }
}

pub(crate) fn to_report_render_output_view(
    output: ReportRenderOutput,
    service_handoff: Option<contracts::ManifestServiceHandoffView>,
) -> ReportRenderOutputView {
    let mut view = ReportRenderOutputView {
        id: output.id,
        execution_id: output.execution_id,
        plan_id: output.plan_id,
        dataset_id: output.dataset_id,
        ast_version_id: output.ast_version_id,
        surface: output.surface,
        status: contracts::ReportRenderOutputStatusView::from_domain(output.status),
        asset_manifest: output.asset_manifest,
        service_handoff,
        model_facing: None,
        created_at: output.created_at,
    };
    view.model_facing = Some(derive_report_render_output_model_facing_summary(&view));
    view
}

pub(crate) fn to_published_report_view(report: PublishedReport) -> PublishedReportView {
    PublishedReportView {
        id: report.id,
        dataset_id: report.dataset_id,
        plan_id: report.plan_id,
        slug: report.slug,
        current_version_id: report.current_version_id,
        created_at: report.created_at,
        updated_at: report.updated_at,
    }
}

pub(crate) fn to_published_report_version_view(
    version: PublishedReportVersion,
) -> PublishedReportVersionView {
    PublishedReportVersionView {
        id: version.id,
        report_id: version.report_id,
        version_no: version.version_no,
        surface: version.surface,
        asset_manifest: version.asset_manifest,
        created_at: version.created_at,
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use domain_model::{
        DatasetId, PublishedReportId, PublishedReportVersionId, PublishedSurface,
        ReportPlanAstVersionId, ReportPlanId, ReportPlanStatus, ReportRenderOutputId,
        ReportRenderOutputStatus, TenantId, WorkflowExecutionId,
    };
    use serde_json::json;

    use super::*;

    fn fixed_time() -> DateTime<Utc> {
        "2026-06-14T00:00:00Z"
            .parse()
            .expect("fixed timestamp should parse")
    }

    #[test]
    fn report_plan_summary_preserves_contract_fields_and_model_facing() {
        let now = fixed_time();
        let ast_version_id = ReportPlanAstVersionId::new();
        let plan = ReportPlan {
            id: ReportPlanId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            owner_user_id: None,
            title: "Monthly Report".to_string(),
            objective: "Track operating signals".to_string(),
            status: ReportPlanStatus::Planned,
            theme_key: "xinbai-monthly".to_string(),
            current_ast_version_id: Some(ast_version_id),
            modules: Vec::new(),
            created_at: now,
            updated_at: now,
        };
        let id = plan.id;
        let dataset_id = plan.dataset_id;

        let view = to_report_plan_summary(plan, None);

        assert_eq!(view.id, id);
        assert_eq!(view.dataset_id, dataset_id);
        assert_eq!(view.title, "Monthly Report");
        assert_eq!(view.objective, "Track operating signals");
        assert_eq!(view.status, contracts::ReportPlanStatusView::Planned);
        assert_eq!(view.theme_key, "xinbai-monthly");
        assert_eq!(view.current_ast_version_id, Some(ast_version_id));
        assert_eq!(
            view.model_facing
                .as_ref()
                .map(|model| &model.capability_class),
            Some(&contracts::ModelFacingCapabilityClassView::ReportPlanning)
        );
    }

    #[test]
    fn report_plan_ast_version_view_preserves_ast_and_version() {
        let now = fixed_time();
        let version = ReportPlanAstVersion {
            id: ReportPlanAstVersionId::new(),
            plan_id: ReportPlanId::new(),
            version_no: 7,
            ast: json!({"modules": [{"key": "risk"}]}),
            created_at: now,
        };
        let id = version.id;
        let plan_id = version.plan_id;

        let view = to_report_plan_ast_version_view(version);

        assert_eq!(view.id, id);
        assert_eq!(view.plan_id, plan_id);
        assert_eq!(view.version_no, 7);
        assert_eq!(view.ast["modules"][0]["key"], json!("risk"));
        assert_eq!(view.created_at, now);
    }

    #[test]
    fn report_render_output_view_preserves_manifest_status_and_model_facing() {
        let now = fixed_time();
        let output = ReportRenderOutput {
            id: ReportRenderOutputId::new(),
            tenant_id: TenantId::new(),
            execution_id: WorkflowExecutionId::new(),
            plan_id: ReportPlanId::new(),
            dataset_id: DatasetId::new(),
            ast_version_id: ReportPlanAstVersionId::new(),
            surface: PublishedSurface::Mobile,
            status: ReportRenderOutputStatus::Rendered,
            asset_manifest: json!({
                "public_url": "/generated-artifacts/report/index.html",
                "exports": {
                    "table_data_csv_url": "/generated-artifacts/report/table-data.csv",
                    "ppt_url": "/generated-artifacts/report/report.ppt",
                    "markdown_url": "/generated-artifacts/report/report.md"
                }
            }),
            created_at: now,
        };
        let id = output.id;
        let execution_id = output.execution_id;

        let view = to_report_render_output_view(output, None);

        assert_eq!(view.id, id);
        assert_eq!(view.execution_id, execution_id);
        assert_eq!(view.surface, PublishedSurface::Mobile);
        assert_eq!(
            view.status,
            contracts::ReportRenderOutputStatusView::Rendered
        );
        assert_eq!(
            view.asset_manifest["exports"]["ppt_url"],
            json!("/generated-artifacts/report/report.ppt")
        );
        assert_eq!(
            view.model_facing
                .as_ref()
                .map(|model| &model.capability_class),
            Some(&contracts::ModelFacingCapabilityClassView::ReportGenerationAndEditing)
        );
    }

    #[test]
    fn published_report_views_preserve_public_artifact_fields() {
        let now = fixed_time();
        let report_id = PublishedReportId::new();
        let version_id = PublishedReportVersionId::new();
        let plan_id = ReportPlanId::new();
        let report = PublishedReport {
            id: report_id,
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            plan_id,
            slug: "xinbai-monthly".to_string(),
            current_version_id: Some(version_id),
            created_at: now,
            updated_at: now,
        };
        let version = PublishedReportVersion {
            id: version_id,
            report_id,
            version_no: 3,
            surface: PublishedSurface::Pc,
            asset_manifest: json!({
                "public_url": "/generated-artifacts/xinbai/index.html",
                "exports": {
                    "table_data_csv_url": "/generated-artifacts/xinbai/table-data.csv",
                    "ppt_url": "/generated-artifacts/xinbai/report.ppt",
                    "markdown_url": "/generated-artifacts/xinbai/report.md"
                }
            }),
            created_at: now,
        };

        let report_view = to_published_report_view(report);
        let version_view = to_published_report_version_view(version);

        assert_eq!(report_view.id, report_id);
        assert_eq!(report_view.plan_id, plan_id);
        assert_eq!(report_view.slug, "xinbai-monthly");
        assert_eq!(report_view.current_version_id, Some(version_id));
        assert_eq!(version_view.id, version_id);
        assert_eq!(version_view.report_id, report_id);
        assert_eq!(version_view.version_no, 3);
        assert_eq!(version_view.surface, PublishedSurface::Pc);
        assert_eq!(
            version_view.asset_manifest["exports"]["markdown_url"],
            json!("/generated-artifacts/xinbai/report.md")
        );
    }
}
