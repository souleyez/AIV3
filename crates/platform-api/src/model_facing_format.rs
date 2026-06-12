use contracts::{
    ChatSessionReportEntryResolutionView, ChatTurnArtifactCommitStatusView, ChatTurnStatusView,
    ManifestServiceHandoffSourceView, ModelFacingCapabilityClassView,
    ModelFacingContinuationStateView, ModelFacingEvidenceStateView, ModelFacingNextActionView,
    ModelFacingReportEntryStateView, ModelFacingServiceLaneView,
};

pub(crate) fn format_model_facing_capability_class(
    value: &ModelFacingCapabilityClassView,
) -> &'static str {
    match value {
        ModelFacingCapabilityClassView::DatasetDirectoryAwareness => "dataset_directory_awareness",
        ModelFacingCapabilityClassView::EvidenceRetrieval => "evidence_retrieval",
        ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis => {
            "material_explanation_and_synthesis"
        }
        ModelFacingCapabilityClassView::ReportPlanning => "report_planning",
        ModelFacingCapabilityClassView::ReportGenerationAndEditing => {
            "report_generation_and_editing"
        }
        ModelFacingCapabilityClassView::ControlledPlatformAction => "controlled_platform_action",
    }
}

pub(crate) fn format_model_facing_evidence_state(
    value: &ModelFacingEvidenceStateView,
) -> &'static str {
    match value {
        ModelFacingEvidenceStateView::CatalogMemory => "catalog_memory",
        ModelFacingEvidenceStateView::SupplyOnly => "supply_only",
        ModelFacingEvidenceStateView::LiveDetail => "live_detail",
        ModelFacingEvidenceStateView::Mixed => "mixed",
        ModelFacingEvidenceStateView::Degraded => "degraded",
    }
}

pub(crate) fn format_model_facing_service_lane(value: &ModelFacingServiceLaneView) -> &'static str {
    match value {
        ModelFacingServiceLaneView::MaterialService => "material_service",
        ModelFacingServiceLaneView::ReportService => "report_service",
        ModelFacingServiceLaneView::ControlledPlatformAction => "controlled_platform_action",
    }
}

pub(crate) fn format_model_facing_report_entry_state(
    value: &ModelFacingReportEntryStateView,
) -> &'static str {
    match value {
        ModelFacingReportEntryStateView::NotApplicable => "not_applicable",
        ModelFacingReportEntryStateView::ConfirmationRequired => "confirmation_required",
        ModelFacingReportEntryStateView::Confirmed => "confirmed",
    }
}

pub(crate) fn format_chat_session_report_entry_resolution(
    value: &ChatSessionReportEntryResolutionView,
) -> &'static str {
    match value {
        ChatSessionReportEntryResolutionView::StayMaterialService => "stay_material_service",
        ChatSessionReportEntryResolutionView::EnterReportService => "enter_report_service",
    }
}

pub(crate) fn format_manifest_service_handoff_source(
    value: &ManifestServiceHandoffSourceView,
) -> &'static str {
    match value {
        ManifestServiceHandoffSourceView::ChatSessionReportEntry => "chat_session_report_entry",
    }
}

pub(crate) fn format_model_facing_next_action(value: &ModelFacingNextActionView) -> &'static str {
    match value {
        ModelFacingNextActionView::AnswerDirectly => "answer_directly",
        ModelFacingNextActionView::ReadDocumentDetail => "read_document_detail",
        ModelFacingNextActionView::CompareDocuments => "compare_documents",
        ModelFacingNextActionView::RequestReportEntryConfirmation => {
            "request_report_entry_confirmation"
        }
        ModelFacingNextActionView::WaitForToolLoop => "wait_for_tool_loop",
        ModelFacingNextActionView::FinalizeArtifactCommit => "finalize_artifact_commit",
        ModelFacingNextActionView::RetryExecution => "retry_execution",
        ModelFacingNextActionView::RefreshDirectory => "refresh_directory",
        ModelFacingNextActionView::ContinueReportPlanning => "continue_report_planning",
        ModelFacingNextActionView::GenerateReportOutput => "generate_report_output",
        ModelFacingNextActionView::PublishReport => "publish_report",
    }
}

pub(crate) fn default_tool_key_for_model_facing_next_action(
    value: &ModelFacingNextActionView,
) -> Option<&'static str> {
    match value {
        ModelFacingNextActionView::RequestReportEntryConfirmation => {
            Some("chat_session.report_entry")
        }
        ModelFacingNextActionView::ReadDocumentDetail => Some("document.read_detail"),
        ModelFacingNextActionView::CompareDocuments => Some("document.compare"),
        ModelFacingNextActionView::RetryExecution => Some("workflow.retry"),
        ModelFacingNextActionView::RefreshDirectory => Some("memory_directory.refresh"),
        ModelFacingNextActionView::ContinueReportPlanning => Some("report.plan"),
        ModelFacingNextActionView::GenerateReportOutput => Some("report.render"),
        ModelFacingNextActionView::PublishReport => Some("report.publish"),
        _ => None,
    }
}

pub(crate) fn default_tool_keys_for_model_facing_next_actions(
    actions: &[ModelFacingNextActionView],
) -> Vec<String> {
    let mut tool_keys = Vec::new();
    for action in actions {
        let Some(tool_key) = default_tool_key_for_model_facing_next_action(action) else {
            continue;
        };
        if !tool_keys.iter().any(|existing| existing == tool_key) {
            tool_keys.push(tool_key.to_string());
        }
    }
    tool_keys
}

pub(crate) fn format_model_facing_continuation_state(
    value: &ModelFacingContinuationStateView,
) -> &'static str {
    match value {
        ModelFacingContinuationStateView::ReadyToAnswer => "ready_to_answer",
        ModelFacingContinuationStateView::NeedsPlatformContinuation => {
            "needs_platform_continuation"
        }
        ModelFacingContinuationStateView::NeedsUserConfirmation => "needs_user_confirmation",
        ModelFacingContinuationStateView::WaitingForRuntime => "waiting_for_runtime",
        ModelFacingContinuationStateView::RetryRequired => "retry_required",
    }
}

pub(crate) fn format_chat_turn_status(value: &ChatTurnStatusView) -> &'static str {
    match value {
        ChatTurnStatusView::Pending => "pending",
        ChatTurnStatusView::Completed => "completed",
        ChatTurnStatusView::Failed => "failed",
    }
}

pub(crate) fn format_chat_turn_artifact_commit_status(
    value: &ChatTurnArtifactCommitStatusView,
) -> &'static str {
    match value {
        ChatTurnArtifactCommitStatusView::NotReady => "not_ready",
        ChatTurnArtifactCommitStatusView::Pending => "pending",
        ChatTurnArtifactCommitStatusView::Failed => "failed",
        ChatTurnArtifactCommitStatusView::Completed => "completed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_facing_formatters_keep_protocol_strings() {
        assert_eq!(
            format_model_facing_capability_class(
                &ModelFacingCapabilityClassView::DatasetDirectoryAwareness
            ),
            "dataset_directory_awareness",
        );
        assert_eq!(
            format_model_facing_capability_class(
                &ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis
            ),
            "material_explanation_and_synthesis",
        );
        assert_eq!(
            format_model_facing_evidence_state(&ModelFacingEvidenceStateView::LiveDetail),
            "live_detail",
        );
        assert_eq!(
            format_model_facing_service_lane(&ModelFacingServiceLaneView::ReportService),
            "report_service",
        );
        assert_eq!(
            format_model_facing_report_entry_state(
                &ModelFacingReportEntryStateView::ConfirmationRequired
            ),
            "confirmation_required",
        );
        assert_eq!(
            format_chat_session_report_entry_resolution(
                &ChatSessionReportEntryResolutionView::EnterReportService
            ),
            "enter_report_service",
        );
        assert_eq!(
            format_manifest_service_handoff_source(
                &ManifestServiceHandoffSourceView::ChatSessionReportEntry
            ),
            "chat_session_report_entry",
        );
        assert_eq!(
            format_model_facing_next_action(&ModelFacingNextActionView::ContinueReportPlanning),
            "continue_report_planning",
        );
        assert_eq!(
            format_model_facing_continuation_state(
                &ModelFacingContinuationStateView::NeedsPlatformContinuation
            ),
            "needs_platform_continuation",
        );
        assert_eq!(
            format_chat_turn_status(&ChatTurnStatusView::Completed),
            "completed",
        );
        assert_eq!(
            format_chat_turn_artifact_commit_status(&ChatTurnArtifactCommitStatusView::Pending),
            "pending",
        );
    }

    #[test]
    fn default_tool_keys_keep_first_seen_order_and_dedupe() {
        assert_eq!(
            default_tool_key_for_model_facing_next_action(
                &ModelFacingNextActionView::AnswerDirectly
            ),
            None,
        );
        assert_eq!(
            default_tool_key_for_model_facing_next_action(
                &ModelFacingNextActionView::GenerateReportOutput
            ),
            Some("report.render"),
        );
        assert_eq!(
            default_tool_keys_for_model_facing_next_actions(&[
                ModelFacingNextActionView::AnswerDirectly,
                ModelFacingNextActionView::GenerateReportOutput,
                ModelFacingNextActionView::GenerateReportOutput,
                ModelFacingNextActionView::PublishReport,
                ModelFacingNextActionView::ReadDocumentDetail,
            ]),
            vec![
                "report.render".to_string(),
                "report.publish".to_string(),
                "document.read_detail".to_string(),
            ],
        );
    }
}
