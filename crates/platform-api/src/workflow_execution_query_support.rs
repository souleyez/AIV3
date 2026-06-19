use domain_model::{WorkflowExecution, WorkflowKind, WorkflowStatus};

use crate::ApiError;

pub(crate) fn parse_workflow_kind_query(
    value: &str,
) -> std::result::Result<WorkflowKind, ApiError> {
    let normalized = value.trim();
    if let Some(kind) = WorkflowKind::from_str(normalized) {
        return Ok(kind);
    }
    match normalized {
        "codex_host_task" | "codex_host" | "executor" | "codex_executor" => {
            Ok(WorkflowKind::CodexHostTask)
        }
        _ => Err(ApiError::bad_request(
            "invalid_workflow_kind",
            format!("unknown workflow kind filter: {normalized}"),
        )),
    }
}

pub(crate) fn parse_workflow_status_query(
    value: &str,
) -> std::result::Result<WorkflowStatus, ApiError> {
    let normalized = value.trim();
    WorkflowStatus::from_str(normalized).ok_or_else(|| {
        ApiError::bad_request(
            "invalid_workflow_status",
            format!("unknown workflow status filter: {normalized}"),
        )
    })
}

pub(crate) fn workflow_execution_list_limit(limit: Option<usize>) -> Option<usize> {
    limit.filter(|limit| *limit > 0).map(|limit| limit.min(200))
}

pub(crate) fn workflow_task_queue_stats_limit(limit: Option<usize>) -> usize {
    limit.filter(|limit| *limit > 0).unwrap_or(200).min(500)
}

pub(crate) fn workflow_execution_matches_filters(
    execution: &WorkflowExecution,
    kind_filter: Option<&WorkflowKind>,
    status_filter: Option<&WorkflowStatus>,
) -> bool {
    if kind_filter.is_some_and(|kind| &execution.kind != kind) {
        return false;
    }
    if status_filter.is_some_and(|status| &execution.status != status) {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{TenantId, WorkflowExecutionId};
    use serde_json::json;

    fn workflow_execution(kind: WorkflowKind, status: WorkflowStatus) -> WorkflowExecution {
        WorkflowExecution {
            id: WorkflowExecutionId::new(),
            tenant_id: TenantId::new(),
            dataset_id: None,
            report_plan_id: None,
            kind,
            version: "test".to_string(),
            stage: "test".to_string(),
            status,
            context: json!({}),
            attempt: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn workflow_execution_query_accepts_codex_executor_aliases() {
        assert_eq!(
            parse_workflow_kind_query("codex_host_task_workflow").expect("canonical kind"),
            WorkflowKind::CodexHostTask
        );
        assert_eq!(
            parse_workflow_kind_query("codex_host_task").expect("short kind"),
            WorkflowKind::CodexHostTask
        );
        assert_eq!(
            parse_workflow_kind_query("codex_executor").expect("operator alias"),
            WorkflowKind::CodexHostTask
        );
    }

    #[test]
    fn workflow_execution_query_rejects_unknown_filters() {
        assert_eq!(
            parse_workflow_kind_query("not-a-workflow")
                .expect_err("unknown kind should fail")
                .payload
                .code,
            "invalid_workflow_kind"
        );
        assert_eq!(
            parse_workflow_status_query("paused")
                .expect_err("unknown status should fail")
                .payload
                .code,
            "invalid_workflow_status"
        );
    }

    #[test]
    fn workflow_query_limits_preserve_route_defaults_and_caps() {
        assert_eq!(workflow_execution_list_limit(None), None);
        assert_eq!(workflow_execution_list_limit(Some(0)), None);
        assert_eq!(workflow_execution_list_limit(Some(25)), Some(25));
        assert_eq!(workflow_execution_list_limit(Some(250)), Some(200));

        assert_eq!(workflow_task_queue_stats_limit(None), 200);
        assert_eq!(workflow_task_queue_stats_limit(Some(0)), 200);
        assert_eq!(workflow_task_queue_stats_limit(Some(25)), 25);
        assert_eq!(workflow_task_queue_stats_limit(Some(750)), 500);
    }

    #[test]
    fn workflow_execution_matches_optional_kind_and_status_filters() {
        let execution = workflow_execution(WorkflowKind::ReportRender, WorkflowStatus::Running);

        assert!(workflow_execution_matches_filters(&execution, None, None));
        assert!(workflow_execution_matches_filters(
            &execution,
            Some(&WorkflowKind::ReportRender),
            Some(&WorkflowStatus::Running),
        ));
        assert!(!workflow_execution_matches_filters(
            &execution,
            Some(&WorkflowKind::CodexHostTask),
            Some(&WorkflowStatus::Running),
        ));
        assert!(!workflow_execution_matches_filters(
            &execution,
            Some(&WorkflowKind::ReportRender),
            Some(&WorkflowStatus::Succeeded),
        ));
    }
}
