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

pub(crate) fn external_channel_static_page_fixed_task_event_with_status_context(
    events: &[AssistantRunEvent],
    fixed_event: &AssistantRunEvent,
) -> AssistantRunEvent {
    let mut event = fixed_event.clone();
    let Some(context) = events.iter().rev().find(|candidate| {
        candidate.sequence_no <= fixed_event.sequence_no
            && matches!(
                candidate.event_name.as_str(),
                "assistant_run.external_channel_static_page_pipeline_queued"
                    | "assistant_run.external_channel_static_page_publish_queued"
            )
    }) else {
        return event;
    };
    let Some(target) = event.payload.as_object_mut() else {
        return event;
    };
    for key in [
        "status_url",
        "status_method",
        "draft_id",
        "image_job_id",
        "poll_after_seconds",
        "recipient_delivery",
        "permission_review_status",
        "editable_after_publish",
    ] {
        if target.get(key).is_none_or(Value::is_null) {
            if let Some(value) = context.payload.get(key) {
                target.insert(key.to_string(), value.clone());
            }
        }
    }
    event
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{AssistantRunEventId, AssistantRunId, TenantId};
    use serde_json::json;

    fn assistant_event(sequence_no: i32, event_name: &str, payload: Value) -> AssistantRunEvent {
        AssistantRunEvent {
            id: AssistantRunEventId::new(),
            tenant_id: TenantId::new(),
            run_id: AssistantRunId::new(),
            sequence_no,
            event_name: event_name.to_string(),
            payload,
            created_at: Utc::now(),
        }
    }

    fn fixed_task_event(payload: Value) -> AssistantRunEvent {
        assistant_event(1, "codex_host.fixed_task.queued", payload)
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

    #[test]
    fn static_page_fixed_task_status_context_copies_missing_fields_from_latest_context() {
        let fixed_event = assistant_event(
            8,
            "codex_host.fixed_task.queued",
            json!({
                "status_url": null,
                "status_method": "POST",
                "template_id": "static_page_image2_data_publish"
            }),
        );
        let events = vec![
            assistant_event(
                3,
                "assistant_run.external_channel_static_page_pipeline_queued",
                json!({
                    "status_url": "https://old.example/status",
                    "draft_id": "draft-old",
                    "recipient_delivery": {"enabled": false}
                }),
            ),
            assistant_event(
                7,
                "assistant_run.external_channel_static_page_publish_queued",
                json!({
                    "status_url": "https://new.example/status",
                    "status_method": "GET",
                    "draft_id": "draft-new",
                    "image_job_id": "image-job-new",
                    "poll_after_seconds": 15,
                    "recipient_delivery": {"enabled": true},
                    "permission_review_status": "ready",
                    "editable_after_publish": true
                }),
            ),
        ];

        let event = external_channel_static_page_fixed_task_event_with_status_context(
            &events,
            &fixed_event,
        );

        assert_eq!(
            event.payload["status_url"],
            json!("https://new.example/status")
        );
        assert_eq!(event.payload["status_method"], json!("POST"));
        assert_eq!(event.payload["draft_id"], json!("draft-new"));
        assert_eq!(event.payload["image_job_id"], json!("image-job-new"));
        assert_eq!(event.payload["poll_after_seconds"], json!(15));
        assert_eq!(
            event.payload["recipient_delivery"],
            json!({"enabled": true})
        );
        assert_eq!(event.payload["permission_review_status"], json!("ready"));
        assert_eq!(event.payload["editable_after_publish"], json!(true));
    }

    #[test]
    fn static_page_fixed_task_status_context_ignores_future_and_non_context_events() {
        let fixed_event = assistant_event(
            5,
            "codex_host.fixed_task.queued",
            json!({"template_id": "static_page_image2_data_publish"}),
        );
        let events = vec![
            assistant_event(
                4,
                "assistant_run.external_channel_static_page_started",
                json!({"status_url": "https://wrong.example/status"}),
            ),
            assistant_event(
                6,
                "assistant_run.external_channel_static_page_publish_queued",
                json!({"status_url": "https://future.example/status"}),
            ),
        ];

        let event = external_channel_static_page_fixed_task_event_with_status_context(
            &events,
            &fixed_event,
        );

        assert_eq!(event.payload.get("status_url"), None);
    }

    #[test]
    fn static_page_fixed_task_status_context_requires_object_payload() {
        let fixed_event = assistant_event(5, "codex_host.fixed_task.queued", json!("raw"));
        let events = vec![assistant_event(
            4,
            "assistant_run.external_channel_static_page_publish_queued",
            json!({"status_url": "https://new.example/status"}),
        )];

        let event = external_channel_static_page_fixed_task_event_with_status_context(
            &events,
            &fixed_event,
        );

        assert_eq!(event.payload, json!("raw"));
    }
}
