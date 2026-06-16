use contracts::{
    StaticPageDraftStatusView, StaticPageDraftView, StaticPageImageJobStatusView,
    StaticPageImageJobView, StaticPageRenderOutputStatusView, StaticPageRenderOutputView,
    StaticPageTemplateView,
};
use domain_model::{StaticPageDraft, StaticPageImageJob, StaticPageRenderOutput};
use serde_json::Value;

use crate::{
    static_page_dataset_artifact_key_from_draft_context,
    static_page_generated_template_reference_from_draft,
    static_page_generated_template_reference_id, static_page_html_download_url,
    static_page_html_preview_url, static_page_published_public_url_from_draft,
    static_page_render_output_retryable_error_reason, static_page_template_preview_url_from_draft,
};

pub(crate) fn to_static_page_draft_view(draft: StaticPageDraft) -> StaticPageDraftView {
    StaticPageDraftView {
        id: draft.id,
        assistant_run_id: draft.assistant_run_id,
        title: draft.title,
        status: StaticPageDraftStatusView::from_domain(draft.status),
        selected_scope: draft.selected_scope,
        visibility_snapshot: draft.visibility_snapshot,
        source_refs: draft.source_refs,
        draft_payload: draft.draft_payload,
        created_at: draft.created_at,
        updated_at: draft.updated_at,
    }
}

pub(crate) fn to_static_page_template_view(
    draft: StaticPageDraft,
) -> Option<StaticPageTemplateView> {
    let public_url = static_page_published_public_url_from_draft(&draft)?;
    let preview_url = static_page_template_preview_url_from_draft(&draft);
    let dataset_artifact_key = static_page_dataset_artifact_key_from_draft_context(&draft);
    let template_reference =
        static_page_generated_template_reference_from_draft(&draft, &public_url);

    Some(StaticPageTemplateView {
        id: static_page_generated_template_reference_id(draft.id),
        draft_id: draft.id,
        assistant_run_id: draft.assistant_run_id,
        title: draft.title,
        public_url,
        preview_url,
        dataset_artifact_key,
        template_reference,
        selected_scope: draft.selected_scope,
        source_refs: draft.source_refs,
        created_at: draft.created_at,
        updated_at: draft.updated_at,
    })
}

pub(crate) fn to_static_page_image_job_view(job: StaticPageImageJob) -> StaticPageImageJobView {
    StaticPageImageJobView {
        id: job.id,
        draft_id: job.draft_id,
        assistant_run_id: job.assistant_run_id,
        status: StaticPageImageJobStatusView::from_domain(job.status),
        queue_position: job.queue_position,
        image_prompt_payload: job.image_prompt_payload,
        preview_asset_key: job.preview_asset_key,
        failure_reason: job.failure_reason,
        confirmed_at: job.confirmed_at,
        created_at: job.created_at,
        updated_at: job.updated_at,
    }
}

pub(crate) fn to_static_page_render_output_view(
    output: StaticPageRenderOutput,
    selected_scope: Option<&Value>,
) -> StaticPageRenderOutputView {
    let html_download_url = static_page_html_download_url(selected_scope, &output);
    let html_preview_url = static_page_html_preview_url(selected_scope, &output);
    let retryable_error_reason = static_page_render_output_retryable_error_reason(&output);
    StaticPageRenderOutputView {
        id: output.id,
        draft_id: output.draft_id,
        assistant_run_id: output.assistant_run_id,
        image_job_id: output.image_job_id,
        status: StaticPageRenderOutputStatusView::from_domain(output.status),
        html: output.html,
        html_download_url: html_download_url.clone(),
        html_download_url_camel: html_download_url.clone(),
        download_url: html_download_url.clone(),
        download_url_camel: html_download_url,
        html_preview_url: html_preview_url.clone(),
        html_preview_url_camel: html_preview_url,
        retryable_error_reason: retryable_error_reason.clone(),
        retryable_error_reason_camel: retryable_error_reason,
        asset_manifest: output.asset_manifest,
        created_at: output.created_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{
        AssistantRunId, StaticPageDraftId, StaticPageDraftStatus, StaticPageImageJobId,
        StaticPageImageJobStatus, StaticPageRenderOutputId, StaticPageRenderOutputStatus, TenantId,
    };
    use serde_json::json;

    const PUBLIC_URL: &str =
        "https://v3.elepcloud.com/generated-artifacts/database-static-pages/test/report/index.html";
    const PREVIEW_URL: &str =
        "https://v3.elepcloud.com/generated-artifacts/database-static-pages/test/report/effect.png";

    fn test_draft() -> StaticPageDraft {
        let now = Utc::now();
        StaticPageDraft {
            id: StaticPageDraftId::new(),
            tenant_id: TenantId::new(),
            assistant_run_id: AssistantRunId::new(),
            owner_user_id: None,
            title: "经营月报".to_string(),
            status: StaticPageDraftStatus::Rendered,
            selected_scope: json!({"type": "external_channel", "channelConnectionId": "main"}),
            visibility_snapshot: json!({"visible": true}),
            source_refs: json!({
                "dataset_artifact_key": "dataset-key-1",
                "image2": {"preview_url": PREVIEW_URL}
            }),
            draft_payload: json!({
                "finalPage": {
                    "publicUrl": PUBLIC_URL
                }
            }),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn static_page_draft_view_preserves_basic_fields() {
        let draft = test_draft();
        let draft_id = draft.id;
        let assistant_run_id = draft.assistant_run_id;
        let created_at = draft.created_at;

        let view = to_static_page_draft_view(draft);

        assert_eq!(view.id, draft_id);
        assert_eq!(view.assistant_run_id, assistant_run_id);
        assert_eq!(view.title, "经营月报");
        assert_eq!(view.status, StaticPageDraftStatusView::Rendered);
        assert_eq!(view.visibility_snapshot, json!({"visible": true}));
        assert_eq!(view.created_at, created_at);
    }

    #[test]
    fn static_page_template_view_builds_generated_reference() {
        let draft = test_draft();
        let draft_id = draft.id;
        let view = to_static_page_template_view(draft).expect("published draft becomes template");

        assert_eq!(view.id, format!("generated-static-page:{draft_id}"));
        assert_eq!(view.draft_id, draft_id);
        assert_eq!(view.public_url, PUBLIC_URL);
        assert_eq!(view.preview_url.as_deref(), Some(PREVIEW_URL));
        assert_eq!(view.dataset_artifact_key.as_deref(), Some("dataset-key-1"));
        assert_eq!(view.template_reference["publicUrl"], json!(PUBLIC_URL));
        assert_eq!(view.template_reference["previewUrl"], json!(PREVIEW_URL));
        assert_eq!(
            view.template_reference["datasetArtifactKey"],
            json!("dataset-key-1")
        );
    }

    #[test]
    fn static_page_image_job_view_preserves_status_and_failure_fields() {
        let now = Utc::now();
        let job = StaticPageImageJob {
            id: StaticPageImageJobId::new(),
            tenant_id: TenantId::new(),
            draft_id: StaticPageDraftId::new(),
            assistant_run_id: AssistantRunId::new(),
            status: StaticPageImageJobStatus::Failed,
            queue_position: Some(3),
            image_prompt_payload: json!({"prompt": "dark mobile report"}),
            preview_asset_key: Some("preview.png".to_string()),
            failure_reason: Some("image timeout".to_string()),
            confirmed_at: Some(now),
            created_at: now,
            updated_at: now,
        };

        let view = to_static_page_image_job_view(job);

        assert_eq!(view.status, StaticPageImageJobStatusView::Failed);
        assert_eq!(view.queue_position, Some(3));
        assert_eq!(view.preview_asset_key.as_deref(), Some("preview.png"));
        assert_eq!(view.failure_reason.as_deref(), Some("image timeout"));
        assert_eq!(view.confirmed_at, Some(now));
    }

    #[test]
    fn static_page_render_output_view_sets_download_aliases_and_retry_reason() {
        let output_id = StaticPageRenderOutputId::new();
        let rendered = StaticPageRenderOutput {
            id: output_id,
            tenant_id: TenantId::new(),
            draft_id: StaticPageDraftId::new(),
            assistant_run_id: AssistantRunId::new(),
            owner_user_id: None,
            image_job_id: Some(StaticPageImageJobId::new()),
            status: StaticPageRenderOutputStatus::Rendered,
            html: "<html>ok</html>".to_string(),
            asset_manifest: json!({}),
            created_at: Utc::now(),
        };
        let view = to_static_page_render_output_view(
            rendered,
            Some(&json!({
                "type": "external_channel",
                "channelConnectionId": "generic chat/主通道"
            })),
        );

        let expected_download = format!(
            "/v1/external/channels/generic%20chat%2F%E4%B8%BB%E9%80%9A%E9%81%93/static-page-renders/{output_id}/download"
        );
        let expected_preview = format!(
            "/v1/external/channels/generic%20chat%2F%E4%B8%BB%E9%80%9A%E9%81%93/static-page-renders/{output_id}/preview"
        );
        assert_eq!(
            view.html_download_url.as_deref(),
            Some(expected_download.as_str())
        );
        assert_eq!(
            view.html_download_url_camel.as_deref(),
            Some(expected_download.as_str())
        );
        assert_eq!(
            view.download_url.as_deref(),
            Some(expected_download.as_str())
        );
        assert_eq!(
            view.download_url_camel.as_deref(),
            Some(expected_download.as_str())
        );
        assert_eq!(
            view.html_preview_url.as_deref(),
            Some(expected_preview.as_str())
        );
        assert_eq!(
            view.html_preview_url_camel.as_deref(),
            Some(expected_preview.as_str())
        );
        assert_eq!(view.retryable_error_reason, None);

        let failed = StaticPageRenderOutput {
            id: StaticPageRenderOutputId::new(),
            tenant_id: TenantId::new(),
            draft_id: StaticPageDraftId::new(),
            assistant_run_id: AssistantRunId::new(),
            owner_user_id: None,
            image_job_id: None,
            status: StaticPageRenderOutputStatus::Failed,
            html: String::new(),
            asset_manifest: json!({"workflow": {"error": {"reason": "render timeout"}}}),
            created_at: Utc::now(),
        };
        let failed_view = to_static_page_render_output_view(failed, None);
        assert_eq!(failed_view.html_download_url, None);
        assert_eq!(
            failed_view.retryable_error_reason.as_deref(),
            Some("render timeout")
        );
        assert_eq!(
            failed_view.retryable_error_reason_camel.as_deref(),
            Some("render timeout")
        );
    }
}
