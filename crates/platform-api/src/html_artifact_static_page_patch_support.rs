use serde_json::{json, Map, Value};

use crate::{html_artifact_patch_operations, validate_static_page_operations, ApiError};

pub(crate) fn html_artifact_static_page_operations_from_patch(
    draft_payload: &Value,
    payload: &Value,
) -> std::result::Result<Vec<Value>, ApiError> {
    let operations = html_artifact_patch_operations(payload)?;
    let mut translated = Vec::new();
    for operation in operations {
        if let Some(translated_operation) =
            static_page_operation_from_html_patch_operation(draft_payload, operation)?
        {
            translated.push(translated_operation);
        }
    }
    Ok(translated)
}

pub(crate) fn static_page_operations_from_html_artifact_patch(
    draft_payload: &Value,
    payload: &Value,
) -> std::result::Result<Vec<Value>, ApiError> {
    validate_static_page_operations(html_artifact_static_page_operations_from_patch(
        draft_payload,
        payload,
    )?)
}

fn static_page_operation_from_html_patch_operation(
    draft_payload: &Value,
    operation: &Value,
) -> std::result::Result<Option<Value>, ApiError> {
    let object = operation.as_object().ok_or_else(|| {
        ApiError::bad_request(
            "html_artifact_invalid_patch",
            "each patch operation must be an object".to_string(),
        )
    })?;
    let op = object
        .get("op")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    if !matches!(op, "add" | "replace" | "remove") {
        return Err(ApiError::bad_request(
            "html_artifact_patch_operation_unsupported",
            format!("{op} cannot be executed against static page drafts"),
        ));
    }
    let path = object
        .get("path")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    let tokens = decode_html_artifact_json_pointer(path)?;
    let value = if op == "remove" {
        Value::Null
    } else {
        object.get("value").cloned().ok_or_else(|| {
            ApiError::bad_request(
                "html_artifact_invalid_patch",
                "add/replace patch operations require value".to_string(),
            )
        })?
    };

    match tokens.as_slice() {
        [field] if field == "styleDirection" || field == "style_direction" => {
            html_patch_string_value(&value, "styleDirection")?;
            Ok(Some(json!({
                "type": "change_style_direction",
                "styleDirection": value,
            })))
        }
        [field] if field == "mobileOrder" || field == "mobile_order" => {
            if !value.is_array() {
                return Err(ApiError::bad_request(
                    "html_artifact_patch_value_invalid",
                    "mobileOrder must be an array".to_string(),
                ));
            }
            Ok(Some(json!({
                "type": "reorder_modules",
                "order": value,
            })))
        }
        [modules, module_selector, field @ ..] if modules == "modules" => {
            let module_id =
                static_page_module_id_from_patch_selector(draft_payload, module_selector)?;
            let operation = static_page_module_operation_from_html_patch(
                draft_payload,
                &module_id,
                field,
                value,
            )?;
            Ok(Some(operation))
        }
        _ => Err(ApiError::bad_request(
            "html_artifact_patch_target_unsupported",
            format!("{path} is not a supported static page patch path"),
        )),
    }
}

fn decode_html_artifact_json_pointer(path: &str) -> std::result::Result<Vec<String>, ApiError> {
    if !path.starts_with('/') {
        return Err(ApiError::bad_request(
            "html_artifact_invalid_patch",
            "patch operation path must be a JSON pointer".to_string(),
        ));
    }
    Ok(path
        .split('/')
        .skip(1)
        .map(|token| token.replace("~1", "/").replace("~0", "~"))
        .collect())
}

fn static_page_module_id_from_patch_selector(
    draft_payload: &Value,
    selector: &str,
) -> std::result::Result<String, ApiError> {
    let modules = draft_payload
        .get("modules")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            ApiError::bad_request(
                "html_artifact_patch_target_missing",
                "static page draft has no modules".to_string(),
            )
        })?;
    if let Ok(index) = selector.parse::<usize>() {
        return modules
            .get(index)
            .and_then(|module| module.get("id"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .ok_or_else(|| {
                ApiError::bad_request(
                    "html_artifact_patch_target_missing",
                    format!("module index {index} was not found"),
                )
            });
    }
    if modules.iter().any(|module| {
        module
            .get("id")
            .and_then(Value::as_str)
            .is_some_and(|id| id == selector)
    }) {
        return Ok(selector.to_string());
    }
    Err(ApiError::bad_request(
        "html_artifact_patch_target_missing",
        format!("module {selector} was not found"),
    ))
}

fn static_page_module_operation_from_html_patch(
    draft_payload: &Value,
    module_id: &str,
    field: &[String],
    value: Value,
) -> std::result::Result<Value, ApiError> {
    let Some(root) = field.first().map(String::as_str) else {
        return Err(ApiError::bad_request(
            "html_artifact_patch_target_unsupported",
            "module patch path must include a field".to_string(),
        ));
    };
    let patch = match root {
        "title" | "content" | "dataLabel" if field.len() == 1 => json!({ root: value }),
        "data_binding" if field.len() == 1 => json!({ "dataBinding": value }),
        "dataBinding" if field.len() == 1 => json!({ "dataBinding": value }),
        "dataBinding" | "data_binding" => {
            if field.len() != 2 {
                return Err(ApiError::bad_request(
                    "html_artifact_patch_target_unsupported",
                    "dataBinding patch paths may only target one field".to_string(),
                ));
            }
            let key = normalize_static_page_nested_patch_key(root, &field[1])?;
            let object = merge_static_page_current_module_object_field(
                draft_payload,
                module_id,
                "dataBinding",
                &key,
                value,
            );
            json!({ "dataBinding": object })
        }
        "visualizationType" if field.len() == 1 => {
            html_patch_string_value(&value, "visualizationType")?;
            json!({ "visualization": { "type": value } })
        }
        "visualization" if field.len() == 1 => json!({ "visualization": value }),
        "visualization" => {
            if field.len() != 2 {
                return Err(ApiError::bad_request(
                    "html_artifact_patch_target_unsupported",
                    "visualization patch paths may only target one field".to_string(),
                ));
            }
            let key = normalize_static_page_nested_patch_key(root, &field[1])?;
            let object = merge_static_page_current_module_object_field(
                draft_payload,
                module_id,
                "visualization",
                &key,
                value,
            );
            json!({ "visualization": object })
        }
        "chartRuntime" if field.len() == 1 => json!({ "chartRuntime": value }),
        "chartOptions" if field.len() == 1 => json!({ "chartOptions": value }),
        "layout" if field.len() == 1 => {
            validate_html_artifact_layout_patch(&value)?;
            json!({ "layout": value })
        }
        "layout" => {
            if field.len() != 2 {
                return Err(ApiError::bad_request(
                    "html_artifact_patch_target_unsupported",
                    "layout patch paths may only target one field".to_string(),
                ));
            }
            let key = field[1].as_str();
            if !matches!(key, "x" | "y" | "w" | "h") {
                return Err(ApiError::bad_request(
                    "html_artifact_patch_target_unsupported",
                    format!("layout.{key} cannot be patched"),
                ));
            }
            validate_html_artifact_layout_number(&value, key)?;
            let object = merge_static_page_current_module_object_field(
                draft_payload,
                module_id,
                "layout",
                key,
                value,
            );
            let layout_value = Value::Object(object);
            validate_html_artifact_layout_patch(&layout_value)?;
            json!({ "layout": layout_value })
        }
        _ => {
            return Err(ApiError::bad_request(
                "html_artifact_patch_target_unsupported",
                format!("modules/{module_id}/{root} cannot be patched"),
            ))
        }
    };
    Ok(json!({
        "type": "update_module",
        "targetModuleId": module_id,
        "patch": patch,
    }))
}

fn normalize_static_page_nested_patch_key(
    parent: &str,
    key: &str,
) -> std::result::Result<String, ApiError> {
    let normalized = match (parent, key) {
        ("data_binding", "source_id") | ("dataBinding", "source_id") => "sourceId",
        ("data_binding", "field_path") | ("dataBinding", "field_path") => "fieldPath",
        ("data_binding", "evidence_ids") | ("dataBinding", "evidence_ids") => "evidenceIds",
        ("visualization", "chart_runtime") => "chartRuntime",
        ("visualization", "chart_options") => "chartOptions",
        ("visualization", "sample_data") => "sampleData",
        _ => key,
    };
    Ok(normalized.to_string())
}

fn merge_static_page_current_module_object_field(
    draft_payload: &Value,
    module_id: &str,
    field: &str,
    key: &str,
    value: Value,
) -> Map<String, Value> {
    let mut object = draft_payload
        .get("modules")
        .and_then(Value::as_array)
        .and_then(|modules| {
            modules.iter().find(|module| {
                module
                    .get("id")
                    .and_then(Value::as_str)
                    .is_some_and(|id| id == module_id)
            })
        })
        .and_then(|module| module.get(field))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    object.insert(key.to_string(), value);
    object
}

fn html_patch_string_value(value: &Value, field_name: &str) -> std::result::Result<(), ApiError> {
    if value
        .as_str()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
    {
        return Ok(());
    }
    Err(ApiError::bad_request(
        "html_artifact_patch_value_invalid",
        format!("{field_name} must be a non-empty string"),
    ))
}

fn validate_html_artifact_layout_patch(value: &Value) -> std::result::Result<(), ApiError> {
    let Some(object) = value.as_object() else {
        return Err(ApiError::bad_request(
            "html_artifact_patch_value_invalid",
            "layout patch must be an object".to_string(),
        ));
    };
    for (key, value) in object {
        if !matches!(key.as_str(), "x" | "y" | "w" | "h") {
            return Err(ApiError::bad_request(
                "html_artifact_patch_target_unsupported",
                format!("layout.{key} cannot be patched"),
            ));
        }
        validate_html_artifact_layout_number(value, key)?;
    }
    Ok(())
}

fn validate_html_artifact_layout_number(
    value: &Value,
    field_name: &str,
) -> std::result::Result<(), ApiError> {
    let Some(number) = value.as_f64() else {
        return Err(ApiError::bad_request(
            "html_artifact_patch_value_invalid",
            format!("layout.{field_name} must be a number"),
        ));
    };
    let valid = match field_name {
        "x" | "y" => (0.0..=100.0).contains(&number),
        "w" | "h" => (1.0..=100.0).contains(&number),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(ApiError::bad_request(
            "html_artifact_patch_value_invalid",
            format!("layout.{field_name} is outside the supported range"),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft_payload() -> Value {
        json!({
            "styleDirection": "client-delivery",
            "modules": [{
                "id": "hero",
                "title": "原标题",
                "content": "原内容",
                "layout": {"x": 0, "y": 0, "w": 6, "h": 3},
                "dataBinding": {"sourceId": "old-source"},
                "visualization": {"type": "kpi"}
            }],
            "mobileOrder": ["hero"]
        })
    }

    #[test]
    fn translates_static_page_style_module_and_nested_layout_patches() {
        let operations = html_artifact_static_page_operations_from_patch(
            &draft_payload(),
            &json!({
                "operations": [
                    {"op": "replace", "path": "/styleDirection", "value": "data-command"},
                    {"op": "replace", "path": "/modules/0/title", "value": "新标题"},
                    {"op": "replace", "path": "/modules/hero/data_binding/field_path", "value": "sales.amount"},
                    {"op": "replace", "path": "/modules/hero/layout/w", "value": 8}
                ]
            }),
        )
        .expect("patch should translate");

        assert_eq!(operations.len(), 4);
        assert_eq!(operations[0]["type"], json!("change_style_direction"));
        assert_eq!(operations[1]["targetModuleId"], json!("hero"));
        assert_eq!(operations[1]["patch"]["title"], json!("新标题"));
        assert_eq!(
            operations[2]["patch"]["dataBinding"]["sourceId"],
            json!("old-source")
        );
        assert_eq!(
            operations[2]["patch"]["dataBinding"]["fieldPath"],
            json!("sales.amount")
        );
        assert_eq!(operations[3]["patch"]["layout"]["w"], json!(8));
    }

    #[test]
    fn wrapper_translates_and_validates_static_page_operations() {
        let operations = static_page_operations_from_html_artifact_patch(
            &draft_payload(),
            &json!({
                "operations": [
                    {"op": "replace", "path": "/modules/0/title", "value": "新标题"},
                    {"op": "replace", "path": "/styleDirection", "value": "data-command"}
                ]
            }),
        )
        .expect("wrapper should translate and validate patch operations");

        assert_eq!(operations.len(), 2);
        assert_eq!(operations[0]["type"], json!("update_module"));
        assert_eq!(operations[0]["targetModuleId"], json!("hero"));
        assert_eq!(operations[1]["type"], json!("change_style_direction"));
    }

    #[test]
    fn wrapper_rejects_unsupported_static_page_patch_targets() {
        let error = static_page_operations_from_html_artifact_patch(
            &draft_payload(),
            &json!({
                "operations": [
                    {"op": "replace", "path": "/assistant_context/secret", "value": "x"}
                ]
            }),
        )
        .expect_err("unsupported target should remain rejected through the wrapper");

        assert_eq!(error.payload.code, "html_artifact_patch_target_unsupported");
    }

    #[test]
    fn supports_remove_as_null_and_mobile_order_patch() {
        let operations = html_artifact_static_page_operations_from_patch(
            &draft_payload(),
            &json!({
                "operations": [
                    {"op": "remove", "path": "/modules/hero/chartOptions"},
                    {"op": "replace", "path": "/mobile_order", "value": ["hero"]}
                ]
            }),
        )
        .expect("remove and reorder should translate");

        assert_eq!(operations[0]["patch"]["chartOptions"], Value::Null);
        assert_eq!(operations[1]["type"], json!("reorder_modules"));
        assert_eq!(operations[1]["order"], json!(["hero"]));
    }

    #[test]
    fn rejects_unsupported_operations_targets_and_layout_values() {
        let unsupported_op = html_artifact_static_page_operations_from_patch(
            &draft_payload(),
            &json!({"operations": [{"op": "copy", "path": "/modules/0/title"}]}),
        )
        .expect_err("copy is not executable against static page drafts");
        assert_eq!(
            unsupported_op.payload.code,
            "html_artifact_patch_operation_unsupported"
        );

        let bad_target = html_artifact_static_page_operations_from_patch(
            &draft_payload(),
            &json!({"operations": [{"op": "replace", "path": "/assistant_context/secret", "value": "x"}]}),
        )
        .expect_err("unsupported root path must be rejected");
        assert_eq!(
            bad_target.payload.code,
            "html_artifact_patch_target_unsupported"
        );

        let bad_layout = html_artifact_static_page_operations_from_patch(
            &draft_payload(),
            &json!({"operations": [{"op": "replace", "path": "/modules/hero/layout/w", "value": 0}]}),
        )
        .expect_err("invalid layout width must be rejected");
        assert_eq!(bad_layout.payload.code, "html_artifact_patch_value_invalid");
    }
}
