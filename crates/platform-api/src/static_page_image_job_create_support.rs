use chrono::{DateTime, Utc};
use contracts::{CreateStaticPageImageJobResponse, WorkflowTaskView};
use domain_model::{
    StaticPageDraft, StaticPageDraftStatus, StaticPageImageJob, WorkflowExecution, WorkflowTask,
    WorkflowTaskId,
};
use serde_json::{json, Map, Value};
use storage::NewStaticPageImageJob;

use crate::static_page_image_prompt_payload_support::{
    build_static_page_image_prompt_payload, static_page_image_prompt_payload_is_prompt_only,
};
use crate::static_page_operation_apply_support::{
    append_static_page_operations_metadata, apply_static_page_operations_to_payload,
};
use crate::static_page_payload_support::merge_json_value;
use crate::static_page_template_prewarm_support::static_page_template_prewarm_key_from_source_refs;
use crate::static_page_view_support::to_static_page_image_job_view;

const DEFAULT_STATIC_PAGE_IMAGE_QUEUE_MESSAGE: &str =
    "资源正在排队，可以联系商务开通高级用户跳过等待。";
const DEFAULT_STATIC_PAGE_IMAGE_OPERATION_SUMMARY: &str = "可视化任务已进入资源队列。";
const STATIC_PAGE_IMAGE_JOB_SUBMIT_ACTION: &str = "submit_static_page_image_preview";
const STATIC_PAGE_IMAGE_JOB_CREATED_EVENT: &str = "static_page_image_job.created";
const STATIC_PAGE_IMAGE_JOB_PREVIEW_DATA_QUALITY_GATE_ERROR: &str =
    "static_page_preview_data_quality_gate";

type StaticPageImageJobQueueOperations = (Vec<Value>, String);

#[derive(Debug, Default)]
pub(crate) struct StaticPageImageJobCreateOptions {
    pub(crate) task_available_at: Option<DateTime<Utc>>,
    pub(crate) task_payload_patch: Option<Value>,
    pub(crate) queue_message: Option<String>,
    pub(crate) operation_summary: Option<String>,
}

pub(crate) fn static_page_image_job_submit_action() -> &'static str {
    STATIC_PAGE_IMAGE_JOB_SUBMIT_ACTION
}

pub(crate) fn static_page_image_job_request_prompt(prompt: &Option<String>) -> Option<&str> {
    prompt.as_deref()
}

pub(crate) fn static_page_image_job_created_event_name() -> &'static str {
    STATIC_PAGE_IMAGE_JOB_CREATED_EVENT
}

pub(crate) fn static_page_image_job_preview_data_quality_gate_error_code() -> &'static str {
    STATIC_PAGE_IMAGE_JOB_PREVIEW_DATA_QUALITY_GATE_ERROR
}

pub(crate) fn static_page_image_job_is_prompt_only_preview(
    request_image_prompt_payload: &Value,
) -> bool {
    static_page_image_prompt_payload_is_prompt_only(request_image_prompt_payload)
}

pub(crate) fn static_page_image_job_should_refresh_data_contract(
    prompt_only_preview: bool,
) -> bool {
    !prompt_only_preview
}

pub(crate) fn static_page_image_job_has_request_prompt_payload(
    request_image_prompt_payload: &Value,
) -> bool {
    !request_image_prompt_payload.is_null()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StaticPageImageJobWorkflowTaskContext {
    pub(crate) workflow_task_id: Option<WorkflowTaskId>,
    pub(crate) workflow_task_available_at: Option<DateTime<Utc>>,
}

pub(crate) fn static_page_image_job_workflow_task_context(
    task: Option<&WorkflowTaskView>,
) -> StaticPageImageJobWorkflowTaskContext {
    StaticPageImageJobWorkflowTaskContext {
        workflow_task_id: task.map(|task| task.id),
        workflow_task_available_at: task.map(|task| task.available_at),
    }
}

pub(crate) fn static_page_image_job_workflow_task_context_from_updated_task(
    task: &WorkflowTask,
) -> StaticPageImageJobWorkflowTaskContext {
    StaticPageImageJobWorkflowTaskContext {
        workflow_task_id: Some(task.id),
        workflow_task_available_at: Some(task.available_at),
    }
}

pub(crate) fn static_page_image_job_workflow_task_payload(
    task: &WorkflowTaskView,
    options: &StaticPageImageJobCreateOptions,
) -> Value {
    let mut task_payload = task.payload.clone();
    if let Some(patch) = options.task_payload_patch.as_ref() {
        merge_json_value(&mut task_payload, patch);
    }
    task_payload
}

pub(crate) fn static_page_image_job_should_update_workflow_task(
    options: &StaticPageImageJobCreateOptions,
) -> bool {
    options.task_payload_patch.is_some() || options.task_available_at.is_some()
}

pub(crate) fn static_page_image_job_workflow_task_available_at(
    options: &StaticPageImageJobCreateOptions,
) -> Option<DateTime<Utc>> {
    options.task_available_at
}

pub(crate) fn static_page_image_generation_workflow_context(
    mut context: Map<String, Value>,
    draft: &StaticPageDraft,
    job: &StaticPageImageJob,
    prompt: Option<&str>,
) -> Map<String, Value> {
    context.insert(
        "static_page_draft_id".to_string(),
        Value::String(draft.id.to_string()),
    );
    context.insert(
        "static_page_image_job_id".to_string(),
        Value::String(job.id.to_string()),
    );
    context.insert(
        "assistant_run_id".to_string(),
        Value::String(draft.assistant_run_id.to_string()),
    );
    if let Some(prompt) = prompt.map(str::trim).filter(|value| !value.is_empty()) {
        context.insert("prompt".to_string(), Value::String(prompt.to_string()));
    }
    if let Some(prewarm_key) = static_page_template_prewarm_key_from_source_refs(&draft.source_refs)
    {
        context.insert("prewarm_key".to_string(), Value::String(prewarm_key));
        context.insert("low_load_only".to_string(), Value::Bool(true));
        context.insert("customer_visible".to_string(), Value::Bool(false));
    }
    context
}

pub(crate) fn static_page_image_job_create_response(
    job: StaticPageImageJob,
) -> CreateStaticPageImageJobResponse {
    CreateStaticPageImageJobResponse {
        image_job: to_static_page_image_job_view(job),
    }
}

pub(crate) fn static_page_image_job_prompt_payload(
    draft: &StaticPageDraft,
    request_image_prompt_payload: Value,
    prompt: Option<&str>,
) -> Value {
    if request_image_prompt_payload.is_null() {
        build_static_page_image_prompt_payload(draft, prompt)
    } else {
        request_image_prompt_payload
    }
}

pub(crate) fn static_page_image_job_queue_message(
    options: &StaticPageImageJobCreateOptions,
) -> &str {
    options
        .queue_message
        .as_deref()
        .unwrap_or(DEFAULT_STATIC_PAGE_IMAGE_QUEUE_MESSAGE)
}

pub(crate) fn static_page_image_job_operation_summary(
    options: &StaticPageImageJobCreateOptions,
) -> &str {
    options
        .operation_summary
        .as_deref()
        .unwrap_or(DEFAULT_STATIC_PAGE_IMAGE_OPERATION_SUMMARY)
}

pub(crate) fn static_page_image_job_queue_operations(
    job: &StaticPageImageJob,
    options: &StaticPageImageJobCreateOptions,
) -> StaticPageImageJobQueueOperations {
    let queue_message = static_page_image_job_queue_message(options);
    let operation_summary = static_page_image_job_operation_summary(options).to_string();
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

pub(crate) fn static_page_image_job_created_event_payload_from_context(
    draft: &StaticPageDraft,
    job: &StaticPageImageJob,
    workflow_execution: &WorkflowExecution,
    workflow_task_context: StaticPageImageJobWorkflowTaskContext,
) -> Value {
    static_page_image_job_created_event_payload(
        draft,
        job,
        workflow_execution,
        workflow_task_context.workflow_task_id,
        workflow_task_context.workflow_task_available_at,
    )
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use contracts::StaticPageImageJobStatusView;
    use domain_model::{
        AssistantRunId, StaticPageDraftId, StaticPageDraftStatus, StaticPageImageJobId,
        StaticPageImageJobStatus, TenantId, WorkflowExecutionId, WorkflowKind, WorkflowStatus,
        WorkflowTaskStatus,
    };
    use serde_json::json;

    use super::*;

    #[test]
    fn submit_action_matches_existing_preview_refresh_action() {
        assert_eq!(
            static_page_image_job_submit_action(),
            "submit_static_page_image_preview"
        );
    }

    #[test]
    fn request_prompt_borrows_optional_prompt_text() {
        assert_eq!(
            static_page_image_job_request_prompt(&Some("生成经营看板".to_string())),
            Some("生成经营看板")
        );
        assert_eq!(static_page_image_job_request_prompt(&None), None);
    }

    #[test]
    fn created_event_name_matches_existing_static_page_image_job_event() {
        assert_eq!(
            static_page_image_job_created_event_name(),
            "static_page_image_job.created"
        );
    }

    #[test]
    fn preview_data_quality_gate_error_code_matches_existing_error_code() {
        assert_eq!(
            static_page_image_job_preview_data_quality_gate_error_code(),
            "static_page_preview_data_quality_gate"
        );
    }

    #[test]
    fn is_prompt_only_preview_accepts_existing_prompt_only_flags() {
        assert!(static_page_image_job_is_prompt_only_preview(&json!({
            "promptOnly": true
        })));
        assert!(static_page_image_job_is_prompt_only_preview(&json!({
            "prompt_only": true
        })));
    }

    #[test]
    fn is_prompt_only_preview_preserves_existing_flag_precedence() {
        assert!(!static_page_image_job_is_prompt_only_preview(&json!({
            "promptOnly": false,
            "prompt_only": true
        })));
    }

    #[test]
    fn is_prompt_only_preview_defaults_to_false() {
        assert!(!static_page_image_job_is_prompt_only_preview(&json!({})));
        assert!(!static_page_image_job_is_prompt_only_preview(&Value::Null));
    }

    #[test]
    fn should_refresh_data_contract_skips_prompt_only_preview() {
        assert!(!static_page_image_job_should_refresh_data_contract(true));
        assert!(static_page_image_job_should_refresh_data_contract(false));
    }

    #[test]
    fn has_request_prompt_payload_is_false_only_for_null_payload() {
        assert!(!static_page_image_job_has_request_prompt_payload(
            &Value::Null
        ));
        assert!(static_page_image_job_has_request_prompt_payload(&json!({})));
        assert!(static_page_image_job_has_request_prompt_payload(&json!({
            "prompt": "use this payload"
        })));
    }

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

    fn workflow_task() -> WorkflowTaskView {
        let now = Utc::now();
        WorkflowTaskView {
            id: WorkflowTaskId::new(),
            queue: "static_page".to_string(),
            task_key: "generate_static_page_image".to_string(),
            logical_queue: None,
            logical_task_key: None,
            remote_task_id: None,
            next_poll_at: None,
            payload: json!({}),
            status: WorkflowTaskStatus::Queued,
            attempt: 0,
            max_attempts: 3,
            available_at: now,
            claimed_at: None,
            finished_at: None,
            error: None,
            updated_at: now,
        }
    }

    fn updated_workflow_task() -> WorkflowTask {
        let now = Utc::now();
        WorkflowTask {
            id: WorkflowTaskId::new(),
            tenant_id: TenantId::new(),
            execution_id: WorkflowExecutionId::new(),
            queue: "static_page".to_string(),
            task_key: "generate_static_page_image".to_string(),
            payload: json!({}),
            status: WorkflowTaskStatus::Queued,
            attempt: 0,
            max_attempts: 3,
            available_at: now,
            claimed_at: None,
            finished_at: None,
            error: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn workflow_task_context_preserves_first_task_id_and_available_at() {
        let task = workflow_task();

        let context = static_page_image_job_workflow_task_context(Some(&task));

        assert_eq!(context.workflow_task_id, Some(task.id));
        assert_eq!(context.workflow_task_available_at, Some(task.available_at));
    }

    #[test]
    fn workflow_task_context_allows_missing_task() {
        let context = static_page_image_job_workflow_task_context(None);

        assert_eq!(context.workflow_task_id, None);
        assert_eq!(context.workflow_task_available_at, None);
    }

    #[test]
    fn workflow_task_context_from_updated_task_uses_updated_task_values() {
        let task = updated_workflow_task();

        let context = static_page_image_job_workflow_task_context_from_updated_task(&task);

        assert_eq!(context.workflow_task_id, Some(task.id));
        assert_eq!(context.workflow_task_available_at, Some(task.available_at));
    }

    #[test]
    fn workflow_task_payload_applies_patch_recursively() {
        let mut task = workflow_task();
        task.payload = json!({
            "kind": "static-page-visual",
            "metadata": {
                "source": "draft",
                "priority": "normal"
            }
        });
        let options = StaticPageImageJobCreateOptions {
            task_payload_patch: Some(json!({
                "metadata": {
                    "priority": "prewarm",
                    "templateId": "template-1"
                }
            })),
            ..StaticPageImageJobCreateOptions::default()
        };

        let payload = static_page_image_job_workflow_task_payload(&task, &options);

        assert_eq!(payload["kind"], json!("static-page-visual"));
        assert_eq!(payload["metadata"]["source"], json!("draft"));
        assert_eq!(payload["metadata"]["priority"], json!("prewarm"));
        assert_eq!(payload["metadata"]["templateId"], json!("template-1"));
    }

    #[test]
    fn workflow_task_payload_preserves_payload_without_patch() {
        let mut task = workflow_task();
        task.payload = json!({"kind": "static-page-visual"});

        let payload = static_page_image_job_workflow_task_payload(
            &task,
            &StaticPageImageJobCreateOptions::default(),
        );

        assert_eq!(payload, task.payload);
    }

    #[test]
    fn should_update_workflow_task_tracks_payload_patch_or_available_at() {
        assert!(!static_page_image_job_should_update_workflow_task(
            &StaticPageImageJobCreateOptions::default()
        ));
        assert!(static_page_image_job_should_update_workflow_task(
            &StaticPageImageJobCreateOptions {
                task_payload_patch: Some(json!({"metadata": {"priority": "prewarm"}})),
                ..StaticPageImageJobCreateOptions::default()
            }
        ));
        assert!(static_page_image_job_should_update_workflow_task(
            &StaticPageImageJobCreateOptions {
                task_available_at: Some(Utc::now()),
                ..StaticPageImageJobCreateOptions::default()
            }
        ));
    }

    #[test]
    fn workflow_task_available_at_returns_configured_available_at() {
        let available_at = Utc::now();
        let options = StaticPageImageJobCreateOptions {
            task_available_at: Some(available_at),
            ..StaticPageImageJobCreateOptions::default()
        };

        assert_eq!(
            static_page_image_job_workflow_task_available_at(&options),
            Some(available_at)
        );
        assert_eq!(
            static_page_image_job_workflow_task_available_at(
                &StaticPageImageJobCreateOptions::default()
            ),
            None
        );
    }

    #[test]
    fn image_generation_workflow_context_preserves_runtime_and_image_fields() {
        let draft = draft();
        let job = queued_job(Some(1));
        let mut context = Map::new();
        context.insert("retries_remaining".to_string(), json!(3));
        context.insert("stage_context".to_string(), json!("queued"));

        let context = static_page_image_generation_workflow_context(
            context,
            &draft,
            &job,
            Some("  生成经营看板  "),
        );

        assert_eq!(context["retries_remaining"], json!(3));
        assert_eq!(context["stage_context"], json!("queued"));
        assert_eq!(context["static_page_draft_id"], json!(draft.id.to_string()));
        assert_eq!(
            context["static_page_image_job_id"],
            json!(job.id.to_string())
        );
        assert_eq!(
            context["assistant_run_id"],
            json!(draft.assistant_run_id.to_string())
        );
        assert_eq!(context["prompt"], json!("生成经营看板"));
        assert!(!context.contains_key("prewarm_key"));
        assert!(!context.contains_key("low_load_only"));
        assert!(!context.contains_key("customer_visible"));
    }

    #[test]
    fn image_generation_workflow_context_adds_prewarm_flags_and_skips_blank_prompt() {
        let mut draft = draft();
        draft.source_refs = json!({
            "prewarm": {
                "key": "static-page-template-prewarm:test"
            }
        });
        let job = queued_job(None);

        let context =
            static_page_image_generation_workflow_context(Map::new(), &draft, &job, Some("   "));

        assert!(!context.contains_key("prompt"));
        assert_eq!(
            context["prewarm_key"],
            json!("static-page-template-prewarm:test")
        );
        assert_eq!(context["low_load_only"], json!(true));
        assert_eq!(context["customer_visible"], json!(false));
    }

    #[test]
    fn create_response_wraps_image_job_view() {
        let job = queued_job(Some(5));

        let response = static_page_image_job_create_response(job.clone());

        assert_eq!(response.image_job.id, job.id);
        assert_eq!(response.image_job.draft_id, job.draft_id);
        assert_eq!(
            response.image_job.status,
            StaticPageImageJobStatusView::Queued
        );
        assert_eq!(response.image_job.queue_position, Some(5));
    }

    #[test]
    fn prompt_payload_builds_from_draft_when_request_payload_is_null() {
        let draft = draft();

        let payload = static_page_image_job_prompt_payload(
            &draft,
            Value::Null,
            Some("  生成门店经营分析效果图  "),
        );

        assert_eq!(payload["draft_id"], json!(draft.id));
        assert_eq!(payload["assistant_run_id"], json!(draft.assistant_run_id));
        assert_eq!(payload["title"], json!(draft.title));
        assert_eq!(payload["prompt"], json!("生成门店经营分析效果图"));
        assert_eq!(payload["promptText"], json!("生成门店经营分析效果图"));
        assert_eq!(payload["prompt_text"], json!("生成门店经营分析效果图"));
    }

    #[test]
    fn prompt_payload_preserves_non_null_request_payload() {
        let draft = draft();
        let request_payload = json!({
            "prompt": "客户已指定的 Image2 payload",
            "style": "dashboard"
        });

        let payload = static_page_image_job_prompt_payload(
            &draft,
            request_payload.clone(),
            Some("fallback prompt"),
        );

        assert_eq!(payload, request_payload);
    }

    #[test]
    fn queue_message_uses_default_or_custom_copy() {
        assert_eq!(
            static_page_image_job_queue_message(&StaticPageImageJobCreateOptions::default()),
            DEFAULT_STATIC_PAGE_IMAGE_QUEUE_MESSAGE
        );

        let options = StaticPageImageJobCreateOptions {
            queue_message: Some("低负载时自动预热模板，客户不可见。".to_string()),
            ..StaticPageImageJobCreateOptions::default()
        };

        assert_eq!(
            static_page_image_job_queue_message(&options),
            "低负载时自动预热模板，客户不可见。"
        );
    }

    #[test]
    fn operation_summary_uses_default_or_custom_copy() {
        assert_eq!(
            static_page_image_job_operation_summary(&StaticPageImageJobCreateOptions::default()),
            DEFAULT_STATIC_PAGE_IMAGE_OPERATION_SUMMARY
        );

        let options = StaticPageImageJobCreateOptions {
            operation_summary: Some("静态页模板预热任务已进入低优先级队列。".to_string()),
            ..StaticPageImageJobCreateOptions::default()
        };

        assert_eq!(
            static_page_image_job_operation_summary(&options),
            "静态页模板预热任务已进入低优先级队列。"
        );
    }

    #[test]
    fn queue_operations_use_default_customer_queue_copy() {
        let job = queued_job(Some(3));

        let fields: StaticPageImageJobQueueOperations = static_page_image_job_queue_operations(
            &job,
            &StaticPageImageJobCreateOptions::default(),
        );
        let (operations, summary) = fields;

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

        let fields: StaticPageImageJobQueueOperations =
            static_page_image_job_queue_operations(&job, &options);
        let (operations, summary) = fields;

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

    #[test]
    fn created_event_payload_from_context_preserves_task_context() {
        let draft = draft();
        let job = queued_job(Some(1));
        let execution = workflow_execution();
        let task = workflow_task();
        let context = static_page_image_job_workflow_task_context(Some(&task));

        let payload = static_page_image_job_created_event_payload_from_context(
            &draft, &job, &execution, context,
        );

        assert_eq!(payload["workflow_task_id"], json!(task.id));
        assert_eq!(
            payload["workflow_task_available_at"],
            json!(task.available_at)
        );
    }
}
