use contracts::{
    CodexHostFixedTaskTemplateContextView, CodexHostFixedTaskTemplateIdView,
    StaticPageImageJobStatusView, StaticPageImageJobView,
};
use serde_json::Value;

pub(crate) fn external_channel_static_page_fixed_task_preview_ready(
    fixed_task: &CodexHostFixedTaskTemplateContextView,
) -> bool {
    fixed_task.template_id == CodexHostFixedTaskTemplateIdView::StaticPageImage2DataPublish
        && fixed_task
            .image2
            .get("preview_asset_key")
            .and_then(Value::as_str)
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
        && fixed_task
            .image2
            .get("human_confirmation_required")
            .and_then(Value::as_bool)
            == Some(false)
        && fixed_task
            .policies
            .get("effect_image_confirmation_required")
            .and_then(Value::as_bool)
            == Some(false)
        && fixed_task
            .policies
            .get("continue_to_publish_after_effect_image")
            .and_then(Value::as_bool)
            == Some(true)
}

pub(crate) fn external_channel_static_page_image_job_preview_ready(
    image_job: &StaticPageImageJobView,
) -> bool {
    image_job
        .preview_asset_key
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
        && matches!(
            image_job.status,
            StaticPageImageJobStatusView::PreviewReady | StaticPageImageJobStatusView::Confirmed
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{AssistantRunId, StaticPageDraftId, StaticPageImageJobId};
    use serde_json::json;

    fn base_image_job(status: StaticPageImageJobStatusView) -> StaticPageImageJobView {
        let now = Utc::now();
        StaticPageImageJobView {
            id: StaticPageImageJobId::new(),
            draft_id: StaticPageDraftId::new(),
            assistant_run_id: AssistantRunId::new(),
            status,
            queue_position: None,
            image_prompt_payload: json!({}),
            preview_asset_key: Some("static-page-previews/xinbai.png".to_string()),
            failure_reason: None,
            confirmed_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn fixed_task_preview_ready_requires_static_page_template_and_auto_publish_policies() {
        let mut fixed_task =
            CodexHostFixedTaskTemplateContextView::static_page_image2_data_publish_example();
        fixed_task.image2["preview_asset_key"] = json!(" static-page-previews/xinbai.png ");
        fixed_task.image2["human_confirmation_required"] = json!(false);
        fixed_task.policies["effect_image_confirmation_required"] = json!(false);
        fixed_task.policies["continue_to_publish_after_effect_image"] = json!(true);

        assert!(external_channel_static_page_fixed_task_preview_ready(
            &fixed_task
        ));

        fixed_task.policies["continue_to_publish_after_effect_image"] = json!(false);
        assert!(!external_channel_static_page_fixed_task_preview_ready(
            &fixed_task
        ));
    }

    #[test]
    fn fixed_task_preview_ready_rejects_missing_preview_and_wrong_template() {
        let mut fixed_task =
            CodexHostFixedTaskTemplateContextView::static_page_image2_data_publish_example();
        fixed_task.image2["preview_asset_key"] = json!("   ");
        fixed_task.image2["human_confirmation_required"] = json!(false);
        fixed_task.policies["effect_image_confirmation_required"] = json!(false);
        fixed_task.policies["continue_to_publish_after_effect_image"] = json!(true);
        assert!(!external_channel_static_page_fixed_task_preview_ready(
            &fixed_task
        ));

        fixed_task.image2["preview_asset_key"] = json!("static-page-previews/xinbai.png");
        fixed_task.template_id = CodexHostFixedTaskTemplateIdView::DataIngestionAnalysis;
        assert!(!external_channel_static_page_fixed_task_preview_ready(
            &fixed_task
        ));
    }

    #[test]
    fn image_job_preview_ready_accepts_preview_ready_and_confirmed_with_asset() {
        assert!(external_channel_static_page_image_job_preview_ready(
            &base_image_job(StaticPageImageJobStatusView::PreviewReady)
        ));
        assert!(external_channel_static_page_image_job_preview_ready(
            &base_image_job(StaticPageImageJobStatusView::Confirmed)
        ));
    }

    #[test]
    fn image_job_preview_ready_rejects_non_ready_status_or_blank_asset() {
        assert!(!external_channel_static_page_image_job_preview_ready(
            &base_image_job(StaticPageImageJobStatusView::Queued)
        ));

        let mut job = base_image_job(StaticPageImageJobStatusView::PreviewReady);
        job.preview_asset_key = Some("   ".to_string());
        assert!(!external_channel_static_page_image_job_preview_ready(&job));

        job.preview_asset_key = None;
        assert!(!external_channel_static_page_image_job_preview_ready(&job));
    }
}
