use crate::ensure_json_object;
use chrono::{DateTime, Utc};
use serde_json::{json, Map, Value};

pub(crate) fn static_page_artifact_stability_metadata(
    dataset_artifact_key: &str,
    baseline_status: &str,
    public_url: Option<&str>,
    now: DateTime<Utc>,
) -> Value {
    json!({
        "schema": "v3.static_page_artifact_stability",
        "schemaVersion": 1,
        "dataset_artifact_key": dataset_artifact_key,
        "datasetArtifactKey": dataset_artifact_key,
        "baseline_status": baseline_status,
        "baselineStatus": baseline_status,
        "reuse_policy": "reuse_accepted_baseline_unless_explicit_redesign",
        "reusePolicy": "reuse_accepted_baseline_unless_explicit_redesign",
        "style_reuse_policy": "reuse_style_unless_explicit_redesign",
        "styleReusePolicy": "reuse_style_unless_explicit_redesign",
        "default_template_scope": "dataset_combination",
        "defaultTemplateScope": "dataset_combination",
        "edit_mode": "incremental_existing_artifact",
        "editMode": "incremental_existing_artifact",
        "image2_reuse_policy": "skip_when_accepted_baseline_exists",
        "image2ReusePolicy": "skip_when_accepted_baseline_exists",
        "data_refresh_policy": "refresh_data_files_from_dataset_sources",
        "dataRefreshPolicy": "refresh_data_files_from_dataset_sources",
        "public_url": public_url,
        "publicUrl": public_url,
        "updated_at": now,
        "updatedAt": now,
    })
}

pub(crate) fn static_page_source_refs_template_binding_eligible(source_refs: &Value) -> bool {
    [
        source_refs.get("template_binding_eligible"),
        source_refs.get("templateBindingEligible"),
        source_refs.pointer("/artifact_stability/template_binding_eligible"),
        source_refs.pointer("/artifact_stability/templateBindingEligible"),
    ]
    .into_iter()
    .flatten()
    .any(|value| value.as_bool().unwrap_or(false))
        || [
            source_refs.get("artifact_role"),
            source_refs.get("artifactRole"),
            source_refs.pointer("/artifact_stability/artifact_role"),
            source_refs.pointer("/artifact_stability/artifactRole"),
        ]
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::trim)
        .any(|value| matches!(value, "template" | "default_template" | "report_template"))
}

pub(crate) fn apply_static_page_artifact_stability_to_source_refs(
    mut source_refs: Value,
    dataset_artifact_key: Option<&str>,
    baseline_status: &str,
    public_url: Option<&str>,
    now: DateTime<Utc>,
) -> Value {
    let Some(dataset_artifact_key) = dataset_artifact_key else {
        return source_refs;
    };
    ensure_json_object(&mut source_refs);
    if let Some(object) = source_refs.as_object_mut() {
        object.insert(
            "dataset_artifact_key".to_string(),
            json!(dataset_artifact_key),
        );
        object.insert(
            "artifact_stability".to_string(),
            static_page_artifact_stability_metadata(
                dataset_artifact_key,
                baseline_status,
                public_url,
                now,
            ),
        );
    }
    source_refs
}

pub(crate) fn apply_static_page_artifact_stability_to_payload(
    mut payload: Value,
    dataset_artifact_key: Option<&str>,
    baseline_status: &str,
    public_url: Option<&str>,
    now: DateTime<Utc>,
) -> Value {
    let Some(dataset_artifact_key) = dataset_artifact_key else {
        return payload;
    };
    ensure_json_object(&mut payload);
    if let Some(object) = payload.as_object_mut() {
        let metadata = static_page_artifact_stability_metadata(
            dataset_artifact_key,
            baseline_status,
            public_url,
            now,
        );
        object.insert("artifactStability".to_string(), metadata.clone());
        object.insert("artifact_stability".to_string(), metadata.clone());
        object.insert(
            "datasetArtifactKey".to_string(),
            json!(dataset_artifact_key),
        );
        if public_url.is_some() || object.contains_key("finalPage") {
            let final_page = object
                .entry("finalPage".to_string())
                .or_insert_with(|| Value::Object(Map::new()));
            ensure_json_object(final_page);
            if let Some(final_page_object) = final_page.as_object_mut() {
                final_page_object.insert(
                    "datasetArtifactKey".to_string(),
                    json!(dataset_artifact_key),
                );
                final_page_object.insert("baselineStatus".to_string(), json!(baseline_status));
                if let Some(public_url) = public_url {
                    final_page_object.insert("publicUrl".to_string(), json!(public_url));
                    final_page_object.insert("public_url".to_string(), json!(public_url));
                    final_page_object.insert("generatedArtifactUrl".to_string(), json!(public_url));
                    final_page_object
                        .insert("generated_artifact_url".to_string(), json!(public_url));
                }
            }
        }
    }
    payload
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn fixed_time() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 6, 16, 8, 30, 0).unwrap()
    }

    #[test]
    fn artifact_stability_metadata_writes_snake_and_camel_case_contract() {
        let metadata = static_page_artifact_stability_metadata(
            "key-001",
            "accepted",
            Some("https://v3.elepcloud.com/generated-artifacts/a/index.html"),
            fixed_time(),
        );

        assert_eq!(metadata["schema"], "v3.static_page_artifact_stability");
        assert_eq!(metadata["schemaVersion"], 1);
        assert_eq!(metadata["dataset_artifact_key"], "key-001");
        assert_eq!(metadata["datasetArtifactKey"], "key-001");
        assert_eq!(metadata["baseline_status"], "accepted");
        assert_eq!(metadata["baselineStatus"], "accepted");
        assert_eq!(
            metadata["reusePolicy"],
            "reuse_accepted_baseline_unless_explicit_redesign"
        );
        assert_eq!(
            metadata["style_reuse_policy"],
            "reuse_style_unless_explicit_redesign"
        );
        assert_eq!(metadata["defaultTemplateScope"], "dataset_combination");
        assert_eq!(metadata["edit_mode"], "incremental_existing_artifact");
        assert_eq!(
            metadata["image2ReusePolicy"],
            "skip_when_accepted_baseline_exists"
        );
        assert_eq!(
            metadata["data_refresh_policy"],
            "refresh_data_files_from_dataset_sources"
        );
        assert_eq!(
            metadata["publicUrl"],
            "https://v3.elepcloud.com/generated-artifacts/a/index.html"
        );
        assert_eq!(metadata["updatedAt"], json!(fixed_time()));
    }

    #[test]
    fn source_refs_template_binding_eligible_accepts_flags_and_roles() {
        assert!(static_page_source_refs_template_binding_eligible(&json!({
            "templateBindingEligible": true
        })));
        assert!(static_page_source_refs_template_binding_eligible(&json!({
            "artifact_stability": {
                "artifact_role": " report_template "
            }
        })));
        assert!(!static_page_source_refs_template_binding_eligible(&json!({
            "artifact_role": "ordinary_report",
            "artifact_stability": {
                "template_binding_eligible": false
            }
        })));
    }

    #[test]
    fn apply_artifact_stability_to_source_refs_preserves_none_and_writes_metadata() {
        assert_eq!(
            apply_static_page_artifact_stability_to_source_refs(
                json!({"keep": true}),
                None,
                "accepted",
                None,
                fixed_time(),
            ),
            json!({"keep": true})
        );

        let source_refs = apply_static_page_artifact_stability_to_source_refs(
            json!({"keep": true}),
            Some("key-001"),
            "accepted",
            Some("https://v3.elepcloud.com/generated-artifacts/a/index.html"),
            fixed_time(),
        );

        assert_eq!(source_refs["keep"], true);
        assert_eq!(source_refs["dataset_artifact_key"], "key-001");
        assert_eq!(
            source_refs["artifact_stability"]["datasetArtifactKey"],
            "key-001"
        );
        assert_eq!(
            source_refs["artifact_stability"]["baseline_status"],
            "accepted"
        );
        assert_eq!(
            source_refs["artifact_stability"]["public_url"],
            "https://v3.elepcloud.com/generated-artifacts/a/index.html"
        );
    }

    #[test]
    fn apply_artifact_stability_to_payload_updates_metadata_and_final_page() {
        let payload = apply_static_page_artifact_stability_to_payload(
            json!({"finalPage": {"existing": "value"}}),
            Some("key-001"),
            "accepted",
            Some("https://v3.elepcloud.com/generated-artifacts/a/index.html"),
            fixed_time(),
        );

        assert_eq!(payload["datasetArtifactKey"], "key-001");
        assert_eq!(
            payload["artifactStability"]["dataset_artifact_key"],
            "key-001"
        );
        assert_eq!(payload["artifact_stability"]["baselineStatus"], "accepted");
        assert_eq!(payload["finalPage"]["existing"], "value");
        assert_eq!(payload["finalPage"]["datasetArtifactKey"], "key-001");
        assert_eq!(payload["finalPage"]["baselineStatus"], "accepted");
        assert_eq!(
            payload["finalPage"]["generated_artifact_url"],
            "https://v3.elepcloud.com/generated-artifacts/a/index.html"
        );
    }

    #[test]
    fn apply_artifact_stability_to_payload_does_not_create_final_page_without_url() {
        let payload = apply_static_page_artifact_stability_to_payload(
            json!({"title": "report"}),
            Some("key-001"),
            "accepted",
            None,
            fixed_time(),
        );

        assert_eq!(payload["datasetArtifactKey"], "key-001");
        assert!(payload.get("finalPage").is_none());
    }
}
