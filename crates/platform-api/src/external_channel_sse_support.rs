use axum::http::HeaderMap;
use contracts::{ExternalBotMessageView, ExternalChannelEventResponse};
use domain_model::{AssistantRunEvent, AssistantRunId};
use serde_json::{json, Map, Value};
use std::{collections::HashMap, time::Duration as StdDuration};

use crate::{
    codex_host_fixed_task_public_artifact_url_allowed, external_channel_api_public_base_url,
    external_channel_generated_artifact_public_base_url,
    external_channel_public_artifact::{
        external_channel_public_artifact_url_from_links_value,
        external_channel_public_artifact_url_from_reply,
        external_channel_public_artifact_url_from_value,
        external_channel_static_page_enrich_report_card,
        external_channel_text_with_public_artifact_link,
    },
    external_channel_public_card::{
        external_channel_public_card_value,
        external_channel_public_status_allows_artifact_link_for_card,
        external_channel_public_status_allows_preview_link,
        prune_external_channel_public_card_links,
    },
    external_channel_public_response,
    external_channel_public_text::{
        external_channel_public_status, external_channel_public_stream_text,
        external_channel_public_text,
    },
    external_channel_static_page_focus::{
        external_channel_static_page_customer_ready_text,
        external_channel_static_page_customer_ready_text_for_payload,
    },
    external_channel_static_page_published_reply, sha256_hex,
    sse_support::{sse_json_event, sse_text_delta_events},
    truncate_assistant_supply_text,
};

pub(crate) const EXTERNAL_CHANNEL_SSE_SCHEMA_V1: &str = "v3.external_channel.sse.v1";
pub(crate) const EXTERNAL_CHANNEL_PUBLIC_STREAM_DEDUPE_KEY: &str = "_stream_dedupe_key";
pub(crate) type ExternalChannelStaticPageSseProgressPayload =
    (&'static str, Value, String, String, Option<AssistantRunId>);
pub(crate) type ExternalChannelStaticPageSseContinuePollingPayload =
    (Value, String, String, Option<AssistantRunId>);

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

pub(crate) fn external_channel_retrieval_started_sse_event(
    message: &ExternalBotMessageView,
) -> String {
    let text = "DataMax 正在检索可见文档、数据源和会话上下文。";
    let data = json!({
        "conversation_external_id": message.conversation_external_id.clone(),
        "message_external_id": message.message_external_id.clone(),
        "idempotency_key": message.idempotency_key.clone(),
        "status": "processing",
        "text": text,
    });
    sse_json_event(
        "external_channel.retrieval_started",
        external_channel_sse_public_payload(
            None,
            &message.idempotency_key,
            &message.conversation_external_id,
            external_channel_static_page_sse_sequence("retrieval_started"),
            "retrieval",
            "processing",
            text,
            None,
            None,
            data,
        ),
    )
}

#[cfg(test)]
pub(crate) fn external_channel_sse_completion(response: ExternalChannelEventResponse) -> String {
    external_channel_sse_completion_with_done(response, true)
}

#[cfg(test)]
pub(crate) fn external_channel_sse_completion_with_done(
    response: ExternalChannelEventResponse,
    include_done: bool,
) -> String {
    let emit_static_page_queued_event = response
        .reply
        .card
        .as_ref()
        .and_then(|card| card.get("type"))
        .and_then(Value::as_str)
        == Some("v3_static_page_image2_pipeline");
    let response = external_channel_public_response(response);
    let text = response.reply.text.clone().unwrap_or_default();
    let assistant_run_id = response.assistant_run_id;
    let idempotency_key = response.idempotency_key.clone();
    let conversation_external_id = response.reply.target_conversation_external_id.clone();
    let mut encoded = sse_text_delta_events("external_channel.delta", &text);
    if emit_static_page_queued_event {
        let card = response.reply.card.clone();
        let data = json!({
            "assistant_run_id": assistant_run_id,
            "idempotency_key": idempotency_key,
            "card": card,
        });
        encoded.push_str(&sse_json_event(
            "external_channel.static_page_queued",
            external_channel_public_stream_payload(external_channel_sse_public_payload(
                assistant_run_id,
                &idempotency_key,
                &conversation_external_id,
                external_channel_static_page_sse_sequence("static_page_generation_queued"),
                "static_page",
                "static_page_generation_queued",
                "静态页/报表页面已进入生成队列。",
                external_channel_card_status_url(response.reply.card.as_ref()),
                external_channel_card_poll_after_seconds(response.reply.card.as_ref()),
                data,
            )),
        ));
    }
    if external_channel_response_needs_input(&response) {
        let needs_input_text = external_channel_needs_input_sse_text(&response);
        let data = json!({
            "assistant_run_id": assistant_run_id,
            "idempotency_key": idempotency_key,
            "status": "needs_input",
            "card": response.reply.card.clone(),
            "text": needs_input_text,
        });
        encoded.push_str(&sse_json_event(
            "external_channel.needs_input",
            external_channel_public_stream_payload(external_channel_sse_public_payload(
                assistant_run_id,
                &idempotency_key,
                &conversation_external_id,
                external_channel_static_page_sse_sequence("needs_input"),
                "needs_input",
                "needs_input",
                &needs_input_text,
                external_channel_card_status_url(response.reply.card.as_ref()),
                external_channel_card_poll_after_seconds(response.reply.card.as_ref()),
                data,
            )),
        ));
    }
    let completed_data = external_channel_completed_stream_data(
        &response,
        assistant_run_id,
        &idempotency_key,
        &text,
    );
    let completed_text = completed_data
        .get("text")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| {
            if text.trim().is_empty() {
                "本轮处理已返回当前结果。".to_string()
            } else {
                text.clone()
            }
        });
    encoded.push_str(&sse_json_event(
        "external_channel.completed",
        external_channel_public_stream_payload(external_channel_sse_public_payload(
            assistant_run_id,
            &idempotency_key,
            &conversation_external_id,
            external_channel_static_page_sse_sequence("completed"),
            "completed",
            "completed",
            &completed_text,
            None,
            None,
            completed_data,
        )),
    ));
    if include_done {
        encoded.push_str(&sse_json_event("done", json!({"ok": true})));
    }
    encoded
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

pub(crate) fn external_channel_static_page_provisional_existing_artifact(
    card: Option<&Value>,
) -> bool {
    card.and_then(|card| card.get("provisional_existing_artifact"))
        .and_then(Value::as_bool)
        == Some(true)
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
    sha256_hex([bytes.as_slice()])
}

pub(crate) fn external_channel_public_stream_payload(mut payload: Value) -> Value {
    if let Some(object) = payload.as_object_mut() {
        object.remove(EXTERNAL_CHANNEL_PUBLIC_STREAM_DEDUPE_KEY);
    }
    compact_external_channel_public_stream_payload(&mut payload);
    payload
}

fn compact_external_channel_public_stream_payload(payload: &mut Value) {
    let Some(object) = payload.as_object_mut() else {
        return;
    };
    if object.get("schema").and_then(Value::as_str) != Some(EXTERNAL_CHANNEL_SSE_SCHEMA_V1) {
        return;
    }

    let raw_data = object.get("data").cloned().unwrap_or(Value::Null);
    let status = object
        .get("status")
        .and_then(Value::as_str)
        .or_else(|| raw_data.get("status").and_then(Value::as_str))
        .or_else(|| {
            raw_data
                .get("response")
                .and_then(|response| response.get("reply"))
                .and_then(|reply| reply.get("task_status"))
                .and_then(Value::as_str)
        })
        .unwrap_or("processing");
    let phase = object
        .get("phase")
        .and_then(Value::as_str)
        .or_else(|| raw_data.get("phase").and_then(Value::as_str))
        .unwrap_or_default();
    let raw_card = object
        .get("card")
        .cloned()
        .or_else(|| raw_data.get("card").cloned())
        .or_else(|| {
            raw_data
                .get("response")
                .and_then(|response| response.get("reply"))
                .and_then(|reply| reply.get("card"))
                .cloned()
        });
    let raw_card = raw_card.map(|mut card| {
        external_channel_static_page_enrich_report_card(&mut card);
        card
    });
    let direct_artifact_url = external_channel_public_artifact_url_from_links_value(
        object
            .get("artifact_links")
            .or_else(|| raw_data.get("artifact_links")),
    );
    let include_artifact_link =
        external_channel_public_status_allows_artifact_link_for_card(status, raw_card.as_ref())
            || (phase == "completed" && direct_artifact_url.is_some());
    let include_preview_link = external_channel_public_status_allows_preview_link(status);
    let display_text = object
        .get("display_text")
        .and_then(Value::as_str)
        .or_else(|| raw_data.get("text").and_then(Value::as_str))
        .unwrap_or("DataMax 正在处理。");
    let mut display_text = external_channel_public_stream_text(display_text);
    if !include_artifact_link
        && !include_preview_link
        && display_text.contains("generated-artifacts/")
    {
        display_text =
            "DataMax 已返回当前处理状态，页面仍在后台继续生成；第三方请按 status_url 继续轮询，完成后会返回最终页面链接。".to_string();
    }
    object.insert(
        "display_text".to_string(),
        Value::String(display_text.clone()),
    );

    let card = raw_card
        .as_ref()
        .map(|card| {
            external_channel_public_stream_card_summary(
                card,
                include_artifact_link,
                include_preview_link,
            )
        })
        .filter(|card| !card.as_object().map(Map::is_empty).unwrap_or(false));

    let public_url = if include_artifact_link {
        raw_card
            .as_ref()
            .and_then(external_channel_public_artifact_url_from_value)
            .or_else(|| direct_artifact_url.clone())
            .or_else(|| {
                raw_data
                    .get("response")
                    .and_then(|response| response.get("reply"))
                    .and_then(|reply| {
                        reply
                            .get("artifact_links")
                            .and_then(Value::as_array)
                            .and_then(|links| {
                                links.iter().find_map(|link| {
                                    link.as_str()
                                        .map(str::trim)
                                        .filter(|url| {
                                            codex_host_fixed_task_public_artifact_url_allowed(url)
                                        })
                                        .map(ToOwned::to_owned)
                                })
                            })
                    })
            })
    } else {
        None
    };

    let mut compact_data = Map::new();
    for key in ["assistant_run_id", "idempotency_key", "status", "phase"] {
        if let Some(value) = object
            .get(key)
            .cloned()
            .or_else(|| raw_data.get(key).cloned())
        {
            compact_data.insert(key.to_string(), value);
        }
    }
    compact_data.insert("text".to_string(), Value::String(display_text));
    if let Some(status_url) = object
        .get("status_url")
        .cloned()
        .or_else(|| raw_data.get("status_url").cloned())
        .or_else(|| {
            raw_card
                .as_ref()
                .and_then(|card| card.get("status_url"))
                .cloned()
        })
    {
        compact_data.insert("status_url".to_string(), status_url.clone());
        object.insert("status_url".to_string(), status_url);
    }
    if let Some(poll_after_seconds) = object
        .get("poll_after_seconds")
        .cloned()
        .or_else(|| raw_data.get("poll_after_seconds").cloned())
        .or_else(|| {
            raw_card
                .as_ref()
                .and_then(|card| card.get("poll_after_seconds"))
                .cloned()
        })
    {
        compact_data.insert("poll_after_seconds".to_string(), poll_after_seconds.clone());
        object.insert("poll_after_seconds".to_string(), poll_after_seconds);
    }
    if let Some(card) = card {
        compact_data.insert("card".to_string(), card.clone());
        object.insert("card".to_string(), card);
    } else {
        object.remove("card");
    }
    if let Some(public_url) = public_url {
        compact_data.insert("public_url".to_string(), Value::String(public_url.clone()));
        compact_data.insert("artifact_links".to_string(), json!([public_url.clone()]));
        object.insert("public_url".to_string(), Value::String(public_url.clone()));
        object.insert("artifact_links".to_string(), json!([public_url]));
    } else {
        object.remove("public_url");
        object.remove("artifact_links");
    }

    object.insert("data".to_string(), Value::Object(compact_data));
    for key in [
        "response",
        "modules",
        "data_snapshot",
        "style_direction",
        "summary",
        "source_refs",
        "runtime_event",
    ] {
        object.remove(key);
    }
}

fn external_channel_public_stream_card_summary(
    card: &Value,
    include_artifact_link: bool,
    include_preview_link: bool,
) -> Value {
    let mut enriched = card.clone();
    external_channel_static_page_enrich_report_card(&mut enriched);
    let mut sanitized = external_channel_public_card_value(enriched);
    prune_external_channel_public_card_links(
        &mut sanitized,
        include_artifact_link,
        include_preview_link,
    );
    let Some(input) = sanitized.as_object() else {
        return Value::Null;
    };
    let mut output = Map::new();
    for key in [
        "type",
        "status",
        "draft_id",
        "status_url",
        "poll_after_seconds",
        "public_url",
        "generated_artifact_url",
        "download_url",
        "html_download_url",
        "artifact_links",
        "data_url",
        "data_snapshot_url",
        "table_data_url",
        "ppt_download_url",
        "markdown_download_url",
        "text_download_url",
        "download_exports",
        "title",
        "report_title",
        "display_title",
        "preview_url",
        "requires_confirmation",
        "can_continue_same_conversation",
        "resume_action",
    ] {
        if let Some(value) = input.get(key).cloned() {
            output.insert(key.to_string(), value);
        }
    }
    Value::Object(output)
}

#[cfg(test)]
pub(crate) fn external_channel_public_stream_events(
    events: &[AssistantRunEvent],
) -> Vec<(String, Value)> {
    events
        .iter()
        .filter(|event| {
            event.payload.get("schema").and_then(Value::as_str)
                == Some(EXTERNAL_CHANNEL_SSE_SCHEMA_V1)
        })
        .map(|event| {
            (
                event.event_name.clone(),
                external_channel_public_stream_payload(event.payload.clone()),
            )
        })
        .collect()
}

pub(crate) fn external_channel_public_stream_replay_body(
    events: &[AssistantRunEvent],
    since_sequence: Option<i32>,
) -> String {
    let since_sequence = since_sequence.unwrap_or(0);
    let mut encoded = String::new();
    for event in events
        .iter()
        .filter(|event| event.sequence_no > since_sequence)
    {
        if event.payload.get("schema").and_then(Value::as_str)
            != Some(EXTERNAL_CHANNEL_SSE_SCHEMA_V1)
        {
            continue;
        }
        encoded.push_str(&sse_json_event(
            &event.event_name,
            external_channel_public_stream_payload(event.payload.clone()),
        ));
    }
    encoded
}

pub(crate) fn external_channel_completed_stream_data(
    response: &ExternalChannelEventResponse,
    assistant_run_id: Option<AssistantRunId>,
    idempotency_key: &str,
    text: &str,
) -> Value {
    let status = response
        .reply
        .task_status
        .as_deref()
        .or_else(|| {
            response
                .reply
                .card
                .as_ref()
                .and_then(|card| card.get("status"))
                .and_then(Value::as_str)
        })
        .unwrap_or("completed");
    json!({
        "assistant_run_id": assistant_run_id,
        "idempotency_key": idempotency_key,
        "status": external_channel_public_status(status),
        "reply_type": response.reply.reply_type.clone(),
        "text": if text.trim().is_empty() {
            "本轮处理已返回当前结果。".to_string()
        } else {
            external_channel_public_stream_text(text)
        },
        "card": response.reply.card.clone(),
        "artifact_links": response.reply.artifact_links.clone(),
    })
}

pub(crate) fn external_channel_response_needs_input(
    response: &ExternalChannelEventResponse,
) -> bool {
    response.reply.task_status.as_deref() == Some("needs_input")
        || response
            .reply
            .card
            .as_ref()
            .and_then(|card| card.get("status"))
            .and_then(Value::as_str)
            == Some("needs_input")
}

pub(crate) fn external_channel_needs_input_sse_text(
    response: &ExternalChannelEventResponse,
) -> String {
    response
        .reply
        .text
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(external_channel_public_text)
        .unwrap_or_else(|| "还需要补充信息后继续处理。".to_string())
}

pub(crate) fn external_channel_response_is_static_page_pipeline(
    response: &ExternalChannelEventResponse,
) -> bool {
    response
        .reply
        .card
        .as_ref()
        .and_then(|card| card.get("type"))
        .and_then(Value::as_str)
        .map(|card_type| {
            card_type.starts_with("v3_static_page_image2")
                || card_type == "v3_static_page_image2_pipeline"
        })
        .unwrap_or(false)
}

pub(crate) fn external_channel_static_page_sse_status(
    response: &ExternalChannelEventResponse,
) -> String {
    let raw_status = response
        .reply
        .card
        .as_ref()
        .and_then(|card| card.get("status"))
        .and_then(Value::as_str)
        .or(response.reply.task_status.as_deref())
        .unwrap_or("processing")
        .to_string();
    if !external_channel_static_page_provisional_existing_artifact(response.reply.card.as_ref())
        && external_channel_public_artifact_url_from_reply(&response.reply).is_some()
    {
        "static_page_published".to_string()
    } else if external_channel_static_page_cancelled_should_continue(
        &raw_status,
        response.reply.card.as_ref(),
    ) {
        "static_page_continue_polling".to_string()
    } else {
        raw_status
    }
}

pub(crate) fn external_channel_static_page_sse_progress_text(
    response: &ExternalChannelEventResponse,
) -> String {
    let status = external_channel_static_page_sse_status(response);
    if let Some(public_url) = external_channel_public_artifact_url_from_reply(&response.reply) {
        if status == "static_page_published" {
            return external_channel_text_with_public_artifact_link(
                external_channel_static_page_customer_ready_text_for_payload(
                    external_channel_static_page_customer_ready_text(),
                    response.reply.card.as_ref(),
                    &public_url,
                ),
                &public_url,
            );
        }
        if status == "static_page_stable_artifact_reused" {
            return external_channel_text_with_public_artifact_link(
                external_channel_static_page_customer_ready_text_for_payload(
                    external_channel_static_page_customer_ready_text(),
                    response.reply.card.as_ref(),
                    &public_url,
                ),
                &public_url,
            );
        }
    }
    if let Some(text) = response
        .reply
        .text
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        return external_channel_public_text(text);
    }
    match status.as_str() {
        "static_page_effect_image_ready" => {
            "页面可视化预览已生成，DataMax 将继续生成最终静态页。".to_string()
        }
        "static_page_publish_queued" => {
            "下一步：DataMax 正在整理数据证据并生成最终静态页。".to_string()
        }
        "static_page_publish_running" => "下一步：DataMax 正在生成可访问的静态页。".to_string(),
        "static_page_publish_retrying" => {
            "静态页发布遇到临时波动，DataMax 会继续重试或切换可用发布链路。".to_string()
        }
        "static_page_published" => external_channel_static_page_customer_ready_text().to_string(),
        "static_page_publish_failed" => {
            "静态页最终发布暂未完成，DataMax 已记录原因，可继续重试或人工接管。".to_string()
        }
        "static_page_publish_needs_human" => {
            "静态页发布需要人工处理，DataMax 已保留当前中间产物和原因。".to_string()
        }
        "static_page_publish_cancelled" => {
            "静态页发布任务已取消，DataMax 已保留当前中间状态。".to_string()
        }
        "static_page_continue_polling" => {
            "静态页生成仍在后台继续，第三方可按 status_url 继续轮询。".to_string()
        }
        _ => "DataMax 静态页任务仍在处理中。".to_string(),
    }
}

pub(crate) fn external_channel_static_page_sse_progress_key(
    response: &ExternalChannelEventResponse,
) -> String {
    let status = external_channel_static_page_sse_status(response);
    let card = response.reply.card.as_ref();
    let public_url = card
        .and_then(|card| {
            card.get("public_url")
                .or_else(|| card.get("generated_artifact_url"))
                .or_else(|| card.get("artifact_public_url"))
        })
        .and_then(Value::as_str)
        .unwrap_or_default();
    let preview = card
        .and_then(|card| {
            card.get("preview_asset_key")
                .or_else(|| card.get("preview_url"))
                .or_else(|| card.get("render_asset_url"))
        })
        .and_then(Value::as_str)
        .unwrap_or_default();
    let runtime = card
        .and_then(|card| card.get("runtime_event"))
        .and_then(|event| {
            event
                .get("heartbeat_count")
                .or_else(|| event.get("elapsed_ms"))
                .or_else(|| event.get("reason"))
        })
        .map(Value::to_string)
        .unwrap_or_default();
    format!("{status}|{public_url}|{preview}|{runtime}")
}

pub(crate) fn external_channel_static_page_preview_public_url(asset_ref: &str) -> Option<String> {
    let trimmed = asset_ref.trim();
    if trimmed.is_empty() || trimmed.starts_with("data:image/") || trimmed.starts_with("blob:") {
        return None;
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        if let Ok(mut url) = reqwest::Url::parse(trimmed) {
            url.set_query(None);
            url.set_fragment(None);
            return Some(truncate_assistant_supply_text(url.as_str(), 500));
        }
        return None;
    }
    if trimmed.starts_with("/generated-artifacts/") {
        return Some(format!(
            "{}{}",
            external_channel_api_public_base_url(),
            truncate_assistant_supply_text(trimmed, 500)
        ));
    }
    if trimmed.starts_with("generated-artifacts/") {
        return Some(format!(
            "{}/{}",
            external_channel_api_public_base_url(),
            truncate_assistant_supply_text(trimmed, 500)
        ));
    }
    if trimmed.starts_with("static-page-previews/") {
        return Some(format!(
            "{}/{}",
            external_channel_generated_artifact_public_base_url(),
            truncate_assistant_supply_text(trimmed, 500)
        ));
    }
    Some(truncate_assistant_supply_text(trimmed, 500))
}

pub(crate) fn external_channel_static_page_sse_progress_payload(
    response: ExternalChannelEventResponse,
) -> ExternalChannelStaticPageSseProgressPayload {
    let status = external_channel_static_page_sse_status(&response);
    let progress_key = external_channel_static_page_sse_progress_key(&response);
    let text = external_channel_static_page_sse_progress_text(&response);
    let event_name = external_channel_static_page_sse_event_name(&status);
    let mut public_response = external_channel_public_response(response);
    let public_status = external_channel_public_status(&status);
    let conversation_external_id = public_response
        .reply
        .target_conversation_external_id
        .clone();
    if public_status == "static_page_published" {
        if let Some(public_url) =
            external_channel_public_artifact_url_from_reply(&public_response.reply)
        {
            let payload = public_response.reply.card.clone().unwrap_or_else(|| {
                json!({
                    "public_url": public_url.clone(),
                    "artifact_links": [public_url.clone()],
                })
            });
            let published_reply = external_channel_static_page_published_reply(
                &conversation_external_id,
                &public_url,
                &payload,
            );
            public_response.reply.card = published_reply.card;
            public_response.reply.artifact_links = published_reply.artifact_links;
        }
    }
    let status_url = external_channel_card_status_url(public_response.reply.card.as_ref());
    let poll_after_seconds =
        external_channel_card_poll_after_seconds(public_response.reply.card.as_ref());
    let data = json!({
        "assistant_run_id": public_response.assistant_run_id,
        "idempotency_key": public_response.idempotency_key.clone(),
        "status": public_status.clone(),
        "card": public_response.reply.card.clone(),
        "artifact_links": public_response.reply.artifact_links.clone(),
        "text": text.clone(),
    });
    let payload = external_channel_sse_public_payload(
        public_response.assistant_run_id,
        &public_response.idempotency_key,
        &conversation_external_id,
        external_channel_static_page_sse_sequence(&status),
        "static_page",
        &public_status,
        &text,
        status_url,
        poll_after_seconds,
        data,
    );
    let dedupe_key = format!("{event_name}:{public_status}:{progress_key}");
    (
        event_name,
        payload,
        text,
        dedupe_key,
        public_response.assistant_run_id,
    )
}

#[cfg(test)]
pub(crate) fn external_channel_static_page_sse_progress_events(
    response: ExternalChannelEventResponse,
) -> String {
    let (event_name, payload, text, _, _) =
        external_channel_static_page_sse_progress_payload(response);
    external_channel_sse_event_with_delta(event_name, payload, &text)
}

pub(crate) fn external_channel_static_page_sse_continue_polling_payload(
    response: ExternalChannelEventResponse,
) -> ExternalChannelStaticPageSseContinuePollingPayload {
    let status = external_channel_static_page_sse_status(&response);
    let progress_key = external_channel_static_page_sse_progress_key(&response);
    let public_response = external_channel_public_response(response);
    let public_status = external_channel_public_status(&status);
    let conversation_external_id = public_response
        .reply
        .target_conversation_external_id
        .clone();
    let status_url = external_channel_card_status_url(public_response.reply.card.as_ref());
    let poll_after_seconds =
        external_channel_card_poll_after_seconds(public_response.reply.card.as_ref());
    let text = "本次流式连接已达到等待上限，DataMax 会继续后台生成；第三方请按 status_url 继续轮询，完成后会返回最终页面链接。";
    let data = json!({
        "assistant_run_id": public_response.assistant_run_id,
        "idempotency_key": public_response.idempotency_key.clone(),
        "status": public_status.clone(),
        "card": public_response.reply.card.clone(),
        "text": text,
    });
    let payload = external_channel_sse_public_payload(
        public_response.assistant_run_id,
        &public_response.idempotency_key,
        &conversation_external_id,
        external_channel_static_page_sse_sequence("static_page_continue_polling"),
        "static_page",
        &public_status,
        text,
        status_url,
        poll_after_seconds.or(Some(30)),
        data,
    );
    let dedupe_key =
        format!("external_channel.static_page_continue_polling:{public_status}:{progress_key}");
    (
        payload,
        text.to_string(),
        dedupe_key,
        public_response.assistant_run_id,
    )
}

#[cfg(test)]
pub(crate) fn external_channel_static_page_sse_continue_polling_event(
    response: ExternalChannelEventResponse,
) -> String {
    let (payload, text, _, _) = external_channel_static_page_sse_continue_polling_payload(response);
    external_channel_sse_event_with_delta(
        "external_channel.static_page_continue_polling",
        payload,
        &text,
    )
}

pub(crate) fn external_channel_static_page_sse_is_terminal(
    response: &ExternalChannelEventResponse,
) -> bool {
    let status = external_channel_static_page_sse_status(response);
    matches!(
        status.as_str(),
        "static_page_published"
            | "static_page_stable_artifact_reused"
            | "static_page_publish_cancelled"
            | "static_page_publish_failed"
            | "static_page_publish_needs_human"
    )
}

pub(crate) fn external_channel_static_page_sse_event_name(status: &str) -> &'static str {
    match status {
        "static_page_effect_image_ready" => "external_channel.static_page_preview_ready",
        "static_page_published" | "static_page_stable_artifact_reused" => {
            "external_channel.static_page_published"
        }
        "static_page_publish_failed"
        | "static_page_publish_cancelled"
        | "static_page_publish_needs_human" => "external_channel.static_page_issue",
        "static_page_continue_polling" => "external_channel.static_page_continue_polling",
        "static_page_publish_queued"
        | "static_page_publish_running"
        | "static_page_publish_retrying" => "external_channel.static_page_publish_progress",
        _ => "external_channel.static_page_progress",
    }
}

pub(crate) fn external_channel_sse_event_with_delta(
    event_name: &str,
    payload: Value,
    text: &str,
) -> String {
    let payload = external_channel_public_stream_payload(payload);
    let mut encoded = sse_text_delta_events("external_channel.delta", text);
    encoded.push_str(&sse_json_event(event_name, payload));
    encoded
}

pub(crate) fn external_channel_answer_retrying_text(reason: &str) -> &'static str {
    match reason {
        "gateway_limit" => "模型通道繁忙，DataMax 正在切换可用通道继续回答。",
        "provider_timeout" => "本次模型回答较慢，DataMax 正在重试或切换通道。",
        "answer_rejected" => "模型回复未达到可展示要求，DataMax 正在重新生成回答。",
        "provider_retry" | "provider_error" => "模型通道暂时不可用，DataMax 正在重试或切换通道。",
        "runtime_unavailable" => "当前模型通道暂不可用，DataMax 正在寻找可用通道继续回答。",
        _ => "DataMax 正在重试或切换可用通道继续回答。",
    }
}

fn external_channel_duration_from_millis_env_value(
    value: Option<String>,
    default_millis: u64,
) -> StdDuration {
    StdDuration::from_millis(
        value
            .and_then(|value| value.trim().parse::<u64>().ok())
            .unwrap_or(default_millis),
    )
}

pub(crate) fn external_channel_static_page_sse_poll_interval() -> StdDuration {
    external_channel_duration_from_millis_env_value(
        std::env::var("EXTERNAL_CHANNEL_STATIC_PAGE_SSE_POLL_MS").ok(),
        5_000,
    )
}

pub(crate) fn external_channel_static_page_sse_timeout() -> StdDuration {
    external_channel_duration_from_millis_env_value(
        std::env::var("EXTERNAL_CHANNEL_STATIC_PAGE_SSE_TIMEOUT_MS").ok(),
        120_000,
    )
}

fn external_channel_env_flag_is_enabled(value: Option<String>) -> bool {
    value
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

pub(crate) fn external_channel_live_answer_stream_enabled() -> bool {
    external_channel_env_flag_is_enabled(
        std::env::var("EXTERNAL_CHANNEL_LIVE_ANSWER_STREAM_ENABLED").ok(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::{
        ExternalBotReplyTypeView, ExternalBotReplyView, ExternalChannelPlatformView,
        ExternalMessageTypeView,
    };
    use domain_model::{AssistantRunEventId, TenantId};
    use uuid::Uuid;

    fn assistant_run_event(
        run_id: AssistantRunId,
        sequence_no: i32,
        event_name: &str,
        payload: Value,
    ) -> AssistantRunEvent {
        AssistantRunEvent {
            id: AssistantRunEventId::new(),
            tenant_id: TenantId(Uuid::new_v4()),
            run_id,
            sequence_no,
            event_name: event_name.to_string(),
            payload,
            created_at: chrono::Utc::now(),
        }
    }

    fn external_channel_response(
        card: Option<Value>,
        task_status: Option<&str>,
        artifact_links: Vec<String>,
    ) -> ExternalChannelEventResponse {
        ExternalChannelEventResponse {
            accepted: true,
            assistant_run_id: None,
            idempotency_key: "idem-static-page".to_string(),
            reply: ExternalBotReplyView {
                target_conversation_external_id: "conv-1".to_string(),
                reply_type: ExternalBotReplyTypeView::TaskStatus,
                text: None,
                card,
                artifact_links,
                task_status: task_status.map(ToOwned::to_owned),
                requires_confirmation: false,
                action_id: None,
                confirmation_id: None,
            },
        }
    }

    fn external_bot_message() -> ExternalBotMessageView {
        ExternalBotMessageView {
            platform: ExternalChannelPlatformView::GenericChat,
            tenant_external_id: "tenant-1".to_string(),
            bot_external_id: "bot-1".to_string(),
            conversation_external_id: "conv-1".to_string(),
            thread_external_id: None,
            sender_external_id: "sender-1".to_string(),
            message_external_id: "msg-1".to_string(),
            message_type: ExternalMessageTypeView::Text,
            text: Some("查询文档".to_string()),
            default_prompt: None,
            output_format: None,
            render_mode: None,
            artifact_type: None,
            template: None,
            mention_external_user_ids: Vec::new(),
            attachment_refs: Vec::new(),
            business_datasource_ids: Vec::new(),
            available_document_external_ids: Vec::new(),
            available_document_source_id: None,
            dataset_external_id: None,
            dataset_external_ids: Vec::new(),
            requested_skills: Vec::new(),
            idempotency_key: "idem-retrieval".to_string(),
            received_at: chrono::Utc::now(),
        }
    }

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
    fn sse_completion_with_done_emits_queued_and_completed_events() {
        let mut response = external_channel_response(
            Some(json!({
                "type": "v3_static_page_image2_pipeline",
                "status": "static_page_generation_queued",
                "draft_id": "draft-1",
                "poll_after_seconds": 5,
            })),
            Some("processing"),
            Vec::new(),
        );
        response.assistant_run_id = Some(AssistantRunId::new());
        response.idempotency_key = "generic:tenant:completion".to_string();
        response.reply.text = Some("页面任务已接收。".to_string());

        let body = external_channel_sse_completion_with_done(response, false);

        assert!(body.contains("event: external_channel.delta"));
        assert!(body.contains("event: external_channel.static_page_queued"));
        assert!(body.contains("\"sequence\":20"));
        assert!(body.contains("event: external_channel.completed"));
        assert!(body.contains("\"sequence\":100"));
        assert!(body.contains("页面任务已接收。"));
        assert!(!body.contains("event: done"));
    }

    #[test]
    fn retrieval_started_sse_event_keeps_public_wire_contract() {
        let body = external_channel_retrieval_started_sse_event(&external_bot_message());

        assert!(body.contains("event: external_channel.retrieval_started"));
        assert!(body.contains("\"schema\":\"v3.external_channel.sse.v1\""));
        assert!(body.contains("\"sequence\":5"));
        assert!(body.contains("\"phase\":\"retrieval\""));
        assert!(body.contains("\"status\":\"processing\""));
        assert!(body.contains("\"conversation_external_id\":\"conv-1\""));
        assert!(body.contains("\"message_external_id\":\"msg-1\""));
        assert!(body.contains("\"idempotency_key\":\"idem-retrieval\""));
        assert!(body.contains("DataMax 正在检索可见文档、数据源和会话上下文。"));
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

    #[test]
    fn public_stream_payload_removes_dedupe_key_and_compacts_internal_fields() {
        let payload = external_channel_sse_public_payload(
            None,
            "idem-1",
            "conv-1",
            90,
            "static_page",
            "static_page_published",
            "报表已生成。",
            Some("https://v3.elepcloud.com/status/run-1".to_string()),
            None,
            json!({
                EXTERNAL_CHANNEL_PUBLIC_STREAM_DEDUPE_KEY: "internal-dedupe",
                "card": {
                    "type": "v3_static_page_pipeline",
                    "status": "static_page_published",
                    "draft_id": "draft-1",
                    "public_url": "https://v3.elepcloud.com/generated-artifacts/demo/index.html",
                    "data_snapshot": {"rows": [1, 2, 3]},
                    "runtime_event": {"debug": true}
                },
                "response": {"reply": {"text": "internal full response"}},
                "modules": [{"id": "large"}],
                "runtime_event": {"debug": true}
            }),
        );

        let payload = external_channel_public_stream_payload(payload);
        let encoded = payload.to_string();

        assert!(payload
            .get(EXTERNAL_CHANNEL_PUBLIC_STREAM_DEDUPE_KEY)
            .is_none());
        assert!(payload.get("response").is_none());
        assert!(payload.get("modules").is_none());
        assert!(payload.get("runtime_event").is_none());
        assert!(!encoded.contains("internal-dedupe"));
        assert!(!encoded.contains("internal full response"));
        assert_eq!(
            payload["public_url"],
            json!("https://v3.elepcloud.com/generated-artifacts/demo/index.html")
        );
        assert_eq!(
            payload.pointer("/data/card/public_url"),
            Some(&json!(
                "https://v3.elepcloud.com/generated-artifacts/demo/index.html"
            ))
        );
    }

    #[test]
    fn public_stream_replay_body_filters_since_sequence_and_public_schema() {
        let run_id = AssistantRunId::new();
        let old_payload = external_channel_sse_public_payload(
            Some(run_id),
            "idem-1",
            "conv-1",
            1,
            "static_page",
            "processing",
            "old event",
            None,
            None,
            json!({}),
        );
        let internal_payload = json!({
            "schema": "internal",
            "display_text": "internal event"
        });
        let new_payload = external_channel_sse_public_payload(
            Some(run_id),
            "idem-1",
            "conv-1",
            3,
            "completed",
            "completed",
            "new event",
            None,
            None,
            json!({}),
        );
        let events = vec![
            assistant_run_event(run_id, 1, "external_channel.progress", old_payload),
            assistant_run_event(run_id, 2, "external_channel.internal", internal_payload),
            assistant_run_event(run_id, 3, "external_channel.completed", new_payload),
        ];

        let replay = external_channel_public_stream_replay_body(&events, Some(1));

        assert!(!replay.contains("old event"));
        assert!(!replay.contains("internal event"));
        assert!(!replay.contains("external_channel.internal"));
        assert!(replay.contains("event: external_channel.completed"));
        assert!(replay.contains("new event"));
    }

    #[test]
    fn completed_stream_data_prefers_task_status_and_sanitizes_text() {
        let run_id = AssistantRunId::new();
        let response = ExternalChannelEventResponse {
            accepted: true,
            assistant_run_id: Some(run_id),
            idempotency_key: "idem-1".to_string(),
            reply: ExternalBotReplyView {
                target_conversation_external_id: "conv-1".to_string(),
                reply_type: ExternalBotReplyTypeView::TaskStatus,
                text: None,
                card: Some(json!({"status": "static_page_published"})),
                artifact_links: vec![
                    "https://v3.elepcloud.com/generated-artifacts/demo/index.html".to_string(),
                ],
                task_status: Some("static_page_effect_image_ready".to_string()),
                requires_confirmation: false,
                action_id: None,
                confirmation_id: None,
            },
        };

        let data = external_channel_completed_stream_data(
            &response,
            Some(run_id),
            "idem-1",
            "Cloudflare Codex 已完成",
        );

        assert_eq!(data["assistant_run_id"], json!(run_id));
        assert_eq!(data["idempotency_key"], json!("idem-1"));
        assert_eq!(data["status"], json!("static_page_preview_ready"));
        assert_eq!(data["text"], json!("DataMax 后台 已完成"));
        assert_eq!(
            data["artifact_links"],
            json!(["https://v3.elepcloud.com/generated-artifacts/demo/index.html"])
        );
    }

    #[test]
    fn completed_stream_data_falls_back_to_card_status_and_default_text() {
        let response = ExternalChannelEventResponse {
            accepted: true,
            assistant_run_id: None,
            idempotency_key: "idem-2".to_string(),
            reply: ExternalBotReplyView {
                target_conversation_external_id: "conv-1".to_string(),
                reply_type: ExternalBotReplyTypeView::ArtifactLink,
                text: None,
                card: Some(json!({"status": "static_page_stable_artifact_reused"})),
                artifact_links: Vec::new(),
                task_status: None,
                requires_confirmation: false,
                action_id: None,
                confirmation_id: None,
            },
        };

        let data = external_channel_completed_stream_data(&response, None, "idem-2", "  ");

        assert_eq!(data["assistant_run_id"], Value::Null);
        assert_eq!(data["status"], json!("static_page_stable_artifact_reused"));
        assert_eq!(data["text"], json!("本轮处理已返回当前结果。"));
        assert_eq!(data["reply_type"], json!("artifact_link"));
    }

    #[test]
    fn response_needs_input_checks_task_status_or_card_status() {
        let from_task_status = ExternalChannelEventResponse {
            accepted: true,
            assistant_run_id: None,
            idempotency_key: "idem-3".to_string(),
            reply: ExternalBotReplyView {
                target_conversation_external_id: "conv-1".to_string(),
                reply_type: ExternalBotReplyTypeView::TaskStatus,
                text: None,
                card: Some(json!({"status": "processing"})),
                artifact_links: Vec::new(),
                task_status: Some("needs_input".to_string()),
                requires_confirmation: false,
                action_id: None,
                confirmation_id: None,
            },
        };
        let from_card_status = ExternalChannelEventResponse {
            accepted: true,
            assistant_run_id: None,
            idempotency_key: "idem-4".to_string(),
            reply: ExternalBotReplyView {
                target_conversation_external_id: "conv-1".to_string(),
                reply_type: ExternalBotReplyTypeView::TaskStatus,
                text: None,
                card: Some(json!({"status": "needs_input"})),
                artifact_links: Vec::new(),
                task_status: Some("processing".to_string()),
                requires_confirmation: false,
                action_id: None,
                confirmation_id: None,
            },
        };
        let processing = ExternalChannelEventResponse {
            accepted: true,
            assistant_run_id: None,
            idempotency_key: "idem-5".to_string(),
            reply: ExternalBotReplyView {
                target_conversation_external_id: "conv-1".to_string(),
                reply_type: ExternalBotReplyTypeView::TaskStatus,
                text: None,
                card: Some(json!({"status": "processing"})),
                artifact_links: Vec::new(),
                task_status: Some("processing".to_string()),
                requires_confirmation: false,
                action_id: None,
                confirmation_id: None,
            },
        };

        assert!(external_channel_response_needs_input(&from_task_status));
        assert!(external_channel_response_needs_input(&from_card_status));
        assert!(!external_channel_response_needs_input(&processing));
    }

    #[test]
    fn needs_input_sse_text_sanitizes_text_or_uses_default() {
        let with_text = ExternalChannelEventResponse {
            accepted: true,
            assistant_run_id: None,
            idempotency_key: "idem-6".to_string(),
            reply: ExternalBotReplyView {
                target_conversation_external_id: "conv-1".to_string(),
                reply_type: ExternalBotReplyTypeView::TaskStatus,
                text: Some("  Cloudflare Codex 需要确认  ".to_string()),
                card: None,
                artifact_links: Vec::new(),
                task_status: Some("needs_input".to_string()),
                requires_confirmation: false,
                action_id: None,
                confirmation_id: None,
            },
        };
        let empty_text = ExternalChannelEventResponse {
            accepted: true,
            assistant_run_id: None,
            idempotency_key: "idem-7".to_string(),
            reply: ExternalBotReplyView {
                target_conversation_external_id: "conv-1".to_string(),
                reply_type: ExternalBotReplyTypeView::TaskStatus,
                text: Some("   ".to_string()),
                card: None,
                artifact_links: Vec::new(),
                task_status: Some("needs_input".to_string()),
                requires_confirmation: false,
                action_id: None,
                confirmation_id: None,
            },
        };

        assert_eq!(
            external_channel_needs_input_sse_text(&with_text),
            "DataMax 后台 需要确认"
        );
        assert_eq!(
            external_channel_needs_input_sse_text(&empty_text),
            "还需要补充信息后继续处理。"
        );
    }

    #[test]
    fn static_page_pipeline_detection_matches_image2_card_type() {
        let image2_pipeline = external_channel_response(
            Some(json!({"type": "v3_static_page_image2_pipeline"})),
            None,
            Vec::new(),
        );
        let image2_step = external_channel_response(
            Some(json!({"type": "v3_static_page_image2_effect_image"})),
            None,
            Vec::new(),
        );
        let non_pipeline = external_channel_response(
            Some(json!({"type": "v3_static_page_report"})),
            None,
            Vec::new(),
        );

        assert!(external_channel_response_is_static_page_pipeline(
            &image2_pipeline
        ));
        assert!(external_channel_response_is_static_page_pipeline(
            &image2_step
        ));
        assert!(!external_channel_response_is_static_page_pipeline(
            &non_pipeline
        ));
    }

    #[test]
    fn static_page_sse_status_promotes_public_artifact_unless_provisional() {
        let public_url = "https://v3.elepcloud.com/generated-artifacts/demo/index.html";
        let published = external_channel_response(
            Some(json!({
                "status": "static_page_publish_running",
                "public_url": public_url,
            })),
            None,
            Vec::new(),
        );
        let provisional = external_channel_response(
            Some(json!({
                "status": "static_page_publish_running",
                "public_url": public_url,
                "provisional_existing_artifact": true,
            })),
            None,
            Vec::new(),
        );
        let cancelled_but_running = external_channel_response(
            Some(json!({
                "status": "static_page_publish_cancelled",
                "runtime_event": {"status": "running"},
            })),
            None,
            Vec::new(),
        );

        assert_eq!(
            external_channel_static_page_sse_status(&published),
            "static_page_published"
        );
        assert!(external_channel_static_page_sse_is_terminal(&published));
        assert_eq!(
            external_channel_static_page_sse_status(&provisional),
            "static_page_publish_running"
        );
        assert!(!external_channel_static_page_sse_is_terminal(&provisional));
        assert_eq!(
            external_channel_static_page_sse_status(&cancelled_but_running),
            "static_page_continue_polling"
        );
    }

    #[test]
    fn static_page_sse_progress_text_promotes_public_artifact_link() {
        let public_url = "https://v3.elepcloud.com/generated-artifacts/demo/index.html";
        let response = external_channel_response(
            Some(json!({
                "status": "static_page_publish_running",
                "public_url": public_url,
                "template_adaptation": {"focus": [{"label": "取高机会"}]},
            })),
            None,
            Vec::new(),
        );

        let text = external_channel_static_page_sse_progress_text(&response);

        assert!(text.contains("已依据客户需求生成可访问的报表页面"));
        assert!(text.contains("系统识别到本轮关注焦点：取高机会"));
        assert!(text.contains("页面链接：[点击查看报表]"));
        assert!(text.contains(public_url));
        assert!(!text.contains("static_page_publish_running"));
    }

    #[test]
    fn static_page_sse_progress_text_uses_public_status_fallbacks() {
        let response = external_channel_response(
            Some(json!({"status": "static_page_publish_retrying"})),
            None,
            Vec::new(),
        );

        assert_eq!(
            external_channel_static_page_sse_progress_text(&response),
            "静态页发布遇到临时波动，DataMax 会继续重试或切换可用发布链路。"
        );
    }

    #[test]
    fn static_page_progress_payload_promotes_public_artifact_url() {
        let public_url =
            "https://v3.elepcloud.com/generated-artifacts/database-static-pages/demo/index.html";
        let mut response = external_channel_response(
            Some(json!({
                "type": "v3_static_page_image2_publish_status",
                "status": "static_page_publish_running",
                "public_url": public_url,
            })),
            Some("processing"),
            Vec::new(),
        );
        response.assistant_run_id = Some(AssistantRunId::new());
        response.idempotency_key = "generic:tenant:static-page-progress".to_string();

        let fields: ExternalChannelStaticPageSseProgressPayload =
            external_channel_static_page_sse_progress_payload(response);
        let (event_name, payload, text, dedupe_key, run_id) = fields;

        assert_eq!(event_name, "external_channel.static_page_published");
        assert_eq!(payload["sequence"], json!(90));
        assert_eq!(payload["status"], json!("static_page_published"));
        assert_eq!(payload["data"]["status"], json!("static_page_published"));
        assert_eq!(payload["data"]["artifact_links"][0], json!(public_url));
        assert!(text.contains("已依据客户需求生成可访问的报表页面"));
        assert!(text.contains(public_url));
        assert!(dedupe_key.starts_with("external_channel.static_page_published:"));
        assert!(run_id.is_some());
    }

    #[test]
    fn static_page_sse_progress_key_and_event_name_keep_wire_values() {
        let response = external_channel_response(
            Some(json!({
                "status": "static_page_effect_image_ready",
                "public_url": "https://v3.elepcloud.com/generated-artifacts/demo/index.html",
                "preview_asset_key": "preview/demo.png",
                "runtime_event": {"heartbeat_count": 3},
            })),
            None,
            Vec::new(),
        );

        assert_eq!(
            external_channel_static_page_sse_progress_key(&response),
            "static_page_published|https://v3.elepcloud.com/generated-artifacts/demo/index.html|preview/demo.png|3"
        );
        assert_eq!(
            external_channel_static_page_sse_event_name("static_page_effect_image_ready"),
            "external_channel.static_page_preview_ready"
        );
        assert_eq!(
            external_channel_static_page_sse_event_name("static_page_publish_running"),
            "external_channel.static_page_publish_progress"
        );
        assert_eq!(
            external_channel_static_page_sse_event_name("static_page_publish_failed"),
            "external_channel.static_page_issue"
        );
    }

    #[test]
    fn static_page_preview_public_url_normalizes_safe_preview_refs() {
        assert_eq!(
            external_channel_static_page_preview_public_url("static-page-previews/job/preview.png")
                .as_deref(),
            Some(
                "https://v3.elepcloud.com/generated-artifacts/static-page-previews/job/preview.png"
            )
        );
        assert_eq!(
            external_channel_static_page_preview_public_url(
                "https://v3.elepcloud.com/generated-artifacts/static-page-previews/job/preview.png?token=secret#frag"
            )
            .as_deref(),
            Some("https://v3.elepcloud.com/generated-artifacts/static-page-previews/job/preview.png")
        );
        assert!(
            external_channel_static_page_preview_public_url("data:image/png;base64,abc").is_none()
        );
        assert!(
            external_channel_static_page_preview_public_url("blob:https://example.test/1")
                .is_none()
        );
    }

    #[test]
    fn static_page_continue_polling_payload_uses_public_envelope() {
        let mut response = external_channel_response(
            Some(json!({
                "type": "v3_static_page_image2_publish_status",
                "status": "static_page_publish_running",
                "status_url": "https://v3.elepcloud.com/v1/external/channels/generic-chat-main/assistant-runs/run-1/reply",
                "poll_after_seconds": 15,
                "codex_host_workflow_execution_id": "hidden",
            })),
            Some("processing"),
            Vec::new(),
        );
        response.assistant_run_id = Some(AssistantRunId::new());
        response.idempotency_key = "generic:tenant:static-page-timeout".to_string();

        let fields: ExternalChannelStaticPageSseContinuePollingPayload =
            external_channel_static_page_sse_continue_polling_payload(response);
        let (payload, text, dedupe_key, run_id) = fields;

        assert_eq!(payload["schema"], json!(EXTERNAL_CHANNEL_SSE_SCHEMA_V1));
        assert_eq!(
            payload["status_url"],
            json!(
                "https://v3.elepcloud.com/v1/external/channels/generic-chat-main/assistant-runs/run-1/reply"
            )
        );
        assert_eq!(payload["poll_after_seconds"], json!(15));
        assert_eq!(payload["sequence"], json!(95));
        assert_eq!(payload["phase"], json!("static_page"));
        assert_eq!(
            payload["data"]["status"],
            json!("static_page_publish_running")
        );
        assert!(payload
            .pointer("/data/card/codex_host_workflow_execution_id")
            .is_none());
        assert!(text.contains("DataMax 会继续后台生成"));
        assert!(dedupe_key.starts_with("external_channel.static_page_continue_polling:"));
        assert!(run_id.is_some());
    }

    #[test]
    fn sse_event_with_delta_emits_public_delta_then_named_event() {
        let body = external_channel_sse_event_with_delta(
            "external_channel.static_page_progress",
            json!({
                "schema": EXTERNAL_CHANNEL_SSE_SCHEMA_V1,
                "event_id": "pending:000050",
                "sequence": 50,
                "phase": "static_page",
                "status": "static_page_publish_running",
                "display_text": "处理中",
                "_stream_dedupe_key": "internal",
                "data": {"text": "处理中"}
            }),
            "处理中",
        );

        assert!(body.contains("event: external_channel.delta"));
        assert!(body.contains("event: external_channel.static_page_progress"));
        assert!(body.contains("\"schema\":\"v3.external_channel.sse.v1\""));
        assert!(!body.contains("_stream_dedupe_key"));
    }

    #[test]
    fn answer_retrying_text_maps_known_reasons_and_default() {
        assert_eq!(
            external_channel_answer_retrying_text("gateway_limit"),
            "模型通道繁忙，DataMax 正在切换可用通道继续回答。"
        );
        assert_eq!(
            external_channel_answer_retrying_text("provider_error"),
            "模型通道暂时不可用，DataMax 正在重试或切换通道。"
        );
        assert_eq!(
            external_channel_answer_retrying_text("unknown"),
            "DataMax 正在重试或切换可用通道继续回答。"
        );
    }

    #[test]
    fn duration_env_value_parser_trims_and_defaults() {
        assert_eq!(
            external_channel_duration_from_millis_env_value(Some(" 2500 ".to_string()), 5_000),
            StdDuration::from_millis(2_500)
        );
        assert_eq!(
            external_channel_duration_from_millis_env_value(Some("bad".to_string()), 5_000),
            StdDuration::from_millis(5_000)
        );
        assert_eq!(
            external_channel_duration_from_millis_env_value(None, 120_000),
            StdDuration::from_millis(120_000)
        );
    }

    #[test]
    fn env_flag_parser_accepts_enabled_values_only() {
        assert!(external_channel_env_flag_is_enabled(Some(
            " true ".to_string()
        )));
        assert!(external_channel_env_flag_is_enabled(Some("ON".to_string())));
        assert!(!external_channel_env_flag_is_enabled(Some(
            "false".to_string()
        )));
        assert!(!external_channel_env_flag_is_enabled(None));
    }
}
