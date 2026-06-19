use contracts::{LlmInvocationView, ToolExecutionView};
use domain_model::{
    ChatMessageId, DatasetOutputId, LlmInvocation, ToolExecution, WorkflowExecutionId,
};

use crate::{
    llm_invocation_view_support::to_llm_invocation_view, tool_view_support::to_tool_execution_view,
    ApiError, AppState,
};

pub(crate) async fn list_llm_invocation_views_for_execution(
    state: &AppState,
    execution_id: WorkflowExecutionId,
) -> std::result::Result<Vec<LlmInvocationView>, ApiError> {
    let invocations = state
        .storage
        .llm_invocations()
        .list_by_execution(state.tenant_id, execution_id)
        .await
        .map_err(ApiError::from_storage)?;
    Ok(to_llm_invocation_views(invocations))
}

pub(crate) async fn list_tool_execution_views_for_execution(
    state: &AppState,
    execution_id: WorkflowExecutionId,
) -> std::result::Result<Vec<ToolExecutionView>, ApiError> {
    let executions = state
        .storage
        .tool_executions()
        .list_by_execution(state.tenant_id, execution_id)
        .await
        .map_err(ApiError::from_storage)?;
    Ok(to_tool_execution_views(executions))
}

pub(crate) async fn list_llm_invocation_views_for_dataset_output(
    state: &AppState,
    output_id: DatasetOutputId,
) -> std::result::Result<Vec<LlmInvocationView>, ApiError> {
    let invocations = state
        .storage
        .llm_invocations()
        .list_by_dataset_output(state.tenant_id, output_id)
        .await
        .map_err(ApiError::from_storage)?;
    Ok(to_llm_invocation_views(invocations))
}

pub(crate) async fn list_tool_execution_views_for_dataset_output(
    state: &AppState,
    output_id: DatasetOutputId,
) -> std::result::Result<Vec<ToolExecutionView>, ApiError> {
    let executions = state
        .storage
        .tool_executions()
        .list_by_dataset_output(state.tenant_id, output_id)
        .await
        .map_err(ApiError::from_storage)?;
    Ok(to_tool_execution_views(executions))
}

pub(crate) async fn list_llm_invocation_views_for_chat_message(
    state: &AppState,
    message_id: ChatMessageId,
) -> std::result::Result<Vec<LlmInvocationView>, ApiError> {
    let invocations = state
        .storage
        .llm_invocations()
        .list_by_chat_message(state.tenant_id, message_id)
        .await
        .map_err(ApiError::from_storage)?;
    Ok(to_llm_invocation_views(invocations))
}

pub(crate) async fn list_tool_execution_views_for_chat_message(
    state: &AppState,
    message_id: ChatMessageId,
) -> std::result::Result<Vec<ToolExecutionView>, ApiError> {
    let executions = state
        .storage
        .tool_executions()
        .list_by_chat_message(state.tenant_id, message_id)
        .await
        .map_err(ApiError::from_storage)?;
    Ok(to_tool_execution_views(executions))
}

fn to_llm_invocation_views(invocations: Vec<LlmInvocation>) -> Vec<LlmInvocationView> {
    invocations
        .into_iter()
        .map(to_llm_invocation_view)
        .collect()
}

fn to_tool_execution_views(executions: Vec<ToolExecution>) -> Vec<ToolExecutionView> {
    executions.into_iter().map(to_tool_execution_view).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Utc};
    use domain_model::{
        LlmInvocationFinishReason, LlmInvocationId, LlmInvocationMode, LlmInvocationSourceKind,
        TenantId, ToolExecutionId, ToolExecutionSourceKind, ToolExecutionStatus,
    };
    use serde_json::json;

    fn fixed_time() -> DateTime<Utc> {
        "2026-06-20T00:00:00Z"
            .parse()
            .expect("fixed timestamp should parse")
    }

    #[test]
    fn llm_invocation_views_preserve_storage_order_and_runtime_fields() {
        let execution_id = WorkflowExecutionId::new();
        let first_id = LlmInvocationId::new();
        let second_id = LlmInvocationId::new();

        let views = to_llm_invocation_views(vec![
            LlmInvocation {
                id: first_id,
                tenant_id: TenantId::new(),
                execution_id,
                source_kind: LlmInvocationSourceKind::WorkflowExecution,
                dataset_output_id: None,
                chat_message_id: None,
                sequence_no: 1,
                mode: LlmInvocationMode::Provider,
                provider: Some("openai".to_string()),
                model: Some("gpt-5.5".to_string()),
                request_id: Some("req-first".to_string()),
                finish_reason: Some(LlmInvocationFinishReason::Stop),
                latency_ms: Some(120),
                usage: None,
                system_prompt_key: Some("workflow.runtime".to_string()),
                system_prompt_version: Some("v1".to_string()),
                tool_trace_count: Some(0),
                created_at: fixed_time(),
            },
            LlmInvocation {
                id: second_id,
                tenant_id: TenantId::new(),
                execution_id,
                source_kind: LlmInvocationSourceKind::WorkflowExecution,
                dataset_output_id: None,
                chat_message_id: None,
                sequence_no: 2,
                mode: LlmInvocationMode::Provider,
                provider: Some("openai".to_string()),
                model: Some("gpt-5.5".to_string()),
                request_id: Some("req-second".to_string()),
                finish_reason: Some(LlmInvocationFinishReason::Error),
                latency_ms: Some(240),
                usage: None,
                system_prompt_key: Some("workflow.runtime".to_string()),
                system_prompt_version: Some("v1".to_string()),
                tool_trace_count: Some(1),
                created_at: fixed_time(),
            },
        ]);

        assert_eq!(views.len(), 2);
        assert_eq!(views[0].id, first_id);
        assert_eq!(views[0].sequence_no, 1);
        assert_eq!(
            views[0].source_kind,
            contracts::LlmInvocationSourceKindView::WorkflowExecution
        );
        assert_eq!(views[1].id, second_id);
        assert_eq!(
            views[1].finish_reason,
            Some(contracts::LlmInvocationFinishReasonView::Error)
        );
    }

    #[test]
    fn tool_execution_views_preserve_storage_order_and_snapshot_fields() {
        let execution_id = WorkflowExecutionId::new();
        let first_id = ToolExecutionId::new();
        let second_id = ToolExecutionId::new();

        let views = to_tool_execution_views(vec![
            ToolExecution {
                id: first_id,
                tenant_id: TenantId::new(),
                execution_id,
                source_kind: ToolExecutionSourceKind::WorkflowExecution,
                dataset_output_id: None,
                chat_message_id: None,
                sequence_no: 1,
                call_id: Some("call-first".to_string()),
                tool_name: "retrieval.search".to_string(),
                tool_snapshot: None,
                status: ToolExecutionStatus::Requested,
                arguments: Some(json!({"query": "first"})),
                result: None,
                created_at: fixed_time(),
            },
            ToolExecution {
                id: second_id,
                tenant_id: TenantId::new(),
                execution_id,
                source_kind: ToolExecutionSourceKind::WorkflowExecution,
                dataset_output_id: None,
                chat_message_id: None,
                sequence_no: 2,
                call_id: Some("call-second".to_string()),
                tool_name: "retrieval.search".to_string(),
                tool_snapshot: None,
                status: ToolExecutionStatus::Completed,
                arguments: Some(json!({"query": "second"})),
                result: Some(json!({"items": []})),
                created_at: fixed_time(),
            },
        ]);

        assert_eq!(views.len(), 2);
        assert_eq!(views[0].id, first_id);
        assert_eq!(views[0].sequence_no, 1);
        assert_eq!(
            views[0].source_kind,
            contracts::ToolExecutionSourceKindView::WorkflowExecution
        );
        assert_eq!(views[1].id, second_id);
        assert_eq!(
            views[1].status,
            contracts::ManifestToolCallStatusView::Completed
        );
    }
}
