use domain_model::AssistantRunEvent;
use serde_json::{json, Value};

use crate::codex_host_fixed_task_safe_text;

pub(crate) fn assistant_run_codex_fixed_task_event_summary(events: &[AssistantRunEvent]) -> Value {
    let fixed_events = events
        .iter()
        .filter(|event| event.event_name.starts_with("codex_host.fixed_task."))
        .collect::<Vec<_>>();
    let runtime_events = events
        .iter()
        .filter(|event| {
            matches!(
                event.event_name.as_str(),
                "codex_host_task.poll_retry"
                    | "codex_host_task.exec_heartbeat"
                    | "codex_host_task.cloudflare_heartbeat"
                    | "codex_host_task.cancelled"
                    | "codex_host_task.exec_failed"
                    | "codex_host_task.exec_completed"
                    | "codex_host_task.completed"
            )
        })
        .collect::<Vec<_>>();
    let latest = fixed_events.last().copied();
    let latest_runtime = runtime_events.last().copied();
    let recent = fixed_events
        .iter()
        .rev()
        .take(8)
        .map(|event| {
            json!({
                "event_id": event.id.to_string(),
                "sequence_no": event.sequence_no,
                "event_name": event.event_name.clone(),
                "template_id": event.payload.get("template_id").cloned().unwrap_or(Value::Null),
                "status": event.payload.get("status").cloned().unwrap_or(Value::Null),
                "artifact_public_url": event
                    .payload
                    .pointer("/output/artifact_public_url")
                    .cloned()
                    .unwrap_or(Value::Null),
                "changed_file_count": event
                    .payload
                    .pointer("/output/changed_file_count")
                    .cloned()
                    .unwrap_or(Value::Null),
                "test_commands": event
                    .payload
                    .pointer("/output/test_commands")
                    .cloned()
                    .unwrap_or_else(|| json!([])),
                "human_review_reason": event
                    .payload
                    .pointer("/output/human_review_reason")
                    .cloned()
                    .unwrap_or(Value::Null),
                "validation_reason": event
                    .payload
                    .pointer("/validation/reason")
                    .cloned()
                    .unwrap_or(Value::Null),
            })
        })
        .collect::<Vec<_>>();
    let recent_runtime = runtime_events
        .iter()
        .rev()
        .take(8)
        .map(|event| {
            json!({
                "event_id": event.id.to_string(),
                "sequence_no": event.sequence_no,
                "event_name": event.event_name.clone(),
                "status": event.payload.get("status").cloned().unwrap_or(Value::Null),
                "reason": event
                    .payload
                    .get("reason")
                    .and_then(Value::as_str)
                    .map(codex_host_fixed_task_safe_text)
                    .unwrap_or_default(),
                "retryable": event.payload.get("retryable").cloned().unwrap_or(Value::Null),
                "attempt": event.payload.get("attempt").cloned().unwrap_or(Value::Null),
                "max_attempts": event.payload.get("max_attempts").cloned().unwrap_or(Value::Null),
                "available_at": event.payload.get("available_at").cloned().unwrap_or(Value::Null),
                "elapsed_ms": event.payload.get("elapsed_ms").cloned().unwrap_or(Value::Null),
                "heartbeat_count": event.payload.get("heartbeat_count").cloned().unwrap_or(Value::Null),
                "secrets_exposed": event
                    .payload
                    .get("secrets_exposed")
                    .cloned()
                    .unwrap_or(Value::Bool(false)),
            })
        })
        .collect::<Vec<_>>();
    json!({
        "event_count": fixed_events.len(),
        "queued_count": fixed_events.iter().filter(|event| event.event_name == "codex_host.fixed_task.queued").count(),
        "completed_count": fixed_events.iter().filter(|event| event.event_name == "codex_host.fixed_task.completed").count(),
        "needs_human_count": fixed_events.iter().filter(|event| event.event_name == "codex_host.fixed_task.needs_human").count(),
        "rejected_count": fixed_events.iter().filter(|event| event.event_name == "codex_host.fixed_task.rejected").count(),
        "poll_retry_count": runtime_events.iter().filter(|event| event.event_name == "codex_host_task.poll_retry").count(),
        "cancelled_count": runtime_events.iter().filter(|event| event.event_name == "codex_host_task.cancelled").count(),
        "exec_failed_count": runtime_events.iter().filter(|event| event.event_name == "codex_host_task.exec_failed").count(),
        "heartbeat_count": runtime_events
            .iter()
            .filter(|event| matches!(
                event.event_name.as_str(),
                "codex_host_task.exec_heartbeat" | "codex_host_task.cloudflare_heartbeat"
            ))
            .count(),
        "exec_completed_count": runtime_events.iter().filter(|event| event.event_name == "codex_host_task.exec_completed").count(),
        "latest": latest.map(|event| {
            json!({
                "event_name": event.event_name.clone(),
                "template_id": event.payload.get("template_id").cloned().unwrap_or(Value::Null),
                "status": event.payload.get("status").cloned().unwrap_or(Value::Null),
                "validation_reason": event.payload.pointer("/validation/reason").cloned().unwrap_or(Value::Null),
            })
        }).unwrap_or(Value::Null),
        "latest_runtime": latest_runtime.map(|event| {
            json!({
                "event_name": event.event_name.clone(),
                "status": event.payload.get("status").cloned().unwrap_or(Value::Null),
                "reason": event
                    .payload
                    .get("reason")
                    .and_then(Value::as_str)
                    .map(codex_host_fixed_task_safe_text)
                    .unwrap_or_default(),
                "retryable": event.payload.get("retryable").cloned().unwrap_or(Value::Null),
                "attempt": event.payload.get("attempt").cloned().unwrap_or(Value::Null),
                "max_attempts": event.payload.get("max_attempts").cloned().unwrap_or(Value::Null),
                "available_at": event.payload.get("available_at").cloned().unwrap_or(Value::Null),
            })
        }).unwrap_or(Value::Null),
        "recent": recent,
        "recent_runtime": recent_runtime,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{AssistantRunEventId, AssistantRunId, TenantId};

    fn event(sequence_no: i32, event_name: &str, payload: Value) -> AssistantRunEvent {
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

    #[test]
    fn assistant_run_codex_fixed_task_event_summary_counts_and_redacts_runtime() {
        let events = vec![
            event(
                1,
                "codex_host.fixed_task.queued",
                json!({
                    "template_id": "static_page_generation",
                    "status": "queued",
                    "raw_prompt": "sk-should-not-leak"
                }),
            ),
            event(
                2,
                "codex_host_task.poll_retry",
                json!({
                    "status": "processing",
                    "reason": "retry after provider key sk-secret-value",
                    "attempt": 1,
                    "max_attempts": 3,
                    "secrets_exposed": true,
                    "raw_error": "raw-provider-payload-should-not-leak"
                }),
            ),
        ];

        let summary = assistant_run_codex_fixed_task_event_summary(&events);
        let serialized = summary.to_string();

        assert_eq!(summary["event_count"], json!(1));
        assert_eq!(summary["queued_count"], json!(1));
        assert_eq!(summary["poll_retry_count"], json!(1));
        assert_eq!(
            summary["latest"]["template_id"],
            json!("static_page_generation")
        );
        assert_eq!(summary["latest_runtime"]["attempt"], json!(1));
        assert_eq!(summary["recent_runtime"][0]["secrets_exposed"], json!(true));
        assert!(summary["latest_runtime"]["reason"]
            .as_str()
            .unwrap_or_default()
            .contains("[redacted]"));
        assert!(!serialized.contains("sk-should-not-leak"));
        assert!(!serialized.contains("raw-provider-payload-should-not-leak"));
    }
}
