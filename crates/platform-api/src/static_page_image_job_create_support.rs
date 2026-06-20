use chrono::{DateTime, Utc};
use domain_model::StaticPageImageJob;
use serde_json::{json, Value};

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

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use domain_model::{
        AssistantRunId, StaticPageDraftId, StaticPageImageJobId, StaticPageImageJobStatus, TenantId,
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
}
