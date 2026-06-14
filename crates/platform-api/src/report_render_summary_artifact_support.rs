use contracts::{self, HtmlArtifactInteractionModeView, HtmlArtifactManifestView};
use domain_model::ReportPlan;
use serde_json::{json, Value};

use crate::external_channel_output_artifact_manifest;
use crate::html_artifact_summary_support::html_artifact_serialized_variant;
use crate::report_render_output_asset::{
    report_render_output_asset_kind, report_render_output_asset_path,
};

pub(crate) fn report_render_summary_artifact_from_view(
    plan: &ReportPlan,
    output: &contracts::ReportRenderOutputView,
) -> HtmlArtifactManifestView {
    let output_id = output.id.to_string();
    let asset_path = report_render_output_asset_path(output).unwrap_or_default();
    let asset_kind = report_render_output_asset_kind(output).unwrap_or_default();
    let publishable = output.status == contracts::ReportRenderOutputStatusView::Rendered
        && !asset_path.is_empty();
    let title = format!("{} · 渲染摘要", plan.title);
    let status = html_artifact_serialized_variant(&output.status);
    let artifact_manifest = external_channel_output_artifact_manifest(
        "report",
        "report_render_summary",
        &title,
        json!(status.clone()),
        None,
        Vec::new(),
        json!({
            "report_plan_id": plan.id,
            "report_render_output_id": output.id,
            "workflow_execution_id": output.execution_id,
            "ast_version_id": output.ast_version_id,
            "asset_kind": asset_kind,
            "asset_path_present": !asset_path.is_empty(),
        }),
        json!({
            "customer_visible": true,
            "credentials_exposed": false,
            "raw_logs_exposed": false,
            "publishable": publishable,
        }),
    );

    HtmlArtifactManifestView {
        kind: "html_artifact".to_string(),
        version: 1,
        id: format!("html-report-render-{output_id}"),
        title,
        source_type: contracts::HtmlArtifactSourceTypeView::Report,
        template_id: contracts::HtmlArtifactTemplateIdView::ReportRenderSummary,
        owner_scope: contracts::HtmlArtifactOwnerScopeView {
            scope_type: "report_render_output".to_string(),
            id: output_id.clone(),
        },
        data_refs: vec![
            contracts::HtmlArtifactDataRefView {
                kind: "report_plan".to_string(),
                id: plan.id.to_string(),
                label: "Report Plan".to_string(),
            },
            contracts::HtmlArtifactDataRefView {
                kind: "report_render_output".to_string(),
                id: output_id.clone(),
                label: "Render Output".to_string(),
            },
            contracts::HtmlArtifactDataRefView {
                kind: "workflow_execution".to_string(),
                id: output.execution_id.to_string(),
                label: "Workflow".to_string(),
            },
        ],
        provenance: contracts::HtmlArtifactProvenanceView {
            producer: "v3-report-runtime".to_string(),
            reason: "report render output summary".to_string(),
            source_run_id: None,
        },
        interaction_mode: HtmlArtifactInteractionModeView::ReadOnly,
        created_at: output.created_at,
        payload: json!({
            "reportTitle": plan.title,
            "objective": plan.objective,
            "surface": output.surface.as_str(),
            "status": status,
            "publishable": publishable,
            "assetPath": asset_path,
            "assetKind": asset_kind,
            "artifactManifest": artifact_manifest,
            "reportPlanId": plan.id,
            "reportRenderOutputId": output.id,
            "workflowExecutionId": output.execution_id,
            "astVersionId": output.ast_version_id,
            "modelFacing": output.model_facing,
            "serviceHandoff": output.service_handoff,
            "warnings": report_render_summary_warnings(output, publishable),
        }),
    }
}

fn report_render_summary_warnings(
    output: &contracts::ReportRenderOutputView,
    publishable: bool,
) -> Vec<Value> {
    if publishable {
        return Vec::new();
    }
    let (title, detail) = if output.status == contracts::ReportRenderOutputStatusView::Failed {
        (
            "渲染失败",
            "需要重试渲染或检查 report-render-worker 写回的 asset manifest。",
        )
    } else {
        (
            "尚不可发布",
            "报告还没有可发布资产路径，先等待渲染完成或重新发起渲染。",
        )
    };
    vec![json!({
        "title": title,
        "detail": detail,
    })]
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{
        DatasetId, PublishedSurface, ReportPlanAstVersionId, ReportPlanId, ReportPlanStatus,
        ReportRenderOutputId, TenantId, WorkflowExecutionId,
    };

    fn rendered_plan(now: chrono::DateTime<Utc>) -> ReportPlan {
        ReportPlan {
            id: ReportPlanId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            owner_user_id: None,
            title: "季度经营报告".to_string(),
            objective: "汇总订单、客服和风险信号。".to_string(),
            status: ReportPlanStatus::Rendered,
            theme_key: "default-local".to_string(),
            current_ast_version_id: Some(ReportPlanAstVersionId::new()),
            modules: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    fn render_output(
        plan: &ReportPlan,
        status: contracts::ReportRenderOutputStatusView,
        asset_manifest: Value,
    ) -> contracts::ReportRenderOutputView {
        let now = Utc::now();
        contracts::ReportRenderOutputView {
            id: ReportRenderOutputId::new(),
            execution_id: WorkflowExecutionId::new(),
            plan_id: plan.id,
            dataset_id: plan.dataset_id,
            ast_version_id: ReportPlanAstVersionId::new(),
            surface: PublishedSurface::Pc,
            status,
            asset_manifest,
            service_handoff: None,
            model_facing: None,
            created_at: now,
        }
    }

    #[test]
    fn report_render_summary_artifact_exposes_publishable_output_manifest() {
        let now = Utc::now();
        let plan = rendered_plan(now);
        let output = render_output(
            &plan,
            contracts::ReportRenderOutputStatusView::Rendered,
            json!({
                "kind": "html",
                "path": "reports/quarterly/pc.html"
            }),
        );
        let output_id = output.id;

        let artifact = report_render_summary_artifact_from_view(&plan, &output);

        assert_eq!(
            artifact.source_type,
            contracts::HtmlArtifactSourceTypeView::Report
        );
        assert_eq!(
            artifact.template_id,
            contracts::HtmlArtifactTemplateIdView::ReportRenderSummary
        );
        assert_eq!(artifact.owner_scope.scope_type, "report_render_output");
        assert_eq!(artifact.owner_scope.id, output_id.to_string());
        assert_eq!(artifact.payload["reportPlanId"], json!(plan.id));
        assert_eq!(
            artifact.payload["assetPath"],
            json!("reports/quarterly/pc.html")
        );
        assert_eq!(artifact.payload["assetKind"], json!("html"));
        assert_eq!(artifact.payload["publishable"], json!(true));
        assert_eq!(
            artifact.payload["artifactManifest"]["schema"],
            json!("v3.output_artifact_manifest")
        );
        assert_eq!(
            artifact.payload["artifactManifest"]["artifact_type"],
            json!("report")
        );
        assert_eq!(
            artifact.payload["artifactManifest"]["artifact_kind"],
            json!("report_render_summary")
        );
        assert_eq!(
            artifact.payload["artifactManifest"]["refs"]["report_render_output_id"],
            json!(output_id)
        );
        assert_eq!(
            artifact.payload["artifactManifest"]["safety"]["publishable"],
            json!(true)
        );
        assert!(artifact.payload["warnings"].as_array().unwrap().is_empty());
    }

    #[test]
    fn report_render_summary_artifact_warns_for_failed_and_missing_asset_output() {
        let now = Utc::now();
        let plan = rendered_plan(now);
        let failed = report_render_summary_artifact_from_view(
            &plan,
            &render_output(
                &plan,
                contracts::ReportRenderOutputStatusView::Failed,
                json!({}),
            ),
        );
        let rendered_without_asset = report_render_summary_artifact_from_view(
            &plan,
            &render_output(
                &plan,
                contracts::ReportRenderOutputStatusView::Rendered,
                json!({ "kind": "html" }),
            ),
        );

        assert_eq!(failed.payload["publishable"], json!(false));
        assert_eq!(failed.payload["warnings"][0]["title"], json!("渲染失败"));
        assert_eq!(
            failed.payload["artifactManifest"]["safety"]["publishable"],
            json!(false)
        );
        assert_eq!(rendered_without_asset.payload["publishable"], json!(false));
        assert_eq!(
            rendered_without_asset.payload["warnings"][0]["title"],
            json!("尚不可发布")
        );
    }
}
