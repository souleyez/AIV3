use contracts::WorkflowExecutionView;
use domain_model::WorkflowExecution;

pub(crate) fn to_workflow_execution_view(execution: WorkflowExecution) -> WorkflowExecutionView {
    WorkflowExecutionView {
        id: execution.id,
        kind: execution.kind,
        status: execution.status,
        stage: execution.stage,
        updated_at: execution.updated_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{TenantId, WorkflowExecutionId, WorkflowKind, WorkflowStatus};
    use serde_json::json;

    #[test]
    fn workflow_execution_view_preserves_public_summary_fields_only() {
        let now = Utc::now();
        let execution = WorkflowExecution {
            id: WorkflowExecutionId::new(),
            tenant_id: TenantId::new(),
            dataset_id: None,
            report_plan_id: None,
            kind: WorkflowKind::ReportPlan,
            version: "report_plan/v1".to_string(),
            stage: "plan_report_ast".to_string(),
            status: WorkflowStatus::Running,
            attempt: 2,
            context: json!({ "secret_context": "not exposed" }),
            created_at: now,
            updated_at: now,
        };
        let execution_id = execution.id;

        let view = to_workflow_execution_view(execution);

        assert_eq!(view.id, execution_id);
        assert_eq!(view.kind, WorkflowKind::ReportPlan);
        assert_eq!(view.status, WorkflowStatus::Running);
        assert_eq!(view.stage, "plan_report_ast");
        assert_eq!(view.updated_at, now);
    }
}
