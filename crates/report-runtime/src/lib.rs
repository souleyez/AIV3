use domain_model::{DatasetId, PublishedSurface, ReportPlanAstVersionId, ReportPlanId};
use serde_json::{json, Value};

#[derive(Clone, Debug)]
pub struct ReportRenderRequest {
    pub plan_id: ReportPlanId,
    pub dataset_id: DatasetId,
    pub ast_version_id: ReportPlanAstVersionId,
    pub surface: PublishedSurface,
    pub theme_key: String,
}

#[derive(Clone, Debug)]
pub struct ReportRenderOutcome {
    pub asset_count: usize,
    pub manifest_key: String,
    pub asset_manifest: Value,
}

pub trait ReportRuntime {
    fn render(&self, request: &ReportRenderRequest) -> ReportRenderOutcome;
}

#[derive(Clone, Debug, Default)]
pub struct PlaceholderReportRuntime;

impl ReportRuntime for PlaceholderReportRuntime {
    fn render(&self, request: &ReportRenderRequest) -> ReportRenderOutcome {
        let manifest_key = format!(
            "renders/{}/{}/{}.json",
            request.plan_id,
            request.ast_version_id,
            request.surface.as_str()
        );

        ReportRenderOutcome {
            asset_count: 2,
            manifest_key: manifest_key.clone(),
            asset_manifest: json!({
                "manifest_key": manifest_key,
                "surface": request.surface.as_str(),
                "theme_key": request.theme_key,
                "assets": [
                    {
                        "kind": "html",
                        "path": format!("renders/{}/{}/index-{}.html", request.plan_id, request.ast_version_id, request.surface.as_str())
                    },
                    {
                        "kind": "css",
                        "path": format!("renders/{}/{}/styles-{}.css", request.plan_id, request.ast_version_id, request.surface.as_str())
                    }
                ]
            }),
        }
    }
}
