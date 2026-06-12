use contracts::{
    ChatTurnArtifactCommitStatusView, ChatTurnProviderStatusView, ChatTurnRuntimeView,
    ChatTurnStatusView, ChatTurnStreamStatusView, ChatTurnToolLoopStatusView,
    ModelFacingCapabilityClassView, ModelFacingEvidenceStateView, ModelFacingNextActionView,
    WorkflowModelFacingSummaryView, WorkflowRuntimeInspectView,
};
use domain_model::{WorkflowKind, WorkflowStatus};

use crate::chat_session_model_facing::derive_chat_session_model_facing_summary;
use crate::dataset_output_model_facing::derive_dataset_output_model_facing_summary;
use crate::model_facing_format::{
    format_chat_turn_artifact_commit_status, format_chat_turn_status,
};
use crate::model_facing_policy::{build_model_facing_summary, degraded_model_facing_summary};
use crate::report_plan_model_facing::derive_report_plan_model_facing_summary;
use crate::report_render_model_facing::derive_report_render_output_model_facing_summary;

pub(crate) fn derive_model_facing_summary(
    inspect: &WorkflowRuntimeInspectView,
) -> WorkflowModelFacingSummaryView {
    if let Some(session) = inspect.chat_session.as_ref() {
        let summary = session
            .model_facing
            .clone()
            .unwrap_or_else(|| derive_chat_session_model_facing_summary(session));
        if execution_failed(inspect) {
            return degraded_model_facing_summary(summary.capability_class, summary.signals);
        }
        return summary;
    }
    if let Some(output) = inspect.dataset_output.as_ref() {
        let summary = output
            .model_facing
            .clone()
            .unwrap_or_else(|| derive_dataset_output_model_facing_summary(output));
        if execution_failed(inspect) {
            return degraded_model_facing_summary(summary.capability_class, summary.signals);
        }
        return summary;
    }
    if let Some(output) = inspect.report_render_output.as_ref() {
        let summary = output
            .model_facing
            .clone()
            .unwrap_or_else(|| derive_report_render_output_model_facing_summary(output));
        if execution_failed(inspect) {
            return degraded_model_facing_summary(summary.capability_class, summary.signals);
        }
        return summary;
    }
    if let Some(plan) = inspect.report_plan.as_ref() {
        let summary = plan
            .model_facing
            .clone()
            .unwrap_or_else(|| derive_report_plan_model_facing_summary(plan));
        if execution_failed(inspect) {
            return degraded_model_facing_summary(summary.capability_class, summary.signals);
        }
        return summary;
    }

    let capability_class = infer_model_facing_capability_class(inspect);
    let evidence_state = infer_model_facing_evidence_state(inspect);
    let allowed_next_actions =
        infer_model_facing_next_actions(inspect, &capability_class, &evidence_state);
    build_model_facing_summary(
        capability_class,
        evidence_state,
        allowed_next_actions,
        collect_model_facing_signals(inspect),
    )
}

fn execution_failed(inspect: &WorkflowRuntimeInspectView) -> bool {
    matches!(
        inspect.execution.status,
        WorkflowStatus::Failed | WorkflowStatus::DeadLettered
    )
}

fn infer_model_facing_capability_class(
    inspect: &WorkflowRuntimeInspectView,
) -> ModelFacingCapabilityClassView {
    match inspect.execution.kind {
        WorkflowKind::MemoryDirectory => ModelFacingCapabilityClassView::DatasetDirectoryAwareness,
        WorkflowKind::DatasetOutput | WorkflowKind::ChatSession => {
            ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis
        }
        WorkflowKind::ReportPlan => ModelFacingCapabilityClassView::ReportPlanning,
        WorkflowKind::ReportRender
        | WorkflowKind::StaticPageImageGeneration
        | WorkflowKind::StaticPageRender => {
            ModelFacingCapabilityClassView::ReportGenerationAndEditing
        }
        WorkflowKind::UploadIngest
        | WorkflowKind::AssistantRunModelCompletion
        | WorkflowKind::CodexHostTask
        | WorkflowKind::VideoExtraction
        | WorkflowKind::ExternalSourceSync
        | WorkflowKind::ExternalActionDispatch => {
            ModelFacingCapabilityClassView::ControlledPlatformAction
        }
    }
}

fn infer_model_facing_evidence_state(
    inspect: &WorkflowRuntimeInspectView,
) -> ModelFacingEvidenceStateView {
    if execution_failed(inspect) {
        return ModelFacingEvidenceStateView::Degraded;
    }

    if latest_assistant_turn(inspect)
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

    let has_memory_directory = inspect
        .chat_session
        .as_ref()
        .and_then(|session| session.latest_memory_directory_id)
        .is_some()
        || inspect
            .dataset_output
            .as_ref()
            .and_then(|output| output.memory_directory_id)
            .is_some();
    let retrieval_evidence_count = count_model_facing_retrieval_evidences(inspect);

    if has_memory_directory && retrieval_evidence_count > 0 {
        return ModelFacingEvidenceStateView::Mixed;
    }
    if retrieval_evidence_count > 0 {
        return ModelFacingEvidenceStateView::SupplyOnly;
    }
    if has_memory_directory {
        return ModelFacingEvidenceStateView::CatalogMemory;
    }

    ModelFacingEvidenceStateView::CatalogMemory
}

fn infer_model_facing_next_actions(
    inspect: &WorkflowRuntimeInspectView,
    capability_class: &ModelFacingCapabilityClassView,
    evidence_state: &ModelFacingEvidenceStateView,
) -> Vec<ModelFacingNextActionView> {
    let mut actions = Vec::new();

    if *evidence_state == ModelFacingEvidenceStateView::Degraded {
        actions.push(ModelFacingNextActionView::RetryExecution);
        return actions;
    }

    if let Some(turn) = latest_assistant_turn(inspect) {
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
            actions.push(ModelFacingNextActionView::ReadDocumentDetail);
        }
        ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis => {
            let retrieval_evidence_count = count_model_facing_retrieval_evidences(inspect);
            if retrieval_evidence_count > 1 {
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

fn collect_model_facing_signals(inspect: &WorkflowRuntimeInspectView) -> Vec<String> {
    let mut signals = vec![format!("workflow_kind={}", inspect.execution.kind.as_str())];
    let retrieval_evidence_count = count_model_facing_retrieval_evidences(inspect);
    signals.push(format!(
        "retrieval_evidence_count={retrieval_evidence_count}"
    ));
    signals.push(format!(
        "has_memory_directory={}",
        inspect
            .chat_session
            .as_ref()
            .and_then(|session| session.latest_memory_directory_id)
            .is_some()
            || inspect
                .dataset_output
                .as_ref()
                .and_then(|output| output.memory_directory_id)
                .is_some()
    ));
    if let Some(turn) = latest_assistant_turn(inspect) {
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

fn count_model_facing_retrieval_evidences(inspect: &WorkflowRuntimeInspectView) -> usize {
    let dataset_output_count = inspect
        .dataset_output
        .as_ref()
        .map(|output| output.retrieval_evidence_ids.len())
        .unwrap_or(0);
    let assistant_message_count = inspect
        .chat_session
        .as_ref()
        .and_then(|session| session.latest_assistant_message.as_ref())
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

    dataset_output_count.max(assistant_message_count)
}

fn latest_assistant_turn(inspect: &WorkflowRuntimeInspectView) -> Option<&ChatTurnRuntimeView> {
    inspect
        .chat_session
        .as_ref()?
        .latest_assistant_message
        .as_ref()?
        .message_manifest_view
        .as_ref()?
        .turn
        .as_ref()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use contracts::WorkflowExecutionView;
    use domain_model::WorkflowExecutionId;

    fn inspect_for(kind: WorkflowKind, status: WorkflowStatus) -> WorkflowRuntimeInspectView {
        WorkflowRuntimeInspectView {
            execution: WorkflowExecutionView {
                id: WorkflowExecutionId::new(),
                kind,
                status,
                stage: "test".to_string(),
                updated_at: Utc::now(),
            },
            execution_scope_runtime: None,
            dataset_output: None,
            chat_session: None,
            report_plan: None,
            report_render_output: None,
            chat_messages: Vec::new(),
            llm_invocations: Vec::new(),
            tool_executions: Vec::new(),
            model_facing: None,
            pretty_summaries: Vec::new(),
            artifact_manifests: Vec::new(),
        }
    }

    #[test]
    fn workflow_runtime_summary_marks_failed_execution_as_degraded() {
        let inspect = inspect_for(WorkflowKind::MemoryDirectory, WorkflowStatus::Failed);

        let summary = derive_model_facing_summary(&inspect);

        assert_eq!(
            summary.capability_class,
            ModelFacingCapabilityClassView::DatasetDirectoryAwareness
        );
        assert_eq!(
            summary.evidence_state,
            ModelFacingEvidenceStateView::Degraded
        );
        assert_eq!(
            summary.recommended_next_action,
            Some(ModelFacingNextActionView::RetryExecution)
        );
        assert!(summary
            .signals
            .iter()
            .any(|signal| signal == "workflow_kind=memory_directory_workflow"));
    }

    #[test]
    fn workflow_runtime_summary_keeps_memory_directory_actions() {
        let inspect = inspect_for(WorkflowKind::MemoryDirectory, WorkflowStatus::Succeeded);

        let summary = derive_model_facing_summary(&inspect);

        assert_eq!(
            summary.capability_class,
            ModelFacingCapabilityClassView::DatasetDirectoryAwareness
        );
        assert_eq!(
            summary.evidence_state,
            ModelFacingEvidenceStateView::CatalogMemory
        );
        assert_eq!(
            summary.recommended_next_action,
            Some(ModelFacingNextActionView::AnswerDirectly)
        );
        assert!(summary
            .allowed_next_actions
            .contains(&ModelFacingNextActionView::RefreshDirectory));
        assert!(summary
            .allowed_tool_keys
            .contains(&"memory_directory.refresh".to_string()));
    }
}
