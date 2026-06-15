use serde_json::Value;

use crate::{
    codex_host_fixed_task_public_artifact_url_allowed, static_page_generated_template_preview_url,
    static_page_generated_template_public_url,
};

pub(crate) fn static_page_relaxed_template_match_is_dataset_overlap(
    relaxed_template_match: &Value,
) -> bool {
    relaxed_template_match.get("policy").and_then(Value::as_str) == Some("dataset_overlap")
}

pub(crate) fn static_page_relaxed_template_match_baseline_public_url(
    relaxed_template_match: &Value,
) -> Option<String> {
    relaxed_template_match
        .get("baseline_public_url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| codex_host_fixed_task_public_artifact_url_allowed(value))
        .map(ToOwned::to_owned)
}

pub(crate) fn static_page_template_baseline_visual_contract_url(
    template_reference: Option<&Value>,
    relaxed_template_match: &Value,
) -> Option<String> {
    if let Some(preview_url) =
        template_reference.and_then(static_page_generated_template_preview_url)
    {
        return Some(preview_url);
    }

    static_page_relaxed_template_match_baseline_public_url(relaxed_template_match)
        .or_else(|| template_reference.and_then(static_page_generated_template_public_url))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const PUBLIC_URL: &str =
        "https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai/report/index.html";
    const PREVIEW_URL: &str =
        "https://v3.elepcloud.com/generated-artifacts/static-page-previews/xinbai-template.png";

    #[test]
    fn relaxed_template_match_requires_dataset_overlap_policy() {
        assert!(static_page_relaxed_template_match_is_dataset_overlap(
            &json!({"policy": "dataset_overlap"})
        ));
        assert!(!static_page_relaxed_template_match_is_dataset_overlap(
            &json!({"policy": "source_overlap"})
        ));
        assert!(!static_page_relaxed_template_match_is_dataset_overlap(
            &json!({"policy": " dataset_overlap "})
        ));
    }

    #[test]
    fn relaxed_baseline_public_url_trims_and_filters_generated_artifacts() {
        assert_eq!(
            static_page_relaxed_template_match_baseline_public_url(&json!({
                "baseline_public_url": format!(" {PUBLIC_URL} ")
            }))
            .as_deref(),
            Some(PUBLIC_URL)
        );
        assert_eq!(
            static_page_relaxed_template_match_baseline_public_url(&json!({
                "baseline_public_url": "https://v3.elepcloud.com/generated-artifacts/pending-x/index.html"
            })),
            None
        );
        assert_eq!(
            static_page_relaxed_template_match_baseline_public_url(&json!({
                "baseline_public_url": "https://example.com/generated-artifacts/static-pages/x/index.html"
            })),
            None
        );
    }

    #[test]
    fn visual_contract_prefers_template_preview_then_relaxed_baseline_then_public_url() {
        let reference_with_preview = json!({
            "source": "v3-static-page-template-library",
            "templateKind": "generated_static_page",
            "publicUrl": PUBLIC_URL,
            "previewUrl": PREVIEW_URL,
        });
        let relaxed_match = json!({
            "policy": "dataset_overlap",
            "baseline_public_url": "https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai/baseline/index.html"
        });
        assert_eq!(
            static_page_template_baseline_visual_contract_url(
                Some(&reference_with_preview),
                &relaxed_match
            )
            .as_deref(),
            Some(PREVIEW_URL)
        );

        let reference_without_preview = json!({
            "source": "v3-static-page-template-library",
            "templateKind": "generated_static_page",
            "publicUrl": PUBLIC_URL,
        });
        assert_eq!(
            static_page_template_baseline_visual_contract_url(
                Some(&reference_without_preview),
                &relaxed_match
            )
            .as_deref(),
            Some("https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai/baseline/index.html")
        );

        assert_eq!(
            static_page_template_baseline_visual_contract_url(
                Some(&reference_without_preview),
                &json!({"policy": "dataset_overlap"})
            )
            .as_deref(),
            Some(PUBLIC_URL)
        );
    }

    #[test]
    fn visual_contract_can_use_relaxed_baseline_without_template_reference() {
        assert_eq!(
            static_page_template_baseline_visual_contract_url(
                None,
                &json!({
                    "policy": "dataset_overlap",
                    "baseline_public_url": PUBLIC_URL
                })
            )
            .as_deref(),
            Some(PUBLIC_URL)
        );
    }
}
