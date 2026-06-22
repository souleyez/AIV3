use chrono::{DateTime, Utc};
use contracts::{
    StaticPageDraftStatusView, StaticPageDraftView, StaticPageImageJobStatusView,
    StaticPageImageJobView, StaticPageRenderOutputStatusView, StaticPageRenderOutputView,
    StaticPageTemplateView,
};
use domain_model::{
    AssistantRunId, StaticPageDraft, StaticPageDraftId, StaticPageDraftStatus, StaticPageImageJob,
    StaticPageImageJobId, StaticPageImageJobStatus, StaticPageRenderOutput,
    StaticPageRenderOutputId, StaticPageRenderOutputStatus,
};
use serde_json::Value;

use crate::{
    static_page_dataset_artifact_key_from_draft_context,
    static_page_generated_template_reference_from_draft,
    static_page_generated_template_reference_id, static_page_html_download_url,
    static_page_html_preview_url, static_page_published_public_url_from_draft,
    static_page_render_output_retryable_error_reason, static_page_template_preview_url_from_draft,
};

type StaticPageDraftViewFields = (
    StaticPageDraftId,
    AssistantRunId,
    String,
    StaticPageDraftStatusView,
    Value,
    Value,
    Value,
    Value,
    DateTime<Utc>,
    DateTime<Utc>,
);

type StaticPageDraftViewIdentityFields = (StaticPageDraftId, AssistantRunId, String);
type StaticPageDraftViewPayloadFields = (Value, Value, Value, Value);
type StaticPageDraftViewTimestampFields = (DateTime<Utc>, DateTime<Utc>);

type StaticPageTemplateViewFields = (
    String,
    StaticPageDraftId,
    AssistantRunId,
    String,
    String,
    Option<String>,
    Option<String>,
    Value,
    Value,
    Value,
    DateTime<Utc>,
    DateTime<Utc>,
);

type StaticPageTemplateViewIdentityFields = (String, StaticPageDraftId, AssistantRunId, String);
type StaticPageTemplateViewContextFields = (Value, Value);
type StaticPageTemplateViewTimestampFields = (DateTime<Utc>, DateTime<Utc>);

type StaticPageImageJobViewFields = (
    StaticPageImageJobId,
    StaticPageDraftId,
    AssistantRunId,
    StaticPageImageJobStatusView,
    Option<i32>,
    Value,
    Option<String>,
    Option<String>,
    Option<DateTime<Utc>>,
    DateTime<Utc>,
    DateTime<Utc>,
);

type StaticPageImageJobViewIdentityFields =
    (StaticPageImageJobId, StaticPageDraftId, AssistantRunId);
type StaticPageImageJobViewDetailFields = (
    Option<i32>,
    Value,
    Option<String>,
    Option<String>,
    Option<DateTime<Utc>>,
);
type StaticPageImageJobViewTimestampFields = (DateTime<Utc>, DateTime<Utc>);

type StaticPageRenderOutputViewIdentityFields = (
    StaticPageRenderOutputId,
    StaticPageDraftId,
    AssistantRunId,
    Option<StaticPageImageJobId>,
);
type StaticPageRenderOutputViewContentFields = (String, Value);
type StaticPageRenderOutputViewDownloadAliasFields = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);
type StaticPageRenderOutputViewPreviewAliasFields = (Option<String>, Option<String>);
type StaticPageRenderOutputViewRetryableErrorReasonAliasFields = (Option<String>, Option<String>);
type StaticPageRenderOutputViewLinkFields = (Option<String>, Option<String>, Option<String>);

type StaticPageRenderOutputViewFields = (
    StaticPageRenderOutputId,
    StaticPageDraftId,
    AssistantRunId,
    Option<StaticPageImageJobId>,
    StaticPageRenderOutputStatusView,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Value,
    DateTime<Utc>,
);

pub(crate) fn to_static_page_draft_view(draft: StaticPageDraft) -> StaticPageDraftView {
    let (
        id,
        assistant_run_id,
        title,
        status,
        selected_scope,
        visibility_snapshot,
        source_refs,
        draft_payload,
        created_at,
        updated_at,
    ) = static_page_draft_view_fields(draft);
    static_page_draft_view_from_fields(
        id,
        assistant_run_id,
        title,
        status,
        selected_scope,
        visibility_snapshot,
        source_refs,
        draft_payload,
        created_at,
        updated_at,
    )
}

pub(crate) fn static_page_draft_view_fields(draft: StaticPageDraft) -> StaticPageDraftViewFields {
    let status = static_page_draft_view_status(draft.status);
    let (id, assistant_run_id, title) =
        static_page_draft_view_identity_fields(draft.id, draft.assistant_run_id, draft.title);
    let (selected_scope, visibility_snapshot, source_refs, draft_payload) =
        static_page_draft_view_payload_fields(
            draft.selected_scope,
            draft.visibility_snapshot,
            draft.source_refs,
            draft.draft_payload,
        );
    let (created_at, updated_at) =
        static_page_draft_view_timestamp_fields(draft.created_at, draft.updated_at);
    (
        id,
        assistant_run_id,
        title,
        status,
        selected_scope,
        visibility_snapshot,
        source_refs,
        draft_payload,
        created_at,
        updated_at,
    )
}

pub(crate) fn static_page_draft_view_from_fields(
    id: StaticPageDraftId,
    assistant_run_id: AssistantRunId,
    title: String,
    status: StaticPageDraftStatusView,
    selected_scope: Value,
    visibility_snapshot: Value,
    source_refs: Value,
    draft_payload: Value,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
) -> StaticPageDraftView {
    StaticPageDraftView {
        id,
        assistant_run_id,
        title,
        status,
        selected_scope,
        visibility_snapshot,
        source_refs,
        draft_payload,
        created_at,
        updated_at,
    }
}

pub(crate) fn static_page_draft_view_identity_fields(
    id: StaticPageDraftId,
    assistant_run_id: AssistantRunId,
    title: String,
) -> StaticPageDraftViewIdentityFields {
    (id, assistant_run_id, title)
}

pub(crate) fn static_page_draft_view_payload_fields(
    selected_scope: Value,
    visibility_snapshot: Value,
    source_refs: Value,
    draft_payload: Value,
) -> StaticPageDraftViewPayloadFields {
    (
        selected_scope,
        visibility_snapshot,
        source_refs,
        draft_payload,
    )
}

pub(crate) fn static_page_draft_view_timestamp_fields(
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
) -> StaticPageDraftViewTimestampFields {
    (created_at, updated_at)
}

pub(crate) fn static_page_draft_view_status(
    status: StaticPageDraftStatus,
) -> StaticPageDraftStatusView {
    StaticPageDraftStatusView::from_domain(status)
}

pub(crate) fn to_static_page_template_view(
    draft: StaticPageDraft,
) -> Option<StaticPageTemplateView> {
    let (
        id,
        draft_id,
        assistant_run_id,
        title,
        public_url,
        preview_url,
        dataset_artifact_key,
        template_reference,
        selected_scope,
        source_refs,
        created_at,
        updated_at,
    ) = static_page_template_view_fields(draft)?;
    Some(static_page_template_view_from_fields(
        id,
        draft_id,
        assistant_run_id,
        title,
        public_url,
        preview_url,
        dataset_artifact_key,
        template_reference,
        selected_scope,
        source_refs,
        created_at,
        updated_at,
    ))
}

pub(crate) fn static_page_template_view_fields(
    draft: StaticPageDraft,
) -> Option<StaticPageTemplateViewFields> {
    let (public_url, preview_url, dataset_artifact_key, template_reference) =
        static_page_template_view_artifact_fields(&draft)?;
    let (id, draft_id, assistant_run_id, title) =
        static_page_template_view_identity_fields(draft.id, draft.assistant_run_id, draft.title);
    let (selected_scope, source_refs) =
        static_page_template_view_context_fields(draft.selected_scope, draft.source_refs);
    let (created_at, updated_at) =
        static_page_template_view_timestamp_fields(draft.created_at, draft.updated_at);

    Some((
        id,
        draft_id,
        assistant_run_id,
        title,
        public_url,
        preview_url,
        dataset_artifact_key,
        template_reference,
        selected_scope,
        source_refs,
        created_at,
        updated_at,
    ))
}

pub(crate) fn static_page_template_view_from_fields(
    id: String,
    draft_id: StaticPageDraftId,
    assistant_run_id: AssistantRunId,
    title: String,
    public_url: String,
    preview_url: Option<String>,
    dataset_artifact_key: Option<String>,
    template_reference: Value,
    selected_scope: Value,
    source_refs: Value,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
) -> StaticPageTemplateView {
    StaticPageTemplateView {
        id,
        draft_id,
        assistant_run_id,
        title,
        public_url,
        preview_url,
        dataset_artifact_key,
        template_reference,
        selected_scope,
        source_refs,
        created_at,
        updated_at,
    }
}

pub(crate) fn static_page_template_view_identity_fields(
    draft_id: StaticPageDraftId,
    assistant_run_id: AssistantRunId,
    title: String,
) -> StaticPageTemplateViewIdentityFields {
    (
        static_page_generated_template_reference_id(draft_id),
        draft_id,
        assistant_run_id,
        title,
    )
}

pub(crate) fn static_page_template_view_context_fields(
    selected_scope: Value,
    source_refs: Value,
) -> StaticPageTemplateViewContextFields {
    (selected_scope, source_refs)
}

pub(crate) fn static_page_template_view_timestamp_fields(
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
) -> StaticPageTemplateViewTimestampFields {
    (created_at, updated_at)
}

pub(crate) fn static_page_template_view_artifact_fields(
    draft: &StaticPageDraft,
) -> Option<(String, Option<String>, Option<String>, Value)> {
    let public_url = static_page_published_public_url_from_draft(draft)?;
    let preview_url = static_page_template_preview_url_from_draft(draft);
    let dataset_artifact_key = static_page_dataset_artifact_key_from_draft_context(draft);
    let template_reference =
        static_page_generated_template_reference_from_draft(draft, &public_url);

    Some((
        public_url,
        preview_url,
        dataset_artifact_key,
        template_reference,
    ))
}

pub(crate) fn to_static_page_image_job_view(job: StaticPageImageJob) -> StaticPageImageJobView {
    let (
        id,
        draft_id,
        assistant_run_id,
        status,
        queue_position,
        image_prompt_payload,
        preview_asset_key,
        failure_reason,
        confirmed_at,
        created_at,
        updated_at,
    ) = static_page_image_job_view_fields(job);
    static_page_image_job_view_from_fields(
        id,
        draft_id,
        assistant_run_id,
        status,
        queue_position,
        image_prompt_payload,
        preview_asset_key,
        failure_reason,
        confirmed_at,
        created_at,
        updated_at,
    )
}

pub(crate) fn static_page_image_job_view_fields(
    job: StaticPageImageJob,
) -> StaticPageImageJobViewFields {
    let status = static_page_image_job_view_status(job.status);
    let (id, draft_id, assistant_run_id) =
        static_page_image_job_view_identity_fields(job.id, job.draft_id, job.assistant_run_id);
    let (queue_position, image_prompt_payload, preview_asset_key, failure_reason, confirmed_at) =
        static_page_image_job_view_detail_fields(
            job.queue_position,
            job.image_prompt_payload,
            job.preview_asset_key,
            job.failure_reason,
            job.confirmed_at,
        );
    let (created_at, updated_at) =
        static_page_image_job_view_timestamp_fields(job.created_at, job.updated_at);
    (
        id,
        draft_id,
        assistant_run_id,
        status,
        queue_position,
        image_prompt_payload,
        preview_asset_key,
        failure_reason,
        confirmed_at,
        created_at,
        updated_at,
    )
}

pub(crate) fn static_page_image_job_view_from_fields(
    id: StaticPageImageJobId,
    draft_id: StaticPageDraftId,
    assistant_run_id: AssistantRunId,
    status: StaticPageImageJobStatusView,
    queue_position: Option<i32>,
    image_prompt_payload: Value,
    preview_asset_key: Option<String>,
    failure_reason: Option<String>,
    confirmed_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
) -> StaticPageImageJobView {
    StaticPageImageJobView {
        id,
        draft_id,
        assistant_run_id,
        status,
        queue_position,
        image_prompt_payload,
        preview_asset_key,
        failure_reason,
        confirmed_at,
        created_at,
        updated_at,
    }
}

pub(crate) fn static_page_image_job_view_identity_fields(
    id: StaticPageImageJobId,
    draft_id: StaticPageDraftId,
    assistant_run_id: AssistantRunId,
) -> StaticPageImageJobViewIdentityFields {
    (id, draft_id, assistant_run_id)
}

pub(crate) fn static_page_image_job_view_detail_fields(
    queue_position: Option<i32>,
    image_prompt_payload: Value,
    preview_asset_key: Option<String>,
    failure_reason: Option<String>,
    confirmed_at: Option<DateTime<Utc>>,
) -> StaticPageImageJobViewDetailFields {
    (
        queue_position,
        image_prompt_payload,
        preview_asset_key,
        failure_reason,
        confirmed_at,
    )
}

pub(crate) fn static_page_image_job_view_timestamp_fields(
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
) -> StaticPageImageJobViewTimestampFields {
    (created_at, updated_at)
}

pub(crate) fn static_page_image_job_view_status(
    status: StaticPageImageJobStatus,
) -> StaticPageImageJobStatusView {
    StaticPageImageJobStatusView::from_domain(status)
}

pub(crate) fn to_static_page_render_output_view(
    output: StaticPageRenderOutput,
    selected_scope: Option<&Value>,
) -> StaticPageRenderOutputView {
    let (
        id,
        draft_id,
        assistant_run_id,
        image_job_id,
        status,
        html,
        html_download_url,
        html_download_url_camel,
        download_url,
        download_url_camel,
        html_preview_url,
        html_preview_url_camel,
        retryable_error_reason,
        retryable_error_reason_camel,
        asset_manifest,
        created_at,
    ) = static_page_render_output_view_fields(output, selected_scope);
    static_page_render_output_view_from_fields(
        id,
        draft_id,
        assistant_run_id,
        image_job_id,
        status,
        html,
        html_download_url,
        html_download_url_camel,
        download_url,
        download_url_camel,
        html_preview_url,
        html_preview_url_camel,
        retryable_error_reason,
        retryable_error_reason_camel,
        asset_manifest,
        created_at,
    )
}

pub(crate) fn static_page_render_output_view_fields(
    output: StaticPageRenderOutput,
    selected_scope: Option<&Value>,
) -> StaticPageRenderOutputViewFields {
    let (html_download_url, html_preview_url, retryable_error_reason) =
        static_page_render_output_view_link_fields(&output, selected_scope);
    let (html_download_url, html_download_url_camel, download_url, download_url_camel) =
        static_page_render_output_view_download_alias_fields(html_download_url);
    let (html_preview_url, html_preview_url_camel) =
        static_page_render_output_view_preview_alias_fields(html_preview_url);
    let (retryable_error_reason, retryable_error_reason_camel) =
        static_page_render_output_view_retryable_error_reason_alias_fields(retryable_error_reason);
    let status = static_page_render_output_view_status(output.status);
    let (id, draft_id, assistant_run_id, image_job_id) =
        static_page_render_output_view_identity_fields(
            output.id,
            output.draft_id,
            output.assistant_run_id,
            output.image_job_id,
        );
    let (html, asset_manifest) =
        static_page_render_output_view_content_fields(output.html, output.asset_manifest);
    let created_at = static_page_render_output_view_timestamp_fields(output.created_at);
    (
        id,
        draft_id,
        assistant_run_id,
        image_job_id,
        status,
        html,
        html_download_url,
        html_download_url_camel,
        download_url,
        download_url_camel,
        html_preview_url,
        html_preview_url_camel,
        retryable_error_reason,
        retryable_error_reason_camel,
        asset_manifest,
        created_at,
    )
}

pub(crate) fn static_page_render_output_view_from_fields(
    id: StaticPageRenderOutputId,
    draft_id: StaticPageDraftId,
    assistant_run_id: AssistantRunId,
    image_job_id: Option<StaticPageImageJobId>,
    status: StaticPageRenderOutputStatusView,
    html: String,
    html_download_url: Option<String>,
    html_download_url_camel: Option<String>,
    download_url: Option<String>,
    download_url_camel: Option<String>,
    html_preview_url: Option<String>,
    html_preview_url_camel: Option<String>,
    retryable_error_reason: Option<String>,
    retryable_error_reason_camel: Option<String>,
    asset_manifest: Value,
    created_at: DateTime<Utc>,
) -> StaticPageRenderOutputView {
    StaticPageRenderOutputView {
        id,
        draft_id,
        assistant_run_id,
        image_job_id,
        status,
        html,
        html_download_url,
        html_download_url_camel,
        download_url,
        download_url_camel,
        html_preview_url,
        html_preview_url_camel,
        retryable_error_reason,
        retryable_error_reason_camel,
        asset_manifest,
        created_at,
    }
}

pub(crate) fn static_page_render_output_view_identity_fields(
    id: StaticPageRenderOutputId,
    draft_id: StaticPageDraftId,
    assistant_run_id: AssistantRunId,
    image_job_id: Option<StaticPageImageJobId>,
) -> StaticPageRenderOutputViewIdentityFields {
    (id, draft_id, assistant_run_id, image_job_id)
}

pub(crate) fn static_page_render_output_view_content_fields(
    html: String,
    asset_manifest: Value,
) -> StaticPageRenderOutputViewContentFields {
    (html, asset_manifest)
}

pub(crate) fn static_page_render_output_view_download_alias_fields(
    html_download_url: Option<String>,
) -> StaticPageRenderOutputViewDownloadAliasFields {
    (
        html_download_url.clone(),
        html_download_url.clone(),
        html_download_url.clone(),
        html_download_url,
    )
}

pub(crate) fn static_page_render_output_view_preview_alias_fields(
    html_preview_url: Option<String>,
) -> StaticPageRenderOutputViewPreviewAliasFields {
    (html_preview_url.clone(), html_preview_url)
}

pub(crate) fn static_page_render_output_view_retryable_error_reason_alias_fields(
    retryable_error_reason: Option<String>,
) -> StaticPageRenderOutputViewRetryableErrorReasonAliasFields {
    (retryable_error_reason.clone(), retryable_error_reason)
}

pub(crate) fn static_page_render_output_view_timestamp_fields(
    created_at: DateTime<Utc>,
) -> DateTime<Utc> {
    created_at
}

pub(crate) fn static_page_render_output_view_status(
    status: StaticPageRenderOutputStatus,
) -> StaticPageRenderOutputStatusView {
    StaticPageRenderOutputStatusView::from_domain(status)
}

pub(crate) fn static_page_render_output_view_link_fields(
    output: &StaticPageRenderOutput,
    selected_scope: Option<&Value>,
) -> StaticPageRenderOutputViewLinkFields {
    (
        static_page_html_download_url(selected_scope, output),
        static_page_html_preview_url(selected_scope, output),
        static_page_render_output_retryable_error_reason(output),
    )
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
    fn static_page_draft_view_fields_preserve_values() {
        let draft = test_draft();
        let draft_id = draft.id;
        let assistant_run_id = draft.assistant_run_id;
        let created_at = draft.created_at;
        let updated_at = draft.updated_at;
        let (
            id,
            returned_run_id,
            title,
            status,
            selected_scope,
            visibility_snapshot,
            source_refs,
            draft_payload,
            returned_created_at,
            returned_updated_at,
        ) = static_page_draft_view_fields(draft);

        assert_eq!(id, draft_id);
        assert_eq!(returned_run_id, assistant_run_id);
        assert_eq!(title, "经营月报");
        assert_eq!(status, StaticPageDraftStatusView::Rendered);
        assert_eq!(
            selected_scope,
            json!({"type": "external_channel", "channelConnectionId": "main"})
        );
        assert_eq!(visibility_snapshot, json!({"visible": true}));
        assert_eq!(source_refs["dataset_artifact_key"], json!("dataset-key-1"));
        assert_eq!(draft_payload["finalPage"]["publicUrl"], json!(PUBLIC_URL));
        assert_eq!(returned_created_at, created_at);
        assert_eq!(returned_updated_at, updated_at);
    }

    #[test]
    fn static_page_draft_view_from_fields_preserves_values() {
        let draft_id = StaticPageDraftId::new();
        let assistant_run_id = AssistantRunId::new();
        let created_at = Utc::now();
        let updated_at = Utc::now();
        let view = static_page_draft_view_from_fields(
            draft_id,
            assistant_run_id,
            "经营月报".to_string(),
            StaticPageDraftStatusView::Rendered,
            json!({"datasets": ["dataset-a"]}),
            json!({"visible": true}),
            json!([{"type": "document"}]),
            json!({"modules": []}),
            created_at,
            updated_at,
        );

        assert_eq!(view.id, draft_id);
        assert_eq!(view.assistant_run_id, assistant_run_id);
        assert_eq!(view.title, "经营月报");
        assert_eq!(view.status, StaticPageDraftStatusView::Rendered);
        assert_eq!(view.selected_scope, json!({"datasets": ["dataset-a"]}));
        assert_eq!(view.visibility_snapshot, json!({"visible": true}));
        assert_eq!(view.source_refs, json!([{"type": "document"}]));
        assert_eq!(view.draft_payload, json!({"modules": []}));
        assert_eq!(view.created_at, created_at);
        assert_eq!(view.updated_at, updated_at);
    }

    #[test]
    fn static_page_draft_view_identity_fields_preserve_values() {
        let draft = test_draft();
        let draft_id = draft.id;
        let assistant_run_id = draft.assistant_run_id;
        let fields: StaticPageDraftViewIdentityFields =
            static_page_draft_view_identity_fields(draft.id, draft.assistant_run_id, draft.title);
        let (id, run_id, title) = fields;

        assert_eq!(id, draft_id);
        assert_eq!(run_id, assistant_run_id);
        assert_eq!(title, "经营月报");
    }

    #[test]
    fn static_page_draft_view_payload_fields_preserve_values() {
        let draft = test_draft();
        let fields: StaticPageDraftViewPayloadFields = static_page_draft_view_payload_fields(
            draft.selected_scope,
            draft.visibility_snapshot,
            draft.source_refs,
            draft.draft_payload,
        );
        let (selected_scope, visibility_snapshot, source_refs, draft_payload) = fields;

        assert_eq!(
            selected_scope,
            json!({"type": "external_channel", "channelConnectionId": "main"})
        );
        assert_eq!(visibility_snapshot, json!({"visible": true}));
        assert_eq!(source_refs["dataset_artifact_key"], json!("dataset-key-1"));
        assert_eq!(draft_payload["finalPage"]["publicUrl"], json!(PUBLIC_URL));
    }

    #[test]
    fn static_page_draft_view_timestamp_fields_preserve_values() {
        let draft = test_draft();
        let expected_created_at = draft.created_at;
        let expected_updated_at = draft.updated_at;
        let fields: StaticPageDraftViewTimestampFields =
            static_page_draft_view_timestamp_fields(draft.created_at, draft.updated_at);
        let (created_at, updated_at) = fields;

        assert_eq!(created_at, expected_created_at);
        assert_eq!(updated_at, expected_updated_at);
    }

    #[test]
    fn static_page_draft_view_status_preserves_archived_status() {
        assert_eq!(
            static_page_draft_view_status(StaticPageDraftStatus::Archived),
            StaticPageDraftStatusView::Archived
        );
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
    fn static_page_template_view_fields_preserve_values() {
        let draft = test_draft();
        let draft_id = draft.id;
        let assistant_run_id = draft.assistant_run_id;
        let created_at = draft.created_at;
        let updated_at = draft.updated_at;
        let (
            id,
            returned_draft_id,
            returned_run_id,
            title,
            public_url,
            preview_url,
            dataset_artifact_key,
            template_reference,
            selected_scope,
            source_refs,
            returned_created_at,
            returned_updated_at,
        ) = static_page_template_view_fields(draft).expect("published draft has template fields");

        assert_eq!(id, format!("generated-static-page:{draft_id}"));
        assert_eq!(returned_draft_id, draft_id);
        assert_eq!(returned_run_id, assistant_run_id);
        assert_eq!(title, "经营月报");
        assert_eq!(public_url, PUBLIC_URL);
        assert_eq!(preview_url.as_deref(), Some(PREVIEW_URL));
        assert_eq!(dataset_artifact_key.as_deref(), Some("dataset-key-1"));
        assert_eq!(template_reference["publicUrl"], json!(PUBLIC_URL));
        assert_eq!(
            selected_scope,
            json!({"type": "external_channel", "channelConnectionId": "main"})
        );
        assert_eq!(source_refs["dataset_artifact_key"], json!("dataset-key-1"));
        assert_eq!(returned_created_at, created_at);
        assert_eq!(returned_updated_at, updated_at);

        let mut unpublished = test_draft();
        unpublished.draft_payload = json!({});
        assert!(static_page_template_view_fields(unpublished).is_none());
    }

    #[test]
    fn static_page_template_view_from_fields_preserves_values() {
        let draft_id = StaticPageDraftId::new();
        let assistant_run_id = AssistantRunId::new();
        let created_at = Utc::now();
        let updated_at = Utc::now();
        let template_reference = json!({"id": "template-ref"});
        let view = static_page_template_view_from_fields(
            "template-id".to_string(),
            draft_id,
            assistant_run_id,
            "经营月报".to_string(),
            PUBLIC_URL.to_string(),
            Some(PREVIEW_URL.to_string()),
            Some("dataset-key-1".to_string()),
            template_reference.clone(),
            json!({"datasets": ["dataset-a"]}),
            json!([{"type": "document"}]),
            created_at,
            updated_at,
        );

        assert_eq!(view.id, "template-id");
        assert_eq!(view.draft_id, draft_id);
        assert_eq!(view.assistant_run_id, assistant_run_id);
        assert_eq!(view.title, "经营月报");
        assert_eq!(view.public_url, PUBLIC_URL);
        assert_eq!(view.preview_url.as_deref(), Some(PREVIEW_URL));
        assert_eq!(view.dataset_artifact_key.as_deref(), Some("dataset-key-1"));
        assert_eq!(view.template_reference, template_reference);
        assert_eq!(view.selected_scope, json!({"datasets": ["dataset-a"]}));
        assert_eq!(view.source_refs, json!([{"type": "document"}]));
        assert_eq!(view.created_at, created_at);
        assert_eq!(view.updated_at, updated_at);
    }

    #[test]
    fn static_page_template_view_identity_fields_preserve_values() {
        let draft = test_draft();
        let draft_id = draft.id;
        let assistant_run_id = draft.assistant_run_id;
        let fields: StaticPageTemplateViewIdentityFields =
            static_page_template_view_identity_fields(
                draft.id,
                draft.assistant_run_id,
                draft.title,
            );
        let (id, returned_draft_id, returned_run_id, title) = fields;

        assert_eq!(id, format!("generated-static-page:{draft_id}"));
        assert_eq!(returned_draft_id, draft_id);
        assert_eq!(returned_run_id, assistant_run_id);
        assert_eq!(title, "经营月报");
    }

    #[test]
    fn static_page_template_view_context_fields_preserve_values() {
        let draft = test_draft();
        let fields: StaticPageTemplateViewContextFields =
            static_page_template_view_context_fields(draft.selected_scope, draft.source_refs);
        let (selected_scope, source_refs) = fields;

        assert_eq!(
            selected_scope,
            json!({"type": "external_channel", "channelConnectionId": "main"})
        );
        assert_eq!(source_refs["dataset_artifact_key"], json!("dataset-key-1"));
        assert_eq!(source_refs["image2"]["preview_url"], json!(PREVIEW_URL));
    }

    #[test]
    fn static_page_template_view_timestamp_fields_preserve_values() {
        let draft = test_draft();
        let expected_created_at = draft.created_at;
        let expected_updated_at = draft.updated_at;
        let fields: StaticPageTemplateViewTimestampFields =
            static_page_template_view_timestamp_fields(draft.created_at, draft.updated_at);
        let (created_at, updated_at) = fields;

        assert_eq!(created_at, expected_created_at);
        assert_eq!(updated_at, expected_updated_at);
    }

    #[test]
    fn static_page_template_view_artifact_fields_preserve_reference_payload() {
        let draft = test_draft();
        let (public_url, preview_url, dataset_artifact_key, template_reference) =
            static_page_template_view_artifact_fields(&draft)
                .expect("published draft has artifact fields");

        assert_eq!(public_url, PUBLIC_URL);
        assert_eq!(preview_url.as_deref(), Some(PREVIEW_URL));
        assert_eq!(dataset_artifact_key.as_deref(), Some("dataset-key-1"));
        assert_eq!(template_reference["publicUrl"], json!(PUBLIC_URL));
        assert_eq!(template_reference["previewUrl"], json!(PREVIEW_URL));
        assert_eq!(
            template_reference["datasetArtifactKey"],
            json!("dataset-key-1")
        );

        let mut unpublished = test_draft();
        unpublished.draft_payload = json!({});
        assert!(static_page_template_view_artifact_fields(&unpublished).is_none());
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
    fn static_page_image_job_view_fields_preserve_values() {
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
        let job_id = job.id;
        let draft_id = job.draft_id;
        let assistant_run_id = job.assistant_run_id;
        let (
            returned_job_id,
            returned_draft_id,
            returned_run_id,
            status,
            queue_position,
            image_prompt_payload,
            preview_asset_key,
            failure_reason,
            confirmed_at,
            created_at,
            updated_at,
        ) = static_page_image_job_view_fields(job);

        assert_eq!(returned_job_id, job_id);
        assert_eq!(returned_draft_id, draft_id);
        assert_eq!(returned_run_id, assistant_run_id);
        assert_eq!(status, StaticPageImageJobStatusView::Failed);
        assert_eq!(queue_position, Some(3));
        assert_eq!(
            image_prompt_payload,
            json!({"prompt": "dark mobile report"})
        );
        assert_eq!(preview_asset_key.as_deref(), Some("preview.png"));
        assert_eq!(failure_reason.as_deref(), Some("image timeout"));
        assert_eq!(confirmed_at, Some(now));
        assert_eq!(created_at, now);
        assert_eq!(updated_at, now);
    }

    #[test]
    fn static_page_image_job_view_from_fields_preserves_values() {
        let now = Utc::now();
        let job_id = StaticPageImageJobId::new();
        let draft_id = StaticPageDraftId::new();
        let assistant_run_id = AssistantRunId::new();
        let view = static_page_image_job_view_from_fields(
            job_id,
            draft_id,
            assistant_run_id,
            StaticPageImageJobStatusView::Failed,
            Some(3),
            json!({"prompt": "dark mobile report"}),
            Some("preview.png".to_string()),
            Some("image timeout".to_string()),
            Some(now),
            now,
            now,
        );

        assert_eq!(view.id, job_id);
        assert_eq!(view.draft_id, draft_id);
        assert_eq!(view.assistant_run_id, assistant_run_id);
        assert_eq!(view.status, StaticPageImageJobStatusView::Failed);
        assert_eq!(view.queue_position, Some(3));
        assert_eq!(
            view.image_prompt_payload,
            json!({"prompt": "dark mobile report"})
        );
        assert_eq!(view.preview_asset_key.as_deref(), Some("preview.png"));
        assert_eq!(view.failure_reason.as_deref(), Some("image timeout"));
        assert_eq!(view.confirmed_at, Some(now));
        assert_eq!(view.created_at, now);
        assert_eq!(view.updated_at, now);
    }

    #[test]
    fn static_page_image_job_view_identity_fields_preserve_values() {
        let job_id = StaticPageImageJobId::new();
        let draft_id = StaticPageDraftId::new();
        let assistant_run_id = AssistantRunId::new();
        let fields: StaticPageImageJobViewIdentityFields =
            static_page_image_job_view_identity_fields(job_id, draft_id, assistant_run_id);
        let (returned_job_id, returned_draft_id, returned_run_id) = fields;

        assert_eq!(returned_job_id, job_id);
        assert_eq!(returned_draft_id, draft_id);
        assert_eq!(returned_run_id, assistant_run_id);
    }

    #[test]
    fn static_page_image_job_view_detail_fields_preserve_values() {
        let now = Utc::now();
        let fields: StaticPageImageJobViewDetailFields = static_page_image_job_view_detail_fields(
            Some(3),
            json!({"prompt": "dark mobile report"}),
            Some("preview.png".to_string()),
            Some("image timeout".to_string()),
            Some(now),
        );
        let (queue_position, image_prompt_payload, preview_asset_key, failure_reason, confirmed_at) =
            fields;

        assert_eq!(queue_position, Some(3));
        assert_eq!(
            image_prompt_payload,
            json!({"prompt": "dark mobile report"})
        );
        assert_eq!(preview_asset_key.as_deref(), Some("preview.png"));
        assert_eq!(failure_reason.as_deref(), Some("image timeout"));
        assert_eq!(confirmed_at, Some(now));
    }

    #[test]
    fn static_page_image_job_view_timestamp_fields_preserve_values() {
        let now = Utc::now();
        let updated_at = now + chrono::Duration::seconds(5);
        let fields: StaticPageImageJobViewTimestampFields =
            static_page_image_job_view_timestamp_fields(now, updated_at);
        let (created_at, returned_updated_at) = fields;

        assert_eq!(created_at, now);
        assert_eq!(returned_updated_at, updated_at);
    }

    #[test]
    fn static_page_image_job_view_status_preserves_failed_status() {
        let now = Utc::now();
        let job = StaticPageImageJob {
            id: StaticPageImageJobId::new(),
            tenant_id: TenantId::new(),
            draft_id: StaticPageDraftId::new(),
            assistant_run_id: AssistantRunId::new(),
            status: StaticPageImageJobStatus::Failed,
            queue_position: None,
            image_prompt_payload: json!({}),
            preview_asset_key: None,
            failure_reason: Some("image timeout".to_string()),
            confirmed_at: None,
            created_at: now,
            updated_at: now,
        };

        assert_eq!(
            static_page_image_job_view_status(job.status),
            StaticPageImageJobStatusView::Failed
        );
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

    #[test]
    fn static_page_render_output_view_fields_preserve_values() {
        let output_id = StaticPageRenderOutputId::new();
        let draft_id = StaticPageDraftId::new();
        let assistant_run_id = AssistantRunId::new();
        let image_job_id = Some(StaticPageImageJobId::new());
        let created_at = Utc::now();
        let output = StaticPageRenderOutput {
            id: output_id,
            tenant_id: TenantId::new(),
            draft_id,
            assistant_run_id,
            owner_user_id: None,
            image_job_id,
            status: StaticPageRenderOutputStatus::Rendered,
            html: "<html>ok</html>".to_string(),
            asset_manifest: json!({"files": ["index.html"]}),
            created_at,
        };
        let (
            returned_output_id,
            returned_draft_id,
            returned_run_id,
            returned_image_job_id,
            status,
            html,
            html_download_url,
            html_download_url_camel,
            download_url,
            download_url_camel,
            html_preview_url,
            html_preview_url_camel,
            retryable_error_reason,
            retryable_error_reason_camel,
            asset_manifest,
            returned_created_at,
        ) = static_page_render_output_view_fields(
            output,
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
        assert_eq!(returned_output_id, output_id);
        assert_eq!(returned_draft_id, draft_id);
        assert_eq!(returned_run_id, assistant_run_id);
        assert_eq!(returned_image_job_id, image_job_id);
        assert_eq!(status, StaticPageRenderOutputStatusView::Rendered);
        assert_eq!(html, "<html>ok</html>");
        assert_eq!(
            html_download_url.as_deref(),
            Some(expected_download.as_str())
        );
        assert_eq!(
            html_download_url_camel.as_deref(),
            Some(expected_download.as_str())
        );
        assert_eq!(download_url.as_deref(), Some(expected_download.as_str()));
        assert_eq!(
            download_url_camel.as_deref(),
            Some(expected_download.as_str())
        );
        assert_eq!(html_preview_url.as_deref(), Some(expected_preview.as_str()));
        assert_eq!(
            html_preview_url_camel.as_deref(),
            Some(expected_preview.as_str())
        );
        assert_eq!(retryable_error_reason, None);
        assert_eq!(retryable_error_reason_camel, None);
        assert_eq!(asset_manifest, json!({"files": ["index.html"]}));
        assert_eq!(returned_created_at, created_at);
    }

    #[test]
    fn static_page_view_field_type_aliases_preserve_shapes() {
        let draft_fields: StaticPageDraftViewFields = static_page_draft_view_fields(test_draft());
        let template_fields: StaticPageTemplateViewFields =
            static_page_template_view_fields(test_draft())
                .expect("published draft has template fields");

        let now = Utc::now();
        let draft = test_draft();
        let image_job = StaticPageImageJob {
            id: StaticPageImageJobId::new(),
            tenant_id: TenantId::new(),
            draft_id: draft.id,
            assistant_run_id: draft.assistant_run_id,
            status: StaticPageImageJobStatus::PreviewReady,
            queue_position: Some(1),
            image_prompt_payload: json!({"prompt": "report"}),
            preview_asset_key: Some("preview.png".to_string()),
            failure_reason: None,
            confirmed_at: Some(now),
            created_at: now,
            updated_at: now,
        };
        let image_fields: StaticPageImageJobViewFields =
            static_page_image_job_view_fields(image_job);

        let output = StaticPageRenderOutput {
            id: StaticPageRenderOutputId::new(),
            tenant_id: TenantId::new(),
            draft_id: draft.id,
            assistant_run_id: draft.assistant_run_id,
            owner_user_id: None,
            image_job_id: Some(StaticPageImageJobId::new()),
            status: StaticPageRenderOutputStatus::Rendered,
            html: "<html>ok</html>".to_string(),
            asset_manifest: json!({"files": ["index.html"]}),
            created_at: now,
        };
        let render_fields: StaticPageRenderOutputViewFields =
            static_page_render_output_view_fields(output, None);

        assert_eq!(draft_fields.2, "经营月报");
        assert_eq!(template_fields.3, "经营月报");
        assert_eq!(image_fields.3, StaticPageImageJobStatusView::PreviewReady);
        assert_eq!(render_fields.4, StaticPageRenderOutputStatusView::Rendered);
        assert_eq!(render_fields.5, "<html>ok</html>");
    }

    #[test]
    fn static_page_render_output_view_from_fields_preserves_values() {
        let output_id = StaticPageRenderOutputId::new();
        let draft_id = StaticPageDraftId::new();
        let assistant_run_id = AssistantRunId::new();
        let image_job_id = Some(StaticPageImageJobId::new());
        let created_at = Utc::now();
        let asset_manifest = json!({"files": ["index.html"]});
        let view = static_page_render_output_view_from_fields(
            output_id,
            draft_id,
            assistant_run_id,
            image_job_id,
            StaticPageRenderOutputStatusView::Rendered,
            "<html>ok</html>".to_string(),
            Some("/download/index.html".to_string()),
            Some("/download/index.html".to_string()),
            Some("/download/index.html".to_string()),
            Some("/download/index.html".to_string()),
            Some("/preview/index.html".to_string()),
            Some("/preview/index.html".to_string()),
            Some("retry later".to_string()),
            Some("retry later".to_string()),
            asset_manifest.clone(),
            created_at,
        );

        assert_eq!(view.id, output_id);
        assert_eq!(view.draft_id, draft_id);
        assert_eq!(view.assistant_run_id, assistant_run_id);
        assert_eq!(view.image_job_id, image_job_id);
        assert_eq!(view.status, StaticPageRenderOutputStatusView::Rendered);
        assert_eq!(view.html, "<html>ok</html>");
        assert_eq!(
            view.html_download_url.as_deref(),
            Some("/download/index.html")
        );
        assert_eq!(
            view.html_download_url_camel.as_deref(),
            Some("/download/index.html")
        );
        assert_eq!(view.download_url.as_deref(), Some("/download/index.html"));
        assert_eq!(
            view.download_url_camel.as_deref(),
            Some("/download/index.html")
        );
        assert_eq!(
            view.html_preview_url.as_deref(),
            Some("/preview/index.html")
        );
        assert_eq!(
            view.html_preview_url_camel.as_deref(),
            Some("/preview/index.html")
        );
        assert_eq!(view.retryable_error_reason.as_deref(), Some("retry later"));
        assert_eq!(
            view.retryable_error_reason_camel.as_deref(),
            Some("retry later")
        );
        assert_eq!(view.asset_manifest, asset_manifest);
        assert_eq!(view.created_at, created_at);
    }

    #[test]
    fn static_page_render_output_view_identity_fields_preserve_values() {
        let output_id = StaticPageRenderOutputId::new();
        let draft_id = StaticPageDraftId::new();
        let assistant_run_id = AssistantRunId::new();
        let image_job_id = Some(StaticPageImageJobId::new());
        let fields: StaticPageRenderOutputViewIdentityFields =
            static_page_render_output_view_identity_fields(
                output_id,
                draft_id,
                assistant_run_id,
                image_job_id,
            );
        let (returned_output_id, returned_draft_id, returned_run_id, returned_image_job_id) =
            fields;

        assert_eq!(returned_output_id, output_id);
        assert_eq!(returned_draft_id, draft_id);
        assert_eq!(returned_run_id, assistant_run_id);
        assert_eq!(returned_image_job_id, image_job_id);
    }

    #[test]
    fn static_page_render_output_view_content_fields_preserve_values() {
        let fields: StaticPageRenderOutputViewContentFields =
            static_page_render_output_view_content_fields(
                "<html>ok</html>".to_string(),
                json!({"files": ["index.html"]}),
            );
        let (html, asset_manifest) = fields;

        assert_eq!(html, "<html>ok</html>");
        assert_eq!(asset_manifest, json!({"files": ["index.html"]}));
    }

    #[test]
    fn static_page_render_output_view_download_alias_fields_preserve_values() {
        let fields: StaticPageRenderOutputViewDownloadAliasFields =
            static_page_render_output_view_download_alias_fields(Some(
                "/download/index.html".to_string(),
            ));
        let (html_download_url, html_download_url_camel, download_url, download_url_camel) = fields;

        assert_eq!(html_download_url.as_deref(), Some("/download/index.html"));
        assert_eq!(
            html_download_url_camel.as_deref(),
            Some("/download/index.html")
        );
        assert_eq!(download_url.as_deref(), Some("/download/index.html"));
        assert_eq!(download_url_camel.as_deref(), Some("/download/index.html"));

        let fields: StaticPageRenderOutputViewDownloadAliasFields =
            static_page_render_output_view_download_alias_fields(None);
        let (html_download_url, html_download_url_camel, download_url, download_url_camel) = fields;
        assert_eq!(html_download_url, None);
        assert_eq!(html_download_url_camel, None);
        assert_eq!(download_url, None);
        assert_eq!(download_url_camel, None);
    }

    #[test]
    fn static_page_render_output_view_preview_alias_fields_preserve_values() {
        let fields: StaticPageRenderOutputViewPreviewAliasFields =
            static_page_render_output_view_preview_alias_fields(Some(
                "/preview/index.html".to_string(),
            ));
        let (html_preview_url, html_preview_url_camel) = fields;

        assert_eq!(html_preview_url.as_deref(), Some("/preview/index.html"));
        assert_eq!(
            html_preview_url_camel.as_deref(),
            Some("/preview/index.html")
        );

        let fields: StaticPageRenderOutputViewPreviewAliasFields =
            static_page_render_output_view_preview_alias_fields(None);
        let (html_preview_url, html_preview_url_camel) = fields;
        assert_eq!(html_preview_url, None);
        assert_eq!(html_preview_url_camel, None);
    }

    #[test]
    fn static_page_render_output_view_retryable_error_reason_alias_fields_preserve_values() {
        let fields: StaticPageRenderOutputViewRetryableErrorReasonAliasFields =
            static_page_render_output_view_retryable_error_reason_alias_fields(Some(
                "retry later".to_string(),
            ));
        let (retryable_error_reason, retryable_error_reason_camel) = fields;

        assert_eq!(retryable_error_reason.as_deref(), Some("retry later"));
        assert_eq!(retryable_error_reason_camel.as_deref(), Some("retry later"));

        let fields: StaticPageRenderOutputViewRetryableErrorReasonAliasFields =
            static_page_render_output_view_retryable_error_reason_alias_fields(None);
        let (retryable_error_reason, retryable_error_reason_camel) = fields;
        assert_eq!(retryable_error_reason, None);
        assert_eq!(retryable_error_reason_camel, None);
    }

    #[test]
    fn static_page_render_output_view_timestamp_fields_preserve_value() {
        let created_at = Utc::now();

        assert_eq!(
            static_page_render_output_view_timestamp_fields(created_at),
            created_at
        );
    }

    #[test]
    fn static_page_render_output_view_status_preserves_cancelled_status() {
        assert_eq!(
            static_page_render_output_view_status(StaticPageRenderOutputStatus::Cancelled),
            StaticPageRenderOutputStatusView::Cancelled
        );
    }

    #[test]
    fn static_page_render_output_view_link_fields_preserve_urls_and_retry_reason() {
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

        let fields: StaticPageRenderOutputViewLinkFields =
            static_page_render_output_view_link_fields(
                &rendered,
                Some(&json!({
                    "type": "external_channel",
                    "channelConnectionId": "generic chat/主通道"
                })),
            );
        let (download_url, preview_url, retryable_error_reason) = fields;

        assert_eq!(
            download_url.as_deref(),
            Some(
                format!(
                    "/v1/external/channels/generic%20chat%2F%E4%B8%BB%E9%80%9A%E9%81%93/static-page-renders/{output_id}/download"
                )
                .as_str()
            )
        );
        assert_eq!(
            preview_url.as_deref(),
            Some(
                format!(
                    "/v1/external/channels/generic%20chat%2F%E4%B8%BB%E9%80%9A%E9%81%93/static-page-renders/{output_id}/preview"
                )
                .as_str()
            )
        );
        assert_eq!(retryable_error_reason, None);

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
        let fields: StaticPageRenderOutputViewLinkFields =
            static_page_render_output_view_link_fields(&failed, None);
        let (download_url, preview_url, retryable_error_reason) = fields;

        assert_eq!(download_url, None);
        assert_eq!(preview_url, None);
        assert_eq!(retryable_error_reason.as_deref(), Some("render timeout"));
    }
}
