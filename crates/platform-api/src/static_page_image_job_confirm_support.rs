use domain_model::StaticPageImageJob;
use serde_json::{json, Value};

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

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use domain_model::{
        AssistantRunId, StaticPageDraftId, StaticPageImageJobId, StaticPageImageJobStatus, TenantId,
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
}
