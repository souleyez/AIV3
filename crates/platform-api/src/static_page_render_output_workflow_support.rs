use domain_model::{StaticPageRenderOutputStatus, WorkflowExecution, WorkflowStatus};
use serde_json::{json, Value};

pub(crate) fn static_page_render_output_status_for_workflow(
    workflow_status: &WorkflowStatus,
    current_status: &StaticPageRenderOutputStatus,
) -> StaticPageRenderOutputStatus {
    match workflow_status {
        WorkflowStatus::Pending => StaticPageRenderOutputStatus::Queued,
        WorkflowStatus::Running => StaticPageRenderOutputStatus::Rendering,
        WorkflowStatus::Failed | WorkflowStatus::DeadLettered => {
            StaticPageRenderOutputStatus::Failed
        }
        WorkflowStatus::Cancelled => StaticPageRenderOutputStatus::Cancelled,
        WorkflowStatus::Succeeded => current_status.clone(),
    }
}

pub(crate) fn merge_static_page_render_output_workflow_manifest(
    manifest: &Value,
    execution: &WorkflowExecution,
) -> Value {
    let mut object = manifest.as_object().cloned().unwrap_or_default();
    let manifest_status = match execution.status {
        WorkflowStatus::Pending => "queued",
        WorkflowStatus::Running => "rendering",
        WorkflowStatus::Succeeded => "rendered",
        WorkflowStatus::Failed | WorkflowStatus::DeadLettered => "failed",
        WorkflowStatus::Cancelled => "cancelled",
    };
    object.insert("status".to_string(), json!(manifest_status));
    let mut workflow_object = object
        .get("workflow")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    workflow_object.insert("status".to_string(), json!(execution.status.as_str()));
    workflow_object.insert("stage".to_string(), json!(execution.stage));
    workflow_object.insert("executionId".to_string(), json!(execution.id));
    workflow_object.insert("updatedAt".to_string(), json!(execution.updated_at));
    workflow_object.insert(
        "lastError".to_string(),
        execution
            .context
            .get("last_error")
            .cloned()
            .unwrap_or(Value::Null),
    );
    workflow_object.insert(
        "retryReason".to_string(),
        execution
            .context
            .get("retry_reason")
            .cloned()
            .unwrap_or(Value::Null),
    );
    workflow_object.insert(
        "cancelReason".to_string(),
        execution
            .context
            .get("cancel_reason")
            .cloned()
            .unwrap_or(Value::Null),
    );
    object.insert("workflow".to_string(), Value::Object(workflow_object));
    Value::Object(object)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{TenantId, WorkflowExecutionId, WorkflowKind};

    fn workflow_execution(
        status: WorkflowStatus,
        stage: &str,
        context: Value,
    ) -> WorkflowExecution {
        let now = Utc::now();
        WorkflowExecution {
            id: WorkflowExecutionId::new(),
            tenant_id: TenantId::new(),
            dataset_id: None,
            report_plan_id: None,
            kind: WorkflowKind::StaticPageRender,
            version: "0.1.0".to_string(),
            stage: stage.to_string(),
            status,
            attempt: 1,
            context,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn workflow_manifest_merge_preserves_task_and_failure_detail() {
        let execution = workflow_execution(
            WorkflowStatus::Failed,
            "render_static_page:failed",
            json!({
                "last_error": "renderer failed to produce html"
            }),
        );

        let merged = merge_static_page_render_output_workflow_manifest(
            &json!({
                "status": "rendering",
                "workflow": {
                    "executionId": execution.id,
                    "taskId": "task-1"
                }
            }),
            &execution,
        );

        assert_eq!(merged["status"], json!("failed"));
        assert_eq!(merged["workflow"]["status"], json!("failed"));
        assert_eq!(merged["workflow"]["taskId"], json!("task-1"));
        assert_eq!(
            merged["workflow"]["lastError"],
            json!("renderer failed to produce html")
        );
    }

    #[test]
    fn render_output_status_tracks_retry_and_terminal_workflow_states() {
        assert_eq!(
            static_page_render_output_status_for_workflow(
                &WorkflowStatus::Pending,
                &StaticPageRenderOutputStatus::Failed,
            ),
            StaticPageRenderOutputStatus::Queued
        );
        assert_eq!(
            static_page_render_output_status_for_workflow(
                &WorkflowStatus::Running,
                &StaticPageRenderOutputStatus::Queued,
            ),
            StaticPageRenderOutputStatus::Rendering
        );
        assert_eq!(
            static_page_render_output_status_for_workflow(
                &WorkflowStatus::DeadLettered,
                &StaticPageRenderOutputStatus::Rendering,
            ),
            StaticPageRenderOutputStatus::Failed
        );
        assert_eq!(
            static_page_render_output_status_for_workflow(
                &WorkflowStatus::Succeeded,
                &StaticPageRenderOutputStatus::Rendered,
            ),
            StaticPageRenderOutputStatus::Rendered
        );
    }

    #[test]
    fn workflow_manifest_merge_preserves_retry_cancel_and_dead_letter_detail() {
        let retry_execution = workflow_execution(
            WorkflowStatus::Pending,
            "queued",
            json!({
                "retry_reason": "manual retry after renderer timeout"
            }),
        );

        let retry_manifest = merge_static_page_render_output_workflow_manifest(
            &json!({
                "status": "failed",
                "workflow": {
                    "executionId": retry_execution.id,
                    "taskId": "task-1"
                }
            }),
            &retry_execution,
        );

        assert_eq!(retry_manifest["status"], json!("queued"));
        assert_eq!(retry_manifest["workflow"]["status"], json!("pending"));
        assert_eq!(retry_manifest["workflow"]["stage"], json!("queued"));
        assert_eq!(retry_manifest["workflow"]["taskId"], json!("task-1"));
        assert_eq!(
            retry_manifest["workflow"]["retryReason"],
            json!("manual retry after renderer timeout")
        );

        let cancel_execution = workflow_execution(
            WorkflowStatus::Cancelled,
            "cancelled",
            json!({
                "cancel_reason": "operator cancelled stale render"
            }),
        );
        let cancel_manifest =
            merge_static_page_render_output_workflow_manifest(&retry_manifest, &cancel_execution);

        assert_eq!(cancel_manifest["status"], json!("cancelled"));
        assert_eq!(
            cancel_manifest["workflow"]["cancelReason"],
            json!("operator cancelled stale render")
        );

        let dead_letter_execution = workflow_execution(
            WorkflowStatus::DeadLettered,
            "dead_lettered",
            json!({
                "last_error": "renderer exhausted retries"
            }),
        );

        let dead_letter_manifest = merge_static_page_render_output_workflow_manifest(
            &retry_manifest,
            &dead_letter_execution,
        );

        assert_eq!(dead_letter_manifest["status"], json!("failed"));
        assert_eq!(
            dead_letter_manifest["workflow"]["status"],
            json!("dead_lettered")
        );
        assert_eq!(
            dead_letter_manifest["workflow"]["stage"],
            json!("dead_lettered")
        );
        assert_eq!(dead_letter_manifest["workflow"]["taskId"], json!("task-1"));
        assert_eq!(
            dead_letter_manifest["workflow"]["lastError"],
            json!("renderer exhausted retries")
        );
    }
}
