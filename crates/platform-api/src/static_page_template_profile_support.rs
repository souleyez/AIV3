use domain_model::StaticPageDraft;
use serde_json::Value;

use crate::static_page_published_public_url_from_draft;
use crate::static_page_template_binding_support::static_page_template_text_contains_any;
use crate::static_page_template_match_support::static_page_template_intent_reuse_class;
use crate::XINBAI_PUBLISHED_REPORT_TITLE;

pub(crate) fn static_page_template_limited_value_text(value: &Value) -> String {
    value.to_string().chars().take(20_000).collect()
}

pub(crate) fn static_page_template_draft_profile_text(draft: &StaticPageDraft) -> String {
    let mut text = String::new();
    text.push_str(&draft.title);
    text.push('\n');
    if let Some(public_url) = static_page_published_public_url_from_draft(draft) {
        text.push_str(&public_url);
        text.push('\n');
    }
    for value in [
        &draft.selected_scope,
        &draft.visibility_snapshot,
        &draft.source_refs,
        &draft.draft_payload,
    ] {
        text.push_str(&static_page_template_limited_value_text(value));
        text.push('\n');
    }
    text
}

pub(crate) fn static_page_template_profile_has_xinbai_primary_default_signal(
    profile: &str,
    profile_lower: &str,
) -> bool {
    static_page_template_text_contains_any(
        profile,
        profile_lower,
        &[
            "xinbai-functional-modular-template-20260604",
            "xinbai_business_report",
            "monthly_report_only_default_template",
            "project_unique_default_template",
            "xinbai_only_accepted_default_template",
            XINBAI_PUBLISHED_REPORT_TITLE,
        ],
    )
}

pub(crate) fn static_page_template_draft_is_xinbai_primary_default_template(
    draft: &StaticPageDraft,
) -> bool {
    if static_page_published_public_url_from_draft(draft).is_some_and(|url| {
        url.to_ascii_lowercase()
            .contains("xinbai-functional-modular-template-20260604")
    }) {
        return true;
    }
    let profile = static_page_template_draft_profile_text(draft);
    let profile_lower = profile.to_ascii_lowercase();
    static_page_template_profile_has_xinbai_primary_default_signal(&profile, &profile_lower)
}

pub(crate) fn static_page_template_draft_is_non_default_noise_baseline(
    draft: &StaticPageDraft,
) -> bool {
    let profile = static_page_template_draft_profile_text(draft);
    let profile_lower = profile.to_ascii_lowercase();
    static_page_template_text_contains_any(
        &profile,
        &profile_lower,
        &[
            "并发编号",
            "smoke",
            "template_prewarm",
            "prewarm",
            "测试页",
            "test page",
            "static_page_template_prewarm_candidate",
            "DataMax 静态页模板预热",
        ],
    )
}

pub(crate) fn static_page_template_draft_is_local_generated_report_instance(
    draft: &StaticPageDraft,
) -> bool {
    if static_page_template_draft_is_xinbai_primary_default_template(draft) {
        return false;
    }
    let profile = static_page_template_draft_profile_text(draft);
    let profile_lower = profile.to_ascii_lowercase();
    static_page_template_text_contains_any(
        &profile,
        &profile_lower,
        &[
            "local_generated_artifact_first",
            "static-page-renderer-v1-local-generated-artifact",
            "direct_html_fallback\":true",
            "directHtml\":true",
            "v3_external_channel_static_page_local_generated_artifact",
            "/database-static-pages/external-channel/",
        ],
    )
}

pub(crate) fn static_page_template_context_prefers_xinbai_primary(
    current_prompt: Option<&str>,
    selected_scope: &Value,
    source_refs: &Value,
) -> bool {
    let mut text = String::new();
    if let Some(prompt) = current_prompt {
        text.push_str(prompt);
        text.push('\n');
    }
    text.push_str(&static_page_template_limited_value_text(selected_scope));
    text.push('\n');
    text.push_str(&static_page_template_limited_value_text(source_refs));
    let lower = text.to_ascii_lowercase();
    let has_xinbai_scope_or_prompt = static_page_template_text_contains_any(
        &text,
        &lower,
        &[
            "新百",
            "新世界",
            "新世界百货",
            "xinbai",
            "hy-sql-traffic-area",
        ],
    );
    if !has_xinbai_scope_or_prompt {
        return false;
    }
    static_page_template_intent_reuse_class(current_prompt.unwrap_or_default())
        == Some("business_report")
        || static_page_template_text_contains_any(
            &text,
            &lower,
            &[
                "取高",
                "高分成",
                "经营",
                "报表",
                "月报",
                "风险",
                "销售缺口",
                "助推",
                "坪效",
                "客流",
                "dashboard",
                "report",
            ],
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{
        AssistantRunId, StaticPageDraftId, StaticPageDraftStatus, TenantId, UserId,
    };
    use serde_json::json;

    use crate::XINBAI_PUBLISHED_REPORT_DEFAULT_PUBLIC_URL;

    fn rendered_draft(title: &str, source_refs: Value, draft_payload: Value) -> StaticPageDraft {
        StaticPageDraft {
            id: StaticPageDraftId::new(),
            tenant_id: TenantId::new(),
            assistant_run_id: AssistantRunId::new(),
            owner_user_id: Some(UserId::new()),
            title: title.to_string(),
            status: StaticPageDraftStatus::Rendered,
            selected_scope: json!({}),
            visibility_snapshot: json!({}),
            source_refs,
            draft_payload,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn profile_text_includes_title_public_url_and_limited_payload() {
        let long_text = "x".repeat(25_000);
        let draft = rendered_draft(
            "静态页：新百经营分析月报",
            json!({}),
            json!({
                "finalPage": {
                    "publicUrl": XINBAI_PUBLISHED_REPORT_DEFAULT_PUBLIC_URL
                },
                "long": long_text
            }),
        );

        let profile = static_page_template_draft_profile_text(&draft);

        assert!(profile.contains("静态页：新百经营分析月报"));
        assert!(profile.contains(XINBAI_PUBLISHED_REPORT_DEFAULT_PUBLIC_URL));
        assert!(profile.len() < 45_000);
    }

    #[test]
    fn xinbai_primary_signal_accepts_primary_markers_and_rejects_noise() {
        let primary = rendered_draft(
            "静态页：新百经营分析月报",
            json!({
                "artifact_stability": {
                    "default_template_scope": "xinbai_business_report"
                }
            }),
            json!({
                "finalPage": {
                    "publicUrl": XINBAI_PUBLISHED_REPORT_DEFAULT_PUBLIC_URL
                },
                "features": [
                    "monthly_report_only_default_template",
                    "project_unique_default_template",
                    "xinbai_only_accepted_default_template"
                ]
            }),
        );
        let smoke = rendered_draft(
            "静态页：经营分析报表 并发编号 3",
            json!({}),
            json!({
                "finalPage": {
                    "publicUrl": "https://v3.elepcloud.com/generated-artifacts/database-static-pages/codex-host/smoke/index.html"
                }
            }),
        );

        assert!(static_page_template_draft_is_xinbai_primary_default_template(&primary));
        assert!(!static_page_template_draft_is_non_default_noise_baseline(
            &primary
        ));
        assert!(!static_page_template_draft_is_xinbai_primary_default_template(&smoke));
        assert!(static_page_template_draft_is_non_default_noise_baseline(
            &smoke
        ));
    }

    #[test]
    fn local_generated_report_instance_excludes_xinbai_primary_template() {
        let primary = rendered_draft(
            "静态页：新百经营分析月报",
            json!({}),
            json!({
                "finalPage": {
                    "publicUrl": XINBAI_PUBLISHED_REPORT_DEFAULT_PUBLIC_URL
                }
            }),
        );
        let generated = rendered_draft(
            "静态页：经营分析报表",
            json!({
                "source": "v3_external_channel_static_page_local_generated_artifact"
            }),
            json!({
                "directHtml": true,
                "finalPage": {
                    "publicUrl": "https://v3.elepcloud.com/generated-artifacts/database-static-pages/external-channel/demo/index.html"
                }
            }),
        );

        assert!(!static_page_template_draft_is_local_generated_report_instance(&primary));
        assert!(static_page_template_draft_is_local_generated_report_instance(&generated));
    }

    #[test]
    fn context_prefers_xinbai_primary_only_for_xinbai_business_context() {
        assert!(static_page_template_context_prefers_xinbai_primary(
            Some("新百经营月报看取高机会"),
            &json!({}),
            &json!({})
        ));
        assert!(static_page_template_context_prefers_xinbai_primary(
            Some("经营报表"),
            &json!({"database_source_ids": ["hy-sql-traffic-area"]}),
            &json!({})
        ));
        assert!(!static_page_template_context_prefers_xinbai_primary(
            Some("经营报表"),
            &json!({"dataset_external_ids": ["other-project"]}),
            &json!({})
        ));
    }
}
