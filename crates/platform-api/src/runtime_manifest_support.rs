use contracts::{LlmInvocationView, ToolExecutionView};

pub(crate) fn manifest_finish_reason_from_invocation(
    reason: &contracts::LlmInvocationFinishReasonView,
) -> contracts::ManifestFinishReasonView {
    match reason {
        contracts::LlmInvocationFinishReasonView::Stop => contracts::ManifestFinishReasonView::Stop,
        contracts::LlmInvocationFinishReasonView::ToolCalls => {
            contracts::ManifestFinishReasonView::ToolCalls
        }
        contracts::LlmInvocationFinishReasonView::Length => {
            contracts::ManifestFinishReasonView::Length
        }
        contracts::LlmInvocationFinishReasonView::ContentFilter => {
            contracts::ManifestFinishReasonView::ContentFilter
        }
        contracts::LlmInvocationFinishReasonView::Error => {
            contracts::ManifestFinishReasonView::Error
        }
        contracts::LlmInvocationFinishReasonView::Other(value) => {
            contracts::ManifestFinishReasonView::Other(value.clone())
        }
    }
}

pub(crate) fn latest_llm_invocation(
    llm_invocations: &[LlmInvocationView],
) -> Option<&LlmInvocationView> {
    llm_invocations
        .iter()
        .max_by_key(|invocation| invocation.sequence_no)
}

pub(crate) fn manifest_runtime_from_latest_llm_invocation(
    llm_invocations: &[LlmInvocationView],
) -> Option<contracts::ManifestRuntimeView> {
    let latest = latest_llm_invocation(llm_invocations)?;

    Some(contracts::ManifestRuntimeView {
        mode: match latest.mode {
            contracts::LlmInvocationModeView::Placeholder => {
                contracts::ManifestRuntimeModeView::Placeholder
            }
            contracts::LlmInvocationModeView::Provider => {
                contracts::ManifestRuntimeModeView::Provider
            }
        },
        provider: latest.provider.clone(),
        model: latest.model.clone(),
        request_id: latest.request_id.clone(),
        finish_reason: latest
            .finish_reason
            .as_ref()
            .map(manifest_finish_reason_from_invocation),
        provider_failure: None,
        latency_ms: latest.latency_ms,
        usage: latest
            .usage
            .as_ref()
            .map(|usage| contracts::ManifestTokenUsageView {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                total_tokens: usage.total_tokens,
            }),
        system_prompt_key: latest.system_prompt_key.clone(),
        system_prompt_version: latest.system_prompt_version.clone(),
        tool_trace_count: latest.tool_trace_count,
    })
}

pub(crate) fn manifest_tool_trace_from_tool_executions(
    tool_executions: &[ToolExecutionView],
) -> Vec<contracts::ManifestToolCallView> {
    let mut trace = tool_executions.to_vec();
    trace.sort_by_key(|execution| (execution.sequence_no, execution.created_at));
    trace
        .into_iter()
        .map(|execution| contracts::ManifestToolCallView {
            call_id: execution.call_id,
            tool_name: execution.tool_name,
            tool: execution.tool,
            status: execution.status,
            arguments: execution.arguments,
            result: execution.result,
        })
        .collect()
}

pub(crate) fn summarize_tool_execution_statuses(
    tool_executions: &[ToolExecutionView],
) -> Option<contracts::ChatTurnToolStatusSummaryView> {
    if tool_executions.is_empty() {
        return None;
    }

    let mut requested_count = 0usize;
    let mut completed_count = 0usize;
    let mut failed_count = 0usize;

    for execution in tool_executions {
        match execution.status {
            contracts::ManifestToolCallStatusView::Requested => requested_count += 1,
            contracts::ManifestToolCallStatusView::Completed => completed_count += 1,
            contracts::ManifestToolCallStatusView::Failed => failed_count += 1,
        }
    }

    Some(contracts::ChatTurnToolStatusSummaryView {
        requested_count,
        completed_count,
        failed_count,
    })
}

pub(crate) fn summarize_execution_scope_runtime(
    llm_invocations: &[LlmInvocationView],
    tool_executions: &[ToolExecutionView],
) -> Option<contracts::WorkflowExecutionRuntimeSummaryView> {
    let execution_scope_invocations = llm_invocations
        .iter()
        .filter(|invocation| {
            invocation.source_kind == contracts::LlmInvocationSourceKindView::WorkflowExecution
        })
        .collect::<Vec<_>>();
    let execution_scope_tool_executions = tool_executions
        .iter()
        .filter(|execution| {
            execution.source_kind == contracts::ToolExecutionSourceKindView::WorkflowExecution
        })
        .collect::<Vec<_>>();

    if execution_scope_invocations.is_empty() && execution_scope_tool_executions.is_empty() {
        return None;
    }

    let latest_invocation = execution_scope_invocations.last().cloned().cloned();
    let failed_tool_execution_count = execution_scope_tool_executions
        .iter()
        .filter(|execution| execution.status == contracts::ManifestToolCallStatusView::Failed)
        .count();
    let latest_provider = latest_invocation
        .as_ref()
        .and_then(|invocation| invocation.provider.clone());
    let latest_model = latest_invocation
        .as_ref()
        .and_then(|invocation| invocation.model.clone());
    let latest_request_id = latest_invocation
        .as_ref()
        .and_then(|invocation| invocation.request_id.clone());
    let latest_finish_reason = latest_invocation
        .as_ref()
        .and_then(|invocation| invocation.finish_reason.clone());
    let latest_tool_trace_count = latest_invocation
        .as_ref()
        .and_then(|invocation| invocation.tool_trace_count);

    Some(contracts::WorkflowExecutionRuntimeSummaryView {
        llm_invocation_count: execution_scope_invocations.len(),
        tool_execution_count: execution_scope_tool_executions.len(),
        failed_tool_execution_count,
        latest_provider,
        latest_model,
        latest_request_id,
        latest_finish_reason,
        latest_tool_trace_count,
    })
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use domain_model::{DatasetOutputId, LlmInvocationId, ToolExecutionId, WorkflowExecutionId};
    use serde_json::json;

    use super::*;

    fn llm_invocation_view(sequence_no: i32) -> LlmInvocationView {
        LlmInvocationView {
            id: LlmInvocationId::new(),
            execution_id: WorkflowExecutionId::new(),
            source_kind: contracts::LlmInvocationSourceKindView::DatasetOutput,
            dataset_output_id: Some(DatasetOutputId::new()),
            chat_message_id: None,
            sequence_no,
            mode: contracts::LlmInvocationModeView::Provider,
            provider: Some(format!("provider_{sequence_no}")),
            model: Some(format!("model_{sequence_no}")),
            request_id: Some(format!("req_{sequence_no}")),
            finish_reason: Some(contracts::LlmInvocationFinishReasonView::Other(format!(
                "finish_{sequence_no}"
            ))),
            latency_ms: Some(100 + u64::try_from(sequence_no).unwrap_or_default()),
            usage: Some(contracts::LlmTokenUsageView {
                input_tokens: 10,
                output_tokens: 20,
                total_tokens: 30,
            }),
            system_prompt_key: Some("runtime.summary".to_string()),
            system_prompt_version: Some("v1".to_string()),
            tool_trace_count: Some(2),
            created_at: Utc::now(),
        }
    }

    fn tool_execution_view(
        sequence_no: i32,
        status: contracts::ManifestToolCallStatusView,
    ) -> ToolExecutionView {
        ToolExecutionView {
            id: ToolExecutionId::new(),
            execution_id: WorkflowExecutionId::new(),
            source_kind: contracts::ToolExecutionSourceKindView::DatasetOutput,
            dataset_output_id: Some(DatasetOutputId::new()),
            chat_message_id: None,
            sequence_no,
            call_id: Some(format!("call_{sequence_no}")),
            tool_name: format!("tool.{sequence_no}"),
            tool: None,
            status,
            arguments: Some(json!({ "sequence": sequence_no })),
            result: Some(json!({ "ok": true })),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn manifest_runtime_uses_latest_invocation_and_preserves_metadata() {
        let runtime = manifest_runtime_from_latest_llm_invocation(&[
            llm_invocation_view(1),
            llm_invocation_view(3),
        ])
        .expect("runtime should be hydrated from latest invocation");

        assert_eq!(runtime.mode, contracts::ManifestRuntimeModeView::Provider);
        assert_eq!(runtime.provider.as_deref(), Some("provider_3"));
        assert_eq!(runtime.model.as_deref(), Some("model_3"));
        assert_eq!(runtime.request_id.as_deref(), Some("req_3"));
        assert_eq!(
            runtime.finish_reason,
            Some(contracts::ManifestFinishReasonView::Other(
                "finish_3".to_string()
            ))
        );
        assert_eq!(runtime.latency_ms, Some(103));
        assert_eq!(
            runtime.usage.as_ref().map(|usage| usage.total_tokens),
            Some(30)
        );
        assert_eq!(
            runtime.system_prompt_key.as_deref(),
            Some("runtime.summary")
        );
        assert_eq!(runtime.system_prompt_version.as_deref(), Some("v1"));
        assert_eq!(runtime.tool_trace_count, Some(2));
    }

    #[test]
    fn manifest_tool_trace_sorts_by_sequence_and_preserves_status_counts() {
        let trace = manifest_tool_trace_from_tool_executions(&[
            tool_execution_view(2, contracts::ManifestToolCallStatusView::Failed),
            tool_execution_view(1, contracts::ManifestToolCallStatusView::Requested),
        ]);
        let summary = summarize_tool_execution_statuses(&[
            tool_execution_view(2, contracts::ManifestToolCallStatusView::Failed),
            tool_execution_view(1, contracts::ManifestToolCallStatusView::Requested),
            tool_execution_view(3, contracts::ManifestToolCallStatusView::Completed),
        ])
        .expect("summary should exist for non-empty trace");

        assert_eq!(trace[0].call_id.as_deref(), Some("call_1"));
        assert_eq!(trace[1].call_id.as_deref(), Some("call_2"));
        assert_eq!(summary.requested_count, 1);
        assert_eq!(summary.completed_count, 1);
        assert_eq!(summary.failed_count, 1);
    }

    #[test]
    fn execution_scope_summary_filters_non_execution_records() {
        let execution_id = WorkflowExecutionId::new();
        let mut dataset_invocation = llm_invocation_view(1);
        dataset_invocation.execution_id = execution_id;
        dataset_invocation.provider = Some("ignore".to_string());

        let mut workflow_invocation = llm_invocation_view(2);
        workflow_invocation.execution_id = execution_id;
        workflow_invocation.source_kind = contracts::LlmInvocationSourceKindView::WorkflowExecution;
        workflow_invocation.dataset_output_id = None;
        workflow_invocation.provider = Some("openai".to_string());
        workflow_invocation.finish_reason = Some(contracts::LlmInvocationFinishReasonView::Error);

        let mut workflow_tool =
            tool_execution_view(1, contracts::ManifestToolCallStatusView::Failed);
        workflow_tool.execution_id = execution_id;
        workflow_tool.source_kind = contracts::ToolExecutionSourceKindView::WorkflowExecution;
        workflow_tool.dataset_output_id = None;

        let summary = summarize_execution_scope_runtime(
            &[dataset_invocation, workflow_invocation],
            &[workflow_tool],
        )
        .expect("workflow summary should exist");

        assert_eq!(summary.llm_invocation_count, 1);
        assert_eq!(summary.tool_execution_count, 1);
        assert_eq!(summary.failed_tool_execution_count, 1);
        assert_eq!(summary.latest_provider.as_deref(), Some("openai"));
        assert_eq!(
            summary.latest_finish_reason,
            Some(contracts::LlmInvocationFinishReasonView::Error)
        );
    }
}
