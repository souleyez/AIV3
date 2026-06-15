use serde_json::Value;

use crate::static_page_payload_support::{ensure_json_object, merge_json_value};

pub(crate) fn merge_static_page_module(payload: &mut Value, module_id: &str, patch: &Value) {
    let Some(modules) = payload
        .as_object_mut()
        .and_then(|object| object.get_mut("modules"))
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    let Some(module) = modules.iter_mut().find(|module| {
        module
            .as_object()
            .and_then(|object| object.get("id"))
            .and_then(Value::as_str)
            == Some(module_id)
    }) else {
        return;
    };
    merge_json_value(module, patch);
}

pub(crate) fn push_static_page_module(payload: &mut Value, module: Value) {
    ensure_json_object(payload);
    let Some(object) = payload.as_object_mut() else {
        return;
    };
    let modules = object
        .entry("modules".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    if !modules.is_array() {
        *modules = Value::Array(Vec::new());
    }
    if let Some(items) = modules.as_array_mut() {
        items.push(module);
    }
}

pub(crate) fn remove_static_page_module(payload: &mut Value, module_id: &str) {
    let Some(modules) = payload
        .as_object_mut()
        .and_then(|object| object.get_mut("modules"))
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    modules.retain(|module| {
        module
            .as_object()
            .and_then(|object| object.get("id"))
            .and_then(Value::as_str)
            != Some(module_id)
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn merge_module_recursively_merges_matching_module_only() {
        let mut payload = json!({
            "modules": [
                {
                    "id": "hero",
                    "title": "Hero",
                    "layout": {"x": 0, "w": 4}
                },
                {
                    "id": "risk",
                    "title": "Risk"
                }
            ]
        });

        merge_static_page_module(
            &mut payload,
            "hero",
            &json!({
                "title": "Updated",
                "layout": {"w": 8}
            }),
        );

        assert_eq!(payload["modules"][0]["title"], json!("Updated"));
        assert_eq!(payload["modules"][0]["layout"], json!({"x": 0, "w": 8}));
        assert_eq!(payload["modules"][1]["title"], json!("Risk"));
    }

    #[test]
    fn merge_module_ignores_missing_modules_and_missing_id() {
        let mut payload = json!({"modules": [{"id": "hero", "title": "Hero"}]});

        merge_static_page_module(&mut payload, "missing", &json!({"title": "Noop"}));

        assert_eq!(payload["modules"][0]["title"], json!("Hero"));

        let mut non_object_payload = json!(null);
        merge_static_page_module(&mut non_object_payload, "hero", &json!({"title": "Noop"}));
        assert_eq!(non_object_payload, Value::Null);
    }

    #[test]
    fn push_module_creates_or_replaces_modules_array() {
        let mut payload = json!({"modules": "invalid"});

        push_static_page_module(&mut payload, json!({"id": "hero"}));

        assert_eq!(payload["modules"], json!([{"id": "hero"}]));

        let mut empty_payload = Value::Null;
        push_static_page_module(&mut empty_payload, json!({"id": "risk"}));
        assert_eq!(empty_payload["modules"], json!([{"id": "risk"}]));
    }

    #[test]
    fn remove_module_keeps_non_matching_and_idless_modules() {
        let mut payload = json!({
            "modules": [
                {"id": "hero"},
                {"title": "No id"},
                {"id": "risk"}
            ]
        });

        remove_static_page_module(&mut payload, "hero");

        assert_eq!(
            payload["modules"],
            json!([
                {"title": "No id"},
                {"id": "risk"}
            ])
        );
    }

    #[test]
    fn remove_module_ignores_non_array_modules() {
        let mut payload = json!({"modules": "invalid"});

        remove_static_page_module(&mut payload, "hero");

        assert_eq!(payload["modules"], json!("invalid"));
    }
}
