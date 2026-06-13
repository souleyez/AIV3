use contracts::ExternalConversationTimelineEventView;
use domain_model::AssistantRunEvent;
use serde_json::{json, Map, Value};
use uuid::Uuid;

use crate::{
    assistant_run_sse_support::ASSISTANT_RUN_SSE_SCHEMA_V1,
    copy_json_fields,
    external_channel_sse_support::{
        external_channel_public_stream_payload, EXTERNAL_CHANNEL_SSE_SCHEMA_V1,
    },
    external_integration_redacted_summary, truncate_assistant_supply_text, ApiError,
};

pub(crate) fn parse_external_conversation_event_id(
    raw: &str,
) -> std::result::Result<Uuid, ApiError> {
    Uuid::parse_str(raw.trim()).map_err(|_| {
        ApiError::bad_request(
            "invalid_external_conversation_test_event_id",
            format!("{raw} is not a valid UUID"),
        )
    })
}

pub(crate) fn external_conversation_timeline_event_view(
    event: AssistantRunEvent,
    include_debug: bool,
) -> ExternalConversationTimelineEventView {
    let payload = external_conversation_timeline_sanitized_payload(event.payload);
    let artifact_links = external_conversation_timeline_artifact_links(&payload);
    let phase = external_conversation_timeline_phase(&event.event_name, &payload);
    let status = external_conversation_timeline_status(&event.event_name, &payload);
    let display_text = external_conversation_timeline_display_text(&event.event_name, &payload);
    let payload_summary = external_conversation_timeline_payload_summary(
        &event.event_name,
        &payload,
        &artifact_links,
    );
    let debug_payload = include_debug.then_some(payload);

    ExternalConversationTimelineEventView {
        sequence_no: event.sequence_no,
        event_name: event.event_name,
        phase,
        status,
        display_text,
        artifact_links,
        payload_summary,
        debug_payload,
        created_at: event.created_at,
    }
}

fn external_conversation_timeline_sanitized_payload(payload: Value) -> Value {
    if payload.get("schema").and_then(Value::as_str) == Some(EXTERNAL_CHANNEL_SSE_SCHEMA_V1)
        || payload.get("schema").and_then(Value::as_str) == Some(ASSISTANT_RUN_SSE_SCHEMA_V1)
    {
        external_channel_public_stream_payload(payload)
    } else {
        external_integration_redacted_summary(payload)
    }
}

fn external_conversation_timeline_phase(event_name: &str, payload: &Value) -> Option<String> {
    external_conversation_timeline_string(
        payload,
        &["phase", "phase_name", "phaseName"],
        &["/data/phase", "/data/phase_name", "/data/phaseName"],
        80,
    )
    .or_else(|| {
        event_name
            .strip_prefix("assistant_run.")
            .and_then(|suffix| suffix.split('_').next())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn external_conversation_timeline_status(event_name: &str, payload: &Value) -> Option<String> {
    external_conversation_timeline_string(
        payload,
        &["status", "task_status", "taskStatus"],
        &[
            "/data/status",
            "/data/task_status",
            "/data/taskStatus",
            "/data/reply/task_status",
            "/data/reply/taskStatus",
            "/data/reply/card/status",
            "/reply/task_status",
            "/reply/card/status",
        ],
        80,
    )
    .or_else(|| {
        let event_name = event_name.to_ascii_lowercase();
        if event_name.contains("failed") {
            Some("failed".to_string())
        } else if event_name.contains("completed") || event_name.contains("published") {
            Some("completed".to_string())
        } else if event_name.contains("queued") {
            Some("queued".to_string())
        } else if event_name.contains("started") || event_name.contains("received") {
            Some("running".to_string())
        } else {
            None
        }
    })
}

fn external_conversation_timeline_display_text(
    event_name: &str,
    payload: &Value,
) -> Option<String> {
    external_conversation_timeline_string(
        payload,
        &["display_text", "displayText", "text", "message"],
        &[
            "/data/display_text",
            "/data/displayText",
            "/data/text",
            "/data/message",
            "/data/reply/text",
            "/data/reply/card/display_text",
            "/data/reply/card/displayText",
            "/reply/text",
            "/reply/card/display_text",
            "/reply/card/displayText",
        ],
        500,
    )
    .or_else(|| {
        if event_name.ends_with(".completed") {
            Some("本轮已完成。".to_string())
        } else if event_name.ends_with(".started") {
            Some("已开始处理。".to_string())
        } else {
            None
        }
    })
}

fn external_conversation_timeline_string(
    payload: &Value,
    keys: &[&str],
    pointers: &[&str],
    max_chars: usize,
) -> Option<String> {
    keys.iter()
        .filter_map(|key| payload.get(*key))
        .chain(
            pointers
                .iter()
                .filter_map(|pointer| payload.pointer(pointer)),
        )
        .filter_map(external_conversation_timeline_string_value)
        .map(|value| truncate_assistant_supply_text(&value, max_chars))
        .find(|value| !value.is_empty())
}

fn external_conversation_timeline_string_value(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => {
            let trimmed = value.trim();
            (!trimmed.is_empty() && trimmed != "[redacted]").then(|| trimmed.to_string())
        }
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

fn external_conversation_timeline_payload_summary(
    event_name: &str,
    payload: &Value,
    artifact_links: &[String],
) -> Value {
    let mut summary = Map::new();
    summary.insert(
        "event_name".to_string(),
        Value::String(event_name.to_string()),
    );
    copy_json_fields(
        payload,
        &mut summary,
        &[
            "schema",
            "event_id",
            "sequence",
            "phase",
            "status",
            "display_text",
            "status_url",
            "poll_after_seconds",
            "reply_type",
            "task_status",
        ],
    );
    if let Some(data) = payload.get("data") {
        copy_json_fields(
            data,
            &mut summary,
            &[
                "phase",
                "status",
                "display_text",
                "status_url",
                "poll_after_seconds",
            ],
        );
        if let Some(reply) = data.get("reply") {
            copy_json_fields(
                reply,
                &mut summary,
                &["reply_type", "task_status", "requires_confirmation"],
            );
        }
    }
    if !artifact_links.is_empty() {
        summary.insert("artifact_links".to_string(), json!(artifact_links));
    }
    if summary.len() == 1 {
        if let Some(object) = payload.as_object() {
            let keys: Vec<String> = object.keys().take(12).cloned().collect();
            if !keys.is_empty() {
                summary.insert("payload_keys".to_string(), json!(keys));
            }
        }
    }
    Value::Object(summary)
}

fn external_conversation_timeline_artifact_links(payload: &Value) -> Vec<String> {
    let mut links = Vec::new();
    external_conversation_timeline_collect_artifact_links(payload, &mut links, 0);
    links
}

fn external_conversation_timeline_collect_artifact_links(
    value: &Value,
    links: &mut Vec<String>,
    depth: usize,
) {
    if depth > 6 {
        return;
    }
    match value {
        Value::Array(items) => {
            for item in items {
                external_conversation_timeline_collect_artifact_links(item, links, depth + 1);
            }
        }
        Value::Object(object) => {
            for key in ["artifact_links", "artifactLinks"] {
                if let Some(value) = object.get(key) {
                    external_conversation_timeline_collect_link_value(value, links);
                }
            }
            for key in [
                "public_url",
                "publicUrl",
                "generated_artifact_url",
                "generatedArtifactUrl",
                "artifact_public_url",
                "artifactPublicUrl",
                "html_preview_url",
                "htmlPreviewUrl",
                "html_download_url",
                "htmlDownloadUrl",
                "download_url",
                "downloadUrl",
            ] {
                if let Some(value) = object.get(key) {
                    external_conversation_timeline_collect_link_value(value, links);
                }
            }
            for value in object.values() {
                external_conversation_timeline_collect_artifact_links(value, links, depth + 1);
            }
        }
        Value::String(_) => {}
        _ => {}
    }
}

fn external_conversation_timeline_collect_link_value(value: &Value, links: &mut Vec<String>) {
    match value {
        Value::String(value) => external_conversation_timeline_push_artifact_link(links, value),
        Value::Array(items) => {
            for item in items {
                external_conversation_timeline_collect_link_value(item, links);
            }
        }
        Value::Object(object) => {
            for key in ["url", "href", "public_url", "publicUrl"] {
                if let Some(value) = object.get(key) {
                    external_conversation_timeline_collect_link_value(value, links);
                }
            }
        }
        _ => {}
    }
}

fn external_conversation_timeline_push_artifact_link(links: &mut Vec<String>, value: &str) {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.starts_with("data:")
        || !(trimmed.starts_with("https://")
            || trimmed.starts_with("http://")
            || trimmed.starts_with('/'))
        || links.iter().any(|link| link == trimmed)
    {
        return;
    }
    links.push(trimmed.to_string());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::external_channel_sse_support::{
        external_channel_sse_public_payload, EXTERNAL_CHANNEL_PUBLIC_STREAM_DEDUPE_KEY,
    };
    use chrono::Utc;
    use domain_model::{AssistantRunEventId, AssistantRunId, TenantId};

    #[test]
    fn timeline_event_view_summarizes_public_stream_payload() {
        let run_id = AssistantRunId::new();
        let payload = external_channel_sse_public_payload(
            Some(run_id),
            "generic:tenant:stream-001",
            "room-1",
            42,
            "static_page",
            "processing",
            "页面正在发布。",
            Some("https://v3.elepcloud.com/status/run-1".to_string()),
            Some(15),
            json!({
                "card": {
                    "public_url": "https://v3.elepcloud.com/generated-artifacts/demo/index.html"
                },
                EXTERNAL_CHANNEL_PUBLIC_STREAM_DEDUPE_KEY: "internal-dedupe"
            }),
        );
        let view = external_conversation_timeline_event_view(
            AssistantRunEvent {
                id: AssistantRunEventId::new(),
                tenant_id: TenantId::new(),
                run_id,
                sequence_no: 7,
                event_name: "assistant_run.external_channel_static_page_publish_running"
                    .to_string(),
                payload,
                created_at: Utc::now(),
            },
            true,
        );

        assert_eq!(view.sequence_no, 7);
        assert_eq!(view.phase.as_deref(), Some("static_page"));
        assert_eq!(view.status.as_deref(), Some("processing"));
        assert_eq!(view.display_text.as_deref(), Some("页面正在发布。"));
        assert!(
            view.artifact_links.is_empty(),
            "unexpected artifact links: {:?}; payload: {:?}",
            view.artifact_links,
            view.debug_payload
        );
        assert_eq!(
            view.payload_summary["schema"],
            json!(EXTERNAL_CHANNEL_SSE_SCHEMA_V1)
        );
        assert!(view
            .debug_payload
            .as_ref()
            .and_then(|payload| payload.get(EXTERNAL_CHANNEL_PUBLIC_STREAM_DEDUPE_KEY))
            .is_none());
        assert!(view
            .debug_payload
            .as_ref()
            .and_then(|payload| payload.pointer("/data/card/public_url"))
            .is_none());
    }

    #[test]
    fn timeline_phase_status_and_display_text_fall_back_from_event_name() {
        let payload = json!({});
        assert_eq!(
            external_conversation_timeline_phase("assistant_run.retrieval_started", &payload)
                .as_deref(),
            Some("retrieval")
        );
        assert_eq!(
            external_conversation_timeline_status("assistant_run.task_queued", &payload).as_deref(),
            Some("queued")
        );
        assert_eq!(
            external_conversation_timeline_status("assistant_run.failed", &payload).as_deref(),
            Some("failed")
        );
        assert_eq!(
            external_conversation_timeline_display_text("assistant_run.completed", &payload)
                .as_deref(),
            Some("本轮已完成。")
        );
    }

    #[test]
    fn timeline_artifact_links_dedupe_and_reject_inline_or_unsafe_links() {
        let links = external_conversation_timeline_artifact_links(&json!({
            "artifact_links": [
                " https://v3.elepcloud.com/generated-artifacts/report/index.html ",
                "data:text/plain;base64,abc",
                "blob:https://v3.elepcloud.com/unsafe",
                "/generated-artifacts/report/table-data.csv"
            ],
            "nested": {
                "downloadUrl": {
                    "url": "https://v3.elepcloud.com/generated-artifacts/report/index.html"
                },
                "html_preview_url": "http://127.0.0.1/preview.html"
            }
        }));

        assert_eq!(
            links,
            vec![
                "https://v3.elepcloud.com/generated-artifacts/report/index.html".to_string(),
                "/generated-artifacts/report/table-data.csv".to_string(),
                "http://127.0.0.1/preview.html".to_string(),
            ]
        );
    }
}
