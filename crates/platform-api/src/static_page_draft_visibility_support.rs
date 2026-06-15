use domain_model::{StaticPageDraft, UserId};

use crate::resource_access::static_page_owner_is_visible;
use crate::static_page_template_profile_support::{
    static_page_template_draft_is_local_generated_report_instance,
    static_page_template_draft_is_non_default_noise_baseline,
};
use crate::{
    static_page_dataset_artifact_key_from_draft_context,
    static_page_draft_is_accepted_template_baseline,
    static_page_draft_is_template_fallback_baseline, static_page_published_public_url_from_draft,
};

pub(crate) fn static_page_public_template_default_is_manageable(draft: &StaticPageDraft) -> bool {
    let dataset_artifact_key = static_page_dataset_artifact_key_from_draft_context(draft)
        .unwrap_or_default()
        .to_ascii_lowercase();
    !static_page_draft_is_template_fallback_baseline(draft)
        && !static_page_template_draft_is_non_default_noise_baseline(draft)
        && !static_page_template_draft_is_local_generated_report_instance(draft)
        && !dataset_artifact_key.contains("template:data-report")
        && static_page_published_public_url_from_draft(draft).is_some()
}

pub(crate) fn static_page_public_template_baseline_is_visible(draft: &StaticPageDraft) -> bool {
    static_page_draft_is_accepted_template_baseline(draft)
        && static_page_public_template_default_is_manageable(draft)
}

pub(crate) fn static_page_draft_list_item_is_visible(
    draft: &StaticPageDraft,
    current_user_id: Option<UserId>,
    allow_public_template_baselines: bool,
) -> bool {
    static_page_owner_is_visible(draft.owner_user_id, current_user_id)
        || (allow_public_template_baselines
            && static_page_public_template_baseline_is_visible(draft))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{
        AssistantRunId, StaticPageDraftId, StaticPageDraftStatus, TenantId, UserId,
    };
    use serde_json::{json, Value};

    fn draft_with_refs_and_payload(source_refs: Value, draft_payload: Value) -> StaticPageDraft {
        StaticPageDraft {
            id: StaticPageDraftId::new(),
            tenant_id: TenantId::new(),
            assistant_run_id: AssistantRunId::new(),
            owner_user_id: Some(UserId::new()),
            title: "静态页：新百经营分析月报".to_string(),
            status: StaticPageDraftStatus::Rendered,
            selected_scope: json!({}),
            visibility_snapshot: json!({}),
            source_refs,
            draft_payload,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn accepted_public_baseline(dataset_artifact_key: &str) -> StaticPageDraft {
        draft_with_refs_and_payload(
            json!({
                "artifact_stability": {
                    "baseline_status": "accepted",
                    "dataset_artifact_key": dataset_artifact_key
                }
            }),
            json!({
                "finalPage": {
                    "status": "rendered",
                    "publicUrl": "https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai/current/index.html"
                }
            }),
        )
    }

    #[test]
    fn public_template_baseline_can_be_visible_without_owner_when_allowed() {
        let draft = accepted_public_baseline(
            "v3-static-page|template:generated-static-page:fixture|dataset_external_id:xinbai-project-dataset",
        );

        assert!(!static_page_draft_list_item_is_visible(&draft, None, false));
        assert!(static_page_draft_list_item_is_visible(&draft, None, true));
        assert!(static_page_public_template_baseline_is_visible(&draft));
    }

    #[test]
    fn public_template_default_rejects_data_report_and_fallback_baselines() {
        let data_report = accepted_public_baseline(
            "v3-static-page|template:data-report|dataset_id:xinbai-operating-analysis",
        );
        assert!(!static_page_public_template_default_is_manageable(
            &data_report
        ));
        assert!(!static_page_public_template_baseline_is_visible(
            &data_report
        ));

        let fallback = draft_with_refs_and_payload(
            json!({
                "artifact_stability": {
                    "baseline_status": "accepted",
                    "dataset_artifact_key": "v3-static-page|template:generated-static-page:fixture|dataset_id:xinbai"
                }
            }),
            json!({
                "finalPage": {
                    "status": "rendered",
                    "publicUrl": "https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai-template-fallback/index.html"
                }
            }),
        );

        assert!(!static_page_public_template_default_is_manageable(
            &fallback
        ));
        assert!(!static_page_public_template_baseline_is_visible(&fallback));
    }

    #[test]
    fn owner_visible_draft_stays_visible_without_public_baseline_flag() {
        let draft = draft_with_refs_and_payload(json!({}), json!({}));
        let owner = draft.owner_user_id;

        assert!(static_page_draft_list_item_is_visible(&draft, owner, false));
        assert!(!static_page_draft_list_item_is_visible(
            &draft,
            Some(UserId::new()),
            false
        ));
    }
}
