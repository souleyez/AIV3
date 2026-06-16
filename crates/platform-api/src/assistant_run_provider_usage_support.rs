use domain_model::{AssistantRun, AssistantRunEvent, AssistantRunId};
use serde_json::{json, Value};

pub(crate) fn assistant_run_provider_usage_events(
    run: &AssistantRun,
    events: &[AssistantRunEvent],
) -> Vec<Value> {
    let mut usage_events = events
        .iter()
        .filter_map(|event| {
            let runtime = event.payload.get("runtime")?;
            assistant_run_provider_usage_event_from_runtime_manifest(
                run.id,
                Some(event.sequence_no),
                Some(event.event_name.as_str()),
                runtime,
            )
        })
        .collect::<Vec<_>>();

    if usage_events.is_empty() {
        if let Some(event) = assistant_run_provider_usage_event_from_runtime_manifest(
            run.id,
            None,
            Some("assistant_run.current_runtime"),
            &run.runtime_manifest,
        ) {
            usage_events.push(event);
        }
    }

    usage_events
}

fn assistant_run_provider_usage_event_from_runtime_manifest(
    run_id: AssistantRunId,
    sequence_no: Option<i32>,
    event_name: Option<&str>,
    runtime: &Value,
) -> Option<Value> {
    let provider = runtime.get("provider").and_then(Value::as_str)?;
    let model = runtime.get("model").and_then(Value::as_str)?;
    let usage = runtime.get("usage").filter(|value| value.is_object());
    let provider_failure = runtime
        .get("provider_failure")
        .filter(|value| value.is_object());

    Some(json!({
        "assistant_run_id": run_id.to_string(),
        "sequence_no": sequence_no,
        "event_name": event_name,
        "mode": runtime.get("mode").and_then(Value::as_str),
        "provider": provider,
        "model": model,
        "lane": runtime.get("lane").and_then(Value::as_str),
        "request_id": runtime.get("request_id").and_then(Value::as_str),
        "status": if provider_failure.is_some() { "failed" } else { "responded" },
        "input_tokens": usage
            .and_then(|usage| usage.get("input_tokens"))
            .and_then(Value::as_u64),
        "output_tokens": usage
            .and_then(|usage| usage.get("output_tokens"))
            .and_then(Value::as_u64),
        "total_tokens": usage
            .and_then(|usage| usage.get("total_tokens"))
            .and_then(Value::as_u64),
        "latency_ms": runtime.get("latency_ms").and_then(Value::as_u64),
        "provider_failure_kind": provider_failure
            .and_then(|failure| failure.get("kind"))
            .and_then(Value::as_str),
    }))
}

pub(crate) fn assistant_run_provider_usage_summary(events: &[Value]) -> Value {
    json!({
        "request_count": events.len(),
        "failed_request_count": events
            .iter()
            .filter(|event| event.get("status").and_then(Value::as_str) == Some("failed"))
            .count(),
        "input_tokens": assistant_run_sum_usage_field(events, "input_tokens"),
        "output_tokens": assistant_run_sum_usage_field(events, "output_tokens"),
        "total_tokens": assistant_run_sum_usage_field(events, "total_tokens"),
        "last_request_id": events
            .iter()
            .rev()
            .find_map(|event| event.get("request_id").and_then(Value::as_str))
    })
}

pub(crate) fn assistant_run_recent_provider_usage_events(
    mut events: Vec<Value>,
    limit: usize,
) -> Vec<Value> {
    if events.len() > limit {
        events = events.split_off(events.len() - limit);
    }
    events
}

fn assistant_run_sum_usage_field(events: &[Value], field: &str) -> u64 {
    events
        .iter()
        .filter_map(|event| event.get(field).and_then(Value::as_u64))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_usage_event_summarizes_runtime_without_failure_body() {
        let run_id = AssistantRunId::new();
        let usage_event = assistant_run_provider_usage_event_from_runtime_manifest(
            run_id,
            Some(7),
            Some("assistant_run.completed"),
            &json!({
                "mode": "provider",
                "provider": "minimax",
                "model": "MiniMax-M2.7",
                "lane": "assistant_chat",
                "request_id": "req-7",
                "usage": {
                    "input_tokens": 11,
                    "output_tokens": 13,
                    "total_tokens": 24
                },
                "latency_ms": 1234,
                "provider_failure": {
                    "kind": "rate_limited",
                    "message": "sk-should-not-leak raw prompt should not leak"
                }
            }),
        )
        .expect("runtime should produce provider usage");

        assert_eq!(usage_event["assistant_run_id"], json!(run_id.to_string()));
        assert_eq!(usage_event["sequence_no"], json!(7));
        assert_eq!(usage_event["event_name"], json!("assistant_run.completed"));
        assert_eq!(usage_event["provider"], json!("minimax"));
        assert_eq!(usage_event["model"], json!("MiniMax-M2.7"));
        assert_eq!(usage_event["status"], json!("failed"));
        assert_eq!(usage_event["provider_failure_kind"], json!("rate_limited"));
        assert_eq!(usage_event["input_tokens"], json!(11));
        assert_eq!(usage_event["output_tokens"], json!(13));
        assert_eq!(usage_event["total_tokens"], json!(24));
        let serialized = serde_json::to_string(&usage_event).expect("event serializes");
        assert!(!serialized.contains("sk-should-not-leak"));
        assert!(!serialized.contains("raw prompt should not leak"));
    }

    #[test]
    fn provider_usage_summary_sums_tokens_and_recent_keeps_tail() {
        let events = vec![
            json!({
                "request_id": "req-1",
                "status": "responded",
                "input_tokens": 3,
                "output_tokens": 5,
                "total_tokens": 8
            }),
            json!({
                "request_id": "req-2",
                "status": "failed",
                "input_tokens": 7,
                "output_tokens": 11,
                "total_tokens": 18
            }),
            json!({
                "request_id": "req-3",
                "status": "responded",
                "input_tokens": 13,
                "output_tokens": 17,
                "total_tokens": 30
            }),
        ];

        let summary = assistant_run_provider_usage_summary(&events);
        let recent = assistant_run_recent_provider_usage_events(events, 2);

        assert_eq!(summary["request_count"], json!(3));
        assert_eq!(summary["failed_request_count"], json!(1));
        assert_eq!(summary["input_tokens"], json!(23));
        assert_eq!(summary["output_tokens"], json!(33));
        assert_eq!(summary["total_tokens"], json!(56));
        assert_eq!(summary["last_request_id"], json!("req-3"));
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0]["request_id"], json!("req-2"));
        assert_eq!(recent[1]["request_id"], json!("req-3"));
    }
}
