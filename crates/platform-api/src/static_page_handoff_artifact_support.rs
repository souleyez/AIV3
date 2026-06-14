use contracts::{self, HtmlArtifactInteractionModeView, HtmlArtifactManifestView};
use domain_model::StaticPageDraft;
use serde_json::{json, Value};

use crate::static_page_payload_support::{
    static_page_payload_modules, static_page_payload_string, static_page_payload_value,
    static_page_value_i64, static_page_value_string,
};

pub(crate) fn static_page_handoff_artifact_from_draft(
    draft: StaticPageDraft,
) -> HtmlArtifactManifestView {
    let draft_id = draft.id.to_string();
    let payload = &draft.draft_payload;
    let modules = static_page_payload_modules(payload)
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|module| {
            let data_binding = module
                .get("dataBinding")
                .or_else(|| module.get("data_binding"))
                .cloned()
                .unwrap_or(Value::Null);
            json!({
                "id": module.get("id").and_then(Value::as_str).unwrap_or_default(),
                "title": module.get("title").and_then(Value::as_str).unwrap_or_default(),
                "content": module.get("content").and_then(Value::as_str).unwrap_or_default(),
                "dataBinding": data_binding,
                "dataBindingLabel": data_binding
                    .get("label")
                    .or_else(|| data_binding.get("fieldPath"))
                    .or_else(|| data_binding.get("field"))
                    .and_then(Value::as_str)
                    .unwrap_or("待绑定"),
                "visualizationType": module
                    .get("visualization")
                    .and_then(|visualization| visualization.get("type"))
                    .or_else(|| module.get("visualizationType"))
                    .and_then(Value::as_str)
                    .unwrap_or("text"),
                "layout": module.get("layout").cloned().unwrap_or(Value::Null),
                "dataQuality": module
                    .get("dataQuality")
                    .or_else(|| module.get("dataQualityStatus"))
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            })
        })
        .collect::<Vec<_>>();

    HtmlArtifactManifestView {
        kind: "html_artifact".to_string(),
        version: 1,
        id: format!("html-static-page-handoff-{draft_id}"),
        title: format!("{} · 交接", draft.title),
        source_type: contracts::HtmlArtifactSourceTypeView::StaticPage,
        template_id: contracts::HtmlArtifactTemplateIdView::StaticPagePlanningHandoff,
        owner_scope: contracts::HtmlArtifactOwnerScopeView {
            scope_type: "static_page_draft".to_string(),
            id: draft_id.clone(),
        },
        data_refs: Vec::new(),
        provenance: contracts::HtmlArtifactProvenanceView {
            producer: "v3-platform-api".to_string(),
            reason: "static page planning handoff".to_string(),
            source_run_id: Some(draft.assistant_run_id.to_string()),
        },
        interaction_mode: HtmlArtifactInteractionModeView::ActionIntent,
        created_at: draft.updated_at,
        payload: json!({
            "objective": draft.title,
            "status": draft.status.as_str(),
            "styleDirection": static_page_payload_string(payload, &["styleDirection", "style_direction"]).unwrap_or_default(),
            "defaultAction": "apply_static_page_intent",
            "intentPlaceholder": "例如：把趋势模块标题改成月度增长趋势，风险模块缩小一点，图表改为折线图。",
            "modules": modules,
            "visualBridge": static_page_visual_bridge_payload(payload)
        }),
    }
}

fn static_page_visual_bridge_payload(payload: &Value) -> Value {
    let preview_contract =
        static_page_payload_value(payload, &["previewContract", "preview_contract"])
            .unwrap_or(Value::Null);
    let preview_image = static_page_payload_value(payload, &["previewImage", "preview_image"])
        .unwrap_or(Value::Null);
    let image_job =
        static_page_payload_value(payload, &["imageJob", "image_job"]).unwrap_or(Value::Null);
    let final_page =
        static_page_payload_value(payload, &["finalPage", "final_page"]).unwrap_or(Value::Null);
    let render_spec =
        static_page_payload_value(payload, &["renderSpec", "render_spec"]).unwrap_or(Value::Null);

    json!({
        "providerLane": "gpt-image-2-cloudflare-queue",
        "role": "effect_preview_reference_only",
        "rule": "可视化只锁定视觉方向和确认指纹；最终 HTML 由 Draft JSON、DataSnapshot、VisualSpec 和 renderer 生成。",
        "status": static_page_value_string(&preview_contract, &["status"])
            .or_else(|| static_page_value_string(&image_job, &["status"]))
            .unwrap_or_else(|| "not_requested".to_string()),
        "imageJobStatus": static_page_value_string(&image_job, &["status"])
            .unwrap_or_else(|| "not_requested".to_string()),
        "imageJobId": static_page_value_string(&image_job, &["id"])
            .or_else(|| static_page_value_string(&preview_contract, &["imageJobId", "image_job_id"]))
            .or_else(|| static_page_value_string(&preview_image, &["imageJobId", "image_job_id"]))
            .unwrap_or_default(),
        "queuePosition": static_page_value_i64(&image_job, &["queuePosition", "queue_position"]),
        "queueMessage": static_page_value_string(&image_job, &["queueMessage", "queue_message"])
            .unwrap_or_default(),
        "previewAssetKey": static_page_value_string(&preview_image, &["assetKey", "asset_key"])
            .or_else(|| static_page_value_string(&preview_contract, &["assetKey", "asset_key"]))
            .or_else(|| static_page_value_string(&image_job, &["previewAssetKey", "preview_asset_key"]))
            .unwrap_or_default(),
        "previousAssetKey": static_page_value_string(&preview_contract, &["previousAssetKey", "previous_asset_key"])
            .unwrap_or_default(),
        "draftFingerprint": static_page_value_string(&preview_contract, &["draftFingerprint", "draft_fingerprint"])
            .unwrap_or_default(),
        "confirmedAt": static_page_value_string(&preview_contract, &["confirmedAt", "confirmed_at"])
            .unwrap_or_default(),
        "staleReason": static_page_value_string(&preview_contract, &["staleReason", "stale_reason"])
            .unwrap_or_default(),
        "styleDirection": static_page_payload_string(payload, &["styleDirection", "style_direction"])
            .unwrap_or_default(),
        "renderModel": static_page_value_string(&render_spec, &["componentModel", "component_model"])
            .or_else(|| {
                final_page
                    .get("assetManifest")
                    .or_else(|| final_page.get("asset_manifest"))
                    .and_then(|asset_manifest| {
                        asset_manifest
                            .get("render_spec")
                            .or_else(|| asset_manifest.get("renderSpec"))
                    })
                    .and_then(|render_spec| {
                        static_page_value_string(render_spec, &["componentModel", "component_model"])
                    })
            })
            .unwrap_or_default(),
        "finalRenderStatus": static_page_value_string(&final_page, &["status"])
            .unwrap_or_else(|| "not_requested".to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{AssistantRunId, StaticPageDraftId, StaticPageDraftStatus, TenantId};

    fn planned_draft(payload: Value) -> StaticPageDraft {
        let now = Utc::now();
        StaticPageDraft {
            id: StaticPageDraftId::new(),
            tenant_id: TenantId::new(),
            assistant_run_id: AssistantRunId::new(),
            owner_user_id: None,
            title: "客户经营页".to_string(),
            status: StaticPageDraftStatus::Planned,
            selected_scope: json!({}),
            visibility_snapshot: json!({}),
            source_refs: json!({}),
            draft_payload: payload,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn handoff_artifact_exposes_action_intent_modules_and_visual_bridge() {
        let artifact = static_page_handoff_artifact_from_draft(planned_draft(json!({
            "styleDirection": "client-delivery",
            "imageJob": {
                "id": "image-job-1",
                "status": "preview_ready",
                "queuePosition": 2,
                "queueMessage": "资源排队中"
            },
            "previewImage": {
                "assetKey": "static-page-previews/image-job-1.json",
                "imageJobId": "image-job-1"
            },
            "previewContract": {
                "status": "stale",
                "imageJobId": "image-job-1",
                "assetKey": "static-page-previews/image-job-1.json",
                "previousAssetKey": "static-page-previews/old.json",
                "draftFingerprint": "design-abc123",
                "staleReason": "draft design changed after preview confirmation"
            },
            "renderSpec": {
                "componentModel": "dom-text-svg-chart"
            },
            "finalPage": {
                "status": "rendered"
            },
            "modules": [{
                "id": "hero",
                "title": "核心判断",
                "content": "增长稳定",
                "dataBinding": {"label": "订单收入"},
                "visualization": {"type": "kpi"},
                "layout": {"x": 0, "y": 0, "w": 6, "h": 3}
            }]
        })));

        assert_eq!(
            artifact.source_type,
            contracts::HtmlArtifactSourceTypeView::StaticPage
        );
        assert_eq!(
            artifact.template_id,
            contracts::HtmlArtifactTemplateIdView::StaticPagePlanningHandoff
        );
        assert_eq!(
            artifact.interaction_mode,
            HtmlArtifactInteractionModeView::ActionIntent
        );
        assert_eq!(artifact.owner_scope.scope_type, "static_page_draft");
        assert_eq!(artifact.payload["modules"][0]["title"], json!("核心判断"));
        assert_eq!(artifact.payload["visualBridge"]["status"], json!("stale"));
        assert_eq!(
            artifact.payload["visualBridge"]["imageJobId"],
            json!("image-job-1")
        );
        assert_eq!(
            artifact.payload["visualBridge"]["previewAssetKey"],
            json!("static-page-previews/image-job-1.json")
        );
        assert_eq!(
            artifact.payload["visualBridge"]["previousAssetKey"],
            json!("static-page-previews/old.json")
        );
        assert_eq!(
            artifact.payload["visualBridge"]["draftFingerprint"],
            json!("design-abc123")
        );
        assert_eq!(artifact.payload["visualBridge"]["queuePosition"], json!(2));
        assert_eq!(
            artifact.payload["visualBridge"]["renderModel"],
            json!("dom-text-svg-chart")
        );
        assert_eq!(
            artifact.payload["visualBridge"]["finalRenderStatus"],
            json!("rendered")
        );
    }

    #[test]
    fn handoff_artifact_falls_back_to_asset_manifest_render_spec() {
        let artifact = static_page_handoff_artifact_from_draft(planned_draft(json!({
            "final_page": {
                "status": "rendered",
                "asset_manifest": {
                    "render_spec": {
                        "component_model": "html-anything"
                    }
                }
            },
            "modules": [{
                "id": "risk",
                "title": "风险",
                "data_binding": {"fieldPath": "risk.items"},
                "visualizationType": "table",
                "dataQualityStatus": "partial"
            }]
        })));

        assert_eq!(
            artifact.payload["modules"][0]["dataBindingLabel"],
            json!("risk.items")
        );
        assert_eq!(
            artifact.payload["modules"][0]["visualizationType"],
            json!("table")
        );
        assert_eq!(
            artifact.payload["modules"][0]["dataQuality"],
            json!("partial")
        );
        assert_eq!(
            artifact.payload["visualBridge"]["status"],
            json!("not_requested")
        );
        assert_eq!(
            artifact.payload["visualBridge"]["renderModel"],
            json!("html-anything")
        );
    }
}
