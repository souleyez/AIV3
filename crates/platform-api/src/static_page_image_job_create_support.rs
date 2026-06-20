use chrono::{DateTime, Utc};
use domain_model::{
    StaticPageDraft, StaticPageDraftStatus, StaticPageImageJob, WorkflowExecution, WorkflowTaskId,
};
use serde_json::{json, Value};
use storage::NewStaticPageImageJob;

use crate::static_page_operation_apply_support::{
    append_static_page_operations_metadata, apply_static_page_operations_to_payload,
};

const DEFAULT_STATIC_PAGE_IMAGE_QUEUE_MESSAGE: &str =
    "资源正在排队，可以联系商务开通高级用户跳过等待。";
const DEFAULT_STATIC_PAGE_IMAGE_OPERATION_SUMMARY: &str = "可视化任务已进入资源队列。";

#[derive(Debug, Default)]
pub(crate) struct StaticPageImageJobCreateOptions {
    pub(crate) task_available_at: Option<DateTime<Utc>>,
    pub(crate) task_payload_patch: Option<Value>,
    pub(crate) queue_message: Option<String>,
    pub(crate) operation_summary: Option<String>,
}

pub(crate) fn static_page_image_job_queue_operations(
    job: &StaticPageImageJob,
    options: &StaticPageImageJobCreateOptions,
) -> (Vec<Value>, String) {
    let queue_message = options
        .queue_message
        .as_deref()
        .unwrap_or(DEFAULT_STATIC_PAGE_IMAGE_QUEUE_MESSAGE);
    let operation_summary = options
        .operation_summary
        .as_deref()
        .unwrap_or(DEFAULT_STATIC_PAGE_IMAGE_OPERATION_SUMMARY)
        .to_string();
    (
        vec![json!({
            "type": "queue_image_job",
            "jobId": job.id,
            "queuePosition": job.queue_position,
            "queueMessage": queue_message,
        })],
        operation_summary,
    )
}

pub(crate) fn new_queued_static_page_image_job(
    draft: &StaticPageDraft,
    image_prompt_payload: Value,
    created_at: DateTime<Utc>,
) -> NewStaticPageImageJob {
    NewStaticPageImageJob {
        draft_id: draft.id,
        assistant_run_id: draft.assistant_run_id,
        status: domain_model::StaticPageImageJobStatus::Queued,
        queue_position: Some(1),
        image_prompt_payload,
        preview_asset_key: None,
        failure_reason: None,
        confirmed_at: None,
        created_at,
    }
}

pub(crate) fn apply_static_page_image_job_queue_to_draft(
    mut draft: StaticPageDraft,
    job: &StaticPageImageJob,
    options: &StaticPageImageJobCreateOptions,
    prompt: Option<&str>,
) -> StaticPageDraft {
    let (operations, operation_summary) = static_page_image_job_queue_operations(job, options);
    draft.draft_payload = apply_static_page_operations_to_payload(
        draft.draft_payload,
        &operations,
        Some(&operation_summary),
    );
    append_static_page_operations_metadata(
        &mut draft.draft_payload,
        &operations,
        prompt,
        &operation_summary,
    );
    draft.status = StaticPageDraftStatus::Queued;
    draft
}

pub(crate) fn static_page_image_job_created_event_payload(
    draft: &StaticPageDraft,
    job: &StaticPageImageJob,
    workflow_execution: &WorkflowExecution,
    workflow_task_id: Option<WorkflowTaskId>,
    workflow_task_available_at: Option<DateTime<Utc>>,
) -> Value {
    json!({
        "draft_id": draft.id,
        "image_job_id": job.id,
        "status": job.status.as_str(),
        "queue_position": job.queue_position,
        "workflow_execution_id": workflow_execution.id,
        "workflow_task_id": workflow_task_id,
        "workflow_task_available_at": workflow_task_available_at,
    })
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use domain_model::{
        AssistantRunId, StaticPageDraftId, StaticPageDraftStatus, StaticPageImageJobId,
        StaticPageImageJobStatus, TenantId, WorkflowExecutionId, WorkflowKind, WorkflowStatus,
    };
    use serde_json::json;

    use super::*;

    fn queued_job(queue_position: Option<i32>) -> StaticPageImageJob {
        let now = Utc::now();
        StaticPageImageJob {
            id: StaticPageImageJobId::new(),
            tenant_id: TenantId::new(),
            draft_id: StaticPageDraftId::new(),
            assistant_run_id: AssistantRunId::new(),
            status: StaticPageImageJobStatus::Queued,
            queue_position,
            image_prompt_payload: json!({}),
            preview_asset_key: None,
            failure_reason: None,
            confirmed_at: None,
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
            title: "static page".to_string(),
            status: StaticPageDraftStatus::Draft,
            selected_scope: Value::Null,
            visibility_snapshot: Value::Null,
            source_refs: Value::Null,
            draft_payload: Value::Null,
            created_at: now,
            updated_at: now,
        }
    }

    fn workflow_execution() -> WorkflowExecution {
        let now = Utc::now();
        WorkflowExecution {
            id: WorkflowExecutionId::new(),
            tenant_id: TenantId::new(),
            dataset_id: None,
            report_plan_id: None,
            kind: WorkflowKind::StaticPageImageGeneration,
            version: "0.1.0".to_string(),
            stage: "queued".to_string(),
            status: WorkflowStatus::Pending,
            attempt: 1,
            context: json!({}),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn queue_operations_use_default_customer_queue_copy() {
        let job = queued_job(Some(3));

        let (operations, summary) = static_page_image_job_queue_operations(
            &job,
            &StaticPageImageJobCreateOptions::default(),
        );

        assert_eq!(summary, DEFAULT_STATIC_PAGE_IMAGE_OPERATION_SUMMARY);
        assert_eq!(operations.len(), 1);
        assert_eq!(operations[0]["type"], json!("queue_image_job"));
        assert_eq!(operations[0]["jobId"], json!(job.id));
        assert_eq!(operations[0]["queuePosition"], json!(3));
        assert_eq!(
            operations[0]["queueMessage"],
            json!(DEFAULT_STATIC_PAGE_IMAGE_QUEUE_MESSAGE)
        );
    }

    #[test]
    fn queue_operations_allow_internal_custom_copy() {
        let job = queued_job(None);
        let options = StaticPageImageJobCreateOptions {
            queue_message: Some("低负载时自动预热模板，客户不可见。".to_string()),
            operation_summary: Some("静态页模板预热任务已进入低优先级队列。".to_string()),
            ..StaticPageImageJobCreateOptions::default()
        };

        let (operations, summary) = static_page_image_job_queue_operations(&job, &options);

        assert_eq!(summary, "静态页模板预热任务已进入低优先级队列。");
        assert_eq!(operations[0]["queuePosition"], Value::Null);
        assert_eq!(
            operations[0]["queueMessage"],
            json!("低负载时自动预热模板，客户不可见。")
        );
    }

    #[test]
    fn queued_image_job_record_preserves_draft_prompt_and_defaults() {
        let draft = draft();
        let created_at = Utc::now();
        let image_prompt_payload = json!({
            "prompt": "生成门店经营分析效果图",
            "style": "business-dashboard"
        });

        let job =
            new_queued_static_page_image_job(&draft, image_prompt_payload.clone(), created_at);

        assert_eq!(job.draft_id, draft.id);
        assert_eq!(job.assistant_run_id, draft.assistant_run_id);
        assert_eq!(job.status, StaticPageImageJobStatus::Queued);
        assert_eq!(job.queue_position, Some(1));
        assert_eq!(job.image_prompt_payload, image_prompt_payload);
        assert_eq!(job.preview_asset_key, None);
        assert_eq!(job.failure_reason, None);
        assert_eq!(job.confirmed_at, None);
        assert_eq!(job.created_at, created_at);
    }

    #[test]
    fn image_job_queue_to_draft_applies_payload_metadata_and_queued_status() {
        let draft = draft();
        let job = queued_job(Some(4));

        let draft = apply_static_page_image_job_queue_to_draft(
            draft,
            &job,
            &StaticPageImageJobCreateOptions::default(),
            Some("生成经营看板效果图"),
        );

        assert_eq!(draft.status, StaticPageDraftStatus::Queued);
        assert_eq!(draft.draft_payload["status"], json!("queued"));
        assert_eq!(draft.draft_payload["imageJob"]["id"], json!(job.id));
        assert_eq!(draft.draft_payload["imageJob"]["status"], json!("queued"));
        assert_eq!(draft.draft_payload["imageJob"]["queuePosition"], json!(4));
        assert_eq!(
            draft.draft_payload["previewContract"]["imageJobId"],
            json!(job.id)
        );
        assert_eq!(
            draft.draft_payload["previewContract"]["status"],
            json!("queued")
        );
        assert_eq!(
            draft.draft_payload["lastOperationSummary"],
            json!(DEFAULT_STATIC_PAGE_IMAGE_OPERATION_SUMMARY)
        );
        assert_eq!(
            draft.draft_payload["operations"][0]["type"],
            json!("queue_image_job")
        );
        assert_eq!(
            draft.draft_payload["operations"][0]["prompt"],
            json!("生成经营看板效果图")
        );
    }

    #[test]
    fn created_event_payload_preserves_queue_and_workflow_context() {
        let draft = draft();
        let job = queued_job(Some(2));
        let execution = workflow_execution();
        let task_id = WorkflowTaskId::new();
        let available_at = Utc::now();

        let payload = static_page_image_job_created_event_payload(
            &draft,
            &job,
            &execution,
            Some(task_id),
            Some(available_at),
        );

        assert_eq!(payload["draft_id"], json!(draft.id));
        assert_eq!(payload["image_job_id"], json!(job.id));
        assert_eq!(payload["status"], json!("queued"));
        assert_eq!(payload["queue_position"], json!(2));
        assert_eq!(payload["workflow_execution_id"], json!(execution.id));
        assert_eq!(payload["workflow_task_id"], json!(task_id));
        assert_eq!(payload["workflow_task_available_at"], json!(available_at));
    }

    #[test]
    fn created_event_payload_allows_missing_queue_task_context() {
        let draft = draft();
        let job = queued_job(None);
        let execution = workflow_execution();

        let payload =
            static_page_image_job_created_event_payload(&draft, &job, &execution, None, None);

        assert_eq!(payload["draft_id"], json!(draft.id));
        assert_eq!(payload["image_job_id"], json!(job.id));
        assert_eq!(payload["status"], json!("queued"));
        assert_eq!(payload["queue_position"], Value::Null);
        assert_eq!(payload["workflow_execution_id"], json!(execution.id));
        assert_eq!(payload["workflow_task_id"], Value::Null);
        assert_eq!(payload["workflow_task_available_at"], Value::Null);
    }
}
