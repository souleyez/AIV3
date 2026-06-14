use contracts::{self, HtmlArtifactInteractionModeView, HtmlArtifactManifestView};
use domain_model::StaticPageDraft;
use serde_json::{json, Value};

use crate::static_page_payload_support::static_page_payload_value;

pub(crate) fn static_page_data_quality_artifact_from_draft(
    draft: StaticPageDraft,
) -> Option<HtmlArtifactManifestView> {
    let final_page = static_page_payload_value(&draft.draft_payload, &["finalPage", "final_page"])?;
    let asset_manifest = final_page
        .get("assetManifest")
        .or_else(|| final_page.get("asset_manifest"))?;
    let summary = static_page_final_data_quality_summary(asset_manifest)?;
    let modules = static_page_final_data_quality_modules(asset_manifest);
    if modules.is_empty()
        && !summary.as_object().is_some_and(|object| {
            object
                .values()
                .any(|value| value.as_i64().unwrap_or_default() > 0)
        })
    {
        return None;
    }

    let draft_id = draft.id.to_string();
    Some(HtmlArtifactManifestView {
        kind: "html_artifact".to_string(),
        version: 1,
        id: format!("html-static-page-quality-{draft_id}"),
        title: format!("{} · 数据质量报告", draft.title),
        source_type: contracts::HtmlArtifactSourceTypeView::StaticPage,
        template_id: contracts::HtmlArtifactTemplateIdView::StaticPageDataQualityReport,
        owner_scope: contracts::HtmlArtifactOwnerScopeView {
            scope_type: "static_page_draft".to_string(),
            id: draft_id.clone(),
        },
        data_refs: Vec::new(),
        provenance: contracts::HtmlArtifactProvenanceView {
            producer: "v3-static-page-renderer".to_string(),
            reason: "static page final render data quality report".to_string(),
            source_run_id: Some(draft.assistant_run_id.to_string()),
        },
        interaction_mode: HtmlArtifactInteractionModeView::ReadOnly,
        created_at: draft.updated_at,
        payload: json!({
            "draftId": draft_id,
            "finalStatus": final_page.get("status").and_then(Value::as_str).unwrap_or("unknown"),
            "summary": summary,
            "modules": modules,
            "note": "最终渲染数据质量报告用于交付前检查模块数据、ECharts 可水合状态和静态回退。"
        }),
    })
}

fn static_page_final_data_quality_summary(asset_manifest: &Value) -> Option<Value> {
    asset_manifest
        .get("export_package")
        .and_then(|package| package.get("debug"))
        .and_then(|debug| debug.get("data_quality_summary"))
        .cloned()
        .or_else(|| {
            asset_manifest
                .get("chart_runtime")
                .and_then(|runtime| runtime.get("dataQualitySummary"))
                .cloned()
        })
        .or_else(|| asset_manifest.get("data_quality_summary").cloned())
}

fn static_page_final_data_quality_modules(asset_manifest: &Value) -> Vec<Value> {
    asset_manifest
        .get("export_package")
        .and_then(|package| package.get("debug"))
        .and_then(|debug| debug.get("data_quality_modules"))
        .or_else(|| {
            asset_manifest
                .get("chart_runtime")
                .and_then(|runtime| runtime.get("modules"))
        })
        .or_else(|| asset_manifest.get("data_quality_modules"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(static_page_data_quality_module_payload)
                .collect()
        })
        .unwrap_or_default()
}

fn static_page_data_quality_module_payload(module: &Value) -> Value {
    json!({
        "moduleId": module.get("moduleId").or_else(|| module.get("module_id")).and_then(Value::as_str).unwrap_or_default(),
        "title": module.get("title").and_then(Value::as_str).unwrap_or("未命名模块"),
        "dataQuality": module.get("dataQuality").or_else(|| module.get("data_quality")).and_then(Value::as_str).unwrap_or("unknown"),
        "dataQualityStatus": module.get("dataQualityStatus").or_else(|| module.get("data_quality_status")).and_then(Value::as_str).unwrap_or("unknown"),
        "dataQualityReason": module.get("dataQualityReason").or_else(|| module.get("data_quality_reason")).and_then(Value::as_str).unwrap_or_default(),
        "recommendedAction": module.get("recommendedAction").or_else(|| module.get("recommended_action")).and_then(Value::as_str).unwrap_or_default(),
        "chartRuntime": module.get("chartRuntime").or_else(|| module.get("chart_runtime")).and_then(Value::as_str).unwrap_or("deterministic"),
        "fallback": module.get("fallback").and_then(Value::as_bool).unwrap_or(false),
        "sampleDataRows": module.get("sampleDataRows").or_else(|| module.get("sample_data_rows")).and_then(Value::as_i64).unwrap_or(0),
        "echartsHydratable": module.get("echartsHydratable").or_else(|| module.get("echarts_hydratable")).and_then(Value::as_bool).unwrap_or(false),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{AssistantRunId, StaticPageDraftId, StaticPageDraftStatus, TenantId};

    fn rendered_draft(asset_manifest: Value) -> StaticPageDraft {
        let now = Utc::now();
        StaticPageDraft {
            id: StaticPageDraftId::new(),
            tenant_id: TenantId::new(),
            assistant_run_id: AssistantRunId::new(),
            owner_user_id: None,
            title: "客户经营页".to_string(),
            status: StaticPageDraftStatus::Rendered,
            selected_scope: json!({}),
            visibility_snapshot: json!({}),
            source_refs: json!({}),
            draft_payload: json!({
                "finalPage": {
                    "status": "rendered",
                    "assetManifest": asset_manifest
                }
            }),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn data_quality_artifact_uses_chart_runtime_summary_and_modules() {
        let artifact = static_page_data_quality_artifact_from_draft(rendered_draft(json!({
            "chart_runtime": {
                "dataQualitySummary": {
                    "confirmedModules": 1,
                    "partialModules": 1,
                    "missingModules": 0,
                    "attentionModules": 1
                },
                "modules": [{
                    "moduleId": "trend",
                    "title": "趋势",
                    "dataQuality": "module_data",
                    "dataQualityStatus": "partial",
                    "chartRuntime": "echarts",
                    "fallback": true,
                    "sampleDataRows": 3,
                    "recommendedAction": "补齐完整月份数据"
                }]
            }
        })))
        .expect("rendered manifest should create data quality artifact");

        assert_eq!(
            artifact.template_id,
            contracts::HtmlArtifactTemplateIdView::StaticPageDataQualityReport
        );
        assert_eq!(
            artifact.interaction_mode,
            HtmlArtifactInteractionModeView::ReadOnly
        );
        assert_eq!(artifact.payload["summary"]["partialModules"], json!(1));
        assert_eq!(
            artifact.payload["modules"][0]["chartRuntime"],
            json!("echarts")
        );
        assert_eq!(artifact.payload["modules"][0]["fallback"], json!(true));
    }

    #[test]
    fn data_quality_artifact_uses_export_package_debug_precedence() {
        let artifact = static_page_data_quality_artifact_from_draft(rendered_draft(json!({
            "data_quality_summary": {"missingModules": 9},
            "export_package": {
                "debug": {
                    "data_quality_summary": {"missingModules": 1},
                    "data_quality_modules": [{
                        "module_id": "risk",
                        "title": "风险",
                        "data_quality": "schema_context",
                        "data_quality_status": "confirmed",
                        "chart_runtime": "svg",
                        "echarts_hydratable": false
                    }]
                }
            }
        })))
        .expect("debug summary should create artifact");

        assert_eq!(artifact.payload["summary"]["missingModules"], json!(1));
        assert_eq!(artifact.payload["modules"][0]["moduleId"], json!("risk"));
        assert_eq!(artifact.payload["modules"][0]["chartRuntime"], json!("svg"));
    }

    #[test]
    fn data_quality_artifact_skips_empty_zero_summary_without_modules() {
        let artifact = static_page_data_quality_artifact_from_draft(rendered_draft(json!({
            "data_quality_summary": {
                "confirmedModules": 0,
                "partialModules": 0,
                "missingModules": 0,
                "attentionModules": 0
            },
            "data_quality_modules": []
        })));

        assert!(artifact.is_none());
    }
}
