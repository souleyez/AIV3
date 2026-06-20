use domain_model::{
    StaticPageDraft, StaticPageImageJob, StaticPageRenderOutput, StaticPageRenderOutputStatus,
    WorkflowExecution, WorkflowStatus, WorkflowTaskId,
};
use serde_json::{json, Value};

use crate::static_page_render_queue_manifest_support::build_static_page_render_queue_manifest;

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

pub(crate) fn static_page_render_queued_event_payload(
    draft: &StaticPageDraft,
    render_output: &StaticPageRenderOutput,
    workflow_execution: &WorkflowExecution,
    workflow_task_id: Option<WorkflowTaskId>,
) -> Value {
    json!({
        "draft_id": draft.id,
        "render_output_id": render_output.id,
        "image_job_id": render_output.image_job_id,
        "workflow_execution_id": workflow_execution.id,
        "workflow_task_id": workflow_task_id,
    })
}

pub(crate) fn apply_static_page_render_workflow_start_to_output(
    draft: &StaticPageDraft,
    mut render_output: StaticPageRenderOutput,
    image_job: Option<&StaticPageImageJob>,
    workflow_execution: &WorkflowExecution,
    workflow_task_id: Option<WorkflowTaskId>,
) -> StaticPageRenderOutput {
    render_output.status = static_page_render_output_status_for_workflow(
        &workflow_execution.status,
        &render_output.status,
    );
    let queue_manifest = build_static_page_render_queue_manifest(
        draft,
        image_job,
        Some(workflow_execution),
        workflow_task_id,
    );
    render_output.asset_manifest =
        merge_static_page_render_output_workflow_manifest(&queue_manifest, workflow_execution);
    render_output
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{
        AssistantRunId, StaticPageDraftId, StaticPageDraftStatus, StaticPageImageJobId,
        StaticPageImageJobStatus, StaticPageRenderOutputId, TenantId, WorkflowExecutionId,
        WorkflowKind,
    };

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

    fn draft() -> StaticPageDraft {
        let now = Utc::now();
        StaticPageDraft {
            id: StaticPageDraftId::new(),
            tenant_id: TenantId::new(),
            assistant_run_id: AssistantRunId::new(),
            owner_user_id: None,
            title: "经营分析".to_string(),
            status: StaticPageDraftStatus::Confirmed,
            selected_scope: json!({"dataset_ids": ["dataset-1"]}),
            visibility_snapshot: json!({}),
            source_refs: json!([]),
            draft_payload: json!({}),
            created_at: now,
            updated_at: now,
        }
    }

    fn render_output(draft: &StaticPageDraft) -> StaticPageRenderOutput {
        StaticPageRenderOutput {
            id: StaticPageRenderOutputId::new(),
            tenant_id: draft.tenant_id,
            draft_id: draft.id,
            assistant_run_id: draft.assistant_run_id,
            owner_user_id: draft.owner_user_id,
            image_job_id: Some(StaticPageImageJobId::new()),
            status: StaticPageRenderOutputStatus::Queued,
            html: String::new(),
            asset_manifest: json!({"status": "queued"}),
            created_at: Utc::now(),
        }
    }

    fn image_job(draft: &StaticPageDraft) -> StaticPageImageJob {
        let now = Utc::now();
        StaticPageImageJob {
            id: StaticPageImageJobId::new(),
            tenant_id: draft.tenant_id,
            draft_id: draft.id,
            assistant_run_id: draft.assistant_run_id,
            status: StaticPageImageJobStatus::Confirmed,
            queue_position: None,
            image_prompt_payload: json!({}),
            preview_asset_key: Some("static-page-previews/preview.png".to_string()),
            failure_reason: None,
            confirmed_at: Some(now),
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

    #[test]
    fn queued_event_payload_preserves_workflow_and_render_context() {
        let draft = draft();
        let output = render_output(&draft);
        let execution = workflow_execution(WorkflowStatus::Running, "rendering", json!({}));
        let task_id = WorkflowTaskId::new();

        let payload =
            static_page_render_queued_event_payload(&draft, &output, &execution, Some(task_id));

        assert_eq!(payload["draft_id"], json!(draft.id));
        assert_eq!(payload["render_output_id"], json!(output.id));
        assert_eq!(payload["image_job_id"], json!(output.image_job_id));
        assert_eq!(payload["workflow_execution_id"], json!(execution.id));
        assert_eq!(payload["workflow_task_id"], json!(task_id));
    }

    #[test]
    fn queued_event_payload_allows_missing_workflow_task_id() {
        let draft = draft();
        let output = render_output(&draft);
        let execution = workflow_execution(WorkflowStatus::Pending, "queued", json!({}));

        let payload = static_page_render_queued_event_payload(&draft, &output, &execution, None);

        assert_eq!(payload["draft_id"], json!(draft.id));
        assert_eq!(payload["render_output_id"], json!(output.id));
        assert_eq!(payload["workflow_execution_id"], json!(execution.id));
        assert_eq!(payload["workflow_task_id"], Value::Null);
    }

    #[test]
    fn workflow_start_updates_render_output_status_and_manifest() {
        let draft = draft();
        let output = render_output(&draft);
        let image_job = image_job(&draft);
        let execution = workflow_execution(WorkflowStatus::Running, "rendering", json!({}));
        let task_id = WorkflowTaskId::new();

        let updated = apply_static_page_render_workflow_start_to_output(
            &draft,
            output,
            Some(&image_job),
            &execution,
            Some(task_id),
        );

        assert_eq!(updated.status, StaticPageRenderOutputStatus::Rendering);
        assert_eq!(updated.asset_manifest["status"], json!("rendering"));
        assert_eq!(updated.asset_manifest["image_job_id"], json!(image_job.id));
        assert_eq!(
            updated.asset_manifest["preview_asset_key"],
            json!("static-page-previews/preview.png")
        );
        assert_eq!(
            updated.asset_manifest["workflow_execution_id"],
            json!(execution.id)
        );
        assert_eq!(updated.asset_manifest["workflow_task_id"], json!(task_id));
        assert_eq!(
            updated.asset_manifest["workflow"]["status"],
            json!("running")
        );
        assert_eq!(
            updated.asset_manifest["workflow"]["executionId"],
            json!(execution.id)
        );
        assert_eq!(updated.asset_manifest["workflow"]["taskId"], json!(task_id));
    }
}
