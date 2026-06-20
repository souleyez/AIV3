use chrono::{DateTime, Utc};
use domain_model::{
    StaticPageDraft, StaticPageImageJob, StaticPageImageJobId, StaticPageRenderOutputStatus,
};
use serde_json::Value;
use storage::NewStaticPageRenderOutput;

use crate::static_page_render_queue_manifest_support::build_static_page_render_queue_manifest;

pub(crate) fn new_queued_static_page_render_output(
    draft: &StaticPageDraft,
    image_job: Option<&StaticPageImageJob>,
    created_at: DateTime<Utc>,
) -> NewStaticPageRenderOutput {
    new_static_page_render_output(
        draft,
        image_job,
        StaticPageRenderOutputStatus::Queued,
        String::new(),
        build_static_page_render_queue_manifest(draft, image_job, None, None),
        created_at,
    )
}

pub(crate) fn new_rendered_static_page_render_output(
    draft: &StaticPageDraft,
    image_job: Option<&StaticPageImageJob>,
    html: String,
    asset_manifest: Value,
    created_at: DateTime<Utc>,
) -> NewStaticPageRenderOutput {
    new_static_page_render_output(
        draft,
        image_job,
        StaticPageRenderOutputStatus::Rendered,
        html,
        asset_manifest,
        created_at,
    )
}

pub(crate) fn new_static_page_render_output(
    draft: &StaticPageDraft,
    image_job: Option<&StaticPageImageJob>,
    status: StaticPageRenderOutputStatus,
    html: String,
    asset_manifest: Value,
    created_at: DateTime<Utc>,
) -> NewStaticPageRenderOutput {
    NewStaticPageRenderOutput {
        draft_id: draft.id,
        assistant_run_id: draft.assistant_run_id,
        owner_user_id: draft.owner_user_id,
        image_job_id: static_page_render_output_image_job_id(image_job),
        status,
        html,
        asset_manifest,
        created_at,
    }
}

pub(crate) fn static_page_render_output_image_job_id(
    image_job: Option<&StaticPageImageJob>,
) -> Option<StaticPageImageJobId> {
    image_job.map(|job| job.id)
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use domain_model::{
        AssistantRunId, StaticPageDraftId, StaticPageDraftStatus, StaticPageImageJobId,
        StaticPageImageJobStatus, TenantId,
    };
    use serde_json::json;

    use super::*;

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
            draft_payload: json!({
                "modules": [
                    {"id": "hero", "title": "总览"}
                ],
                "dataSnapshot": {
                    "source": "test_snapshot"
                }
            }),
            created_at: now,
            updated_at: now,
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
            preview_asset_key: Some("static-page-previews/preview.json".to_string()),
            failure_reason: None,
            confirmed_at: Some(now),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn queued_render_output_preserves_draft_job_and_queue_manifest() {
        let draft = draft();
        let job = image_job(&draft);
        let created_at = Utc
            .with_ymd_and_hms(2026, 6, 20, 10, 0, 0)
            .single()
            .expect("valid timestamp");

        let output = new_queued_static_page_render_output(&draft, Some(&job), created_at);

        assert_eq!(output.draft_id, draft.id);
        assert_eq!(output.assistant_run_id, draft.assistant_run_id);
        assert_eq!(output.owner_user_id, draft.owner_user_id);
        assert_eq!(output.image_job_id, Some(job.id));
        assert_eq!(output.status, StaticPageRenderOutputStatus::Queued);
        assert_eq!(output.html, "");
        assert_eq!(output.created_at, created_at);
        assert_eq!(output.asset_manifest["status"], json!("queued"));
        assert_eq!(output.asset_manifest["draft_id"], json!(draft.id));
        assert_eq!(output.asset_manifest["image_job_id"], json!(job.id));
        assert_eq!(
            output.asset_manifest["preview_asset_key"],
            json!("static-page-previews/preview.json")
        );
    }

    #[test]
    fn render_output_image_job_id_preserves_optional_job_id() {
        let draft = draft();
        let job = image_job(&draft);

        assert_eq!(
            static_page_render_output_image_job_id(Some(&job)),
            Some(job.id)
        );
        assert_eq!(static_page_render_output_image_job_id(None), None);
    }

    #[test]
    fn render_output_constructor_preserves_common_fields_status_html_and_manifest() {
        let draft = draft();
        let job = image_job(&draft);
        let created_at = Utc
            .with_ymd_and_hms(2026, 6, 20, 10, 3, 0)
            .single()
            .expect("valid timestamp");
        let manifest = json!({
            "renderer": "static-page-renderer-v1",
            "files": ["index.html"]
        });

        let output = new_static_page_render_output(
            &draft,
            Some(&job),
            StaticPageRenderOutputStatus::Rendered,
            "<html>ok</html>".to_string(),
            manifest.clone(),
            created_at,
        );

        assert_eq!(output.draft_id, draft.id);
        assert_eq!(output.assistant_run_id, draft.assistant_run_id);
        assert_eq!(output.owner_user_id, draft.owner_user_id);
        assert_eq!(output.image_job_id, Some(job.id));
        assert_eq!(output.status, StaticPageRenderOutputStatus::Rendered);
        assert_eq!(output.html, "<html>ok</html>");
        assert_eq!(output.asset_manifest, manifest);
        assert_eq!(output.created_at, created_at);
    }

    #[test]
    fn rendered_render_output_preserves_html_manifest_and_optional_job() {
        let draft = draft();
        let created_at = Utc
            .with_ymd_and_hms(2026, 6, 20, 10, 5, 0)
            .single()
            .expect("valid timestamp");
        let manifest = json!({
            "renderer": "static-page-renderer-v1",
            "files": ["index.html"]
        });

        let output = new_rendered_static_page_render_output(
            &draft,
            None,
            "<html>ok</html>".to_string(),
            manifest.clone(),
            created_at,
        );

        assert_eq!(output.draft_id, draft.id);
        assert_eq!(output.assistant_run_id, draft.assistant_run_id);
        assert_eq!(output.owner_user_id, draft.owner_user_id);
        assert_eq!(output.image_job_id, None);
        assert_eq!(output.status, StaticPageRenderOutputStatus::Rendered);
        assert_eq!(output.html, "<html>ok</html>");
        assert_eq!(output.asset_manifest, manifest);
        assert_eq!(output.created_at, created_at);
    }
}
