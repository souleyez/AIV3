use contracts::{
    ChatSessionReportEntryView, ChatSessionView, ChatTurnArtifactCommitStatusView,
    ChatTurnProviderStatusView, ChatTurnRuntimeView, ChatTurnStatusView, ChatTurnStreamStatusView,
    ChatTurnToolLoopStatusView, ModelFacingCapabilityClassView, ModelFacingEvidenceStateView,
    ModelFacingNextActionView, ModelFacingReportEntryStateView, WorkflowModelFacingSummaryView,
};
use domain_model::WorkflowKind;

use crate::chat_message_model_facing::{
    chat_message_has_answer_content, chat_message_indexed_document_count,
};
use crate::dataset_output_model_facing::dataset_output_distinct_document_count;
use crate::model_facing_document_focus::{
    format_model_facing_document_focus, infer_model_facing_document_focus, ModelFacingDocumentFocus,
};
use crate::model_facing_format::{
    format_chat_session_report_entry_resolution, format_chat_turn_artifact_commit_status,
    format_chat_turn_status, format_model_facing_report_entry_state,
};
use crate::model_facing_policy::build_model_facing_summary;

pub(crate) fn derive_chat_session_model_facing_summary(
    session: &ChatSessionView,
) -> WorkflowModelFacingSummaryView {
    let base_capability_class = infer_chat_session_model_facing_capability_class(session);
    let evidence_state = infer_chat_session_model_facing_evidence_state(session);
    let report_entry = chat_session_report_entry(session);
    let mut signals = collect_chat_session_model_facing_signals(session);

    match report_entry.map(|entry| entry.state.clone()) {
        Some(ModelFacingReportEntryStateView::ConfirmationRequired) => {
            let mut summary = build_model_facing_summary(
                base_capability_class,
                evidence_state,
                vec![ModelFacingNextActionView::RequestReportEntryConfirmation],
                signals,
            );
            summary.report_entry_state = ModelFacingReportEntryStateView::ConfirmationRequired;
            summary
        }
        Some(ModelFacingReportEntryStateView::Confirmed) => {
            if let Some(report_plan_id) =
                report_entry.and_then(|entry| entry.confirmed_report_plan_id)
            {
                signals.push(format!("confirmed_report_plan_id={report_plan_id}"));
            }
            build_model_facing_summary(
                ModelFacingCapabilityClassView::ReportPlanning,
                evidence_state,
                vec![ModelFacingNextActionView::ContinueReportPlanning],
                signals,
            )
        }
        _ => {
            let allowed_next_actions = infer_chat_session_model_facing_next_actions(
                session,
                &base_capability_class,
                &evidence_state,
            );
            build_model_facing_summary(
                base_capability_class,
                evidence_state,
                allowed_next_actions,
                signals,
            )
        }
    }
}

fn infer_chat_session_model_facing_capability_class(
    session: &ChatSessionView,
) -> ModelFacingCapabilityClassView {
    let has_answer_content = chat_session_has_answer_content(session);
    let retrieval_evidence_count = count_chat_session_model_facing_retrieval_evidences(session);

    if !has_answer_content
        && retrieval_evidence_count == 0
        && (session.latest_memory_directory_id.is_some()
            || session
                .latest_dataset_output
                .as_ref()
                .and_then(|output| output.memory_directory_id)
                .is_some())
    {
        return ModelFacingCapabilityClassView::DatasetDirectoryAwareness;
    }
    if !has_answer_content && retrieval_evidence_count > 0 {
        return ModelFacingCapabilityClassView::EvidenceRetrieval;
    }

    ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis
}

fn infer_chat_session_model_facing_evidence_state(
    session: &ChatSessionView,
) -> ModelFacingEvidenceStateView {
    if latest_chat_session_turn(session)
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

    let has_memory_directory = session.latest_memory_directory_id.is_some()
        || session
            .latest_dataset_output
            .as_ref()
            .and_then(|output| output.memory_directory_id)
            .is_some();
    let retrieval_evidence_count = count_chat_session_model_facing_retrieval_evidences(session);
    let has_answer_content = chat_session_has_answer_content(session);
    let document_focus = chat_session_document_focus(session);

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

fn infer_chat_session_model_facing_next_actions(
    session: &ChatSessionView,
    capability_class: &ModelFacingCapabilityClassView,
    evidence_state: &ModelFacingEvidenceStateView,
) -> Vec<ModelFacingNextActionView> {
    if *evidence_state == ModelFacingEvidenceStateView::Degraded {
        return vec![ModelFacingNextActionView::RetryExecution];
    }

    let document_focus = chat_session_document_focus(session);
    let mut actions = Vec::new();
    if let Some(turn) = latest_chat_session_turn(session) {
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
                count_chat_session_model_facing_retrieval_evidences(session);
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

fn collect_chat_session_model_facing_signals(session: &ChatSessionView) -> Vec<String> {
    let document_focus = chat_session_document_focus(session);
    let distinct_document_count = chat_session_distinct_document_count(session);
    let indexed_document_count = chat_session_indexed_document_count(session);
    let mut signals = vec![
        format!("workflow_kind={}", WorkflowKind::ChatSession.as_str()),
        format!(
            "retrieval_evidence_count={}",
            count_chat_session_model_facing_retrieval_evidences(session)
        ),
        format!(
            "answer_content_present={}",
            chat_session_has_answer_content(session)
        ),
        format!(
            "document_focus={}",
            format_model_facing_document_focus(document_focus)
        ),
        format!("distinct_document_count={distinct_document_count}"),
        format!("indexed_document_count={indexed_document_count}"),
        format!(
            "has_memory_directory={}",
            session.latest_memory_directory_id.is_some()
                || session
                    .latest_dataset_output
                    .as_ref()
                    .and_then(|output| output.memory_directory_id)
                    .is_some()
        ),
    ];
    if let Some(turn) = latest_chat_session_turn(session) {
        signals.push(format!(
            "chat_turn_status={}",
            format_chat_turn_status(&turn.status)
        ));
        signals.push(format!(
            "artifact_commit_status={}",
            format_chat_turn_artifact_commit_status(&turn.artifact_commit_status)
        ));
    }
    if let Some(report_entry) = chat_session_report_entry(session) {
        signals.push(format!(
            "report_entry_state={}",
            format_model_facing_report_entry_state(&report_entry.state)
        ));
        if let Some(resolved_action) = report_entry.resolved_action.as_ref() {
            signals.push(format!(
                "report_entry_resolved_action={}",
                format_chat_session_report_entry_resolution(resolved_action)
            ));
        }
        if let Some(report_plan_id) = report_entry.confirmed_report_plan_id {
            signals.push(format!("confirmed_report_plan_id={report_plan_id}"));
        }
    }
    signals
}

fn count_chat_session_model_facing_retrieval_evidences(session: &ChatSessionView) -> usize {
    let latest_assistant_message_count = session
        .latest_assistant_message
        .as_ref()
        .and_then(|message| message.message_manifest_view.as_ref())
        .and_then(|manifest| manifest.output.as_ref())
        .map(|output| {
            output
                .sections
                .iter()
                .map(|section| section.retrieval_evidence_ids.len())
                .sum::<usize>()
        })
        .unwrap_or(0);
    let latest_dataset_output_count = session
        .latest_dataset_output
        .as_ref()
        .map(|output| output.retrieval_evidence_ids.len())
        .unwrap_or(0);

    latest_assistant_message_count.max(latest_dataset_output_count)
}

fn latest_chat_session_turn(session: &ChatSessionView) -> Option<&ChatTurnRuntimeView> {
    session
        .latest_assistant_message
        .as_ref()
        .and_then(|message| message.message_manifest_view.as_ref())
        .and_then(|manifest| manifest.turn.as_ref())
        .or_else(|| {
            session
                .session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
        })
}

fn chat_session_report_entry(session: &ChatSessionView) -> Option<&ChatSessionReportEntryView> {
    session
        .session_manifest_view
        .as_ref()
        .and_then(|manifest| manifest.report_entry.as_ref())
}

fn chat_session_has_answer_content(session: &ChatSessionView) -> bool {
    session
        .latest_assistant_message
        .as_ref()
        .map(chat_message_has_answer_content)
        .unwrap_or(false)
}

fn chat_session_distinct_document_count(session: &ChatSessionView) -> usize {
    session
        .latest_dataset_output
        .as_ref()
        .map(dataset_output_distinct_document_count)
        .unwrap_or(0)
}

fn chat_session_indexed_document_count(session: &ChatSessionView) -> usize {
    session
        .latest_assistant_message
        .as_ref()
        .map(chat_message_indexed_document_count)
        .or_else(|| {
            session
                .latest_dataset_output
                .as_ref()
                .and_then(|output| output.output_manifest_view.as_ref())
                .map(|manifest| manifest.indexed_document_count)
        })
        .unwrap_or(0)
}

fn chat_session_document_focus(session: &ChatSessionView) -> ModelFacingDocumentFocus {
    infer_model_facing_document_focus(
        chat_session_distinct_document_count(session),
        chat_session_indexed_document_count(session),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use contracts::{
        ChatSessionManifestStatusView, ChatTurnStreamModeView, ManifestContextBindingView,
        ModelFacingContinuationStateView, ModelFacingServiceLaneView,
    };
    use domain_model::{
        ChatSessionId, DatasetId, MemoryDirectoryId, ReportPlanId, WorkflowExecutionId,
    };
    use serde_json::json;

    fn chat_session_view() -> ChatSessionView {
        let now = Utc::now();
        ChatSessionView {
            id: ChatSessionId::new(),
            dataset_id: DatasetId::new(),
            execution_id: WorkflowExecutionId::new(),
            title: "Chat".to_string(),
            latest_memory_directory_id: None,
            latest_memory_directory: None,
            latest_dataset_output_id: None,
            latest_dataset_output: None,
            latest_assistant_message_id: None,
            latest_assistant_message: None,
            session_manifest: json!({}),
            session_manifest_view: Some(contracts::ChatSessionManifestView {
                generator: Some("chat-session-workflow".to_string()),
                schema_version: Some("0.3.0".to_string()),
                status: ChatSessionManifestStatusView::AssistantReplied,
                initial_prompt: Some("Question".to_string()),
                last_prompt: Some("Question".to_string()),
                last_turn_kind: None,
                context_binding: Some(ManifestContextBindingView::CreationTime),
                latest_memory_directory_id: None,
                latest_memory_directory_version_no: None,
                latest_dataset_output_id: None,
                report_entry: None,
                last_turn: None,
                runtime: None,
            }),
            model_facing: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn chat_turn(status: ChatTurnStatusView) -> ChatTurnRuntimeView {
        let now = Utc::now();
        let is_failed = status == ChatTurnStatusView::Failed;
        ChatTurnRuntimeView {
            turn_id: "turn-fixture".to_string(),
            status,
            stream_mode: ChatTurnStreamModeView::Buffered,
            stream_status: ChatTurnStreamStatusView::NotRequested,
            artifact_commit_status: ChatTurnArtifactCommitStatusView::NotReady,
            provider_status: if is_failed {
                ChatTurnProviderStatusView::Failed
            } else {
                ChatTurnProviderStatusView::Responded
            },
            provider_failure: None,
            tool_loop_status: ChatTurnToolLoopStatusView::NotRequested,
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

    #[test]
    fn chat_session_summary_uses_memory_directory_awareness() {
        let mut session = chat_session_view();
        session.latest_memory_directory_id = Some(MemoryDirectoryId::new());

        let summary = derive_chat_session_model_facing_summary(&session);

        assert_eq!(
            summary.capability_class,
            ModelFacingCapabilityClassView::DatasetDirectoryAwareness
        );
        assert_eq!(
            summary.recommended_next_action,
            Some(ModelFacingNextActionView::AnswerDirectly)
        );
        assert!(summary
            .allowed_next_actions
            .contains(&ModelFacingNextActionView::RefreshDirectory));
        assert!(summary
            .signals
            .iter()
            .any(|signal| signal == "has_memory_directory=true"));
    }

    #[test]
    fn chat_session_summary_marks_failed_turn_as_degraded() {
        let mut session = chat_session_view();
        if let Some(manifest) = session.session_manifest_view.as_mut() {
            manifest.last_turn = Some(chat_turn(ChatTurnStatusView::Failed));
        }

        let summary = derive_chat_session_model_facing_summary(&session);

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
            vec![ModelFacingNextActionView::RetryExecution]
        );
    }

    #[test]
    fn chat_session_summary_promotes_confirmed_report_entry() {
        let report_plan_id = ReportPlanId::new();
        let mut session = chat_session_view();
        if let Some(manifest) = session.session_manifest_view.as_mut() {
            manifest.report_entry = Some(ChatSessionReportEntryView {
                state: ModelFacingReportEntryStateView::Confirmed,
                requested_at: Some(Utc::now()),
                resolved_at: Some(Utc::now()),
                resolved_action: None,
                suggested_title: Some("Dataset Report".to_string()),
                suggested_objective: Some("Build a report".to_string()),
                confirmed_report_plan_id: Some(report_plan_id),
            });
        }

        let summary = derive_chat_session_model_facing_summary(&session);

        assert_eq!(
            summary.capability_class,
            ModelFacingCapabilityClassView::ReportPlanning
        );
        assert_eq!(
            summary.service_lane,
            ModelFacingServiceLaneView::ReportService
        );
        assert_eq!(
            summary.report_entry_state,
            ModelFacingReportEntryStateView::Confirmed
        );
        assert_eq!(
            summary.recommended_next_action,
            Some(ModelFacingNextActionView::ContinueReportPlanning)
        );
        assert!(summary
            .signals
            .iter()
            .any(|signal| *signal == format!("confirmed_report_plan_id={report_plan_id}")));
    }
}
