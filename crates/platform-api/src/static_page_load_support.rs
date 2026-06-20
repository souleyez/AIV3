use domain_model::{
    StaticPageDraft, StaticPageDraftId, StaticPageImageJob, StaticPageImageJobId, UserId,
};

use crate::not_found_errors::{
    static_page_draft_not_found_error, static_page_image_job_not_found_error,
};
use crate::resource_access::static_page_owner_is_visible;
use crate::static_page_render_gate_support::static_page_image_job_ready_for_render;
use crate::{ApiError, AppState};

pub(crate) async fn load_static_page_draft_or_404(
    state: &AppState,
    draft_id: StaticPageDraftId,
) -> std::result::Result<StaticPageDraft, ApiError> {
    state
        .storage
        .static_page_drafts()
        .get_by_id(state.tenant_id, draft_id)
        .await
        .map_err(ApiError::from_storage)?
        .ok_or_else(|| {
            ApiError::not_found(
                "static_page_draft_not_found",
                format!("static page draft {} was not found", draft_id),
            )
        })
}

pub(crate) async fn load_visible_static_page_draft(
    state: &AppState,
    draft_id: StaticPageDraftId,
    current_user_id: Option<UserId>,
) -> std::result::Result<StaticPageDraft, ApiError> {
    let draft = load_static_page_draft_or_404(state, draft_id).await?;
    if static_page_owner_is_visible(draft.owner_user_id, current_user_id) {
        return Ok(draft);
    }
    Err(static_page_draft_not_found_error(draft_id))
}

async fn load_static_page_image_job_or_404(
    state: &AppState,
    job_id: StaticPageImageJobId,
) -> std::result::Result<StaticPageImageJob, ApiError> {
    state
        .storage
        .static_page_image_jobs()
        .get_by_id(state.tenant_id, job_id)
        .await
        .map_err(ApiError::from_storage)?
        .ok_or_else(|| static_page_image_job_not_found_error(job_id))
}

pub(crate) async fn load_visible_static_page_image_job(
    state: &AppState,
    job_id: StaticPageImageJobId,
    current_user_id: Option<UserId>,
) -> std::result::Result<StaticPageImageJob, ApiError> {
    let job = load_static_page_image_job_or_404(state, job_id).await?;
    load_visible_static_page_draft(state, job.draft_id, current_user_id).await?;
    Ok(job)
}

pub(crate) async fn resolve_static_page_render_image_job(
    state: &AppState,
    draft: &StaticPageDraft,
    requested_job_id: Option<StaticPageImageJobId>,
) -> std::result::Result<Option<StaticPageImageJob>, ApiError> {
    let job = if let Some(job_id) = requested_job_id {
        Some(load_static_page_image_job_or_404(state, job_id).await?)
    } else {
        state
            .storage
            .static_page_image_jobs()
            .list_by_draft(state.tenant_id, draft.id)
            .await
            .map_err(ApiError::from_storage)?
            .into_iter()
            .find(|job| static_page_image_job_ready_for_render(draft, job))
    };
    let Some(job) = job else {
        return Err(ApiError::bad_request(
            "static_page_preview_not_confirmed",
            "generate an effect preview before rendering the final static page".to_string(),
        ));
    };
    ensure_static_page_render_image_job_matches_draft(draft, &job)?;
    Ok(Some(job))
}

fn ensure_static_page_render_image_job_matches_draft(
    draft: &StaticPageDraft,
    job: &StaticPageImageJob,
) -> std::result::Result<(), ApiError> {
    if job.draft_id != draft.id {
        return Err(ApiError::bad_request(
            "static_page_image_job_mismatch",
            "image job does not belong to this static page draft".to_string(),
        ));
    }
    if !static_page_image_job_ready_for_render(draft, &job) {
        return Err(ApiError::bad_request(
            "static_page_preview_not_confirmed",
            "generate an effect preview before rendering the final static page".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use domain_model::{AssistantRunId, StaticPageDraftStatus, StaticPageImageJobStatus, TenantId};
    use serde_json::{json, Value};

    use super::*;

    fn test_draft(source_refs: Value) -> StaticPageDraft {
        let now = Utc::now();
        StaticPageDraft {
            id: StaticPageDraftId::new(),
            tenant_id: TenantId::new(),
            owner_user_id: None,
            assistant_run_id: AssistantRunId::new(),
            title: "static page".to_string(),
            status: StaticPageDraftStatus::Previewed,
            selected_scope: Value::Null,
            visibility_snapshot: Value::Null,
            source_refs,
            draft_payload: Value::Null,
            created_at: now,
            updated_at: now,
        }
    }

    fn test_job(
        draft: &StaticPageDraft,
        status: StaticPageImageJobStatus,
        preview_asset_key: Option<&str>,
    ) -> StaticPageImageJob {
        let now = Utc::now();
        StaticPageImageJob {
            id: StaticPageImageJobId::new(),
            tenant_id: draft.tenant_id,
            draft_id: draft.id,
            assistant_run_id: draft.assistant_run_id,
            status,
            queue_position: None,
            image_prompt_payload: Value::Null,
            preview_asset_key: preview_asset_key.map(ToOwned::to_owned),
            failure_reason: None,
            confirmed_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn render_image_job_match_accepts_renderable_job_for_same_draft() {
        let draft = test_draft(json!({ "effect_image_confirmation_required": false }));
        let job = test_job(
            &draft,
            StaticPageImageJobStatus::PreviewReady,
            Some("preview.png"),
        );

        ensure_static_page_render_image_job_matches_draft(&draft, &job)
            .expect("preview-ready auto-continue job should match");
    }

    #[test]
    fn render_image_job_match_rejects_job_from_another_draft() {
        let draft = test_draft(Value::Null);
        let other_draft = test_draft(Value::Null);
        let job = test_job(&other_draft, StaticPageImageJobStatus::Confirmed, None);

        let error = ensure_static_page_render_image_job_matches_draft(&draft, &job)
            .expect_err("different draft should fail");

        assert_eq!(error.payload.code, "static_page_image_job_mismatch");
        assert!(error
            .payload
            .message
            .contains("image job does not belong to this static page draft"));
    }

    #[test]
    fn render_image_job_match_rejects_unrenderable_preview_job() {
        let draft = test_draft(Value::Null);
        let job = test_job(&draft, StaticPageImageJobStatus::PreviewReady, None);

        let error = ensure_static_page_render_image_job_matches_draft(&draft, &job)
            .expect_err("preview job without asset should fail");

        assert_eq!(error.payload.code, "static_page_preview_not_confirmed");
        assert!(error
            .payload
            .message
            .contains("generate an effect preview before rendering"));
    }
}
