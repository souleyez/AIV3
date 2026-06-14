use contracts::{StaticPageDraftStatusView, UpdateStaticPageDraftRequest};
use serde_json::{json, Value};

fn static_page_update_value_baseline_status(value: &Value) -> Option<&str> {
    [
        value.pointer("/artifact_stability/baseline_status"),
        value.pointer("/artifact_stability/baselineStatus"),
        value.pointer("/artifactStability/baselineStatus"),
        value.pointer("/artifactStability/baseline_status"),
        value.pointer("/finalPage/baselineStatus"),
        value.pointer("/finalPage/baseline_status"),
        value.pointer("/final_page/baselineStatus"),
        value.pointer("/final_page/baseline_status"),
    ]
    .into_iter()
    .flatten()
    .filter_map(Value::as_str)
    .map(str::trim)
    .find(|value| !value.is_empty())
}

fn static_page_update_value_status(value: &Value) -> Option<&str> {
    [
        value.pointer("/status"),
        value.pointer("/backendStatus"),
        value.pointer("/backend_status"),
    ]
    .into_iter()
    .flatten()
    .filter_map(Value::as_str)
    .map(str::trim)
    .find(|value| !value.is_empty())
}

fn static_page_update_report_shelf_defaults(value: &Value) -> Option<&Value> {
    [
        value.pointer("/artifact_stability/report_shelf_defaults"),
        value.pointer("/artifact_stability/reportShelfDefaults"),
        value.pointer("/artifactStability/reportShelfDefaults"),
        value.pointer("/artifactStability/report_shelf_defaults"),
        value.pointer("/report_shelf_defaults"),
        value.pointer("/reportShelfDefaults"),
    ]
    .into_iter()
    .flatten()
    .find(|value| value.as_object().is_some_and(|object| !object.is_empty()))
}

fn static_page_report_shelf_defaults_value_is_safe(value: &Value) -> bool {
    let Some(defaults) = value.as_object() else {
        return false;
    };
    !defaults.is_empty()
        && defaults.iter().all(|(dataset_id, enabled)| {
            let dataset_id = dataset_id.trim();
            !dataset_id.is_empty()
                && (enabled.is_boolean()
                    || enabled.as_str().is_some_and(|value| {
                        matches!(
                            value.trim().to_ascii_lowercase().as_str(),
                            "true"
                                | "false"
                                | "1"
                                | "0"
                                | "yes"
                                | "no"
                                | "default"
                                | "not_default"
                                | "non_default"
                                | "accepted"
                                | "retired"
                                | "enabled"
                                | "disabled"
                        )
                    }))
        })
}

pub(crate) fn static_page_public_template_default_update_is_safe(
    request: &UpdateStaticPageDraftRequest,
) -> bool {
    if request.title.is_some()
        || request.status.is_some()
        || request.selected_scope.is_some()
        || request.visibility_snapshot.is_some()
        || request.draft_payload.is_some()
    {
        return false;
    }
    let Some(source_refs) = request.source_refs.as_ref() else {
        return false;
    };
    let baseline_status_safe =
        static_page_update_value_baseline_status(source_refs).is_none_or(|status| {
            matches!(
                status.trim().to_ascii_lowercase().as_str(),
                "accepted" | "retired"
            )
        });
    baseline_status_safe
        && static_page_update_report_shelf_defaults(source_refs)
            .is_some_and(static_page_report_shelf_defaults_value_is_safe)
}

pub(crate) fn merge_public_template_default_source_refs(
    mut current: Value,
    requested: &Value,
) -> Value {
    let Some(defaults) = static_page_update_report_shelf_defaults(requested).cloned() else {
        return current;
    };
    if !current.is_object() {
        current = json!({});
    }
    let object = current.as_object_mut().expect("source_refs is object");
    let stability_entry = object
        .entry("artifact_stability".to_string())
        .or_insert_with(|| json!({}));
    if !stability_entry.is_object() {
        *stability_entry = json!({});
    }
    let stability = stability_entry
        .as_object_mut()
        .expect("artifact_stability is object");
    stability.insert("report_shelf_defaults".to_string(), defaults.clone());
    stability.insert("reportShelfDefaults".to_string(), defaults);
    if static_page_update_value_baseline_status(requested)
        .is_some_and(|status| status.trim().eq_ignore_ascii_case("accepted"))
    {
        stability.insert("baseline_status".to_string(), json!("accepted"));
        stability.insert("baselineStatus".to_string(), json!("accepted"));
    }
    let camel_stability = Value::Object(stability.clone());
    object.insert("artifactStability".to_string(), camel_stability);
    current
}

pub(crate) fn static_page_public_template_update_is_safe(
    request: &UpdateStaticPageDraftRequest,
) -> bool {
    if request.title.is_some()
        || request.selected_scope.is_some()
        || request.visibility_snapshot.is_some()
    {
        return false;
    }
    let archives = request
        .status
        .as_ref()
        .is_some_and(|status| status == &StaticPageDraftStatusView::Archived)
        || request
            .draft_payload
            .as_ref()
            .and_then(static_page_update_value_status)
            .is_some_and(|status| status == "archived");
    let retires_template = request
        .source_refs
        .as_ref()
        .and_then(static_page_update_value_baseline_status)
        .is_some_and(|status| status == "retired")
        || request
            .draft_payload
            .as_ref()
            .and_then(static_page_update_value_baseline_status)
            .is_some_and(|status| status == "retired");
    archives || retires_template
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_update_allows_only_safe_report_shelf_defaults() {
        let request = UpdateStaticPageDraftRequest {
            source_refs: Some(json!({
                "artifact_stability": {
                    "baseline_status": "accepted",
                    "report_shelf_defaults": {
                        "31588c60-0885-47c4-81fe-4ff5c27de8e7": "default",
                        "dataset-b": false
                    }
                }
            })),
            ..Default::default()
        };

        assert!(static_page_public_template_default_update_is_safe(&request));
    }

    #[test]
    fn default_update_rejects_payload_changes_and_unsafe_defaults() {
        let payload_change = UpdateStaticPageDraftRequest {
            draft_payload: Some(json!({ "status": "rendered" })),
            source_refs: Some(json!({
                "reportShelfDefaults": {
                    "dataset-a": true
                }
            })),
            ..Default::default()
        };
        let unsafe_defaults = UpdateStaticPageDraftRequest {
            source_refs: Some(json!({
                "reportShelfDefaults": {
                    "": true,
                    "dataset-a": "delete"
                }
            })),
            ..Default::default()
        };

        assert!(!static_page_public_template_default_update_is_safe(
            &payload_change
        ));
        assert!(!static_page_public_template_default_update_is_safe(
            &unsafe_defaults
        ));
    }

    #[test]
    fn merge_default_source_refs_writes_snake_and_camel_stability_fields() {
        let merged = merge_public_template_default_source_refs(
            json!({
                "artifact_stability": {
                    "dataset_artifact_key": "v3-static-page|template:generated-static-page:fixture"
                },
                "local_thread_id": "thread-1"
            }),
            &json!({
                "artifactStability": {
                    "baselineStatus": "accepted",
                    "reportShelfDefaults": {
                        "dataset-a": true
                    }
                }
            }),
        );

        assert_eq!(
            merged.pointer("/artifact_stability/baseline_status"),
            Some(&json!("accepted"))
        );
        assert_eq!(
            merged.pointer("/artifactStability/reportShelfDefaults/dataset-a"),
            Some(&json!(true))
        );
        assert_eq!(merged.pointer("/local_thread_id"), Some(&json!("thread-1")));
    }

    #[test]
    fn public_template_update_allows_archive_or_retire_only() {
        let archive_request = UpdateStaticPageDraftRequest {
            status: Some(StaticPageDraftStatusView::Archived),
            ..Default::default()
        };
        let retire_request = UpdateStaticPageDraftRequest {
            draft_payload: Some(json!({
                "artifactStability": {
                    "baselineStatus": "retired"
                }
            })),
            ..Default::default()
        };
        let unsafe_request = UpdateStaticPageDraftRequest {
            title: Some("rewrite public template".to_string()),
            source_refs: Some(json!({
                "artifact_stability": {
                    "baseline_status": "retired"
                }
            })),
            ..Default::default()
        };

        assert!(static_page_public_template_update_is_safe(&archive_request));
        assert!(static_page_public_template_update_is_safe(&retire_request));
        assert!(!static_page_public_template_update_is_safe(&unsafe_request));
    }
}
