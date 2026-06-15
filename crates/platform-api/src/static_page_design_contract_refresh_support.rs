use serde_json::Value;

use crate::{
    build_static_page_data_snapshot, build_static_page_preview_contract,
    build_static_page_render_spec, build_static_page_visual_spec, ensure_json_object,
    mark_static_page_payload_preview_stale, set_payload_string, set_payload_value,
    static_page_payload_mobile_order, static_page_payload_modules, static_page_payload_string,
    static_page_payload_value, static_page_preview_contract_status,
};

pub(crate) fn refresh_static_page_payload_design_contract(payload: &mut Value) {
    ensure_json_object(payload);
    let style_direction =
        static_page_payload_string(payload, &["styleDirection", "style_direction"])
            .unwrap_or_else(|| "client-delivery".to_string());
    let modules = static_page_payload_modules(payload);
    let visual_spec = build_static_page_visual_spec(&style_direction);
    let render_spec = static_page_payload_value(payload, &["renderSpec", "render_spec"])
        .unwrap_or_else(build_static_page_render_spec);
    let mobile_order = static_page_payload_mobile_order(payload, &modules);
    let selected_scope = payload
        .get("assistant_context")
        .and_then(|context| context.get("selected_scope"))
        .cloned()
        .or_else(|| payload.get("selected_scope").cloned())
        .unwrap_or(Value::Null);
    let data_snapshot = build_static_page_data_snapshot(payload, &selected_scope);
    let previous_contract =
        static_page_payload_value(payload, &["previewContract", "preview_contract"]);
    let preview_contract = build_static_page_preview_contract(
        &style_direction,
        &modules,
        &render_spec,
        &mobile_order,
        previous_contract,
    );
    if static_page_preview_contract_status(&preview_contract) == Some("stale") {
        mark_static_page_payload_preview_stale(payload);
    }

    set_payload_string(payload, "styleDirection", &style_direction);
    set_payload_string(payload, "style_direction", &style_direction);
    set_payload_value(payload, "visualSpec", visual_spec.clone());
    set_payload_value(payload, "visual_spec", visual_spec);
    set_payload_value(payload, "renderSpec", render_spec.clone());
    set_payload_value(payload, "render_spec", render_spec);
    set_payload_value(payload, "mobileOrder", mobile_order.clone());
    set_payload_value(payload, "mobile_order", mobile_order);
    set_payload_value(payload, "dataSnapshot", data_snapshot.clone());
    set_payload_value(payload, "data_snapshot", data_snapshot);
    set_payload_value(payload, "previewContract", preview_contract.clone());
    set_payload_value(payload, "preview_contract", preview_contract);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn refresh_design_contract_writes_dual_contract_fields() {
        let mut payload = json!({
            "modules": [
                { "id": "hero" },
                { "id": "risk" }
            ],
            "mobileOrder": ["risk"]
        });

        refresh_static_page_payload_design_contract(&mut payload);

        assert_eq!(payload["styleDirection"], json!("client-delivery"));
        assert_eq!(payload["style_direction"], json!("client-delivery"));
        assert_eq!(
            payload["visualSpec"]["styleDirection"],
            json!("client-delivery")
        );
        assert_eq!(
            payload["visual_spec"]["styleDirection"],
            json!("client-delivery")
        );
        assert_eq!(
            payload["renderSpec"]["renderer"],
            json!("static-page-renderer-v1")
        );
        assert_eq!(
            payload["render_spec"]["renderer"],
            json!("static-page-renderer-v1")
        );
        assert_eq!(payload["mobileOrder"], json!(["risk", "hero"]));
        assert_eq!(payload["mobile_order"], json!(["risk", "hero"]));
        assert_eq!(
            payload["previewContract"]["kind"],
            json!("static-page-preview-contract")
        );
        assert_eq!(
            payload["preview_contract"]["kind"],
            json!("static-page-preview-contract")
        );
        assert!(payload.get("dataSnapshot").is_some());
        assert!(payload.get("data_snapshot").is_some());
    }

    #[test]
    fn refresh_design_contract_preserves_supplied_render_spec() {
        let mut payload = json!({
            "style_direction": "data-command",
            "render_spec": {
                "renderer": "custom-renderer",
                "componentModel": "custom-model"
            }
        });

        refresh_static_page_payload_design_contract(&mut payload);

        assert_eq!(payload["styleDirection"], json!("data-command"));
        assert_eq!(payload["renderSpec"]["renderer"], json!("custom-renderer"));
        assert_eq!(
            payload["render_spec"]["componentModel"],
            json!("custom-model")
        );
    }

    #[test]
    fn refresh_design_contract_marks_changed_preview_payload_stale() {
        let mut payload = json!({
            "modules": [{ "id": "hero" }],
            "previewImage": { "assetKey": "old-preview.png" },
            "finalPage": { "url": "old.html" },
            "imageJob": { "status": "preview_ready" },
            "previewContract": {
                "status": "confirmed",
                "draftFingerprint": "design-old",
                "assetKey": "old-preview.png",
                "confirmedAt": "2026-06-15T00:00:00Z"
            }
        });

        refresh_static_page_payload_design_contract(&mut payload);

        assert_eq!(payload["status"], json!("planning"));
        assert_eq!(payload["imageJob"]["status"], json!("stale"));
        assert!(payload.get("previewImage").is_none());
        assert!(payload.get("finalPage").is_none());
        assert_eq!(payload["previewContract"]["status"], json!("stale"));
        assert_eq!(payload["previewContract"]["assetKey"], Value::Null);
        assert_eq!(payload["previewContract"]["confirmedAt"], Value::Null);
    }
}
