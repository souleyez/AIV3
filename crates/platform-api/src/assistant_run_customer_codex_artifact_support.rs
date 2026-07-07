use std::collections::BTreeSet;

use contracts::{
    CodexHostFixedTaskHumanReviewPolicyView, CodexHostFixedTaskTemplateContextView,
    CodexHostFixedTaskTemplateIdView,
};
use domain_model::{AssistantRunEvent, AssistantRunId, TenantId, WorkflowExecutionId};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::static_page_revision_artifact_support::static_page_public_url_from_current_artifact;
use crate::{
    assistant_run_current_artifact_brief,
    assistant_run_evidence_state_support::assistant_run_evidence_supplied_count,
    assistant_run_scope_policy_support::assistant_run_scope_intent,
    assistant_run_scope_selection_support::{
        selected_dataset_ids_from_scope, selected_document_ids_from_scope,
    },
    assistant_run_text_support::truncate_assistant_supply_text,
    env_csv_contains, external_channel_data_ingestion_scope_has_source,
    external_channel_generated_artifact_public_base_url, platform_env_flag,
    selected_string_ids_from_scope, ASSISTANT_RUN_CUSTOMER_ARTIFACTS_READY_EVENT,
    ASSISTANT_RUN_GENERATED_STATIC_PAGE_EDIT_ARTIFACTS_READY_EVENT,
    CODEX_CAPABILITY_CUSTOMER_ARTIFACT_REQUEST, CODEX_CAPABILITY_CUSTOMER_COMPLEX_REQUEST,
    CODEX_CAPABILITY_DATA_INGESTION_ANALYSIS, CODEX_CAPABILITY_GENERATED_STATIC_PAGE_EDIT,
    CODEX_CAPABILITY_GENERATED_STATIC_PAGE_PUBLISH, CODEX_CAPABILITY_V3_PRODUCT_CHANGE_REQUEST,
};

pub(crate) fn assistant_run_append_deduped_output_artifacts(
    output_artifacts: Vec<Value>,
    additional_artifacts: Vec<Value>,
) -> Vec<Value> {
    let mut seen = BTreeSet::new();
    output_artifacts
        .into_iter()
        .chain(additional_artifacts)
        .filter(|artifact| {
            let key = serde_json::to_string(artifact).unwrap_or_default();
            seen.insert(key)
        })
        .collect()
}

pub(crate) fn assistant_run_customer_codex_output_artifacts_from_events(
    events: &[AssistantRunEvent],
) -> Vec<Value> {
    assistant_run_append_deduped_output_artifacts(
        Vec::new(),
        events
            .iter()
            .filter_map(assistant_run_customer_codex_output_artifact_from_ready_event)
            .collect(),
    )
}

fn assistant_run_customer_codex_output_artifact_from_ready_event(
    event: &AssistantRunEvent,
) -> Option<Value> {
    let event_name = event.event_name.as_str();
    if !matches!(
        event_name,
        ASSISTANT_RUN_CUSTOMER_ARTIFACTS_READY_EVENT
            | ASSISTANT_RUN_GENERATED_STATIC_PAGE_EDIT_ARTIFACTS_READY_EVENT
    ) {
        return None;
    }
    let payload = &event.payload;
    if payload.get("source").and_then(Value::as_str) != Some("codex_host_customer_artifacts") {
        return None;
    }
    let mut customer_artifacts =
        assistant_run_safe_customer_codex_artifacts(payload.get("customer_artifacts")?)?;
    let artifacts = customer_artifacts
        .get("artifacts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if artifacts.is_empty() {
        return None;
    }
    let artifact_paths = artifacts
        .iter()
        .filter_map(|artifact| artifact.get("path").and_then(Value::as_str))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let default_capability =
        if event_name == ASSISTANT_RUN_GENERATED_STATIC_PAGE_EDIT_ARTIFACTS_READY_EVENT {
            CODEX_CAPABILITY_GENERATED_STATIC_PAGE_EDIT
        } else {
            CODEX_CAPABILITY_CUSTOMER_ARTIFACT_REQUEST
        };
    let capability = payload
        .get("capability")
        .and_then(Value::as_str)
        .and_then(assistant_run_safe_customer_codex_artifact_capability)
        .unwrap_or_else(|| default_capability.to_string());
    let default_route =
        assistant_run_customer_codex_default_route_for_capability(event_name, &capability);
    let route = payload
        .get("route")
        .and_then(Value::as_str)
        .and_then(assistant_run_safe_customer_codex_artifact_route)
        .unwrap_or(default_route);
    if let Some(object) = customer_artifacts.as_object_mut() {
        object.insert("capability".to_string(), json!(capability.clone()));
    }
    let status = assistant_run_safe_customer_codex_text(
        payload
            .get("status")
            .or_else(|| customer_artifacts.get("status"))
            .and_then(Value::as_str)
            .unwrap_or("available"),
        40,
    );
    let workflow_execution_id = assistant_run_safe_uuid_string(payload, "workflow_execution_id");
    let title = customer_artifacts
        .get("title")
        .and_then(Value::as_str)
        .map(|value| assistant_run_safe_customer_codex_display_text(value, 160))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            if event_name == ASSISTANT_RUN_GENERATED_STATIC_PAGE_EDIT_ARTIFACTS_READY_EVENT {
                "Codex generated page edit artifacts".to_string()
            } else {
                "Codex customer artifacts".to_string()
            }
        });
    let manifest_path = customer_artifacts
        .get("manifest_path")
        .and_then(Value::as_str)
        .and_then(assistant_run_safe_customer_codex_relative_path);
    let artifact_count = artifacts.len();
    let primary_url = customer_artifacts
        .get("primary_url")
        .or_else(|| customer_artifacts.get("public_url"))
        .and_then(Value::as_str)
        .and_then(assistant_run_safe_customer_codex_generated_artifact_url);
    let published = customer_artifacts
        .get("published")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && primary_url.is_some();
    let status = if published {
        "published".to_string()
    } else {
        status
    };
    let links =
        assistant_run_customer_codex_artifact_links(&customer_artifacts, primary_url.as_deref());
    let primary_url_value = primary_url
        .as_ref()
        .map(|url| json!(url))
        .unwrap_or(Value::Null);

    Some(json!({
        "type": "codex_customer_artifact_bundle",
        "artifact_type": "codex_customer_artifacts",
        "artifact_kind": "customer_artifact_bundle",
        "status": status.clone(),
        "capability": capability,
        "route": route,
        "workflow_execution_id": workflow_execution_id.clone(),
        "published": published,
        "primary_url": primary_url_value.clone(),
        "public_url": primary_url_value.clone(),
        "artifact_count": artifact_count,
        "customer_artifacts": customer_artifacts,
        "artifact_manifest": {
            "schema": "v3.output_artifact_manifest",
            "schema_version": 1,
            "artifact_type": "codex_customer_artifacts",
            "artifact_kind": "customer_artifact_bundle",
            "title": title,
            "status": status,
            "primary_url": primary_url_value,
            "links": links,
            "refs": {
                "workflow_execution_id": workflow_execution_id,
                "artifact_paths": artifact_paths,
                "manifest_path": manifest_path,
            },
            "safety": {
                "credentials_exposed": false,
                "raw_logs_exposed": false,
                "workspace_paths_only": true,
                "absolute_paths_exposed": false,
                "published": published,
                "requires_datamax_publish_validation": !published,
            },
        },
    }))
}

pub(crate) fn assistant_run_customer_codex_sidecar_scope_summary(selected_scope: &Value) -> Value {
    json!({
        "intent": assistant_run_scope_intent(selected_scope),
        "dataset_count": selected_dataset_ids_from_scope(selected_scope).len(),
        "document_count": selected_document_ids_from_scope(selected_scope).len(),
        "has_database_sources": selected_scope.get("database_sources").is_some() || selected_scope.get("databaseSources").is_some(),
        "scope_type": selected_scope.get("type").and_then(Value::as_str).unwrap_or("unknown"),
    })
}

pub(crate) fn assistant_run_customer_codex_sidecar_evidence_summary(
    evidence_state: &Value,
) -> Value {
    json!({
        "status": evidence_state.get("status").and_then(Value::as_str).unwrap_or("unknown"),
        "supplied_item_count": assistant_run_evidence_supplied_count(evidence_state),
        "supply_quality": evidence_state.get("supply_quality").cloned().unwrap_or(Value::Null),
    })
}

pub(crate) fn assistant_run_customer_codex_sidecar_task_prompt(
    capability: &str,
    forwarded_prompt: &str,
    selected_scope: &Value,
    evidence_state: &Value,
) -> String {
    let permission_scope = assistant_run_customer_codex_sidecar_permission_scope(capability);
    let capability_instructions =
        assistant_run_customer_codex_sidecar_capability_instructions(capability);
    format!(
        "Run DataMax Codex Host capability `{capability}` for a customer request from the web UI.\n\
         Return a concise structured result, action intent, or customer artifact package. Preserve the main AssistantRun answer path; this sidecar must not block customer-visible text.\n\
         Safety boundary: do not modify V3 product source code, services, migrations, auth, public APIs, provider configuration, deployment, commits, or system files. \
         If the customer asks for V3 product changes, return needs_operator_review. V3-generated static pages and customer artifacts are customer-owned outputs, not V3 product code.\n\
         Permission scope: {permission_scope}.\n\
         Capability instructions: {capability_instructions}\n\n\
         User request:\n{prompt}\n\n\
         Selected scope summary:\n{scope}\n\n\
         Evidence summary:\n{evidence}",
        prompt = truncate_assistant_supply_text(forwarded_prompt, 1600),
        scope = assistant_run_customer_codex_sidecar_scope_summary(selected_scope),
        evidence = assistant_run_customer_codex_sidecar_evidence_summary(evidence_state),
    )
}

pub(crate) fn assistant_run_customer_codex_sidecar_route(capability: &str) -> &'static str {
    match capability {
        CODEX_CAPABILITY_DATA_INGESTION_ANALYSIS => "assistant_run_data_ingestion_analysis",
        CODEX_CAPABILITY_GENERATED_STATIC_PAGE_EDIT => "assistant_run_generated_static_page_edit",
        CODEX_CAPABILITY_GENERATED_STATIC_PAGE_PUBLISH => {
            "assistant_run_generated_static_page_publish"
        }
        CODEX_CAPABILITY_CUSTOMER_ARTIFACT_REQUEST => "assistant_run_customer_artifact_request",
        _ => "assistant_run_customer_complex_request",
    }
}

pub(crate) fn assistant_run_customer_codex_sidecar_permission_scope(
    capability: &str,
) -> &'static str {
    match capability {
        CODEX_CAPABILITY_DATA_INGESTION_ANALYSIS => {
            "data-ingestion analysis and staging-spec planning with one target DataMax dataset identified or proposed; no V3 public API changes"
        }
        CODEX_CAPABILITY_GENERATED_STATIC_PAGE_EDIT => {
            "workspace-write only inside the isolated task workspace seeded from the current generated static page artifact; no V3 repo writes"
        }
        CODEX_CAPABILITY_GENERATED_STATIC_PAGE_PUBLISH => {
            "workspace-write only inside the isolated task workspace for a new generated static page package; DataMax validates and publishes generated artifacts; no V3 repo writes"
        }
        CODEX_CAPABILITY_CUSTOMER_ARTIFACT_REQUEST => {
            "workspace-write only inside the isolated customer task workspace for generated customer artifacts; no V3 repo writes"
        }
        _ => "read-only analysis and planning; no filesystem writes",
    }
}

pub(crate) fn assistant_run_customer_codex_sidecar_capability_instructions(
    capability: &str,
) -> &'static str {
    match capability {
        CODEX_CAPABILITY_DATA_INGESTION_ANALYSIS => {
            "Use the fixed data_ingestion_analysis template. Let Codex take over the customer's database/API integration request, but make sure the result identifies one target DataMax dataset or proposes one dataset to create/attach before continuing. Keep the existing safety boundary: do not request or emit credentials, do not change public APIs, and do not write production data without confirmation."
        }
        CODEX_CAPABILITY_GENERATED_STATIC_PAGE_EDIT => {
            "Revise only the supplied V3-generated static page/customer artifact context. Read workspace-seed.json and, when present, existing-artifact/ as the current page copy. Produce a new package under the task workspace for DataMax validation; do not overwrite stable URLs or bypass DataMax publish checks. If you create files, write customer-artifact-manifest.json at the workspace root with artifacts[].path as workspace-relative paths plus title, kind, and mime_type."
        }
        CODEX_CAPABILITY_GENERATED_STATIC_PAGE_PUBLISH => {
            "Create a new generated static page/customer page package inside the isolated task workspace. Use the supplied selected scope and evidence summary as context, write customer-artifact-manifest.json at the workspace root with artifacts[].path as workspace-relative paths plus title, kind, and mime_type, and rely on DataMax validation/publish checks before any public generated-artifact URL is exposed."
        }
        CODEX_CAPABILITY_CUSTOMER_ARTIFACT_REQUEST => {
            "Use the isolated task workspace as the customer's Codex scratchpad. Create or modify customer-facing artifacts there when useful. Write customer-artifact-manifest.json at the workspace root with artifacts[].path as workspace-relative paths plus title, kind, and mime_type; also return a concise next action intent and keep normal answer generation unblocked."
        }
        _ => {
            "Analyze the customer request, supplied scope, and evidence. Return a structured plan, findings, or recommended DataMax actions without assuming write access."
        }
    }
}

pub(crate) fn assistant_run_customer_codex_sidecar_scope_blocked_payload(
    capability: &str,
    reason: &str,
) -> Value {
    json!({
        "capability": capability,
        "route": capability,
        "status": "needs_operator_review",
        "reason": reason,
        "allowed_next_step": "operator can review this as a product change request",
        "non_blocking": true,
        "main_answer_path_preserved": true,
        "v3_product_repo_write_allowed": false,
        "customer_writable": false,
    })
}

pub(crate) fn assistant_run_customer_codex_sidecar_queued_payload(
    capability: &str,
    execution_id: WorkflowExecutionId,
) -> Value {
    json!({
        "capability": capability,
        "workflow_execution_id": execution_id.to_string(),
        "non_blocking": true,
        "main_answer_path_preserved": true,
    })
}

pub(crate) fn assistant_run_customer_codex_sidecar_preflight_rejected_payload(
    capability: &str,
    reason: &str,
) -> Value {
    json!({
        "capability": capability,
        "reason": reason,
        "non_blocking": true,
    })
}

pub(crate) fn assistant_run_customer_codex_sidecar_preflight(
    capability: &str,
) -> std::result::Result<(), &'static str> {
    if !platform_env_flag("CODEX_HOST_TASK_ENABLED", false) {
        return Err("codex_host_task_disabled");
    }
    if !env_csv_contains("CODEX_HOST_TASK_ALLOWLIST", capability) {
        return Err("codex_host_task_not_allowlisted");
    }
    if !env_csv_contains("CODEX_HOST_AGENT_PROFILE_ALLOWED_CAPABILITIES", capability) {
        return Err("codex_host_agent_capability_not_allowlisted");
    }
    Ok(())
}

pub(crate) fn assistant_run_customer_codex_sidecar_workspace_seed_payload(
    public_url: &str,
    current_artifact_brief: Value,
) -> Value {
    json!({
        "schema": "v3.codex_host_workspace_seed",
        "version": 1,
        "kind": "generated_static_page_edit",
        "source": "assistant_run.current_artifact",
        "existing_artifact": {
            "public_url": public_url,
            "index_url": public_url,
            "revision_requested": true,
            "source": "assistant_run_current_artifact"
        },
        "current_artifact": current_artifact_brief,
        "output_manifest": {
            "preferred_path": "customer-artifact-manifest.json",
            "compatible_paths": ["artifacts/manifest.json", "generated-artifacts/manifest.json"]
        },
        "safety": {
            "workspace_write_only": true,
            "v3_product_repo_write_allowed": false,
            "stable_url_overwrite_allowed": false
        }
    })
}

pub(crate) fn assistant_run_customer_codex_sidecar_workspace_seed(
    capability: &str,
    current_artifact: Option<&Value>,
) -> Option<Value> {
    if capability != CODEX_CAPABILITY_GENERATED_STATIC_PAGE_EDIT {
        return None;
    }
    let current_artifact = current_artifact?;
    let public_url = static_page_public_url_from_current_artifact(current_artifact)?;
    Some(assistant_run_customer_codex_sidecar_workspace_seed_payload(
        &public_url,
        assistant_run_current_artifact_brief(current_artifact),
    ))
}

pub(crate) fn assistant_run_customer_codex_sidecar_context_payload(
    capability: &str,
    selected_scope: &Value,
    evidence_state: &Value,
    current_artifact: Value,
) -> Value {
    json!({
        "version": 1,
        "route": assistant_run_customer_codex_sidecar_route(capability),
        "non_blocking": true,
        "main_answer_path_preserved": true,
        "permission_scope": assistant_run_customer_codex_sidecar_permission_scope(capability),
        "selected_scope_summary": assistant_run_customer_codex_sidecar_scope_summary(selected_scope),
        "evidence_summary": assistant_run_customer_codex_sidecar_evidence_summary(evidence_state),
        "current_artifact": current_artifact,
    })
}

pub(crate) fn assistant_run_customer_codex_sidecar_fixed_task_context(
    tenant_id: TenantId,
    assistant_run_id: AssistantRunId,
    execution_id: WorkflowExecutionId,
    capability: &str,
    forwarded_prompt: &str,
    selected_scope: &Value,
    evidence_state: &Value,
) -> Option<CodexHostFixedTaskTemplateContextView> {
    if capability != CODEX_CAPABILITY_DATA_INGESTION_ANALYSIS {
        return None;
    }
    Some(assistant_run_data_ingestion_sidecar_fixed_task_context(
        tenant_id,
        assistant_run_id,
        execution_id,
        forwarded_prompt,
        selected_scope,
        evidence_state,
    ))
}

pub(crate) fn assistant_run_data_ingestion_sidecar_dataset_scope(
    tenant_id: TenantId,
    selected_scope: &Value,
) -> Value {
    json!({
        "tenant_id": tenant_id.to_string(),
        "dataset_ids": selected_dataset_ids_from_scope(selected_scope)
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        "database_source_ids": selected_string_ids_from_scope(
            selected_scope,
            &[
                "database_source_ids",
                "databaseSourceIds",
                "database_sources",
                "databaseSources",
                "business_datasource_ids",
                "businessDatasourceIds",
                "businessDataSourceIds",
                "source_ids",
                "sourceIds"
            ]
        ),
        "selected_document_ids": selected_document_ids_from_scope(selected_scope)
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        "uploaded_file_ids": selected_string_ids_from_scope(
            selected_scope,
            &["uploaded_file_ids", "uploadedFiles", "files", "file_ids"]
        ),
        "table_ids": selected_string_ids_from_scope(
            selected_scope,
            &["table_ids", "tables", "selected_tables", "selectedTables"]
        ),
        "scope_source": "v3_main_assistant_selected_scope",
    })
}

pub(crate) fn assistant_run_data_ingestion_sidecar_scope_has_source(
    selected_scope: &Value,
) -> bool {
    let dataset_scope =
        assistant_run_data_ingestion_sidecar_dataset_scope(TenantId::new(), selected_scope);
    external_channel_data_ingestion_scope_has_source(&dataset_scope)
}

pub(crate) fn assistant_run_data_ingestion_sidecar_evidence_summary(
    evidence_state: &Value,
) -> Value {
    json!({
        "source_visibility": "v3_main_assistant_selected_scope_only",
        "supplied_item_count": assistant_run_evidence_supplied_count(evidence_state),
        "supply_quality": evidence_state.get("supply_quality").cloned().unwrap_or(Value::Null),
        "raw_credentials_supplied": false
    })
}

pub(crate) fn assistant_run_data_ingestion_sidecar_requirements(forwarded_prompt: &str) -> Value {
    json!({
        "user_goal": truncate_assistant_supply_text(forwarded_prompt, 1200),
        "intent": "data_ingestion_analysis",
        "source": "v3_main_assistant_cc_sidecar",
        "target_dataset_required": true,
        "target_dataset_resolution": "use_selected_dataset_when_available_or_propose_one_datamax_dataset_to_create_or_attach",
        "requested_outputs": [
            "data_quality_report",
            "field_mapping_plan",
            "staging_spec",
            "validation_checks",
            "recommended_next_actions"
        ],
    })
}

pub(crate) fn assistant_run_data_ingestion_sidecar_policies() -> Value {
    json!({
        "mode": "read_only_analysis_or_staging_spec",
        "credential_policy": "do_not_request_or_emit_credentials",
        "production_write_policy": "needs_human_confirmation",
        "public_api_change_allowed": false,
        "schema_change_allowed_without_confirmation": false
    })
}

pub(crate) fn assistant_run_data_ingestion_sidecar_fixed_task_context(
    tenant_id: TenantId,
    assistant_run_id: AssistantRunId,
    execution_id: WorkflowExecutionId,
    forwarded_prompt: &str,
    selected_scope: &Value,
    evidence_state: &Value,
) -> CodexHostFixedTaskTemplateContextView {
    CodexHostFixedTaskTemplateContextView {
        template_id: CodexHostFixedTaskTemplateIdView::DataIngestionAnalysis,
        version: 1,
        assistant_run_id: Some(assistant_run_id.to_string()),
        draft_id: None,
        case_id: Some(format!("assistant-run-data-ingestion-{execution_id}")),
        dataset_scope: assistant_run_data_ingestion_sidecar_dataset_scope(
            tenant_id,
            selected_scope,
        ),
        requirements: assistant_run_data_ingestion_sidecar_requirements(forwarded_prompt),
        image2: Value::Null,
        policies: assistant_run_data_ingestion_sidecar_policies(),
        low_quality_signals: Vec::new(),
        user_question: Some(truncate_assistant_supply_text(forwarded_prompt, 1000)),
        customer_answer: None,
        evidence_summary: assistant_run_data_ingestion_sidecar_evidence_summary(evidence_state),
        trace_summary: Value::Null,
        allowed_write_scope: None,
        human_review_policy:
            CodexHostFixedTaskHumanReviewPolicyView::AutoForReadOnlyAnalysisOrStagingSpec,
    }
}

fn assistant_run_safe_customer_codex_artifacts(customer_artifacts: &Value) -> Option<Value> {
    if !customer_artifacts.is_object() {
        return None;
    }
    if customer_artifacts.get("schema").and_then(Value::as_str)
        != Some("v3.customer_codex_artifacts")
    {
        return None;
    }
    if customer_artifacts.get("version").and_then(Value::as_i64) != Some(1) {
        return None;
    }
    let artifacts = customer_artifacts
        .get("artifacts")
        .and_then(Value::as_array)?
        .iter()
        .filter_map(assistant_run_safe_customer_codex_artifact_item)
        .collect::<Vec<_>>();
    if artifacts.is_empty() {
        return None;
    }
    let status = assistant_run_safe_customer_codex_text(
        customer_artifacts
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("available"),
        40,
    );
    let capability = customer_artifacts
        .get("capability")
        .and_then(Value::as_str)
        .and_then(assistant_run_safe_customer_codex_artifact_capability)
        .unwrap_or_else(|| CODEX_CAPABILITY_CUSTOMER_ARTIFACT_REQUEST.to_string());
    let manifest_path = customer_artifacts
        .get("manifest_path")
        .and_then(Value::as_str)
        .and_then(assistant_run_safe_customer_codex_relative_path);
    let title = customer_artifacts
        .get("title")
        .and_then(Value::as_str)
        .map(|value| assistant_run_safe_customer_codex_display_text(value, 160))
        .unwrap_or_default();
    let summary = customer_artifacts
        .get("summary")
        .and_then(Value::as_str)
        .map(|value| assistant_run_safe_customer_codex_display_text(value, 600))
        .unwrap_or_default();
    let primary_url = customer_artifacts
        .get("primary_url")
        .or_else(|| customer_artifacts.get("public_url"))
        .and_then(Value::as_str)
        .and_then(assistant_run_safe_customer_codex_generated_artifact_url)
        .or_else(|| {
            artifacts
                .iter()
                .filter_map(|artifact| artifact.get("public_url").and_then(Value::as_str))
                .find_map(assistant_run_safe_customer_codex_generated_artifact_url)
        });
    let published_manifest_url = customer_artifacts
        .get("published_manifest_url")
        .or_else(|| customer_artifacts.pointer("/publish/manifest_url"))
        .and_then(Value::as_str)
        .and_then(assistant_run_safe_customer_codex_generated_artifact_url);
    let published = customer_artifacts
        .get("published")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && primary_url.is_some();
    let published_artifact_count = customer_artifacts
        .get("published_artifact_count")
        .or_else(|| customer_artifacts.pointer("/publish/artifact_count"))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| {
            artifacts
                .iter()
                .filter(|artifact| artifact.get("public_url").and_then(Value::as_str).is_some())
                .count() as u64
        });

    let primary_url_value = primary_url
        .as_ref()
        .map(|url| json!(url))
        .unwrap_or(Value::Null);
    let published_manifest_url_value = published_manifest_url
        .as_ref()
        .map(|url| json!(url))
        .unwrap_or(Value::Null);

    Some(json!({
        "schema": "v3.customer_codex_artifacts",
        "version": 1,
        "status": if published { "published".to_string() } else { status },
        "capability": capability,
        "manifest_path": manifest_path,
        "artifact_count": artifacts.len(),
        "artifacts": artifacts,
        "summary": summary,
        "title": title,
        "published": published,
        "published_artifact_count": published_artifact_count,
        "primary_url": primary_url_value.clone(),
        "public_url": primary_url_value,
        "published_manifest_url": published_manifest_url_value,
        "validation": {
            "workspace_scoped": true,
            "v3_product_repo_write_blocked": true,
            "absolute_paths_redacted": true,
            "published_to_generated_artifacts": published,
        },
    }))
}

fn assistant_run_safe_customer_codex_artifact_item(item: &Value) -> Option<Value> {
    let path = item
        .get("path")
        .and_then(Value::as_str)
        .and_then(assistant_run_safe_customer_codex_relative_path)?;
    let title = item
        .get("title")
        .or_else(|| item.get("name"))
        .or_else(|| item.get("label"))
        .and_then(Value::as_str)
        .map(|value| assistant_run_safe_customer_codex_display_text(value, 160))
        .unwrap_or_default();
    let kind = item
        .get("kind")
        .or_else(|| item.get("type"))
        .or_else(|| item.get("artifact_kind"))
        .and_then(Value::as_str)
        .map(|value| assistant_run_safe_customer_codex_text(value, 80))
        .unwrap_or_else(|| "file".to_string());
    let mime_type = item
        .get("mime_type")
        .or_else(|| item.get("mimeType"))
        .or_else(|| item.get("content_type"))
        .and_then(Value::as_str)
        .map(|value| assistant_run_safe_customer_codex_text(value, 120))
        .unwrap_or_default();
    let bytes = item.get("bytes").and_then(Value::as_u64).unwrap_or(0);
    let sha256 = item
        .get("sha256")
        .and_then(Value::as_str)
        .filter(|value| value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit()))
        .unwrap_or_default();
    let public_url = item
        .get("public_url")
        .or_else(|| item.get("publicUrl"))
        .and_then(Value::as_str)
        .and_then(assistant_run_safe_customer_codex_generated_artifact_url);
    let published = item
        .get("published")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && public_url.is_some();

    Some(json!({
        "path": path,
        "title": title,
        "kind": kind,
        "mime_type": mime_type,
        "bytes": bytes,
        "sha256": sha256,
        "published": published,
        "public_url": public_url,
    }))
}

fn assistant_run_safe_customer_codex_relative_path(path: &str) -> Option<String> {
    let trimmed = path.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('/')
        || trimmed.starts_with('~')
        || trimmed.starts_with("\\\\")
        || trimmed.contains('\\')
        || trimmed.contains(':')
        || trimmed.contains('\0')
    {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("/srv/aiv3/repo")
        || lower.contains("/users/")
        || lower.contains("node_modules")
        || lower.contains(".git")
        || lower.contains(".env")
        || lower.ends_with(".env")
        || lower.contains("secret")
        || lower.contains("credential")
        || lower.contains("access_token")
        || lower.contains("refresh_token")
        || lower.contains("api_key")
        || lower.contains("apikey")
        || lower.contains("private_key")
        || lower.contains("password")
        || lower.ends_with(".pem")
        || lower.ends_with(".key")
        || lower.ends_with(".p12")
        || lower.ends_with(".pfx")
        || lower.contains("id_rsa")
        || lower.contains("id_dsa")
        || lower.contains("id_ecdsa")
        || lower.contains("id_ed25519")
        || lower.contains("authorized_keys")
    {
        return None;
    }
    if trimmed
        .split('/')
        .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return None;
    }
    Some(trimmed.to_string())
}

fn assistant_run_safe_customer_codex_generated_artifact_url(url: &str) -> Option<String> {
    let trimmed = url.trim();
    if trimmed.is_empty()
        || trimmed.contains('\0')
        || trimmed.contains("/generated-artifacts/pending-")
        || trimmed.contains("/generated-artifacts/pending/")
        || trimmed.ends_with("/generated-artifacts/pending")
    {
        return None;
    }
    if trimmed.starts_with("/generated-artifacts/") {
        return Some(trimmed.to_string());
    }
    let default_base = "https://v3.elepcloud.com/generated-artifacts";
    if trimmed.starts_with(&format!("{default_base}/")) {
        return Some(trimmed.to_string());
    }
    let configured_base = external_channel_generated_artifact_public_base_url();
    if configured_base != default_base
        && trimmed.starts_with(&format!("{}/", configured_base.trim_end_matches('/')))
    {
        return Some(trimmed.to_string());
    }
    None
}

fn assistant_run_customer_codex_artifact_links(
    customer_artifacts: &Value,
    primary_url: Option<&str>,
) -> Vec<Value> {
    let mut links = Vec::new();
    let mut seen = BTreeSet::new();
    if let Some(url) = primary_url {
        if seen.insert(url.to_string()) {
            links.push(json!({"rel": "public", "url": url}));
        }
    }
    if let Some(url) = customer_artifacts
        .get("published_manifest_url")
        .and_then(Value::as_str)
        .and_then(assistant_run_safe_customer_codex_generated_artifact_url)
    {
        if seen.insert(url.clone()) {
            links.push(json!({"rel": "manifest", "url": url}));
        }
    }
    if let Some(artifacts) = customer_artifacts
        .get("artifacts")
        .and_then(Value::as_array)
    {
        for artifact in artifacts {
            let Some(url) = artifact
                .get("public_url")
                .and_then(Value::as_str)
                .and_then(assistant_run_safe_customer_codex_generated_artifact_url)
            else {
                continue;
            };
            if !seen.insert(url.clone()) {
                continue;
            }
            let path = artifact
                .get("path")
                .and_then(Value::as_str)
                .and_then(assistant_run_safe_customer_codex_relative_path);
            let kind = artifact
                .get("kind")
                .and_then(Value::as_str)
                .map(|value| assistant_run_safe_customer_codex_text(value, 80))
                .unwrap_or_else(|| "file".to_string());
            links.push(json!({
                "rel": "file",
                "url": url,
                "path": path,
                "kind": kind,
            }));
        }
    }
    links
}

fn assistant_run_safe_customer_codex_artifact_capability(value: &str) -> Option<String> {
    let trimmed = value.trim();
    match trimmed {
        CODEX_CAPABILITY_CUSTOMER_ARTIFACT_REQUEST
        | CODEX_CAPABILITY_GENERATED_STATIC_PAGE_EDIT
        | CODEX_CAPABILITY_GENERATED_STATIC_PAGE_PUBLISH => Some(trimmed.to_string()),
        _ => None,
    }
}

fn assistant_run_safe_customer_codex_artifact_route(value: &str) -> Option<String> {
    assistant_run_safe_customer_codex_artifact_capability(value)
}

fn assistant_run_customer_codex_default_route_for_capability(
    event_name: &str,
    capability: &str,
) -> String {
    match capability {
        CODEX_CAPABILITY_GENERATED_STATIC_PAGE_EDIT => "generated_static_page_edit",
        CODEX_CAPABILITY_GENERATED_STATIC_PAGE_PUBLISH => "generated_static_page_publish",
        CODEX_CAPABILITY_CUSTOMER_COMPLEX_REQUEST => "customer_complex_request",
        CODEX_CAPABILITY_V3_PRODUCT_CHANGE_REQUEST => "v3_product_change_request",
        _ if event_name == ASSISTANT_RUN_GENERATED_STATIC_PAGE_EDIT_ARTIFACTS_READY_EVENT => {
            "generated_static_page_edit"
        }
        _ => "customer_artifact_request",
    }
    .to_string()
}

fn assistant_run_customer_codex_text_looks_sensitive(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("authorization")
        || lower.contains("bearer ")
        || lower.contains("api_key")
        || lower.contains("apikey")
        || lower.contains("access_token")
        || lower.contains("refresh_token")
        || lower.contains("private_key")
        || lower.contains("secret")
        || lower.contains("cookie")
        || lower.contains("database_url")
        || lower.contains("mysql://")
        || lower.contains("postgres://")
        || lower.contains("postgresql://")
        || lower.contains("mongodb://")
        || lower.contains("/users/")
        || lower.contains("/srv/aiv3/repo")
        || lower.contains("/srv/aiv3/shared")
        || lower.contains("/private/var/")
        || lower.contains("\\.codex")
        || lower.contains("/.codex")
        || lower.contains(".env")
        || lower.contains("sk-")
        || lower.contains(":\\")
}

fn assistant_run_safe_customer_codex_display_text(value: &str, max_chars: usize) -> String {
    let text = assistant_run_safe_customer_codex_text(value, max_chars);
    if assistant_run_customer_codex_text_looks_sensitive(&text) {
        String::new()
    } else {
        text
    }
}

fn assistant_run_safe_customer_codex_text(value: &str, max_chars: usize) -> String {
    truncate_assistant_supply_text(value.trim(), max_chars)
}

fn assistant_run_safe_uuid_string(payload: &Value, key: &str) -> Value {
    payload
        .get(key)
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .map(|uuid| json!(uuid.to_string()))
        .unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    struct TestEnvVarRestore {
        key: &'static str,
        old_value: Option<String>,
    }

    impl TestEnvVarRestore {
        fn set(key: &'static str, value: &str) -> Self {
            let old_value = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, old_value }
        }

        fn remove(key: &'static str) -> Self {
            let old_value = std::env::var(key).ok();
            std::env::remove_var(key);
            Self { key, old_value }
        }
    }

    impl Drop for TestEnvVarRestore {
        fn drop(&mut self) {
            if let Some(value) = &self.old_value {
                std::env::set_var(self.key, value);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }

    fn preflight_env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn assistant_run_customer_codex_dedup_preserves_first_seen_artifacts() {
        let first = json!({"type": "artifact", "path": "reports/index.html"});
        let second = json!({"type": "artifact", "path": "reports/notes.md"});

        let merged = assistant_run_append_deduped_output_artifacts(
            vec![first.clone(), second.clone()],
            vec![first.clone()],
        );

        assert_eq!(merged, vec![first, second]);
    }

    #[test]
    fn assistant_run_customer_codex_sidecar_summaries_preserve_scope_and_evidence_shape() {
        let scope = json!({
            "type": "selected_scope",
            "intent": "data_analysis",
            "datasets": ["00000000-0000-0000-0000-000000000001"],
            "documents": ["00000000-0000-0000-0000-000000000002"],
            "databaseSources": [{"id": "hy-sql-traffic-area"}]
        });

        let summary = assistant_run_customer_codex_sidecar_scope_summary(&scope);

        assert_eq!(
            summary,
            json!({
                "intent": "data_analysis",
                "dataset_count": 1,
                "document_count": 1,
                "has_database_sources": true,
                "scope_type": "selected_scope"
            })
        );

        let evidence_state = json!({
            "status": "supplied",
            "supplied_items": [{}, {}],
            "supply_quality": {"status": "strong", "suppliedItemCount": 2}
        });

        let summary = assistant_run_customer_codex_sidecar_evidence_summary(&evidence_state);

        assert_eq!(
            summary,
            json!({
                "status": "supplied",
                "supplied_item_count": 2,
                "supply_quality": {"status": "strong", "suppliedItemCount": 2}
            })
        );
    }

    #[test]
    fn assistant_run_customer_codex_sidecar_capability_mappings_stay_stable() {
        assert_eq!(
            assistant_run_customer_codex_sidecar_route(CODEX_CAPABILITY_DATA_INGESTION_ANALYSIS),
            "assistant_run_data_ingestion_analysis"
        );
        assert_eq!(
            assistant_run_customer_codex_sidecar_route(CODEX_CAPABILITY_GENERATED_STATIC_PAGE_EDIT),
            "assistant_run_generated_static_page_edit"
        );
        assert_eq!(
            assistant_run_customer_codex_sidecar_route(
                CODEX_CAPABILITY_GENERATED_STATIC_PAGE_PUBLISH
            ),
            "assistant_run_generated_static_page_publish"
        );
        assert_eq!(
            assistant_run_customer_codex_sidecar_route(CODEX_CAPABILITY_CUSTOMER_ARTIFACT_REQUEST),
            "assistant_run_customer_artifact_request"
        );
        assert_eq!(
            assistant_run_customer_codex_sidecar_route(CODEX_CAPABILITY_CUSTOMER_COMPLEX_REQUEST),
            "assistant_run_customer_complex_request"
        );

        assert!(assistant_run_customer_codex_sidecar_permission_scope(
            CODEX_CAPABILITY_DATA_INGESTION_ANALYSIS
        )
        .contains("target DataMax dataset"));
        assert!(assistant_run_customer_codex_sidecar_permission_scope(
            CODEX_CAPABILITY_GENERATED_STATIC_PAGE_EDIT
        )
        .contains("seeded"));
        assert!(assistant_run_customer_codex_sidecar_permission_scope(
            CODEX_CAPABILITY_GENERATED_STATIC_PAGE_PUBLISH
        )
        .contains("new generated static page package"));
        assert!(assistant_run_customer_codex_sidecar_permission_scope(
            CODEX_CAPABILITY_CUSTOMER_ARTIFACT_REQUEST
        )
        .contains("customer task workspace"));
        assert_eq!(
            assistant_run_customer_codex_sidecar_permission_scope("other"),
            "read-only analysis and planning; no filesystem writes"
        );

        assert!(
            assistant_run_customer_codex_sidecar_capability_instructions(
                CODEX_CAPABILITY_DATA_INGESTION_ANALYSIS
            )
            .contains("data_ingestion_analysis")
        );
        assert!(
            assistant_run_customer_codex_sidecar_capability_instructions(
                CODEX_CAPABILITY_GENERATED_STATIC_PAGE_EDIT
            )
            .contains("workspace-seed.json")
        );
        assert!(
            assistant_run_customer_codex_sidecar_capability_instructions(
                CODEX_CAPABILITY_GENERATED_STATIC_PAGE_PUBLISH
            )
            .contains("customer-artifact-manifest.json")
        );
        assert!(
            assistant_run_customer_codex_sidecar_capability_instructions(
                CODEX_CAPABILITY_CUSTOMER_ARTIFACT_REQUEST
            )
            .contains("Codex scratchpad")
        );
        assert!(
            assistant_run_customer_codex_sidecar_capability_instructions("other")
                .contains("without assuming write access")
        );
    }

    #[test]
    fn assistant_run_customer_codex_sidecar_task_prompt_preserves_boundaries() {
        let selected_scope = json!({
            "type": "external_channel",
            "datasets": ["00000000-0000-0000-0000-000000000101"],
            "databaseSources": ["hy-sql"]
        });
        let evidence_state = json!({
            "status": "supplied",
            "supplied_items": [{"id": "ev-1"}],
            "supply_quality": {"status": "strong"}
        });

        let task = assistant_run_customer_codex_sidecar_task_prompt(
            CODEX_CAPABILITY_GENERATED_STATIC_PAGE_PUBLISH,
            "请根据当前数据范围生成一份客户可看的经营报表页面。",
            &selected_scope,
            &evidence_state,
        );

        assert!(task.contains("Run DataMax Codex Host capability"));
        assert!(task.contains(CODEX_CAPABILITY_GENERATED_STATIC_PAGE_PUBLISH));
        assert!(task.contains("Preserve the main AssistantRun answer path"));
        assert!(task.contains("this sidecar must not block customer-visible text"));
        assert!(task.contains("do not modify V3 product source code"));
        assert!(task.contains("Permission scope: workspace-write only"));
        assert!(task.contains("customer-artifact-manifest.json"));
        assert!(task.contains("User request:\n请根据当前数据范围生成一份客户可看的经营报表页面。"));
        assert!(task.contains("\"scope_type\":\"external_channel\""));
        assert!(task.contains("\"dataset_count\":1"));
        assert!(task.contains("\"supplied_item_count\":1"));
        assert!(task.contains("\"supply_quality\":{\"status\":\"strong\"}"));
    }

    #[test]
    fn assistant_run_customer_codex_sidecar_task_prompt_truncates_user_request() {
        let long_prompt = "a".repeat(1700);
        let task = assistant_run_customer_codex_sidecar_task_prompt(
            CODEX_CAPABILITY_CUSTOMER_ARTIFACT_REQUEST,
            &long_prompt,
            &json!({}),
            &json!({}),
        );

        let user_request = task
            .split("User request:\n")
            .nth(1)
            .and_then(|tail| tail.split("\n\nSelected scope summary:").next())
            .expect("task prompt should include user request section");
        assert_eq!(user_request.chars().count(), 1600);
    }

    #[test]
    fn assistant_run_customer_codex_sidecar_scope_blocked_payload_marks_product_change() {
        let payload = assistant_run_customer_codex_sidecar_scope_blocked_payload(
            CODEX_CAPABILITY_V3_PRODUCT_CHANGE_REQUEST,
            "v3_product_change_not_customer_writable",
        );

        assert_eq!(
            payload,
            json!({
                "capability": CODEX_CAPABILITY_V3_PRODUCT_CHANGE_REQUEST,
                "route": CODEX_CAPABILITY_V3_PRODUCT_CHANGE_REQUEST,
                "status": "needs_operator_review",
                "reason": "v3_product_change_not_customer_writable",
                "allowed_next_step": "operator can review this as a product change request",
                "non_blocking": true,
                "main_answer_path_preserved": true,
                "v3_product_repo_write_allowed": false,
                "customer_writable": false
            })
        );
    }

    #[test]
    fn assistant_run_customer_codex_sidecar_queued_payload_preserves_customer_visible_shape() {
        let execution_id = WorkflowExecutionId::new();
        let payload = assistant_run_customer_codex_sidecar_queued_payload(
            CODEX_CAPABILITY_CUSTOMER_ARTIFACT_REQUEST,
            execution_id,
        );

        assert_eq!(
            payload,
            json!({
                "capability": CODEX_CAPABILITY_CUSTOMER_ARTIFACT_REQUEST,
                "workflow_execution_id": execution_id.to_string(),
                "non_blocking": true,
                "main_answer_path_preserved": true
            })
        );
    }

    #[test]
    fn assistant_run_customer_codex_sidecar_preflight_rejected_payload_preserves_shape() {
        let payload = assistant_run_customer_codex_sidecar_preflight_rejected_payload(
            CODEX_CAPABILITY_CUSTOMER_ARTIFACT_REQUEST,
            "codex_host_task_not_allowlisted",
        );

        assert_eq!(
            payload,
            json!({
                "capability": CODEX_CAPABILITY_CUSTOMER_ARTIFACT_REQUEST,
                "reason": "codex_host_task_not_allowlisted",
                "non_blocking": true
            })
        );
    }

    #[test]
    fn assistant_run_customer_codex_sidecar_workspace_seed_payload_preserves_shape() {
        let payload = assistant_run_customer_codex_sidecar_workspace_seed_payload(
            "https://v3.elepcloud.com/generated-artifacts/report/index.html",
            json!({
                "artifactType": "static_page",
                "publicUrl": "https://v3.elepcloud.com/generated-artifacts/report/index.html"
            }),
        );

        assert_eq!(
            payload,
            json!({
                "schema": "v3.codex_host_workspace_seed",
                "version": 1,
                "kind": "generated_static_page_edit",
                "source": "assistant_run.current_artifact",
                "existing_artifact": {
                    "public_url": "https://v3.elepcloud.com/generated-artifacts/report/index.html",
                    "index_url": "https://v3.elepcloud.com/generated-artifacts/report/index.html",
                    "revision_requested": true,
                    "source": "assistant_run_current_artifact"
                },
                "current_artifact": {
                    "artifactType": "static_page",
                    "publicUrl": "https://v3.elepcloud.com/generated-artifacts/report/index.html"
                },
                "output_manifest": {
                    "preferred_path": "customer-artifact-manifest.json",
                    "compatible_paths": ["artifacts/manifest.json", "generated-artifacts/manifest.json"]
                },
                "safety": {
                    "workspace_write_only": true,
                    "v3_product_repo_write_allowed": false,
                    "stable_url_overwrite_allowed": false
                }
            })
        );
    }

    #[test]
    fn assistant_run_customer_codex_sidecar_workspace_seed_builds_only_for_static_page_edit() {
        let current_artifact = json!({
            "type": "static_page_draft",
            "label": "经营分析报表",
            "finalPage": {
                "publicUrl": "https://v3.elepcloud.com/generated-artifacts/report/index.html"
            },
            "modules": [{"id": "kpi"}, {"id": "trend"}]
        });

        let payload = assistant_run_customer_codex_sidecar_workspace_seed(
            CODEX_CAPABILITY_GENERATED_STATIC_PAGE_EDIT,
            Some(&current_artifact),
        )
        .expect("static-page edit capability with public url should seed workspace");

        assert_eq!(payload["kind"], json!("generated_static_page_edit"));
        assert_eq!(
            payload["existing_artifact"]["public_url"],
            json!("https://v3.elepcloud.com/generated-artifacts/report/index.html")
        );
        assert_eq!(
            payload["current_artifact"]["type"],
            json!("static_page_draft")
        );
        assert_eq!(
            payload["current_artifact"]["publicUrl"],
            json!("https://v3.elepcloud.com/generated-artifacts/report/index.html")
        );
        assert_eq!(payload["current_artifact"]["moduleCount"], json!(2));
        assert_eq!(
            payload["safety"]["v3_product_repo_write_allowed"],
            json!(false)
        );

        assert!(assistant_run_customer_codex_sidecar_workspace_seed(
            CODEX_CAPABILITY_CUSTOMER_ARTIFACT_REQUEST,
            Some(&current_artifact),
        )
        .is_none());
        assert!(assistant_run_customer_codex_sidecar_workspace_seed(
            CODEX_CAPABILITY_GENERATED_STATIC_PAGE_EDIT,
            None,
        )
        .is_none());
        assert!(assistant_run_customer_codex_sidecar_workspace_seed(
            CODEX_CAPABILITY_GENERATED_STATIC_PAGE_EDIT,
            Some(&json!({"type": "static_page_draft"})),
        )
        .is_none());
    }

    #[test]
    fn assistant_run_customer_codex_sidecar_context_payload_preserves_shape() {
        let selected_scope = json!({
            "type": "selected_scope",
            "intent": "artifact_edit",
            "datasets": ["00000000-0000-0000-0000-000000000101"],
            "documents": ["00000000-0000-0000-0000-000000000202"],
            "databaseSources": ["hy-sql"]
        });
        let evidence_state = json!({
            "status": "supplied",
            "supplied_items": [{"id": "ev-1"}],
            "supply_quality": {"status": "usable"}
        });
        let current_artifact = json!({
            "artifactType": "static_page",
            "publicUrl": "https://v3.elepcloud.com/generated-artifacts/report/index.html"
        });

        let payload = assistant_run_customer_codex_sidecar_context_payload(
            CODEX_CAPABILITY_GENERATED_STATIC_PAGE_EDIT,
            &selected_scope,
            &evidence_state,
            current_artifact.clone(),
        );

        assert_eq!(
            payload,
            json!({
                "version": 1,
                "route": "assistant_run_generated_static_page_edit",
                "non_blocking": true,
                "main_answer_path_preserved": true,
                "permission_scope": "workspace-write only inside the isolated task workspace seeded from the current generated static page artifact; no V3 repo writes",
                "selected_scope_summary": {
                    "intent": "artifact_edit",
                    "dataset_count": 1,
                    "document_count": 1,
                    "has_database_sources": true,
                    "scope_type": "selected_scope"
                },
                "evidence_summary": {
                    "status": "supplied",
                    "supplied_item_count": 1,
                    "supply_quality": {"status": "usable"}
                },
                "current_artifact": current_artifact
            })
        );

        let no_artifact = assistant_run_customer_codex_sidecar_context_payload(
            CODEX_CAPABILITY_CUSTOMER_COMPLEX_REQUEST,
            &json!({}),
            &json!({}),
            Value::Null,
        );
        assert_eq!(no_artifact["current_artifact"], Value::Null);
        assert_eq!(
            no_artifact["route"],
            json!("assistant_run_customer_complex_request")
        );
    }

    #[test]
    fn assistant_run_customer_codex_sidecar_fixed_task_context_routes_only_data_ingestion() {
        let tenant_id = TenantId::new();
        let assistant_run_id = AssistantRunId::new();
        let execution_id = WorkflowExecutionId::new();
        let selected_scope = json!({
            "datasets": ["00000000-0000-0000-0000-000000000101"],
            "databaseSources": ["hy-sql"]
        });

        let fixed_task = assistant_run_customer_codex_sidecar_fixed_task_context(
            tenant_id,
            assistant_run_id,
            execution_id,
            CODEX_CAPABILITY_DATA_INGESTION_ANALYSIS,
            "接入业务库并生成质量报告。",
            &selected_scope,
            &json!({"supplied_items": [{"id": "ev-1"}]}),
        )
        .expect("data-ingestion capability should produce a fixed task");

        assert_eq!(
            fixed_task.template_id,
            CodexHostFixedTaskTemplateIdView::DataIngestionAnalysis
        );
        assert_eq!(
            fixed_task.case_id,
            Some(format!("assistant-run-data-ingestion-{execution_id}"))
        );
        assert_eq!(
            fixed_task.dataset_scope["database_source_ids"],
            json!(["hy-sql"])
        );

        assert!(assistant_run_customer_codex_sidecar_fixed_task_context(
            tenant_id,
            assistant_run_id,
            execution_id,
            CODEX_CAPABILITY_CUSTOMER_ARTIFACT_REQUEST,
            "生成客户产物包。",
            &selected_scope,
            &json!({})
        )
        .is_none());
    }

    #[test]
    fn assistant_run_data_ingestion_sidecar_dataset_scope_collects_visible_sources() {
        let tenant_id = TenantId::new();
        let dataset_id = "00000000-0000-0000-0000-000000000101";
        let document_id = "00000000-0000-0000-0000-000000000202";
        let selected_scope = json!({
            "datasets": [dataset_id],
            "documents": [document_id],
            "databaseSources": [{"id": "hy-sql"}, "crm-db"],
            "uploadedFiles": [{"file_id": "file-001"}],
            "selectedTables": [{"table_id": "bi_traffic_area"}]
        });

        let dataset_scope =
            assistant_run_data_ingestion_sidecar_dataset_scope(tenant_id, &selected_scope);

        assert_eq!(dataset_scope["tenant_id"], json!(tenant_id.to_string()));
        assert_eq!(dataset_scope["dataset_ids"], json!([dataset_id]));
        assert_eq!(dataset_scope["selected_document_ids"], json!([document_id]));
        assert_eq!(
            dataset_scope["database_source_ids"],
            json!(["crm-db", "hy-sql"])
        );
        assert_eq!(dataset_scope["uploaded_file_ids"], json!(["file-001"]));
        assert_eq!(dataset_scope["table_ids"], json!(["bi_traffic_area"]));
        assert_eq!(
            dataset_scope["scope_source"],
            json!("v3_main_assistant_selected_scope")
        );
        assert!(assistant_run_data_ingestion_sidecar_scope_has_source(
            &selected_scope
        ));
        assert!(!assistant_run_data_ingestion_sidecar_scope_has_source(
            &json!({})
        ));
    }

    #[test]
    fn assistant_run_data_ingestion_sidecar_evidence_summary_preserves_safe_shape() {
        let evidence_state = json!({
            "status": "supplied",
            "supplied_items": [{"id": "ev-1"}, {"id": "ev-2"}],
            "supply_quality": {"status": "strong", "suppliedItemCount": 2}
        });

        assert_eq!(
            assistant_run_data_ingestion_sidecar_evidence_summary(&evidence_state),
            json!({
                "source_visibility": "v3_main_assistant_selected_scope_only",
                "supplied_item_count": 2,
                "supply_quality": {"status": "strong", "suppliedItemCount": 2},
                "raw_credentials_supplied": false
            })
        );

        assert_eq!(
            assistant_run_data_ingestion_sidecar_evidence_summary(&json!({})),
            json!({
                "source_visibility": "v3_main_assistant_selected_scope_only",
                "supplied_item_count": 0,
                "supply_quality": Value::Null,
                "raw_credentials_supplied": false
            })
        );
    }

    #[test]
    fn assistant_run_data_ingestion_sidecar_requirements_preserve_template_contract() {
        let requirements = assistant_run_data_ingestion_sidecar_requirements(
            "请接入客户数据库，生成经营分析数据集和校验方案。",
        );

        assert_eq!(
            requirements,
            json!({
                "user_goal": "请接入客户数据库，生成经营分析数据集和校验方案。",
                "intent": "data_ingestion_analysis",
                "source": "v3_main_assistant_cc_sidecar",
                "target_dataset_required": true,
                "target_dataset_resolution": "use_selected_dataset_when_available_or_propose_one_datamax_dataset_to_create_or_attach",
                "requested_outputs": [
                    "data_quality_report",
                    "field_mapping_plan",
                    "staging_spec",
                    "validation_checks",
                    "recommended_next_actions"
                ],
            })
        );

        let long_goal = "a".repeat(1300);
        let truncated = assistant_run_data_ingestion_sidecar_requirements(&long_goal);
        assert_eq!(
            truncated
                .get("user_goal")
                .and_then(Value::as_str)
                .unwrap()
                .chars()
                .count(),
            1200
        );
    }

    #[test]
    fn assistant_run_data_ingestion_sidecar_policies_preserve_safety_contract() {
        assert_eq!(
            assistant_run_data_ingestion_sidecar_policies(),
            json!({
                "mode": "read_only_analysis_or_staging_spec",
                "credential_policy": "do_not_request_or_emit_credentials",
                "production_write_policy": "needs_human_confirmation",
                "public_api_change_allowed": false,
                "schema_change_allowed_without_confirmation": false
            })
        );
    }

    #[test]
    fn assistant_run_data_ingestion_sidecar_fixed_task_context_preserves_contract() {
        let tenant_id = TenantId::new();
        let assistant_run_id = AssistantRunId::new();
        let execution_id = WorkflowExecutionId::new();
        let selected_scope = json!({
            "datasets": ["00000000-0000-0000-0000-000000000101"],
            "databaseSources": ["hy-sql"]
        });
        let evidence_state = json!({
            "supplied_items": [{"id": "ev-1"}],
            "supply_quality": {"status": "partial"}
        });

        let context = assistant_run_data_ingestion_sidecar_fixed_task_context(
            tenant_id,
            assistant_run_id,
            execution_id,
            "接入业务库并生成数据质量报告。",
            &selected_scope,
            &evidence_state,
        );

        assert_eq!(
            context.template_id,
            CodexHostFixedTaskTemplateIdView::DataIngestionAnalysis
        );
        assert_eq!(context.version, 1);
        assert_eq!(context.assistant_run_id, Some(assistant_run_id.to_string()));
        assert_eq!(
            context.case_id,
            Some(format!("assistant-run-data-ingestion-{execution_id}"))
        );
        assert_eq!(
            context.dataset_scope["scope_source"],
            json!("v3_main_assistant_selected_scope")
        );
        assert_eq!(
            context.dataset_scope["database_source_ids"],
            json!(["hy-sql"])
        );
        assert_eq!(
            context.requirements["intent"],
            json!("data_ingestion_analysis")
        );
        assert_eq!(
            context.policies["credential_policy"],
            json!("do_not_request_or_emit_credentials")
        );
        assert_eq!(
            context.user_question,
            Some("接入业务库并生成数据质量报告。".to_string())
        );
        assert_eq!(
            context.evidence_summary["raw_credentials_supplied"],
            json!(false)
        );
        assert_eq!(context.image2, Value::Null);
        assert_eq!(context.trace_summary, Value::Null);
        assert_eq!(context.allowed_write_scope, None);
        assert_eq!(
            context.human_review_policy,
            CodexHostFixedTaskHumanReviewPolicyView::AutoForReadOnlyAnalysisOrStagingSpec
        );
    }

    #[test]
    fn customer_codex_preflight_preserves_env_gates() {
        let _guard = preflight_env_lock().lock().expect("preflight env lock");
        let _enabled = TestEnvVarRestore::remove("CODEX_HOST_TASK_ENABLED");
        let _task_allowlist = TestEnvVarRestore::remove("CODEX_HOST_TASK_ALLOWLIST");
        let _agent_allowlist =
            TestEnvVarRestore::remove("CODEX_HOST_AGENT_PROFILE_ALLOWED_CAPABILITIES");

        assert_eq!(
            assistant_run_customer_codex_sidecar_preflight(
                CODEX_CAPABILITY_CUSTOMER_ARTIFACT_REQUEST
            )
            .expect_err("disabled task gate should reject"),
            "codex_host_task_disabled"
        );

        let _enabled = TestEnvVarRestore::set("CODEX_HOST_TASK_ENABLED", "true");
        assert_eq!(
            assistant_run_customer_codex_sidecar_preflight(
                CODEX_CAPABILITY_CUSTOMER_ARTIFACT_REQUEST
            )
            .expect_err("missing task allowlist should reject"),
            "codex_host_task_not_allowlisted"
        );

        let _task_allowlist = TestEnvVarRestore::set(
            "CODEX_HOST_TASK_ALLOWLIST",
            CODEX_CAPABILITY_CUSTOMER_ARTIFACT_REQUEST,
        );
        assert_eq!(
            assistant_run_customer_codex_sidecar_preflight(
                CODEX_CAPABILITY_CUSTOMER_ARTIFACT_REQUEST
            )
            .expect_err("missing agent capability allowlist should reject"),
            "codex_host_agent_capability_not_allowlisted"
        );

        let _agent_allowlist = TestEnvVarRestore::set(
            "CODEX_HOST_AGENT_PROFILE_ALLOWED_CAPABILITIES",
            CODEX_CAPABILITY_CUSTOMER_ARTIFACT_REQUEST,
        );
        assert_eq!(
            assistant_run_customer_codex_sidecar_preflight(
                CODEX_CAPABILITY_CUSTOMER_ARTIFACT_REQUEST
            ),
            Ok(())
        );
    }
}
