use chrono::Utc;
use serde_json::{json, Value};

use crate::refresh_static_page_payload_design_contract;
use crate::static_page_module_operation_support::{
    merge_static_page_module, push_static_page_module, remove_static_page_module,
};
use crate::static_page_operation_metadata_support::{
    static_page_operation_module_id, static_page_operation_type,
};
use crate::static_page_payload_support::{
    ensure_json_object, set_payload_string, set_payload_value,
};

pub(crate) fn apply_static_page_operations_to_payload(
    mut payload: Value,
    operations: &[Value],
    summary: Option<&str>,
) -> Value {
    ensure_json_object(&mut payload);
    for operation in operations {
        apply_static_page_operation_to_payload(&mut payload, operation);
    }
    if let Some(summary) = summary.filter(|value| !value.trim().is_empty()) {
        set_payload_string(&mut payload, "modelSummary", summary);
        set_payload_string(&mut payload, "model_summary", summary);
    }
    refresh_static_page_payload_design_contract(&mut payload);
    payload
}

pub(crate) fn apply_static_page_operation_to_payload(payload: &mut Value, operation: &Value) {
    let Some(operation_type) = static_page_operation_type(operation) else {
        return;
    };
    match operation_type {
        "change_style_direction" => {
            if let Some(style) = operation
                .get("styleDirection")
                .or_else(|| operation.get("style_direction"))
                .and_then(Value::as_str)
            {
                set_payload_string(payload, "styleDirection", style);
                set_payload_string(payload, "style_direction", style);
            }
        }
        "refresh_summary" => {
            if let Some(summary) = operation
                .get("modelSummary")
                .or_else(|| operation.get("model_summary"))
                .and_then(Value::as_str)
            {
                set_payload_string(payload, "modelSummary", summary);
                set_payload_string(payload, "model_summary", summary);
            }
        }
        "update_module" => {
            if let (Some(module_id), Some(patch)) = (
                static_page_operation_module_id(operation),
                operation.get("patch"),
            ) {
                merge_static_page_module(payload, module_id, patch);
            }
        }
        "change_visualization" => {
            if let (Some(module_id), Some(visualization_type)) = (
                static_page_operation_module_id(operation),
                operation.get("visualizationType").and_then(Value::as_str),
            ) {
                let mut visualization = json!({
                    "type": visualization_type,
                });
                if let Some(chart_runtime) = operation.get("chartRuntime") {
                    set_payload_value(&mut visualization, "chartRuntime", chart_runtime.clone());
                }
                if let Some(chart_options) = operation.get("chartOptions") {
                    set_payload_value(&mut visualization, "chartOptions", chart_options.clone());
                }
                merge_static_page_module(
                    payload,
                    module_id,
                    &json!({
                        "visualization": visualization,
                    }),
                );
            }
        }
        "change_data_binding" => {
            if let (Some(module_id), Some(data_binding)) = (
                static_page_operation_module_id(operation),
                operation.get("dataBinding"),
            ) {
                merge_static_page_module(
                    payload,
                    module_id,
                    &json!({
                        "dataBinding": data_binding,
                    }),
                );
            }
        }
        "add_module" => {
            if let Some(module) = operation.get("module") {
                push_static_page_module(payload, module.clone());
            }
        }
        "remove_module" => {
            if let Some(module_id) = static_page_operation_module_id(operation) {
                remove_static_page_module(payload, module_id);
            }
        }
        "move_module" | "resize_module" => {
            if let (Some(module_id), Some(layout)) = (
                static_page_operation_module_id(operation),
                operation.get("layout").or_else(|| operation.get("patch")),
            ) {
                merge_static_page_module(
                    payload,
                    module_id,
                    &json!({
                        "layout": layout,
                    }),
                );
            }
        }
        "reorder_modules" => {
            if let Some(order) = operation.get("order").and_then(Value::as_array) {
                set_payload_value(payload, "mobileOrder", Value::Array(order.clone()));
                set_payload_value(payload, "mobile_order", Value::Array(order.clone()));
            }
        }
        "queue_image_job" => {
            set_payload_string(payload, "status", "queued");
            set_payload_value(
                payload,
                "imageJob",
                json!({
                    "id": operation.get("jobId").cloned().unwrap_or(Value::Null),
                    "status": "queued",
                    "queuePosition": operation.get("queuePosition").cloned().unwrap_or(Value::Null),
                    "queueMessage": operation
                        .get("queueMessage")
                        .and_then(Value::as_str)
                        .unwrap_or("资源正在排队，可以联系商务开通高级用户跳过等待。"),
                }),
            );
            set_payload_value(
                payload,
                "previewContract",
                json!({
                    "status": "queued",
                    "imageJobId": operation.get("jobId").cloned().unwrap_or(Value::Null),
                    "assetKey": Value::Null,
                    "queuePosition": operation.get("queuePosition").cloned().unwrap_or(Value::Null),
                    "confirmedAt": Value::Null,
                }),
            );
        }
        "mark_preview_ready" => {
            set_payload_string(payload, "status", "preview_ready");
            if let Some(preview) = operation.get("previewImage") {
                set_payload_value(payload, "previewImage", preview.clone());
                set_payload_value(
                    payload,
                    "previewContract",
                    json!({
                        "status": "preview_ready",
                        "imageJobId": preview.get("imageJobId").cloned().unwrap_or(Value::Null),
                        "assetKey": preview.get("assetKey").cloned().unwrap_or(Value::Null),
                        "queuePosition": Value::Null,
                    }),
                );
            }
        }
        "confirm_preview" => {
            set_payload_string(payload, "status", "effect_confirmed");
            if let Some(preview) = operation.get("previewImage") {
                set_payload_value(payload, "previewImage", preview.clone());
                set_payload_value(
                    payload,
                    "previewContract",
                    json!({
                        "status": "confirmed",
                        "imageJobId": preview.get("imageJobId").cloned().unwrap_or(Value::Null),
                        "assetKey": preview.get("assetKey").cloned().unwrap_or(Value::Null),
                        "queuePosition": Value::Null,
                        "confirmedAt": Utc::now(),
                    }),
                );
            }
        }
        "reset_image_job" => {
            set_payload_string(payload, "status", "planning");
            set_payload_value(
                payload,
                "imageJob",
                json!({
                    "id": Value::Null,
                    "status": "idle",
                    "queuePosition": Value::Null,
                    "queueMessage": "",
                }),
            );
            set_payload_value(payload, "previewImage", Value::Null);
            set_payload_value(payload, "finalPage", Value::Null);
            set_payload_value(
                payload,
                "previewContract",
                json!({
                    "status": "not_requested",
                    "imageJobId": Value::Null,
                    "assetKey": Value::Null,
                    "queuePosition": Value::Null,
                    "confirmedAt": Value::Null,
                }),
            );
        }
        "reset_final_render" => {
            set_payload_string(payload, "status", "effect_confirmed");
            set_payload_value(payload, "finalPage", Value::Null);
            let asset_key = payload
                .get("previewImage")
                .and_then(|preview| preview.get("assetKey"))
                .cloned()
                .unwrap_or(Value::Null);
            set_payload_value(
                payload,
                "previewContract",
                json!({
                    "status": "confirmed",
                    "assetKey": asset_key,
                    "queuePosition": Value::Null,
                }),
            );
        }
        "request_final_render" => {
            set_payload_string(payload, "status", "rendering");
            if let Some(final_page) = operation
                .get("finalPage")
                .or_else(|| operation.get("payload"))
            {
                set_payload_value(payload, "finalPage", final_page.clone());
            }
        }
        _ => {}
    }
}

pub(crate) fn mark_static_page_payload_preview_stale(payload: &mut Value) {
    ensure_json_object(payload);
    let Some(object) = payload.as_object_mut() else {
        return;
    };
    object.remove("previewImage");
    object.remove("preview_image");
    object.remove("finalPage");
    object.remove("final_page");
    object.insert("status".to_string(), json!("planning"));
    if let Some(image_job) = object.get_mut("imageJob").and_then(Value::as_object_mut) {
        image_job.insert("status".to_string(), json!("stale"));
    }
    if let Some(image_job) = object.get_mut("image_job").and_then(Value::as_object_mut) {
        image_job.insert("status".to_string(), json!("stale"));
    }
}

pub(crate) fn finalize_static_page_operations_payload(
    payload: &mut Value,
    operations: &[Value],
    prompt: Option<&str>,
    summary: &str,
) {
    ensure_json_object(payload);
    if !summary.trim().is_empty() {
        set_payload_string(payload, "modelSummary", summary);
        set_payload_string(payload, "model_summary", summary);
    }
    refresh_static_page_payload_design_contract(payload);
    append_static_page_operations_metadata(payload, operations, prompt, summary);
}

pub(crate) fn append_static_page_operations_metadata(
    payload: &mut Value,
    operations: &[Value],
    prompt: Option<&str>,
    summary: &str,
) {
    ensure_json_object(payload);
    let Some(object) = payload.as_object_mut() else {
        return;
    };
    let entry = object
        .entry("operations".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    if !entry.is_array() {
        *entry = Value::Array(Vec::new());
    }
    if let Some(items) = entry.as_array_mut() {
        let applied_at = Utc::now().to_rfc3339();
        for operation in operations {
            let mut operation = operation.clone();
            if let Some(object) = operation.as_object_mut() {
                object
                    .entry("appliedAt".to_string())
                    .or_insert_with(|| json!(applied_at));
                if let Some(prompt) = prompt.map(str::trim).filter(|value| !value.is_empty()) {
                    object
                        .entry("prompt".to_string())
                        .or_insert_with(|| json!(prompt));
                }
            }
            items.push(operation);
        }
    }
    object.insert("lastOperationSummary".to_string(), json!(summary));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_module_visualization_binding_order_and_summary() {
        let payload = json!({
            "modules": [
                {
                    "id": "sales",
                    "title": "旧标题",
                    "layout": {"w": 2}
                },
                {
                    "id": "risk",
                    "title": "风险"
                }
            ]
        });

        let payload = apply_static_page_operations_to_payload(
            payload,
            &[
                json!({
                    "type": "update_module",
                    "targetModuleId": "sales",
                    "patch": {"title": "销售趋势", "layout": {"h": 3}}
                }),
                json!({
                    "type": "change_visualization",
                    "moduleId": "sales",
                    "visualizationType": "line",
                    "chartRuntime": "echarts",
                    "chartOptions": {"smooth": true}
                }),
                json!({
                    "type": "change_data_binding",
                    "moduleId": "sales",
                    "dataBinding": {"metric": "revenue"}
                }),
                json!({
                    "type": "reorder_modules",
                    "order": ["risk", "sales"]
                }),
            ],
            Some("模型总结"),
        );

        assert_eq!(payload["modules"][0]["title"], json!("销售趋势"));
        assert_eq!(payload["modules"][0]["layout"], json!({"w": 2, "h": 3}));
        assert_eq!(
            payload["modules"][0]["visualization"]["type"],
            json!("line")
        );
        assert_eq!(
            payload["modules"][0]["visualization"]["chartRuntime"],
            json!("echarts")
        );
        assert_eq!(
            payload["modules"][0]["dataBinding"],
            json!({"metric": "revenue"})
        );
        assert_eq!(payload["mobileOrder"], json!(["risk", "sales"]));
        assert_eq!(payload["modelSummary"], json!("模型总结"));
        assert!(payload.get("previewContract").is_some());
    }

    #[test]
    fn applies_module_add_remove_and_layout_operations() {
        let mut payload = json!({
            "modules": [
                {"id": "old", "title": "旧模块"},
                {"id": "hero", "layout": {"x": 0}}
            ]
        });

        apply_static_page_operation_to_payload(
            &mut payload,
            &json!({"type": "add_module", "module": {"id": "new", "title": "新增"}}),
        );
        apply_static_page_operation_to_payload(
            &mut payload,
            &json!({"type": "remove_module", "targetModuleId": "old"}),
        );
        apply_static_page_operation_to_payload(
            &mut payload,
            &json!({"type": "move_module", "moduleId": "hero", "layout": {"y": 2}}),
        );

        assert_eq!(payload["modules"][0]["id"], json!("hero"));
        assert_eq!(payload["modules"][0]["layout"], json!({"x": 0, "y": 2}));
        assert_eq!(payload["modules"][1], json!({"id": "new", "title": "新增"}));
    }

    #[test]
    fn applies_image_job_preview_and_final_render_statuses() {
        let mut payload = json!({});

        apply_static_page_operation_to_payload(
            &mut payload,
            &json!({
                "type": "queue_image_job",
                "jobId": "job-1",
                "queuePosition": 3
            }),
        );
        assert_eq!(payload["status"], json!("queued"));
        assert_eq!(payload["imageJob"]["status"], json!("queued"));
        assert_eq!(payload["previewContract"]["imageJobId"], json!("job-1"));

        apply_static_page_operation_to_payload(
            &mut payload,
            &json!({
                "type": "confirm_preview",
                "previewImage": {
                    "imageJobId": "job-1",
                    "assetKey": "asset/a.png"
                }
            }),
        );
        assert_eq!(payload["status"], json!("effect_confirmed"));
        assert_eq!(payload["previewImage"]["assetKey"], json!("asset/a.png"));
        assert_eq!(payload["previewContract"]["status"], json!("confirmed"));

        apply_static_page_operation_to_payload(
            &mut payload,
            &json!({
                "type": "request_final_render",
                "finalPage": {"url": "https://example.test/index.html"}
            }),
        );
        assert_eq!(payload["status"], json!("rendering"));
        assert_eq!(
            payload["finalPage"],
            json!({"url": "https://example.test/index.html"})
        );
    }

    #[test]
    fn stale_preview_clears_preview_and_marks_image_jobs() {
        let mut payload = json!({
            "status": "preview_ready",
            "previewImage": {"assetKey": "asset/a.png"},
            "preview_image": {"assetKey": "asset/a.png"},
            "finalPage": {"url": "https://example.test/index.html"},
            "final_page": {"url": "https://example.test/index.html"},
            "imageJob": {"status": "queued"},
            "image_job": {"status": "queued"}
        });

        mark_static_page_payload_preview_stale(&mut payload);

        assert_eq!(payload["status"], json!("planning"));
        assert!(payload.get("previewImage").is_none());
        assert!(payload.get("preview_image").is_none());
        assert!(payload.get("finalPage").is_none());
        assert!(payload.get("final_page").is_none());
        assert_eq!(payload["imageJob"]["status"], json!("stale"));
        assert_eq!(payload["image_job"]["status"], json!("stale"));
    }

    #[test]
    fn append_metadata_replaces_invalid_operation_list_and_preserves_explicit_fields() {
        let mut payload = json!({"operations": "invalid"});

        append_static_page_operations_metadata(
            &mut payload,
            &[
                json!({"type": "update_module"}),
                json!({
                    "type": "remove_module",
                    "appliedAt": "fixed-time",
                    "prompt": "fixed prompt"
                }),
            ],
            Some(" 用户提示 "),
            "处理完成",
        );

        let operations = payload["operations"].as_array().expect("operations array");
        assert_eq!(operations.len(), 2);
        assert_eq!(operations[0]["prompt"], json!("用户提示"));
        assert!(operations[0]["appliedAt"].as_str().is_some());
        assert_eq!(operations[1]["appliedAt"], json!("fixed-time"));
        assert_eq!(operations[1]["prompt"], json!("fixed prompt"));
        assert_eq!(payload["lastOperationSummary"], json!("处理完成"));
    }

    #[test]
    fn finalize_payload_sets_summary_refreshes_contract_and_records_metadata() {
        let mut payload = json!({
            "modules": [{"id": "hero", "title": "Hero"}],
            "previewContract": {"status": "not_requested"}
        });

        finalize_static_page_operations_payload(
            &mut payload,
            &[json!({"type": "change_style_direction", "styleDirection": "dark-mobile"})],
            Some("调暗风格"),
            "已更新风格",
        );

        assert_eq!(payload["modelSummary"], json!("已更新风格"));
        assert_eq!(payload["model_summary"], json!("已更新风格"));
        assert_eq!(payload["operations"][0]["prompt"], json!("调暗风格"));
        assert_eq!(payload["lastOperationSummary"], json!("已更新风格"));
        assert!(payload.get("visualSpec").is_some());
        assert!(payload.get("renderSpec").is_some());
    }
}
