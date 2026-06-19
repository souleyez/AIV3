use std::collections::BTreeSet;

use domain_model::{AssistantRunEvent, AssistantRunId, WorkflowExecution};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    assistant_run_customer_codex_artifact_support::assistant_run_customer_codex_output_artifacts_from_events,
    value_array, workflow_context_support::workflow_context_uuid, ApiError, AppState,
};

pub(crate) async fn load_workflow_runtime_artifact_manifests(
    state: &AppState,
    execution: &WorkflowExecution,
) -> std::result::Result<Vec<Value>, ApiError> {
    let Some(run_id) = workflow_runtime_assistant_run_id(&execution.context) else {
        return Ok(Vec::new());
    };
    let Some(run) = state
        .storage
        .assistant_runs()
        .get_by_id(state.tenant_id, run_id)
        .await
        .map_err(ApiError::from_storage)?
    else {
        return Ok(Vec::new());
    };
    let events = state
        .storage
        .assistant_runs()
        .list_events(state.tenant_id, run_id)
        .await
        .map_err(ApiError::from_storage)?;
    Ok(collect_workflow_runtime_artifact_manifests(
        &run.output_artifacts,
        &events,
    ))
}

pub(crate) fn workflow_runtime_assistant_run_id(context: &Value) -> Option<AssistantRunId> {
    workflow_context_uuid(context, "assistant_run_id")
        .or_else(|| {
            context
                .pointer("/fixed_task/assistant_run_id")
                .and_then(Value::as_str)
                .and_then(|raw| Uuid::parse_str(raw).ok())
        })
        .map(AssistantRunId)
}

pub(crate) fn output_artifact_manifests_from_output_artifacts(
    output_artifacts: &Value,
) -> Vec<Value> {
    dedupe_output_artifact_manifests(
        value_array(output_artifacts.clone())
            .into_iter()
            .filter_map(|artifact| safe_output_artifact_manifest(&artifact))
            .collect(),
    )
}

pub(crate) fn dedupe_output_artifact_manifests(manifests: Vec<Value>) -> Vec<Value> {
    let mut seen = BTreeSet::new();
    manifests
        .into_iter()
        .filter(|manifest| {
            let key = serde_json::to_string(manifest).unwrap_or_default();
            seen.insert(key)
        })
        .collect()
}

pub(crate) fn safe_output_artifact_manifest(artifact: &Value) -> Option<Value> {
    let manifest = artifact
        .get("artifact_manifest")
        .or_else(|| artifact.get("artifactManifest"))?;
    let schema = manifest.get("schema").and_then(Value::as_str)?;
    let schema_version = manifest
        .get("schema_version")
        .or_else(|| manifest.get("schemaVersion"))
        .and_then(Value::as_i64)?;
    if schema != "v3.output_artifact_manifest" || schema_version != 1 {
        return None;
    }
    Some(json!({
        "schema": "v3.output_artifact_manifest",
        "schema_version": 1,
        "artifact_type": manifest.get("artifact_type").cloned().unwrap_or(Value::Null),
        "artifact_kind": manifest.get("artifact_kind").cloned().unwrap_or(Value::Null),
        "title": manifest.get("title").cloned().unwrap_or(Value::Null),
        "status": manifest.get("status").cloned().unwrap_or(Value::Null),
        "primary_url": manifest.get("primary_url").cloned().unwrap_or(Value::Null),
        "links": manifest.get("links").cloned().unwrap_or_else(|| json!([])),
        "refs": manifest.get("refs").cloned().unwrap_or_else(|| json!({})),
        "safety": manifest.get("safety").cloned().unwrap_or_else(|| json!({})),
    }))
}

fn collect_workflow_runtime_artifact_manifests(
    output_artifacts: &Value,
    events: &[AssistantRunEvent],
) -> Vec<Value> {
    let mut manifests = output_artifact_manifests_from_output_artifacts(output_artifacts);
    let event_output_artifacts = assistant_run_customer_codex_output_artifacts_from_events(events);
    manifests.extend(
        events
            .iter()
            .filter_map(|event| safe_output_artifact_manifest(&event.payload)),
    );
    manifests.extend(
        event_output_artifacts
            .iter()
            .filter_map(safe_output_artifact_manifest),
    );
    dedupe_output_artifact_manifests(manifests)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{AssistantRunEventId, TenantId};
    use serde_json::json;

    #[test]
    fn workflow_runtime_assistant_run_id_reads_direct_and_fixed_task_contexts() {
        let direct_run_id = AssistantRunId::new();
        let fixed_task_run_id = AssistantRunId::new();

        assert_eq!(
            workflow_runtime_assistant_run_id(&json!({
                "assistant_run_id": direct_run_id.to_string()
            })),
            Some(direct_run_id)
        );
        assert_eq!(
            workflow_runtime_assistant_run_id(&json!({
                "fixed_task": {
                    "assistant_run_id": fixed_task_run_id.to_string()
                }
            })),
            Some(fixed_task_run_id)
        );
        assert_eq!(
            workflow_runtime_assistant_run_id(&json!({
                "assistant_run_id": "not-a-uuid",
                "fixed_task": {
                    "assistant_run_id": "also-not-a-uuid"
                }
            })),
            None
        );
    }

    #[test]
    fn safe_output_artifact_manifest_keeps_only_allowlisted_fields() {
        let artifact = json!({
            "artifactManifest": {
                "schema": "v3.output_artifact_manifest",
                "schemaVersion": 1,
                "artifact_type": "static_page",
                "artifact_kind": "generated_artifact",
                "title": "Generated report",
                "status": "published",
                "primary_url": "https://v3.elepcloud.com/generated-artifacts/a/index.html",
                "raw_log": "must-not-leak"
            }
        });

        let manifest =
            safe_output_artifact_manifest(&artifact).expect("valid manifest should be accepted");

        assert_eq!(manifest["schema"], json!("v3.output_artifact_manifest"));
        assert_eq!(manifest["schema_version"], json!(1));
        assert_eq!(manifest["artifact_type"], json!("static_page"));
        assert_eq!(manifest["links"], json!([]));
        assert_eq!(manifest["refs"], json!({}));
        assert_eq!(manifest["safety"], json!({}));
        assert!(manifest.get("raw_log").is_none());
    }

    #[test]
    fn output_artifact_manifests_from_output_artifacts_returns_only_safe_manifests() {
        let output_artifacts = json!([
            {
                "type": "external_channel_static_page_artifact",
                "artifact_manifest": {
                    "schema": "v3.output_artifact_manifest",
                    "schema_version": 1,
                    "artifact_type": "static_page",
                    "artifact_kind": "generated_artifact",
                    "primary_url": "https://v3.elepcloud.com/generated-artifacts/a/index.html",
                    "safety": {
                        "credentials_exposed": false,
                        "raw_logs_exposed": false
                    },
                    "raw_log": "must-not-leak"
                }
            },
            {
                "type": "external_channel_data_ingestion_analysis",
                "artifactManifest": {
                    "schema": "v3.output_artifact_manifest",
                    "schema_version": 1,
                    "artifact_type": "data_ingestion_analysis",
                    "artifact_kind": "data_ingestion_staging_plan",
                    "safety": {
                        "production_write_allowed": false,
                        "raw_table_dump_exposed": false
                    }
                }
            },
            {
                "artifact_manifest": {
                    "schema": "legacy",
                    "schema_version": 1,
                    "artifact_type": "raw_log"
                }
            },
            {
                "artifact_manifest": {
                    "schema": "v3.output_artifact_manifest",
                    "schema_version": 2,
                    "artifact_type": "future"
                }
            },
            {
                "artifact_manifest": {
                    "schema": "v3.output_artifact_manifest",
                    "schema_version": 1,
                    "artifact_type": "static_page",
                    "artifact_kind": "generated_artifact",
                    "primary_url": "https://v3.elepcloud.com/generated-artifacts/a/index.html",
                    "safety": {
                        "credentials_exposed": false,
                        "raw_logs_exposed": false
                    }
                }
            }
        ]);

        let manifests = output_artifact_manifests_from_output_artifacts(&output_artifacts);

        assert_eq!(manifests.len(), 2);
        assert_eq!(manifests[0]["artifact_type"], json!("static_page"));
        assert!(manifests[0].get("raw_log").is_none());
        assert_eq!(
            manifests[1]["artifact_kind"],
            json!("data_ingestion_staging_plan")
        );
    }

    #[test]
    fn dedupe_output_artifact_manifests_preserves_first_seen_order() {
        let first =
            json!({"schema": "v3.output_artifact_manifest", "schema_version": 1, "title": "A"});
        let second =
            json!({"schema": "v3.output_artifact_manifest", "schema_version": 1, "title": "B"});

        let deduped =
            dedupe_output_artifact_manifests(vec![first.clone(), second.clone(), first.clone()]);

        assert_eq!(deduped, vec![first, second]);
    }

    #[test]
    fn collect_workflow_runtime_artifact_manifests_merges_run_event_and_customer_artifacts() {
        let run_id = AssistantRunId::new();
        let run_manifest = json!({
            "artifact_manifest": {
                "schema": "v3.output_artifact_manifest",
                "schema_version": 1,
                "artifact_type": "static_page",
                "artifact_kind": "generated_artifact",
                "title": "Run artifact"
            }
        });
        let event_manifest = json!({
            "artifact_manifest": {
                "schema": "v3.output_artifact_manifest",
                "schema_version": 1,
                "artifact_type": "data_ingestion_analysis",
                "artifact_kind": "data_ingestion_staging_plan",
                "title": "Event artifact"
            }
        });
        let events = vec![
            AssistantRunEvent {
                id: AssistantRunEventId::new(),
                tenant_id: TenantId::new(),
                run_id,
                sequence_no: 1,
                event_name: "assistant_run.some_runtime_artifact".to_string(),
                payload: event_manifest.clone(),
                created_at: Utc::now(),
            },
            AssistantRunEvent {
                id: AssistantRunEventId::new(),
                tenant_id: TenantId::new(),
                run_id,
                sequence_no: 2,
                event_name: crate::ASSISTANT_RUN_GENERATED_STATIC_PAGE_EDIT_ARTIFACTS_READY_EVENT
                    .to_string(),
                payload: json!({
                    "source": "codex_host_customer_artifacts",
                    "workflow_execution_id": uuid::Uuid::new_v4().to_string(),
                    "assistant_run_id": run_id.to_string(),
                    "capability": crate::CODEX_CAPABILITY_GENERATED_STATIC_PAGE_EDIT,
                    "route": "generated_static_page_edit",
                    "status": "available",
                    "customer_artifacts": {
                        "schema": "v3.customer_codex_artifacts",
                        "version": 1,
                        "status": "available",
                        "capability": crate::CODEX_CAPABILITY_GENERATED_STATIC_PAGE_EDIT,
                        "manifest_path": "generated-artifacts/manifest.json",
                        "artifact_count": 1,
                        "artifacts": [{
                            "path": "generated-artifacts/index.html",
                            "title": "Revised page",
                            "kind": "html",
                            "mime_type": "text/html",
                            "bytes": 2048,
                            "sha256": "abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd"
                        }]
                    }
                }),
                created_at: Utc::now(),
            },
        ];

        let manifests = collect_workflow_runtime_artifact_manifests(
            &Value::Array(vec![run_manifest.clone(), run_manifest]),
            &events,
        );

        assert_eq!(manifests.len(), 3);
        assert_eq!(manifests[0]["artifact_type"], json!("static_page"));
        assert_eq!(
            manifests[1]["artifact_type"],
            json!("data_ingestion_analysis")
        );
        assert_eq!(
            manifests[2]["artifact_type"],
            json!("codex_customer_artifacts")
        );
    }
}
