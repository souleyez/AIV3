use domain_model::AssistantRunEvent;
use serde_json::Value;

pub(crate) fn external_channel_fixed_task_template_id(event: &AssistantRunEvent) -> Option<String> {
    event
        .payload
        .get("template_id")
        .and_then(Value::as_str)
        .or_else(|| event.payload.get("capability").and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(crate) fn external_channel_fixed_task_workflow_execution_id(
    event: &AssistantRunEvent,
) -> Value {
    event
        .payload
        .get("workflow_execution_id")
        .or_else(|| event.payload.get("codex_host_workflow_execution_id"))
        .cloned()
        .unwrap_or(Value::Null)
}

pub(crate) fn external_channel_fixed_task_card_type(template_id: &str) -> &'static str {
    match template_id {
        "data_ingestion_analysis" => "v3_data_ingestion_analysis",
        "static_page_image2_data_publish" => "v3_static_page_image2_publish_status",
        "answer_quality_autofix" => "v3_answer_quality_autofix",
        _ => "v3_codex_fixed_task",
    }
}

pub(crate) fn external_channel_fixed_task_status_prefix(template_id: &str) -> &'static str {
    match template_id {
        "data_ingestion_analysis" => "data_ingestion_analysis",
        "static_page_image2_data_publish" => "static_page_publish",
        "answer_quality_autofix" => "answer_quality_autofix",
        _ => "codex_fixed_task",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{AssistantRunEventId, AssistantRunId, TenantId};
    use serde_json::json;

    fn fixed_task_event(payload: Value) -> AssistantRunEvent {
        AssistantRunEvent {
            id: AssistantRunEventId::new(),
            tenant_id: TenantId::new(),
            run_id: AssistantRunId::new(),
            sequence_no: 1,
            event_name: "codex_host.fixed_task.queued".to_string(),
            payload,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn template_id_prefers_template_id_and_trims_blank() {
        let event = fixed_task_event(json!({
            "template_id": " static_page_image2_data_publish ",
            "capability": "data_ingestion_analysis"
        }));

        assert_eq!(
            external_channel_fixed_task_template_id(&event).as_deref(),
            Some("static_page_image2_data_publish")
        );
    }

    #[test]
    fn template_id_falls_back_to_capability_and_ignores_empty_values() {
        let event = fixed_task_event(json!({
            "capability": " answer_quality_autofix "
        }));
        assert_eq!(
            external_channel_fixed_task_template_id(&event).as_deref(),
            Some("answer_quality_autofix")
        );

        let empty = fixed_task_event(json!({
            "template_id": " ",
            "capability": " "
        }));
        assert_eq!(external_channel_fixed_task_template_id(&empty), None);
    }

    #[test]
    fn workflow_execution_id_prefers_workflow_execution_id() {
        let event = fixed_task_event(json!({
            "workflow_execution_id": "workflow-a",
            "codex_host_workflow_execution_id": "workflow-b"
        }));
        assert_eq!(
            external_channel_fixed_task_workflow_execution_id(&event),
            json!("workflow-a")
        );

        let fallback = fixed_task_event(json!({
            "codex_host_workflow_execution_id": "workflow-b"
        }));
        assert_eq!(
            external_channel_fixed_task_workflow_execution_id(&fallback),
            json!("workflow-b")
        );

        let missing = fixed_task_event(json!({}));
        assert_eq!(
            external_channel_fixed_task_workflow_execution_id(&missing),
            Value::Null
        );
    }

    #[test]
    fn card_type_maps_known_templates_and_unknown_default() {
        assert_eq!(
            external_channel_fixed_task_card_type("data_ingestion_analysis"),
            "v3_data_ingestion_analysis"
        );
        assert_eq!(
            external_channel_fixed_task_card_type("static_page_image2_data_publish"),
            "v3_static_page_image2_publish_status"
        );
        assert_eq!(
            external_channel_fixed_task_card_type("answer_quality_autofix"),
            "v3_answer_quality_autofix"
        );
        assert_eq!(
            external_channel_fixed_task_card_type("unknown"),
            "v3_codex_fixed_task"
        );
    }

    #[test]
    fn status_prefix_maps_known_templates_and_unknown_default() {
        assert_eq!(
            external_channel_fixed_task_status_prefix("data_ingestion_analysis"),
            "data_ingestion_analysis"
        );
        assert_eq!(
            external_channel_fixed_task_status_prefix("static_page_image2_data_publish"),
            "static_page_publish"
        );
        assert_eq!(
            external_channel_fixed_task_status_prefix("answer_quality_autofix"),
            "answer_quality_autofix"
        );
        assert_eq!(
            external_channel_fixed_task_status_prefix("unknown"),
            "codex_fixed_task"
        );
    }
}
