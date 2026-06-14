use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use axum::http::StatusCode;
use contracts::ExternalChannelEventResponse;
use domain_model::AssistantRunId;
use llm_gateway::LlmStreamDelta;
use serde_json::{json, Value};

use crate::{
    external_channel_answer_retrying_text, external_channel_assistant_run_reply_status_url,
    external_channel_sse_public_payload, external_channel_static_page_sse_sequence,
    sse_support::sse_text_delta_event, ApiError,
};

pub(crate) enum ExternalChannelEventSseWorkerMessage {
    AnswerDelta(String),
    Progress(ExternalChannelSseProgressMessage),
    Finished(std::result::Result<(StatusCode, ExternalChannelEventResponse), ApiError>),
}

pub(crate) struct ExternalChannelSseProgressMessage {
    pub(crate) run_id: AssistantRunId,
    pub(crate) event_name: &'static str,
    pub(crate) dedupe_key: String,
    pub(crate) display_text: String,
    pub(crate) payload: Value,
}

#[derive(Clone)]
pub(crate) struct ExternalChannelAnswerDeltaSink {
    sender: tokio::sync::mpsc::UnboundedSender<ExternalChannelEventSseWorkerMessage>,
    run_id: Option<AssistantRunId>,
    connection_id: Option<String>,
    idempotency_key: Option<String>,
    conversation_external_id: Option<String>,
    progress_sequence: Arc<AtomicUsize>,
}

impl ExternalChannelAnswerDeltaSink {
    pub(crate) fn new(
        sender: tokio::sync::mpsc::UnboundedSender<ExternalChannelEventSseWorkerMessage>,
    ) -> Self {
        Self {
            sender,
            run_id: None,
            connection_id: None,
            idempotency_key: None,
            conversation_external_id: None,
            progress_sequence: Arc::new(AtomicUsize::new(
                external_channel_static_page_sse_sequence("answer_retrying") as usize,
            )),
        }
    }

    pub(crate) fn with_run(
        mut self,
        connection_id: String,
        run_id: AssistantRunId,
        idempotency_key: String,
        conversation_external_id: String,
    ) -> Self {
        self.connection_id = Some(connection_id);
        self.run_id = Some(run_id);
        self.idempotency_key = Some(idempotency_key);
        self.conversation_external_id = Some(conversation_external_id);
        self
    }

    pub(crate) fn emit(&self, delta: LlmStreamDelta) {
        if delta.delta.is_empty() {
            return;
        }
        let _ = self
            .sender
            .send(ExternalChannelEventSseWorkerMessage::AnswerDelta(
                sse_text_delta_event("external_channel.delta", delta.index, &delta.delta),
            ));
    }

    pub(crate) fn emit_many(&self, deltas: Vec<LlmStreamDelta>) {
        for delta in deltas {
            self.emit(delta);
        }
    }

    pub(crate) fn emit_answer_retrying(&self, reason: &'static str) {
        let Some(run_id) = self.run_id else {
            return;
        };
        let Some(connection_id) = self.connection_id.as_deref() else {
            return;
        };
        let idempotency_key = self.idempotency_key.as_deref().unwrap_or_default();
        let conversation_external_id = self.conversation_external_id.as_deref().unwrap_or_default();
        let display_text = external_channel_answer_retrying_text(reason);
        let sequence = self.progress_sequence.fetch_add(1, Ordering::AcqRel) as i64;
        let status_url = external_channel_assistant_run_reply_status_url(connection_id, run_id);
        let data = json!({
            "assistant_run_id": run_id,
            "idempotency_key": idempotency_key,
            "conversation_external_id": conversation_external_id,
            "status": "retrying",
            "phase": "answering",
            "reason": reason,
            "retryable": true,
            "text": display_text,
        });
        let payload = external_channel_sse_public_payload(
            Some(run_id),
            idempotency_key,
            conversation_external_id,
            sequence,
            "answering",
            "retrying",
            display_text,
            Some(status_url),
            Some(15),
            data,
        );
        let dedupe_key = format!("external_channel.answer_retrying:{reason}:{sequence}");
        let _ = self
            .sender
            .send(ExternalChannelEventSseWorkerMessage::Progress(
                ExternalChannelSseProgressMessage {
                    run_id,
                    event_name: "external_channel.answer_retrying",
                    dedupe_key,
                    display_text: display_text.to_string(),
                    payload,
                },
            ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answer_delta_sink_ignores_empty_delta_and_formats_non_empty_delta() {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let sink = ExternalChannelAnswerDeltaSink::new(sender);

        sink.emit(LlmStreamDelta {
            index: 1,
            delta: String::new(),
        });
        assert!(receiver.try_recv().is_err());

        sink.emit(LlmStreamDelta {
            index: 2,
            delta: "继续".to_string(),
        });

        let ExternalChannelEventSseWorkerMessage::AnswerDelta(body) =
            receiver.try_recv().expect("delta event")
        else {
            panic!("expected answer delta");
        };
        assert!(body.contains("event: external_channel.delta"));
        assert!(body.contains("\"index\":2"));
        assert!(body.contains("继续"));
    }

    #[test]
    fn answer_retrying_requires_run_context() {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let sink = ExternalChannelAnswerDeltaSink::new(sender);

        sink.emit_answer_retrying("provider_error");

        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn answer_retrying_emits_progress_payload_with_status_url_and_sequence() {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let run_id = AssistantRunId::new();
        let sink = ExternalChannelAnswerDeltaSink::new(sender).with_run(
            "generic-chat-main".to_string(),
            run_id,
            "third-party:test".to_string(),
            "conv-1".to_string(),
        );

        sink.emit_answer_retrying("provider_timeout");

        let ExternalChannelEventSseWorkerMessage::Progress(progress) =
            receiver.try_recv().expect("progress event")
        else {
            panic!("expected progress");
        };
        assert_eq!(progress.run_id, run_id);
        assert_eq!(progress.event_name, "external_channel.answer_retrying");
        assert!(progress
            .dedupe_key
            .starts_with("external_channel.answer_retrying:provider_timeout:"));
        assert_eq!(progress.payload["status"], json!("retrying"));
        assert_eq!(progress.payload["phase"], json!("answering"));
        assert_eq!(progress.payload["reason"], json!("provider_timeout"));
        assert_eq!(progress.payload["poll_after_seconds"], json!(15));
        assert_eq!(progress.payload["sequence"], json!(55));
        assert!(progress
            .payload
            .get("status_url")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("/v1/external/channels/generic-chat-main/assistant-runs/"));
    }

    #[test]
    fn emit_many_preserves_non_empty_delta_order() {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let sink = ExternalChannelAnswerDeltaSink::new(sender);

        sink.emit_many(vec![
            LlmStreamDelta {
                index: 1,
                delta: "A".to_string(),
            },
            LlmStreamDelta {
                index: 2,
                delta: String::new(),
            },
            LlmStreamDelta {
                index: 3,
                delta: "B".to_string(),
            },
        ]);

        let first = receiver.try_recv().expect("first delta");
        let second = receiver.try_recv().expect("second delta");
        assert!(receiver.try_recv().is_err());
        let ExternalChannelEventSseWorkerMessage::AnswerDelta(first) = first else {
            panic!("expected first delta");
        };
        let ExternalChannelEventSseWorkerMessage::AnswerDelta(second) = second else {
            panic!("expected second delta");
        };
        assert!(first.contains("\"index\":1"));
        assert!(second.contains("\"index\":3"));
    }
}
