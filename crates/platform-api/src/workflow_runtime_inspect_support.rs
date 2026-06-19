use contracts::{
    ChatMessageView, ChatSessionView, DatasetOutputView, LlmInvocationView, ReportPlanSummary,
    ReportRenderOutputView, ToolExecutionView, WorkflowExecutionRuntimeSummaryView,
    WorkflowRuntimeInspectView,
};
use domain_model::WorkflowExecution;
use serde_json::Value;

use crate::{
    derive_model_facing_summary, render_workflow_runtime_pretty_summaries,
    to_workflow_execution_view,
};

pub(crate) struct WorkflowRuntimeInspectParts {
    pub(crate) execution: WorkflowExecution,
    pub(crate) execution_scope_runtime: Option<WorkflowExecutionRuntimeSummaryView>,
    pub(crate) dataset_output: Option<DatasetOutputView>,
    pub(crate) chat_session: Option<ChatSessionView>,
    pub(crate) report_plan: Option<ReportPlanSummary>,
    pub(crate) report_render_output: Option<ReportRenderOutputView>,
    pub(crate) chat_messages: Vec<ChatMessageView>,
    pub(crate) llm_invocations: Vec<LlmInvocationView>,
    pub(crate) tool_executions: Vec<ToolExecutionView>,
    pub(crate) artifact_manifests: Vec<Value>,
}

pub(crate) fn build_workflow_runtime_inspect_view(
    parts: WorkflowRuntimeInspectParts,
) -> WorkflowRuntimeInspectView {
    let mut inspect = WorkflowRuntimeInspectView {
        execution: to_workflow_execution_view(parts.execution),
        execution_scope_runtime: parts.execution_scope_runtime,
        dataset_output: parts.dataset_output,
        chat_session: parts.chat_session,
        report_plan: parts.report_plan,
        report_render_output: parts.report_render_output,
        chat_messages: parts.chat_messages,
        llm_invocations: parts.llm_invocations,
        tool_executions: parts.tool_executions,
        model_facing: None,
        pretty_summaries: Vec::new(),
        artifact_manifests: parts.artifact_manifests,
    };
    inspect.model_facing = Some(derive_model_facing_summary(&inspect));
    inspect.pretty_summaries = render_workflow_runtime_pretty_summaries(&inspect);
    inspect
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{TenantId, WorkflowExecutionId, WorkflowKind, WorkflowStatus};
    use serde_json::json;

    #[test]
    fn runtime_inspect_view_derives_model_facing_and_pretty_summaries() {
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
            attempt: 1,
            context: json!({}),
            created_at: now,
            updated_at: now,
        };

        let inspect = build_workflow_runtime_inspect_view(WorkflowRuntimeInspectParts {
            execution,
            execution_scope_runtime: None,
            dataset_output: None,
            chat_session: None,
            report_plan: None,
            report_render_output: None,
            chat_messages: Vec::new(),
            llm_invocations: Vec::new(),
            tool_executions: Vec::new(),
            artifact_manifests: vec![json!({ "kind": "test_manifest" })],
        });

        assert_eq!(inspect.execution.kind, WorkflowKind::ReportPlan);
        assert_eq!(inspect.execution.status, WorkflowStatus::Running);
        assert!(inspect.model_facing.is_some());
        assert!(!inspect.pretty_summaries.is_empty());
        assert_eq!(
            inspect.artifact_manifests,
            vec![json!({ "kind": "test_manifest" })]
        );
    }
}
