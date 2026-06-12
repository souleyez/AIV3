use contracts::{
    ChatMessageView, ChatTurnArtifactCommitStatusView, ChatTurnProviderStatusView,
    ChatTurnRuntimeView, ChatTurnStatusView, ChatTurnStreamStatusView, ChatTurnToolLoopStatusView,
    ManifestServiceHandoffView, ModelFacingCapabilityClassView, ModelFacingEvidenceStateView,
    ModelFacingNextActionView, ModelFacingReportEntryStateView, WorkflowModelFacingSummaryView,
};
use domain_model::{ChatMessageRole, WorkflowKind};

use crate::model_facing_document_focus::{
    format_model_facing_document_focus, infer_model_facing_document_focus, ModelFacingDocumentFocus,
};
use crate::model_facing_format::{
    format_chat_turn_artifact_commit_status, format_chat_turn_status,
};
use crate::model_facing_handoff::{
    collect_service_handoff_signals, infer_service_handoff_capability_class,
    infer_service_handoff_next_actions,
};
use crate::model_facing_policy::build_model_facing_summary;

pub(crate) fn derive_chat_message_model_facing_summary(
    message: &ChatMessageView,
) -> Option<WorkflowModelFacingSummaryView> {
    if !matches!(message.role, ChatMessageRole::Assistant) {
        return None;
    }

    let base_capability_class = infer_chat_message_model_facing_capability_class(message);
    let evidence_state = infer_chat_message_model_facing_evidence_state(message);
    let mut signals = collect_chat_message_model_facing_signals(message);

    if let Some(handoff) = chat_message_service_handoff(message) {
        signals.extend(collect_service_handoff_signals(handoff));
        match handoff.report_entry_state {
            ModelFacingReportEntryStateView::ConfirmationRequired => {
                let mut summary = build_model_facing_summary(
                    base_capability_class,
                    evidence_state,
                    vec![ModelFacingNextActionView::RequestReportEntryConfirmation],
                    signals,
                );
                summary.service_lane = handoff.service_lane.clone();
                summary.report_entry_state = handoff.report_entry_state.clone();
                return Some(summary);
            }
            ModelFacingReportEntryStateView::Confirmed => {
                let capability_class =
                    infer_service_handoff_capability_class(handoff, &base_capability_class);
                let allowed_next_actions = infer_service_handoff_next_actions(
                    handoff,
                    &base_capability_class,
                    &evidence_state,
                );
                let mut summary = build_model_facing_summary(
                    capability_class,
                    evidence_state,
                    allowed_next_actions,
                    signals,
                );
                summary.service_lane = handoff.service_lane.clone();
                summary.report_entry_state = handoff.report_entry_state.clone();
                return Some(summary);
            }
            ModelFacingReportEntryStateView::NotApplicable => {}
        }
    }

    let allowed_next_actions = infer_chat_message_model_facing_next_actions(
        message,
        &base_capability_class,
        &evidence_state,
    );
    Some(build_model_facing_summary(
        base_capability_class,
        evidence_state,
        allowed_next_actions,
        signals,
    ))
}

fn chat_message_service_handoff(message: &ChatMessageView) -> Option<&ManifestServiceHandoffView> {
    message
        .message_manifest_view
        .as_ref()
        .and_then(|manifest| manifest.service_handoff.as_ref())
}

fn infer_chat_message_model_facing_capability_class(
    message: &ChatMessageView,
) -> ModelFacingCapabilityClassView {
    let has_answer_content = chat_message_has_answer_content(message);
    let retrieval_evidence_count = count_chat_message_model_facing_retrieval_evidences(message);

    if !has_answer_content
        && retrieval_evidence_count == 0
        && message
            .message_manifest_view
            .as_ref()
            .and_then(|manifest| manifest.latest_memory_directory_id)
            .is_some()
    {
        return ModelFacingCapabilityClassView::DatasetDirectoryAwareness;
    }
    if !has_answer_content && retrieval_evidence_count > 0 {
        return ModelFacingCapabilityClassView::EvidenceRetrieval;
    }

    ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis
}

fn infer_chat_message_model_facing_evidence_state(
    message: &ChatMessageView,
) -> ModelFacingEvidenceStateView {
    if chat_message_turn(message)
        .map(|turn| {
            turn.status == ChatTurnStatusView::Failed
                || turn.artifact_commit_status == ChatTurnArtifactCommitStatusView::Failed
                || turn.stream_status == ChatTurnStreamStatusView::Failed
                || turn.tool_loop_status == ChatTurnToolLoopStatusView::Failed
                || turn.provider_status == ChatTurnProviderStatusView::Failed
        })
        .unwrap_or(false)
    {
        return ModelFacingEvidenceStateView::Degraded;
    }

    let has_memory_directory = message
        .message_manifest_view
        .as_ref()
        .and_then(|manifest| manifest.latest_memory_directory_id)
        .is_some();
    let retrieval_evidence_count = count_chat_message_model_facing_retrieval_evidences(message);
    let has_answer_content = chat_message_has_answer_content(message);
    let document_focus = chat_message_document_focus(message);

    if retrieval_evidence_count > 0 {
        if document_focus == ModelFacingDocumentFocus::SingleDocument && has_answer_content {
            return ModelFacingEvidenceStateView::LiveDetail;
        }
        if document_focus == ModelFacingDocumentFocus::MultiDocument
            || (has_memory_directory && has_answer_content)
        {
            return ModelFacingEvidenceStateView::Mixed;
        }
        return ModelFacingEvidenceStateView::SupplyOnly;
    }
    if has_memory_directory {
        return ModelFacingEvidenceStateView::CatalogMemory;
    }

    ModelFacingEvidenceStateView::CatalogMemory
}

fn infer_chat_message_model_facing_next_actions(
    message: &ChatMessageView,
    capability_class: &ModelFacingCapabilityClassView,
    evidence_state: &ModelFacingEvidenceStateView,
) -> Vec<ModelFacingNextActionView> {
    if *evidence_state == ModelFacingEvidenceStateView::Degraded {
        return vec![ModelFacingNextActionView::RetryExecution];
    }

    let document_focus = chat_message_document_focus(message);
    let mut actions = Vec::new();
    if let Some(turn) = chat_message_turn(message) {
        if turn.tool_loop_status == ChatTurnToolLoopStatusView::Pending {
            actions.push(ModelFacingNextActionView::WaitForToolLoop);
        }
        if turn.artifact_commit_status == ChatTurnArtifactCommitStatusView::Pending {
            actions.push(ModelFacingNextActionView::FinalizeArtifactCommit);
        }
    }

    match capability_class {
        ModelFacingCapabilityClassView::DatasetDirectoryAwareness => {
            actions.push(ModelFacingNextActionView::RefreshDirectory);
            actions.push(ModelFacingNextActionView::AnswerDirectly);
        }
        ModelFacingCapabilityClassView::EvidenceRetrieval => {
            if document_focus == ModelFacingDocumentFocus::MultiDocument {
                actions.push(ModelFacingNextActionView::CompareDocuments);
            }
            actions.push(ModelFacingNextActionView::ReadDocumentDetail);
        }
        ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis => {
            let retrieval_evidence_count =
                count_chat_message_model_facing_retrieval_evidences(message);
            if document_focus == ModelFacingDocumentFocus::MultiDocument
                || retrieval_evidence_count > 1
            {
                actions.push(ModelFacingNextActionView::CompareDocuments);
            }
            if retrieval_evidence_count > 0 {
                actions.push(ModelFacingNextActionView::ReadDocumentDetail);
            }
            actions.push(ModelFacingNextActionView::AnswerDirectly);
        }
        ModelFacingCapabilityClassView::ReportPlanning => {
            actions.push(ModelFacingNextActionView::ContinueReportPlanning);
        }
        ModelFacingCapabilityClassView::ReportGenerationAndEditing => {
            actions.push(ModelFacingNextActionView::GenerateReportOutput);
        }
        ModelFacingCapabilityClassView::ControlledPlatformAction => {
            actions.push(ModelFacingNextActionView::RetryExecution);
        }
    }

    actions
}

fn collect_chat_message_model_facing_signals(message: &ChatMessageView) -> Vec<String> {
    let document_focus = chat_message_document_focus(message);
    let indexed_document_count = chat_message_indexed_document_count(message);
    let mut signals = vec![
        format!("workflow_kind={}", WorkflowKind::ChatSession.as_str()),
        format!(
            "retrieval_evidence_count={}",
            count_chat_message_model_facing_retrieval_evidences(message)
        ),
        format!(
            "answer_content_present={}",
            chat_message_has_answer_content(message)
        ),
        format!(
            "document_focus={}",
            format_model_facing_document_focus(document_focus)
        ),
        "distinct_document_count=0".to_string(),
        format!("indexed_document_count={indexed_document_count}"),
        format!(
            "has_memory_directory={}",
            message
                .message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.latest_memory_directory_id)
                .is_some()
        ),
    ];
    if let Some(turn) = chat_message_turn(message) {
        signals.push(format!(
            "chat_turn_status={}",
            format_chat_turn_status(&turn.status)
        ));
        signals.push(format!(
            "artifact_commit_status={}",
            format_chat_turn_artifact_commit_status(&turn.artifact_commit_status)
        ));
    }
    signals
}

fn count_chat_message_model_facing_retrieval_evidences(message: &ChatMessageView) -> usize {
    message
        .message_manifest_view
        .as_ref()
        .and_then(|manifest| manifest.output.as_ref())
        .map(|output| {
            output
                .sections
                .iter()
                .map(|section| section.retrieval_evidence_ids.len())
                .sum::<usize>()
        })
        .unwrap_or(0)
}

pub(crate) fn chat_message_has_answer_content(message: &ChatMessageView) -> bool {
    !message.content.trim().is_empty()
        || message
            .message_manifest_view
            .as_ref()
            .and_then(|manifest| manifest.output.as_ref())
            .map(|content| {
                content
                    .sections
                    .iter()
                    .any(|section| !section.content.trim().is_empty())
            })
            .unwrap_or(false)
}

pub(crate) fn chat_message_indexed_document_count(message: &ChatMessageView) -> usize {
    message
        .message_manifest_view
        .as_ref()
        .map(|manifest| manifest.indexed_document_count)
        .unwrap_or(0)
}

fn chat_message_document_focus(message: &ChatMessageView) -> ModelFacingDocumentFocus {
    infer_model_facing_document_focus(0, chat_message_indexed_document_count(message))
}

fn chat_message_turn(message: &ChatMessageView) -> Option<&ChatTurnRuntimeView> {
    message
        .message_manifest_view
        .as_ref()
        .and_then(|manifest| manifest.turn.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use contracts::{
        ChatMessageManifestView, ChatMessageOutputFormatView, ChatMessageOutputView,
        ChatMessageSectionKindView, ChatMessageSectionView, ChatSessionReportEntryResolutionView,
        ChatTurnStreamModeView, ManifestContextBindingView, ManifestServiceHandoffSourceView,
        ModelFacingContinuationStateView, ModelFacingReportEntryStateView,
        ModelFacingServiceLaneView,
    };
    use domain_model::{
        ChatMessageId, ChatSessionId, DatasetId, MemoryDirectoryId, ReportPlanId,
        RetrievalEvidenceId,
    };
    use serde_json::json;

    fn chat_turn(
        status: ChatTurnStatusView,
        tool_loop_status: ChatTurnToolLoopStatusView,
        artifact_commit_status: ChatTurnArtifactCommitStatusView,
    ) -> ChatTurnRuntimeView {
        let now = Utc::now();
        ChatTurnRuntimeView {
            turn_id: "turn-fixture".to_string(),
            status,
            stream_mode: ChatTurnStreamModeView::Buffered,
            stream_status: ChatTurnStreamStatusView::NotRequested,
            artifact_commit_status,
            provider_status: ChatTurnProviderStatusView::Responded,
            provider_failure: None,
            tool_loop_status,
            provider_request_id: None,
            provider_requested_at: None,
            provider_responded_at: None,
            first_token_at: None,
            stream_completed_at: None,
            artifact_commit_ready_at: None,
            artifact_commit_failure_source: None,
            tool_calls_emitted_at: None,
            tool_loop_settled_at: None,
            finish_reason: None,
            assistant_message_id: None,
            assistant_message_persisted_at: None,
            tool_trace_count: 0,
            tool_status_summary: None,
            events: Vec::new(),
            started_at: now,
            completed_at: None,
        }
    }

    fn message_manifest(
        indexed_document_count: usize,
        sections: Vec<ChatMessageSectionView>,
    ) -> ChatMessageManifestView {
        ChatMessageManifestView {
            generator: "test".to_string(),
            schema_version: "0.3.0".to_string(),
            dataset_id: DatasetId::new(),
            prompt: "fixture prompt".to_string(),
            indexed_document_count,
            refreshed_chunks: 0,
            prior_message_count: 0,
            latest_memory_directory_id: None,
            latest_memory_directory_version_no: None,
            latest_dataset_output_id: None,
            output: Some(ChatMessageOutputView {
                format: ChatMessageOutputFormatView::Markdown,
                sections,
            }),
            service_handoff: None,
            tool_trace: Vec::new(),
            turn: None,
            context_binding: ManifestContextBindingView::CreationTime,
            runtime: None,
        }
    }

    fn assistant_message(
        content: &str,
        manifest: Option<ChatMessageManifestView>,
    ) -> ChatMessageView {
        ChatMessageView {
            id: ChatMessageId::new(),
            session_id: ChatSessionId::new(),
            role: ChatMessageRole::Assistant,
            turn_index: 1,
            content: content.to_string(),
            llm_invocations: Vec::new(),
            tool_executions: Vec::new(),
            message_manifest: json!({ "fixture": true }),
            message_manifest_view: manifest,
            model_facing: None,
            created_at: Utc::now(),
        }
    }

    fn reply_section(content: &str, retrieval_evidence_count: usize) -> ChatMessageSectionView {
        ChatMessageSectionView {
            section_key: "reply".to_string(),
            kind: ChatMessageSectionKindView::Reply,
            title: "Reply".to_string(),
            content: content.to_string(),
            retrieval_evidence_ids: (0..retrieval_evidence_count)
                .map(|_| RetrievalEvidenceId::new())
                .collect(),
        }
    }

    #[test]
    fn chat_message_summary_ignores_non_assistant_messages() {
        let mut message = assistant_message("hello", None);
        message.role = ChatMessageRole::User;

        assert!(derive_chat_message_model_facing_summary(&message).is_none());
    }

    #[test]
    fn chat_message_summary_marks_multi_document_answer_as_mixed() {
        let message = assistant_message(
            "answer",
            Some(message_manifest(2, vec![reply_section("answer", 2)])),
        );

        let summary = derive_chat_message_model_facing_summary(&message)
            .expect("assistant message should expose model-facing summary");

        assert_eq!(
            summary.capability_class,
            ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis
        );
        assert_eq!(summary.evidence_state, ModelFacingEvidenceStateView::Mixed);
        assert_eq!(
            summary.recommended_next_action,
            Some(ModelFacingNextActionView::AnswerDirectly)
        );
        assert!(summary
            .allowed_next_actions
            .contains(&ModelFacingNextActionView::CompareDocuments));
        assert!(summary
            .allowed_next_actions
            .contains(&ModelFacingNextActionView::ReadDocumentDetail));
        assert!(summary
            .signals
            .iter()
            .any(|signal| signal == "document_focus=multi_document"));
    }

    #[test]
    fn chat_message_summary_marks_failed_turn_as_degraded() {
        let mut manifest = message_manifest(0, Vec::new());
        manifest.turn = Some(chat_turn(
            ChatTurnStatusView::Failed,
            ChatTurnToolLoopStatusView::NotRequested,
            ChatTurnArtifactCommitStatusView::NotReady,
        ));
        let message = assistant_message("", Some(manifest));

        let summary = derive_chat_message_model_facing_summary(&message)
            .expect("assistant message should expose model-facing summary");

        assert_eq!(
            summary.evidence_state,
            ModelFacingEvidenceStateView::Degraded
        );
        assert_eq!(
            summary.continuation_state,
            ModelFacingContinuationStateView::RetryRequired
        );
        assert_eq!(
            summary.allowed_next_actions,
            vec![ModelFacingNextActionView::RetryExecution],
        );
        assert!(summary
            .signals
            .iter()
            .any(|signal| signal == "chat_turn_status=failed"));
    }

    #[test]
    fn chat_message_summary_keeps_runtime_pending_next_actions() {
        let mut manifest = message_manifest(0, vec![reply_section("", 0)]);
        manifest.turn = Some(chat_turn(
            ChatTurnStatusView::Pending,
            ChatTurnToolLoopStatusView::Pending,
            ChatTurnArtifactCommitStatusView::Pending,
        ));
        let message = assistant_message("", Some(manifest));

        let summary = derive_chat_message_model_facing_summary(&message)
            .expect("assistant message should expose model-facing summary");

        assert!(summary
            .allowed_next_actions
            .contains(&ModelFacingNextActionView::WaitForToolLoop));
        assert!(summary
            .allowed_next_actions
            .contains(&ModelFacingNextActionView::FinalizeArtifactCommit));
        assert!(summary
            .signals
            .iter()
            .any(|signal| signal == "artifact_commit_status=pending"));
    }

    #[test]
    fn chat_message_summary_applies_service_handoff_confirmation() {
        let mut manifest = message_manifest(0, Vec::new());
        manifest.latest_memory_directory_id = Some(MemoryDirectoryId::new());
        manifest.service_handoff = Some(ManifestServiceHandoffView {
            source: ManifestServiceHandoffSourceView::ChatSessionReportEntry,
            service_lane: ModelFacingServiceLaneView::ReportService,
            report_entry_state: ModelFacingReportEntryStateView::ConfirmationRequired,
            requested_at: Some(Utc::now()),
            resolved_at: None,
            resolved_action: Some(ChatSessionReportEntryResolutionView::EnterReportService),
            suggested_title: Some("Monthly report".to_string()),
            suggested_objective: Some("Summarize operating signals".to_string()),
            confirmed_report_plan_id: Some(ReportPlanId::new()),
        });
        let message = assistant_message("", Some(manifest));

        let summary = derive_chat_message_model_facing_summary(&message)
            .expect("assistant message should expose model-facing summary");

        assert_eq!(
            summary.report_entry_state,
            ModelFacingReportEntryStateView::ConfirmationRequired
        );
        assert_eq!(
            summary.recommended_next_action,
            Some(ModelFacingNextActionView::RequestReportEntryConfirmation)
        );
        assert!(summary
            .signals
            .iter()
            .any(|signal| signal == "service_handoff_source=chat_session_report_entry"));
    }
}
