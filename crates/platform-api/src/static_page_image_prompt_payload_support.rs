use domain_model::StaticPageDraft;
use serde_json::{json, Value};

use crate::build_static_page_data_snapshot;
use crate::static_page_payload_support::{
    build_static_page_preview_contract, static_page_payload_mobile_order,
    static_page_payload_modules, static_page_payload_string, static_page_payload_value,
};
use crate::static_page_visual_render_spec_support::{
    build_static_page_render_spec, build_static_page_visual_spec,
};

pub(crate) fn build_static_page_image_prompt_payload(
    draft: &StaticPageDraft,
    prompt: Option<&str>,
) -> Value {
    let payload = &draft.draft_payload;
    let style_direction =
        static_page_payload_string(payload, &["styleDirection", "style_direction"])
            .unwrap_or_else(|| "client-delivery".to_string());
    let modules = static_page_payload_modules(payload);
    let visual_spec = static_page_payload_value(payload, &["visualSpec", "visual_spec"])
        .unwrap_or_else(|| build_static_page_visual_spec(&style_direction));
    let render_spec = static_page_payload_value(payload, &["renderSpec", "render_spec"])
        .unwrap_or_else(build_static_page_render_spec);
    let mobile_order = static_page_payload_mobile_order(payload, &modules);
    let data_snapshot = static_page_payload_value(payload, &["dataSnapshot", "data_snapshot"])
        .unwrap_or_else(|| build_static_page_data_snapshot(payload, &draft.selected_scope));
    let template_reference = payload
        .get("templateReference")
        .or_else(|| payload.get("template_reference"))
        .or_else(|| payload.pointer("/source/templateReference"))
        .cloned()
        .unwrap_or(Value::Null);
    let template_references = payload
        .get("designReferences")
        .or_else(|| payload.get("design_references"))
        .or_else(|| payload.pointer("/source/templateReferences"))
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new()));
    let template_adaptation = payload
        .get("templateAdaptation")
        .or_else(|| payload.get("template_adaptation"))
        .or_else(|| payload.pointer("/source/templateAdaptation"))
        .cloned()
        .unwrap_or(Value::Null);
    let preview_contract =
        static_page_payload_value(payload, &["previewContract", "preview_contract"])
            .unwrap_or_else(|| {
                build_static_page_preview_contract(
                    &style_direction,
                    &modules,
                    &render_spec,
                    &mobile_order,
                    None,
                )
            });
    json!({
        "draft_id": draft.id,
        "assistant_run_id": draft.assistant_run_id,
        "title": draft.title,
        "prompt": prompt.map(str::trim).filter(|value| !value.is_empty()),
        "promptText": prompt.map(str::trim).filter(|value| !value.is_empty()),
        "prompt_text": prompt.map(str::trim).filter(|value| !value.is_empty()),
        "style_direction": style_direction,
        "visual_spec": visual_spec,
        "render_spec": render_spec,
        "data_snapshot": data_snapshot,
        "template_reference": template_reference,
        "template_references": template_references,
        "template_adaptation": template_adaptation,
        "template_reuse_contract": {
            "policy": "reuse_generated_template_when_available",
            "data_rule": "template controls visual structure only; bind current selected_scope data",
            "image2_rule": "new Image2 is only required when no usable template exists or the user explicitly asks to change style",
        },
        "preview_contract": preview_contract,
        "design_contract": {
            "contract_source": "StaticPageDraft",
            "visual_source": "effect image is a preview contract, not final source code",
            "final_source": "generated effect image visual blueprint + real data_snapshot",
            "editable_core": "DOM text + SVG/chart components + safe ECharts JSON options",
        },
        "image_first_contract": {
            "role": "requirements_to_image2_then_image_to_html",
            "rule": "Generate the effect image from requirements first; then infer the visual layout from the image and bind real data_snapshot into HTML.",
            "fake_data_allowed": false
        },
        "selected_scope": draft.selected_scope,
        "visibility_snapshot": draft.visibility_snapshot,
        "modules": modules,
        "mobile_order": mobile_order,
        "data_bindings": payload.get("data_bindings").cloned().unwrap_or_else(|| json!([])),
        "queue_copy": "资源正在排队，可以联系商务开通高级用户跳过等待。",
    })
}

pub(crate) fn static_page_image_prompt_payload_is_prompt_only(payload: &Value) -> bool {
    payload
        .get("promptOnly")
        .or_else(|| payload.get("prompt_only"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{AssistantRunId, StaticPageDraftId, StaticPageDraftStatus, TenantId};

    fn draft_with_payload(payload: Value) -> StaticPageDraft {
        StaticPageDraft {
            id: StaticPageDraftId::new(),
            tenant_id: TenantId::new(),
            assistant_run_id: AssistantRunId::new(),
            owner_user_id: None,
            title: "经营月报".to_string(),
            status: StaticPageDraftStatus::Draft,
            selected_scope: json!({
                "datasets": ["dataset-a"],
                "documents": ["doc-a"]
            }),
            visibility_snapshot: json!({
                "dataset_count": 1,
                "document_count": 1
            }),
            source_refs: json!([]),
            draft_payload: payload,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn image_prompt_payload_prefers_existing_contract_fields_and_trims_prompt() {
        let draft = draft_with_payload(json!({
            "styleDirection": "dark-mobile",
            "visualSpec": {"tone": "dark"},
            "renderSpec": {"runtime": "safe-echarts"},
            "dataSnapshot": {"source": "provided"},
            "templateReference": {"id": "template-1"},
            "designReferences": [{"id": "ref-1"}],
            "templateAdaptation": {"mode": "reuse"},
            "previewContract": {"status": "confirmed"},
            "modules": [
                {"id": "trend"},
                {"id": "risk"}
            ],
            "mobileOrder": ["risk", "trend"],
            "data_bindings": [{"moduleId": "trend"}]
        }));

        let payload = build_static_page_image_prompt_payload(&draft, Some("  做暗色移动端  "));

        assert_eq!(payload["title"], json!("经营月报"));
        assert_eq!(payload["prompt"], json!("做暗色移动端"));
        assert_eq!(payload["promptText"], json!("做暗色移动端"));
        assert_eq!(payload["prompt_text"], json!("做暗色移动端"));
        assert_eq!(payload["style_direction"], json!("dark-mobile"));
        assert_eq!(payload["visual_spec"], json!({"tone": "dark"}));
        assert_eq!(payload["render_spec"], json!({"runtime": "safe-echarts"}));
        assert_eq!(payload["data_snapshot"], json!({"source": "provided"}));
        assert_eq!(payload["template_reference"], json!({"id": "template-1"}));
        assert_eq!(payload["template_references"], json!([{"id": "ref-1"}]));
        assert_eq!(payload["template_adaptation"], json!({"mode": "reuse"}));
        assert_eq!(payload["preview_contract"], json!({"status": "confirmed"}));
        assert_eq!(payload["mobile_order"], json!(["risk", "trend"]));
        assert_eq!(payload["data_bindings"], json!([{"moduleId": "trend"}]));
        assert_eq!(
            payload["image_first_contract"]["fake_data_allowed"],
            json!(false)
        );
    }

    #[test]
    fn image_prompt_payload_falls_back_to_source_template_fields_and_generated_contracts() {
        let draft = draft_with_payload(json!({
            "source": {
                "templateReference": {"id": "source-template"},
                "templateReferences": [{"id": "source-ref"}],
                "templateAdaptation": {"source": "source"}
            },
            "modules": [
                {"id": "hero"}
            ],
            "mobileOrder": ["missing", "hero"],
            "assistant_context": {
                "evidence_state": {
                    "supplied_items": []
                }
            }
        }));

        let payload = build_static_page_image_prompt_payload(&draft, Some("   "));

        assert_eq!(payload["prompt"], Value::Null);
        assert_eq!(payload["promptText"], Value::Null);
        assert_eq!(payload["style_direction"], json!("client-delivery"));
        assert_eq!(
            payload["template_reference"],
            json!({"id": "source-template"})
        );
        assert_eq!(
            payload["template_references"],
            json!([{"id": "source-ref"}])
        );
        assert_eq!(payload["template_adaptation"], json!({"source": "source"}));
        assert_eq!(payload["modules"], json!([{"id": "hero"}]));
        assert_eq!(payload["mobile_order"], json!(["hero"]));
        assert_eq!(payload["data_bindings"], json!([]));
        assert_eq!(
            payload["preview_contract"]["kind"],
            json!("static-page-preview-contract")
        );
        assert_eq!(
            payload["data_snapshot"]["selected_scope"],
            json!({
                "datasets": ["dataset-a"],
                "documents": ["doc-a"]
            })
        );
    }

    #[test]
    fn prompt_only_flag_accepts_camel_or_snake_case_true_values() {
        assert!(static_page_image_prompt_payload_is_prompt_only(
            &json!({"promptOnly": true})
        ));
        assert!(static_page_image_prompt_payload_is_prompt_only(
            &json!({"prompt_only": true})
        ));
        assert!(!static_page_image_prompt_payload_is_prompt_only(
            &json!({"promptOnly": false, "prompt_only": true})
        ));
        assert!(!static_page_image_prompt_payload_is_prompt_only(&json!({})));
    }
}
