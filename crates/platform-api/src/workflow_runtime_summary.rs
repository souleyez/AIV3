use std::fmt::Display;

use contracts::{ManifestToolCallStatusView, ToolExecutionView, WorkflowRuntimeInspectView};

use crate::model_facing_format::{
    format_manifest_service_handoff_source, format_model_facing_capability_class,
    format_model_facing_continuation_state, format_model_facing_evidence_state,
    format_model_facing_next_action, format_model_facing_report_entry_state,
    format_model_facing_service_lane,
};
use crate::report_render_output_asset::{
    report_render_output_asset_kind, report_render_output_has_asset_path,
};

pub fn render_workflow_runtime_pretty_summaries(
    inspect: &WorkflowRuntimeInspectView,
) -> Vec<String> {
    [
        render_model_facing_runtime_summary(inspect),
        render_execution_scope_runtime_summary(inspect),
        render_dataset_output_runtime_summary(inspect),
        render_report_plan_runtime_summary(inspect),
        render_report_render_output_runtime_summary(inspect),
        render_latest_assistant_turn_summary(inspect),
    ]
    .into_iter()
    .flatten()
    .collect()
}

pub fn render_model_facing_runtime_summary(inspect: &WorkflowRuntimeInspectView) -> Option<String> {
    let summary = inspect.model_facing.as_ref()?;
    let mut lines = begin_summary_block("Model-facing Runtime");
    push_summary_line(
        &mut lines,
        "capability_class",
        format_model_facing_capability_class(&summary.capability_class),
    );
    push_summary_line(
        &mut lines,
        "service_lane",
        format_model_facing_service_lane(&summary.service_lane),
    );
    push_summary_line(
        &mut lines,
        "report_entry_state",
        format_model_facing_report_entry_state(&summary.report_entry_state),
    );
    push_summary_line(
        &mut lines,
        "evidence_state",
        format_model_facing_evidence_state(&summary.evidence_state),
    );
    push_summary_line(
        &mut lines,
        "continuation_state",
        format_model_facing_continuation_state(&summary.continuation_state),
    );
    if let Some(action) = summary.recommended_next_action.as_ref() {
        push_summary_line(
            &mut lines,
            "recommended_next_action",
            format_model_facing_next_action(action),
        );
    }
    if !summary.allowed_next_actions.is_empty() {
        push_summary_line(
            &mut lines,
            "allowed_next_actions",
            summary
                .allowed_next_actions
                .iter()
                .map(format_model_facing_next_action)
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    if let Some(tool_key) = summary.recommended_tool_key.as_ref() {
        push_summary_line(&mut lines, "recommended_tool_key", tool_key);
    }
    if !summary.allowed_tool_keys.is_empty() {
        push_summary_line(
            &mut lines,
            "allowed_tool_keys",
            summary.allowed_tool_keys.join(", "),
        );
    }
    if !summary.signals.is_empty() {
        push_summary_line(&mut lines, "signals", summary.signals.join(", "));
    }

    Some(lines.join("\n"))
}

pub fn render_execution_scope_runtime_summary(
    inspect: &WorkflowRuntimeInspectView,
) -> Option<String> {
    let summary = inspect.execution_scope_runtime.as_ref()?;
    let mut lines = begin_summary_block("Execution-scope Runtime");
    push_summary_line(
        &mut lines,
        "llm_invocation_count",
        summary.llm_invocation_count,
    );
    push_summary_line(
        &mut lines,
        "tool_execution_count",
        summary.tool_execution_count,
    );
    push_summary_line(
        &mut lines,
        "failed_tool_execution_count",
        summary.failed_tool_execution_count,
    );
    push_optional_summary_line(
        &mut lines,
        "latest_provider",
        summary.latest_provider.as_deref(),
    );
    push_optional_summary_line(&mut lines, "latest_model", summary.latest_model.as_deref());
    push_optional_summary_line(
        &mut lines,
        "latest_request_id",
        summary.latest_request_id.as_deref(),
    );
    push_optional_summary_line(
        &mut lines,
        "latest_finish_reason",
        summary
            .latest_finish_reason
            .as_ref()
            .map(|value| value.as_str()),
    );
    push_optional_summary_line(
        &mut lines,
        "latest_tool_trace_count",
        summary.latest_tool_trace_count,
    );

    Some(lines.join("\n"))
}

pub fn render_latest_assistant_turn_summary(
    inspect: &WorkflowRuntimeInspectView,
) -> Option<String> {
    let assistant_message = inspect
        .chat_session
        .as_ref()?
        .latest_assistant_message
        .as_ref()?;
    let manifest = assistant_message.message_manifest_view.as_ref()?;
    let turn = manifest.turn.as_ref()?;
    let mut lines = begin_summary_block("Latest Assistant Turn");
    push_summary_line(&mut lines, "assistant_message_id", assistant_message.id);
    push_summary_line(&mut lines, "turn_id", &turn.turn_id);
    push_summary_line(&mut lines, "status", format!("{:?}", turn.status));
    push_summary_line(&mut lines, "stream_mode", format!("{:?}", turn.stream_mode));
    push_summary_line(
        &mut lines,
        "stream_status",
        format!("{:?}", turn.stream_status),
    );
    push_summary_line(
        &mut lines,
        "artifact_commit_status",
        format!("{:?}", turn.artifact_commit_status),
    );
    push_summary_line(
        &mut lines,
        "provider_status",
        format!("{:?}", turn.provider_status),
    );
    push_summary_line(
        &mut lines,
        "tool_loop_status",
        format!("{:?}", turn.tool_loop_status),
    );
    push_summary_line(&mut lines, "tool_trace_count", turn.tool_trace_count);
    push_summary_line(
        &mut lines,
        "llm_invocation_count",
        assistant_message.llm_invocations.len(),
    );
    push_summary_line(
        &mut lines,
        "tool_execution_count",
        assistant_message.tool_executions.len(),
    );
    push_optional_summary_line(
        &mut lines,
        "provider_request_id",
        turn.provider_request_id.as_deref(),
    );
    push_optional_summary_line(
        &mut lines,
        "finish_reason",
        turn.finish_reason.as_ref().map(|value| value.as_str()),
    );
    if let Some(failure) = turn.provider_failure.as_ref() {
        push_summary_line(&mut lines, "provider_failure_kind", failure.kind.as_str());
        push_summary_line(
            &mut lines,
            "provider_failure_message",
            failure.message.as_str(),
        );
    }
    push_optional_summary_line(
        &mut lines,
        "provider_requested_at",
        turn.provider_requested_at,
    );
    push_optional_summary_line(
        &mut lines,
        "provider_responded_at",
        turn.provider_responded_at,
    );
    push_optional_summary_line(&mut lines, "first_token_at", turn.first_token_at);
    push_optional_summary_line(&mut lines, "stream_completed_at", turn.stream_completed_at);
    push_optional_summary_line(
        &mut lines,
        "artifact_commit_ready_at",
        turn.artifact_commit_ready_at,
    );
    push_optional_summary_line(
        &mut lines,
        "artifact_commit_failure_source",
        turn.artifact_commit_failure_source
            .as_ref()
            .map(|value| format!("{value:?}")),
    );
    push_optional_summary_line(
        &mut lines,
        "tool_calls_emitted_at",
        turn.tool_calls_emitted_at,
    );
    push_optional_summary_line(
        &mut lines,
        "tool_loop_settled_at",
        turn.tool_loop_settled_at,
    );
    push_optional_summary_line(
        &mut lines,
        "assistant_message_persisted_at",
        turn.assistant_message_persisted_at,
    );
    push_optional_summary_line(&mut lines, "completed_at", turn.completed_at);
    if let Some(summary) = turn.tool_status_summary.as_ref() {
        push_summary_line(
            &mut lines,
            "tool_status_summary",
            format_tool_status_summary(
                summary.requested_count,
                summary.completed_count,
                summary.failed_count,
            ),
        );
    }

    Some(lines.join("\n"))
}

pub fn render_dataset_output_runtime_summary(
    inspect: &WorkflowRuntimeInspectView,
) -> Option<String> {
    let output = inspect.dataset_output.as_ref()?;
    let manifest = output.output_manifest_view.as_ref()?;
    let runtime = manifest.runtime.as_ref()?;
    let mut lines = begin_summary_block("Dataset Output Runtime");
    push_summary_line(&mut lines, "dataset_output_id", output.id);
    push_summary_line(
        &mut lines,
        "llm_invocation_count",
        output.llm_invocations.len(),
    );
    push_summary_line(
        &mut lines,
        "tool_execution_count",
        output.tool_executions.len(),
    );
    push_summary_line(
        &mut lines,
        "retrieval_evidence_count",
        output.retrieval_evidence_ids.len(),
    );
    push_summary_line(
        &mut lines,
        "output_section_count",
        manifest
            .output
            .as_ref()
            .map(|output| output.sections.len())
            .unwrap_or(0),
    );
    push_summary_line(&mut lines, "runtime_mode", format!("{:?}", runtime.mode));
    push_optional_summary_line(&mut lines, "provider", runtime.provider.as_deref());
    push_optional_summary_line(&mut lines, "model", runtime.model.as_deref());
    push_optional_summary_line(&mut lines, "request_id", runtime.request_id.as_deref());
    push_optional_summary_line(
        &mut lines,
        "finish_reason",
        runtime.finish_reason.as_ref().map(|value| value.as_str()),
    );
    if let Some(failure) = runtime.provider_failure.as_ref() {
        push_summary_line(&mut lines, "provider_failure_kind", failure.kind.as_str());
        push_summary_line(
            &mut lines,
            "provider_failure_message",
            failure.message.as_str(),
        );
    }
    push_optional_summary_line(&mut lines, "latency_ms", runtime.latency_ms);
    if let Some(usage) = runtime.usage.as_ref() {
        push_summary_line(
            &mut lines,
            "token_usage",
            format!(
                "input={}, output={}, total={}",
                usage.input_tokens, usage.output_tokens, usage.total_tokens
            ),
        );
    }
    push_optional_summary_line(
        &mut lines,
        "runtime_tool_trace_count",
        runtime.tool_trace_count,
    );
    if let Some((requested_count, completed_count, failed_count)) =
        summarize_tool_execution_status_counts(&output.tool_executions)
    {
        push_summary_line(
            &mut lines,
            "tool_status_summary",
            format_tool_status_summary(requested_count, completed_count, failed_count),
        );
    }

    Some(lines.join("\n"))
}

pub fn render_report_plan_runtime_summary(inspect: &WorkflowRuntimeInspectView) -> Option<String> {
    let plan = inspect.report_plan.as_ref()?;
    let mut lines = begin_summary_block("Report Plan Runtime");
    push_summary_line(&mut lines, "report_plan_id", plan.id);
    push_summary_line(&mut lines, "title", &plan.title);
    push_summary_line(&mut lines, "status", format!("{:?}", plan.status));
    push_summary_line(&mut lines, "theme_key", &plan.theme_key);
    push_optional_summary_line(
        &mut lines,
        "current_ast_version_id",
        plan.current_ast_version_id,
    );
    if let Some(handoff) = plan.service_handoff.as_ref() {
        push_summary_line(
            &mut lines,
            "service_handoff_source",
            format_manifest_service_handoff_source(&handoff.source),
        );
        push_summary_line(
            &mut lines,
            "report_entry_state",
            format_model_facing_report_entry_state(&handoff.report_entry_state),
        );
        push_optional_summary_line(
            &mut lines,
            "confirmed_report_plan_id",
            handoff.confirmed_report_plan_id,
        );
    }

    Some(lines.join("\n"))
}

pub fn render_report_render_output_runtime_summary(
    inspect: &WorkflowRuntimeInspectView,
) -> Option<String> {
    let output = inspect.report_render_output.as_ref()?;
    let mut lines = begin_summary_block("Report Render Runtime");
    push_summary_line(&mut lines, "report_render_output_id", output.id);
    push_summary_line(&mut lines, "report_plan_id", output.plan_id);
    push_summary_line(&mut lines, "surface", output.surface.as_str());
    push_summary_line(&mut lines, "status", format!("{:?}", output.status));
    push_summary_line(
        &mut lines,
        "has_asset_path",
        report_render_output_has_asset_path(output),
    );
    push_optional_summary_line(
        &mut lines,
        "asset_kind",
        report_render_output_asset_kind(output),
    );
    if let Some(handoff) = output.service_handoff.as_ref() {
        push_summary_line(
            &mut lines,
            "service_handoff_source",
            format_manifest_service_handoff_source(&handoff.source),
        );
        push_summary_line(
            &mut lines,
            "report_entry_state",
            format_model_facing_report_entry_state(&handoff.report_entry_state),
        );
        push_optional_summary_line(
            &mut lines,
            "confirmed_report_plan_id",
            handoff.confirmed_report_plan_id,
        );
    }

    Some(lines.join("\n"))
}

pub(crate) fn begin_summary_block(title: &str) -> Vec<String> {
    vec![title.to_string()]
}

pub(crate) fn push_summary_line(lines: &mut Vec<String>, label: &str, value: impl Display) {
    lines.push(format!("  {label}: {value}"));
}

pub(crate) fn push_optional_summary_line<T: Display>(
    lines: &mut Vec<String>,
    label: &str,
    value: Option<T>,
) {
    if let Some(value) = value {
        push_summary_line(lines, label, value);
    }
}

pub(crate) fn format_tool_status_summary(
    requested_count: usize,
    completed_count: usize,
    failed_count: usize,
) -> String {
    format!("requested={requested_count}, completed={completed_count}, failed={failed_count}")
}

pub(crate) fn summarize_tool_execution_status_counts(
    tool_executions: &[ToolExecutionView],
) -> Option<(usize, usize, usize)> {
    if tool_executions.is_empty() {
        return None;
    }

    let mut requested_count = 0usize;
    let mut completed_count = 0usize;
    let mut failed_count = 0usize;
    for execution in tool_executions {
        match execution.status {
            ManifestToolCallStatusView::Requested => requested_count += 1,
            ManifestToolCallStatusView::Completed => completed_count += 1,
            ManifestToolCallStatusView::Failed => failed_count += 1,
        }
    }

    Some((requested_count, completed_count, failed_count))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{ToolExecutionId, WorkflowExecutionId};

    fn tool_execution(status: ManifestToolCallStatusView) -> ToolExecutionView {
        ToolExecutionView {
            id: ToolExecutionId::new(),
            execution_id: WorkflowExecutionId::new(),
            source_kind: contracts::ToolExecutionSourceKindView::WorkflowExecution,
            dataset_output_id: None,
            chat_message_id: None,
            sequence_no: 1,
            call_id: None,
            tool_name: "test.tool".to_string(),
            tool: None,
            status,
            arguments: None,
            result: None,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn summary_lines_keep_existing_runtime_text_shape() {
        let mut lines = begin_summary_block("Runtime");
        push_summary_line(&mut lines, "status", "ok");
        push_optional_summary_line(&mut lines, "request_id", Some("req_1"));
        push_optional_summary_line::<&str>(&mut lines, "missing", None);

        assert_eq!(
            lines,
            vec![
                "Runtime".to_string(),
                "  status: ok".to_string(),
                "  request_id: req_1".to_string(),
            ],
        );
    }

    #[test]
    fn tool_execution_status_counts_preserve_requested_completed_failed_order() {
        let executions = vec![
            tool_execution(ManifestToolCallStatusView::Failed),
            tool_execution(ManifestToolCallStatusView::Requested),
            tool_execution(ManifestToolCallStatusView::Completed),
            tool_execution(ManifestToolCallStatusView::Completed),
        ];

        assert_eq!(
            summarize_tool_execution_status_counts(&executions),
            Some((1, 2, 1)),
        );
        assert_eq!(
            format_tool_status_summary(1, 2, 1),
            "requested=1, completed=2, failed=1",
        );
        assert_eq!(summarize_tool_execution_status_counts(&[]), None);
    }
}
