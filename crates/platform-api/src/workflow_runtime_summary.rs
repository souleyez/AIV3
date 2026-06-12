use std::fmt::Display;

use contracts::{ManifestToolCallStatusView, ToolExecutionView};

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
