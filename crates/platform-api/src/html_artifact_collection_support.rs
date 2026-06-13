use std::collections::HashSet;

use contracts::HtmlArtifactManifestView;
use domain_model::{AssistantRunEvent, HtmlArtifact};
use serde_json::Value;

pub(crate) fn collect_html_artifacts_from_events(
    events: &[AssistantRunEvent],
    artifacts: &mut Vec<HtmlArtifactManifestView>,
    limit: usize,
) {
    for event in events.iter().rev() {
        collect_html_artifacts_from_value(&event.payload, artifacts, limit);
        if artifacts.len() >= limit {
            break;
        }
    }
}

pub(crate) fn collect_html_artifacts_from_records(
    records: &[HtmlArtifact],
    artifacts: &mut Vec<HtmlArtifactManifestView>,
    limit: usize,
) {
    for record in records {
        if artifacts.len() >= limit {
            break;
        }
        if let Ok(manifest) =
            serde_json::from_value::<HtmlArtifactManifestView>(record.manifest.clone())
        {
            artifacts.push(manifest);
        }
    }
}

fn collect_html_artifacts_from_value(
    value: &Value,
    artifacts: &mut Vec<HtmlArtifactManifestView>,
    limit: usize,
) {
    if artifacts.len() >= limit {
        return;
    }

    match value {
        Value::Object(object) => {
            if let Some(entries) = object.get("html_artifacts").and_then(Value::as_array) {
                for entry in entries {
                    if artifacts.len() >= limit {
                        break;
                    }
                    if let Ok(manifest) =
                        serde_json::from_value::<HtmlArtifactManifestView>(entry.clone())
                    {
                        artifacts.push(manifest);
                    }
                }
            }

            for (key, child) in object {
                if key == "html_artifacts" {
                    continue;
                }
                collect_html_artifacts_from_value(child, artifacts, limit);
                if artifacts.len() >= limit {
                    break;
                }
            }
        }
        Value::Array(entries) => {
            for entry in entries {
                collect_html_artifacts_from_value(entry, artifacts, limit);
                if artifacts.len() >= limit {
                    break;
                }
            }
        }
        _ => {}
    }
}

pub(crate) fn sort_and_dedupe_html_artifacts(artifacts: &mut Vec<HtmlArtifactManifestView>) {
    artifacts.sort_by(|left, right| {
        right
            .created_at
            .cmp(&left.created_at)
            .then_with(|| left.id.cmp(&right.id))
    });
    let mut seen = HashSet::<String>::new();
    artifacts.retain(|artifact| seen.insert(artifact.id.clone()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use contracts::{
        HtmlArtifactInteractionModeView, HtmlArtifactOwnerScopeView, HtmlArtifactProvenanceView,
        HtmlArtifactSourceTypeView, HtmlArtifactTemplateIdView,
    };
    use serde_json::json;

    fn manifest(id: &str, created_offset_seconds: i64) -> HtmlArtifactManifestView {
        HtmlArtifactManifestView {
            kind: "html_artifact".to_string(),
            version: 1,
            id: id.to_string(),
            title: id.to_string(),
            source_type: HtmlArtifactSourceTypeView::Manual,
            template_id: HtmlArtifactTemplateIdView::CodeReviewSummary,
            owner_scope: HtmlArtifactOwnerScopeView {
                scope_type: "assistant_run".to_string(),
                id: "run-1".to_string(),
            },
            data_refs: Vec::new(),
            provenance: HtmlArtifactProvenanceView {
                producer: "test".to_string(),
                reason: "unit_test".to_string(),
                source_run_id: None,
            },
            interaction_mode: HtmlArtifactInteractionModeView::ReadOnly,
            created_at: Utc::now() + Duration::seconds(created_offset_seconds),
            payload: json!({}),
        }
    }

    #[test]
    fn collect_from_value_keeps_existing_recursive_order_and_limit() {
        let first = manifest("artifact-first", 0);
        let second = manifest("artifact-second", 1);
        let nested = manifest("artifact-nested", 2);
        let payload = json!({
            "html_artifacts": [
                serde_json::to_value(&first).expect("serializes"),
                json!({"not": "a manifest"}),
                serde_json::to_value(&second).expect("serializes")
            ],
            "nested": {
                "items": [{
                    "html_artifacts": [
                        serde_json::to_value(&nested).expect("serializes")
                    ]
                }]
            }
        });
        let mut artifacts = Vec::new();

        collect_html_artifacts_from_value(&payload, &mut artifacts, 2);

        assert_eq!(
            artifacts
                .iter()
                .map(|artifact| artifact.id.as_str())
                .collect::<Vec<_>>(),
            vec!["artifact-first", "artifact-second"]
        );
    }

    #[test]
    fn sort_and_dedupe_keeps_newest_per_id_and_sorts_by_created_desc() {
        let duplicate_old = manifest("artifact-same", 0);
        let newest = manifest("artifact-newest", 10);
        let duplicate_new = manifest("artifact-same", 20);
        let mut artifacts = vec![duplicate_old, newest, duplicate_new];

        sort_and_dedupe_html_artifacts(&mut artifacts);

        assert_eq!(
            artifacts
                .iter()
                .map(|artifact| artifact.id.as_str())
                .collect::<Vec<_>>(),
            vec!["artifact-same", "artifact-newest"]
        );
    }
}
