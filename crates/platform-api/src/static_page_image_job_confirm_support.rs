use chrono::{DateTime, Utc};
use domain_model::{
    StaticPageDraft, StaticPageDraftStatus, StaticPageImageJob, StaticPageImageJobStatus,
};
use serde_json::{json, Value};

use crate::static_page_operation_apply_support::{
    append_static_page_operations_metadata, apply_static_page_operations_to_payload,
};

const STATIC_PAGE_CONFIRM_PREVIEW_SUMMARY: &str = "可视化已确认，可以进入最终静态页渲染。";

pub(crate) fn static_page_confirm_preview_asset_key(
    requested_preview_asset_key: Option<&str>,
    job: &StaticPageImageJob,
) -> String {
    requested_preview_asset_key
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| job.preview_asset_key.clone())
        .unwrap_or_else(|| format!("static-page-previews/{}.json", job.id))
}

pub(crate) fn static_page_confirm_preview_operations(
    job: &StaticPageImageJob,
    preview_asset_key: &str,
) -> Vec<Value> {
    vec![json!({
        "type": "confirm_preview",
        "previewImage": {
            "kind": "static-page-effect-preview",
            "assetKey": preview_asset_key,
            "imageJobId": job.id,
        }
    })]
}

pub(crate) fn apply_static_page_confirm_preview_to_draft(
    mut draft: StaticPageDraft,
    job: &StaticPageImageJob,
    preview_asset_key: &str,
) -> StaticPageDraft {
    let operations = static_page_confirm_preview_operations(job, preview_asset_key);
    draft.draft_payload = apply_static_page_operations_to_payload(
        draft.draft_payload,
        &operations,
        Some(STATIC_PAGE_CONFIRM_PREVIEW_SUMMARY),
    );
    append_static_page_operations_metadata(
        &mut draft.draft_payload,
        &operations,
        None,
        STATIC_PAGE_CONFIRM_PREVIEW_SUMMARY,
    );
    draft.status = StaticPageDraftStatus::Confirmed;
    draft
}

pub(crate) fn apply_static_page_confirm_preview_to_job(
    mut job: StaticPageImageJob,
    preview_asset_key: String,
    confirmed_at: DateTime<Utc>,
) -> StaticPageImageJob {
    job.status = StaticPageImageJobStatus::Confirmed;
    job.queue_position = None;
    job.preview_asset_key = Some(preview_asset_key);
    job.failure_reason = None;
    job.confirmed_at = Some(confirmed_at);
    job
}

pub(crate) fn static_page_image_job_confirmed_event_payload(
    draft: &StaticPageDraft,
    job: &StaticPageImageJob,
) -> Value {
    json!({
        "draft_id": draft.id,
        "image_job_id": job.id,
        "preview_asset_key": job.preview_asset_key,
    })
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use domain_model::{
        AssistantRunId, StaticPageDraftId, StaticPageDraftStatus, StaticPageImageJobId,
        StaticPageImageJobStatus, TenantId,
    };
    use serde_json::json;

    use super::*;

    fn preview_job(preview_asset_key: Option<&str>) -> StaticPageImageJob {
        let now = Utc::now();
        StaticPageImageJob {
            id: StaticPageImageJobId::new(),
            tenant_id: TenantId::new(),
            draft_id: StaticPageDraftId::new(),
            assistant_run_id: AssistantRunId::new(),
            status: StaticPageImageJobStatus::PreviewReady,
            queue_position: Some(1),
            image_prompt_payload: json!({}),
            preview_asset_key: preview_asset_key.map(ToOwned::to_owned),
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
            status: StaticPageDraftStatus::Confirmed,
            selected_scope: Value::Null,
            visibility_snapshot: Value::Null,
            source_refs: Value::Null,
            draft_payload: Value::Null,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn confirm_preview_asset_key_prefers_trimmed_request() {
        let job = preview_job(Some("static-page-previews/existing.png"));

        assert_eq!(
            static_page_confirm_preview_asset_key(
                Some("  static-page-previews/requested.png  "),
                &job
            ),
            "static-page-previews/requested.png"
        );
    }

    #[test]
    fn confirm_preview_asset_key_falls_back_to_existing_job_key() {
        let job = preview_job(Some("static-page-previews/existing.png"));

        assert_eq!(
            static_page_confirm_preview_asset_key(Some("   "), &job),
            "static-page-previews/existing.png"
        );
    }

    #[test]
    fn confirm_preview_asset_key_generates_default_key_when_missing() {
        let job = preview_job(None);

        assert_eq!(
            static_page_confirm_preview_asset_key(None, &job),
            format!("static-page-previews/{}.json", job.id)
        );
    }

    #[test]
    fn confirm_preview_operations_reference_job_and_asset() {
        let job = preview_job(None);
        let operations =
            static_page_confirm_preview_operations(&job, "static-page-previews/confirmed.png");

        assert_eq!(operations.len(), 1);
        assert_eq!(operations[0]["type"], json!("confirm_preview"));
        assert_eq!(
            operations[0]["previewImage"]["kind"],
            json!("static-page-effect-preview")
        );
        assert_eq!(
            operations[0]["previewImage"]["assetKey"],
            json!("static-page-previews/confirmed.png")
        );
        assert_eq!(operations[0]["previewImage"]["imageJobId"], json!(job.id));
    }

    #[test]
    fn confirm_preview_to_draft_applies_payload_metadata_and_confirmed_status() {
        let mut draft = draft();
        draft.status = StaticPageDraftStatus::Previewed;
        let job = preview_job(Some("static-page-previews/confirmed.png"));

        let draft = apply_static_page_confirm_preview_to_draft(
            draft,
            &job,
            "static-page-previews/confirmed.png",
        );

        assert_eq!(draft.status, StaticPageDraftStatus::Confirmed);
        assert_eq!(draft.draft_payload["status"], json!("effect_confirmed"));
        assert_eq!(
            draft.draft_payload["previewImage"]["kind"],
            json!("static-page-effect-preview")
        );
        assert_eq!(
            draft.draft_payload["previewImage"]["assetKey"],
            json!("static-page-previews/confirmed.png")
        );
        assert_eq!(
            draft.draft_payload["previewImage"]["imageJobId"],
            json!(job.id)
        );
        assert_eq!(
            draft.draft_payload["previewContract"]["status"],
            json!("confirmed")
        );
        assert_eq!(
            draft.draft_payload["previewContract"]["assetKey"],
            json!("static-page-previews/confirmed.png")
        );
        assert_eq!(
            draft.draft_payload["lastOperationSummary"],
            json!(STATIC_PAGE_CONFIRM_PREVIEW_SUMMARY)
        );
        assert_eq!(
            draft.draft_payload["operations"][0]["type"],
            json!("confirm_preview")
        );
    }

    #[test]
    fn confirm_preview_to_job_sets_confirmed_state_and_clears_queue_failure() {
        let mut job = preview_job(Some("static-page-previews/old.png"));
        job.failure_reason = Some("previous transient failure".to_string());
        let confirmed_at = Utc::now();

        let job = apply_static_page_confirm_preview_to_job(
            job,
            "static-page-previews/confirmed.png".to_string(),
            confirmed_at,
        );

        assert_eq!(job.status, StaticPageImageJobStatus::Confirmed);
        assert_eq!(job.queue_position, None);
        assert_eq!(
            job.preview_asset_key.as_deref(),
            Some("static-page-previews/confirmed.png")
        );
        assert_eq!(job.failure_reason, None);
        assert_eq!(job.confirmed_at, Some(confirmed_at));
    }

    #[test]
    fn confirmed_event_payload_preserves_draft_job_and_preview_asset() {
        let draft = draft();
        let job = preview_job(Some("static-page-previews/confirmed.png"));

        let payload = static_page_image_job_confirmed_event_payload(&draft, &job);

        assert_eq!(payload["draft_id"], json!(draft.id));
        assert_eq!(payload["image_job_id"], json!(job.id));
        assert_eq!(
            payload["preview_asset_key"],
            json!("static-page-previews/confirmed.png")
        );
    }

    #[test]
    fn confirmed_event_payload_allows_missing_preview_asset() {
        let draft = draft();
        let job = preview_job(None);

        let payload = static_page_image_job_confirmed_event_payload(&draft, &job);

        assert_eq!(payload["draft_id"], json!(draft.id));
        assert_eq!(payload["image_job_id"], json!(job.id));
        assert_eq!(payload["preview_asset_key"], Value::Null);
    }
}
