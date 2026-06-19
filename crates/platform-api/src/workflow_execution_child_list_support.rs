use contracts::{WorkflowEventView, WorkflowTaskView};
use domain_model::{WorkflowEventRecord, WorkflowExecutionId, WorkflowTask};

use crate::{to_workflow_event_view, to_workflow_task_view, ApiError, AppState};

pub(crate) async fn list_workflow_event_views_for_execution(
    state: &AppState,
    execution_id: WorkflowExecutionId,
) -> std::result::Result<Vec<WorkflowEventView>, ApiError> {
    let events = state
        .storage
        .workflow_events()
        .list_by_execution(execution_id)
        .await
        .map_err(ApiError::from_storage)?;
    Ok(to_workflow_event_views(events))
}

pub(crate) async fn list_workflow_task_views_for_execution(
    state: &AppState,
    execution_id: WorkflowExecutionId,
) -> std::result::Result<Vec<WorkflowTaskView>, ApiError> {
    let tasks = state
        .storage
        .workflow_tasks()
        .list_by_execution(execution_id)
        .await
        .map_err(ApiError::from_storage)?;
    Ok(to_workflow_task_views(tasks))
}

fn to_workflow_event_views(events: Vec<WorkflowEventRecord>) -> Vec<WorkflowEventView> {
    events.into_iter().map(to_workflow_event_view).collect()
}

fn to_workflow_task_views(tasks: Vec<WorkflowTask>) -> Vec<WorkflowTaskView> {
    tasks.into_iter().map(to_workflow_task_view).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Utc};
    use domain_model::{TenantId, WorkflowEventId, WorkflowTaskId, WorkflowTaskStatus};
    use serde_json::json;

    fn fixed_time() -> DateTime<Utc> {
        "2026-06-20T00:00:00Z"
            .parse()
            .expect("fixed timestamp should parse")
    }

    #[test]
    fn workflow_event_views_preserve_storage_order_and_event_fields() {
        let execution_id = WorkflowExecutionId::new();
        let now = fixed_time();
        let first_id = WorkflowEventId::new();
        let second_id = WorkflowEventId::new();

        let views = to_workflow_event_views(vec![
            WorkflowEventRecord {
                id: first_id,
                execution_id,
                sequence_no: 1,
                event_name: "workflow.started".to_string(),
                payload: json!({"stage": "start"}),
                created_at: now,
            },
            WorkflowEventRecord {
                id: second_id,
                execution_id,
                sequence_no: 2,
                event_name: "workflow.completed".to_string(),
                payload: json!({"status": "succeeded"}),
                created_at: now,
            },
        ]);

        assert_eq!(views.len(), 2);
        assert_eq!(views[0].id, first_id);
        assert_eq!(views[0].sequence_no, 1);
        assert_eq!(views[0].payload, json!({"stage": "start"}));
        assert_eq!(views[1].id, second_id);
        assert_eq!(views[1].event_name, "workflow.completed");
    }

    #[test]
    fn workflow_task_views_use_existing_task_view_mapping() {
        let now = fixed_time();
        let task_id = WorkflowTaskId::new();

        let views = to_workflow_task_views(vec![WorkflowTask {
            id: task_id,
            tenant_id: TenantId::new(),
            execution_id: WorkflowExecutionId::new(),
            queue: "default".to_string(),
            task_key: "generate_static_page_image".to_string(),
            payload: json!({"static_page_image_orchestrator": {"task_id": "image_job_1"}}),
            status: WorkflowTaskStatus::Queued,
            attempt: 0,
            max_attempts: 3,
            available_at: now,
            claimed_at: None,
            finished_at: None,
            error: None,
            created_at: now,
            updated_at: now,
        }]);

        assert_eq!(views.len(), 1);
        assert_eq!(views[0].id, task_id);
        assert_eq!(
            views[0].logical_queue.as_deref(),
            Some("static_page_image_preview")
        );
        assert_eq!(views[0].remote_task_id.as_deref(), Some("image_job_1"));
    }
}
