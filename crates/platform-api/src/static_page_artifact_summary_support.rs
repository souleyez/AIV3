use serde_json::Value;

pub(crate) fn static_page_artifact_string(artifact: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        artifact
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    })
}

pub(crate) fn static_page_artifact_id(artifact: &Value) -> Option<String> {
    ["backendDraftId", "backend_draft_id", "backendId", "id"]
        .iter()
        .find_map(|key| {
            artifact
                .get(*key)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
        })
}

pub(crate) fn static_page_artifact_module_count(artifact: &Value) -> usize {
    ["moduleCount", "module_count"]
        .iter()
        .find_map(|key| artifact.get(*key).and_then(Value::as_u64))
        .map(|value| value as usize)
        .or_else(|| {
            artifact
                .get("modules")
                .and_then(Value::as_array)
                .map(Vec::len)
        })
        .unwrap_or(0)
}

pub(crate) fn static_page_artifact_preview_status(artifact: &Value) -> Option<String> {
    static_page_artifact_string(artifact, &["previewStatus", "preview_status"])
        .or_else(|| {
            artifact
                .get("previewContract")
                .or_else(|| artifact.get("preview_contract"))
                .and_then(|value| value.get("status"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
        })
        .or_else(|| {
            artifact
                .get("imageJob")
                .or_else(|| artifact.get("image_job"))
                .and_then(|value| value.get("status"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
        })
}

pub(crate) fn static_page_artifact_final_status(artifact: &Value) -> Option<String> {
    static_page_artifact_string(artifact, &["finalRenderStatus", "final_render_status"]).or_else(
        || {
            artifact
                .get("finalPage")
                .or_else(|| artifact.get("final_page"))
                .and_then(|value| value.get("status"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
        },
    )
}

pub(crate) fn static_page_artifact_preview_stale(artifact: &Value) -> bool {
    artifact
        .get("previewStale")
        .or_else(|| artifact.get("preview_stale"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || static_page_artifact_preview_status(artifact).as_deref() == Some("stale")
        || artifact
            .get("imageJob")
            .or_else(|| artifact.get("image_job"))
            .and_then(|value| value.get("status"))
            .and_then(Value::as_str)
            == Some("stale")
}

pub(crate) fn static_page_artifact_label(artifact: &Value) -> Option<String> {
    ["objective", "title"].iter().find_map(|key| {
        artifact
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| format!("当前静态页：{}", truncate_static_page_artifact_label(value)))
    })
}

fn truncate_static_page_artifact_label(value: &str) -> String {
    value.chars().take(80).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn artifact_string_trims_first_non_empty_alias() {
        let artifact = json!({
            "status": "  ",
            "backendStatus": " ready ",
            "backend_status": "ignored"
        });

        assert_eq!(
            static_page_artifact_string(&artifact, &["status", "backendStatus", "backend_status"]),
            Some("ready".to_string())
        );
    }

    #[test]
    fn artifact_id_accepts_backend_aliases_before_id() {
        let artifact = json!({
            "backend_draft_id": " draft-1 ",
            "id": "fallback"
        });

        assert_eq!(
            static_page_artifact_id(&artifact),
            Some("draft-1".to_string())
        );
    }

    #[test]
    fn artifact_module_count_prefers_explicit_count_then_modules() {
        assert_eq!(
            static_page_artifact_module_count(&json!({
                "module_count": 7,
                "modules": [{}, {}]
            })),
            7
        );
        assert_eq!(
            static_page_artifact_module_count(&json!({
                "modules": [{}, {}, {}]
            })),
            3
        );
    }

    #[test]
    fn artifact_statuses_read_direct_and_nested_values() {
        let artifact = json!({
            "preview_contract": { "status": " preview-ready " },
            "final_page": { "status": " published " }
        });

        assert_eq!(
            static_page_artifact_preview_status(&artifact),
            Some("preview-ready".to_string())
        );
        assert_eq!(
            static_page_artifact_final_status(&artifact),
            Some("published".to_string())
        );
    }

    #[test]
    fn artifact_preview_stale_accepts_flags_and_nested_status() {
        assert!(static_page_artifact_preview_stale(&json!({
            "preview_stale": true
        })));
        assert!(static_page_artifact_preview_stale(&json!({
            "previewStatus": "stale"
        })));
        assert!(static_page_artifact_preview_stale(&json!({
            "image_job": { "status": "stale" }
        })));
        assert!(!static_page_artifact_preview_stale(&json!({
            "previewStatus": "preview-ready"
        })));
    }

    #[test]
    fn artifact_label_prefers_objective_and_truncates_to_eighty_chars() {
        let long_title = "一二三四五六七八九十一二三四五六七八九十一二三四五六七八九十一二三四五六七八九十一二三四五六七八九十一二三四五六七八九十一二三四五六七八九十一二三四五六七八九十额外";
        let label = static_page_artifact_label(&json!({
            "objective": long_title,
            "title": "ignored"
        }))
        .expect("label");

        assert!(label.starts_with("当前静态页："));
        assert_eq!(label.trim_start_matches("当前静态页：").chars().count(), 80);
    }
}
