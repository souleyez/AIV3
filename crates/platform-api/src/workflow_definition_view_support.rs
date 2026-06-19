use contracts::WorkflowDefinitionView;
use workflow_engine::WorkflowDefinitionSummary;

pub(crate) fn to_workflow_definition_view(
    definition: WorkflowDefinitionSummary,
) -> WorkflowDefinitionView {
    WorkflowDefinitionView {
        kind: definition.kind,
        version: definition.version,
        summary: definition.summary,
        accepted_signals: definition
            .accepted_signals
            .into_iter()
            .map(|signal| signal.as_str().to_string())
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain_model::WorkflowKind;
    use workflow_engine::WorkflowSignalKind;

    #[test]
    fn workflow_definition_view_preserves_descriptor_fields_and_signal_order() {
        let view = to_workflow_definition_view(WorkflowDefinitionSummary {
            kind: WorkflowKind::ReportPlan,
            version: "report_plan/v1".to_string(),
            summary: "Plan a report".to_string(),
            accepted_signals: vec![
                WorkflowSignalKind::Start,
                WorkflowSignalKind::StepCompleted,
                WorkflowSignalKind::PublishRequested,
            ],
        });

        assert_eq!(view.kind, WorkflowKind::ReportPlan);
        assert_eq!(view.version, "report_plan/v1");
        assert_eq!(view.summary, "Plan a report");
        assert_eq!(
            view.accepted_signals,
            vec!["start", "step_completed", "publish_requested"]
        );
    }
}
