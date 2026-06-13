use chrono::Utc;
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

pub(crate) fn build_static_page_preview_contract(
    style_direction: &str,
    modules: &Value,
    render_spec: &Value,
    mobile_order: &Value,
    patch: Option<Value>,
) -> Value {
    let contract_source = json!({
        "style_direction": style_direction,
        "modules": modules,
        "render_spec": render_spec,
        "mobile_order": mobile_order,
    });
    let next_fingerprint = static_page_design_fingerprint(&contract_source);
    let should_mark_stale = patch.as_ref().is_some_and(|previous_contract| {
        static_page_preview_contract_is_stale(previous_contract, &next_fingerprint)
    });
    let mut contract = json!({
        "version": 1,
        "kind": "static-page-preview-contract",
        "status": "not_requested",
        "draftFingerprint": next_fingerprint,
        "imageJobId": Value::Null,
        "assetKey": Value::Null,
        "confirmedAt": Value::Null,
        "renderExpectation": "final HTML/CSS/SVG should reproduce the generated preview without baking editable text or charts into the image",
    });
    if let Some(patch) = patch {
        merge_json_value(&mut contract, &patch);
        set_payload_string(
            &mut contract,
            "draftFingerprint",
            &static_page_design_fingerprint(&contract_source),
        );
    }
    if should_mark_stale {
        mark_static_page_preview_contract_stale(
            &mut contract,
            "draft design changed after the preview was requested or confirmed",
        );
    }
    contract
}

fn static_page_preview_contract_is_stale(
    previous_contract: &Value,
    next_fingerprint: &str,
) -> bool {
    let Some(previous_fingerprint) = previous_contract
        .get("draftFingerprint")
        .or_else(|| previous_contract.get("draft_fingerprint"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return false;
    };
    if previous_fingerprint == next_fingerprint {
        return false;
    }
    matches!(
        previous_contract
            .get("status")
            .and_then(Value::as_str)
            .map(str::trim),
        Some("queued" | "running" | "preview_ready" | "confirmed" | "rendering" | "rendered")
    )
}

pub(crate) fn mark_static_page_preview_contract_stale(contract: &mut Value, reason: &str) {
    set_payload_string(contract, "status", "stale");
    set_payload_value(contract, "imageJobId", Value::Null);
    set_payload_value(contract, "assetKey", Value::Null);
    set_payload_value(contract, "confirmedAt", Value::Null);
    set_payload_string(contract, "staleReason", reason);
    set_payload_string(contract, "staleAt", &Utc::now().to_rfc3339());
}

pub(crate) fn static_page_preview_contract_status(contract: &Value) -> Option<&str> {
    contract
        .get("status")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub(crate) fn static_page_design_fingerprint(value: &Value) -> String {
    let serialized = serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string());
    let mut hash: u32 = 0;
    for byte in serialized.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u32);
    }
    format!("design-{hash:08x}")
}

pub(crate) fn static_page_payload_modules(payload: &Value) -> Value {
    payload
        .get("modules")
        .and_then(Value::as_array)
        .cloned()
        .map(Value::Array)
        .unwrap_or_else(|| Value::Array(Vec::new()))
}

pub(crate) fn static_page_payload_mobile_order(payload: &Value, modules: &Value) -> Value {
    let module_ids = modules
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|module| {
                    module
                        .get("id")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(ToOwned::to_owned)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let module_id_set = module_ids.iter().cloned().collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::<String>::new();
    let mut ordered = Vec::<Value>::new();
    if let Some(requested) = static_page_payload_value(payload, &["mobileOrder", "mobile_order"])
        .and_then(|value| value.as_array().cloned())
    {
        for item in requested {
            let Some(id) = item
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
            else {
                continue;
            };
            if module_id_set.contains(&id) && seen.insert(id.clone()) {
                ordered.push(json!(id));
            }
        }
    }
    for id in module_ids {
        if seen.insert(id.clone()) {
            ordered.push(json!(id));
        }
    }
    Value::Array(ordered)
}

pub(crate) fn static_page_payload_string(payload: &Value, keys: &[&str]) -> Option<String> {
    static_page_value_string(payload, keys)
}

pub(crate) fn static_page_value_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    })
}

pub(crate) fn static_page_value_i64(value: &Value, keys: &[&str]) -> Option<i64> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_i64))
}

pub(crate) fn static_page_payload_value(payload: &Value, keys: &[&str]) -> Option<Value> {
    keys.iter().find_map(|key| payload.get(*key).cloned())
}

pub(crate) fn merge_json_value(target: &mut Value, patch: &Value) {
    match (target, patch) {
        (Value::Object(target_object), Value::Object(patch_object)) => {
            for (key, value) in patch_object {
                match target_object.get_mut(key) {
                    Some(existing) => merge_json_value(existing, value),
                    None => {
                        target_object.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        (target, patch) => {
            *target = patch.clone();
        }
    }
}

pub(crate) fn ensure_json_object(value: &mut Value) {
    if !value.is_object() {
        *value = Value::Object(Map::new());
    }
}

pub(crate) fn set_payload_string(payload: &mut Value, key: &str, value: &str) {
    set_payload_value(payload, key, json!(value));
}

pub(crate) fn set_payload_value(payload: &mut Value, key: &str, value: Value) {
    ensure_json_object(payload);
    if let Some(object) = payload.as_object_mut() {
        object.insert(key.to_string(), value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_contract_marks_active_previous_contract_stale_when_design_changes() {
        let modules = json!([{
            "id": "hero",
            "title": "经营总览"
        }]);
        let render_spec = json!({
            "componentModel": "data-bound-static-html"
        });
        let mobile_order = json!(["hero"]);
        let previous = build_static_page_preview_contract(
            "dark-mobile",
            &modules,
            &render_spec,
            &mobile_order,
            Some(json!({
                "status": "confirmed",
                "draftFingerprint": "design-old",
                "assetKey": "static-page-previews/old.png",
                "confirmedAt": "2026-06-13T00:00:00Z"
            })),
        );

        assert_eq!(previous["status"], json!("stale"));
        assert_eq!(previous["imageJobId"], Value::Null);
        assert_eq!(previous["assetKey"], Value::Null);
        assert_eq!(previous["confirmedAt"], Value::Null);
        assert_eq!(
            previous["staleReason"],
            json!("draft design changed after the preview was requested or confirmed")
        );
        assert_eq!(
            previous["draftFingerprint"],
            static_page_design_fingerprint(&json!({
                "style_direction": "dark-mobile",
                "modules": modules,
                "render_spec": render_spec,
                "mobile_order": mobile_order,
            }))
        );
    }

    #[test]
    fn mobile_order_keeps_valid_requested_ids_then_appends_missing_modules() {
        let payload = json!({
            "mobileOrder": ["risk", "missing", "hero", "risk"],
            "modules": [
                { "id": "hero" },
                { "id": "risk" },
                { "id": "trend" }
            ]
        });
        let modules = static_page_payload_modules(&payload);

        assert_eq!(
            static_page_payload_mobile_order(&payload, &modules),
            json!(["risk", "hero", "trend"])
        );
    }

    #[test]
    fn merge_json_value_recursively_merges_objects_and_replaces_scalars() {
        let mut target = json!({
            "module": {
                "title": "旧标题",
                "layout": { "x": 0, "y": 0 },
                "items": [1, 2]
            }
        });
        merge_json_value(
            &mut target,
            &json!({
                "module": {
                    "layout": { "w": 6 },
                    "items": [3],
                    "content": "新内容"
                }
            }),
        );

        assert_eq!(
            target,
            json!({
                "module": {
                    "title": "旧标题",
                    "layout": { "x": 0, "y": 0, "w": 6 },
                    "items": [3],
                    "content": "新内容"
                }
            })
        );
    }
}
