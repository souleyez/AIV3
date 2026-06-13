use chrono::{DateTime, Utc};
use contracts::{LlmInvocationView, ToolExecutionView};
use domain_model::{
    ChatMessage, ChatMessageId, ChatMessageRole, DatasetId, DatasetOutputId, MemoryDirectoryId,
    ReportPlanId, RetrievalEvidenceId,
};
use serde_json::Value;
use uuid::Uuid;

use crate::manifest_runtime_view_support::{
    parse_manifest_context_binding, parse_manifest_finish_reason, parse_manifest_provider_failure,
    parse_manifest_runtime,
};
use crate::manifest_service_handoff_support::{
    parse_chat_session_report_entry_resolution, parse_manifest_service_handoff,
    parse_manifest_timestamp, parse_model_facing_report_entry_state,
};
use crate::runtime_manifest_support::{
    latest_llm_invocation, manifest_finish_reason_from_invocation,
    manifest_runtime_from_latest_llm_invocation, manifest_tool_trace_from_tool_executions,
    summarize_tool_execution_statuses,
};
use crate::tool_view_support::parse_manifest_tool_trace;

fn parse_chat_session_status(value: &str) -> Option<contracts::ChatSessionManifestStatusView> {
    match value {
        "pending_assistant_reply" => {
            Some(contracts::ChatSessionManifestStatusView::PendingAssistantReply)
        }
        "assistant_replied" => Some(contracts::ChatSessionManifestStatusView::AssistantReplied),
        _ => None,
    }
}

fn parse_chat_session_turn_kind(value: &str) -> Option<contracts::ChatSessionTurnKindView> {
    match value {
        "placeholder_orchestration" => {
            Some(contracts::ChatSessionTurnKindView::PlaceholderOrchestration)
        }
        _ => None,
    }
}

fn parse_chat_turn_status(value: &str) -> Option<contracts::ChatTurnStatusView> {
    match value {
        "pending" => Some(contracts::ChatTurnStatusView::Pending),
        "completed" => Some(contracts::ChatTurnStatusView::Completed),
        "failed" => Some(contracts::ChatTurnStatusView::Failed),
        _ => None,
    }
}

fn parse_chat_turn_stream_mode(value: &str) -> Option<contracts::ChatTurnStreamModeView> {
    match value {
        "buffered" => Some(contracts::ChatTurnStreamModeView::Buffered),
        "streaming" => Some(contracts::ChatTurnStreamModeView::Streaming),
        _ => None,
    }
}

fn parse_chat_turn_stream_status(value: &str) -> Option<contracts::ChatTurnStreamStatusView> {
    match value {
        "not_requested" => Some(contracts::ChatTurnStreamStatusView::NotRequested),
        "pending" => Some(contracts::ChatTurnStreamStatusView::Pending),
        "completed" => Some(contracts::ChatTurnStreamStatusView::Completed),
        "failed" => Some(contracts::ChatTurnStreamStatusView::Failed),
        _ => None,
    }
}

fn parse_chat_turn_artifact_commit_status(
    value: &str,
) -> Option<contracts::ChatTurnArtifactCommitStatusView> {
    match value {
        "not_ready" => Some(contracts::ChatTurnArtifactCommitStatusView::NotReady),
        "pending" => Some(contracts::ChatTurnArtifactCommitStatusView::Pending),
        "failed" => Some(contracts::ChatTurnArtifactCommitStatusView::Failed),
        "completed" => Some(contracts::ChatTurnArtifactCommitStatusView::Completed),
        _ => None,
    }
}

fn parse_chat_turn_artifact_commit_failure_source(
    value: &str,
) -> Option<contracts::ChatTurnArtifactCommitFailureSourceView> {
    match value {
        "workflow_event_recovery_persist" => {
            Some(contracts::ChatTurnArtifactCommitFailureSourceView::WorkflowEventRecoveryPersist)
        }
        "response_ready_session_update" => {
            Some(contracts::ChatTurnArtifactCommitFailureSourceView::ResponseReadySessionUpdate)
        }
        "assistant_message_create" => {
            Some(contracts::ChatTurnArtifactCommitFailureSourceView::AssistantMessageCreate)
        }
        "assistant_message_manifest_update" => {
            Some(contracts::ChatTurnArtifactCommitFailureSourceView::AssistantMessageManifestUpdate)
        }
        "llm_invocation_persist" => {
            Some(contracts::ChatTurnArtifactCommitFailureSourceView::LlmInvocationPersist)
        }
        "tool_execution_persist" => {
            Some(contracts::ChatTurnArtifactCommitFailureSourceView::ToolExecutionPersist)
        }
        "session_context_update" => {
            Some(contracts::ChatTurnArtifactCommitFailureSourceView::SessionContextUpdate)
        }
        _ => None,
    }
}

fn parse_chat_turn_provider_status(value: &str) -> Option<contracts::ChatTurnProviderStatusView> {
    match value {
        "pending" => Some(contracts::ChatTurnProviderStatusView::Pending),
        "responded" => Some(contracts::ChatTurnProviderStatusView::Responded),
        "failed" => Some(contracts::ChatTurnProviderStatusView::Failed),
        _ => None,
    }
}

fn parse_chat_turn_tool_loop_status(value: &str) -> Option<contracts::ChatTurnToolLoopStatusView> {
    match value {
        "not_requested" => Some(contracts::ChatTurnToolLoopStatusView::NotRequested),
        "pending" => Some(contracts::ChatTurnToolLoopStatusView::Pending),
        "completed" => Some(contracts::ChatTurnToolLoopStatusView::Completed),
        "failed" => Some(contracts::ChatTurnToolLoopStatusView::Failed),
        _ => None,
    }
}

fn parse_chat_turn_event_kind(value: &str) -> Option<contracts::ChatTurnEventKindView> {
    match value {
        "turn_started" => Some(contracts::ChatTurnEventKindView::TurnStarted),
        "provider_requested" => Some(contracts::ChatTurnEventKindView::ProviderRequested),
        "provider_responded" => Some(contracts::ChatTurnEventKindView::ProviderResponded),
        "first_token_emitted" => Some(contracts::ChatTurnEventKindView::FirstTokenEmitted),
        "stream_completed" => Some(contracts::ChatTurnEventKindView::StreamCompleted),
        "stream_failed" => Some(contracts::ChatTurnEventKindView::StreamFailed),
        "artifact_commit_ready" => Some(contracts::ChatTurnEventKindView::ArtifactCommitReady),
        "artifact_commit_failed" => Some(contracts::ChatTurnEventKindView::ArtifactCommitFailed),
        "tool_calls_emitted" => Some(contracts::ChatTurnEventKindView::ToolCallsEmitted),
        "tool_loop_completed" => Some(contracts::ChatTurnEventKindView::ToolLoopCompleted),
        "tool_loop_failed" => Some(contracts::ChatTurnEventKindView::ToolLoopFailed),
        "assistant_message_persisted" => {
            Some(contracts::ChatTurnEventKindView::AssistantMessagePersisted)
        }
        "turn_completed" => Some(contracts::ChatTurnEventKindView::TurnCompleted),
        "turn_failed" => Some(contracts::ChatTurnEventKindView::TurnFailed),
        _ => None,
    }
}

fn parse_chat_turn_tool_status_summary(
    value: &Value,
) -> Option<contracts::ChatTurnToolStatusSummaryView> {
    let object = value.as_object()?;
    Some(contracts::ChatTurnToolStatusSummaryView {
        requested_count: object.get("requested_count")?.as_u64()? as usize,
        completed_count: object.get("completed_count")?.as_u64()? as usize,
        failed_count: object.get("failed_count")?.as_u64()? as usize,
    })
}

pub(crate) fn build_chat_turn_events(
    turn: &contracts::ChatTurnRuntimeView,
) -> Vec<contracts::ChatTurnEventView> {
    let mut events = vec![contracts::ChatTurnEventView {
        kind: contracts::ChatTurnEventKindView::TurnStarted,
        at: turn.started_at,
        provider_request_id: None,
        finish_reason: None,
        tool_trace_count: None,
    }];

    if turn.provider_request_id.is_some() {
        events.push(contracts::ChatTurnEventView {
            kind: contracts::ChatTurnEventKindView::ProviderRequested,
            at: turn.provider_requested_at.unwrap_or(turn.started_at),
            provider_request_id: turn.provider_request_id.clone(),
            finish_reason: None,
            tool_trace_count: None,
        });
    }

    match turn.provider_status {
        contracts::ChatTurnProviderStatusView::Responded
        | contracts::ChatTurnProviderStatusView::Failed => {
            if let Some(provider_responded_at) = turn.provider_responded_at.or(turn.completed_at) {
                events.push(contracts::ChatTurnEventView {
                    kind: contracts::ChatTurnEventKindView::ProviderResponded,
                    at: provider_responded_at,
                    provider_request_id: turn.provider_request_id.clone(),
                    finish_reason: turn.finish_reason.clone(),
                    tool_trace_count: Some(turn.tool_trace_count),
                });
            }
        }
        contracts::ChatTurnProviderStatusView::Pending => {}
    }

    if let Some(first_token_at) = turn.first_token_at {
        events.push(contracts::ChatTurnEventView {
            kind: contracts::ChatTurnEventKindView::FirstTokenEmitted,
            at: first_token_at,
            provider_request_id: turn.provider_request_id.clone(),
            finish_reason: turn.finish_reason.clone(),
            tool_trace_count: Some(turn.tool_trace_count),
        });
    }

    if let Some(stream_completed_at) = turn.stream_completed_at {
        match turn.stream_status {
            contracts::ChatTurnStreamStatusView::Completed => {
                events.push(contracts::ChatTurnEventView {
                    kind: contracts::ChatTurnEventKindView::StreamCompleted,
                    at: stream_completed_at,
                    provider_request_id: turn.provider_request_id.clone(),
                    finish_reason: turn.finish_reason.clone(),
                    tool_trace_count: Some(turn.tool_trace_count),
                });
            }
            contracts::ChatTurnStreamStatusView::Failed => {
                events.push(contracts::ChatTurnEventView {
                    kind: contracts::ChatTurnEventKindView::StreamFailed,
                    at: stream_completed_at,
                    provider_request_id: turn.provider_request_id.clone(),
                    finish_reason: turn.finish_reason.clone(),
                    tool_trace_count: Some(turn.tool_trace_count),
                });
            }
            contracts::ChatTurnStreamStatusView::NotRequested
            | contracts::ChatTurnStreamStatusView::Pending => {}
        }
    }

    if let Some(artifact_commit_ready_at) = turn.artifact_commit_ready_at {
        events.push(contracts::ChatTurnEventView {
            kind: contracts::ChatTurnEventKindView::ArtifactCommitReady,
            at: artifact_commit_ready_at,
            provider_request_id: turn.provider_request_id.clone(),
            finish_reason: turn.finish_reason.clone(),
            tool_trace_count: Some(turn.tool_trace_count),
        });
    }

    if matches!(
        turn.artifact_commit_status,
        contracts::ChatTurnArtifactCommitStatusView::Failed
    ) {
        if let Some(artifact_commit_failed_at) = turn.completed_at.or(turn.artifact_commit_ready_at)
        {
            events.push(contracts::ChatTurnEventView {
                kind: contracts::ChatTurnEventKindView::ArtifactCommitFailed,
                at: artifact_commit_failed_at,
                provider_request_id: turn.provider_request_id.clone(),
                finish_reason: turn.finish_reason.clone(),
                tool_trace_count: Some(turn.tool_trace_count),
            });
        }
    }

    if let Some(tool_calls_emitted_at) = turn.tool_calls_emitted_at {
        events.push(contracts::ChatTurnEventView {
            kind: contracts::ChatTurnEventKindView::ToolCallsEmitted,
            at: tool_calls_emitted_at,
            provider_request_id: turn.provider_request_id.clone(),
            finish_reason: turn.finish_reason.clone(),
            tool_trace_count: Some(turn.tool_trace_count),
        });
    }

    if let Some(tool_loop_settled_at) = turn.tool_loop_settled_at {
        match turn.tool_loop_status {
            contracts::ChatTurnToolLoopStatusView::Completed => {
                events.push(contracts::ChatTurnEventView {
                    kind: contracts::ChatTurnEventKindView::ToolLoopCompleted,
                    at: tool_loop_settled_at,
                    provider_request_id: turn.provider_request_id.clone(),
                    finish_reason: turn.finish_reason.clone(),
                    tool_trace_count: Some(turn.tool_trace_count),
                });
            }
            contracts::ChatTurnToolLoopStatusView::Failed => {
                events.push(contracts::ChatTurnEventView {
                    kind: contracts::ChatTurnEventKindView::ToolLoopFailed,
                    at: tool_loop_settled_at,
                    provider_request_id: turn.provider_request_id.clone(),
                    finish_reason: turn.finish_reason.clone(),
                    tool_trace_count: Some(turn.tool_trace_count),
                });
            }
            contracts::ChatTurnToolLoopStatusView::NotRequested
            | contracts::ChatTurnToolLoopStatusView::Pending => {}
        }
    }

    if let Some(persisted_at) = turn.assistant_message_persisted_at {
        events.push(contracts::ChatTurnEventView {
            kind: contracts::ChatTurnEventKindView::AssistantMessagePersisted,
            at: persisted_at,
            provider_request_id: turn.provider_request_id.clone(),
            finish_reason: turn.finish_reason.clone(),
            tool_trace_count: Some(turn.tool_trace_count),
        });
    }

    if let Some(completed_at) = turn.completed_at {
        match turn.status {
            contracts::ChatTurnStatusView::Completed => {
                events.push(contracts::ChatTurnEventView {
                    kind: contracts::ChatTurnEventKindView::TurnCompleted,
                    at: completed_at,
                    provider_request_id: turn.provider_request_id.clone(),
                    finish_reason: turn.finish_reason.clone(),
                    tool_trace_count: Some(turn.tool_trace_count),
                });
            }
            contracts::ChatTurnStatusView::Failed => {
                events.push(contracts::ChatTurnEventView {
                    kind: contracts::ChatTurnEventKindView::TurnFailed,
                    at: completed_at,
                    provider_request_id: turn.provider_request_id.clone(),
                    finish_reason: turn.finish_reason.clone(),
                    tool_trace_count: Some(turn.tool_trace_count),
                });
            }
            contracts::ChatTurnStatusView::Pending => {}
        }
    }

    events
}

fn parse_chat_turn_events(
    value: Option<&Value>,
    fallback_turn: &contracts::ChatTurnRuntimeView,
) -> Option<Vec<contracts::ChatTurnEventView>> {
    let Some(value) = value else {
        return Some(build_chat_turn_events(fallback_turn));
    };

    value
        .as_array()?
        .iter()
        .map(|entry| {
            let entry = entry.as_object()?;
            Some(contracts::ChatTurnEventView {
                kind: parse_chat_turn_event_kind(entry.get("kind")?.as_str()?)?,
                at: parse_manifest_timestamp(entry.get("at")?)?,
                provider_request_id: entry
                    .get("provider_request_id")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                finish_reason: entry
                    .get("finish_reason")
                    .and_then(Value::as_str)
                    .map(parse_manifest_finish_reason),
                tool_trace_count: entry
                    .get("tool_trace_count")
                    .and_then(Value::as_u64)
                    .map(|value| value as usize),
            })
        })
        .collect::<Option<Vec<_>>>()
}

pub(crate) fn parse_chat_turn_runtime(value: &Value) -> Option<contracts::ChatTurnRuntimeView> {
    let object = value.as_object()?;
    let status = parse_chat_turn_status(object.get("status")?.as_str()?)?;
    let stream_mode = parse_chat_turn_stream_mode(object.get("stream_mode")?.as_str()?)?;
    let finish_reason = object
        .get("finish_reason")
        .and_then(Value::as_str)
        .map(parse_manifest_finish_reason);
    let tool_status_summary = object
        .get("tool_status_summary")
        .and_then(parse_chat_turn_tool_status_summary);
    let provider_requested_at = object
        .get("provider_requested_at")
        .and_then(parse_manifest_timestamp);
    let provider_responded_at = object
        .get("provider_responded_at")
        .and_then(parse_manifest_timestamp);
    let artifact_commit_ready_at = object
        .get("artifact_commit_ready_at")
        .and_then(parse_manifest_timestamp);
    let assistant_message_persisted_at = object
        .get("assistant_message_persisted_at")
        .and_then(parse_manifest_timestamp);
    let completed_at = object
        .get("completed_at")
        .and_then(parse_manifest_timestamp);
    let provider_status = object
        .get("provider_status")
        .and_then(Value::as_str)
        .and_then(parse_chat_turn_provider_status)
        .unwrap_or_else(
            || match (&status, finish_reason.as_ref(), provider_responded_at) {
                (contracts::ChatTurnStatusView::Pending, _, _) => {
                    contracts::ChatTurnProviderStatusView::Pending
                }
                (_, Some(contracts::ManifestFinishReasonView::Error), _) => {
                    contracts::ChatTurnProviderStatusView::Failed
                }
                (_, _, Some(_)) => contracts::ChatTurnProviderStatusView::Responded,
                (contracts::ChatTurnStatusView::Failed, _, None) => {
                    contracts::ChatTurnProviderStatusView::Failed
                }
                (_, _, None) => contracts::ChatTurnProviderStatusView::Responded,
            },
        );
    let provider_failure = object
        .get("provider_failure")
        .and_then(parse_manifest_provider_failure);
    let stream_status = object
        .get("stream_status")
        .and_then(Value::as_str)
        .and_then(parse_chat_turn_stream_status)
        .unwrap_or_else(|| {
            infer_chat_turn_stream_status(
                &stream_mode,
                &provider_status,
                provider_requested_at,
                provider_responded_at,
                completed_at,
            )
        });
    let artifact_commit_status = object
        .get("artifact_commit_status")
        .and_then(Value::as_str)
        .and_then(parse_chat_turn_artifact_commit_status)
        .unwrap_or_else(|| {
            infer_chat_turn_artifact_commit_status(
                &status,
                assistant_message_persisted_at,
                provider_responded_at,
                artifact_commit_ready_at,
            )
        });
    let artifact_commit_failure_source = object
        .get("artifact_commit_failure_source")
        .and_then(Value::as_str)
        .and_then(parse_chat_turn_artifact_commit_failure_source);
    let tool_loop_status = object
        .get("tool_loop_status")
        .and_then(Value::as_str)
        .and_then(parse_chat_turn_tool_loop_status)
        .unwrap_or_else(|| {
            infer_chat_turn_tool_loop_status(
                object
                    .get("tool_trace_count")
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as usize,
                tool_status_summary.as_ref(),
            )
        });

    let mut turn = contracts::ChatTurnRuntimeView {
        turn_id: object.get("turn_id")?.as_str()?.to_string(),
        status,
        stream_mode,
        stream_status,
        artifact_commit_status,
        provider_status,
        provider_failure,
        tool_loop_status,
        provider_request_id: object
            .get("provider_request_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        provider_requested_at,
        provider_responded_at,
        first_token_at: object
            .get("first_token_at")
            .and_then(parse_manifest_timestamp),
        stream_completed_at: object
            .get("stream_completed_at")
            .and_then(parse_manifest_timestamp),
        artifact_commit_ready_at,
        artifact_commit_failure_source,
        tool_calls_emitted_at: object
            .get("tool_calls_emitted_at")
            .and_then(parse_manifest_timestamp),
        tool_loop_settled_at: object
            .get("tool_loop_settled_at")
            .and_then(parse_manifest_timestamp),
        finish_reason,
        assistant_message_id: object
            .get("assistant_message_id")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .map(domain_model::ChatMessageId::from),
        assistant_message_persisted_at,
        tool_trace_count: object.get("tool_trace_count")?.as_u64()? as usize,
        tool_status_summary,
        events: Vec::new(),
        started_at: parse_manifest_timestamp(object.get("started_at")?)?,
        completed_at,
    };
    if turn.tool_calls_emitted_at.is_none() && turn.tool_trace_count > 0 {
        turn.tool_calls_emitted_at = turn.provider_responded_at.or(turn.completed_at);
    }
    if turn.first_token_at.is_none() {
        turn.first_token_at = infer_chat_turn_first_token_at(&turn);
    }
    if turn.stream_completed_at.is_none() {
        turn.stream_completed_at = infer_chat_turn_stream_completed_at(&turn);
    }
    if turn.artifact_commit_ready_at.is_none() {
        turn.artifact_commit_ready_at = infer_chat_turn_artifact_commit_ready_at(&turn);
    }
    turn.artifact_commit_status = infer_chat_turn_artifact_commit_status(
        &turn.status,
        turn.assistant_message_persisted_at,
        turn.provider_responded_at,
        turn.artifact_commit_ready_at,
    );
    if !matches!(
        turn.artifact_commit_status,
        contracts::ChatTurnArtifactCommitStatusView::Failed
    ) {
        turn.artifact_commit_failure_source = None;
    }
    if turn.tool_loop_settled_at.is_none() {
        turn.tool_loop_settled_at = match turn.tool_loop_status {
            contracts::ChatTurnToolLoopStatusView::Completed
            | contracts::ChatTurnToolLoopStatusView::Failed => {
                turn.completed_at.or(turn.provider_responded_at)
            }
            contracts::ChatTurnToolLoopStatusView::NotRequested
            | contracts::ChatTurnToolLoopStatusView::Pending => None,
        };
    }
    turn.events = parse_chat_turn_events(object.get("events"), &turn)?;

    Some(turn)
}

pub(crate) fn parse_chat_session_manifest(
    value: &Value,
) -> Option<contracts::ChatSessionManifestView> {
    let object = value.as_object()?;
    let report_entry = object.get("report_entry").and_then(|value| {
        let entry = value.as_object()?;
        Some(contracts::ChatSessionReportEntryView {
            state: parse_model_facing_report_entry_state(entry.get("state")?.as_str()?)?,
            requested_at: entry.get("requested_at").and_then(parse_manifest_timestamp),
            resolved_at: entry.get("resolved_at").and_then(parse_manifest_timestamp),
            resolved_action: entry
                .get("resolved_action")
                .and_then(Value::as_str)
                .and_then(parse_chat_session_report_entry_resolution),
            suggested_title: entry
                .get("suggested_title")
                .and_then(Value::as_str)
                .map(str::to_string),
            suggested_objective: entry
                .get("suggested_objective")
                .and_then(Value::as_str)
                .map(str::to_string),
            confirmed_report_plan_id: entry
                .get("confirmed_report_plan_id")
                .and_then(Value::as_str)
                .and_then(|value| Uuid::parse_str(value).ok())
                .map(ReportPlanId::from),
        })
    });

    Some(contracts::ChatSessionManifestView {
        generator: object
            .get("generator")
            .and_then(Value::as_str)
            .map(str::to_string),
        schema_version: object
            .get("schema_version")
            .and_then(Value::as_str)
            .map(str::to_string),
        status: parse_chat_session_status(object.get("status")?.as_str()?)?,
        initial_prompt: object
            .get("initial_prompt")
            .and_then(Value::as_str)
            .map(str::to_string),
        last_prompt: object
            .get("last_prompt")
            .and_then(Value::as_str)
            .map(str::to_string),
        last_turn_kind: object
            .get("last_turn_kind")
            .and_then(Value::as_str)
            .and_then(parse_chat_session_turn_kind),
        context_binding: object
            .get("context_binding")
            .and_then(Value::as_str)
            .and_then(parse_manifest_context_binding),
        latest_memory_directory_id: object
            .get("latest_memory_directory_id")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .map(MemoryDirectoryId::from),
        latest_memory_directory_version_no: object
            .get("latest_memory_directory_version_no")
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok()),
        latest_dataset_output_id: object
            .get("latest_dataset_output_id")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .map(DatasetOutputId::from),
        report_entry,
        last_turn: object.get("last_turn").and_then(parse_chat_turn_runtime),
        runtime: object.get("runtime").and_then(parse_manifest_runtime),
    })
}

fn parse_chat_message_output(value: &Value) -> Option<contracts::ChatMessageOutputView> {
    let output = value.as_object()?;
    let parse_chat_message_output_format = |value: &str| match value {
        "markdown" => Some(contracts::ChatMessageOutputFormatView::Markdown),
        _ => None,
    };
    let parse_chat_message_section_kind = |value: &str| match value {
        "reply" => Some(contracts::ChatMessageSectionKindView::Reply),
        _ => None,
    };
    let sections = output
        .get("sections")?
        .as_array()?
        .iter()
        .map(|section| {
            let section = section.as_object()?;
            Some(contracts::ChatMessageSectionView {
                section_key: section.get("section_key")?.as_str()?.to_string(),
                kind: parse_chat_message_section_kind(section.get("kind")?.as_str()?)?,
                title: section.get("title")?.as_str()?.to_string(),
                content: section.get("content")?.as_str()?.to_string(),
                retrieval_evidence_ids: section
                    .get("retrieval_evidence_ids")?
                    .as_array()?
                    .iter()
                    .map(|evidence_id| {
                        let evidence_id = evidence_id.as_str()?;
                        Some(RetrievalEvidenceId::from(
                            Uuid::parse_str(evidence_id).ok()?,
                        ))
                    })
                    .collect::<Option<Vec<_>>>()?,
            })
        })
        .collect::<Option<Vec<_>>>()?;

    Some(contracts::ChatMessageOutputView {
        format: parse_chat_message_output_format(output.get("format")?.as_str()?)?,
        sections,
    })
}

pub(crate) fn parse_chat_message_manifest(
    value: &Value,
) -> Option<contracts::ChatMessageManifestView> {
    let object = value.as_object()?;
    let tool_trace = parse_manifest_tool_trace(object.get("tool_trace"))?;
    let legacy_tool_trace_count = Some(tool_trace.len());
    let runtime = object
        .get("runtime")
        .and_then(parse_manifest_runtime)
        .or_else(|| {
            object
                .get("generator")
                .and_then(Value::as_str)
                .filter(|generator| *generator == "chat-session-worker")
                .map(|_| contracts::ManifestRuntimeView {
                    mode: contracts::ManifestRuntimeModeView::Placeholder,
                    provider: None,
                    model: None,
                    request_id: None,
                    finish_reason: None,
                    provider_failure: None,
                    latency_ms: None,
                    usage: None,
                    system_prompt_key: None,
                    system_prompt_version: None,
                    tool_trace_count: legacy_tool_trace_count,
                })
        });

    Some(contracts::ChatMessageManifestView {
        generator: object.get("generator")?.as_str()?.to_string(),
        schema_version: object.get("schema_version")?.as_str()?.to_string(),
        dataset_id: DatasetId::from(Uuid::parse_str(object.get("dataset_id")?.as_str()?).ok()?),
        prompt: object.get("prompt")?.as_str()?.to_string(),
        indexed_document_count: object.get("indexed_document_count")?.as_u64()? as usize,
        refreshed_chunks: object.get("refreshed_chunks")?.as_u64()? as usize,
        prior_message_count: object.get("prior_message_count")?.as_u64()? as usize,
        latest_memory_directory_id: object
            .get("latest_memory_directory_id")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .map(MemoryDirectoryId::from),
        latest_memory_directory_version_no: object
            .get("latest_memory_directory_version_no")
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok()),
        latest_dataset_output_id: object
            .get("latest_dataset_output_id")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .map(DatasetOutputId::from),
        output: object.get("output").and_then(parse_chat_message_output),
        service_handoff: object
            .get("service_handoff")
            .and_then(parse_manifest_service_handoff),
        tool_trace,
        turn: object.get("turn").and_then(parse_chat_turn_runtime),
        context_binding: parse_manifest_context_binding(object.get("context_binding")?.as_str()?)?,
        runtime,
    })
}

pub(crate) fn infer_chat_turn_tool_loop_status(
    tool_trace_count: usize,
    summary: Option<&contracts::ChatTurnToolStatusSummaryView>,
) -> contracts::ChatTurnToolLoopStatusView {
    let Some(summary) = summary else {
        return if tool_trace_count > 0 {
            contracts::ChatTurnToolLoopStatusView::Pending
        } else {
            contracts::ChatTurnToolLoopStatusView::NotRequested
        };
    };

    if summary.requested_count > 0 {
        contracts::ChatTurnToolLoopStatusView::Pending
    } else if summary.failed_count > 0 {
        contracts::ChatTurnToolLoopStatusView::Failed
    } else if summary.completed_count > 0 || tool_trace_count > 0 {
        contracts::ChatTurnToolLoopStatusView::Completed
    } else {
        contracts::ChatTurnToolLoopStatusView::NotRequested
    }
}

pub(crate) fn infer_chat_turn_stream_status(
    stream_mode: &contracts::ChatTurnStreamModeView,
    provider_status: &contracts::ChatTurnProviderStatusView,
    provider_requested_at: Option<DateTime<Utc>>,
    provider_responded_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
) -> contracts::ChatTurnStreamStatusView {
    if matches!(stream_mode, contracts::ChatTurnStreamModeView::Buffered) {
        return contracts::ChatTurnStreamStatusView::NotRequested;
    }

    match provider_status {
        contracts::ChatTurnProviderStatusView::Failed => {
            contracts::ChatTurnStreamStatusView::Failed
        }
        contracts::ChatTurnProviderStatusView::Responded => {
            if provider_responded_at.is_some() || completed_at.is_some() {
                contracts::ChatTurnStreamStatusView::Completed
            } else {
                contracts::ChatTurnStreamStatusView::Pending
            }
        }
        contracts::ChatTurnProviderStatusView::Pending => {
            if provider_requested_at.is_some() {
                contracts::ChatTurnStreamStatusView::Pending
            } else {
                contracts::ChatTurnStreamStatusView::NotRequested
            }
        }
    }
}

pub(crate) fn infer_chat_turn_first_token_at(
    turn: &contracts::ChatTurnRuntimeView,
) -> Option<DateTime<Utc>> {
    if matches!(
        turn.stream_mode,
        contracts::ChatTurnStreamModeView::Buffered
    ) {
        return None;
    }

    match turn.stream_status {
        contracts::ChatTurnStreamStatusView::Completed => {
            turn.provider_responded_at.or(turn.completed_at)
        }
        contracts::ChatTurnStreamStatusView::NotRequested
        | contracts::ChatTurnStreamStatusView::Pending
        | contracts::ChatTurnStreamStatusView::Failed => None,
    }
}

pub(crate) fn infer_chat_turn_stream_completed_at(
    turn: &contracts::ChatTurnRuntimeView,
) -> Option<DateTime<Utc>> {
    if matches!(
        turn.stream_mode,
        contracts::ChatTurnStreamModeView::Buffered
    ) {
        return None;
    }

    match turn.stream_status {
        contracts::ChatTurnStreamStatusView::Completed
        | contracts::ChatTurnStreamStatusView::Failed => {
            turn.provider_responded_at.or(turn.completed_at)
        }
        contracts::ChatTurnStreamStatusView::NotRequested
        | contracts::ChatTurnStreamStatusView::Pending => None,
    }
}

pub(crate) fn infer_chat_turn_artifact_commit_ready_at(
    turn: &contracts::ChatTurnRuntimeView,
) -> Option<DateTime<Utc>> {
    if matches!(
        turn.stream_mode,
        contracts::ChatTurnStreamModeView::Buffered
    ) {
        match turn.provider_status {
            contracts::ChatTurnProviderStatusView::Responded => {
                return turn.provider_responded_at.or(turn.completed_at);
            }
            contracts::ChatTurnProviderStatusView::Pending
            | contracts::ChatTurnProviderStatusView::Failed => return None,
        }
    }

    match turn.stream_status {
        contracts::ChatTurnStreamStatusView::Completed => turn
            .stream_completed_at
            .or(turn.provider_responded_at)
            .or(turn.completed_at),
        contracts::ChatTurnStreamStatusView::NotRequested
        | contracts::ChatTurnStreamStatusView::Pending
        | contracts::ChatTurnStreamStatusView::Failed => None,
    }
}

pub(crate) fn infer_chat_turn_artifact_commit_status(
    status: &contracts::ChatTurnStatusView,
    assistant_message_persisted_at: Option<DateTime<Utc>>,
    provider_responded_at: Option<DateTime<Utc>>,
    artifact_commit_ready_at: Option<DateTime<Utc>>,
) -> contracts::ChatTurnArtifactCommitStatusView {
    if assistant_message_persisted_at.is_some() {
        contracts::ChatTurnArtifactCommitStatusView::Completed
    } else if matches!(status, contracts::ChatTurnStatusView::Failed)
        && artifact_commit_ready_at.is_some()
    {
        contracts::ChatTurnArtifactCommitStatusView::Failed
    } else if artifact_commit_ready_at.is_some() || provider_responded_at.is_some() {
        contracts::ChatTurnArtifactCommitStatusView::Pending
    } else {
        contracts::ChatTurnArtifactCommitStatusView::NotReady
    }
}

pub(crate) fn infer_chat_turn_tool_loop_settled_at(
    turn: &contracts::ChatTurnRuntimeView,
    tool_executions: &[ToolExecutionView],
) -> Option<DateTime<Utc>> {
    match turn.tool_loop_status {
        contracts::ChatTurnToolLoopStatusView::Completed
        | contracts::ChatTurnToolLoopStatusView::Failed => tool_executions
            .iter()
            .map(|execution| execution.created_at)
            .max()
            .or(turn.tool_loop_settled_at)
            .or(turn.completed_at)
            .or(turn.provider_responded_at),
        contracts::ChatTurnToolLoopStatusView::NotRequested
        | contracts::ChatTurnToolLoopStatusView::Pending => None,
    }
}

fn hydrate_chat_turn_from_latest_llm_invocation(
    turn: &mut contracts::ChatTurnRuntimeView,
    llm_invocations: &[LlmInvocationView],
) {
    let Some(latest) = latest_llm_invocation(llm_invocations) else {
        return;
    };

    turn.provider_request_id = latest.request_id.clone();
    turn.finish_reason = latest
        .finish_reason
        .as_ref()
        .map(manifest_finish_reason_from_invocation);
    turn.provider_status = match latest.finish_reason.as_ref() {
        Some(contracts::LlmInvocationFinishReasonView::Error) => {
            contracts::ChatTurnProviderStatusView::Failed
        }
        _ => contracts::ChatTurnProviderStatusView::Responded,
    };
    if let Some(tool_trace_count) = latest.tool_trace_count {
        turn.tool_trace_count = tool_trace_count;
    }
    turn.stream_status = infer_chat_turn_stream_status(
        &turn.stream_mode,
        &turn.provider_status,
        turn.provider_requested_at,
        turn.provider_responded_at,
        turn.completed_at,
    );
    if turn.first_token_at.is_none() {
        turn.first_token_at = infer_chat_turn_first_token_at(turn);
    }
    if turn.stream_completed_at.is_none() {
        turn.stream_completed_at = infer_chat_turn_stream_completed_at(turn);
    }
    if turn.artifact_commit_ready_at.is_none() {
        turn.artifact_commit_ready_at = infer_chat_turn_artifact_commit_ready_at(turn);
    }
    turn.artifact_commit_status = infer_chat_turn_artifact_commit_status(
        &turn.status,
        turn.assistant_message_persisted_at,
        turn.provider_responded_at,
        turn.artifact_commit_ready_at,
    );
    if !matches!(
        turn.artifact_commit_status,
        contracts::ChatTurnArtifactCommitStatusView::Failed
    ) {
        turn.artifact_commit_failure_source = None;
    }
    if turn.tool_trace_count > 0 && turn.tool_calls_emitted_at.is_none() {
        turn.tool_calls_emitted_at = turn.provider_responded_at.or(turn.completed_at);
    }
    turn.tool_loop_status =
        infer_chat_turn_tool_loop_status(turn.tool_trace_count, turn.tool_status_summary.as_ref());
    if matches!(
        turn.tool_loop_status,
        contracts::ChatTurnToolLoopStatusView::Completed
            | contracts::ChatTurnToolLoopStatusView::Failed
    ) && turn.tool_loop_settled_at.is_none()
    {
        turn.tool_loop_settled_at = turn.completed_at.or(turn.provider_responded_at);
    }
    turn.events = build_chat_turn_events(turn);
}

pub(crate) fn hydrate_chat_message_manifest_view(
    value: &Value,
    llm_invocations: &[LlmInvocationView],
    tool_executions: &[ToolExecutionView],
) -> Option<contracts::ChatMessageManifestView> {
    let mut view = parse_chat_message_manifest(value)?;
    if let Some(runtime) = manifest_runtime_from_latest_llm_invocation(llm_invocations) {
        view.runtime = Some(runtime);
    }
    if !tool_executions.is_empty() {
        view.tool_trace = manifest_tool_trace_from_tool_executions(tool_executions);
        if let Some(runtime) = view.runtime.as_mut() {
            runtime.tool_trace_count = Some(tool_executions.len());
        }
    }
    if let Some(turn) = view.turn.as_mut() {
        if turn.finish_reason.is_none() {
            turn.finish_reason = view
                .runtime
                .as_ref()
                .and_then(|runtime| runtime.finish_reason.clone());
        }
        if matches!(
            turn.provider_status,
            contracts::ChatTurnProviderStatusView::Pending
        ) {
            turn.provider_status = match turn.finish_reason {
                Some(contracts::ManifestFinishReasonView::Error) => {
                    contracts::ChatTurnProviderStatusView::Failed
                }
                Some(_) => contracts::ChatTurnProviderStatusView::Responded,
                None => turn.provider_status.clone(),
            };
        }
        turn.events = build_chat_turn_events(turn);
        hydrate_chat_turn_from_latest_llm_invocation(turn, llm_invocations);
        if !tool_executions.is_empty() {
            turn.tool_trace_count = tool_executions.len();
            turn.tool_status_summary = summarize_tool_execution_statuses(tool_executions);
            turn.tool_loop_status = infer_chat_turn_tool_loop_status(
                turn.tool_trace_count,
                turn.tool_status_summary.as_ref(),
            );
            if turn.tool_calls_emitted_at.is_none() {
                turn.tool_calls_emitted_at = tool_executions
                    .iter()
                    .map(|execution| execution.created_at)
                    .min()
                    .or(turn.provider_responded_at);
            }
            turn.tool_loop_settled_at = infer_chat_turn_tool_loop_settled_at(turn, tool_executions);
            turn.events = build_chat_turn_events(turn);
        }
        if turn.stream_status == contracts::ChatTurnStreamStatusView::NotRequested
            && matches!(
                turn.stream_mode,
                contracts::ChatTurnStreamModeView::Streaming
            )
        {
            turn.stream_status = infer_chat_turn_stream_status(
                &turn.stream_mode,
                &turn.provider_status,
                turn.provider_requested_at,
                turn.provider_responded_at,
                turn.completed_at,
            );
        }
        if turn.first_token_at.is_none() {
            turn.first_token_at = infer_chat_turn_first_token_at(turn);
        }
        if turn.stream_completed_at.is_none() {
            turn.stream_completed_at = infer_chat_turn_stream_completed_at(turn);
        }
    }
    Some(view)
}

pub(crate) fn hydrate_assistant_turn_from_message_record(
    message: &ChatMessage,
    manifest_view: &mut Option<contracts::ChatMessageManifestView>,
) {
    if !matches!(message.role, ChatMessageRole::Assistant) {
        return;
    }

    hydrate_assistant_turn_from_message_metadata(message.id, message.created_at, manifest_view);
}

pub(crate) fn hydrate_assistant_turn_from_message_metadata(
    message_id: ChatMessageId,
    persisted_at: DateTime<Utc>,
    manifest_view: &mut Option<contracts::ChatMessageManifestView>,
) {
    let Some(manifest_view) = manifest_view.as_mut() else {
        return;
    };
    let Some(turn) = manifest_view.turn.as_mut() else {
        return;
    };

    if turn.assistant_message_id.is_none() {
        turn.assistant_message_id = Some(message_id);
    }
    if turn.assistant_message_persisted_at.is_none() {
        turn.assistant_message_persisted_at = Some(persisted_at);
    }
    if turn.completed_at.is_none() {
        turn.completed_at = turn.assistant_message_persisted_at;
    }
    if turn.artifact_commit_ready_at.is_none() {
        turn.artifact_commit_ready_at = infer_chat_turn_artifact_commit_ready_at(turn);
    }
    turn.artifact_commit_status = infer_chat_turn_artifact_commit_status(
        &turn.status,
        turn.assistant_message_persisted_at,
        turn.provider_responded_at,
        turn.artifact_commit_ready_at,
    );
    if !matches!(
        turn.artifact_commit_status,
        contracts::ChatTurnArtifactCommitStatusView::Failed
    ) {
        turn.artifact_commit_failure_source = None;
    }
    turn.events = build_chat_turn_events(turn);
}

#[cfg(test)]
mod tests {
    use domain_model::{
        ChatSessionId, LlmInvocationId, TenantId, ToolExecutionId, WorkflowExecutionId,
    };
    use serde_json::json;

    use super::*;

    fn fixed_time() -> DateTime<Utc> {
        "2026-06-14T00:00:00Z"
            .parse()
            .expect("fixed timestamp should parse")
    }

    fn llm_invocation_view(
        sequence_no: i32,
        finish_reason: contracts::LlmInvocationFinishReasonView,
    ) -> LlmInvocationView {
        LlmInvocationView {
            id: LlmInvocationId::new(),
            execution_id: WorkflowExecutionId::new(),
            source_kind: contracts::LlmInvocationSourceKindView::ChatMessage,
            dataset_output_id: None,
            chat_message_id: Some(ChatMessageId::new()),
            sequence_no,
            mode: contracts::LlmInvocationModeView::Provider,
            provider: Some(format!("provider_{sequence_no}")),
            model: Some(format!("model_{sequence_no}")),
            request_id: Some(format!("req_{sequence_no}")),
            finish_reason: Some(finish_reason),
            latency_ms: Some(100 + u64::try_from(sequence_no).unwrap_or_default()),
            usage: Some(contracts::LlmTokenUsageView {
                input_tokens: 10,
                output_tokens: 20,
                total_tokens: 30,
            }),
            system_prompt_key: Some("chat.answer".to_string()),
            system_prompt_version: Some("v1".to_string()),
            tool_trace_count: Some(2),
            created_at: fixed_time(),
        }
    }

    fn tool_execution_view(
        sequence_no: i32,
        status: contracts::ManifestToolCallStatusView,
        created_at: DateTime<Utc>,
    ) -> ToolExecutionView {
        ToolExecutionView {
            id: ToolExecutionId::new(),
            execution_id: WorkflowExecutionId::new(),
            source_kind: contracts::ToolExecutionSourceKindView::ChatMessage,
            dataset_output_id: None,
            chat_message_id: Some(ChatMessageId::new()),
            sequence_no,
            call_id: Some(format!("call_{sequence_no}")),
            tool_name: format!("tool.{sequence_no}"),
            tool: None,
            status,
            arguments: Some(json!({ "sequence": sequence_no })),
            result: Some(json!({ "ok": true })),
            created_at,
        }
    }

    #[test]
    fn chat_turn_runtime_infers_stream_tool_loop_and_event_timeline() {
        let turn = parse_chat_turn_runtime(&json!({
            "turn_id": "turn-1",
            "status": "completed",
            "stream_mode": "streaming",
            "provider_request_id": "req_1",
            "provider_requested_at": "2026-06-14T00:00:01Z",
            "provider_responded_at": "2026-06-14T00:00:05Z",
            "finish_reason": "tool_calls",
            "tool_trace_count": 2,
            "tool_status_summary": {
                "requested_count": 0,
                "completed_count": 2,
                "failed_count": 0
            },
            "started_at": "2026-06-14T00:00:00Z",
            "completed_at": "2026-06-14T00:00:06Z"
        }))
        .expect("valid turn runtime should parse");

        assert_eq!(
            turn.stream_status,
            contracts::ChatTurnStreamStatusView::Completed
        );
        assert_eq!(
            turn.artifact_commit_status,
            contracts::ChatTurnArtifactCommitStatusView::Pending
        );
        assert_eq!(
            turn.tool_loop_status,
            contracts::ChatTurnToolLoopStatusView::Completed
        );
        assert_eq!(
            turn.finish_reason,
            Some(contracts::ManifestFinishReasonView::ToolCalls)
        );
        assert!(turn.tool_calls_emitted_at.is_some());
        assert!(turn.first_token_at.is_some());
        assert!(turn
            .events
            .iter()
            .any(|event| event.kind == contracts::ChatTurnEventKindView::StreamCompleted));
        assert!(turn
            .events
            .iter()
            .any(|event| event.kind == contracts::ChatTurnEventKindView::ToolLoopCompleted));
    }

    #[test]
    fn chat_session_manifest_preserves_report_entry_and_latest_scope() {
        let memory_directory_id = MemoryDirectoryId::new();
        let dataset_output_id = DatasetOutputId::new();
        let report_plan_id = ReportPlanId::new();

        let manifest = parse_chat_session_manifest(&json!({
            "generator": "chat-session-workflow",
            "schema_version": "0.3.0",
            "status": "assistant_replied",
            "initial_prompt": "first",
            "last_prompt": "follow up",
            "last_turn_kind": "placeholder_orchestration",
            "context_binding": "creation_time",
            "latest_memory_directory_id": memory_directory_id,
            "latest_memory_directory_version_no": 8,
            "latest_dataset_output_id": dataset_output_id,
            "report_entry": {
                "state": "confirmed",
                "requested_at": "2026-06-14T00:00:00Z",
                "resolved_at": "2026-06-14T00:01:00Z",
                "resolved_action": "enter_report_service",
                "suggested_title": "Monthly Report",
                "suggested_objective": "Track risks",
                "confirmed_report_plan_id": report_plan_id
            },
            "last_turn": {
                "turn_id": "turn-2",
                "status": "completed",
                "stream_mode": "buffered",
                "provider_status": "responded",
                "provider_responded_at": "2026-06-14T00:00:02Z",
                "tool_trace_count": 0,
                "started_at": "2026-06-14T00:00:00Z",
                "completed_at": "2026-06-14T00:00:03Z"
            }
        }))
        .expect("valid chat session manifest should parse");

        assert_eq!(
            manifest.status,
            contracts::ChatSessionManifestStatusView::AssistantReplied
        );
        assert_eq!(
            manifest.latest_memory_directory_id,
            Some(memory_directory_id)
        );
        assert_eq!(manifest.latest_memory_directory_version_no, Some(8));
        assert_eq!(manifest.latest_dataset_output_id, Some(dataset_output_id));
        let report_entry = manifest.report_entry.expect("report entry should parse");
        assert_eq!(
            report_entry.state,
            contracts::ModelFacingReportEntryStateView::Confirmed
        );
        assert_eq!(
            report_entry.resolved_action,
            Some(contracts::ChatSessionReportEntryResolutionView::EnterReportService)
        );
        assert_eq!(report_entry.confirmed_report_plan_id, Some(report_plan_id));
        assert_eq!(
            manifest
                .last_turn
                .as_ref()
                .map(|turn| &turn.artifact_commit_status),
            Some(&contracts::ChatTurnArtifactCommitStatusView::Pending)
        );
    }

    #[test]
    fn chat_message_manifest_preserves_output_tool_trace_handoff_and_placeholder_runtime() {
        let dataset_id = DatasetId::new();
        let evidence_id = RetrievalEvidenceId::new();
        let report_plan_id = ReportPlanId::new();

        let manifest = parse_chat_message_manifest(&json!({
            "generator": "chat-session-worker",
            "schema_version": "0.3.0",
            "dataset_id": dataset_id,
            "prompt": "邓工是谁",
            "indexed_document_count": 3,
            "refreshed_chunks": 12,
            "prior_message_count": 2,
            "context_binding": "creation_time",
            "output": {
                "format": "markdown",
                "sections": [{
                    "section_key": "reply",
                    "kind": "reply",
                    "title": "回答",
                    "content": "邓工是项目负责人。",
                    "retrieval_evidence_ids": [evidence_id]
                }]
            },
            "service_handoff": {
                "source": "chat_session_report_entry",
                "service_lane": "report_service",
                "report_entry_state": "confirmed",
                "confirmed_report_plan_id": report_plan_id
            },
            "tool_trace": [{
                "tool_name": "retrieve_evidence",
                "status": "completed"
            }],
            "turn": {
                "turn_id": "turn-3",
                "status": "completed",
                "stream_mode": "buffered",
                "provider_status": "responded",
                "provider_responded_at": "2026-06-14T00:00:02Z",
                "tool_trace_count": 1,
                "started_at": "2026-06-14T00:00:00Z",
                "completed_at": "2026-06-14T00:00:03Z"
            }
        }))
        .expect("valid chat message manifest should parse");

        assert_eq!(manifest.dataset_id, dataset_id);
        assert_eq!(manifest.indexed_document_count, 3);
        assert_eq!(
            manifest
                .output
                .as_ref()
                .and_then(|output| output.sections.first())
                .map(|section| section.retrieval_evidence_ids.as_slice()),
            Some(&[evidence_id][..])
        );
        assert_eq!(manifest.tool_trace.len(), 1);
        assert_eq!(
            manifest
                .service_handoff
                .as_ref()
                .and_then(|handoff| handoff.confirmed_report_plan_id),
            Some(report_plan_id)
        );
        assert_eq!(
            manifest.runtime.as_ref().map(|runtime| &runtime.mode),
            Some(&contracts::ManifestRuntimeModeView::Placeholder)
        );
        assert_eq!(
            manifest
                .runtime
                .as_ref()
                .and_then(|runtime| runtime.tool_trace_count),
            Some(1)
        );
    }

    #[test]
    fn chat_message_manifest_hydrate_prefers_latest_llm_runtime_and_tool_execution_trace() {
        let dataset_id = DatasetId::new();
        let earlier_tool_at: DateTime<Utc> = "2026-06-14T00:00:03Z"
            .parse()
            .expect("fixed timestamp should parse");
        let later_tool_at: DateTime<Utc> = "2026-06-14T00:00:05Z"
            .parse()
            .expect("fixed timestamp should parse");

        let manifest = hydrate_chat_message_manifest_view(
            &json!({
                "generator": "chat-session-worker",
                "schema_version": "0.3.0",
                "dataset_id": dataset_id,
                "prompt": "生成报表",
                "indexed_document_count": 4,
                "refreshed_chunks": 20,
                "prior_message_count": 1,
                "context_binding": "creation_time",
                "tool_trace": [],
                "turn": {
                    "turn_id": "turn-hydrate",
                    "status": "completed",
                    "stream_mode": "streaming",
                    "provider_status": "pending",
                    "provider_requested_at": "2026-06-14T00:00:01Z",
                    "tool_trace_count": 0,
                    "started_at": "2026-06-14T00:00:00Z",
                    "completed_at": "2026-06-14T00:00:06Z"
                }
            }),
            &[
                llm_invocation_view(1, contracts::LlmInvocationFinishReasonView::Stop),
                llm_invocation_view(2, contracts::LlmInvocationFinishReasonView::ToolCalls),
            ],
            &[
                tool_execution_view(
                    2,
                    contracts::ManifestToolCallStatusView::Completed,
                    later_tool_at,
                ),
                tool_execution_view(
                    1,
                    contracts::ManifestToolCallStatusView::Failed,
                    earlier_tool_at,
                ),
            ],
        )
        .expect("valid manifest should hydrate");

        let runtime = manifest.runtime.expect("runtime should be hydrated");
        assert_eq!(runtime.provider.as_deref(), Some("provider_2"));
        assert_eq!(runtime.request_id.as_deref(), Some("req_2"));
        assert_eq!(
            runtime.finish_reason,
            Some(contracts::ManifestFinishReasonView::ToolCalls)
        );
        assert_eq!(runtime.tool_trace_count, Some(2));
        assert_eq!(manifest.tool_trace[0].call_id.as_deref(), Some("call_1"));
        assert_eq!(manifest.tool_trace[1].call_id.as_deref(), Some("call_2"));

        let turn = manifest.turn.expect("turn should hydrate");
        assert_eq!(turn.provider_request_id.as_deref(), Some("req_2"));
        assert_eq!(turn.tool_trace_count, 2);
        assert_eq!(
            turn.tool_status_summary.as_ref().map(|summary| (
                summary.requested_count,
                summary.completed_count,
                summary.failed_count
            )),
            Some((0, 1, 1))
        );
        assert_eq!(turn.tool_calls_emitted_at, turn.completed_at);
        assert_eq!(turn.tool_loop_settled_at, Some(later_tool_at));
        assert!(turn
            .events
            .iter()
            .any(|event| event.kind == contracts::ChatTurnEventKindView::ToolLoopFailed));
    }

    #[test]
    fn assistant_turn_metadata_hydrate_sets_message_commit_fields_and_events() {
        let dataset_id = DatasetId::new();
        let message_id = ChatMessageId::new();
        let persisted_at: DateTime<Utc> = "2026-06-14T00:00:08Z"
            .parse()
            .expect("fixed timestamp should parse");
        let mut manifest = parse_chat_message_manifest(&json!({
            "generator": "chat-session-worker",
            "schema_version": "0.3.0",
            "dataset_id": dataset_id,
            "prompt": "继续回答",
            "indexed_document_count": 2,
            "refreshed_chunks": 8,
            "prior_message_count": 1,
            "context_binding": "creation_time",
            "tool_trace": [],
            "turn": {
                "turn_id": "turn-message-commit",
                "status": "completed",
                "stream_mode": "buffered",
                "provider_status": "responded",
                "provider_responded_at": "2026-06-14T00:00:05Z",
                "tool_trace_count": 0,
                "started_at": "2026-06-14T00:00:00Z"
            }
        }));

        hydrate_assistant_turn_from_message_metadata(message_id, persisted_at, &mut manifest);

        let turn = manifest
            .as_ref()
            .and_then(|manifest| manifest.turn.as_ref())
            .expect("turn should remain available");
        assert_eq!(turn.assistant_message_id, Some(message_id));
        assert_eq!(turn.assistant_message_persisted_at, Some(persisted_at));
        assert_eq!(turn.completed_at, Some(persisted_at));
        assert_eq!(
            turn.artifact_commit_status,
            contracts::ChatTurnArtifactCommitStatusView::Completed
        );
        assert!(
            turn.events
                .iter()
                .any(|event| event.kind
                    == contracts::ChatTurnEventKindView::AssistantMessagePersisted)
        );
        assert!(turn
            .events
            .iter()
            .any(|event| event.kind == contracts::ChatTurnEventKindView::TurnCompleted));
    }

    #[test]
    fn assistant_turn_record_hydrate_ignores_non_assistant_messages() {
        let dataset_id = DatasetId::new();
        let now = fixed_time();
        let message = ChatMessage {
            id: ChatMessageId::new(),
            tenant_id: TenantId::new(),
            session_id: ChatSessionId::new(),
            role: ChatMessageRole::User,
            turn_index: 1,
            content: "用户消息".to_string(),
            message_manifest: json!({}),
            created_at: now,
        };
        let mut manifest = parse_chat_message_manifest(&json!({
            "generator": "chat-session-worker",
            "schema_version": "0.3.0",
            "dataset_id": dataset_id,
            "prompt": "用户消息",
            "indexed_document_count": 1,
            "refreshed_chunks": 1,
            "prior_message_count": 0,
            "context_binding": "creation_time",
            "tool_trace": [],
            "turn": {
                "turn_id": "turn-user",
                "status": "pending",
                "stream_mode": "buffered",
                "provider_status": "pending",
                "tool_trace_count": 0,
                "started_at": "2026-06-14T00:00:00Z"
            }
        }));

        hydrate_assistant_turn_from_message_record(&message, &mut manifest);

        let turn = manifest
            .as_ref()
            .and_then(|manifest| manifest.turn.as_ref())
            .expect("turn should remain available");
        assert_eq!(turn.assistant_message_id, None);
        assert_eq!(turn.assistant_message_persisted_at, None);
        assert_eq!(turn.completed_at, None);
        assert_eq!(
            turn.artifact_commit_status,
            contracts::ChatTurnArtifactCommitStatusView::NotReady
        );
    }

    #[test]
    fn chat_manifest_parsers_reject_unknown_required_values() {
        assert!(parse_chat_session_manifest(&json!({
            "status": "unknown"
        }))
        .is_none());
        assert!(parse_chat_turn_runtime(&json!({
            "turn_id": "turn-4",
            "status": "completed",
            "stream_mode": "invalid",
            "tool_trace_count": 0,
            "started_at": fixed_time()
        }))
        .is_none());
        assert!(parse_chat_message_manifest(&json!({
            "generator": "chat-session-worker",
            "schema_version": "0.3.0",
            "dataset_id": DatasetId::new(),
            "prompt": "hello",
            "indexed_document_count": 1,
            "refreshed_chunks": 1,
            "prior_message_count": 0,
            "context_binding": "unknown"
        }))
        .is_none());
    }
}
