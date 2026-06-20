use domain_model::{StaticPageDraft, StaticPageImageJob};
use static_page_renderer::StaticPageRenderRequest;

pub(crate) fn build_static_page_render_request(
    draft: &StaticPageDraft,
    image_job: Option<&StaticPageImageJob>,
) -> StaticPageRenderRequest {
    StaticPageRenderRequest {
        draft_id: draft.id.to_string(),
        assistant_run_id: draft.assistant_run_id.to_string(),
        title: draft.title.clone(),
        draft_payload: draft.draft_payload.clone(),
        selected_scope: draft.selected_scope.clone(),
        visibility_snapshot: draft.visibility_snapshot.clone(),
        preview_asset_key: image_job.and_then(|job| job.preview_asset_key.clone()),
        image_job_id: image_job.map(|job| job.id.to_string()),
    }
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

    fn draft() -> StaticPageDraft {
        let now = Utc::now();
        StaticPageDraft {
            id: StaticPageDraftId::new(),
            tenant_id: TenantId::new(),
            assistant_run_id: AssistantRunId::new(),
            owner_user_id: None,
            title: "门店取高分析".to_string(),
            status: StaticPageDraftStatus::Confirmed,
            selected_scope: json!({"dataset_ids": ["dataset-1"]}),
            visibility_snapshot: json!({"scope": "visible"}),
            source_refs: json!([]),
            draft_payload: json!({
                "modules": [
                    {"id": "risk", "title": "风险识别"}
                ]
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
            preview_asset_key: Some("static-page-previews/image.json".to_string()),
            failure_reason: None,
            confirmed_at: Some(now),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn render_request_preserves_draft_and_image_context() {
        let draft = draft();
        let job = image_job(&draft);

        let request = build_static_page_render_request(&draft, Some(&job));

        assert_eq!(request.draft_id, draft.id.to_string());
        assert_eq!(request.assistant_run_id, draft.assistant_run_id.to_string());
        assert_eq!(request.title, "门店取高分析");
        assert_eq!(request.draft_payload, draft.draft_payload);
        assert_eq!(request.selected_scope, draft.selected_scope);
        assert_eq!(request.visibility_snapshot, draft.visibility_snapshot);
        assert_eq!(
            request.preview_asset_key.as_deref(),
            Some("static-page-previews/image.json")
        );
        assert_eq!(request.image_job_id, Some(job.id.to_string()));
    }

    #[test]
    fn direct_html_render_request_omits_image_context() {
        let draft = draft();

        let request = build_static_page_render_request(&draft, None);

        assert_eq!(request.draft_id, draft.id.to_string());
        assert_eq!(request.assistant_run_id, draft.assistant_run_id.to_string());
        assert_eq!(request.preview_asset_key, None);
        assert_eq!(request.image_job_id, None);
    }
}
