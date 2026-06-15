use domain_model::StaticPageDraftStatus;
use serde_json::Value;
use std::collections::BTreeMap;

pub(crate) fn summarize_static_page_operations(operations: &[Value]) -> String {
    if operations.is_empty() {
        return "未追加静态页操作。".to_string();
    }
    let mut types = BTreeMap::<String, usize>::new();
    for operation in operations {
        if let Some(operation_type) = static_page_operation_type(operation) {
            *types.entry(operation_type.to_string()).or_default() += 1;
        }
    }
    let labels = types
        .into_iter()
        .map(|(operation_type, count)| format!("{operation_type} x{count}"))
        .collect::<Vec<_>>()
        .join("，");
    format!("已追加静态页操作：{labels}。")
}

pub(crate) fn status_from_static_page_payload(payload: &Value) -> Option<StaticPageDraftStatus> {
    let status = payload
        .as_object()
        .and_then(|object| object.get("status"))
        .and_then(Value::as_str)
        .map(str::trim)?;
    match status {
        "draft" => Some(StaticPageDraftStatus::Draft),
        "planning" | "planned" => Some(StaticPageDraftStatus::Planned),
        "queued" => Some(StaticPageDraftStatus::Queued),
        "preview_ready" | "previewed" => Some(StaticPageDraftStatus::Previewed),
        "effect_confirmed" | "confirmed" => Some(StaticPageDraftStatus::Confirmed),
        "rendering" | "rendered" => Some(StaticPageDraftStatus::Rendered),
        "archived" => Some(StaticPageDraftStatus::Archived),
        _ => None,
    }
}

pub(crate) fn status_from_static_page_operations(
    operations: &[Value],
) -> Option<StaticPageDraftStatus> {
    operations.iter().fold(None, |status, operation| {
        match static_page_operation_type(operation) {
            Some("queue_image_job") => Some(StaticPageDraftStatus::Queued),
            Some("mark_preview_ready") => Some(StaticPageDraftStatus::Previewed),
            Some("confirm_preview") => Some(StaticPageDraftStatus::Confirmed),
            Some("reset_final_render") => Some(StaticPageDraftStatus::Confirmed),
            Some("request_final_render") => Some(StaticPageDraftStatus::Rendered),
            Some(_) => Some(status.unwrap_or(StaticPageDraftStatus::Planned)),
            None => status,
        }
    })
}

pub(crate) fn static_page_operation_type(operation: &Value) -> Option<&str> {
    operation
        .as_object()
        .and_then(|object| object.get("type"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub(crate) fn static_page_operation_module_id(operation: &Value) -> Option<&str> {
    operation
        .get("targetModuleId")
        .or_else(|| operation.get("moduleId"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn operation_type_trims_and_ignores_empty_values() {
        assert_eq!(
            static_page_operation_type(&json!({"type": " update_module "})),
            Some("update_module")
        );
        assert_eq!(static_page_operation_type(&json!({"type": "   "})), None);
        assert_eq!(static_page_operation_type(&json!([])), None);
    }

    #[test]
    fn operation_module_id_prefers_target_module_id_and_trims() {
        assert_eq!(
            static_page_operation_module_id(&json!({
                "targetModuleId": " hero ",
                "moduleId": "fallback"
            })),
            Some("hero")
        );
        assert_eq!(
            static_page_operation_module_id(&json!({"moduleId": "chart"})),
            Some("chart")
        );
        assert_eq!(
            static_page_operation_module_id(&json!({"targetModuleId": ""})),
            None
        );
    }

    #[test]
    fn summarize_operations_counts_sorted_operation_types() {
        let summary = summarize_static_page_operations(&[
            json!({"type": "update_module"}),
            json!({"type": "queue_image_job"}),
            json!({"type": "update_module"}),
            json!({"type": ""}),
        ]);

        assert_eq!(
            summary,
            "已追加静态页操作：queue_image_job x1，update_module x2。"
        );
    }

    #[test]
    fn summarize_operations_handles_empty_list() {
        assert_eq!(summarize_static_page_operations(&[]), "未追加静态页操作。");
    }

    #[test]
    fn payload_status_maps_aliases_to_draft_status() {
        assert_eq!(
            status_from_static_page_payload(&json!({"status": "planning"})),
            Some(StaticPageDraftStatus::Planned)
        );
        assert_eq!(
            status_from_static_page_payload(&json!({"status": "preview_ready"})),
            Some(StaticPageDraftStatus::Previewed)
        );
        assert_eq!(
            status_from_static_page_payload(&json!({"status": "effect_confirmed"})),
            Some(StaticPageDraftStatus::Confirmed)
        );
        assert_eq!(
            status_from_static_page_payload(&json!({"status": "rendering"})),
            Some(StaticPageDraftStatus::Rendered)
        );
        assert_eq!(
            status_from_static_page_payload(&json!({"status": "unknown"})),
            None
        );
    }

    #[test]
    fn operation_status_uses_latest_terminal_operation() {
        let status = status_from_static_page_operations(&[
            json!({"type": "update_module"}),
            json!({"type": "queue_image_job"}),
            json!({"type": "mark_preview_ready"}),
            json!({"type": "confirm_preview"}),
            json!({"type": "request_final_render"}),
        ]);

        assert_eq!(status, Some(StaticPageDraftStatus::Rendered));
    }

    #[test]
    fn operation_status_uses_planned_for_non_status_operations() {
        assert_eq!(
            status_from_static_page_operations(&[json!({"type": "update_module"})]),
            Some(StaticPageDraftStatus::Planned)
        );
        assert_eq!(status_from_static_page_operations(&[json!({})]), None);
    }
}
