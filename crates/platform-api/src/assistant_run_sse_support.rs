use contracts::{ContinueAssistantRunResponse, CreateAssistantRunResponse};
use domain_model::AssistantRunId;
use serde_json::{json, Value};

use crate::sse_support::{sse_json_event, sse_text_delta_events};

pub(crate) const ASSISTANT_RUN_SSE_SCHEMA_V1: &str = "v3.assistant_run.sse.v1";

fn assistant_run_sse_public_payload(
    run_id: Option<AssistantRunId>,
    sequence: i64,
    phase: &str,
    status: &str,
    display_text: &str,
    data: Value,
) -> Value {
    let event_run_id = run_id
        .map(|run_id| run_id.to_string())
        .unwrap_or_else(|| "pending".to_string());
    let mut payload = json!({
        "schema": ASSISTANT_RUN_SSE_SCHEMA_V1,
        "event_id": format!("{event_run_id}:{sequence:06}"),
        "sequence": sequence,
        "assistant_run_id": run_id,
        "phase": phase,
        "status": status,
        "display_text": display_text,
        "data": data,
    });
    if let (Some(payload), Some(data)) = (payload.as_object_mut(), data.as_object()) {
        for (key, value) in data {
            payload.entry(key.clone()).or_insert_with(|| value.clone());
        }
    }
    payload
}

pub(crate) fn create_assistant_run_sse_accepted_event() -> String {
    sse_json_event(
        "assistant_run.accepted",
        assistant_run_sse_public_payload(
            None,
            0,
            "started",
            "accepted",
            "DataMax 已开始处理本轮消息。",
            json!({
                "entrypoint": "create_assistant_run",
                "stream": "sse",
                "status": "accepted",
            }),
        ),
    )
}

pub(crate) fn continue_assistant_run_sse_accepted_event(run_id: AssistantRunId) -> String {
    sse_json_event(
        "assistant_run.accepted",
        assistant_run_sse_public_payload(
            Some(run_id),
            0,
            "continuing",
            "accepted",
            "DataMax 已开始继续处理当前任务。",
            json!({
                "entrypoint": "continue_assistant_run",
                "assistant_run_id": run_id,
                "stream": "sse",
                "status": "accepted",
            }),
        ),
    )
}

pub(crate) fn create_assistant_run_sse_completion(response: CreateAssistantRunResponse) -> String {
    create_assistant_run_sse_completion_with_delta(response, true)
}

pub(crate) fn create_assistant_run_sse_completion_with_delta(
    response: CreateAssistantRunResponse,
    emit_answer_delta: bool,
) -> String {
    let text = response.assistant_message.content.clone();
    let assistant_run_id = response.assistant_run_id;
    let completed_data = json!({
        "assistant_run_id": assistant_run_id,
        "response": response,
    });
    let mut encoded = if emit_answer_delta {
        sse_text_delta_events("assistant_run.delta", &text)
    } else {
        String::new()
    };
    encoded.push_str(&sse_json_event(
        "assistant_run.completed",
        assistant_run_sse_public_payload(
            Some(assistant_run_id),
            100,
            "completed",
            "completed",
            "本轮回复已生成。",
            completed_data,
        ),
    ));
    encoded.push_str(&sse_json_event("done", json!({"ok": true})));
    encoded
}

pub(crate) fn continue_assistant_run_sse_completion(
    response: ContinueAssistantRunResponse,
) -> String {
    continue_assistant_run_sse_completion_with_delta(response, true)
}

pub(crate) fn continue_assistant_run_sse_completion_with_delta(
    response: ContinueAssistantRunResponse,
    emit_answer_delta: bool,
) -> String {
    let text = response.assistant_message.content.clone();
    let assistant_run_id = response.run.id;
    let completed_data = json!({
        "assistant_run_id": assistant_run_id,
        "response": response,
    });
    let mut encoded = if emit_answer_delta {
        sse_text_delta_events("assistant_run.delta", &text)
    } else {
        String::new()
    };
    encoded.push_str(&sse_json_event(
        "assistant_run.completed",
        assistant_run_sse_public_payload(
            Some(assistant_run_id),
            100,
            "completed",
            "completed",
            "本轮继续处理已生成回复。",
            completed_data,
        ),
    ));
    encoded.push_str(&sse_json_event("done", json!({"ok": true})));
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assistant_run_sse_public_payload_flattens_data_without_overriding_envelope() {
        let payload = assistant_run_sse_public_payload(
            None,
            7,
            "started",
            "accepted",
            "DataMax 已开始处理本轮消息。",
            json!({
                "entrypoint": "create_assistant_run",
                "status": "shadow-status",
            }),
        );

        assert_eq!(
            payload.get("schema").and_then(Value::as_str),
            Some(ASSISTANT_RUN_SSE_SCHEMA_V1)
        );
        assert_eq!(
            payload.get("event_id").and_then(Value::as_str),
            Some("pending:000007")
        );
        assert_eq!(
            payload.get("status").and_then(Value::as_str),
            Some("accepted")
        );
        assert_eq!(
            payload.get("entrypoint").and_then(Value::as_str),
            Some("create_assistant_run")
        );
    }
}
