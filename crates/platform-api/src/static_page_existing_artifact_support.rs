use serde_json::{json, Value};

use crate::{
    codex_host_fixed_task_public_artifact_url_allowed, static_page_artifact_sibling_url,
    static_page_prompt_generated_artifact_urls,
    static_page_prompt_requests_existing_artifact_revision,
    static_page_prompt_requests_explicit_redesign,
};

pub(crate) fn static_page_existing_artifact_reference_from_prompt(prompt: &str) -> Value {
    let Some(public_url) = static_page_prompt_generated_artifact_urls(prompt)
        .into_iter()
        .next()
    else {
        return Value::Null;
    };
    json!({
        "kind": "v3_generated_static_page",
        "source": "prompt_generated_artifact_url",
        "reference_role": "existing_artifact_to_revise",
        "public_url": public_url,
        "index_url": public_url,
        "data_url": static_page_artifact_sibling_url(&public_url, "data.json"),
        "data_snapshot_url": static_page_artifact_sibling_url(&public_url, "data-snapshot.json"),
        "revision_requested": static_page_prompt_requests_existing_artifact_revision(prompt),
        "preserve_style_unless_redesign_requested": !static_page_prompt_requests_explicit_redesign(prompt),
        "data_binding_policy": "read_existing_data_json_when_available_and_rebind_requested_modules",
        "publish_mode": "new_generated_artifact_only",
        "materialization_policy": "host_agent_maps_v3_generated_artifact_to_local_workspace_when_available",
    })
}

pub(crate) fn static_page_generated_template_reference_is_page(reference: &Value) -> bool {
    let source = reference
        .get("source")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let template_kind = reference
        .get("templateKind")
        .or_else(|| reference.get("template_kind"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let template_id = reference
        .get("templateId")
        .or_else(|| reference.get("template_id"))
        .or_else(|| reference.get("id"))
        .and_then(Value::as_str)
        .unwrap_or_default();

    source.eq_ignore_ascii_case("v3-static-page-template-library")
        || template_kind.eq_ignore_ascii_case("generated_static_page")
        || template_id.starts_with("generated-static-page:")
        || template_id.starts_with("static-page-template:")
        || template_id.starts_with("static_page_template:")
}

pub(crate) fn static_page_generated_template_public_url(reference: &Value) -> Option<String> {
    if !static_page_generated_template_reference_is_page(reference) {
        return None;
    }
    [
        "publicUrl",
        "public_url",
        "generatedArtifactUrl",
        "generated_artifact_url",
    ]
    .into_iter()
    .filter_map(|key| reference.get(key).and_then(Value::as_str))
    .map(str::trim)
    .filter(|value| codex_host_fixed_task_public_artifact_url_allowed(value))
    .map(ToOwned::to_owned)
    .next()
}

pub(crate) fn static_page_generated_template_preview_url(reference: &Value) -> Option<String> {
    if !static_page_generated_template_reference_is_page(reference) {
        return None;
    }
    [
        "previewUrl",
        "preview_url",
        "effectImageUrl",
        "effect_image_url",
        "visualContractUrl",
        "visual_contract_url",
    ]
    .into_iter()
    .filter_map(|key| reference.get(key).and_then(Value::as_str))
    .map(str::trim)
    .filter(|value| codex_host_fixed_task_public_artifact_url_allowed(value))
    .map(ToOwned::to_owned)
    .next()
}

pub(crate) fn static_page_existing_artifact_reference_from_public_url(
    public_url: &str,
    source: &str,
) -> Value {
    json!({
        "kind": "v3_generated_static_page",
        "source": source,
        "reference_role": "existing_artifact_to_revise",
        "public_url": public_url,
        "index_url": public_url,
        "data_url": static_page_artifact_sibling_url(public_url, "data.json"),
        "data_snapshot_url": static_page_artifact_sibling_url(public_url, "data-snapshot.json"),
        "revision_requested": true,
        "preserve_style_unless_redesign_requested": true,
        "data_binding_policy": "read_existing_data_json_when_available_and_rebind_requested_modules",
        "publish_mode": "new_generated_artifact_only",
        "materialization_policy": "host_agent_maps_v3_generated_artifact_to_local_workspace_when_available",
    })
}

pub(crate) fn static_page_existing_artifact_reference_from_template_context(
    prompt: &str,
    template_reference: Option<&Value>,
    source_refs: Option<&Value>,
) -> Value {
    if static_page_prompt_requests_explicit_redesign(prompt) {
        return Value::Null;
    }

    if let Some(reference) = template_reference {
        if let Some(public_url) = static_page_generated_template_public_url(reference) {
            let mut existing = static_page_existing_artifact_reference_from_public_url(
                &public_url,
                "generated_static_page_template_reference",
            );
            if let Some(object) = existing.as_object_mut() {
                object.insert("template_reference".to_string(), reference.clone());
            }
            return existing;
        }
    }

    if let Some(source_refs) = source_refs {
        for reference in source_refs
            .get("template_references")
            .or_else(|| source_refs.get("templateReferences"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(public_url) = static_page_generated_template_public_url(reference) {
                let mut existing = static_page_existing_artifact_reference_from_public_url(
                    &public_url,
                    "generated_static_page_template_source_refs",
                );
                if let Some(object) = existing.as_object_mut() {
                    object.insert("template_reference".to_string(), reference.clone());
                }
                return existing;
            }
        }

        if source_refs
            .pointer("/relaxed_template_match/policy")
            .and_then(Value::as_str)
            == Some("dataset_overlap")
        {
            if let Some(public_url) = source_refs
                .pointer("/relaxed_template_match/baseline_public_url")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| codex_host_fixed_task_public_artifact_url_allowed(value))
            {
                let mut existing = static_page_existing_artifact_reference_from_public_url(
                    public_url,
                    "dataset_overlap_static_page_template_baseline",
                );
                if let Some(object) = existing.as_object_mut() {
                    object.insert(
                        "relaxed_template_match".to_string(),
                        source_refs
                            .get("relaxed_template_match")
                            .cloned()
                            .unwrap_or(Value::Null),
                    );
                }
                return existing;
            }
        }
    }

    Value::Null
}

pub(crate) fn static_page_existing_artifact_reference_for_fixed_task(
    prompt: &str,
    template_reference: Option<&Value>,
    source_refs: Option<&Value>,
) -> Value {
    let explicit = static_page_existing_artifact_reference_from_prompt(prompt);
    if !explicit.is_null() {
        return explicit;
    }
    static_page_existing_artifact_reference_from_template_context(
        prompt,
        template_reference,
        source_refs,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const PUBLIC_URL: &str =
        "https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai/report/index.html";

    #[test]
    fn prompt_reference_builds_existing_artifact_links() {
        let reference = static_page_existing_artifact_reference_from_prompt(
            "修复页面 https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai/report/index.html?focus=old#frag，切换门店后重算。",
        );

        assert_eq!(reference["source"], json!("prompt_generated_artifact_url"));
        assert_eq!(reference["public_url"], json!(PUBLIC_URL));
        assert_eq!(
            reference["data_url"],
            json!("https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai/report/data.json")
        );
        assert_eq!(
            reference["data_snapshot_url"],
            json!("https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai/report/data-snapshot.json")
        );
        assert_eq!(reference["revision_requested"], json!(true));
        assert_eq!(
            reference["preserve_style_unless_redesign_requested"],
            json!(true)
        );
    }

    #[test]
    fn prompt_reference_returns_null_without_generated_url() {
        assert_eq!(
            static_page_existing_artifact_reference_from_prompt("重新生成新百经营月报"),
            Value::Null
        );
    }

    #[test]
    fn template_reference_is_used_unless_prompt_requests_redesign() {
        let template_reference = json!({
            "source": "v3-static-page-template-library",
            "templateKind": "generated_static_page",
            "publicUrl": PUBLIC_URL,
        });

        let reference = static_page_existing_artifact_reference_for_fixed_task(
            "修复近7日销售，切换区域和门店后需要跟着变化",
            Some(&template_reference),
            None,
        );
        assert_eq!(
            reference["source"],
            json!("generated_static_page_template_reference")
        );
        assert_eq!(reference["public_url"], json!(PUBLIC_URL));
        assert_eq!(reference["template_reference"], template_reference);

        assert_eq!(
            static_page_existing_artifact_reference_for_fixed_task(
                "重新出图，换个暗黑移动端风格",
                Some(&template_reference),
                None,
            ),
            Value::Null
        );
    }

    #[test]
    fn source_refs_template_and_relaxed_overlap_fallback_are_supported() {
        let source_refs = json!({
            "template_references": [
                {
                    "templateId": "generated-static-page:template-001",
                    "generatedArtifactUrl": PUBLIC_URL
                }
            ],
            "relaxed_template_match": {
                "policy": "dataset_overlap",
                "baseline_public_url": "https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai/fallback/index.html"
            }
        });

        let reference = static_page_existing_artifact_reference_from_template_context(
            "生成新百经营月报",
            None,
            Some(&source_refs),
        );
        assert_eq!(
            reference["source"],
            json!("generated_static_page_template_source_refs")
        );
        assert_eq!(reference["public_url"], json!(PUBLIC_URL));

        let relaxed_only = json!({
            "relaxed_template_match": {
                "policy": "dataset_overlap",
                "baseline_public_url": "https://v3.elepcloud.com/generated-artifacts/database-static-pages/xinbai/fallback/index.html"
            }
        });
        let relaxed_reference = static_page_existing_artifact_reference_from_template_context(
            "生成新百经营月报",
            None,
            Some(&relaxed_only),
        );
        assert_eq!(
            relaxed_reference["source"],
            json!("dataset_overlap_static_page_template_baseline")
        );
        assert_eq!(
            relaxed_reference["relaxed_template_match"],
            relaxed_only["relaxed_template_match"]
        );
    }
}
