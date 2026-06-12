use axum::http::HeaderMap;
use domain_model::{AssistantRunEvent, AssistantRunId};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

pub(crate) const EXTERNAL_CHANNEL_SSE_SCHEMA_V1: &str = "v3.external_channel.sse.v1";
pub(crate) const EXTERNAL_CHANNEL_PUBLIC_STREAM_DEDUPE_KEY: &str = "_stream_dedupe_key";

pub(crate) fn external_channel_sse_envelope(
    run_id: Option<AssistantRunId>,
    idempotency_key: &str,
    conversation_external_id: &str,
    sequence: i64,
    phase: &str,
    status: &str,
    display_text: &str,
    status_url: Option<String>,
    poll_after_seconds: Option<u64>,
    data: Value,
) -> Value {
    let event_run_id = run_id
        .map(|run_id| run_id.to_string())
        .unwrap_or_else(|| "pending".to_string());
    json!({
        "schema": EXTERNAL_CHANNEL_SSE_SCHEMA_V1,
        "event_id": format!("{event_run_id}:{sequence:06}"),
        "sequence": sequence,
        "assistant_run_id": run_id,
        "idempotency_key": idempotency_key,
        "conversation_external_id": conversation_external_id,
        "phase": phase,
        "status": status,
        "display_text": display_text,
        "status_url": status_url,
        "poll_after_seconds": poll_after_seconds,
        "data": data,
    })
}

pub(crate) fn external_channel_sse_public_payload(
    run_id: Option<AssistantRunId>,
    idempotency_key: &str,
    conversation_external_id: &str,
    sequence: i64,
    phase: &str,
    status: &str,
    display_text: &str,
    status_url: Option<String>,
    poll_after_seconds: Option<u64>,
    data: Value,
) -> Value {
    let mut payload = external_channel_sse_envelope(
        run_id,
        idempotency_key,
        conversation_external_id,
        sequence,
        phase,
        status,
        display_text,
        status_url,
        poll_after_seconds,
        data.clone(),
    );
    if let (Some(payload), Some(data)) = (payload.as_object_mut(), data.as_object()) {
        for (key, value) in data {
            payload.entry(key.clone()).or_insert_with(|| value.clone());
        }
    }
    payload
}

pub(crate) fn external_channel_static_page_sse_sequence(status: &str) -> i64 {
    match status {
        "started" => 0,
        "retrieval_started" => 5,
        "answer_retrying" => 55,
        "static_page_planning" => 10,
        "static_page_image_preview_queued" | "static_page_generation_queued" => 20,
        "static_page_effect_image_ready" | "static_page_preview_ready" => 30,
        "static_page_publish_queued" => 40,
        "static_page_publish_running" => 41,
        "static_page_publish_retrying" => 42,
        "static_page_publish_failed" | "static_page_publish_needs_human" => 80,
        "static_page_publish_cancelled" => 81,
        "static_page_published" | "static_page_stable_artifact_reused" => 90,
        "needs_input" => 70,
        "continue_polling" | "static_page_continue_polling" => 95,
        "completed" => 100,
        _ => 50,
    }
}

pub(crate) fn external_channel_card_status_url(card: Option<&Value>) -> Option<String> {
    card.and_then(|card| card.get("status_url"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub(crate) fn external_channel_card_poll_after_seconds(card: Option<&Value>) -> Option<u64> {
    card.and_then(|card| card.get("poll_after_seconds"))
        .and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_str()?.trim().parse::<u64>().ok())
        })
}

pub(crate) fn external_channel_status_is_continuable(status: &str) -> bool {
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "accepted"
            | "queued"
            | "running"
            | "retrying"
            | "processing"
            | "continue_polling"
            | "static_page_continue_polling"
            | "static_page_generation_pending"
            | "static_page_generation_queued"
            | "static_page_generation_running"
            | "static_page_generation_retrying"
            | "static_page_image_preview_queued"
            | "static_page_image_preview_running"
            | "static_page_image_preview_retrying"
            | "static_page_image2_auto_publish_pending"
            | "static_page_image2_auto_publish_running"
            | "static_page_image2_auto_publish_retrying"
            | "static_page_publish_queued"
            | "static_page_publish_running"
            | "static_page_publish_retrying"
    )
}

pub(crate) fn external_channel_value_is_true(value: &Value) -> bool {
    match value {
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_u64().map(|value| value > 0).unwrap_or(false),
        Value::String(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "y" | "retryable" | "continue_polling"
        ),
        _ => false,
    }
}

pub(crate) fn external_channel_static_page_has_background_continuation(
    card: Option<&Value>,
) -> bool {
    let Some(card) = card else {
        return false;
    };

    if external_channel_card_poll_after_seconds(Some(card)).is_some() {
        return true;
    }

    for pointer in [
        "/retryable",
        "/background_continuation",
        "/backgroundContinuation",
        "/continue_polling",
        "/continuePolling",
        "/continues_in_background",
        "/continuesInBackground",
        "/runtime_event/retryable",
        "/runtime_event/background_continuation",
        "/runtime_event/backgroundContinuation",
        "/runtime_event/continue_polling",
        "/runtime_event/continuePolling",
        "/runtimeEvent/retryable",
        "/runtimeEvent/background_continuation",
        "/runtimeEvent/backgroundContinuation",
        "/runtimeEvent/continue_polling",
        "/runtimeEvent/continuePolling",
    ] {
        if card
            .pointer(pointer)
            .map(external_channel_value_is_true)
            .unwrap_or(false)
        {
            return true;
        }
    }

    for pointer in [
        "/workflow_status",
        "/workflowStatus",
        "/workflow/status",
        "/runtime_event/status",
        "/runtime_event/workflow_status",
        "/runtime_event/workflowStatus",
        "/runtimeEvent/status",
        "/runtimeEvent/workflow_status",
        "/runtimeEvent/workflowStatus",
    ] {
        if card
            .pointer(pointer)
            .and_then(Value::as_str)
            .map(external_channel_status_is_continuable)
            .unwrap_or(false)
        {
            return true;
        }
    }

    false
}

pub(crate) fn external_channel_static_page_cancelled_should_continue(
    status: &str,
    card: Option<&Value>,
) -> bool {
    status.trim() == "static_page_publish_cancelled"
        && external_channel_static_page_has_background_continuation(card)
}

fn parse_external_channel_stream_sequence_value(value: &Value) -> Option<i32> {
    value
        .as_i64()
        .or_else(|| value.as_str()?.trim().parse::<i64>().ok())
        .and_then(|sequence| i32::try_from(sequence.max(0)).ok())
}

fn parse_external_channel_stream_sequence_text(value: &str) -> Option<i32> {
    let candidate = value.trim().rsplit(':').next().unwrap_or_default().trim();
    candidate
        .parse::<i64>()
        .ok()
        .and_then(|sequence| i32::try_from(sequence.max(0)).ok())
}

pub(crate) fn parse_external_channel_stream_resume_sequence(
    headers: &HeaderMap,
    query: &HashMap<String, String>,
    payload: &Value,
) -> Option<i32> {
    headers
        .get("last-event-id")
        .and_then(|value| value.to_str().ok())
        .and_then(parse_external_channel_stream_sequence_text)
        .or_else(|| {
            query
                .get("since_sequence")
                .or_else(|| query.get("sinceSequence"))
                .and_then(|value| parse_external_channel_stream_sequence_text(value))
        })
        .or_else(|| {
            payload
                .get("stream_since_sequence")
                .or_else(|| payload.get("streamSinceSequence"))
                .and_then(parse_external_channel_stream_sequence_value)
        })
}

pub(crate) fn external_channel_public_stream_has_event(
    events: &[AssistantRunEvent],
    event_name: &str,
) -> bool {
    events.iter().any(|event| {
        event.event_name == event_name
            && event.payload.get("schema").and_then(Value::as_str)
                == Some(EXTERNAL_CHANNEL_SSE_SCHEMA_V1)
    })
}

pub(crate) fn external_channel_public_stream_dedupe_hash(value: &Value) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    format!("{digest:x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain_model::{AssistantRunEventId, TenantId};
    use uuid::Uuid;

    #[test]
    fn external_channel_sse_public_payload_preserves_envelope_fields_over_data() {
        let run_id = AssistantRunId::new();
        let payload = external_channel_sse_public_payload(
            Some(run_id),
            "idem-1",
            "conv-1",
            7,
            "processing",
            "static_page_generation_queued",
            "正在处理",
            Some("https://example.test/status".to_string()),
            Some(3),
            json!({
                "sequence": 999,
                "phase": "override",
                "custom": "kept"
            }),
        );

        assert_eq!(payload["schema"], json!(EXTERNAL_CHANNEL_SSE_SCHEMA_V1));
        assert_eq!(payload["event_id"], json!(format!("{run_id}:000007")));
        assert_eq!(payload["sequence"], json!(7));
        assert_eq!(payload["phase"], json!("processing"));
        assert_eq!(payload["custom"], json!("kept"));
        assert_eq!(payload["data"]["sequence"], json!(999));
    }

    #[test]
    fn external_channel_static_page_sequence_keeps_status_ordering_contract() {
        assert_eq!(external_channel_static_page_sse_sequence("started"), 0);
        assert_eq!(
            external_channel_static_page_sse_sequence("static_page_planning"),
            10
        );
        assert_eq!(
            external_channel_static_page_sse_sequence("static_page_published"),
            90
        );
        assert_eq!(external_channel_static_page_sse_sequence("completed"), 100);
        assert_eq!(external_channel_static_page_sse_sequence("unknown"), 50);
    }

    #[test]
    fn card_status_and_poll_fields_are_trimmed_and_leniently_parsed() {
        let card = json!({
            "status_url": "  https://example.test/status  ",
            "poll_after_seconds": " 15 "
        });

        assert_eq!(
            external_channel_card_status_url(Some(&card)).as_deref(),
            Some("https://example.test/status")
        );
        assert_eq!(
            external_channel_card_poll_after_seconds(Some(&card)),
            Some(15)
        );
    }

    #[test]
    fn cancelled_static_page_continues_only_with_background_signal() {
        let retryable_card = json!({
            "runtime_event": {
                "background_continuation": "retryable"
            }
        });
        let completed_card = json!({
            "runtime_event": {
                "status": "completed"
            }
        });

        assert!(external_channel_static_page_cancelled_should_continue(
            "static_page_publish_cancelled",
            Some(&retryable_card)
        ));
        assert!(!external_channel_static_page_cancelled_should_continue(
            "static_page_publish_cancelled",
            Some(&completed_card)
        ));
        assert!(!external_channel_static_page_cancelled_should_continue(
            "static_page_publish_failed",
            Some(&retryable_card)
        ));
    }

    #[test]
    fn stream_resume_sequence_prefers_header_then_query_then_payload() {
        let mut headers = HeaderMap::new();
        headers.insert("last-event-id", "run-1:000042".parse().unwrap());
        let mut query = HashMap::new();
        query.insert("sinceSequence".to_string(), "7".to_string());
        let payload = json!({"stream_since_sequence": 3});

        assert_eq!(
            parse_external_channel_stream_resume_sequence(&headers, &query, &payload),
            Some(42)
        );

        let headers = HeaderMap::new();
        assert_eq!(
            parse_external_channel_stream_resume_sequence(&headers, &query, &payload),
            Some(7)
        );

        let query = HashMap::new();
        assert_eq!(
            parse_external_channel_stream_resume_sequence(&headers, &query, &payload),
            Some(3)
        );
    }

    #[test]
    fn stream_resume_sequence_clamps_negative_and_accepts_camel_payload() {
        let headers = HeaderMap::new();
        let query = HashMap::new();
        let payload = json!({"streamSinceSequence": "-5"});

        assert_eq!(
            parse_external_channel_stream_resume_sequence(&headers, &query, &payload),
            Some(0)
        );
    }

    #[test]
    fn public_stream_has_event_requires_public_schema() {
        let run_id = AssistantRunId::new();
        let visible_event = AssistantRunEvent {
            id: AssistantRunEventId::new(),
            tenant_id: TenantId(Uuid::new_v4()),
            run_id,
            sequence_no: 1,
            event_name: "external_channel.completed".to_string(),
            payload: json!({"schema": EXTERNAL_CHANNEL_SSE_SCHEMA_V1}),
            created_at: chrono::Utc::now(),
        };
        let internal_event = AssistantRunEvent {
            event_name: "external_channel.completed".to_string(),
            payload: json!({"schema": "internal"}),
            ..visible_event.clone()
        };

        assert!(external_channel_public_stream_has_event(
            &[visible_event, internal_event],
            "external_channel.completed"
        ));
        assert!(!external_channel_public_stream_has_event(
            &[],
            "external_channel.completed"
        ));
    }

    #[test]
    fn public_stream_dedupe_hash_is_stable_for_same_payload() {
        let payload = json!({"status": "completed", "sequence": 3});

        assert_eq!(
            external_channel_public_stream_dedupe_hash(&payload),
            external_channel_public_stream_dedupe_hash(&payload)
        );
        assert_ne!(
            external_channel_public_stream_dedupe_hash(&payload),
            external_channel_public_stream_dedupe_hash(
                &json!({"status": "completed", "sequence": 4})
            )
        );
    }
}
