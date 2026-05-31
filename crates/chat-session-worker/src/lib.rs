use anyhow::Result;
use chrono::{DateTime, Utc};
use domain_model::{ChatMessageId, DatasetId, DatasetOutputId, MemoryDirectoryId};
use llm_gateway::{
    render_runtime_manifest, LlmFinishReason, LlmProvider, LlmRequest, LlmRuntimeMetadata,
    LlmToolCall, LlmToolCallStatus, MODEL_LANE_CHAT_SESSION,
};
use prompt_registry::CHAT_SESSION_PLACEHOLDER_PROMPT_KEY;
use serde_json::{json, Value};
use std::sync::Arc;
use tool_registry::render_tool_trace_manifest;

#[derive(Clone, Debug)]
pub struct ChatSessionJob {
    pub dataset_id: DatasetId,
    pub initial_prompt: String,
    pub prompt: String,
    pub indexed_document_count: usize,
    pub refreshed_chunks: usize,
    pub prior_message_count: usize,
    pub latest_memory_directory_id: Option<MemoryDirectoryId>,
    pub latest_memory_directory_version_no: Option<i32>,
    pub latest_dataset_output_id: Option<DatasetOutputId>,
    pub latest_dataset_output_retrieval_evidence_ids: Vec<domain_model::RetrievalEvidenceId>,
    pub tool_calls: Vec<LlmToolCall>,
    pub service_handoff: Option<contracts::ManifestServiceHandoffView>,
    pub turn_stream_mode: String,
    pub turn_id: String,
    pub turn_started_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct ChatSessionOutcome {
    pub assistant_message: String,
    pub message_manifest: Value,
    pub session_manifest: Value,
    pub completed_at: DateTime<Utc>,
    pub runtime: LlmRuntimeMetadata,
    pub tool_calls: Vec<LlmToolCall>,
}

pub trait ChatSessionOrchestrator {
    fn generate(
        &self,
        job: &ChatSessionJob,
        requested_at: DateTime<Utc>,
    ) -> Result<ChatSessionOutcome>;
}

#[derive(Clone, Debug)]
pub struct PlaceholderChatSessionOrchestrator {
    provider: Arc<dyn LlmProvider>,
    model: String,
    lane: String,
}

impl PlaceholderChatSessionOrchestrator {
    pub fn new(provider: Arc<dyn LlmProvider>, model: impl Into<String>) -> Self {
        Self::new_with_lane(provider, model, MODEL_LANE_CHAT_SESSION)
    }

    pub fn new_with_lane(
        provider: Arc<dyn LlmProvider>,
        model: impl Into<String>,
        lane: impl Into<String>,
    ) -> Self {
        Self {
            provider,
            model: model.into(),
            lane: lane.into(),
        }
    }
}

fn render_chat_turn_manifest(
    job: &ChatSessionJob,
    runtime: &LlmRuntimeMetadata,
    tool_calls: &[LlmToolCall],
    requested_at: DateTime<Utc>,
    responded_at: DateTime<Utc>,
) -> Value {
    let tool_loop_status = render_tool_loop_status(tool_calls);
    let provider_status = provider_status_from_runtime(runtime);
    json!({
        "turn_id": job.turn_id,
        "status": "pending",
        "stream_mode": job.turn_stream_mode,
        "stream_status": render_stream_status(&job.turn_stream_mode, provider_status),
        "artifact_commit_status": "pending",
        "provider_status": provider_status,
        "provider_failure": render_provider_failure(runtime),
        "tool_loop_status": tool_loop_status,
        "provider_request_id": runtime.request_id.as_deref(),
        "provider_requested_at": requested_at,
        "provider_responded_at": responded_at,
        "first_token_at": render_first_token_at(&job.turn_stream_mode, provider_status, responded_at),
        "stream_completed_at": render_stream_completed_at(&job.turn_stream_mode, provider_status, responded_at),
        "artifact_commit_ready_at": render_artifact_commit_ready_at(&job.turn_stream_mode, provider_status, responded_at),
        "artifact_commit_failure_source": Value::Null,
        "tool_calls_emitted_at": (runtime.tool_trace_count > 0).then_some(responded_at),
        "tool_loop_settled_at": render_tool_loop_settled_at(tool_calls, responded_at),
        "finish_reason": runtime.finish_reason.as_ref().map(LlmFinishReason::as_str),
        "assistant_message_id": Value::Null,
        "assistant_message_persisted_at": Value::Null,
        "tool_trace_count": runtime.tool_trace_count,
        "tool_status_summary": render_tool_status_summary(tool_calls),
        "events": render_chat_turn_events(job, runtime, tool_calls, requested_at, responded_at),
        "started_at": job.turn_started_at,
        "completed_at": Value::Null,
    })
}

fn render_turn_recovery_payload(
    assistant_message_content: &str,
    runtime: &LlmRuntimeMetadata,
    tool_calls: &[LlmToolCall],
    captured_at: DateTime<Utc>,
) -> Value {
    json!({
        "assistant_message_content": assistant_message_content,
        "runtime": render_runtime_manifest(runtime),
        "tool_trace": render_tool_trace_manifest(tool_calls),
        "captured_at": captured_at,
    })
}

pub fn render_completed_session_manifest(job: &ChatSessionJob) -> Value {
    json!({
        "generator": "chat-session-workflow",
        "schema_version": "0.3.0",
        "status": "assistant_replied",
        "initial_prompt": job.initial_prompt,
        "last_prompt": job.prompt,
        "last_turn_kind": "placeholder_orchestration",
        "context_binding": "creation_time",
        "latest_memory_directory_id": job.latest_memory_directory_id,
        "latest_memory_directory_version_no": job.latest_memory_directory_version_no,
        "latest_dataset_output_id": job.latest_dataset_output_id,
        "report_entry": report_entry_from_service_handoff(job.service_handoff.as_ref()),
    })
}

pub fn render_assistant_message_manifest(
    job: &ChatSessionJob,
    assistant_message_content: &str,
    runtime: &LlmRuntimeMetadata,
    tool_calls: &[LlmToolCall],
    requested_at: DateTime<Utc>,
    responded_at: DateTime<Utc>,
) -> Value {
    let turn_manifest =
        render_chat_turn_manifest(job, runtime, tool_calls, requested_at, responded_at);
    let runtime_manifest = render_runtime_manifest(runtime);
    json!({
        "generator": "chat-session-worker",
        "schema_version": "0.5.0",
        "dataset_id": job.dataset_id,
        "prompt": job.prompt,
        "indexed_document_count": job.indexed_document_count,
        "refreshed_chunks": job.refreshed_chunks,
        "prior_message_count": job.prior_message_count,
        "output": {
            "format": "markdown",
            "sections": [
                {
                    "section_key": "assistant_reply",
                    "kind": "reply",
                    "title": "Assistant Reply",
                    "content": assistant_message_content,
                    "retrieval_evidence_ids": job.latest_dataset_output_retrieval_evidence_ids,
                }
            ]
        },
        "latest_memory_directory_id": job.latest_memory_directory_id,
        "latest_memory_directory_version_no": job.latest_memory_directory_version_no,
        "latest_dataset_output_id": job.latest_dataset_output_id,
        "service_handoff": job.service_handoff.as_ref(),
        "tool_trace": render_tool_trace_manifest(tool_calls),
        "turn": turn_manifest,
        "context_binding": "creation_time",
        "runtime": runtime_manifest,
    })
}

pub fn render_in_flight_session_manifest(
    job: &ChatSessionJob,
    requested_at: DateTime<Utc>,
) -> Value {
    json!({
        "generator": "chat-session-workflow",
        "schema_version": "0.3.0",
        "status": "pending_assistant_reply",
        "initial_prompt": job.initial_prompt,
        "last_prompt": job.prompt,
        "last_turn_kind": "placeholder_orchestration",
        "context_binding": "creation_time",
        "latest_memory_directory_id": job.latest_memory_directory_id,
        "latest_memory_directory_version_no": job.latest_memory_directory_version_no,
        "latest_dataset_output_id": job.latest_dataset_output_id,
        "report_entry": report_entry_from_service_handoff(job.service_handoff.as_ref()),
        "last_turn": {
            "turn_id": job.turn_id,
            "status": "pending",
            "stream_mode": job.turn_stream_mode,
            "stream_status": render_stream_status(&job.turn_stream_mode, "pending"),
            "artifact_commit_status": "not_ready",
            "provider_status": "pending",
            "provider_failure": Value::Null,
            "tool_loop_status": "not_requested",
            "provider_request_id": Value::Null,
            "provider_requested_at": requested_at,
            "provider_responded_at": Value::Null,
            "first_token_at": Value::Null,
            "stream_completed_at": Value::Null,
            "artifact_commit_ready_at": Value::Null,
            "artifact_commit_failure_source": Value::Null,
            "tool_calls_emitted_at": Value::Null,
            "tool_loop_settled_at": Value::Null,
            "finish_reason": Value::Null,
            "assistant_message_id": Value::Null,
            "assistant_message_persisted_at": Value::Null,
            "tool_trace_count": 0,
            "events": [
                {
                    "kind": "turn_started",
                    "at": job.turn_started_at,
                    "provider_request_id": Value::Null,
                    "finish_reason": Value::Null,
                    "tool_trace_count": Value::Null,
                },
                {
                    "kind": "provider_requested",
                    "at": requested_at,
                    "provider_request_id": Value::Null,
                    "finish_reason": Value::Null,
                    "tool_trace_count": Value::Null,
                }
            ],
            "started_at": job.turn_started_at,
            "completed_at": Value::Null,
        },
    })
}

pub fn render_response_ready_session_manifest(
    job: &ChatSessionJob,
    assistant_message_content: &str,
    runtime: &LlmRuntimeMetadata,
    tool_calls: &[LlmToolCall],
    requested_at: DateTime<Utc>,
    responded_at: DateTime<Utc>,
) -> Value {
    let tool_loop_status = render_tool_loop_status(tool_calls);
    let provider_status = provider_status_from_runtime(runtime);
    let mut events = vec![
        json!({
            "kind": "turn_started",
            "at": job.turn_started_at,
            "provider_request_id": Value::Null,
            "finish_reason": Value::Null,
            "tool_trace_count": Value::Null,
        }),
        json!({
            "kind": "provider_requested",
            "at": requested_at,
            "provider_request_id": runtime.request_id.as_deref(),
            "finish_reason": Value::Null,
            "tool_trace_count": Value::Null,
        }),
        json!({
            "kind": "provider_responded",
            "at": responded_at,
            "provider_request_id": runtime.request_id.as_deref(),
            "finish_reason": runtime.finish_reason.as_ref().map(LlmFinishReason::as_str),
            "tool_trace_count": runtime.tool_trace_count,
        }),
    ];

    match render_stream_status(&job.turn_stream_mode, provider_status) {
        "completed" => {
            events.push(json!({
                "kind": "first_token_emitted",
                "at": responded_at,
                "provider_request_id": runtime.request_id.as_deref(),
                "finish_reason": runtime.finish_reason.as_ref().map(LlmFinishReason::as_str),
                "tool_trace_count": runtime.tool_trace_count,
            }));
            events.push(json!({
                "kind": "stream_completed",
                "at": responded_at,
                "provider_request_id": runtime.request_id.as_deref(),
                "finish_reason": runtime.finish_reason.as_ref().map(LlmFinishReason::as_str),
                "tool_trace_count": runtime.tool_trace_count,
            }));
        }
        "failed" => {
            events.push(json!({
                "kind": "stream_failed",
                "at": responded_at,
                "provider_request_id": runtime.request_id.as_deref(),
                "finish_reason": runtime.finish_reason.as_ref().map(LlmFinishReason::as_str),
                "tool_trace_count": runtime.tool_trace_count,
            }));
        }
        _ => {}
    }

    if let Some(artifact_commit_ready_at) =
        render_artifact_commit_ready_at(&job.turn_stream_mode, provider_status, responded_at)
    {
        events.push(json!({
            "kind": "artifact_commit_ready",
            "at": artifact_commit_ready_at,
            "provider_request_id": runtime.request_id.as_deref(),
            "finish_reason": runtime.finish_reason.as_ref().map(LlmFinishReason::as_str),
            "tool_trace_count": runtime.tool_trace_count,
        }));
    }

    if runtime.tool_trace_count > 0 {
        events.push(json!({
            "kind": "tool_calls_emitted",
            "at": responded_at,
            "provider_request_id": runtime.request_id.as_deref(),
            "finish_reason": runtime.finish_reason.as_ref().map(LlmFinishReason::as_str),
            "tool_trace_count": runtime.tool_trace_count,
        }));
        match tool_loop_status {
            "completed" => events.push(json!({
                "kind": "tool_loop_completed",
                "at": responded_at,
                "provider_request_id": runtime.request_id.as_deref(),
                "finish_reason": runtime.finish_reason.as_ref().map(LlmFinishReason::as_str),
                "tool_trace_count": runtime.tool_trace_count,
            })),
            "failed" => events.push(json!({
                "kind": "tool_loop_failed",
                "at": responded_at,
                "provider_request_id": runtime.request_id.as_deref(),
                "finish_reason": runtime.finish_reason.as_ref().map(LlmFinishReason::as_str),
                "tool_trace_count": runtime.tool_trace_count,
            })),
            _ => {}
        }
    }

    json!({
        "generator": "chat-session-workflow",
        "schema_version": "0.3.0",
        "status": "pending_assistant_reply",
        "initial_prompt": job.initial_prompt,
        "last_prompt": job.prompt,
        "last_turn_kind": "placeholder_orchestration",
        "context_binding": "creation_time",
        "latest_memory_directory_id": job.latest_memory_directory_id,
        "latest_memory_directory_version_no": job.latest_memory_directory_version_no,
        "latest_dataset_output_id": job.latest_dataset_output_id,
        "report_entry": report_entry_from_service_handoff(job.service_handoff.as_ref()),
        "last_turn": {
            "turn_id": job.turn_id,
            "status": "pending",
            "stream_mode": job.turn_stream_mode,
            "stream_status": render_stream_status(&job.turn_stream_mode, provider_status),
            "artifact_commit_status": "pending",
            "provider_status": provider_status,
            "provider_failure": render_provider_failure(runtime),
            "tool_loop_status": tool_loop_status,
            "provider_request_id": runtime.request_id.as_deref(),
            "provider_requested_at": requested_at,
            "provider_responded_at": responded_at,
            "first_token_at": render_first_token_at(&job.turn_stream_mode, provider_status, responded_at),
            "stream_completed_at": render_stream_completed_at(&job.turn_stream_mode, provider_status, responded_at),
            "artifact_commit_ready_at": render_artifact_commit_ready_at(&job.turn_stream_mode, provider_status, responded_at),
            "artifact_commit_failure_source": Value::Null,
            "tool_calls_emitted_at": (runtime.tool_trace_count > 0).then_some(responded_at),
            "tool_loop_settled_at": render_tool_loop_settled_at(tool_calls, responded_at),
            "finish_reason": runtime.finish_reason.as_ref().map(LlmFinishReason::as_str),
            "assistant_message_id": Value::Null,
            "assistant_message_persisted_at": Value::Null,
            "tool_trace_count": runtime.tool_trace_count,
            "tool_status_summary": render_tool_status_summary(tool_calls),
            "recovery": render_turn_recovery_payload(
                assistant_message_content,
                runtime,
                tool_calls,
                responded_at,
            ),
            "events": events,
            "started_at": job.turn_started_at,
            "completed_at": Value::Null,
        },
    })
}

pub fn render_failed_session_manifest(
    job: &ChatSessionJob,
    requested_at: DateTime<Utc>,
    failed_at: DateTime<Utc>,
    assistant_message_content: Option<&str>,
    runtime: Option<&LlmRuntimeMetadata>,
    tool_calls: &[LlmToolCall],
    artifact_commit_failure_source: Option<&str>,
) -> Value {
    let tool_loop_status = render_tool_loop_status(tool_calls);
    let provider_status = runtime
        .map(provider_status_from_runtime)
        .unwrap_or("failed");
    let provider_responded_at =
        runtime.and_then(|value| provider_responded_at_from_runtime(value, failed_at));
    let provider_request_id = runtime.and_then(|value| value.request_id.as_deref());
    let finish_reason = runtime
        .and_then(|value| value.finish_reason.as_ref())
        .map(LlmFinishReason::as_str)
        .unwrap_or("error");
    let tool_trace_count = runtime.map(|value| value.tool_trace_count).unwrap_or(0);
    let artifact_commit_ready_at =
        render_artifact_commit_ready_at(&job.turn_stream_mode, provider_status, failed_at);
    let artifact_commit_status = if artifact_commit_failure_source.is_some() {
        "failed"
    } else {
        "not_ready"
    };
    let mut events = vec![
        json!({
            "kind": "turn_started",
            "at": job.turn_started_at,
            "provider_request_id": Value::Null,
            "finish_reason": Value::Null,
            "tool_trace_count": Value::Null,
        }),
        json!({
            "kind": "provider_requested",
            "at": requested_at,
            "provider_request_id": provider_request_id,
            "finish_reason": Value::Null,
            "tool_trace_count": Value::Null,
        }),
    ];

    if provider_responded_at.is_some() {
        events.push(json!({
            "kind": "provider_responded",
            "at": provider_responded_at,
            "provider_request_id": provider_request_id,
            "finish_reason": finish_reason,
            "tool_trace_count": tool_trace_count,
        }));
        if job.turn_stream_mode == "streaming" && provider_status == "responded" {
            events.push(json!({
                "kind": "first_token_emitted",
                "at": failed_at,
                "provider_request_id": provider_request_id,
                "finish_reason": finish_reason,
                "tool_trace_count": tool_trace_count,
            }));
            events.push(json!({
                "kind": "stream_completed",
                "at": failed_at,
                "provider_request_id": provider_request_id,
                "finish_reason": finish_reason,
                "tool_trace_count": tool_trace_count,
            }));
        } else if job.turn_stream_mode == "streaming" {
            events.push(json!({
                "kind": "stream_failed",
                "at": failed_at,
                "provider_request_id": provider_request_id,
                "finish_reason": finish_reason,
                "tool_trace_count": tool_trace_count,
            }));
        }
    }

    if let Some(artifact_commit_ready_at) = artifact_commit_ready_at {
        events.push(json!({
            "kind": "artifact_commit_ready",
            "at": artifact_commit_ready_at,
            "provider_request_id": provider_request_id,
            "finish_reason": finish_reason,
            "tool_trace_count": tool_trace_count,
        }));
    }

    if tool_trace_count > 0 {
        events.push(json!({
            "kind": "tool_calls_emitted",
            "at": failed_at,
            "provider_request_id": provider_request_id,
            "finish_reason": finish_reason,
            "tool_trace_count": tool_trace_count,
        }));
        match tool_loop_status {
            "completed" => events.push(json!({
                "kind": "tool_loop_completed",
                "at": failed_at,
                "provider_request_id": provider_request_id,
                "finish_reason": finish_reason,
                "tool_trace_count": tool_trace_count,
            })),
            "failed" => events.push(json!({
                "kind": "tool_loop_failed",
                "at": failed_at,
                "provider_request_id": provider_request_id,
                "finish_reason": finish_reason,
                "tool_trace_count": tool_trace_count,
            })),
            _ => {}
        }
    }

    if artifact_commit_status == "failed" {
        events.push(json!({
            "kind": "artifact_commit_failed",
            "at": failed_at,
            "provider_request_id": provider_request_id,
            "finish_reason": finish_reason,
            "tool_trace_count": tool_trace_count,
        }));
    }

    events.push(json!({
        "kind": "turn_failed",
        "at": failed_at,
        "provider_request_id": provider_request_id,
        "finish_reason": finish_reason,
        "tool_trace_count": tool_trace_count,
    }));

    json!({
        "generator": "chat-session-workflow",
        "schema_version": "0.3.0",
        "status": "pending_assistant_reply",
        "initial_prompt": job.initial_prompt,
        "last_prompt": job.prompt,
        "last_turn_kind": "placeholder_orchestration",
        "context_binding": "creation_time",
        "latest_memory_directory_id": job.latest_memory_directory_id,
        "latest_memory_directory_version_no": job.latest_memory_directory_version_no,
        "latest_dataset_output_id": job.latest_dataset_output_id,
        "report_entry": report_entry_from_service_handoff(job.service_handoff.as_ref()),
        "last_turn": {
            "turn_id": job.turn_id,
            "status": "failed",
            "stream_mode": job.turn_stream_mode,
            "stream_status": render_stream_status(&job.turn_stream_mode, provider_status),
            "artifact_commit_status": artifact_commit_status,
            "provider_status": provider_status,
            "provider_failure": runtime
                .map(render_provider_failure)
                .unwrap_or(Value::Null),
            "tool_loop_status": tool_loop_status,
            "provider_request_id": provider_request_id,
            "provider_requested_at": requested_at,
            "provider_responded_at": provider_responded_at,
            "first_token_at": Value::Null,
            "stream_completed_at": render_stream_completed_at(&job.turn_stream_mode, provider_status, failed_at),
            "artifact_commit_ready_at": artifact_commit_ready_at,
            "artifact_commit_failure_source": artifact_commit_failure_source,
            "tool_calls_emitted_at": (tool_trace_count > 0).then_some(failed_at),
            "tool_loop_settled_at": render_tool_loop_settled_at(tool_calls, failed_at),
            "finish_reason": finish_reason,
            "assistant_message_id": Value::Null,
            "assistant_message_persisted_at": Value::Null,
            "tool_trace_count": tool_trace_count,
            "tool_status_summary": render_tool_status_summary(tool_calls),
            "recovery": assistant_message_content.zip(runtime).map(|(content, runtime)| {
                render_turn_recovery_payload(content, runtime, tool_calls, failed_at)
            }),
            "events": events,
            "started_at": job.turn_started_at,
            "completed_at": failed_at,
        },
    })
}

fn report_entry_from_service_handoff(
    handoff: Option<&contracts::ManifestServiceHandoffView>,
) -> Option<contracts::ChatSessionReportEntryView> {
    handoff.map(|handoff| contracts::ChatSessionReportEntryView {
        state: handoff.report_entry_state.clone(),
        requested_at: handoff.requested_at,
        resolved_at: handoff.resolved_at,
        resolved_action: handoff.resolved_action.clone(),
        suggested_title: handoff.suggested_title.clone(),
        suggested_objective: handoff.suggested_objective.clone(),
        confirmed_report_plan_id: handoff.confirmed_report_plan_id,
    })
}

pub fn chat_runtime_error_message(runtime: &LlmRuntimeMetadata) -> Option<String> {
    match runtime.finish_reason {
        Some(LlmFinishReason::Error) => Some(format!(
            "chat session provider {} returned finish_reason=error{}{}",
            runtime.provider,
            runtime
                .request_id
                .as_ref()
                .map(|value| format!(" (request_id={value})"))
                .unwrap_or_default(),
            runtime
                .provider_failure
                .as_ref()
                .map(|failure| format!(" [{}: {}]", failure.kind.as_str(), failure.message))
                .unwrap_or_default()
        )),
        _ => None,
    }
}

pub fn finalize_assistant_message_manifest(
    manifest: &Value,
    assistant_message_id: ChatMessageId,
    persisted_at: DateTime<Utc>,
) -> Value {
    let mut manifest = manifest.clone();
    let Some(turn) = manifest.get_mut("turn").and_then(Value::as_object_mut) else {
        return manifest;
    };

    let provider_request_id = turn
        .get("provider_request_id")
        .and_then(Value::as_str)
        .map(str::to_string);
    let finish_reason = turn
        .get("finish_reason")
        .and_then(Value::as_str)
        .map(str::to_string);
    let tool_trace_count = turn
        .get("tool_trace_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);

    turn.insert("status".to_string(), json!("completed"));
    turn.insert(
        "assistant_message_id".to_string(),
        json!(assistant_message_id.to_string()),
    );
    turn.insert(
        "assistant_message_persisted_at".to_string(),
        json!(persisted_at),
    );
    turn.insert("artifact_commit_status".to_string(), json!("completed"));
    turn.insert("artifact_commit_failure_source".to_string(), Value::Null);
    if turn
        .get("artifact_commit_ready_at")
        .map(|value| value.is_null())
        .unwrap_or(true)
    {
        turn.insert("artifact_commit_ready_at".to_string(), json!(persisted_at));
    }
    turn.insert("completed_at".to_string(), json!(persisted_at));

    if let Some(events) = turn.get_mut("events").and_then(Value::as_array_mut) {
        events.push(json!({
            "kind": "assistant_message_persisted",
            "at": persisted_at,
            "provider_request_id": provider_request_id,
            "finish_reason": finish_reason,
            "tool_trace_count": tool_trace_count,
        }));
        events.push(json!({
            "kind": "turn_completed",
            "at": persisted_at,
            "provider_request_id": provider_request_id,
            "finish_reason": finish_reason,
            "tool_trace_count": tool_trace_count,
        }));
    }

    manifest
}

fn render_tool_status_summary(tool_calls: &[LlmToolCall]) -> Value {
    if tool_calls.is_empty() {
        return Value::Null;
    }

    let mut requested_count = 0usize;
    let mut completed_count = 0usize;
    let mut failed_count = 0usize;

    for tool_call in tool_calls {
        match tool_call.status {
            LlmToolCallStatus::Requested => requested_count += 1,
            LlmToolCallStatus::Completed => completed_count += 1,
            LlmToolCallStatus::Failed => failed_count += 1,
        }
    }

    json!({
        "requested_count": requested_count,
        "completed_count": completed_count,
        "failed_count": failed_count,
    })
}

fn render_tool_loop_status(tool_calls: &[LlmToolCall]) -> &'static str {
    if tool_calls.is_empty() {
        return "not_requested";
    }

    let mut requested_count = 0usize;
    let mut failed_count = 0usize;
    for tool_call in tool_calls {
        match tool_call.status {
            LlmToolCallStatus::Requested => requested_count += 1,
            LlmToolCallStatus::Completed => {}
            LlmToolCallStatus::Failed => failed_count += 1,
        }
    }

    if requested_count > 0 {
        "pending"
    } else if failed_count > 0 {
        "failed"
    } else {
        "completed"
    }
}

fn render_tool_loop_settled_at(
    tool_calls: &[LlmToolCall],
    terminal_at: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    match render_tool_loop_status(tool_calls) {
        "completed" | "failed" => Some(terminal_at),
        _ => None,
    }
}

fn render_stream_status(stream_mode: &str, provider_status: &str) -> &'static str {
    if stream_mode != "streaming" {
        return "not_requested";
    }

    match provider_status {
        "pending" => "pending",
        "responded" => "completed",
        "failed" => "failed",
        _ => "pending",
    }
}

fn render_first_token_at(
    stream_mode: &str,
    provider_status: &str,
    responded_at: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    if stream_mode == "streaming" && provider_status == "responded" {
        Some(responded_at)
    } else {
        None
    }
}

fn render_stream_completed_at(
    stream_mode: &str,
    provider_status: &str,
    terminal_at: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    if stream_mode != "streaming" {
        return None;
    }

    match provider_status {
        "responded" | "failed" => Some(terminal_at),
        _ => None,
    }
}

fn render_artifact_commit_ready_at(
    stream_mode: &str,
    provider_status: &str,
    responded_at: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    if stream_mode == "buffered" && provider_status == "responded" {
        return Some(responded_at);
    }

    match render_stream_status(stream_mode, provider_status) {
        "completed" => Some(responded_at),
        _ => None,
    }
}

impl ChatSessionOrchestrator for PlaceholderChatSessionOrchestrator {
    fn generate(
        &self,
        job: &ChatSessionJob,
        requested_at: DateTime<Utc>,
    ) -> Result<ChatSessionOutcome> {
        let placeholder_message = format!(
            "这是 chat_session_workflow 的占位回复。\n\n- Prompt: {}\n- Dataset: {}\n- Indexed documents: {}\n- Refreshed chunks: {}\n- Prior messages: {}\n- Bound memory directory: {} (version {})\n- Bound dataset output: {}\n\n当前实现只负责把会话编排壳、上下文引用和消息持久化打通，后续再接真实 LLM、retrieval evidence 和 tool trace。",
            job.prompt,
            job.dataset_id,
            job.indexed_document_count,
            job.refreshed_chunks,
            job.prior_message_count,
            job.latest_memory_directory_id
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string()),
            job.latest_memory_directory_version_no
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string()),
            job.latest_dataset_output_id
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string()),
        );
        let response = self.provider.complete(&LlmRequest {
            model: self.model.clone(),
            lane: Some(self.lane.clone()),
            system_prompt_key: Some(CHAT_SESSION_PLACEHOLDER_PROMPT_KEY.to_string()),
            input: placeholder_message,
        })?;
        let completed_at = Utc::now();
        let mut tool_calls = job.tool_calls.clone();
        tool_calls.extend(response.tool_calls.clone());
        let mut runtime = response.runtime.clone();
        runtime.tool_trace_count = tool_calls.len();

        Ok(ChatSessionOutcome {
            assistant_message: response.output_text.clone(),
            message_manifest: render_assistant_message_manifest(
                job,
                &response.output_text,
                &runtime,
                &tool_calls,
                requested_at,
                completed_at,
            ),
            session_manifest: render_completed_session_manifest(job),
            completed_at,
            runtime,
            tool_calls,
        })
    }
}

fn provider_status_from_runtime(runtime: &LlmRuntimeMetadata) -> &'static str {
    match runtime.finish_reason {
        Some(LlmFinishReason::Error) => "failed",
        _ => "responded",
    }
}

fn provider_responded_at_from_runtime(
    runtime: &LlmRuntimeMetadata,
    terminal_at: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    match runtime.provider_failure.as_ref() {
        Some(failure) if !failure.kind.provider_responded() => None,
        _ => Some(terminal_at),
    }
}

fn render_provider_failure(runtime: &LlmRuntimeMetadata) -> Value {
    runtime
        .provider_failure
        .as_ref()
        .map(|failure| {
            json!({
                "kind": failure.kind.as_str(),
                "message": failure.message,
            })
        })
        .unwrap_or(Value::Null)
}

fn render_chat_turn_events(
    job: &ChatSessionJob,
    runtime: &LlmRuntimeMetadata,
    tool_calls: &[LlmToolCall],
    requested_at: DateTime<Utc>,
    responded_at: DateTime<Utc>,
) -> Value {
    let mut events = vec![json!({
        "kind": "turn_started",
        "at": job.turn_started_at,
        "provider_request_id": Value::Null,
        "finish_reason": Value::Null,
        "tool_trace_count": Value::Null,
    })];

    if runtime.request_id.is_some() {
        events.push(json!({
            "kind": "provider_requested",
            "at": requested_at,
            "provider_request_id": runtime.request_id.as_deref(),
            "finish_reason": Value::Null,
            "tool_trace_count": Value::Null,
        }));
    }

    events.push(json!({
        "kind": "provider_responded",
        "at": responded_at,
        "provider_request_id": runtime.request_id.as_deref(),
        "finish_reason": runtime.finish_reason.as_ref().map(LlmFinishReason::as_str),
        "tool_trace_count": runtime.tool_trace_count,
    }));

    match render_stream_status(&job.turn_stream_mode, provider_status_from_runtime(runtime)) {
        "completed" => {
            events.push(json!({
                "kind": "first_token_emitted",
                "at": responded_at,
                "provider_request_id": runtime.request_id.as_deref(),
                "finish_reason": runtime.finish_reason.as_ref().map(LlmFinishReason::as_str),
                "tool_trace_count": runtime.tool_trace_count,
            }));
            events.push(json!({
                "kind": "stream_completed",
                "at": responded_at,
                "provider_request_id": runtime.request_id.as_deref(),
                "finish_reason": runtime.finish_reason.as_ref().map(LlmFinishReason::as_str),
                "tool_trace_count": runtime.tool_trace_count,
            }));
        }
        "failed" => {
            events.push(json!({
                "kind": "stream_failed",
                "at": responded_at,
                "provider_request_id": runtime.request_id.as_deref(),
                "finish_reason": runtime.finish_reason.as_ref().map(LlmFinishReason::as_str),
                "tool_trace_count": runtime.tool_trace_count,
            }));
        }
        _ => {}
    }

    if runtime.tool_trace_count > 0 {
        events.push(json!({
            "kind": "tool_calls_emitted",
            "at": responded_at,
            "provider_request_id": runtime.request_id.as_deref(),
            "finish_reason": runtime.finish_reason.as_ref().map(LlmFinishReason::as_str),
            "tool_trace_count": runtime.tool_trace_count,
        }));
        match render_tool_loop_status(tool_calls) {
            "completed" => events.push(json!({
                "kind": "tool_loop_completed",
                "at": responded_at,
                "provider_request_id": runtime.request_id.as_deref(),
                "finish_reason": runtime.finish_reason.as_ref().map(LlmFinishReason::as_str),
                "tool_trace_count": runtime.tool_trace_count,
            })),
            "failed" => events.push(json!({
                "kind": "tool_loop_failed",
                "at": responded_at,
                "provider_request_id": runtime.request_id.as_deref(),
                "finish_reason": runtime.finish_reason.as_ref().map(LlmFinishReason::as_str),
                "tool_trace_count": runtime.tool_trace_count,
            })),
            _ => {}
        }
    }

    Value::Array(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use prompt_registry::bootstrap_default_prompt_registry;

    const PLACEHOLDER_MODEL: &str = "placeholder-chat-session-v1";

    #[test]
    fn placeholder_chat_session_references_prompt_and_counts() {
        let dataset_id = DatasetId::new();
        let requested_at = Utc::now() + chrono::TimeDelta::milliseconds(25);
        let orchestrator = PlaceholderChatSessionOrchestrator::new(
            Arc::new(
                llm_gateway::PlaceholderLlmProvider::new("placeholder")
                    .with_prompt_registry(bootstrap_default_prompt_registry()),
            ),
            PLACEHOLDER_MODEL,
        );
        let outcome = orchestrator
            .generate(
                &ChatSessionJob {
                    dataset_id,
                    initial_prompt: "Summarize the latest evidence and answer the user".to_string(),
                    prompt: "Summarize the latest evidence and answer the user".to_string(),
                    indexed_document_count: 3,
                    refreshed_chunks: 12,
                    prior_message_count: 1,
                    latest_memory_directory_id: None,
                    latest_memory_directory_version_no: Some(4),
                    latest_dataset_output_id: None,
                    latest_dataset_output_retrieval_evidence_ids: vec![],
                    tool_calls: vec![],
                    service_handoff: None,
                    turn_stream_mode: "buffered".to_string(),
                    turn_id: "turn_placeholder".to_string(),
                    turn_started_at: Utc::now(),
                },
                requested_at,
            )
            .expect("placeholder chat session should succeed");

        assert!(outcome
            .assistant_message
            .contains("Summarize the latest evidence"));
        assert_eq!(
            outcome.message_manifest["latest_memory_directory_version_no"],
            json!(4)
        );
        assert_eq!(outcome.message_manifest["indexed_document_count"], json!(3));
        assert_eq!(
            outcome.message_manifest["runtime"]["tool_trace_count"],
            json!(0)
        );
        assert_eq!(
            outcome.session_manifest["status"],
            json!("assistant_replied")
        );
        assert_eq!(
            outcome.session_manifest["initial_prompt"],
            json!("Summarize the latest evidence and answer the user")
        );
        assert!(outcome.session_manifest.get("last_turn").is_none());
        assert!(outcome.session_manifest.get("runtime").is_none());
        assert_eq!(
            outcome.message_manifest["runtime"]["provider"],
            json!("placeholder")
        );
        assert_eq!(
            outcome.message_manifest["runtime"]["model"],
            json!("placeholder-chat-session-v1")
        );
        assert_eq!(
            outcome.message_manifest["runtime"]["system_prompt_key"],
            json!("chat_session.placeholder")
        );
        assert_eq!(
            outcome.message_manifest["runtime"]["system_prompt_version"],
            json!("v1")
        );
        assert_eq!(
            outcome.message_manifest["runtime"]["finish_reason"],
            json!("stop")
        );
        assert!(outcome.message_manifest["runtime"]["request_id"].is_string());
        assert!(outcome.message_manifest["runtime"]["latency_ms"].is_number());
        assert_eq!(
            outcome.message_manifest["runtime"]["usage"]["total_tokens"]
                .as_u64()
                .map(|value| value > 0),
            Some(true)
        );
        assert_eq!(outcome.message_manifest["turn"]["status"], json!("pending"));
        assert_eq!(
            outcome.message_manifest["turn"]["stream_mode"],
            json!("buffered")
        );
        assert_eq!(
            outcome.message_manifest["turn"]["stream_status"],
            json!("not_requested")
        );
        assert_eq!(
            outcome.message_manifest["turn"]["provider_status"],
            json!("responded")
        );
        assert!(outcome.message_manifest["turn"]["first_token_at"].is_null());
        assert!(outcome.message_manifest["turn"]["stream_completed_at"].is_null());
        assert_eq!(
            outcome.message_manifest["turn"]["finish_reason"],
            json!("stop")
        );
        assert_eq!(
            outcome.message_manifest["turn"]["events"]
                .as_array()
                .map(Vec::len),
            Some(3)
        );
        assert_eq!(
            outcome.message_manifest["turn"]["events"][0]["kind"],
            json!("turn_started")
        );
        assert_eq!(
            outcome.message_manifest["turn"]["events"][1]["kind"],
            json!("provider_requested")
        );
        assert_eq!(
            outcome.message_manifest["turn"]["events"][2]["kind"],
            json!("provider_responded")
        );
        assert!(outcome.message_manifest["turn"]["assistant_message_persisted_at"].is_null());
        assert!(outcome.message_manifest["turn"]["completed_at"].is_null());
        assert_eq!(
            outcome.message_manifest["turn"]["tool_trace_count"],
            json!(0)
        );
        assert_eq!(outcome.message_manifest["tool_trace"], json!([]));
        assert!(outcome.tool_calls.is_empty());
        assert_eq!(
            outcome.message_manifest["output"]["format"],
            json!("markdown")
        );
        assert_eq!(
            outcome.message_manifest["output"]["sections"][0]["kind"],
            json!("reply")
        );
        assert_eq!(
            outcome.message_manifest["output"]["sections"][0]["content"],
            outcome.assistant_message
        );
        assert_eq!(
            outcome.message_manifest["output"]["sections"][0]["retrieval_evidence_ids"],
            json!([])
        );
    }

    #[test]
    fn scripted_provider_emits_provider_mode_runtime_manifest() {
        let dataset_id = DatasetId::new();
        let requested_at = Utc::now() + chrono::TimeDelta::milliseconds(25);
        let orchestrator = PlaceholderChatSessionOrchestrator::new(
            Arc::new(
                llm_gateway::ScriptedLlmProvider::new("openai")
                    .with_prompt_registry(bootstrap_default_prompt_registry())
                    .with_response_text("provider reply")
                    .with_request_id("req_chat_provider")
                    .with_finish_reason(llm_gateway::LlmFinishReason::ToolCalls)
                    .with_latency_ms(456)
                    .with_usage(llm_gateway::LlmTokenUsage {
                        input_tokens: 3,
                        output_tokens: 5,
                        total_tokens: 8,
                    })
                    .with_tool_calls(vec![llm_gateway::LlmToolCall {
                        call_id: Some("call_chat_provider".to_string()),
                        tool_name: "weather.lookup".to_string(),
                        status: llm_gateway::LlmToolCallStatus::Completed,
                        arguments: Some(json!({ "city": "Shanghai" })),
                        result: Some(json!({ "summary": "sunny" })),
                    }]),
            ),
            "gpt-5.4",
        );
        let outcome = orchestrator
            .generate(
                &ChatSessionJob {
                    dataset_id,
                    initial_prompt: "Summarize the latest evidence and answer the user".to_string(),
                    prompt: "Summarize the latest evidence and answer the user".to_string(),
                    indexed_document_count: 3,
                    refreshed_chunks: 12,
                    prior_message_count: 1,
                    latest_memory_directory_id: None,
                    latest_memory_directory_version_no: Some(4),
                    latest_dataset_output_id: None,
                    latest_dataset_output_retrieval_evidence_ids: vec![
                        domain_model::RetrievalEvidenceId::new(),
                        domain_model::RetrievalEvidenceId::new(),
                    ],
                    tool_calls: vec![],
                    service_handoff: None,
                    turn_stream_mode: "streaming".to_string(),
                    turn_id: "turn_provider".to_string(),
                    turn_started_at: Utc::now(),
                },
                requested_at,
            )
            .expect("scripted chat session should succeed");

        assert_eq!(outcome.assistant_message, "provider reply");
        assert_eq!(
            outcome.message_manifest["runtime"]["mode"],
            json!("provider")
        );
        assert_eq!(
            outcome.message_manifest["runtime"]["provider"],
            json!("openai")
        );
        assert_eq!(
            outcome.message_manifest["runtime"]["request_id"],
            json!("req_chat_provider")
        );
        assert_eq!(
            outcome.message_manifest["runtime"]["finish_reason"],
            json!("tool_calls")
        );
        assert_eq!(
            outcome.message_manifest["runtime"]["latency_ms"],
            json!(456)
        );
        assert_eq!(
            outcome.message_manifest["runtime"]["usage"]["total_tokens"],
            json!(8)
        );
        assert_eq!(
            outcome.message_manifest["runtime"]["tool_trace_count"],
            json!(1)
        );
        assert_eq!(
            outcome.message_manifest["turn"]["turn_id"],
            json!("turn_provider")
        );
        assert_eq!(
            outcome.message_manifest["turn"]["provider_request_id"],
            json!("req_chat_provider")
        );
        assert_eq!(
            outcome.message_manifest["turn"]["provider_status"],
            json!("responded")
        );
        assert_eq!(
            outcome.message_manifest["turn"]["stream_status"],
            json!("completed")
        );
        assert_eq!(
            outcome.message_manifest["turn"]["tool_loop_status"],
            json!("completed")
        );
        assert_eq!(
            outcome.message_manifest["turn"]["finish_reason"],
            json!("tool_calls")
        );
        assert_eq!(
            outcome.message_manifest["turn"]["events"]
                .as_array()
                .map(Vec::len),
            Some(7)
        );
        assert_eq!(
            outcome.message_manifest["turn"]["events"][2]["kind"],
            json!("provider_responded")
        );
        assert_eq!(
            outcome.message_manifest["turn"]["events"][3]["kind"],
            json!("first_token_emitted")
        );
        assert_eq!(
            outcome.message_manifest["turn"]["events"][4]["kind"],
            json!("stream_completed")
        );
        assert_eq!(
            outcome.message_manifest["turn"]["events"][5]["kind"],
            json!("tool_calls_emitted")
        );
        assert_eq!(
            outcome.message_manifest["turn"]["events"][6]["kind"],
            json!("tool_loop_completed")
        );
        assert_eq!(
            outcome.message_manifest["turn"]["first_token_at"],
            json!(outcome.completed_at)
        );
        assert_eq!(
            outcome.message_manifest["turn"]["stream_completed_at"],
            json!(outcome.completed_at)
        );
        assert_eq!(
            outcome.message_manifest["turn"]["tool_calls_emitted_at"],
            json!(outcome.completed_at)
        );
        assert_eq!(
            outcome.message_manifest["turn"]["tool_loop_settled_at"],
            json!(outcome.completed_at)
        );
        assert!(outcome.message_manifest["turn"]["assistant_message_persisted_at"].is_null());
        assert!(outcome.message_manifest["turn"]["completed_at"].is_null());
        assert_eq!(
            outcome.message_manifest["turn"]["stream_mode"],
            json!("streaming")
        );
        assert_eq!(
            outcome.session_manifest["last_prompt"],
            json!("Summarize the latest evidence and answer the user")
        );
        assert!(outcome.session_manifest.get("last_turn").is_none());
        assert!(outcome.session_manifest.get("runtime").is_none());
        assert_eq!(
            outcome.message_manifest["tool_trace"][0]["tool_name"],
            json!("weather.lookup")
        );
        assert_eq!(
            outcome.message_manifest["tool_trace"][0]["status"],
            json!("completed")
        );
        assert_eq!(outcome.tool_calls.len(), 1);
        assert_eq!(outcome.tool_calls[0].tool_name, "weather.lookup");
        assert_eq!(
            outcome.tool_calls[0].status,
            llm_gateway::LlmToolCallStatus::Completed
        );
        assert_eq!(
            outcome.message_manifest["output"]["sections"][0]["kind"],
            json!("reply")
        );
        assert_eq!(
            outcome.message_manifest["output"]["sections"][0]["content"],
            json!("provider reply")
        );
        assert_eq!(
            outcome.message_manifest["output"]["sections"][0]["retrieval_evidence_ids"]
                .as_array()
                .map(Vec::len),
            Some(2)
        );
    }

    #[test]
    fn precomputed_tool_calls_are_merged_into_chat_manifest_trace() {
        let retrieval_evidence_id = domain_model::RetrievalEvidenceId::new();
        let requested_at = Utc::now() + chrono::TimeDelta::milliseconds(25);
        let orchestrator = PlaceholderChatSessionOrchestrator::new(
            Arc::new(
                llm_gateway::ScriptedLlmProvider::new("openai")
                    .with_prompt_registry(bootstrap_default_prompt_registry())
                    .with_response_text("provider reply")
                    .with_tool_calls(vec![llm_gateway::LlmToolCall {
                        call_id: Some("call_provider".to_string()),
                        tool_name: "document.compare".to_string(),
                        status: llm_gateway::LlmToolCallStatus::Completed,
                        arguments: Some(json!({ "document_ids": ["doc_1", "doc_2"] })),
                        result: Some(json!({ "summary": "comparison loaded" })),
                    }]),
            ),
            "gpt-5.4",
        );

        let outcome = orchestrator
            .generate(
                &ChatSessionJob {
                    dataset_id: DatasetId::new(),
                    initial_prompt: "Summarize the latest evidence".to_string(),
                    prompt: "Summarize the latest evidence".to_string(),
                    indexed_document_count: 2,
                    refreshed_chunks: 4,
                    prior_message_count: 1,
                    latest_memory_directory_id: None,
                    latest_memory_directory_version_no: None,
                    latest_dataset_output_id: None,
                    latest_dataset_output_retrieval_evidence_ids: vec![retrieval_evidence_id],
                    tool_calls: vec![llm_gateway::LlmToolCall {
                        call_id: Some("call_retrieval".to_string()),
                        tool_name: "retrieval.search".to_string(),
                        status: llm_gateway::LlmToolCallStatus::Completed,
                        arguments: Some(json!({ "query": "latest evidence" })),
                        result: Some(
                            json!({ "hits": [{ "retrieval_evidence_id": retrieval_evidence_id }] }),
                        ),
                    }],
                    service_handoff: None,
                    turn_stream_mode: "buffered".to_string(),
                    turn_id: "turn_merged_tools".to_string(),
                    turn_started_at: Utc::now(),
                },
                requested_at,
            )
            .expect("chat session with precomputed tool call should succeed");

        assert_eq!(outcome.runtime.tool_trace_count, 2);
        assert_eq!(
            outcome.message_manifest["runtime"]["tool_trace_count"],
            json!(2)
        );
        assert_eq!(
            outcome.message_manifest["tool_trace"][0]["tool_name"],
            json!("retrieval.search")
        );
        assert_eq!(
            outcome.message_manifest["tool_trace"][1]["tool_name"],
            json!("document.compare")
        );
    }

    #[test]
    fn assistant_message_manifest_carries_report_service_handoff() {
        let requested_at = Utc::now();
        let responded_at = requested_at + chrono::TimeDelta::milliseconds(50);
        let report_plan_id = domain_model::ReportPlanId::new();
        let manifest = render_assistant_message_manifest(
            &ChatSessionJob {
                dataset_id: DatasetId::new(),
                initial_prompt: "Summarize the dataset".to_string(),
                prompt: "Turn this into a report".to_string(),
                indexed_document_count: 2,
                refreshed_chunks: 4,
                prior_message_count: 1,
                latest_memory_directory_id: None,
                latest_memory_directory_version_no: Some(3),
                latest_dataset_output_id: None,
                latest_dataset_output_retrieval_evidence_ids: vec![],
                tool_calls: vec![],
                service_handoff: Some(contracts::ManifestServiceHandoffView {
                    source: contracts::ManifestServiceHandoffSourceView::ChatSessionReportEntry,
                    service_lane: contracts::ModelFacingServiceLaneView::ReportService,
                    report_entry_state: contracts::ModelFacingReportEntryStateView::Confirmed,
                    requested_at: Some(requested_at),
                    resolved_at: Some(responded_at),
                    resolved_action: Some(
                        contracts::ChatSessionReportEntryResolutionView::EnterReportService,
                    ),
                    suggested_title: Some("Dataset Report".to_string()),
                    suggested_objective: Some(
                        "Turn the current dataset context into a report-ready output.".to_string(),
                    ),
                    confirmed_report_plan_id: Some(report_plan_id),
                }),
                turn_stream_mode: "buffered".to_string(),
                turn_id: "turn_handoff".to_string(),
                turn_started_at: requested_at,
            },
            "Report handoff acknowledged",
            &LlmRuntimeMetadata {
                mode: llm_gateway::LlmRuntimeMode::Placeholder,
                provider: "placeholder".to_string(),
                model: PLACEHOLDER_MODEL.to_string(),
                lane: Some(MODEL_LANE_CHAT_SESSION.to_string()),
                request_id: Some("req_handoff".to_string()),
                finish_reason: Some(LlmFinishReason::Stop),
                provider_failure: None,
                latency_ms: Some(50),
                usage: None,
                system_prompt_key: Some(CHAT_SESSION_PLACEHOLDER_PROMPT_KEY.to_string()),
                system_prompt_version: Some("v1".to_string()),
                tool_trace_count: 0,
            },
            &[],
            requested_at,
            responded_at,
        );

        assert_eq!(
            manifest["service_handoff"]["source"],
            json!("chat_session_report_entry")
        );
        assert_eq!(
            manifest["service_handoff"]["service_lane"],
            json!("report_service")
        );
        assert_eq!(
            manifest["service_handoff"]["report_entry_state"],
            json!("confirmed")
        );
        assert_eq!(
            manifest["service_handoff"]["resolved_action"],
            json!("enter_report_service")
        );
        assert_eq!(
            manifest["service_handoff"]["confirmed_report_plan_id"],
            json!(report_plan_id)
        );
    }

    #[test]
    fn session_manifests_preserve_report_entry_handoff() {
        let requested_at = Utc::now();
        let responded_at = requested_at + chrono::TimeDelta::milliseconds(50);
        let report_plan_id = domain_model::ReportPlanId::new();
        let job = ChatSessionJob {
            dataset_id: DatasetId::new(),
            initial_prompt: "Summarize the dataset".to_string(),
            prompt: "Turn this into a report".to_string(),
            indexed_document_count: 2,
            refreshed_chunks: 4,
            prior_message_count: 3,
            latest_memory_directory_id: None,
            latest_memory_directory_version_no: Some(3),
            latest_dataset_output_id: None,
            latest_dataset_output_retrieval_evidence_ids: vec![],
            tool_calls: vec![],
            service_handoff: Some(contracts::ManifestServiceHandoffView {
                source: contracts::ManifestServiceHandoffSourceView::ChatSessionReportEntry,
                service_lane: contracts::ModelFacingServiceLaneView::ReportService,
                report_entry_state: contracts::ModelFacingReportEntryStateView::Confirmed,
                requested_at: Some(requested_at),
                resolved_at: Some(responded_at),
                resolved_action: Some(
                    contracts::ChatSessionReportEntryResolutionView::EnterReportService,
                ),
                suggested_title: Some("Dataset Report".to_string()),
                suggested_objective: Some(
                    "Turn the current dataset context into a report-ready output.".to_string(),
                ),
                confirmed_report_plan_id: Some(report_plan_id),
            }),
            turn_stream_mode: "buffered".to_string(),
            turn_id: "turn_session_handoff".to_string(),
            turn_started_at: requested_at,
        };
        let runtime = LlmRuntimeMetadata {
            mode: llm_gateway::LlmRuntimeMode::Placeholder,
            provider: "placeholder".to_string(),
            model: PLACEHOLDER_MODEL.to_string(),
            lane: Some(MODEL_LANE_CHAT_SESSION.to_string()),
            request_id: Some("req_session_handoff".to_string()),
            finish_reason: Some(LlmFinishReason::Stop),
            provider_failure: None,
            latency_ms: Some(50),
            usage: None,
            system_prompt_key: Some(CHAT_SESSION_PLACEHOLDER_PROMPT_KEY.to_string()),
            system_prompt_version: Some("v1".to_string()),
            tool_trace_count: 0,
        };

        let manifests = [
            render_in_flight_session_manifest(&job, requested_at),
            render_response_ready_session_manifest(
                &job,
                "Report handoff acknowledged",
                &runtime,
                &[],
                requested_at,
                responded_at,
            ),
            render_completed_session_manifest(&job),
            render_failed_session_manifest(
                &job,
                requested_at,
                responded_at,
                Some("Report handoff failed"),
                Some(&runtime),
                &[],
                None,
            ),
        ];

        for manifest in manifests {
            assert_eq!(manifest["report_entry"]["state"], json!("confirmed"));
            assert_eq!(
                manifest["report_entry"]["resolved_action"],
                json!("enter_report_service")
            );
            assert_eq!(
                manifest["report_entry"]["confirmed_report_plan_id"],
                json!(report_plan_id)
            );
        }
    }

    #[test]
    fn in_flight_session_manifest_exposes_pending_provider_request_event() {
        let dataset_id = DatasetId::new();
        let started_at = Utc::now();
        let requested_at = started_at + chrono::TimeDelta::milliseconds(25);
        let manifest = render_in_flight_session_manifest(
            &ChatSessionJob {
                dataset_id,
                initial_prompt: "Initial prompt".to_string(),
                prompt: "Current prompt".to_string(),
                indexed_document_count: 0,
                refreshed_chunks: 0,
                prior_message_count: 0,
                latest_memory_directory_id: None,
                latest_memory_directory_version_no: Some(2),
                latest_dataset_output_id: None,
                latest_dataset_output_retrieval_evidence_ids: vec![],
                tool_calls: vec![],
                service_handoff: None,
                turn_stream_mode: "streaming".to_string(),
                turn_id: "turn_inflight".to_string(),
                turn_started_at: started_at,
            },
            requested_at,
        );

        assert_eq!(manifest["status"], json!("pending_assistant_reply"));
        assert_eq!(manifest["last_turn"]["status"], json!("pending"));
        assert_eq!(manifest["last_turn"]["provider_status"], json!("pending"));
        assert_eq!(
            manifest["last_turn"]["tool_loop_status"],
            json!("not_requested")
        );
        assert_eq!(
            manifest["last_turn"]["provider_requested_at"],
            json!(requested_at)
        );
        assert!(manifest["last_turn"]["provider_responded_at"].is_null());
        assert!(manifest["last_turn"]["tool_calls_emitted_at"].is_null());
        assert!(manifest["last_turn"]["tool_loop_settled_at"].is_null());
        assert_eq!(
            manifest["last_turn"]["events"].as_array().map(Vec::len),
            Some(2)
        );
        assert_eq!(
            manifest["last_turn"]["events"][0]["kind"],
            json!("turn_started")
        );
        assert_eq!(
            manifest["last_turn"]["events"][1]["kind"],
            json!("provider_requested")
        );
        assert_eq!(
            manifest["last_turn"]["events"][1]["at"],
            json!(requested_at)
        );
    }

    #[test]
    fn response_ready_session_manifest_exposes_provider_response_before_message_commit() {
        let dataset_id = DatasetId::new();
        let started_at = Utc::now();
        let requested_at = started_at + chrono::TimeDelta::milliseconds(25);
        let responded_at = requested_at + chrono::TimeDelta::milliseconds(120);
        let runtime = LlmRuntimeMetadata {
            mode: llm_gateway::LlmRuntimeMode::Provider,
            provider: "openai".to_string(),
            model: "gpt-5.4".to_string(),
            lane: Some(MODEL_LANE_CHAT_SESSION.to_string()),
            request_id: Some("req_response_ready".to_string()),
            finish_reason: Some(LlmFinishReason::ToolCalls),
            provider_failure: None,
            latency_ms: Some(120),
            usage: None,
            system_prompt_key: Some("chat_session.placeholder".to_string()),
            system_prompt_version: Some("v1".to_string()),
            tool_trace_count: 2,
        };
        let manifest = render_response_ready_session_manifest(
            &ChatSessionJob {
                dataset_id,
                initial_prompt: "Initial prompt".to_string(),
                prompt: "Current prompt".to_string(),
                indexed_document_count: 0,
                refreshed_chunks: 0,
                prior_message_count: 0,
                latest_memory_directory_id: None,
                latest_memory_directory_version_no: Some(2),
                latest_dataset_output_id: None,
                latest_dataset_output_retrieval_evidence_ids: vec![],
                tool_calls: vec![],
                service_handoff: None,
                turn_stream_mode: "streaming".to_string(),
                turn_id: "turn_response_ready".to_string(),
                turn_started_at: started_at,
            },
            "Assistant response",
            &runtime,
            &[
                LlmToolCall {
                    call_id: Some("call_response_ready".to_string()),
                    tool_name: "weather.lookup".to_string(),
                    status: LlmToolCallStatus::Completed,
                    arguments: Some(json!({ "city": "Shanghai" })),
                    result: Some(json!({ "summary": "sunny" })),
                },
                LlmToolCall {
                    call_id: Some("call_response_ready_pending".to_string()),
                    tool_name: "weather.prepare".to_string(),
                    status: LlmToolCallStatus::Requested,
                    arguments: Some(json!({ "city": "Shanghai" })),
                    result: None,
                },
            ],
            requested_at,
            responded_at,
        );

        assert_eq!(manifest["status"], json!("pending_assistant_reply"));
        assert_eq!(manifest["last_turn"]["status"], json!("pending"));
        assert_eq!(manifest["last_turn"]["stream_status"], json!("completed"));
        assert_eq!(manifest["last_turn"]["provider_status"], json!("responded"));
        assert_eq!(
            manifest["last_turn"]["artifact_commit_status"],
            json!("pending")
        );
        assert_eq!(manifest["last_turn"]["tool_loop_status"], json!("pending"));
        assert_eq!(
            manifest["last_turn"]["provider_request_id"],
            json!("req_response_ready")
        );
        assert_eq!(
            manifest["last_turn"]["provider_requested_at"],
            json!(requested_at)
        );
        assert_eq!(
            manifest["last_turn"]["provider_responded_at"],
            json!(responded_at)
        );
        assert_eq!(
            manifest["last_turn"]["artifact_commit_ready_at"],
            json!(responded_at)
        );
        assert_eq!(manifest["last_turn"]["first_token_at"], json!(responded_at));
        assert_eq!(
            manifest["last_turn"]["stream_completed_at"],
            json!(responded_at)
        );
        assert_eq!(
            manifest["last_turn"]["recovery"]["assistant_message_content"],
            json!("Assistant response")
        );
        assert_eq!(
            manifest["last_turn"]["recovery"]["runtime"]["request_id"],
            json!("req_response_ready")
        );
        assert_eq!(
            manifest["last_turn"]["recovery"]["tool_trace"][0]["tool_name"],
            json!("weather.lookup")
        );
        assert_eq!(
            manifest["last_turn"]["tool_calls_emitted_at"],
            json!(responded_at)
        );
        assert!(manifest["last_turn"]["tool_loop_settled_at"].is_null());
        assert_eq!(manifest["last_turn"]["finish_reason"], json!("tool_calls"));
        assert_eq!(manifest["last_turn"]["completed_at"], Value::Null);
        assert_eq!(
            manifest["last_turn"]["tool_status_summary"]["requested_count"],
            json!(1)
        );
        assert_eq!(
            manifest["last_turn"]["tool_status_summary"]["completed_count"],
            json!(1)
        );
        assert_eq!(
            manifest["last_turn"]["tool_status_summary"]["failed_count"],
            json!(0)
        );
        assert_eq!(
            manifest["last_turn"]["events"].as_array().map(Vec::len),
            Some(7)
        );
        assert_eq!(
            manifest["last_turn"]["events"][2]["kind"],
            json!("provider_responded")
        );
        assert_eq!(
            manifest["last_turn"]["events"][2]["at"],
            json!(responded_at)
        );
        assert_eq!(
            manifest["last_turn"]["events"][3]["kind"],
            json!("first_token_emitted")
        );
        assert_eq!(
            manifest["last_turn"]["events"][4]["kind"],
            json!("stream_completed")
        );
        assert_eq!(
            manifest["last_turn"]["events"][5]["kind"],
            json!("artifact_commit_ready")
        );
        assert_eq!(
            manifest["last_turn"]["events"][6]["kind"],
            json!("tool_calls_emitted")
        );
    }

    #[test]
    fn finalize_assistant_message_manifest_marks_turn_completed_after_persist() {
        let persisted_at = Utc::now();
        let assistant_message_id = ChatMessageId::new();
        let manifest = finalize_assistant_message_manifest(
            &json!({
                "turn": {
                    "turn_id": "turn_finalize",
                    "status": "pending",
                    "stream_mode": "buffered",
                    "provider_status": "responded",
                    "provider_request_id": "req_finalize",
                    "provider_requested_at": persisted_at,
                    "provider_responded_at": persisted_at,
                    "finish_reason": "stop",
                    "assistant_message_id": Value::Null,
                    "assistant_message_persisted_at": Value::Null,
                    "tool_trace_count": 0,
                    "events": [
                        {
                            "kind": "turn_started",
                            "at": persisted_at,
                            "provider_request_id": Value::Null,
                            "finish_reason": Value::Null,
                            "tool_trace_count": Value::Null
                        },
                        {
                            "kind": "provider_requested",
                            "at": persisted_at,
                            "provider_request_id": "req_finalize",
                            "finish_reason": Value::Null,
                            "tool_trace_count": Value::Null
                        },
                        {
                            "kind": "provider_responded",
                            "at": persisted_at,
                            "provider_request_id": "req_finalize",
                            "finish_reason": "stop",
                            "tool_trace_count": 0
                        }
                    ],
                    "started_at": persisted_at,
                    "completed_at": Value::Null
                }
            }),
            assistant_message_id,
            persisted_at,
        );

        assert_eq!(manifest["turn"]["status"], json!("completed"));
        assert_eq!(
            manifest["turn"]["artifact_commit_status"],
            json!("completed")
        );
        assert_eq!(
            manifest["turn"]["assistant_message_id"],
            json!(assistant_message_id.to_string())
        );
        assert_eq!(
            manifest["turn"]["artifact_commit_ready_at"],
            json!(persisted_at)
        );
        assert_eq!(
            manifest["turn"]["assistant_message_persisted_at"],
            json!(persisted_at)
        );
        assert_eq!(manifest["turn"]["completed_at"], json!(persisted_at));
        assert_eq!(manifest["turn"]["events"].as_array().map(Vec::len), Some(5));
        assert_eq!(
            manifest["turn"]["events"][3]["kind"],
            json!("assistant_message_persisted")
        );
        assert_eq!(
            manifest["turn"]["events"][4]["kind"],
            json!("turn_completed")
        );
    }

    #[test]
    fn failed_session_manifest_marks_turn_failed_for_polling_clients() {
        let dataset_id = DatasetId::new();
        let started_at = Utc::now();
        let requested_at = started_at + chrono::TimeDelta::milliseconds(25);
        let failed_at = requested_at + chrono::TimeDelta::milliseconds(220);
        let manifest = render_failed_session_manifest(
            &ChatSessionJob {
                dataset_id,
                initial_prompt: "Initial prompt".to_string(),
                prompt: "Current prompt".to_string(),
                indexed_document_count: 0,
                refreshed_chunks: 0,
                prior_message_count: 0,
                latest_memory_directory_id: None,
                latest_memory_directory_version_no: Some(2),
                latest_dataset_output_id: None,
                latest_dataset_output_retrieval_evidence_ids: vec![],
                tool_calls: vec![],
                service_handoff: None,
                turn_stream_mode: "buffered".to_string(),
                turn_id: "turn_failed".to_string(),
                turn_started_at: started_at,
            },
            requested_at,
            failed_at,
            None,
            None,
            &[],
            None,
        );

        assert_eq!(manifest["status"], json!("pending_assistant_reply"));
        assert_eq!(manifest["last_turn"]["status"], json!("failed"));
        assert_eq!(manifest["last_turn"]["provider_status"], json!("failed"));
        assert_eq!(
            manifest["last_turn"]["artifact_commit_status"],
            json!("not_ready")
        );
        assert!(manifest["last_turn"]["artifact_commit_failure_source"].is_null());
        assert_eq!(
            manifest["last_turn"]["tool_loop_status"],
            json!("not_requested")
        );
        assert_eq!(manifest["last_turn"]["finish_reason"], json!("error"));
        assert_eq!(
            manifest["last_turn"]["provider_requested_at"],
            json!(requested_at)
        );
        assert!(manifest["last_turn"]["provider_responded_at"].is_null());
        assert!(manifest["last_turn"]["artifact_commit_ready_at"].is_null());
        assert!(manifest["last_turn"]["tool_calls_emitted_at"].is_null());
        assert!(manifest["last_turn"]["tool_loop_settled_at"].is_null());
        assert!(manifest["last_turn"]["recovery"].is_null());
        assert_eq!(manifest["last_turn"]["completed_at"], json!(failed_at));
        assert_eq!(
            manifest["last_turn"]["events"].as_array().map(Vec::len),
            Some(3)
        );
        assert_eq!(
            manifest["last_turn"]["events"][2]["kind"],
            json!("turn_failed")
        );
        assert_eq!(manifest["last_turn"]["events"][2]["at"], json!(failed_at));
    }

    #[test]
    fn failed_session_manifest_preserves_runtime_metadata_when_provider_reports_error() {
        let dataset_id = DatasetId::new();
        let started_at = Utc::now();
        let requested_at = started_at + chrono::TimeDelta::milliseconds(25);
        let failed_at = requested_at + chrono::TimeDelta::milliseconds(220);
        let runtime = LlmRuntimeMetadata {
            mode: llm_gateway::LlmRuntimeMode::Provider,
            provider: "openai".to_string(),
            model: "gpt-5.4".to_string(),
            lane: Some(MODEL_LANE_CHAT_SESSION.to_string()),
            request_id: Some("req_failed_runtime".to_string()),
            finish_reason: Some(LlmFinishReason::Error),
            provider_failure: Some(llm_gateway::LlmProviderFailure {
                kind: llm_gateway::LlmProviderFailureKind::FinishReasonError,
                message: "openai returned finish_reason=error".to_string(),
            }),
            latency_ms: Some(220),
            usage: None,
            system_prompt_key: Some("chat_session.placeholder".to_string()),
            system_prompt_version: Some("v1".to_string()),
            tool_trace_count: 1,
        };
        let manifest = render_failed_session_manifest(
            &ChatSessionJob {
                dataset_id,
                initial_prompt: "Initial prompt".to_string(),
                prompt: "Current prompt".to_string(),
                indexed_document_count: 0,
                refreshed_chunks: 0,
                prior_message_count: 0,
                latest_memory_directory_id: None,
                latest_memory_directory_version_no: Some(2),
                latest_dataset_output_id: None,
                latest_dataset_output_retrieval_evidence_ids: vec![],
                tool_calls: vec![],
                service_handoff: None,
                turn_stream_mode: "buffered".to_string(),
                turn_id: "turn_failed_runtime".to_string(),
                turn_started_at: started_at,
            },
            requested_at,
            failed_at,
            Some("Assistant failed response"),
            Some(&runtime),
            &[LlmToolCall {
                call_id: Some("call_failed_runtime".to_string()),
                tool_name: "weather.lookup".to_string(),
                status: LlmToolCallStatus::Failed,
                arguments: Some(json!({ "city": "Shanghai" })),
                result: Some(json!({ "error": "tool crashed" })),
            }],
            None,
        );

        assert_eq!(
            manifest["last_turn"]["provider_request_id"],
            json!("req_failed_runtime")
        );
        assert_eq!(
            manifest["last_turn"]["provider_requested_at"],
            json!(requested_at)
        );
        assert_eq!(
            manifest["last_turn"]["provider_responded_at"],
            json!(failed_at)
        );
        assert_eq!(manifest["last_turn"]["tool_loop_status"], json!("failed"));
        assert_eq!(
            manifest["last_turn"]["tool_calls_emitted_at"],
            json!(failed_at)
        );
        assert_eq!(
            manifest["last_turn"]["tool_loop_settled_at"],
            json!(failed_at)
        );
        assert_eq!(
            manifest["last_turn"]["tool_status_summary"]["requested_count"],
            json!(0)
        );
        assert_eq!(
            manifest["last_turn"]["tool_status_summary"]["completed_count"],
            json!(0)
        );
        assert_eq!(
            manifest["last_turn"]["tool_status_summary"]["failed_count"],
            json!(1)
        );
        assert_eq!(manifest["last_turn"]["finish_reason"], json!("error"));
        assert_eq!(manifest["last_turn"]["tool_trace_count"], json!(1));
        assert_eq!(
            manifest["last_turn"]["provider_failure"]["kind"],
            json!("finish_reason_error")
        );
        assert_eq!(
            manifest["last_turn"]["provider_failure"]["message"],
            json!("openai returned finish_reason=error")
        );
        assert_eq!(
            manifest["last_turn"]["artifact_commit_status"],
            json!("not_ready")
        );
        assert!(manifest["last_turn"]["artifact_commit_ready_at"].is_null());
        assert!(manifest["last_turn"]["artifact_commit_failure_source"].is_null());
        assert_eq!(
            manifest["last_turn"]["recovery"]["assistant_message_content"],
            json!("Assistant failed response")
        );
        assert_eq!(
            manifest["last_turn"]["recovery"]["runtime"]["request_id"],
            json!("req_failed_runtime")
        );
        assert_eq!(
            manifest["last_turn"]["recovery"]["runtime"]["provider_failure"]["kind"],
            json!("finish_reason_error")
        );
        assert_eq!(
            manifest["last_turn"]["recovery"]["tool_trace"][0]["status"],
            json!("failed")
        );
        assert_eq!(
            manifest["last_turn"]["events"].as_array().map(Vec::len),
            Some(6)
        );
        assert_eq!(
            manifest["last_turn"]["events"][2]["kind"],
            json!("provider_responded")
        );
        assert_eq!(
            manifest["last_turn"]["events"][3]["kind"],
            json!("tool_calls_emitted")
        );
        assert_eq!(
            manifest["last_turn"]["events"][4]["kind"],
            json!("tool_loop_failed")
        );
        assert_eq!(
            manifest["last_turn"]["events"][5]["kind"],
            json!("turn_failed")
        );
    }

    #[test]
    fn failed_session_manifest_keeps_provider_unresponded_for_request_timeouts() {
        let dataset_id = DatasetId::new();
        let started_at = Utc::now();
        let requested_at = started_at + chrono::TimeDelta::milliseconds(25);
        let failed_at = requested_at + chrono::TimeDelta::milliseconds(220);
        let runtime = LlmRuntimeMetadata {
            mode: llm_gateway::LlmRuntimeMode::Provider,
            provider: "openai".to_string(),
            model: "gpt-5.4".to_string(),
            lane: Some(MODEL_LANE_CHAT_SESSION.to_string()),
            request_id: None,
            finish_reason: Some(LlmFinishReason::Error),
            provider_failure: Some(llm_gateway::LlmProviderFailure {
                kind: llm_gateway::LlmProviderFailureKind::RequestTimeout,
                message: "openai request to https://api.example.test timed out".to_string(),
            }),
            latency_ms: Some(220),
            usage: None,
            system_prompt_key: Some("chat_session.placeholder".to_string()),
            system_prompt_version: Some("v1".to_string()),
            tool_trace_count: 0,
        };
        let manifest = render_failed_session_manifest(
            &ChatSessionJob {
                dataset_id,
                initial_prompt: "Initial prompt".to_string(),
                prompt: "Current prompt".to_string(),
                indexed_document_count: 0,
                refreshed_chunks: 0,
                prior_message_count: 0,
                latest_memory_directory_id: None,
                latest_memory_directory_version_no: Some(2),
                latest_dataset_output_id: None,
                latest_dataset_output_retrieval_evidence_ids: vec![],
                tool_calls: vec![],
                service_handoff: None,
                turn_stream_mode: "streaming".to_string(),
                turn_id: "turn_request_timeout".to_string(),
                turn_started_at: started_at,
            },
            requested_at,
            failed_at,
            None,
            Some(&runtime),
            &[],
            None,
        );

        assert_eq!(manifest["last_turn"]["provider_status"], json!("failed"));
        assert_eq!(
            manifest["last_turn"]["provider_failure"]["kind"],
            json!("request_timeout")
        );
        assert!(manifest["last_turn"]["provider_responded_at"].is_null());
        assert!(manifest["last_turn"]["artifact_commit_ready_at"].is_null());
        assert_eq!(
            manifest["last_turn"]["events"].as_array().map(Vec::len),
            Some(3)
        );
        assert_eq!(
            manifest["last_turn"]["events"][0]["kind"],
            json!("turn_started")
        );
        assert_eq!(
            manifest["last_turn"]["events"][1]["kind"],
            json!("provider_requested")
        );
        assert_eq!(
            manifest["last_turn"]["events"][2]["kind"],
            json!("turn_failed")
        );
    }

    #[test]
    fn failed_session_manifest_marks_artifact_commit_failed_after_provider_response() {
        let dataset_id = DatasetId::new();
        let started_at = Utc::now();
        let requested_at = started_at + chrono::TimeDelta::milliseconds(25);
        let failed_at = requested_at + chrono::TimeDelta::milliseconds(220);
        let runtime = LlmRuntimeMetadata {
            mode: llm_gateway::LlmRuntimeMode::Provider,
            provider: "openai".to_string(),
            model: "gpt-5.4".to_string(),
            lane: Some(MODEL_LANE_CHAT_SESSION.to_string()),
            request_id: Some("req_commit_failed".to_string()),
            finish_reason: Some(LlmFinishReason::Stop),
            provider_failure: None,
            latency_ms: Some(220),
            usage: None,
            system_prompt_key: Some("chat_session.placeholder".to_string()),
            system_prompt_version: Some("v1".to_string()),
            tool_trace_count: 0,
        };
        let manifest = render_failed_session_manifest(
            &ChatSessionJob {
                dataset_id,
                initial_prompt: "Initial prompt".to_string(),
                prompt: "Current prompt".to_string(),
                indexed_document_count: 0,
                refreshed_chunks: 0,
                prior_message_count: 0,
                latest_memory_directory_id: None,
                latest_memory_directory_version_no: Some(2),
                latest_dataset_output_id: None,
                latest_dataset_output_retrieval_evidence_ids: vec![],
                tool_calls: vec![],
                service_handoff: None,
                turn_stream_mode: "buffered".to_string(),
                turn_id: "turn_commit_failed".to_string(),
                turn_started_at: started_at,
            },
            requested_at,
            failed_at,
            Some("Assistant commit response"),
            Some(&runtime),
            &[],
            Some("assistant_message_create"),
        );

        assert_eq!(manifest["last_turn"]["provider_status"], json!("responded"));
        assert_eq!(
            manifest["last_turn"]["stream_status"],
            json!("not_requested")
        );
        assert_eq!(
            manifest["last_turn"]["artifact_commit_status"],
            json!("failed")
        );
        assert_eq!(
            manifest["last_turn"]["artifact_commit_failure_source"],
            json!("assistant_message_create")
        );
        assert_eq!(
            manifest["last_turn"]["artifact_commit_ready_at"],
            json!(failed_at)
        );
        assert_eq!(
            manifest["last_turn"]["provider_responded_at"],
            json!(failed_at)
        );
        assert_eq!(
            manifest["last_turn"]["recovery"]["assistant_message_content"],
            json!("Assistant commit response")
        );
        assert_eq!(
            manifest["last_turn"]["events"].as_array().map(Vec::len),
            Some(6)
        );
        assert_eq!(
            manifest["last_turn"]["events"][2]["kind"],
            json!("provider_responded")
        );
        assert_eq!(
            manifest["last_turn"]["events"][3]["kind"],
            json!("artifact_commit_ready")
        );
        assert_eq!(
            manifest["last_turn"]["events"][4]["kind"],
            json!("artifact_commit_failed")
        );
        assert_eq!(
            manifest["last_turn"]["events"][5]["kind"],
            json!("turn_failed")
        );
    }

    #[test]
    fn chat_runtime_error_message_only_flags_error_finish_reason() {
        let runtime = LlmRuntimeMetadata {
            mode: llm_gateway::LlmRuntimeMode::Provider,
            provider: "openai".to_string(),
            model: "gpt-5.4".to_string(),
            lane: Some(MODEL_LANE_CHAT_SESSION.to_string()),
            request_id: Some("req_runtime_error".to_string()),
            finish_reason: Some(LlmFinishReason::Error),
            provider_failure: None,
            latency_ms: Some(1),
            usage: None,
            system_prompt_key: None,
            system_prompt_version: None,
            tool_trace_count: 0,
        };
        assert_eq!(
            chat_runtime_error_message(&runtime),
            Some(
                "chat session provider openai returned finish_reason=error (request_id=req_runtime_error)"
                    .to_string()
            )
        );

        let ok_runtime = LlmRuntimeMetadata {
            finish_reason: Some(LlmFinishReason::Stop),
            ..runtime
        };
        assert_eq!(chat_runtime_error_message(&ok_runtime), None);
    }

    #[test]
    fn chat_runtime_error_message_includes_provider_failure_details() {
        let runtime = LlmRuntimeMetadata {
            mode: llm_gateway::LlmRuntimeMode::Provider,
            provider: "openai".to_string(),
            model: "gpt-5.4".to_string(),
            lane: Some(MODEL_LANE_CHAT_SESSION.to_string()),
            request_id: None,
            finish_reason: Some(LlmFinishReason::Error),
            provider_failure: Some(llm_gateway::LlmProviderFailure {
                kind: llm_gateway::LlmProviderFailureKind::HttpStatus,
                message: "openai returned HTTP 502 with body upstream unavailable".to_string(),
            }),
            latency_ms: Some(1),
            usage: None,
            system_prompt_key: None,
            system_prompt_version: None,
            tool_trace_count: 0,
        };

        assert_eq!(
            chat_runtime_error_message(&runtime),
            Some(
                "chat session provider openai returned finish_reason=error [http_status: openai returned HTTP 502 with body upstream unavailable]"
                    .to_string()
            )
        );
    }
}
