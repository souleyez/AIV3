use domain_model::{StaticPageDraft, StaticPageDraftId};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::static_page_template_match_support::{
    static_page_default_prompt_from_scope_or_refs, static_page_stable_key_token,
    static_page_template_stability_key_with_default_prompt,
};
use crate::static_page_template_reference_support::{
    normalize_static_page_template_reference_id, static_page_template_reference_id_from_source_refs,
};
use crate::{
    codex_host_fixed_task_public_artifact_url_allowed,
    external_channel_static_page_source_ref_string, static_page_dataset_artifact_key,
};

pub(crate) fn static_page_dataset_artifact_key_from_source_refs(
    source_refs: &Value,
) -> Option<String> {
    source_refs
        .get("dataset_artifact_key")
        .and_then(Value::as_str)
        .or_else(|| {
            source_refs
                .pointer("/artifact_stability/dataset_artifact_key")
                .and_then(Value::as_str)
        })
        .or_else(|| {
            source_refs
                .pointer("/artifact_stability/datasetArtifactKey")
                .and_then(Value::as_str)
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub(crate) fn static_page_dataset_artifact_key_from_draft_context(
    draft: &StaticPageDraft,
) -> Option<String> {
    static_page_dataset_artifact_key_from_source_refs(&draft.source_refs).or_else(|| {
        let base_template_stability_key =
            static_page_template_reference_id_from_source_refs(&draft.source_refs)
                .and_then(static_page_stable_key_token)
                .map(|value| format!("template:{value}"))
                .unwrap_or_else(|| "template:default".to_string());
        let template_stability_key = static_page_template_stability_key_with_default_prompt(
            base_template_stability_key,
            static_page_default_prompt_from_scope_or_refs(
                &draft.selected_scope,
                &draft.source_refs,
            ),
        );
        static_page_dataset_artifact_key(
            &draft.selected_scope,
            &draft.source_refs,
            &template_stability_key,
            external_channel_static_page_source_ref_string(
                &draft.source_refs,
                "channel_connection_id",
            )
            .as_deref(),
        )
    })
}

pub(crate) fn static_page_published_public_url_from_draft(
    draft: &StaticPageDraft,
) -> Option<String> {
    [
        draft
            .draft_payload
            .pointer("/finalPage/publicUrl")
            .and_then(Value::as_str),
        draft
            .draft_payload
            .pointer("/finalPage/public_url")
            .and_then(Value::as_str),
        draft
            .draft_payload
            .pointer("/finalPage/generatedArtifactUrl")
            .and_then(Value::as_str),
        draft
            .draft_payload
            .pointer("/artifactStability/publicUrl")
            .and_then(Value::as_str),
        draft
            .draft_payload
            .pointer("/artifact_stability/public_url")
            .and_then(Value::as_str),
        draft
            .source_refs
            .pointer("/artifact_stability/public_url")
            .and_then(Value::as_str),
        draft
            .source_refs
            .pointer("/artifact_stability/publicUrl")
            .and_then(Value::as_str),
    ]
    .into_iter()
    .flatten()
    .map(str::trim)
    .filter(|value| codex_host_fixed_task_public_artifact_url_allowed(value))
    .map(ToOwned::to_owned)
    .next()
}

pub(crate) fn static_page_draft_effective_baseline_status(draft: &StaticPageDraft) -> Option<&str> {
    [
        draft
            .source_refs
            .pointer("/artifact_stability/baseline_status")
            .and_then(Value::as_str),
        draft
            .source_refs
            .pointer("/artifact_stability/baselineStatus")
            .and_then(Value::as_str),
        draft
            .draft_payload
            .pointer("/artifact_stability/baseline_status")
            .and_then(Value::as_str),
        draft
            .draft_payload
            .pointer("/artifact_stability/baselineStatus")
            .and_then(Value::as_str),
        draft
            .draft_payload
            .get("baseline_status")
            .and_then(Value::as_str),
        draft
            .draft_payload
            .get("baselineStatus")
            .and_then(Value::as_str),
        draft
            .draft_payload
            .pointer("/artifactStability/baselineStatus")
            .and_then(Value::as_str),
        draft
            .draft_payload
            .pointer("/artifactStability/baseline_status")
            .and_then(Value::as_str),
        draft
            .draft_payload
            .pointer("/finalPage/baselineStatus")
            .and_then(Value::as_str),
        draft
            .draft_payload
            .pointer("/finalPage/baseline_status")
            .and_then(Value::as_str),
        draft
            .draft_payload
            .pointer("/final_page/baseline_status")
            .and_then(Value::as_str),
        draft
            .draft_payload
            .pointer("/final_page/baselineStatus")
            .and_then(Value::as_str),
    ]
    .into_iter()
    .flatten()
    .map(str::trim)
    .find(|value| !value.is_empty())
}

pub(crate) fn static_page_draft_is_accepted_template_baseline(draft: &StaticPageDraft) -> bool {
    static_page_draft_effective_baseline_status(draft) == Some("accepted")
}

pub(crate) fn static_page_draft_is_template_fallback_baseline(draft: &StaticPageDraft) -> bool {
    if static_page_published_public_url_from_draft(draft).is_some_and(|url| {
        url.contains("-template-fallback/") || url.contains("/template-fallback/")
    }) {
        return true;
    }
    if draft
        .draft_payload
        .pointer("/validation_report/snapshot_policy")
        .and_then(Value::as_str)
        == Some("v3_local_template_fallback_after_publish_failure")
    {
        return true;
    }
    if draft
        .draft_payload
        .pointer("/manifest/kind")
        .and_then(Value::as_str)
        == Some("v3_codex_host_static_page_template_fallback")
    {
        return true;
    }
    [
        draft.draft_payload.pointer("/mode"),
        draft.draft_payload.pointer("/finalPage/mode"),
        draft.source_refs.pointer("/mode"),
    ]
    .into_iter()
    .flatten()
    .any(|value| value.as_str() == Some("static_page_template_fallback"))
}

pub(crate) fn static_page_template_preview_url_from_draft(
    draft: &StaticPageDraft,
) -> Option<String> {
    [
        draft
            .draft_payload
            .pointer("/finalPage/previewUrl")
            .and_then(Value::as_str),
        draft
            .draft_payload
            .pointer("/finalPage/preview_url")
            .and_then(Value::as_str),
        draft
            .draft_payload
            .pointer("/finalPage/effectImageUrl")
            .and_then(Value::as_str),
        draft
            .draft_payload
            .pointer("/finalPage/effect_image_url")
            .and_then(Value::as_str),
        draft
            .source_refs
            .pointer("/image2/preview_url")
            .and_then(Value::as_str),
        draft
            .source_refs
            .pointer("/preview_url")
            .and_then(Value::as_str),
    ]
    .into_iter()
    .flatten()
    .map(str::trim)
    .filter(|value| codex_host_fixed_task_public_artifact_url_allowed(value))
    .map(ToOwned::to_owned)
    .next()
}

pub(crate) fn static_page_generated_template_reference_id(draft_id: StaticPageDraftId) -> String {
    format!("generated-static-page:{draft_id}")
}

pub(crate) fn static_page_generated_template_draft_id(
    raw_id: Option<&str>,
) -> Option<StaticPageDraftId> {
    let raw_id = normalize_static_page_template_reference_id(raw_id)?;
    let draft_id = raw_id
        .strip_prefix("generated-static-page:")
        .or_else(|| raw_id.strip_prefix("static-page-template:"))
        .or_else(|| raw_id.strip_prefix("static_page_template:"))?;
    Uuid::parse_str(draft_id.trim()).ok().map(StaticPageDraftId)
}

pub(crate) fn static_page_generated_template_reference_from_draft(
    draft: &StaticPageDraft,
    public_url: &str,
) -> Value {
    let template_id = static_page_generated_template_reference_id(draft.id);
    let dataset_artifact_key = static_page_dataset_artifact_key_from_draft_context(draft);
    let preview_url = static_page_template_preview_url_from_draft(draft);
    json!({
        "templateId": template_id.as_str(),
        "template_id": template_id.as_str(),
        "source": "v3-static-page-template-library",
        "templateKind": "generated_static_page",
        "template_kind": "generated_static_page",
        "draftId": draft.id,
        "draft_id": draft.id,
        "assistantRunId": draft.assistant_run_id,
        "assistant_run_id": draft.assistant_run_id,
        "label": draft.title,
        "category": "generated",
        "scenario": "published_static_page",
        "publicUrl": public_url,
        "public_url": public_url,
        "previewUrl": preview_url.as_deref(),
        "preview_url": preview_url.as_deref(),
        "datasetArtifactKey": dataset_artifact_key.as_deref(),
        "dataset_artifact_key": dataset_artifact_key.as_deref(),
        "styleReusePolicy": "reuse_style_unless_explicit_redesign",
        "style_reuse_policy": "reuse_style_unless_explicit_redesign",
        "dataRefreshPolicy": "refresh_data_files_from_dataset_sources",
        "data_refresh_policy": "refresh_data_files_from_dataset_sources",
        "designIntent": "复用已发布静态页的视觉风格、版式层级和组件组织；事实、指标和明细仍以当前可见数据集自动刷新后的数据为准。",
        "guardrails": [
            "template supplies visual style and page structure only",
            "current dataset scope remains authoritative for data",
            "do not regenerate Image2 unless the user explicitly requests a new style"
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{AssistantRunId, StaticPageDraftStatus, TenantId, UserId};

    const PUBLIC_URL: &str =
        "https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai/report/index.html";
    const PREVIEW_URL: &str =
        "https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai/report/effect.png";

    fn test_draft(
        selected_scope: Value,
        source_refs: Value,
        draft_payload: Value,
    ) -> StaticPageDraft {
        StaticPageDraft {
            id: StaticPageDraftId::new(),
            tenant_id: TenantId::new(),
            assistant_run_id: AssistantRunId::new(),
            owner_user_id: Some(UserId::new()),
            title: "静态页：新百经营分析月报".to_string(),
            status: StaticPageDraftStatus::Rendered,
            selected_scope,
            visibility_snapshot: json!({}),
            source_refs,
            draft_payload,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn dataset_artifact_key_reads_direct_and_artifact_stability_metadata() {
        assert_eq!(
            static_page_dataset_artifact_key_from_source_refs(&json!({
                "dataset_artifact_key": " key-direct "
            }))
            .as_deref(),
            Some("key-direct")
        );
        assert_eq!(
            static_page_dataset_artifact_key_from_source_refs(&json!({
                "artifact_stability": {
                    "datasetArtifactKey": " key-camel "
                }
            }))
            .as_deref(),
            Some("key-camel")
        );
        assert_eq!(
            static_page_dataset_artifact_key_from_source_refs(&json!({
                "dataset_artifact_key": "  "
            })),
            None
        );
    }

    #[test]
    fn draft_context_falls_back_to_scope_material_key() {
        let draft = test_draft(
            json!({
                "dataset_external_ids": ["dataset-a"]
            }),
            json!({
                "template_reference_id": "generated-static-page:template-001",
                "channel_connection_id": "generic-chat-main",
                "answer_policy": {
                    "default_prompt": "经营月报"
                }
            }),
            json!({}),
        );

        let key = static_page_dataset_artifact_key_from_draft_context(&draft)
            .expect("material scope should create a stable artifact key");

        assert!(key.starts_with("v3-static-page|template:generated-static-page\\:template-001"));
        assert!(key.contains("|dataset_external_id:dataset-a"));
        assert!(key.contains("|channel:generic-chat-main"));
        assert!(key.contains("|default-prompt:"));
    }

    #[test]
    fn published_and_preview_urls_use_allowed_generated_artifact_urls() {
        let draft = test_draft(
            json!({}),
            json!({
                "artifact_stability": {
                    "publicUrl": "https://example.com/private.html"
                },
                "image2": {
                    "preview_url": PREVIEW_URL
                }
            }),
            json!({
                "finalPage": {
                    "publicUrl": " https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai/report/index.html?focus=old "
                }
            }),
        );

        assert_eq!(
            static_page_published_public_url_from_draft(&draft).as_deref(),
            Some("https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai/report/index.html?focus=old")
        );
        assert_eq!(
            static_page_template_preview_url_from_draft(&draft).as_deref(),
            Some(PREVIEW_URL)
        );
    }

    #[test]
    fn baseline_status_prefers_source_refs_and_detects_fallbacks() {
        let mut draft = test_draft(
            json!({}),
            json!({
                "artifact_stability": {
                    "baseline_status": " retired "
                }
            }),
            json!({
                "artifactStability": {
                    "baselineStatus": "accepted"
                },
                "finalPage": {
                    "baselineStatus": "accepted",
                    "publicUrl": PUBLIC_URL
                }
            }),
        );

        assert_eq!(
            static_page_draft_effective_baseline_status(&draft),
            Some("retired")
        );
        assert!(!static_page_draft_is_accepted_template_baseline(&draft));
        assert!(!static_page_draft_is_template_fallback_baseline(&draft));

        draft.source_refs = json!({
            "artifact_stability": {
                "baseline_status": "accepted"
            }
        });
        draft.draft_payload = json!({
            "finalPage": {
                "publicUrl": "https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai/template-fallback/index.html"
            }
        });
        assert!(static_page_draft_is_accepted_template_baseline(&draft));
        assert!(static_page_draft_is_template_fallback_baseline(&draft));

        draft.draft_payload = json!({
            "manifest": {
                "kind": "v3_codex_host_static_page_template_fallback"
            }
        });
        assert!(static_page_draft_is_template_fallback_baseline(&draft));
    }

    #[test]
    fn generated_template_ids_round_trip_and_reference_keeps_contract() {
        let draft = test_draft(
            json!({
                "dataset_external_ids": ["dataset-a"]
            }),
            json!({
                "artifact_stability": {
                    "dataset_artifact_key": "v3-static-page|template:default|dataset_external_id:dataset-a"
                },
                "preview_url": PREVIEW_URL
            }),
            json!({}),
        );
        let reference_id = static_page_generated_template_reference_id(draft.id);

        assert_eq!(
            static_page_generated_template_draft_id(Some(&reference_id)),
            Some(draft.id)
        );
        assert_eq!(
            static_page_generated_template_draft_id(Some(&format!(
                "static-page-template:{}",
                draft.id
            ))),
            Some(draft.id)
        );
        assert_eq!(
            static_page_generated_template_draft_id(Some("generated-static-page:not-a-uuid")),
            None
        );

        let reference = static_page_generated_template_reference_from_draft(&draft, PUBLIC_URL);

        assert_eq!(reference["templateId"], json!(reference_id));
        assert_eq!(
            reference["source"],
            json!("v3-static-page-template-library")
        );
        assert_eq!(reference["publicUrl"], json!(PUBLIC_URL));
        assert_eq!(reference["previewUrl"], json!(PREVIEW_URL));
        assert_eq!(
            reference["datasetArtifactKey"],
            json!("v3-static-page|template:default|dataset_external_id:dataset-a")
        );
        assert_eq!(
            reference["styleReusePolicy"],
            json!("reuse_style_unless_explicit_redesign")
        );
        assert_eq!(
            reference["dataRefreshPolicy"],
            json!("refresh_data_files_from_dataset_sources")
        );
        assert!(reference["guardrails"].as_array().is_some_and(|items| items
            .iter()
            .any(|item| item == "current dataset scope remains authoritative for data")));
    }
}
