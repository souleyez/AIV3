use chrono::{DateTime, Utc};
use contracts::{
    AssetProfileParseTaskPayload, CreateFashionDesignImageAssetImportBatchRequest,
    CreateFashionDesignImageAssetImportBatchResponse, CreateFashionDesignImageAssetImportRequest,
    CreateFashionDesignImageAssetImportResponse, FashionDesignImageAssetImportItem,
    FashionDesignImageAssetImportPackage,
};
use domain_model::{
    DatasetId, TenantId, WorkflowEventId, WorkflowEventRecord, WorkflowExecution,
    WorkflowExecutionId, WorkflowKind, WorkflowStatus,
};
use event_bus::{workflow_task_enqueued_subject, EventEnvelope};
use serde_json::{json, Map, Value};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path as StdPath, PathBuf},
};
use storage::{
    AssetItemRecord, AssetParseRunRecord, AssetProfileRecord, DatasetAssetMembershipRecord,
    NewAssetItem, NewAssetParseRun, NewAssetProfile, NewDatasetAssetMembership, NewWorkflowTask,
};
use uuid::Uuid;
use zip::ZipArchive;

use crate::{
    asset_library_view_support::{
        asset_item_view, asset_parse_run_view, asset_profile_view, dataset_asset_membership_view,
    },
    document_local_object_support::resolve_platform_local_object_path,
    external_document_object_support::safe_external_path_segment,
    fashion_postchain_adapter_support::{
        fashion_design_image_parser_worker_input_contract, fashion_postchain_profile_attributes,
        FashionDesignImageParserWorkerInput, FASHION_DESIGN_IMAGE_PARSER_NAME,
        FASHION_DESIGN_IMAGE_PARSER_TASK_KIND, FASHION_DESIGN_IMAGE_PARSER_VERSION,
        FASHION_DESIGN_IMAGE_PROFILE_KIND,
    },
    sha256_hex,
    zip_ingest_support::{
        infer_zip_child_content_type, safe_zip_entry_output_name, zip_entry_should_skip,
        zip_entry_title, zip_ingest_env_u64, zip_ingest_env_usize,
    },
    ApiError, AppState,
};

#[derive(Clone, Debug)]
pub(crate) struct FashionDesignImageAssetImportInput {
    pub dataset_id: DatasetId,
    pub asset_library_id: Option<Uuid>,
    pub collection_id: Option<Uuid>,
    pub stable_source_id_override: Option<String>,
    pub external_id: Option<String>,
    pub title: String,
    pub image_url: Option<String>,
    pub object_key: Option<String>,
    pub content_type: Option<String>,
    pub profile_payload: Value,
    pub metadata: Value,
}

#[derive(Clone, Debug)]
pub(crate) struct PreparedFashionDesignImageAssetImport {
    pub asset: NewAssetItem,
    pub dataset_membership: NewDatasetAssetMembership,
    pub parse_run: NewAssetParseRun,
    pub profile: NewAssetProfile,
}

#[derive(Clone, Debug)]
pub(crate) struct SyncedFashionDesignImageAssetImport {
    pub asset: AssetItemRecord,
    pub dataset_membership: DatasetAssetMembershipRecord,
    pub parse_run: AssetParseRunRecord,
    pub profile: AssetProfileRecord,
}

fn asset_parse_max_attempts() -> u32 {
    std::env::var("ASSET_PARSE_MAX_ATTEMPTS")
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .filter(|value| (1..=5).contains(value))
        .unwrap_or(2)
}

fn asset_parse_workflow_version(
    workflow_catalog: &workflow_engine::WorkflowCatalog,
) -> std::result::Result<String, ApiError> {
    workflow_catalog
        .find_definition(WorkflowKind::UploadIngest)
        .map(|definition| definition.version().to_string())
        .ok_or_else(|| {
            ApiError::internal(
                "workflow_definition_missing",
                "upload_ingest workflow definition is not registered".to_string(),
            )
        })
}

fn build_asset_parse_enqueue_task(
    enabled: bool,
    asset_id: Uuid,
    parse_run_id: Uuid,
    parser_name: &str,
    parser_version: &str,
    parse_status: &str,
    queued_at: DateTime<Utc>,
    max_attempts: u32,
) -> Option<NewWorkflowTask> {
    if !enabled || parse_status != "pending" {
        return None;
    }
    let dedupe_key = asset_parse_task_dedupe_key(asset_id, parser_name, parser_version);
    let payload = AssetProfileParseTaskPayload {
        asset_id: asset_id.to_string(),
        parse_run_id: parse_run_id.to_string(),
        parser_name: parser_name.to_string(),
        parser_version: parser_version.to_string(),
        profile_kind: FASHION_DESIGN_IMAGE_PROFILE_KIND.to_string(),
        dedupe_key,
    };
    Some(NewWorkflowTask {
        queue: contracts::ASSET_PROFILE_PARSE_QUEUE.to_string(),
        task_key: contracts::ASSET_PROFILE_PARSE_TASK_KEY.to_string(),
        payload: serde_json::to_value(payload).ok()?,
        available_at: queued_at,
        max_attempts: max_attempts.clamp(1, 5),
    })
}

fn asset_parse_task_dedupe_key(asset_id: Uuid, parser_name: &str, parser_version: &str) -> String {
    let material = format!("{asset_id}:{parser_name}:{parser_version}");
    sha256_hex([material.as_bytes()])
}

fn asset_parse_runnable_dedupe_key(
    tenant_id: TenantId,
    asset_id: Uuid,
    parser_name: &str,
    parser_version: &str,
) -> String {
    let material = format!("{}:{asset_id}:{parser_name}:{parser_version}", tenant_id.0);
    sha256_hex([material.as_bytes()])
}

async fn enqueue_asset_parse_if_enabled(
    state: &AppState,
    tenant_id: TenantId,
    dataset_id: DatasetId,
    asset: &AssetItemRecord,
    parse_run: &AssetParseRunRecord,
) -> std::result::Result<(), ApiError> {
    let queued_at = Utc::now();
    let Some(mut task) = build_asset_parse_enqueue_task(
        crate::env_flag("ASSET_PARSE_ENABLED", false),
        asset.id,
        parse_run.id,
        &parse_run.parser_name,
        &parse_run.parser_version,
        &parse_run.status,
        queued_at,
        asset_parse_max_attempts(),
    ) else {
        return Ok(());
    };
    let dedupe_key = asset_parse_runnable_dedupe_key(
        tenant_id,
        asset.id,
        &parse_run.parser_name,
        &parse_run.parser_version,
    );
    task.payload["dedupe_key"] = json!(dedupe_key);
    let execution_id = WorkflowExecutionId::new();
    let execution = WorkflowExecution {
        id: execution_id,
        tenant_id,
        dataset_id: Some(dataset_id),
        report_plan_id: None,
        kind: WorkflowKind::UploadIngest,
        version: asset_parse_workflow_version(&state.workflow_catalog)?,
        stage: contracts::ASSET_PROFILE_PARSE_TASK_KEY.to_string(),
        status: WorkflowStatus::Running,
        attempt: 0,
        context: json!({
            "workflow_envelope": "asset_profile_parse",
            "asset_id": asset.id,
            "parse_run_id": parse_run.id,
            "parser_name": parse_run.parser_name,
            "parser_version": parse_run.parser_version,
            "dedupe_key": dedupe_key,
        }),
        created_at: queued_at,
        updated_at: queued_at,
    };
    let initial_event = WorkflowEventRecord {
        id: WorkflowEventId::new(),
        execution_id,
        sequence_no: 1,
        event_name: "asset_profile_parse.queued".to_string(),
        payload: json!({
            "asset_id": asset.id,
            "parse_run_id": parse_run.id,
            "parser_name": parse_run.parser_name,
            "parser_version": parse_run.parser_version,
            "queue": task.queue,
            "task_key": task.task_key,
        }),
        created_at: queued_at,
    };
    let persisted = state
        .storage
        .workflow_tasks()
        .create_asset_parse_if_absent(&execution, &initial_event, &task, &dedupe_key, queued_at)
        .await
        .map_err(ApiError::from_storage)?;
    if let Some(persisted) = persisted {
        state
            .event_bus
            .publish(EventEnvelope {
                subject: workflow_task_enqueued_subject(
                    contracts::ASSET_PROFILE_PARSE_QUEUE,
                    contracts::ASSET_PROFILE_PARSE_TASK_KEY,
                ),
                payload: json!({
                    "task_id": persisted.id,
                    "tenant_id": persisted.tenant_id,
                    "execution_id": persisted.execution_id,
                    "queue": persisted.queue,
                    "task_key": persisted.task_key,
                    "status": persisted.status.as_str(),
                    "available_at": persisted.available_at,
                }),
                published_at: persisted.created_at,
            })
            .await;
    }
    Ok(())
}

pub(crate) fn fashion_design_asset_import_live_execute_operator_manifest_dry_run(
    auth_supplied: bool,
    dataset_id_supplied: bool,
    asset_library_id_supplied: bool,
    ack_live_write: bool,
    approval_id_supplied: bool,
    local_thread_id_supplied: bool,
) -> Value {
    let required_inputs_satisfied = auth_supplied
        && dataset_id_supplied
        && asset_library_id_supplied
        && ack_live_write
        && approval_id_supplied;
    json!({
        "contract": "fashion_design_asset_import_live_execute_operator_manifest_dry_run_v1",
        "mode": "dry_run",
        "no_write": true,
        "action": "fashion_design_image_asset_import_live_execute",
        "execute_ready": required_inputs_satisfied,
        "required_inputs": {
            "v3_user_session_required": true,
            "v3_user_session_supplied": auth_supplied,
            "existing_dataset_required": true,
            "existing_dataset_supplied": dataset_id_supplied,
            "existing_asset_library_required": true,
            "existing_asset_library_supplied": asset_library_id_supplied,
            "ack_live_write_required": true,
            "ack_live_write_supplied": ack_live_write,
            "approval_id_required": true,
            "approval_id_supplied": approval_id_supplied,
            "local_thread_id_optional": true,
            "local_thread_id_supplied": local_thread_id_supplied,
        },
        "planned_steps": [
            {
                "step": "upload_png_and_zip_fixture",
                "requires": ["v3_user_session", "ack_live_write", "approval_id"],
                "write_scope": "temporary_upload_objects",
            },
            {
                "step": "attach_existing_dataset_to_existing_asset_library",
                "requires": ["existing_dataset", "existing_asset_library", "dataset_manage_permission"],
                "write_scope": "dataset_asset_library_membership",
            },
            {
                "step": "create_fashion_design_asset_import_batch",
                "requires": ["uploaded_fixture_objects", "existing_dataset", "existing_asset_library"],
                "write_scope": "asset_items_dataset_memberships_parse_runs_profiles",
            },
            {
                "step": "read_asset_library_scope_summary",
                "requires": ["existing_asset_library"],
                "write_scope": "read_only_verification",
            }
        ],
        "review_gates": {
            "operator_review_required": true,
            "approval_id_hash_only_in_receipts": true,
            "manifest_must_be_reviewed_before_execute": true,
            "dataset_and_asset_library_must_already_exist": true,
            "dataset_must_be_visible_and_manageable_by_session_user": true,
            "asset_library_must_belong_to_same_tenant": true,
            "execute_requires_ack_live_write": true,
        },
        "rollback_plan": {
            "operator_review_required": true,
            "rollback_strategy": "delete_or_archive_smoke_assets_and_memberships_by_approval_hash_after_review",
            "cleanup_manifest_required": true,
            "automatic_cleanup_allowed": false,
            "manual_db_changes_allowed_without_review": false,
        },
        "audit_requirements": {
            "record_approval_id_hash": true,
            "record_counts_only": true,
            "record_dataset_id_value": false,
            "record_asset_library_id_value": false,
            "record_auth_material": false,
            "record_object_locators": false,
            "record_source_urls": false,
            "record_local_paths": false,
        },
        "redaction": {
            "raw_session_excluded": true,
            "raw_dataset_id_excluded": true,
            "raw_asset_library_id_excluded": true,
            "raw_approval_id_excluded": true,
            "raw_object_locator_excluded": true,
            "raw_url_excluded": true,
            "local_path_excluded": true,
        },
        "production_write_allowed": false,
    })
}

pub(crate) fn fashion_design_asset_import_post_execute_cleanup_manifest_dry_run(
    approval_hash_supplied: bool,
    live_receipt_supplied: bool,
    imported_asset_count: usize,
    dataset_membership_count: usize,
    parse_run_count: usize,
    profile_count: usize,
) -> Value {
    let counts_present = imported_asset_count > 0
        && dataset_membership_count > 0
        && parse_run_count > 0
        && profile_count > 0;
    let cleanup_manifest_ready = approval_hash_supplied && live_receipt_supplied && counts_present;
    json!({
        "contract": "fashion_design_asset_import_post_execute_cleanup_manifest_dry_run_v1",
        "mode": "dry_run",
        "no_write": true,
        "no_delete": true,
        "action": "fashion_design_image_asset_import_cleanup_manifest",
        "cleanup_manifest_ready": cleanup_manifest_ready,
        "required_inputs": {
            "approval_hash_required": true,
            "approval_hash_supplied": approval_hash_supplied,
            "live_execute_receipt_required": true,
            "live_execute_receipt_supplied": live_receipt_supplied,
            "counts_required": true,
            "counts_supplied": counts_present,
        },
        "receipt_shape": {
            "record_counts_only": true,
            "approval_hash_value_excluded_from_shared_receipt": true,
            "raw_asset_ids_excluded_from_shared_receipt": true,
            "raw_dataset_ids_excluded_from_shared_receipt": true,
            "raw_asset_library_ids_excluded_from_shared_receipt": true,
            "raw_object_locators_excluded_from_shared_receipt": true,
            "raw_source_urls_excluded_from_shared_receipt": true,
        },
        "selection_policy": {
            "scope": "same_tenant_reviewed_smoke_records_only",
            "match_by_approval_hash": true,
            "match_by_parser": FASHION_DESIGN_IMAGE_PARSER_NAME,
            "match_by_parser_version": FASHION_DESIGN_IMAGE_PARSER_VERSION,
            "match_by_profile_kind": FASHION_DESIGN_IMAGE_PROFILE_KIND,
            "private_operator_manifest_may_include_record_ids_after_review": true,
            "shared_receipt_must_not_include_record_ids": true,
        },
        "counts": {
            "asset_count": imported_asset_count,
            "dataset_membership_count": dataset_membership_count,
            "parse_run_count": parse_run_count,
            "profile_count": profile_count,
        },
        "planned_cleanup_actions": [
            {
                "step": "review_matched_smoke_assets",
                "write_scope": "read_only_verification",
            },
            {
                "step": "remove_or_archive_dataset_asset_memberships",
                "write_scope": "dataset_asset_memberships",
            },
            {
                "step": "remove_or_archive_asset_profiles",
                "write_scope": "asset_profiles",
            },
            {
                "step": "mark_or_remove_parse_runs",
                "write_scope": "asset_parse_runs",
            },
            {
                "step": "remove_or_archive_asset_items",
                "write_scope": "asset_items",
            },
            {
                "step": "review_temporary_upload_objects",
                "write_scope": "temporary_upload_objects_after_storage_review",
            }
        ],
        "execution_controls": {
            "operator_review_required": true,
            "cleanup_manifest_must_be_reviewed_before_action": true,
            "automatic_cleanup_allowed": false,
            "manual_db_changes_allowed_without_review": false,
            "temporary_object_cleanup_requires_storage_review": true,
        },
        "audit_requirements": {
            "record_approval_hash": true,
            "record_counts_only": true,
            "record_auth_material": false,
            "record_object_locators": false,
            "record_source_urls": false,
            "record_local_paths": false,
        },
        "redaction": {
            "raw_approval_hash_value_excluded": true,
            "raw_asset_ids_excluded": true,
            "raw_dataset_ids_excluded": true,
            "raw_asset_library_ids_excluded": true,
            "raw_object_locator_excluded": true,
            "raw_url_excluded": true,
            "local_path_excluded": true,
        },
        "production_write_allowed": false,
    })
}

pub(crate) fn fashion_design_asset_import_operator_handoff_summary_dry_run(
    execute_ready: bool,
    live_receipt_supplied: bool,
    cleanup_manifest_ready: bool,
    cleanup_auto_allowed: bool,
    manual_review_required: bool,
    imported_asset_count: usize,
) -> Value {
    let execute_state = if live_receipt_supplied {
        "executed_receipt_available"
    } else if execute_ready {
        "ready_for_controlled_execute"
    } else {
        "blocked_missing_execute_inputs"
    };
    let cleanup_state = if cleanup_manifest_ready {
        "cleanup_manifest_ready_for_review"
    } else if live_receipt_supplied {
        "cleanup_manifest_waiting_for_counts_or_review"
    } else {
        "cleanup_waiting_for_live_receipt"
    };
    let next_operator_action = if !execute_ready {
        "supply_reviewed_session_dataset_asset_library_ack_and_approval"
    } else if !live_receipt_supplied {
        "review_manifest_then_run_controlled_execute"
    } else if cleanup_manifest_ready {
        "review_cleanup_manifest_before_any_cleanup"
    } else {
        "review_live_receipt_and_build_cleanup_manifest"
    };
    json!({
        "contract": "fashion_design_asset_import_operator_handoff_summary_dry_run_v1",
        "mode": "dry_run",
        "no_write": true,
        "no_delete": true,
        "no_callback": true,
        "no_task_creation": true,
        "public_contract_changed": false,
        "production_write_allowed": false,
        "task_card_summary": {
            "status": if live_receipt_supplied { "executed" } else if execute_ready { "ready" } else { "waiting_for_inputs" },
            "execute_state": execute_state,
            "cleanup_state": cleanup_state,
            "next_operator_action": next_operator_action,
            "manual_review_required": manual_review_required,
            "cleanup_auto_allowed": cleanup_auto_allowed,
            "imported_asset_count": imported_asset_count,
        },
        "validation_summary": {
            "execute_ready": execute_ready,
            "live_receipt_supplied": live_receipt_supplied,
            "cleanup_manifest_ready": cleanup_manifest_ready,
            "cleanup_auto_allowed": cleanup_auto_allowed,
            "manual_review_required": manual_review_required,
            "imported_asset_count": imported_asset_count,
            "safe_for_shared_receipt": true,
        },
        "operator_controls": {
            "execute_requires_manifest_review": true,
            "cleanup_requires_manifest_review": true,
            "cleanup_auto_allowed": cleanup_auto_allowed,
            "callback_allowed": false,
            "task_creation_allowed": false,
        },
        "redaction": {
            "raw_session_excluded": true,
            "raw_dataset_id_excluded": true,
            "raw_asset_library_id_excluded": true,
            "raw_approval_id_excluded": true,
            "raw_approval_hash_value_excluded": true,
            "raw_asset_ids_excluded": true,
            "raw_object_locator_excluded": true,
            "raw_url_excluded": true,
            "local_path_excluded": true,
        },
    })
}

pub(crate) fn fashion_design_asset_import_operator_handoff_contract_drift_guard_dry_run(
    detail_status: &str,
    execute_state: &str,
    cleanup_state: &str,
) -> Value {
    let allowed_detail_statuses = ["waiting_for_inputs", "ready", "executed"];
    let allowed_execute_states = [
        "blocked_missing_execute_inputs",
        "ready_for_controlled_execute",
        "executed_receipt_available",
    ];
    let allowed_cleanup_states = [
        "cleanup_waiting_for_live_receipt",
        "cleanup_manifest_waiting_for_counts_or_review",
        "cleanup_manifest_ready_for_review",
    ];
    let detail_status_allowed = allowed_detail_statuses.contains(&detail_status);
    let execute_state_allowed = allowed_execute_states.contains(&execute_state);
    let cleanup_state_allowed = allowed_cleanup_states.contains(&cleanup_state);
    json!({
        "contract": "fashion_design_asset_import_operator_handoff_contract_drift_guard_dry_run_v1",
        "mode": "dry_run",
        "surface": "task_card_detail_panel_only",
        "no_write": true,
        "no_delete": true,
        "no_callback": true,
        "no_task_creation": true,
        "public_contract_changed": false,
        "production_write_allowed": false,
        "existing_task_card_status_enum_mutation_allowed": false,
        "new_task_card_creation_allowed": false,
        "third_party_callback_allowed": false,
        "detail_status": detail_status,
        "execute_state": execute_state,
        "cleanup_state": cleanup_state,
        "allowed_detail_statuses": allowed_detail_statuses,
        "allowed_execute_states": allowed_execute_states,
        "allowed_cleanup_states": allowed_cleanup_states,
        "validation": {
            "detail_status_allowed": detail_status_allowed,
            "execute_state_allowed": execute_state_allowed,
            "cleanup_state_allowed": cleanup_state_allowed,
            "safe_for_task_card_detail": detail_status_allowed && execute_state_allowed && cleanup_state_allowed,
            "safe_for_validation_receipt": true,
        },
        "field_contract": {
            "allowed_top_level_fields": [
                "contract",
                "mode",
                "surface",
                "task_card_detail",
                "validation_detail",
                "operator_controls",
                "redaction"
            ],
            "summary_must_not_replace_task_card_status": true,
            "summary_must_not_emit_external_sse": true,
            "summary_must_not_trigger_callback": true,
        },
        "operator_controls": {
            "detail_display_only": true,
            "callback_allowed": false,
            "task_creation_allowed": false,
            "task_status_mutation_allowed": false,
            "cleanup_auto_allowed": false,
        },
        "redaction": {
            "raw_session_excluded": true,
            "raw_dataset_id_excluded": true,
            "raw_asset_library_id_excluded": true,
            "raw_approval_id_excluded": true,
            "raw_approval_hash_value_excluded": true,
            "raw_asset_ids_excluded": true,
            "raw_object_locator_excluded": true,
            "raw_url_excluded": true,
            "local_path_excluded": true,
        },
    })
}

pub(crate) fn fashion_design_asset_import_operator_readiness_rollup_dry_run(
    execute_manifest: &Value,
    handoff_guard: &Value,
    cleanup_manifest: &Value,
) -> Value {
    let execute_manifest_contract_ok = execute_manifest["contract"]
        == json!("fashion_design_asset_import_live_execute_operator_manifest_dry_run_v1");
    let handoff_guard_contract_ok = handoff_guard["contract"]
        == json!("fashion_design_asset_import_operator_handoff_contract_drift_guard_dry_run_v1");
    let cleanup_manifest_contract_ok = cleanup_manifest["contract"]
        == json!("fashion_design_asset_import_post_execute_cleanup_manifest_dry_run_v1");
    let execute_ready = execute_manifest["execute_ready"].as_bool().unwrap_or(false);
    let handoff_detail_safe = handoff_guard["validation"]["safe_for_task_card_detail"]
        .as_bool()
        .unwrap_or(false);
    let cleanup_manifest_ready = cleanup_manifest["cleanup_manifest_ready"]
        .as_bool()
        .unwrap_or(false);
    let cleanup_auto_allowed = cleanup_manifest["execution_controls"]["automatic_cleanup_allowed"]
        .as_bool()
        .unwrap_or(false);
    let live_receipt_supplied = cleanup_manifest["required_inputs"]
        ["live_execute_receipt_supplied"]
        .as_bool()
        .unwrap_or(false);
    let input_contracts_ok =
        execute_manifest_contract_ok && handoff_guard_contract_ok && cleanup_manifest_contract_ok;
    let side_effect_guards_ok = execute_manifest["no_write"].as_bool().unwrap_or(false)
        && handoff_guard["no_write"].as_bool().unwrap_or(false)
        && handoff_guard["no_delete"].as_bool().unwrap_or(false)
        && handoff_guard["no_callback"].as_bool().unwrap_or(false)
        && handoff_guard["no_task_creation"].as_bool().unwrap_or(false)
        && !handoff_guard["public_contract_changed"]
            .as_bool()
            .unwrap_or(true)
        && cleanup_manifest["no_write"].as_bool().unwrap_or(false)
        && cleanup_manifest["no_delete"].as_bool().unwrap_or(false)
        && !cleanup_auto_allowed
        && !execute_manifest["production_write_allowed"]
            .as_bool()
            .unwrap_or(true)
        && !handoff_guard["production_write_allowed"]
            .as_bool()
            .unwrap_or(true)
        && !cleanup_manifest["production_write_allowed"]
            .as_bool()
            .unwrap_or(true);
    let ready_for_operator_execute = input_contracts_ok
        && side_effect_guards_ok
        && execute_ready
        && handoff_detail_safe
        && !live_receipt_supplied;
    let ready_for_operator_cleanup_review = input_contracts_ok
        && side_effect_guards_ok
        && handoff_detail_safe
        && live_receipt_supplied
        && cleanup_manifest_ready
        && !cleanup_auto_allowed;
    let overall_status = if !input_contracts_ok {
        "blocked_contract_mismatch"
    } else if !side_effect_guards_ok || !handoff_detail_safe {
        "blocked_safety_guard"
    } else if ready_for_operator_cleanup_review {
        "ready_for_cleanup_review"
    } else if live_receipt_supplied {
        "executed_waiting_for_cleanup_manifest"
    } else if ready_for_operator_execute {
        "ready_for_execute_review"
    } else {
        "waiting_for_execute_inputs"
    };
    let next_operator_action = match overall_status {
        "blocked_contract_mismatch" => "refresh_dry_run_inputs_before_review",
        "blocked_safety_guard" => "fix_guard_or_manifest_before_review",
        "ready_for_cleanup_review" => "review_cleanup_manifest_before_any_cleanup",
        "executed_waiting_for_cleanup_manifest" => "build_or_complete_cleanup_manifest",
        "ready_for_execute_review" => "review_rollup_and_manifest_then_run_controlled_execute",
        _ => "supply_reviewed_session_dataset_asset_library_ack_and_approval",
    };
    json!({
        "contract": "fashion_design_asset_import_operator_readiness_rollup_dry_run_v1",
        "mode": "dry_run",
        "no_write": true,
        "no_delete": true,
        "no_callback": true,
        "no_task_creation": true,
        "public_contract_changed": false,
        "production_write_allowed": false,
        "inputs": {
            "execute_manifest_contract_ok": execute_manifest_contract_ok,
            "handoff_guard_contract_ok": handoff_guard_contract_ok,
            "cleanup_manifest_contract_ok": cleanup_manifest_contract_ok,
            "input_contracts_ok": input_contracts_ok,
            "execute_ready": execute_ready,
            "handoff_detail_safe": handoff_detail_safe,
            "cleanup_manifest_ready": cleanup_manifest_ready,
            "cleanup_auto_allowed": cleanup_auto_allowed,
            "live_receipt_supplied": live_receipt_supplied,
        },
        "readiness": {
            "overall_status": overall_status,
            "ready_for_operator_execute": ready_for_operator_execute,
            "ready_for_operator_cleanup_review": ready_for_operator_cleanup_review,
            "side_effect_guards_ok": side_effect_guards_ok,
            "next_operator_action": next_operator_action,
            "manual_review_required": true,
        },
        "release_gate_summary": {
            "reviewed_execute_manifest_required": true,
            "reviewed_cleanup_manifest_required_after_execute": true,
            "public_contract_change_allowed": false,
            "external_callback_allowed": false,
            "task_creation_allowed": false,
            "automatic_cleanup_allowed": false,
            "record_counts_only": true,
            "safe_for_validation_receipt": input_contracts_ok && side_effect_guards_ok,
        },
        "operator_controls": {
            "execute_requires_operator_review": true,
            "cleanup_requires_operator_review": true,
            "live_execute_requires_ack": true,
            "cleanup_auto_allowed": false,
            "callback_allowed": false,
            "task_creation_allowed": false,
            "status_mutation_allowed": false,
        },
        "redaction": {
            "raw_session_excluded": true,
            "raw_dataset_id_excluded": true,
            "raw_asset_library_id_excluded": true,
            "raw_approval_id_excluded": true,
            "raw_approval_hash_value_excluded": true,
            "raw_asset_ids_excluded": true,
            "raw_object_locator_excluded": true,
            "raw_url_excluded": true,
            "local_path_excluded": true,
        },
    })
}

pub(crate) fn fashion_design_asset_import_operator_readiness_contract_drift_guard_dry_run(
    readiness_rollup: &Value,
    command_template_redacted: bool,
    public_docs_asset_imports_open: bool,
) -> Value {
    let rollup_contract_ok = readiness_rollup["contract"]
        == json!("fashion_design_asset_import_operator_readiness_rollup_dry_run_v1");
    let rollup_side_effect_guards_ok = readiness_rollup["no_write"].as_bool().unwrap_or(false)
        && readiness_rollup["no_delete"].as_bool().unwrap_or(false)
        && readiness_rollup["no_callback"].as_bool().unwrap_or(false)
        && readiness_rollup["no_task_creation"]
            .as_bool()
            .unwrap_or(false)
        && !readiness_rollup["public_contract_changed"]
            .as_bool()
            .unwrap_or(true)
        && !readiness_rollup["production_write_allowed"]
            .as_bool()
            .unwrap_or(true)
        && readiness_rollup["release_gate_summary"]["safe_for_validation_receipt"]
            .as_bool()
            .unwrap_or(false);
    let rollup_contains_command_material = false;
    let public_docs_asset_imports_closed = !public_docs_asset_imports_open;
    let safe_for_private_operator_review = rollup_contract_ok
        && rollup_side_effect_guards_ok
        && !rollup_contains_command_material
        && command_template_redacted;
    let safe_for_public_docs = public_docs_asset_imports_closed;
    let drift_status = if !rollup_contract_ok {
        "blocked_rollup_contract_mismatch"
    } else if !rollup_side_effect_guards_ok {
        "blocked_rollup_safety_guard"
    } else if rollup_contains_command_material {
        "blocked_rollup_command_material"
    } else if !command_template_redacted {
        "blocked_unredacted_command_template"
    } else if !public_docs_asset_imports_closed {
        "blocked_public_docs_expose_unreviewed_asset_imports"
    } else {
        "safe_private_runbook_only"
    };
    json!({
        "contract": "fashion_design_asset_import_operator_readiness_contract_drift_guard_dry_run_v1",
        "mode": "dry_run",
        "surface": "private_operator_runbook_and_validation_only",
        "no_write": true,
        "no_delete": true,
        "no_callback": true,
        "no_task_creation": true,
        "public_contract_changed": false,
        "production_write_allowed": false,
        "drift_status": drift_status,
        "readiness_rollup_contract_ok": rollup_contract_ok,
        "readiness_rollup_side_effect_guards_ok": rollup_side_effect_guards_ok,
        "rollup_contains_command_material": rollup_contains_command_material,
        "command_template_redacted": command_template_redacted,
        "command_template_placeholder_only": command_template_redacted,
        "public_docs_asset_imports_open": public_docs_asset_imports_open,
        "public_docs_asset_imports_closed": public_docs_asset_imports_closed,
        "validation": {
            "safe_for_private_operator_review": safe_for_private_operator_review,
            "safe_for_public_docs": safe_for_public_docs,
            "safe_for_shared_validation_receipt": safe_for_private_operator_review && safe_for_public_docs,
        },
        "forbidden_public_material": {
            "base_url_included": false,
            "session_material_included": false,
            "dataset_value_included": false,
            "asset_library_value_included": false,
            "approval_value_included": false,
            "object_locator_value_included": false,
            "executable_command_args_included": false,
            "raw_command_included": false,
        },
        "operator_controls": {
            "private_runbook_only": true,
            "public_docs_change_allowed": false,
            "callback_allowed": false,
            "task_creation_allowed": false,
            "task_status_mutation_allowed": false,
            "cleanup_auto_allowed": false,
        },
        "redaction": {
            "raw_base_url_excluded": true,
            "raw_session_excluded": true,
            "raw_dataset_id_excluded": true,
            "raw_asset_library_id_excluded": true,
            "raw_approval_id_excluded": true,
            "raw_object_locator_excluded": true,
            "raw_command_excluded": true,
            "local_path_excluded": true,
        },
    })
}

pub(crate) fn fashion_design_asset_import_live_execute_private_runbook_receipt_package_dry_run(
    readiness_contract_guard: &Value,
) -> Value {
    let guard_contract_ok = readiness_contract_guard["contract"]
        == json!("fashion_design_asset_import_operator_readiness_contract_drift_guard_dry_run_v1");
    let guard_safe_for_shared_receipt = readiness_contract_guard["validation"]
        ["safe_for_shared_validation_receipt"]
        .as_bool()
        .unwrap_or(false);
    let command_template_redacted = readiness_contract_guard["command_template_redacted"]
        .as_bool()
        .unwrap_or(false);
    let public_docs_closed = readiness_contract_guard["public_docs_asset_imports_closed"]
        .as_bool()
        .unwrap_or(false);
    let raw_material_excluded = !readiness_contract_guard["forbidden_public_material"]
        ["base_url_included"]
        .as_bool()
        .unwrap_or(true)
        && !readiness_contract_guard["forbidden_public_material"]["session_material_included"]
            .as_bool()
            .unwrap_or(true)
        && !readiness_contract_guard["forbidden_public_material"]["dataset_value_included"]
            .as_bool()
            .unwrap_or(true)
        && !readiness_contract_guard["forbidden_public_material"]["asset_library_value_included"]
            .as_bool()
            .unwrap_or(true)
        && !readiness_contract_guard["forbidden_public_material"]["approval_value_included"]
            .as_bool()
            .unwrap_or(true)
        && !readiness_contract_guard["forbidden_public_material"]["object_locator_value_included"]
            .as_bool()
            .unwrap_or(true)
        && !readiness_contract_guard["forbidden_public_material"]
            ["executable_command_args_included"]
            .as_bool()
            .unwrap_or(true)
        && !readiness_contract_guard["forbidden_public_material"]["raw_command_included"]
            .as_bool()
            .unwrap_or(true);
    let shared_receipt_safe = guard_contract_ok
        && guard_safe_for_shared_receipt
        && raw_material_excluded
        && public_docs_closed;
    let private_runbook_ready = guard_contract_ok
        && command_template_redacted
        && raw_material_excluded
        && readiness_contract_guard["operator_controls"]["private_runbook_only"]
            .as_bool()
            .unwrap_or(false);
    let package_ready_for_review = private_runbook_ready && shared_receipt_safe;
    let package_status = if !guard_contract_ok {
        "blocked_guard_contract_mismatch"
    } else if !raw_material_excluded {
        "blocked_raw_material_present"
    } else if !private_runbook_ready {
        "blocked_private_runbook_not_ready"
    } else if !shared_receipt_safe {
        "blocked_shared_validation_receipt_not_safe"
    } else {
        "ready_for_operator_review"
    };
    json!({
        "contract": "fashion_design_asset_import_live_execute_private_runbook_receipt_package_dry_run_v1",
        "mode": "dry_run",
        "surface": "private_runbook_and_shared_validation_receipt_split",
        "no_write": true,
        "no_delete": true,
        "no_callback": true,
        "no_task_creation": true,
        "public_contract_changed": false,
        "production_write_allowed": false,
        "package_status": package_status,
        "private_operator_runbook": {
            "operator_only": true,
            "included_in_shared_validation_receipt": false,
            "contains_redacted_command_template": command_template_redacted,
            "contains_raw_session": false,
            "contains_raw_dataset": false,
            "contains_raw_asset_library": false,
            "contains_raw_approval": false,
            "contains_raw_object_locator": false,
            "pre_execute_review_required": true,
            "allowed_sections": [
                "redacted_command_template_placeholders",
                "required_inputs_checklist",
                "manual_review_gates",
                "rollback_requirements"
            ],
        },
        "shared_validation_receipt": {
            "contains_private_runbook": false,
            "contains_command_template": false,
            "contains_raw_session": false,
            "contains_raw_dataset": false,
            "contains_raw_asset_library": false,
            "contains_raw_approval": false,
            "contains_raw_object_locator": false,
            "contains_status_labels": true,
            "contains_counts_only": true,
            "public_docs_asset_imports_closed": public_docs_closed,
        },
        "separation": {
            "private_runbook_separate_from_shared_receipt": true,
            "shared_receipt_safe": shared_receipt_safe,
            "pre_execute_raw_material_excluded": raw_material_excluded,
            "command_template_private_only": true,
        },
        "validation": {
            "package_ready_for_review": package_ready_for_review,
            "private_runbook_ready": private_runbook_ready,
            "shared_validation_receipt_safe": shared_receipt_safe,
            "pre_execute_raw_material_excluded": raw_material_excluded,
        },
        "operator_controls": {
            "manual_review_required": true,
            "callback_allowed": false,
            "task_creation_allowed": false,
            "task_status_mutation_allowed": false,
            "cleanup_auto_allowed": false,
            "public_docs_change_allowed": false,
        },
        "redaction": {
            "raw_base_url_excluded": true,
            "raw_session_excluded": true,
            "raw_dataset_id_excluded": true,
            "raw_asset_library_id_excluded": true,
            "raw_approval_id_excluded": true,
            "raw_object_locator_excluded": true,
            "raw_command_excluded_from_shared_receipt": true,
            "local_path_excluded": true,
        },
    })
}

pub(crate) fn fashion_design_asset_import_receipt_package_display_contract_drift_guard_dry_run(
    receipt_package: &Value,
) -> Value {
    let package_contract_ok = receipt_package["contract"]
        == json!(
            "fashion_design_asset_import_live_execute_private_runbook_receipt_package_dry_run_v1"
        );
    let package_ready_for_review = receipt_package["validation"]["package_ready_for_review"]
        .as_bool()
        .unwrap_or(false);
    let private_runbook_separate = receipt_package["separation"]
        ["private_runbook_separate_from_shared_receipt"]
        .as_bool()
        .unwrap_or(false);
    let shared_receipt_safe = receipt_package["validation"]["shared_validation_receipt_safe"]
        .as_bool()
        .unwrap_or(false);
    let private_runbook_hidden_from_shared_receipt = !receipt_package["shared_validation_receipt"]
        ["contains_private_runbook"]
        .as_bool()
        .unwrap_or(true)
        && !receipt_package["private_operator_runbook"]["included_in_shared_validation_receipt"]
            .as_bool()
            .unwrap_or(true);
    let command_template_excluded_from_shared_receipt = !receipt_package
        ["shared_validation_receipt"]["contains_command_template"]
        .as_bool()
        .unwrap_or(true);
    let no_side_effects = receipt_package["no_write"].as_bool().unwrap_or(false)
        && receipt_package["no_delete"].as_bool().unwrap_or(false)
        && receipt_package["no_callback"].as_bool().unwrap_or(false)
        && receipt_package["no_task_creation"]
            .as_bool()
            .unwrap_or(false)
        && !receipt_package["public_contract_changed"]
            .as_bool()
            .unwrap_or(true)
        && !receipt_package["production_write_allowed"]
            .as_bool()
            .unwrap_or(true);
    let display_safe = package_contract_ok
        && package_ready_for_review
        && private_runbook_separate
        && shared_receipt_safe
        && private_runbook_hidden_from_shared_receipt
        && command_template_excluded_from_shared_receipt
        && no_side_effects;
    let display_status = if !package_contract_ok {
        "blocked_package_contract_mismatch"
    } else if !package_ready_for_review {
        "blocked_package_not_ready"
    } else if !private_runbook_separate || !private_runbook_hidden_from_shared_receipt {
        "blocked_private_runbook_shared_receipt_leak"
    } else if !command_template_excluded_from_shared_receipt {
        "blocked_command_template_shared_receipt_leak"
    } else if !shared_receipt_safe {
        "blocked_shared_receipt_not_safe"
    } else if !no_side_effects {
        "blocked_side_effect_guard"
    } else {
        "ready_for_detail_display"
    };
    json!({
        "contract": "fashion_design_asset_import_receipt_package_display_contract_drift_guard_dry_run_v1",
        "mode": "dry_run",
        "surface": "task_card_detail_and_validation_receipt_only",
        "no_write": true,
        "no_delete": true,
        "no_callback": true,
        "no_task_creation": true,
        "public_contract_changed": false,
        "production_write_allowed": false,
        "display_status": display_status,
        "package_contract_ok": package_contract_ok,
        "package_ready_for_review": package_ready_for_review,
        "private_runbook_separate_from_shared_receipt": private_runbook_separate,
        "shared_validation_receipt_safe": shared_receipt_safe,
        "private_runbook_hidden_from_shared_receipt": private_runbook_hidden_from_shared_receipt,
        "command_template_excluded_from_shared_receipt": command_template_excluded_from_shared_receipt,
        "no_side_effects": no_side_effects,
        "allowed_detail_fields": [
            "package_status",
            "private_runbook_ready",
            "shared_validation_receipt_safe",
            "pre_execute_raw_material_excluded",
            "manual_review_required"
        ],
        "forbidden_display_behaviors": {
            "creates_task_card": false,
            "mutates_task_card_status_enum": false,
            "emits_external_sse": false,
            "triggers_callback": false,
            "changes_public_docs": false,
            "includes_private_runbook_in_shared_receipt": false,
            "includes_command_template_in_shared_receipt": false,
        },
        "validation": {
            "safe_for_task_card_detail": display_safe,
            "safe_for_validation_ledger": display_safe,
            "safe_for_public_contract_guard": display_safe,
        },
        "operator_controls": {
            "detail_display_only": true,
            "callback_allowed": false,
            "task_creation_allowed": false,
            "task_status_mutation_allowed": false,
            "public_docs_change_allowed": false,
            "cleanup_auto_allowed": false,
        },
        "redaction": {
            "raw_base_url_excluded": true,
            "raw_session_excluded": true,
            "raw_dataset_id_excluded": true,
            "raw_asset_library_id_excluded": true,
            "raw_approval_id_excluded": true,
            "raw_object_locator_excluded": true,
            "raw_command_excluded_from_shared_receipt": true,
            "local_path_excluded": true,
        },
    })
}

pub(crate) fn fashion_design_asset_import_operator_validation_rollup_dry_run(
    readiness_contract_guard: &Value,
    receipt_package: &Value,
    display_guard: &Value,
) -> Value {
    let readiness_guard_contract_ok = readiness_contract_guard["contract"]
        == json!("fashion_design_asset_import_operator_readiness_contract_drift_guard_dry_run_v1");
    let receipt_package_contract_ok = receipt_package["contract"]
        == json!(
            "fashion_design_asset_import_live_execute_private_runbook_receipt_package_dry_run_v1"
        );
    let display_guard_contract_ok = display_guard["contract"]
        == json!(
            "fashion_design_asset_import_receipt_package_display_contract_drift_guard_dry_run_v1"
        );
    let readiness_guard_ready = readiness_guard_contract_ok
        && readiness_contract_guard["validation"]["safe_for_shared_validation_receipt"]
            .as_bool()
            .unwrap_or(false)
        && readiness_contract_guard["operator_controls"]["private_runbook_only"]
            .as_bool()
            .unwrap_or(false)
        && readiness_contract_guard["command_template_redacted"]
            .as_bool()
            .unwrap_or(false)
        && readiness_contract_guard["public_docs_asset_imports_closed"]
            .as_bool()
            .unwrap_or(false);
    let receipt_package_ready = receipt_package_contract_ok
        && receipt_package["validation"]["package_ready_for_review"]
            .as_bool()
            .unwrap_or(false)
        && receipt_package["validation"]["shared_validation_receipt_safe"]
            .as_bool()
            .unwrap_or(false)
        && receipt_package["separation"]["private_runbook_separate_from_shared_receipt"]
            .as_bool()
            .unwrap_or(false)
        && receipt_package["separation"]["pre_execute_raw_material_excluded"]
            .as_bool()
            .unwrap_or(false);
    let display_guard_ready = display_guard_contract_ok
        && display_guard["validation"]["safe_for_task_card_detail"]
            .as_bool()
            .unwrap_or(false)
        && display_guard["validation"]["safe_for_validation_ledger"]
            .as_bool()
            .unwrap_or(false)
        && display_guard["operator_controls"]["detail_display_only"]
            .as_bool()
            .unwrap_or(false)
        && !display_guard["forbidden_display_behaviors"]["creates_task_card"]
            .as_bool()
            .unwrap_or(true)
        && !display_guard["forbidden_display_behaviors"]["mutates_task_card_status_enum"]
            .as_bool()
            .unwrap_or(true)
        && !display_guard["forbidden_display_behaviors"]
            ["includes_private_runbook_in_shared_receipt"]
            .as_bool()
            .unwrap_or(true)
        && !display_guard["forbidden_display_behaviors"]
            ["includes_command_template_in_shared_receipt"]
            .as_bool()
            .unwrap_or(true);
    let side_effect_guards_ok = readiness_contract_guard["no_write"]
        .as_bool()
        .unwrap_or(false)
        && readiness_contract_guard["no_delete"]
            .as_bool()
            .unwrap_or(false)
        && readiness_contract_guard["no_callback"]
            .as_bool()
            .unwrap_or(false)
        && readiness_contract_guard["no_task_creation"]
            .as_bool()
            .unwrap_or(false)
        && !readiness_contract_guard["public_contract_changed"]
            .as_bool()
            .unwrap_or(true)
        && !readiness_contract_guard["production_write_allowed"]
            .as_bool()
            .unwrap_or(true)
        && receipt_package["no_write"].as_bool().unwrap_or(false)
        && receipt_package["no_delete"].as_bool().unwrap_or(false)
        && receipt_package["no_callback"].as_bool().unwrap_or(false)
        && receipt_package["no_task_creation"]
            .as_bool()
            .unwrap_or(false)
        && !receipt_package["public_contract_changed"]
            .as_bool()
            .unwrap_or(true)
        && !receipt_package["production_write_allowed"]
            .as_bool()
            .unwrap_or(true)
        && display_guard["no_write"].as_bool().unwrap_or(false)
        && display_guard["no_delete"].as_bool().unwrap_or(false)
        && display_guard["no_callback"].as_bool().unwrap_or(false)
        && display_guard["no_task_creation"].as_bool().unwrap_or(false)
        && !display_guard["public_contract_changed"]
            .as_bool()
            .unwrap_or(true)
        && !display_guard["production_write_allowed"]
            .as_bool()
            .unwrap_or(true);
    let validation_rollup_ready = readiness_guard_ready
        && receipt_package_ready
        && display_guard_ready
        && side_effect_guards_ok;
    let release_blocker_count = [
        readiness_guard_ready,
        receipt_package_ready,
        display_guard_ready,
        side_effect_guards_ok,
    ]
    .iter()
    .filter(|ready| !**ready)
    .count();
    let rollup_status = if validation_rollup_ready {
        "ready_for_operator_validation_review"
    } else if !readiness_guard_ready {
        "blocked_readiness_contract_guard"
    } else if !receipt_package_ready {
        "blocked_private_runbook_receipt_package"
    } else if !display_guard_ready {
        "blocked_display_contract_guard"
    } else {
        "blocked_side_effect_guard"
    };
    json!({
        "contract": "fashion_design_asset_import_operator_validation_rollup_dry_run_v1",
        "mode": "dry_run",
        "surface": "operator_validation_release_gate_summary",
        "no_write": true,
        "no_delete": true,
        "no_callback": true,
        "no_task_creation": true,
        "public_contract_changed": false,
        "production_write_allowed": false,
        "rollup_status": rollup_status,
        "validation_rollup_ready": validation_rollup_ready,
        "release_blocker_count": release_blocker_count,
        "inputs": {
            "readiness_guard_contract_ok": readiness_guard_contract_ok,
            "receipt_package_contract_ok": receipt_package_contract_ok,
            "display_guard_contract_ok": display_guard_contract_ok,
            "readiness_guard_ready": readiness_guard_ready,
            "receipt_package_ready": receipt_package_ready,
            "display_guard_ready": display_guard_ready,
            "side_effect_guards_ok": side_effect_guards_ok,
        },
        "release_gate_summary": {
            "single_operator_gate_ready": validation_rollup_ready,
            "ready_for_live_execute": false,
            "live_execute_still_requires_private_inputs": true,
            "operator_credentials_required": true,
            "reviewed_dataset_required": true,
            "reviewed_asset_library_required": true,
            "reviewed_approval_required": true,
            "ack_live_write_required": true,
            "safe_for_shared_validation_receipt": validation_rollup_ready,
        },
        "forbidden_release_material": {
            "base_url_included": false,
            "session_material_included": false,
            "dataset_value_included": false,
            "asset_library_value_included": false,
            "approval_value_included": false,
            "object_locator_value_included": false,
            "private_runbook_in_shared_receipt": false,
            "command_template_in_shared_receipt": false,
        },
        "operator_controls": {
            "manual_review_required": true,
            "detail_display_only": true,
            "callback_allowed": false,
            "task_creation_allowed": false,
            "task_status_mutation_allowed": false,
            "public_docs_change_allowed": false,
            "cleanup_auto_allowed": false,
        },
        "redaction": {
            "raw_base_url_excluded": true,
            "raw_session_excluded": true,
            "raw_dataset_id_excluded": true,
            "raw_asset_library_id_excluded": true,
            "raw_approval_id_excluded": true,
            "raw_object_locator_excluded": true,
            "raw_command_excluded_from_shared_receipt": true,
            "local_path_excluded": true,
        },
    })
}

pub(crate) fn fashion_design_asset_import_operator_validation_rollup_display_contract_guard_dry_run(
    validation_rollup: &Value,
) -> Value {
    let rollup_contract_ok = validation_rollup["contract"]
        == json!("fashion_design_asset_import_operator_validation_rollup_dry_run_v1");
    let rollup_ready = validation_rollup["validation_rollup_ready"]
        .as_bool()
        .unwrap_or(false);
    let not_live_execute_ready = !validation_rollup["release_gate_summary"]
        ["ready_for_live_execute"]
        .as_bool()
        .unwrap_or(true);
    let live_private_inputs_required = validation_rollup["release_gate_summary"]
        ["live_execute_still_requires_private_inputs"]
        .as_bool()
        .unwrap_or(false);
    let shared_receipt_safe = validation_rollup["release_gate_summary"]
        ["safe_for_shared_validation_receipt"]
        .as_bool()
        .unwrap_or(false);
    let no_private_material = !validation_rollup["forbidden_release_material"]["base_url_included"]
        .as_bool()
        .unwrap_or(true)
        && !validation_rollup["forbidden_release_material"]["session_material_included"]
            .as_bool()
            .unwrap_or(true)
        && !validation_rollup["forbidden_release_material"]["dataset_value_included"]
            .as_bool()
            .unwrap_or(true)
        && !validation_rollup["forbidden_release_material"]["asset_library_value_included"]
            .as_bool()
            .unwrap_or(true)
        && !validation_rollup["forbidden_release_material"]["approval_value_included"]
            .as_bool()
            .unwrap_or(true)
        && !validation_rollup["forbidden_release_material"]["object_locator_value_included"]
            .as_bool()
            .unwrap_or(true)
        && !validation_rollup["forbidden_release_material"]["private_runbook_in_shared_receipt"]
            .as_bool()
            .unwrap_or(true)
        && !validation_rollup["forbidden_release_material"]["command_template_in_shared_receipt"]
            .as_bool()
            .unwrap_or(true);
    let no_side_effects = validation_rollup["no_write"].as_bool().unwrap_or(false)
        && validation_rollup["no_delete"].as_bool().unwrap_or(false)
        && validation_rollup["no_callback"].as_bool().unwrap_or(false)
        && validation_rollup["no_task_creation"]
            .as_bool()
            .unwrap_or(false)
        && !validation_rollup["public_contract_changed"]
            .as_bool()
            .unwrap_or(true)
        && !validation_rollup["production_write_allowed"]
            .as_bool()
            .unwrap_or(true)
        && !validation_rollup["operator_controls"]["callback_allowed"]
            .as_bool()
            .unwrap_or(true)
        && !validation_rollup["operator_controls"]["task_creation_allowed"]
            .as_bool()
            .unwrap_or(true)
        && !validation_rollup["operator_controls"]["task_status_mutation_allowed"]
            .as_bool()
            .unwrap_or(true)
        && !validation_rollup["operator_controls"]["public_docs_change_allowed"]
            .as_bool()
            .unwrap_or(true);
    let display_safe = rollup_contract_ok
        && rollup_ready
        && not_live_execute_ready
        && live_private_inputs_required
        && shared_receipt_safe
        && no_private_material
        && no_side_effects;
    let display_status = if !rollup_contract_ok {
        "blocked_validation_rollup_contract_mismatch"
    } else if !rollup_ready {
        "blocked_validation_rollup_not_ready"
    } else if !not_live_execute_ready || !live_private_inputs_required {
        "blocked_live_execute_gate_confusion"
    } else if !no_private_material {
        "blocked_private_material_leak"
    } else if !no_side_effects {
        "blocked_side_effect_guard"
    } else {
        "ready_for_validation_gate_display"
    };
    json!({
        "contract": "fashion_design_asset_import_operator_validation_rollup_display_contract_guard_dry_run_v1",
        "mode": "dry_run",
        "surface": "validation_rollup_detail_and_markdown_only",
        "no_write": true,
        "no_delete": true,
        "no_callback": true,
        "no_task_creation": true,
        "public_contract_changed": false,
        "production_write_allowed": false,
        "display_status": display_status,
        "rollup_contract_ok": rollup_contract_ok,
        "rollup_ready": rollup_ready,
        "not_live_execute_ready": not_live_execute_ready,
        "live_private_inputs_required": live_private_inputs_required,
        "shared_receipt_safe": shared_receipt_safe,
        "no_private_material": no_private_material,
        "no_side_effects": no_side_effects,
        "allowed_display_fields": [
            "rollup_status",
            "release_blocker_count",
            "validation_rollup_ready",
            "ready_for_live_execute",
            "live_execute_still_requires_private_inputs"
        ],
        "forbidden_display_behaviors": {
            "auto_triggers_live_execute": false,
            "creates_task_card": false,
            "updates_task_card": false,
            "mutates_task_card_status_enum": false,
            "emits_external_sse": false,
            "triggers_callback": false,
            "includes_private_runbook": false,
            "includes_command_template": false,
            "includes_raw_input_requirements": false,
        },
        "validation": {
            "safe_for_task_card_detail": display_safe,
            "safe_for_validation_ledger": display_safe,
            "safe_for_markdown_summary": display_safe,
        },
        "operator_controls": {
            "detail_display_only": true,
            "manual_review_required": true,
            "callback_allowed": false,
            "task_creation_allowed": false,
            "task_status_mutation_allowed": false,
            "public_docs_change_allowed": false,
            "cleanup_auto_allowed": false,
        },
        "redaction": {
            "raw_base_url_excluded": true,
            "raw_session_excluded": true,
            "raw_dataset_id_excluded": true,
            "raw_asset_library_id_excluded": true,
            "raw_approval_id_excluded": true,
            "raw_object_locator_excluded": true,
            "raw_command_excluded_from_shared_receipt": true,
            "local_path_excluded": true,
        },
    })
}

pub(crate) fn fashion_design_asset_import_operator_validation_release_summary_dry_run(
    execute_manifest: &Value,
    validation_rollup_display_guard: &Value,
) -> Value {
    let execute_manifest_contract_ok = execute_manifest["contract"]
        == json!("fashion_design_asset_import_live_execute_operator_manifest_dry_run_v1");
    let display_guard_contract_ok = validation_rollup_display_guard["contract"]
        == json!(
            "fashion_design_asset_import_operator_validation_rollup_display_contract_guard_dry_run_v1"
        );
    let required_inputs = &execute_manifest["required_inputs"];
    let required_private_inputs = [
        (
            "v3_user_session",
            required_inputs["v3_user_session_required"]
                .as_bool()
                .unwrap_or(false),
            required_inputs["v3_user_session_supplied"]
                .as_bool()
                .unwrap_or(false),
        ),
        (
            "existing_dataset",
            required_inputs["existing_dataset_required"]
                .as_bool()
                .unwrap_or(false),
            required_inputs["existing_dataset_supplied"]
                .as_bool()
                .unwrap_or(false),
        ),
        (
            "existing_asset_library",
            required_inputs["existing_asset_library_required"]
                .as_bool()
                .unwrap_or(false),
            required_inputs["existing_asset_library_supplied"]
                .as_bool()
                .unwrap_or(false),
        ),
        (
            "ack_live_write",
            required_inputs["ack_live_write_required"]
                .as_bool()
                .unwrap_or(false),
            required_inputs["ack_live_write_supplied"]
                .as_bool()
                .unwrap_or(false),
        ),
        (
            "approval_id",
            required_inputs["approval_id_required"]
                .as_bool()
                .unwrap_or(false),
            required_inputs["approval_id_supplied"]
                .as_bool()
                .unwrap_or(false),
        ),
    ];
    let missing_private_inputs: Vec<&str> = required_private_inputs
        .iter()
        .filter_map(|(name, required, supplied)| {
            if *required && !*supplied {
                Some(*name)
            } else {
                None
            }
        })
        .collect();
    let required_private_inputs_declared = required_private_inputs
        .iter()
        .all(|(_, required, _)| *required);
    let private_inputs_only_missing = required_private_inputs_declared;
    let manifest_no_side_effects = execute_manifest["mode"] == json!("dry_run")
        && execute_manifest["no_write"].as_bool().unwrap_or(false)
        && !execute_manifest["production_write_allowed"]
            .as_bool()
            .unwrap_or(true)
        && execute_manifest["review_gates"]["operator_review_required"]
            .as_bool()
            .unwrap_or(false)
        && execute_manifest["review_gates"]["manifest_must_be_reviewed_before_execute"]
            .as_bool()
            .unwrap_or(false)
        && execute_manifest["review_gates"]["execute_requires_ack_live_write"]
            .as_bool()
            .unwrap_or(false)
        && execute_manifest["audit_requirements"]["record_counts_only"]
            .as_bool()
            .unwrap_or(false)
        && !execute_manifest["audit_requirements"]["record_auth_material"]
            .as_bool()
            .unwrap_or(true)
        && !execute_manifest["audit_requirements"]["record_object_locators"]
            .as_bool()
            .unwrap_or(true)
        && !execute_manifest["audit_requirements"]["record_source_urls"]
            .as_bool()
            .unwrap_or(true)
        && !execute_manifest["audit_requirements"]["record_local_paths"]
            .as_bool()
            .unwrap_or(true);
    let display_guard_ready = validation_rollup_display_guard["display_status"]
        == json!("ready_for_validation_gate_display")
        && validation_rollup_display_guard["validation"]["safe_for_task_card_detail"]
            .as_bool()
            .unwrap_or(false)
        && validation_rollup_display_guard["validation"]["safe_for_validation_ledger"]
            .as_bool()
            .unwrap_or(false)
        && validation_rollup_display_guard["validation"]["safe_for_markdown_summary"]
            .as_bool()
            .unwrap_or(false)
        && validation_rollup_display_guard["not_live_execute_ready"]
            .as_bool()
            .unwrap_or(false)
        && validation_rollup_display_guard["live_private_inputs_required"]
            .as_bool()
            .unwrap_or(false);
    let public_contract_stable = !execute_manifest["public_contract_changed"]
        .as_bool()
        .unwrap_or(false)
        && !validation_rollup_display_guard["public_contract_changed"]
            .as_bool()
            .unwrap_or(true)
        && !validation_rollup_display_guard["production_write_allowed"]
            .as_bool()
            .unwrap_or(true);
    let task_card_contract_stable = !validation_rollup_display_guard["forbidden_display_behaviors"]
        ["creates_task_card"]
        .as_bool()
        .unwrap_or(true)
        && !validation_rollup_display_guard["forbidden_display_behaviors"]["updates_task_card"]
            .as_bool()
            .unwrap_or(true)
        && !validation_rollup_display_guard["forbidden_display_behaviors"]
            ["mutates_task_card_status_enum"]
            .as_bool()
            .unwrap_or(true)
        && !validation_rollup_display_guard["operator_controls"]["task_creation_allowed"]
            .as_bool()
            .unwrap_or(true)
        && !validation_rollup_display_guard["operator_controls"]["task_status_mutation_allowed"]
            .as_bool()
            .unwrap_or(true);
    let shared_receipt_stable = validation_rollup_display_guard["no_private_material"]
        .as_bool()
        .unwrap_or(false)
        && !validation_rollup_display_guard["forbidden_display_behaviors"]
            ["includes_private_runbook"]
            .as_bool()
            .unwrap_or(true)
        && !validation_rollup_display_guard["forbidden_display_behaviors"]
            ["includes_command_template"]
            .as_bool()
            .unwrap_or(true)
        && !validation_rollup_display_guard["forbidden_display_behaviors"]
            ["includes_raw_input_requirements"]
            .as_bool()
            .unwrap_or(true);
    let release_ready_for_operator_private_input_injection = execute_manifest_contract_ok
        && display_guard_contract_ok
        && manifest_no_side_effects
        && display_guard_ready
        && private_inputs_only_missing
        && public_contract_stable
        && task_card_contract_stable
        && shared_receipt_stable;
    let release_status = if !execute_manifest_contract_ok || !display_guard_contract_ok {
        "blocked_release_summary_contract_mismatch"
    } else if !display_guard_ready {
        "blocked_validation_display_guard"
    } else if !manifest_no_side_effects {
        "blocked_manifest_side_effect_guard"
    } else if !public_contract_stable {
        "blocked_public_contract_drift"
    } else if !task_card_contract_stable {
        "blocked_task_card_contract_drift"
    } else if !shared_receipt_stable {
        "blocked_shared_receipt_contract_drift"
    } else if !private_inputs_only_missing {
        "blocked_private_input_manifest_shape"
    } else {
        "ready_for_private_operator_input_review"
    };
    json!({
        "contract": "fashion_design_asset_import_operator_validation_release_summary_dry_run_v1",
        "mode": "dry_run",
        "surface": "operator_execute_preflight_release_summary",
        "no_write": true,
        "no_delete": true,
        "no_callback": true,
        "no_task_creation": true,
        "public_contract_changed": false,
        "production_write_allowed": false,
        "release_status": release_status,
        "execute_manifest_contract_ok": execute_manifest_contract_ok,
        "display_guard_contract_ok": display_guard_contract_ok,
        "manifest_no_side_effects": manifest_no_side_effects,
        "display_guard_ready": display_guard_ready,
        "private_inputs_only_missing": private_inputs_only_missing,
        "missing_private_inputs": missing_private_inputs,
        "release_ready_for_operator_private_input_injection":
            release_ready_for_operator_private_input_injection,
        "ready_for_live_execute": false,
        "live_execute_still_requires_private_values": true,
        "private_input_placeholders": {
            "operator_session_required": required_inputs["v3_user_session_required"].as_bool().unwrap_or(false),
            "operator_session_supplied": required_inputs["v3_user_session_supplied"].as_bool().unwrap_or(false),
            "dataset_required": required_inputs["existing_dataset_required"].as_bool().unwrap_or(false),
            "dataset_supplied": required_inputs["existing_dataset_supplied"].as_bool().unwrap_or(false),
            "asset_library_required": required_inputs["existing_asset_library_required"].as_bool().unwrap_or(false),
            "asset_library_supplied": required_inputs["existing_asset_library_supplied"].as_bool().unwrap_or(false),
            "ack_live_write_required": required_inputs["ack_live_write_required"].as_bool().unwrap_or(false),
            "ack_live_write_supplied": required_inputs["ack_live_write_supplied"].as_bool().unwrap_or(false),
            "approval_required": required_inputs["approval_id_required"].as_bool().unwrap_or(false),
            "approval_supplied": required_inputs["approval_id_supplied"].as_bool().unwrap_or(false),
        },
        "unchanged_contracts": {
            "public_third_party_contract": public_contract_stable,
            "task_card_status_enum": task_card_contract_stable,
            "shared_validation_receipt": shared_receipt_stable,
            "asset_imports_public_contract": public_contract_stable,
        },
        "forbidden_release_behaviors": {
            "auto_execute": false,
            "public_contract_change": false,
            "task_card_create": false,
            "task_card_update": false,
            "task_card_status_enum_mutation": false,
            "shared_receipt_private_material": false,
            "callback_dispatch": false,
            "external_sse": false,
        },
        "validation": {
            "ready_for_release_summary_review": release_ready_for_operator_private_input_injection,
            "safe_for_validation_ledger": release_ready_for_operator_private_input_injection,
            "safe_for_markdown_summary": release_ready_for_operator_private_input_injection,
            "execute_manifest_private_inputs_declared": required_private_inputs_declared,
        },
        "operator_controls": {
            "manual_private_input_injection_required": true,
            "detail_display_only": true,
            "live_execute_allowed": false,
            "callback_allowed": false,
            "task_creation_allowed": false,
            "task_status_mutation_allowed": false,
            "public_docs_change_allowed": false,
        },
        "redaction": {
            "raw_base_url_excluded": true,
            "raw_session_excluded": true,
            "raw_dataset_id_excluded": true,
            "raw_asset_library_id_excluded": true,
            "raw_approval_id_excluded": true,
            "raw_object_locator_excluded": true,
            "raw_command_excluded_from_shared_receipt": true,
            "local_path_excluded": true,
        },
    })
}

pub(crate) fn fashion_design_asset_import_operator_validation_release_summary_field_ledger_guard_dry_run(
    release_summary: &Value,
) -> Value {
    let release_summary_contract_ok = release_summary["contract"]
        == json!("fashion_design_asset_import_operator_validation_release_summary_dry_run_v1");
    let release_summary_ready = release_summary["release_status"]
        == json!("ready_for_private_operator_input_review")
        && release_summary["release_ready_for_operator_private_input_injection"]
            .as_bool()
            .unwrap_or(false)
        && release_summary["validation"]["ready_for_release_summary_review"]
            .as_bool()
            .unwrap_or(false)
        && release_summary["validation"]["safe_for_validation_ledger"]
            .as_bool()
            .unwrap_or(false)
        && release_summary["validation"]["safe_for_markdown_summary"]
            .as_bool()
            .unwrap_or(false);
    let detail_display_only = release_summary["operator_controls"]["detail_display_only"]
        .as_bool()
        .unwrap_or(false);
    let no_live_execute = !release_summary["ready_for_live_execute"]
        .as_bool()
        .unwrap_or(true)
        && release_summary["live_execute_still_requires_private_values"]
            .as_bool()
            .unwrap_or(false)
        && !release_summary["operator_controls"]["live_execute_allowed"]
            .as_bool()
            .unwrap_or(true);
    let public_contract_stable = release_summary["unchanged_contracts"]
        ["public_third_party_contract"]
        .as_bool()
        .unwrap_or(false)
        && release_summary["unchanged_contracts"]["asset_imports_public_contract"]
            .as_bool()
            .unwrap_or(false)
        && !release_summary["forbidden_release_behaviors"]["public_contract_change"]
            .as_bool()
            .unwrap_or(true)
        && !release_summary["public_contract_changed"]
            .as_bool()
            .unwrap_or(true);
    let task_card_contract_stable = release_summary["unchanged_contracts"]["task_card_status_enum"]
        .as_bool()
        .unwrap_or(false)
        && !release_summary["forbidden_release_behaviors"]["task_card_create"]
            .as_bool()
            .unwrap_or(true)
        && !release_summary["forbidden_release_behaviors"]["task_card_update"]
            .as_bool()
            .unwrap_or(true)
        && !release_summary["forbidden_release_behaviors"]["task_card_status_enum_mutation"]
            .as_bool()
            .unwrap_or(true)
        && !release_summary["operator_controls"]["task_creation_allowed"]
            .as_bool()
            .unwrap_or(true)
        && !release_summary["operator_controls"]["task_status_mutation_allowed"]
            .as_bool()
            .unwrap_or(true);
    let shared_validation_receipt_stable = release_summary["unchanged_contracts"]
        ["shared_validation_receipt"]
        .as_bool()
        .unwrap_or(false)
        && !release_summary["forbidden_release_behaviors"]["shared_receipt_private_material"]
            .as_bool()
            .unwrap_or(true);
    let no_side_effects = release_summary["no_write"].as_bool().unwrap_or(false)
        && release_summary["no_delete"].as_bool().unwrap_or(false)
        && release_summary["no_callback"].as_bool().unwrap_or(false)
        && release_summary["no_task_creation"]
            .as_bool()
            .unwrap_or(false)
        && !release_summary["production_write_allowed"]
            .as_bool()
            .unwrap_or(true)
        && !release_summary["forbidden_release_behaviors"]["auto_execute"]
            .as_bool()
            .unwrap_or(true)
        && !release_summary["forbidden_release_behaviors"]["callback_dispatch"]
            .as_bool()
            .unwrap_or(true)
        && !release_summary["forbidden_release_behaviors"]["external_sse"]
            .as_bool()
            .unwrap_or(true)
        && !release_summary["operator_controls"]["callback_allowed"]
            .as_bool()
            .unwrap_or(true)
        && !release_summary["operator_controls"]["public_docs_change_allowed"]
            .as_bool()
            .unwrap_or(true);
    let safe_for_field_ledger = release_summary_contract_ok
        && release_summary_ready
        && detail_display_only
        && no_live_execute
        && public_contract_stable
        && task_card_contract_stable
        && shared_validation_receipt_stable
        && no_side_effects;
    let ledger_status = if !release_summary_contract_ok {
        "blocked_release_summary_contract_mismatch"
    } else if !release_summary_ready {
        "blocked_release_summary_not_ready"
    } else if !detail_display_only || !no_live_execute || !no_side_effects {
        "blocked_live_execute_or_side_effect_risk"
    } else if !public_contract_stable {
        "blocked_public_contract_drift"
    } else if !task_card_contract_stable {
        "blocked_task_card_contract_drift"
    } else if !shared_validation_receipt_stable {
        "blocked_shared_validation_receipt_drift"
    } else {
        "ready_for_release_summary_field_ledger"
    };
    json!({
        "contract": "fashion_design_asset_import_operator_validation_release_summary_field_ledger_guard_dry_run_v1",
        "mode": "dry_run",
        "surface": "task_detail_validation_ledger_field_allowlist",
        "no_write": true,
        "no_delete": true,
        "no_callback": true,
        "no_task_creation": true,
        "public_contract_changed": false,
        "production_write_allowed": false,
        "ledger_status": ledger_status,
        "release_summary_contract_ok": release_summary_contract_ok,
        "release_summary_ready": release_summary_ready,
        "detail_display_only": detail_display_only,
        "no_live_execute": no_live_execute,
        "public_contract_stable": public_contract_stable,
        "task_card_contract_stable": task_card_contract_stable,
        "shared_validation_receipt_stable": shared_validation_receipt_stable,
        "no_side_effects": no_side_effects,
        "allowed_task_detail_fields": [
            "release_status",
            "missing_private_inputs",
            "release_ready_for_operator_private_input_injection",
            "ready_for_live_execute",
            "live_execute_still_requires_private_values",
            "unchanged_contracts",
            "forbidden_release_behaviors",
            "redaction"
        ],
        "allowed_validation_ledger_fields": [
            "contract",
            "mode",
            "surface",
            "release_status",
            "release_ready_for_operator_private_input_injection",
            "ready_for_live_execute",
            "live_execute_still_requires_private_values",
            "unchanged_contracts",
            "validation",
            "operator_controls",
            "redaction"
        ],
        "forbidden_field_material": {
            "private_input_values": false,
            "raw_session": false,
            "raw_dataset_id": false,
            "raw_asset_library_id": false,
            "raw_approval_id": false,
            "raw_object_locator": false,
            "command_template": false,
            "callback_payload": false,
            "external_sse_payload": false,
        },
        "forbidden_ledger_behaviors": {
            "auto_execute": false,
            "creates_task_card": false,
            "updates_task_card": false,
            "mutates_task_card_status_enum": false,
            "changes_shared_validation_receipt": false,
            "emits_external_sse": false,
            "triggers_callback": false,
        },
        "validation": {
            "safe_for_task_card_detail": safe_for_field_ledger,
            "safe_for_validation_ledger": safe_for_field_ledger,
            "safe_for_markdown_summary": safe_for_field_ledger,
        },
        "operator_controls": {
            "field_ledger_only": true,
            "operator_review_required": true,
            "live_execute_allowed": false,
            "task_creation_allowed": false,
            "task_status_mutation_allowed": false,
            "callback_allowed": false,
            "public_docs_change_allowed": false,
        },
        "redaction": {
            "raw_base_url_excluded": true,
            "raw_session_excluded": true,
            "raw_dataset_id_excluded": true,
            "raw_asset_library_id_excluded": true,
            "raw_approval_id_excluded": true,
            "raw_object_locator_excluded": true,
            "raw_command_excluded_from_shared_receipt": true,
            "local_path_excluded": true,
        },
    })
}

pub(crate) fn fashion_design_asset_import_operator_ready_handoff_receipt_dry_run(
    execute_manifest: &Value,
    release_summary: &Value,
    field_ledger_guard: &Value,
) -> Value {
    let execute_manifest_contract_ok = execute_manifest["contract"]
        == json!("fashion_design_asset_import_live_execute_operator_manifest_dry_run_v1");
    let release_summary_contract_ok = release_summary["contract"]
        == json!("fashion_design_asset_import_operator_validation_release_summary_dry_run_v1");
    let field_ledger_contract_ok = field_ledger_guard["contract"]
        == json!(
            "fashion_design_asset_import_operator_validation_release_summary_field_ledger_guard_dry_run_v1"
        );
    let required_inputs = &execute_manifest["required_inputs"];
    let private_input_labels = [
        "v3_user_session",
        "existing_dataset",
        "existing_asset_library",
        "ack_live_write",
        "approval_id",
    ];
    let private_inputs_declared = required_inputs["v3_user_session_required"]
        .as_bool()
        .unwrap_or(false)
        && required_inputs["existing_dataset_required"]
            .as_bool()
            .unwrap_or(false)
        && required_inputs["existing_asset_library_required"]
            .as_bool()
            .unwrap_or(false)
        && required_inputs["ack_live_write_required"]
            .as_bool()
            .unwrap_or(false)
        && required_inputs["approval_id_required"]
            .as_bool()
            .unwrap_or(false)
        && release_summary["validation"]["execute_manifest_private_inputs_declared"]
            .as_bool()
            .unwrap_or(false);
    let manifest_review_gate_ready = execute_manifest_contract_ok
        && execute_manifest["mode"] == json!("dry_run")
        && execute_manifest["no_write"].as_bool().unwrap_or(false)
        && !execute_manifest["production_write_allowed"]
            .as_bool()
            .unwrap_or(true)
        && execute_manifest["review_gates"]["operator_review_required"]
            .as_bool()
            .unwrap_or(false)
        && execute_manifest["review_gates"]["manifest_must_be_reviewed_before_execute"]
            .as_bool()
            .unwrap_or(false)
        && execute_manifest["review_gates"]["execute_requires_ack_live_write"]
            .as_bool()
            .unwrap_or(false)
        && execute_manifest["audit_requirements"]["record_counts_only"]
            .as_bool()
            .unwrap_or(false)
        && !execute_manifest["audit_requirements"]["record_auth_material"]
            .as_bool()
            .unwrap_or(true)
        && !execute_manifest["audit_requirements"]["record_object_locators"]
            .as_bool()
            .unwrap_or(true)
        && !execute_manifest["audit_requirements"]["record_source_urls"]
            .as_bool()
            .unwrap_or(true);
    let release_summary_ready = release_summary_contract_ok
        && release_summary["release_status"] == json!("ready_for_private_operator_input_review")
        && release_summary["release_ready_for_operator_private_input_injection"]
            .as_bool()
            .unwrap_or(false)
        && !release_summary["ready_for_live_execute"]
            .as_bool()
            .unwrap_or(true)
        && release_summary["live_execute_still_requires_private_values"]
            .as_bool()
            .unwrap_or(false);
    let field_ledger_ready = field_ledger_contract_ok
        && field_ledger_guard["ledger_status"] == json!("ready_for_release_summary_field_ledger")
        && field_ledger_guard["validation"]["safe_for_task_card_detail"]
            .as_bool()
            .unwrap_or(false)
        && field_ledger_guard["validation"]["safe_for_validation_ledger"]
            .as_bool()
            .unwrap_or(false)
        && field_ledger_guard["operator_controls"]["field_ledger_only"]
            .as_bool()
            .unwrap_or(false)
        && field_ledger_guard["no_live_execute"]
            .as_bool()
            .unwrap_or(false);
    let public_contract_stable = release_summary["unchanged_contracts"]
        ["public_third_party_contract"]
        .as_bool()
        .unwrap_or(false)
        && release_summary["unchanged_contracts"]["asset_imports_public_contract"]
            .as_bool()
            .unwrap_or(false)
        && field_ledger_guard["public_contract_stable"]
            .as_bool()
            .unwrap_or(false)
        && !release_summary["public_contract_changed"]
            .as_bool()
            .unwrap_or(true)
        && !field_ledger_guard["public_contract_changed"]
            .as_bool()
            .unwrap_or(true);
    let task_card_contract_stable = release_summary["unchanged_contracts"]["task_card_status_enum"]
        .as_bool()
        .unwrap_or(false)
        && field_ledger_guard["task_card_contract_stable"]
            .as_bool()
            .unwrap_or(false)
        && !release_summary["forbidden_release_behaviors"]["task_card_create"]
            .as_bool()
            .unwrap_or(true)
        && !release_summary["forbidden_release_behaviors"]["task_card_update"]
            .as_bool()
            .unwrap_or(true)
        && !release_summary["forbidden_release_behaviors"]["task_card_status_enum_mutation"]
            .as_bool()
            .unwrap_or(true);
    let shared_validation_receipt_stable = release_summary["unchanged_contracts"]
        ["shared_validation_receipt"]
        .as_bool()
        .unwrap_or(false)
        && field_ledger_guard["shared_validation_receipt_stable"]
            .as_bool()
            .unwrap_or(false)
        && !field_ledger_guard["forbidden_ledger_behaviors"]["changes_shared_validation_receipt"]
            .as_bool()
            .unwrap_or(true);
    let no_side_effects = release_summary["no_write"].as_bool().unwrap_or(false)
        && release_summary["no_delete"].as_bool().unwrap_or(false)
        && release_summary["no_callback"].as_bool().unwrap_or(false)
        && release_summary["no_task_creation"]
            .as_bool()
            .unwrap_or(false)
        && field_ledger_guard["no_write"].as_bool().unwrap_or(false)
        && field_ledger_guard["no_delete"].as_bool().unwrap_or(false)
        && field_ledger_guard["no_callback"].as_bool().unwrap_or(false)
        && field_ledger_guard["no_task_creation"]
            .as_bool()
            .unwrap_or(false)
        && !release_summary["production_write_allowed"]
            .as_bool()
            .unwrap_or(true)
        && !field_ledger_guard["production_write_allowed"]
            .as_bool()
            .unwrap_or(true)
        && !release_summary["forbidden_release_behaviors"]["auto_execute"]
            .as_bool()
            .unwrap_or(true)
        && !field_ledger_guard["forbidden_ledger_behaviors"]["auto_execute"]
            .as_bool()
            .unwrap_or(true)
        && !release_summary["operator_controls"]["callback_allowed"]
            .as_bool()
            .unwrap_or(true)
        && !field_ledger_guard["operator_controls"]["callback_allowed"]
            .as_bool()
            .unwrap_or(true);
    let handoff_ready_for_operator_private_values = execute_manifest_contract_ok
        && release_summary_contract_ok
        && field_ledger_contract_ok
        && private_inputs_declared
        && manifest_review_gate_ready
        && release_summary_ready
        && field_ledger_ready
        && public_contract_stable
        && task_card_contract_stable
        && shared_validation_receipt_stable
        && no_side_effects;
    let handoff_status = if !execute_manifest_contract_ok
        || !release_summary_contract_ok
        || !field_ledger_contract_ok
    {
        "blocked_operator_handoff_contract_mismatch"
    } else if !private_inputs_declared || !manifest_review_gate_ready {
        "blocked_execute_manifest_review_gate"
    } else if !release_summary_ready {
        "blocked_release_summary_not_ready"
    } else if !field_ledger_ready {
        "blocked_field_ledger_not_ready"
    } else if !public_contract_stable {
        "blocked_public_contract_drift"
    } else if !task_card_contract_stable {
        "blocked_task_card_contract_drift"
    } else if !shared_validation_receipt_stable {
        "blocked_shared_validation_receipt_drift"
    } else if !no_side_effects {
        "blocked_side_effect_guard"
    } else {
        "ready_for_operator_private_values"
    };
    json!({
        "contract": "fashion_design_asset_import_operator_ready_handoff_receipt_dry_run_v1",
        "mode": "dry_run",
        "surface": "operator_ready_private_execute_handoff_receipt",
        "no_write": true,
        "no_delete": true,
        "no_callback": true,
        "no_task_creation": true,
        "public_contract_changed": false,
        "production_write_allowed": false,
        "handoff_status": handoff_status,
        "execute_manifest_contract_ok": execute_manifest_contract_ok,
        "release_summary_contract_ok": release_summary_contract_ok,
        "field_ledger_contract_ok": field_ledger_contract_ok,
        "private_inputs_declared": private_inputs_declared,
        "manifest_review_gate_ready": manifest_review_gate_ready,
        "release_summary_ready": release_summary_ready,
        "field_ledger_ready": field_ledger_ready,
        "public_contract_stable": public_contract_stable,
        "task_card_contract_stable": task_card_contract_stable,
        "shared_validation_receipt_stable": shared_validation_receipt_stable,
        "no_side_effects": no_side_effects,
        "handoff_ready_for_operator_private_values": handoff_ready_for_operator_private_values,
        "ready_for_live_execute": false,
        "live_execute_still_requires_private_values": true,
        "operator_private_input_labels": private_input_labels,
        "operator_next_step": {
            "private_value_injection_required": true,
            "operator_review_required": true,
            "controlled_execute_after_review_only": true,
            "public_contract_change_required": false,
            "task_card_status_enum_change_required": false,
            "shared_validation_receipt_change_required": false,
            "main_site_display_field_change_required": false,
        },
        "unchanged_contracts": {
            "public_third_party_contract": public_contract_stable,
            "task_card_status_enum": task_card_contract_stable,
            "shared_validation_receipt": shared_validation_receipt_stable,
            "main_site_display_fields": field_ledger_ready,
            "asset_imports_public_contract": public_contract_stable,
        },
        "forbidden_handoff_behaviors": {
            "auto_execute": false,
            "creates_task_card": false,
            "updates_task_card": false,
            "mutates_task_card_status_enum": false,
            "changes_shared_validation_receipt": false,
            "changes_public_contract": false,
            "changes_main_site_display_fields": false,
            "emits_external_sse": false,
            "triggers_callback": false,
            "includes_private_input_values": false,
            "includes_command_template": false,
        },
        "validation": {
            "safe_for_operator_handoff_receipt": handoff_ready_for_operator_private_values,
            "safe_for_validation_ledger": handoff_ready_for_operator_private_values,
            "safe_for_markdown_summary": handoff_ready_for_operator_private_values,
        },
        "operator_controls": {
            "private_value_injection_required": true,
            "operator_review_required": true,
            "live_execute_allowed": false,
            "controlled_execute_after_review_only": true,
            "task_creation_allowed": false,
            "task_status_mutation_allowed": false,
            "callback_allowed": false,
            "public_docs_change_allowed": false,
        },
        "redaction": {
            "raw_base_url_excluded": true,
            "raw_session_excluded": true,
            "raw_dataset_id_excluded": true,
            "raw_asset_library_id_excluded": true,
            "raw_approval_id_excluded": true,
            "raw_object_locator_excluded": true,
            "raw_command_excluded_from_shared_receipt": true,
            "local_path_excluded": true,
        },
    })
}

pub(crate) fn fashion_design_asset_import_final_validation_readiness_bundle_dry_run(
    operator_ready_handoff_receipt: &Value,
) -> Value {
    let handoff_contract_ok = operator_ready_handoff_receipt["contract"]
        == json!("fashion_design_asset_import_operator_ready_handoff_receipt_dry_run_v1");
    let handoff_ready = handoff_contract_ok
        && operator_ready_handoff_receipt["mode"] == json!("dry_run")
        && operator_ready_handoff_receipt["surface"]
            == json!("operator_ready_private_execute_handoff_receipt")
        && operator_ready_handoff_receipt["handoff_status"]
            == json!("ready_for_operator_private_values")
        && operator_ready_handoff_receipt["handoff_ready_for_operator_private_values"]
            .as_bool()
            .unwrap_or(false)
        && !operator_ready_handoff_receipt["ready_for_live_execute"]
            .as_bool()
            .unwrap_or(true)
        && operator_ready_handoff_receipt["live_execute_still_requires_private_values"]
            .as_bool()
            .unwrap_or(false);
    let unchanged_contracts = &operator_ready_handoff_receipt["unchanged_contracts"];
    let contracts_stable = unchanged_contracts["public_third_party_contract"]
        .as_bool()
        .unwrap_or(false)
        && unchanged_contracts["task_card_status_enum"]
            .as_bool()
            .unwrap_or(false)
        && unchanged_contracts["shared_validation_receipt"]
            .as_bool()
            .unwrap_or(false)
        && unchanged_contracts["main_site_display_fields"]
            .as_bool()
            .unwrap_or(false)
        && unchanged_contracts["asset_imports_public_contract"]
            .as_bool()
            .unwrap_or(false)
        && !operator_ready_handoff_receipt["public_contract_changed"]
            .as_bool()
            .unwrap_or(true);
    let forbidden = &operator_ready_handoff_receipt["forbidden_handoff_behaviors"];
    let no_task_card_or_shared_receipt_mutation =
        !forbidden["creates_task_card"].as_bool().unwrap_or(true)
            && !forbidden["updates_task_card"].as_bool().unwrap_or(true)
            && !forbidden["mutates_task_card_status_enum"]
                .as_bool()
                .unwrap_or(true)
            && !forbidden["changes_shared_validation_receipt"]
                .as_bool()
                .unwrap_or(true)
            && !forbidden["changes_main_site_display_fields"]
                .as_bool()
                .unwrap_or(true);
    let no_external_effects = operator_ready_handoff_receipt["no_write"]
        .as_bool()
        .unwrap_or(false)
        && operator_ready_handoff_receipt["no_delete"]
            .as_bool()
            .unwrap_or(false)
        && operator_ready_handoff_receipt["no_callback"]
            .as_bool()
            .unwrap_or(false)
        && operator_ready_handoff_receipt["no_task_creation"]
            .as_bool()
            .unwrap_or(false)
        && !operator_ready_handoff_receipt["production_write_allowed"]
            .as_bool()
            .unwrap_or(true)
        && !forbidden["auto_execute"].as_bool().unwrap_or(true)
        && !forbidden["emits_external_sse"].as_bool().unwrap_or(true)
        && !forbidden["triggers_callback"].as_bool().unwrap_or(true)
        && !operator_ready_handoff_receipt["operator_controls"]["live_execute_allowed"]
            .as_bool()
            .unwrap_or(true)
        && !operator_ready_handoff_receipt["operator_controls"]["callback_allowed"]
            .as_bool()
            .unwrap_or(true)
        && !operator_ready_handoff_receipt["operator_controls"]["task_creation_allowed"]
            .as_bool()
            .unwrap_or(true)
        && !operator_ready_handoff_receipt["operator_controls"]["task_status_mutation_allowed"]
            .as_bool()
            .unwrap_or(true);
    let redaction_ready = operator_ready_handoff_receipt["redaction"]["raw_base_url_excluded"]
        .as_bool()
        .unwrap_or(false)
        && operator_ready_handoff_receipt["redaction"]["raw_session_excluded"]
            .as_bool()
            .unwrap_or(false)
        && operator_ready_handoff_receipt["redaction"]["raw_dataset_id_excluded"]
            .as_bool()
            .unwrap_or(false)
        && operator_ready_handoff_receipt["redaction"]["raw_asset_library_id_excluded"]
            .as_bool()
            .unwrap_or(false)
        && operator_ready_handoff_receipt["redaction"]["raw_approval_id_excluded"]
            .as_bool()
            .unwrap_or(false)
        && operator_ready_handoff_receipt["redaction"]["raw_object_locator_excluded"]
            .as_bool()
            .unwrap_or(false)
        && !forbidden["includes_private_input_values"]
            .as_bool()
            .unwrap_or(true)
        && !forbidden["includes_command_template"]
            .as_bool()
            .unwrap_or(true);
    let bundle_ready_for_operator_review = handoff_ready
        && contracts_stable
        && no_task_card_or_shared_receipt_mutation
        && no_external_effects
        && redaction_ready;
    let bundle_status = if !handoff_contract_ok {
        "blocked_handoff_contract_mismatch"
    } else if !handoff_ready {
        "blocked_handoff_not_ready"
    } else if !contracts_stable {
        "blocked_contract_stability"
    } else if !no_task_card_or_shared_receipt_mutation {
        "blocked_mutation_guard"
    } else if !no_external_effects {
        "blocked_side_effect_guard"
    } else if !redaction_ready {
        "blocked_redaction_guard"
    } else {
        "ready_for_operator_review_bundle"
    };
    let release_blockers: Vec<&str> = if bundle_ready_for_operator_review {
        Vec::new()
    } else {
        vec![bundle_status]
    };
    json!({
        "contract": "fashion_design_asset_import_final_validation_readiness_bundle_dry_run_v1",
        "mode": "dry_run",
        "surface": "final_validation_readiness_bundle",
        "no_write": true,
        "no_delete": true,
        "no_callback": true,
        "no_task_creation": true,
        "public_contract_changed": false,
        "production_write_allowed": false,
        "bundle_status": bundle_status,
        "bundle_ready_for_operator_review": bundle_ready_for_operator_review,
        "release_blocker_count": release_blockers.len(),
        "release_blockers": release_blockers,
        "ready_for_live_execute": false,
        "live_execute_still_requires_private_values": true,
        "handoff_status": operator_ready_handoff_receipt["handoff_status"].clone(),
        "operator_private_input_label_count": operator_ready_handoff_receipt["operator_private_input_labels"]
            .as_array()
            .map(|labels| labels.len())
            .unwrap_or(0),
        "included_sections": [
            "execute_manifest_review_gate",
            "operator_validation_release_summary",
            "release_summary_field_ledger",
            "operator_ready_handoff_receipt",
        ],
        "validation_readiness": {
            "handoff_receipt_ready": handoff_ready,
            "private_values_only": handoff_ready,
            "public_contract_stable": unchanged_contracts["public_third_party_contract"].as_bool().unwrap_or(false),
            "task_card_contract_stable": unchanged_contracts["task_card_status_enum"].as_bool().unwrap_or(false),
            "shared_validation_receipt_stable": unchanged_contracts["shared_validation_receipt"].as_bool().unwrap_or(false),
            "main_site_display_fields_stable": unchanged_contracts["main_site_display_fields"].as_bool().unwrap_or(false),
            "asset_imports_public_contract_stable": unchanged_contracts["asset_imports_public_contract"].as_bool().unwrap_or(false),
            "no_task_card_or_shared_receipt_mutation": no_task_card_or_shared_receipt_mutation,
            "no_external_effects": no_external_effects,
            "redaction_ready": redaction_ready,
            "single_gate_ready": bundle_ready_for_operator_review,
        },
        "operator_next_step": {
            "private_value_injection_required": true,
            "operator_review_required": true,
            "final_bundle_review_required": true,
            "controlled_execute_after_review_only": true,
            "public_contract_change_required": false,
            "task_card_status_enum_change_required": false,
            "shared_validation_receipt_change_required": false,
            "main_site_display_field_change_required": false,
        },
        "unchanged_contracts": {
            "public_third_party_contract": unchanged_contracts["public_third_party_contract"].as_bool().unwrap_or(false),
            "task_card_status_enum": unchanged_contracts["task_card_status_enum"].as_bool().unwrap_or(false),
            "shared_validation_receipt": unchanged_contracts["shared_validation_receipt"].as_bool().unwrap_or(false),
            "main_site_display_fields": unchanged_contracts["main_site_display_fields"].as_bool().unwrap_or(false),
            "asset_imports_public_contract": unchanged_contracts["asset_imports_public_contract"].as_bool().unwrap_or(false),
        },
        "forbidden_bundle_behaviors": {
            "auto_execute": false,
            "creates_task_card": false,
            "updates_task_card": false,
            "mutates_task_card_status_enum": false,
            "changes_shared_validation_receipt": false,
            "changes_public_contract": false,
            "changes_main_site_display_fields": false,
            "emits_external_sse": false,
            "triggers_callback": false,
            "includes_private_input_values": false,
            "includes_command_template": false,
        },
        "validation": {
            "safe_for_final_validation_bundle": bundle_ready_for_operator_review,
            "safe_for_validation_ledger": bundle_ready_for_operator_review,
            "safe_for_markdown_summary": bundle_ready_for_operator_review,
        },
        "operator_controls": {
            "final_bundle_only": true,
            "private_value_injection_required": true,
            "operator_review_required": true,
            "live_execute_allowed": false,
            "controlled_execute_after_review_only": true,
            "task_creation_allowed": false,
            "task_status_mutation_allowed": false,
            "callback_allowed": false,
            "public_docs_change_allowed": false,
        },
        "redaction": {
            "raw_base_url_excluded": true,
            "raw_session_excluded": true,
            "raw_dataset_id_excluded": true,
            "raw_asset_library_id_excluded": true,
            "raw_approval_id_excluded": true,
            "raw_object_locator_excluded": true,
            "raw_command_excluded_from_shared_receipt": true,
            "local_path_excluded": true,
        },
    })
}

pub(crate) fn fashion_design_asset_retrieval_evidence_live_writer_readiness_dry_run(
    profile_ready: bool,
    materialized_text_ready: bool,
    dataset_membership_verified: bool,
    asset_evidence_table_available: bool,
    union_search_adapter_available: bool,
) -> Value {
    let writer_ready_for_operator_review = profile_ready
        && materialized_text_ready
        && dataset_membership_verified
        && asset_evidence_table_available
        && union_search_adapter_available;
    let writer_status = if !profile_ready {
        "blocked_profile_not_ready"
    } else if !materialized_text_ready {
        "blocked_materialized_text_missing"
    } else if !dataset_membership_verified {
        "blocked_dataset_membership_guard"
    } else if !asset_evidence_table_available {
        "blocked_asset_evidence_table_missing"
    } else if !union_search_adapter_available {
        "blocked_union_search_adapter_missing"
    } else {
        "ready_for_controlled_writer_review"
    };
    let release_blockers: Vec<&str> = if writer_ready_for_operator_review {
        Vec::new()
    } else {
        vec![writer_status]
    };

    json!({
        "contract": "fashion_design_asset_retrieval_evidence_live_writer_readiness_dry_run_v1",
        "mode": "dry_run",
        "surface": "asset_retrieval_evidence_writer_review",
        "source_kind": "asset_profile",
        "profile_schema": FASHION_DESIGN_IMAGE_PROFILE_KIND,
        "writer_status": writer_status,
        "writer_ready_for_operator_review": writer_ready_for_operator_review,
        "release_blocker_count": release_blockers.len(),
        "release_blockers": release_blockers,
        "no_write": true,
        "no_delete": true,
        "no_callback": true,
        "no_task_creation": true,
        "production_write_allowed": false,
        "schema_migration_required_before_live_write": !asset_evidence_table_available,
        "reviewed_migration_required": true,
        "live_write_still_requires_operator_ack": true,
        "input_readiness": {
            "asset_profile_ready": profile_ready,
            "materialized_retrieval_text_ready": materialized_text_ready,
            "dataset_membership_verified": dataset_membership_verified,
            "asset_evidence_table_available": asset_evidence_table_available,
            "union_search_adapter_available": union_search_adapter_available,
        },
        "write_order": [
            {
                "step": 1,
                "action": "verify_asset_profile_upserted",
                "required": true,
            },
            {
                "step": 2,
                "action": "materialize_retrieval_evidence_text",
                "requires": "asset_profile_upserted",
            },
            {
                "step": 3,
                "action": "verify_dataset_asset_membership",
                "requires": "current_tenant_scope",
            },
            {
                "step": 4,
                "action": "upsert_asset_retrieval_evidence",
                "requires": "materialized_text_and_membership_verified",
                "idempotent": true,
            },
            {
                "step": 5,
                "action": "verify_union_search_source_kind",
                "requires": "asset_evidence_upserted",
            },
        ],
        "field_ledger": {
            "allowed_shared_receipt_fields": [
                "contract",
                "mode",
                "surface",
                "source_kind",
                "profile_schema",
                "writer_status",
                "writer_ready_for_operator_review",
                "release_blocker_count",
                "input_readiness",
                "write_order",
                "idempotency",
                "membership_guard",
                "rollback_requirements",
                "audit_requirements",
                "redaction"
            ],
            "record_counts_only": true,
            "raw_locator_fields_allowed": false,
            "provider_payload_fields_allowed": false,
        },
        "idempotency": {
            "strategy": "upsert_by_dataset_asset_profile_parser_version",
            "key_template_redacted": true,
            "dedupe_scope": [
                "tenant_id",
                "dataset_id",
                "asset_id",
                "profile_kind",
                "parser_name",
                "parser_version"
            ],
            "retry_safe": true,
        },
        "membership_guard": {
            "table": "dataset_asset_memberships",
            "tenant_scope_required": true,
            "dataset_scope_required": true,
            "asset_visibility_required": true,
            "deny_hidden_assets": true,
            "expiry_filter_required": true,
        },
        "search_contract": {
            "method": "union_document_and_asset_evidence_search",
            "returns_source_kind": "asset_profile",
            "document_search_contract_unchanged": true,
            "asset_profile_exact_document_citation_allowed": false,
        },
        "rollback_requirements": {
            "reviewed_manifest_required": true,
            "delete_by_idempotency_key_only": true,
            "automatic_rollback_allowed": false,
            "record_raw_locators": false,
        },
        "audit_requirements": {
            "record_counts_only": true,
            "record_dedupe_scope": true,
            "record_membership_guard_result": true,
            "record_raw_locators": false,
            "record_provider_payload": false,
        },
        "operator_controls": {
            "operator_review_required": true,
            "live_write_allowed": false,
            "controlled_write_after_review_only": true,
            "task_creation_allowed": false,
            "callback_allowed": false,
            "public_docs_change_allowed": false,
        },
        "redaction": {
            "raw_object_locator_excluded": true,
            "provider_payload_excluded": true,
            "source_url_value_included": false,
            "local_path_value_included": false,
            "secret_material_included": false,
        },
        "validation": {
            "safe_for_validation_ledger": true,
            "safe_for_markdown_summary": true,
            "public_third_party_contract_changed": false,
            "task_card_contract_changed": false,
            "shared_receipt_private_material_included": false,
        },
    })
}

pub(crate) fn fashion_design_third_party_asset_import_private_endpoint_guard_dry_run(
    public_docs_asset_imports_open: bool,
    inbound_secret_configured: bool,
    connection_scope_ready: bool,
    source_scope_ready: bool,
    dataset_scope_supplied: bool,
    asset_library_scope_supplied: bool,
    assets_supplied: bool,
) -> Value {
    let public_docs_closed = !public_docs_asset_imports_open;
    let private_endpoint_ready = public_docs_closed
        && inbound_secret_configured
        && connection_scope_ready
        && source_scope_ready
        && dataset_scope_supplied
        && asset_library_scope_supplied
        && assets_supplied;
    let endpoint_status = if public_docs_asset_imports_open {
        "blocked_public_docs_expose_unreviewed_asset_imports"
    } else if !inbound_secret_configured {
        "blocked_inbound_secret_not_configured"
    } else if !connection_scope_ready {
        "blocked_connection_scope"
    } else if !source_scope_ready {
        "blocked_source_scope"
    } else if !dataset_scope_supplied {
        "blocked_dataset_scope_missing"
    } else if !asset_library_scope_supplied {
        "blocked_asset_library_scope_missing"
    } else if !assets_supplied {
        "blocked_assets_missing"
    } else {
        "ready_for_private_endpoint_review"
    };
    let release_blockers: Vec<&str> = if private_endpoint_ready {
        Vec::new()
    } else {
        vec![endpoint_status]
    };

    json!({
        "contract": "fashion_design_third_party_asset_import_private_endpoint_guard_dry_run_v1",
        "mode": "dry_run",
        "surface": "third_party_private_asset_import_endpoint_review",
        "endpoint_shape": {
            "method": "POST",
            "path_template": "/v1/external/channels/{connection_id}/asset-imports",
            "query_required": false,
            "public_docs_exposed": public_docs_asset_imports_open,
        },
        "endpoint_status": endpoint_status,
        "private_endpoint_ready": private_endpoint_ready,
        "release_blocker_count": release_blockers.len(),
        "release_blockers": release_blockers,
        "no_write": true,
        "no_delete": true,
        "no_callback": true,
        "no_external_sse": true,
        "no_task_creation": true,
        "public_contract_changed": false,
        "production_write_allowed": false,
        "public_docs_asset_imports_closed": public_docs_closed,
        "input_readiness": {
            "inbound_secret_configured": inbound_secret_configured,
            "connection_scope_ready": connection_scope_ready,
            "source_scope_ready": source_scope_ready,
            "dataset_scope_supplied": dataset_scope_supplied,
            "asset_library_scope_supplied": asset_library_scope_supplied,
            "assets_supplied": assets_supplied,
        },
        "request_contract": {
            "required_fields": [
                "request_id",
                "asset_library_external_id",
                "dataset_external_ids",
                "assets"
            ],
            "optional_fields": [
                "source_external_id",
                "asset_collection_external_id",
                "packages",
                "metadata"
            ],
            "asset_input_modes": [
                "url",
                "object_ref",
                "attachment_ref"
            ],
            "default_profile_schema": FASHION_DESIGN_IMAGE_PROFILE_KIND,
            "structured_request_only": true,
            "raw_binary_in_json_allowed": false,
        },
        "response_contract": {
            "reply_shape": "structured_json",
            "required_fields": [
                "request_id",
                "task_ref",
                "parse_status",
                "assets"
            ],
            "asset_fields": [
                "asset_external_id",
                "title",
                "content_type",
                "status",
                "profile_schema"
            ],
            "statuses": [
                "accepted",
                "queued",
                "processing",
                "completed",
                "partial_completed",
                "failed"
            ],
            "customer_storage_ready": true,
        },
        "privacy_policy": {
            "third_party_assets_private_by_default": true,
            "main_site_default_visibility": false,
            "tenant_scope_required": true,
            "connection_scope_required": true,
            "duplicate_public_assets_not_created": true,
            "cross_customer_dedupe_allowed_without_visibility_merge": true,
        },
        "dataset_attachment_policy": {
            "dataset_external_ids_supported": true,
            "asset_library_external_id_supported": true,
            "asset_collection_external_id_supported": true,
            "membership_write_requires_private_endpoint_execute": true,
            "chat_scope_still_prefers_dataset_external_ids": true,
        },
        "field_ledger": {
            "allowed_shared_receipt_fields": [
                "contract",
                "mode",
                "surface",
                "endpoint_status",
                "private_endpoint_ready",
                "release_blocker_count",
                "public_docs_asset_imports_closed",
                "request_contract",
                "response_contract",
                "privacy_policy",
                "dataset_attachment_policy",
                "redaction"
            ],
            "record_counts_only": true,
            "raw_asset_values_allowed": false,
            "secret_material_allowed": false,
            "source_url_values_allowed": false,
        },
        "operator_controls": {
            "private_channel_only": true,
            "public_docs_change_required_before_general_release": true,
            "controlled_execute_after_review_only": true,
            "task_creation_allowed": false,
            "callback_allowed": false,
        },
        "redaction": {
            "inbound_secret_value_included": false,
            "connection_id_value_included": false,
            "source_id_value_included": false,
            "dataset_external_id_values_included": false,
            "asset_library_external_id_value_included": false,
            "asset_external_id_values_included": false,
            "source_url_values_included": false,
            "raw_asset_payload_included": false,
        },
        "validation": {
            "public_contract_stable": true,
            "private_visibility_guard_ready": private_endpoint_ready,
            "structured_json_ready": true,
            "dataset_attachment_guard_ready": dataset_scope_supplied && asset_library_scope_supplied,
            "safe_for_shared_validation_receipt": private_endpoint_ready,
        },
    })
}

pub(crate) fn fashion_design_main_site_gallery_task_card_dry_run(
    import_request_ready: bool,
    normalized_response_ready: bool,
    scope_summary_ready: bool,
    parse_status_ready: bool,
    field_ledger_ready: bool,
    stable_task_identity_ready: bool,
) -> Value {
    let task_card_ready = import_request_ready
        && normalized_response_ready
        && scope_summary_ready
        && parse_status_ready
        && field_ledger_ready
        && stable_task_identity_ready;
    let task_status = if !import_request_ready {
        "blocked_import_request_missing"
    } else if !normalized_response_ready {
        "blocked_normalized_response_missing"
    } else if !scope_summary_ready {
        "blocked_scope_summary_missing"
    } else if !parse_status_ready {
        "blocked_parse_status_missing"
    } else if !field_ledger_ready {
        "blocked_field_ledger_missing"
    } else if !stable_task_identity_ready {
        "blocked_unstable_task_identity"
    } else {
        "ready_for_task_card_dry_run"
    };
    let release_blockers: Vec<&str> = if task_card_ready {
        Vec::new()
    } else {
        vec![task_status]
    };

    json!({
        "contract": "fashion_design_main_site_gallery_task_card_dry_run_v1",
        "mode": "dry_run",
        "surface": "main_site_asset_gallery_task_card_detail_review",
        "task_status": task_status,
        "task_card_ready": task_card_ready,
        "release_blocker_count": release_blockers.len(),
        "release_blockers": release_blockers,
        "no_write": true,
        "no_delete": true,
        "no_callback": true,
        "no_external_sse": true,
        "no_chat_message": true,
        "no_task_creation": true,
        "no_artifact_creation": true,
        "public_contract_changed": false,
        "production_write_allowed": false,
        "planned_task_card": {
            "type": "asset_gallery_import_task",
            "stable_identity_source": "asset_library_id_dataset_id_import_batch_id",
            "title_strategy": "asset_library_name_or_import_title",
            "status_sequence": [
                "queued",
                "running",
                "retrying",
                "completed",
                "failed"
            ],
            "active_statuses": [
                "queued",
                "running",
                "retrying"
            ],
            "terminal_statuses": [
                "completed",
                "failed",
                "cancelled"
            ],
            "open_behavior": "open_task_detail_panel",
            "delete_behavior": "dismiss_or_delete_failed_dry_run_only",
        },
        "detail_contract": {
            "sections": [
                "generation_prompt",
                "data_sources",
                "parse_status_summary",
                "field_ledger",
                "validation_receipt"
            ],
            "shows_generation_prompt": true,
            "shows_data_sources": true,
            "shows_parse_status": true,
            "shows_field_ledger": true,
            "shows_validation_receipt": true,
            "raw_locator_values_allowed": false,
            "provider_payload_allowed": false,
        },
        "refresh_policy": {
            "card_persists_after_creation": true,
            "selected_card_refresh_only": true,
            "no_global_polling": true,
            "no_unrelated_artifact_injection": true,
            "does_not_post_to_main_chat": true,
            "local_visibility_ref_required": true,
        },
        "input_readiness": {
            "import_request_ready": import_request_ready,
            "normalized_response_ready": normalized_response_ready,
            "scope_summary_ready": scope_summary_ready,
            "parse_status_ready": parse_status_ready,
            "field_ledger_ready": field_ledger_ready,
            "stable_task_identity_ready": stable_task_identity_ready,
        },
        "field_ledger": {
            "allowed_card_fields": [
                "id",
                "type",
                "title",
                "status",
                "subtitle",
                "sourceLines",
                "editInfo",
                "detail"
            ],
            "allowed_detail_fields": [
                "promptSummary",
                "dataSources",
                "parseStatusSummary",
                "fieldLedger",
                "validationReceipt"
            ],
            "raw_asset_values_allowed": false,
            "source_url_values_allowed": false,
            "secret_material_allowed": false,
            "record_counts_only": true,
        },
        "validation": {
            "safe_for_main_site_task_card": task_card_ready,
            "task_card_contract_stable": stable_task_identity_ready,
            "detail_contract_ready": field_ledger_ready && parse_status_ready,
            "shared_receipt_safe": task_card_ready,
            "public_third_party_contract_changed": false,
        },
        "redaction": {
            "raw_asset_locator_excluded": true,
            "source_url_value_included": false,
            "provider_payload_excluded": true,
            "secret_material_included": false,
            "local_path_value_included": false,
        },
    })
}

pub(crate) async fn upsert_fashion_design_image_asset_import(
    state: &AppState,
    input: FashionDesignImageAssetImportInput,
) -> std::result::Result<SyncedFashionDesignImageAssetImport, ApiError> {
    upsert_fashion_design_image_asset_import_for_tenant(state, state.tenant_id, input).await
}

pub(crate) async fn create_fashion_design_image_asset_import_response(
    state: &AppState,
    request: CreateFashionDesignImageAssetImportRequest,
) -> std::result::Result<CreateFashionDesignImageAssetImportResponse, ApiError> {
    let asset_library_id =
        parse_optional_uuid_ref("asset_library_id", request.asset_library_id.as_deref())?;
    let collection_id = parse_optional_uuid_ref("collection_id", request.collection_id.as_deref())?;
    validate_fashion_design_image_asset_import_scope_refs(state, asset_library_id, collection_id)
        .await?;
    let synced = upsert_fashion_design_image_asset_import(
        state,
        fashion_design_image_asset_import_input_from_request(request)?,
    )
    .await?;
    Ok(fashion_design_image_asset_import_response(synced))
}

pub(crate) async fn create_fashion_design_image_asset_import_batch_response(
    state: &AppState,
    request: CreateFashionDesignImageAssetImportBatchRequest,
) -> std::result::Result<CreateFashionDesignImageAssetImportBatchResponse, ApiError> {
    if request.assets.is_empty() && request.packages.is_empty() {
        return Err(ApiError::bad_request(
            "missing_asset_import_batch_assets",
            "asset import batch requires at least one asset or package".to_string(),
        ));
    }
    if request.assets.len() > 100 {
        return Err(ApiError::bad_request(
            "asset_import_batch_too_large",
            "asset import batch supports at most 100 assets per request".to_string(),
        ));
    }
    let asset_library_id =
        parse_optional_uuid_ref("asset_library_id", request.asset_library_id.as_deref())?;
    let collection_id = parse_optional_uuid_ref("collection_id", request.collection_id.as_deref())?;
    validate_fashion_design_image_asset_import_scope_refs(state, asset_library_id, collection_id)
        .await?;

    let direct_asset_count = request.assets.len();
    let package_count = request.packages.len();
    let inputs = fashion_design_image_asset_import_inputs_from_batch_request(request)?;
    if inputs.len() > 100 {
        return Err(ApiError::bad_request(
            "asset_import_batch_too_large",
            "asset import batch supports at most 100 expanded assets per request".to_string(),
        ));
    }
    let mut items = Vec::with_capacity(inputs.len());
    for input in inputs {
        let synced = upsert_fashion_design_image_asset_import(state, input).await?;
        items.push(fashion_design_image_asset_import_response(synced));
    }

    Ok(CreateFashionDesignImageAssetImportBatchResponse {
        accepted: true,
        asset_count: items.len(),
        package_count,
        expanded_asset_count: items.len().saturating_sub(direct_asset_count),
        items,
    })
}

pub(crate) async fn validate_fashion_design_image_asset_import_scope_refs(
    state: &AppState,
    asset_library_id: Option<Uuid>,
    collection_id: Option<Uuid>,
) -> std::result::Result<(), ApiError> {
    if let Some(asset_library_id) = asset_library_id {
        state
            .storage
            .asset_libraries()
            .get_by_id(state.tenant_id, asset_library_id)
            .await
            .map_err(ApiError::from_storage)?
            .ok_or_else(|| {
                ApiError::not_found(
                    "asset_library_not_found",
                    format!("asset library {asset_library_id} was not found"),
                )
            })?;
    }

    if let Some(collection_id) = collection_id {
        let collection_asset_library_id = state
            .storage
            .asset_libraries()
            .get_collection_asset_library_id(state.tenant_id, collection_id)
            .await
            .map_err(ApiError::from_storage)?
            .ok_or_else(|| {
                ApiError::not_found(
                    "asset_collection_not_found",
                    format!("asset collection {collection_id} was not found"),
                )
            })?;
        validate_fashion_design_image_asset_import_collection_scope(
            asset_library_id,
            collection_asset_library_id,
            collection_id,
        )?;
    }

    Ok(())
}

fn validate_fashion_design_image_asset_import_collection_scope(
    asset_library_id: Option<Uuid>,
    collection_asset_library_id: Uuid,
    collection_id: Uuid,
) -> std::result::Result<(), ApiError> {
    let Some(asset_library_id) = asset_library_id else {
        return Err(ApiError::bad_request(
            "asset_import_collection_requires_asset_library",
            "asset import collection_id requires asset_library_id".to_string(),
        ));
    };
    if asset_library_id != collection_asset_library_id {
        return Err(ApiError::bad_request(
            "asset_import_collection_library_mismatch",
            format!(
                "asset collection {collection_id} does not belong to asset library {asset_library_id}"
            ),
        ));
    }
    Ok(())
}

pub(crate) async fn upsert_fashion_design_image_asset_import_for_tenant(
    state: &AppState,
    tenant_id: TenantId,
    input: FashionDesignImageAssetImportInput,
) -> std::result::Result<SyncedFashionDesignImageAssetImport, ApiError> {
    let prepared = prepare_fashion_design_image_asset_import(input)?;
    let asset = state
        .storage
        .asset_items()
        .upsert_by_source(tenant_id, prepared.asset)
        .await
        .map_err(ApiError::from_storage)?;
    let dataset_membership = state
        .storage
        .asset_items()
        .upsert_dataset_membership(tenant_id, asset.id, prepared.dataset_membership)
        .await
        .map_err(ApiError::from_storage)?;
    let parse_run = state
        .storage
        .asset_items()
        .upsert_parse_run(tenant_id, asset.id, prepared.parse_run)
        .await
        .map_err(ApiError::from_storage)?;
    let profile = state
        .storage
        .asset_items()
        .upsert_profile(tenant_id, asset.id, prepared.profile)
        .await
        .map_err(ApiError::from_storage)?;

    enqueue_asset_parse_if_enabled(
        state,
        tenant_id,
        dataset_membership.dataset_id,
        &asset,
        &parse_run,
    )
    .await?;

    Ok(SyncedFashionDesignImageAssetImport {
        asset,
        dataset_membership,
        parse_run,
        profile,
    })
}

pub(crate) fn fashion_design_image_asset_import_input_from_request(
    request: CreateFashionDesignImageAssetImportRequest,
) -> std::result::Result<FashionDesignImageAssetImportInput, ApiError> {
    Ok(FashionDesignImageAssetImportInput {
        dataset_id: request.dataset_id,
        asset_library_id: parse_optional_uuid_ref(
            "asset_library_id",
            request.asset_library_id.as_deref(),
        )?,
        collection_id: parse_optional_uuid_ref("collection_id", request.collection_id.as_deref())?,
        stable_source_id_override: None,
        external_id: request.external_id,
        title: request.title,
        image_url: request.image_url,
        object_key: request.object_key,
        content_type: request.content_type,
        profile_payload: request.profile_payload,
        metadata: request.metadata,
    })
}

pub(crate) fn fashion_design_image_asset_import_inputs_from_batch_request(
    request: CreateFashionDesignImageAssetImportBatchRequest,
) -> std::result::Result<Vec<FashionDesignImageAssetImportInput>, ApiError> {
    let asset_library_id =
        parse_optional_uuid_ref("asset_library_id", request.asset_library_id.as_deref())?;
    let collection_id = parse_optional_uuid_ref("collection_id", request.collection_id.as_deref())?;
    let batch_metadata = request.metadata;
    let mut inputs = request
        .assets
        .into_iter()
        .map(|item| {
            fashion_design_image_asset_import_input_from_batch_item(
                request.dataset_id,
                asset_library_id,
                collection_id,
                &batch_metadata,
                item,
            )
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;

    for (package_index, package) in request.packages.into_iter().enumerate() {
        let items = expand_fashion_design_image_asset_import_package(package, package_index)?;
        for item in items {
            inputs.push(fashion_design_image_asset_import_input_from_batch_item(
                request.dataset_id,
                asset_library_id,
                collection_id,
                &batch_metadata,
                item,
            )?);
        }
    }

    Ok(inputs)
}

fn fashion_design_image_asset_import_input_from_batch_item(
    dataset_id: DatasetId,
    asset_library_id: Option<Uuid>,
    collection_id: Option<Uuid>,
    batch_metadata: &Value,
    item: FashionDesignImageAssetImportItem,
) -> std::result::Result<FashionDesignImageAssetImportInput, ApiError> {
    Ok(FashionDesignImageAssetImportInput {
        dataset_id,
        asset_library_id,
        collection_id,
        stable_source_id_override: None,
        external_id: item.external_id,
        title: item.title,
        image_url: item.image_url,
        object_key: item.object_key,
        content_type: item.content_type,
        profile_payload: item.profile_payload,
        metadata: merge_batch_asset_metadata(batch_metadata, item.metadata),
    })
}

fn expand_fashion_design_image_asset_import_package(
    package: FashionDesignImageAssetImportPackage,
    package_index: usize,
) -> std::result::Result<Vec<FashionDesignImageAssetImportItem>, ApiError> {
    validate_fashion_design_image_package_source(&package)?;
    let zip_path = resolve_platform_local_object_path(&package.object_key).ok_or_else(|| {
        ApiError::bad_request(
            "asset_import_package_object_not_found",
            "asset import package object file was not found on this server".to_string(),
        )
    })?;
    let file = File::open(&zip_path).map_err(|error| {
        ApiError::bad_request(
            "asset_import_package_open_failed",
            format!("failed to open asset import package: {error}"),
        )
    })?;
    let mut archive = ZipArchive::new(file).map_err(|error| {
        ApiError::bad_request(
            "asset_import_package_invalid",
            format!("asset import package is not a readable zip archive: {error}"),
        )
    })?;

    let max_images = zip_ingest_env_usize("ASSET_IMPORT_ZIP_MAX_IMAGES", 100).clamp(1, 500);
    let max_entry_bytes =
        zip_ingest_env_u64("ASSET_IMPORT_ZIP_MAX_ENTRY_BYTES", 80 * 1024 * 1024).max(1);
    let max_total_bytes =
        zip_ingest_env_u64("ASSET_IMPORT_ZIP_MAX_TOTAL_BYTES", 300 * 1024 * 1024).max(1);
    let output_root = fashion_design_image_zip_output_root(&zip_path, package_index);
    fs::create_dir_all(&output_root).map_err(|error| {
        ApiError::internal(
            "asset_import_package_expand_failed",
            format!("failed to create asset package extraction directory: {error}"),
        )
    })?;

    let mut items = Vec::new();
    let mut total_bytes = 0u64;
    for index in 0..archive.len() {
        if items.len() >= max_images {
            break;
        }
        let file = archive.by_index(index).map_err(|error| {
            ApiError::bad_request(
                "asset_import_package_read_failed",
                format!("failed to read asset package entry {index}: {error}"),
            )
        })?;
        if file.is_dir() {
            continue;
        }
        let Some(enclosed_name) = file.enclosed_name().map(PathBuf::from) else {
            continue;
        };
        let entry_name = enclosed_name.to_string_lossy().replace('\\', "/");
        if zip_entry_should_skip(&entry_name) {
            continue;
        }
        let extension = enclosed_name
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| format!(".{}", value.to_ascii_lowercase()))
            .unwrap_or_default();
        if !fashion_design_image_zip_extension_supported(&extension) {
            continue;
        }
        let entry_size = file.size();
        if entry_size > max_entry_bytes {
            continue;
        }
        total_bytes = total_bytes.saturating_add(entry_size);
        if total_bytes > max_total_bytes {
            return Err(ApiError::bad_request(
                "asset_import_package_too_large",
                format!("asset package expanded image content exceeds {max_total_bytes} bytes"),
            ));
        }

        let safe_name = safe_zip_entry_output_name(items.len(), &entry_name, &extension);
        let output_path = output_root.join(safe_name);
        let mut output = File::create(&output_path).map_err(|error| {
            ApiError::internal(
                "asset_import_package_expand_failed",
                format!("failed to create extracted asset image: {error}"),
            )
        })?;
        let copied =
            std::io::copy(&mut file.take(max_entry_bytes + 1), &mut output).map_err(|error| {
                ApiError::internal(
                    "asset_import_package_expand_failed",
                    format!("failed to write extracted asset image: {error}"),
                )
            })?;
        output.flush().map_err(|error| {
            ApiError::internal(
                "asset_import_package_expand_failed",
                format!("failed to flush extracted asset image: {error}"),
            )
        })?;
        if copied > max_entry_bytes {
            let _ = fs::remove_file(&output_path);
            continue;
        }

        items.push(FashionDesignImageAssetImportItem {
            external_id: fashion_design_image_zip_entry_external_id(&package, items.len()),
            title: fashion_design_image_zip_entry_title(&package, &entry_name),
            image_url: None,
            object_key: Some(output_path.to_string_lossy().to_string()),
            content_type: Some(infer_zip_child_content_type(&extension).to_string()),
            profile_payload: json!({}),
            metadata: fashion_design_image_zip_entry_metadata(
                &package,
                package_index,
                index,
                &entry_name,
                copied,
            ),
        });
    }

    if items.is_empty() {
        return Err(ApiError::bad_request(
            "asset_import_package_empty",
            "asset import package did not contain supported image files".to_string(),
        ));
    }

    Ok(items)
}

fn validate_fashion_design_image_package_source(
    package: &FashionDesignImageAssetImportPackage,
) -> std::result::Result<(), ApiError> {
    let object_key = required_trimmed("package_object_key", &package.object_key)?;
    let content_type = package.content_type.as_deref().and_then(trimmed);
    let is_zip_content_type = content_type
        .as_deref()
        .map(|value| {
            matches!(
                value
                    .split(';')
                    .next()
                    .unwrap_or(value)
                    .trim()
                    .to_ascii_lowercase()
                    .as_str(),
                "application/zip" | "application/x-zip-compressed" | "multipart/x-zip"
            )
        })
        .unwrap_or(false);
    if !is_zip_content_type && !object_key.to_ascii_lowercase().ends_with(".zip") {
        return Err(ApiError::bad_request(
            "invalid_asset_import_package_content_type",
            "asset import package must be a zip archive".to_string(),
        ));
    }
    Ok(())
}

fn fashion_design_image_zip_output_root(zip_path: &StdPath, package_index: usize) -> PathBuf {
    let base = zip_path
        .parent()
        .map(StdPath::to_path_buf)
        .unwrap_or_else(std::env::temp_dir);
    let stem = zip_path
        .file_stem()
        .and_then(|value| value.to_str())
        .map(safe_external_path_segment)
        .unwrap_or_else(|| "asset-package".to_string());
    base.join("_asset_zip_extracted")
        .join(format!("{:03}-{}", package_index + 1, stem))
}

fn fashion_design_image_zip_extension_supported(extension: &str) -> bool {
    matches!(
        extension,
        ".png" | ".jpg" | ".jpeg" | ".webp" | ".bmp" | ".tif" | ".tiff" | ".gif"
    )
}

fn fashion_design_image_zip_entry_external_id(
    package: &FashionDesignImageAssetImportPackage,
    asset_index: usize,
) -> Option<String> {
    if let Some(external_id) = package.external_id.as_deref().and_then(trimmed) {
        return Some(format!("{external_id}:{:04}", asset_index + 1));
    }
    trimmed(&package.object_key).map(|object_key| {
        let digest = sha256_hex([b"fashion-design-zip:", object_key.as_bytes()]);
        format!("zip-ref-{}:{:04}", &digest[..16], asset_index + 1)
    })
}

fn fashion_design_image_zip_entry_title(
    package: &FashionDesignImageAssetImportPackage,
    entry_name: &str,
) -> String {
    let entry_title = zip_entry_title(entry_name);
    package
        .title
        .as_deref()
        .and_then(trimmed)
        .map(|title| format!("{title} - {entry_title}"))
        .unwrap_or(entry_title)
}

fn fashion_design_image_zip_entry_metadata(
    package: &FashionDesignImageAssetImportPackage,
    package_index: usize,
    entry_index: usize,
    entry_name: &str,
    size_bytes: u64,
) -> Value {
    let mut metadata = fashion_design_image_safe_metadata_map(&package.metadata);
    metadata.insert(
        "source_package".to_string(),
        json!({
            "object_key_present": !package.object_key.trim().is_empty(),
            "external_id": package.external_id.as_deref(),
            "title": package.title.as_deref(),
            "package_index": package_index,
        }),
    );
    metadata.insert(
        "zip_entry".to_string(),
        json!({
            "entry_name_present": !entry_name.trim().is_empty(),
            "extension": StdPath::new(entry_name)
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| value.to_ascii_lowercase()),
            "entry_index": entry_index,
            "size_bytes": size_bytes,
        }),
    );
    Value::Object(metadata)
}

pub(crate) fn prepare_fashion_design_image_asset_import(
    input: FashionDesignImageAssetImportInput,
) -> std::result::Result<PreparedFashionDesignImageAssetImport, ApiError> {
    let title = required_trimmed("title", &input.title)?;
    let source_id = fashion_design_image_source_id(&input)?;
    let content_type = input.content_type.as_deref().and_then(trimmed);
    if let Some(content_type) = content_type.as_deref() {
        if !content_type
            .split(';')
            .next()
            .unwrap_or(content_type)
            .trim()
            .eq_ignore_ascii_case("image/png")
            && !content_type
                .split(';')
                .next()
                .unwrap_or(content_type)
                .trim()
                .eq_ignore_ascii_case("image/jpeg")
            && !content_type
                .split(';')
                .next()
                .unwrap_or(content_type)
                .trim()
                .eq_ignore_ascii_case("image/webp")
            && !content_type
                .split(';')
                .next()
                .unwrap_or(content_type)
                .trim()
                .starts_with("image/")
        {
            return Err(ApiError::bad_request(
                "invalid_asset_import_content_type",
                "asset import content_type must be an image/* type".to_string(),
            ));
        }
    }

    Ok(PreparedFashionDesignImageAssetImport {
        asset: NewAssetItem {
            asset_library_id: input.asset_library_id,
            collection_id: input.collection_id,
            external_id: input.external_id.as_deref().and_then(trimmed),
            title,
            asset_kind: "image".to_string(),
            source_kind: "fashion_design_image_import".to_string(),
            source_id: Some(source_id),
            content_type,
            object_key: input.object_key.as_deref().and_then(trimmed),
            metadata: fashion_design_image_asset_metadata(&input),
        },
        dataset_membership: NewDatasetAssetMembership {
            dataset_id: input.dataset_id,
            membership_kind: "imported".to_string(),
            expires_at: None,
        },
        parse_run: NewAssetParseRun {
            parser_name: "datamax-fashion-image-parser".to_string(),
            parser_version: "2026-06-17".to_string(),
            status: "pending".to_string(),
            started_at: None,
            finished_at: None,
            error_code: None,
            error_message: None,
            metadata: fashion_design_image_parse_run_metadata(&input),
        },
        profile: NewAssetProfile {
            profile_kind: FASHION_DESIGN_IMAGE_PROFILE_KIND.to_string(),
            profile_version: "v1".to_string(),
            attributes: fashion_postchain_profile_attributes(&input.profile_payload),
            embedding_status: "not_requested".to_string(),
        },
    })
}

fn fashion_design_image_asset_import_response(
    synced: SyncedFashionDesignImageAssetImport,
) -> CreateFashionDesignImageAssetImportResponse {
    CreateFashionDesignImageAssetImportResponse {
        asset: asset_item_view(synced.asset),
        dataset_membership: dataset_asset_membership_view(synced.dataset_membership),
        parse_run: asset_parse_run_view(synced.parse_run),
        profile: asset_profile_view(synced.profile),
    }
}

fn parse_optional_uuid_ref(
    field: &str,
    value: Option<&str>,
) -> std::result::Result<Option<Uuid>, ApiError> {
    let Some(value) = value.and_then(trimmed) else {
        return Ok(None);
    };
    Uuid::parse_str(&value).map(Some).map_err(|_| {
        ApiError::bad_request(
            &format!("invalid_asset_import_{field}"),
            format!("asset import {field} must be a UUID"),
        )
    })
}

fn fashion_design_image_source_id(
    input: &FashionDesignImageAssetImportInput,
) -> std::result::Result<String, ApiError> {
    for value in [
        input.stable_source_id_override.as_deref(),
        input.external_id.as_deref(),
        input.object_key.as_deref(),
        input.image_url.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(value) = trimmed(value) {
            return Ok(value);
        }
    }
    Err(ApiError::bad_request(
        "missing_asset_import_source",
        "asset import requires external_id, object_key, or image_url".to_string(),
    ))
}

fn fashion_design_image_asset_metadata(input: &FashionDesignImageAssetImportInput) -> Value {
    let mut metadata = fashion_design_image_safe_metadata_map(&input.metadata);
    metadata.insert(
        "asset_import".to_string(),
        json!({
            "kind": "fashion_design_image",
            "source_present": fashion_design_image_has_source(input),
            "source_kind": fashion_design_image_source_kind(input),
            "external_id_present": input.external_id.as_deref().and_then(trimmed).is_some(),
            "object_key_present": input.object_key.as_deref().and_then(trimmed).is_some(),
            "image_url_present": input.image_url.as_deref().and_then(trimmed).is_some(),
            "content_type": input.content_type.as_deref().and_then(trimmed),
            "profile_schema": FASHION_DESIGN_IMAGE_PROFILE_KIND,
        }),
    );
    Value::Object(metadata)
}

fn fashion_design_image_parse_run_metadata(input: &FashionDesignImageAssetImportInput) -> Value {
    let profile_seeded = input
        .profile_payload
        .as_object()
        .is_some_and(|object| !object.is_empty());
    let worker_input_contract =
        fashion_design_image_parser_worker_input_contract(&FashionDesignImageParserWorkerInput {
            source_ref_present: fashion_design_image_has_source(input),
            source_kind: fashion_design_image_source_kind(input),
            content_type: input.content_type.as_deref().and_then(trimmed),
            profile_seeded,
        });
    json!({
        "asset_import": {
            "kind": "fashion_design_image",
            "source_present": fashion_design_image_has_source(input),
            "source_kind": fashion_design_image_source_kind(input),
            "external_id_present": input.external_id.as_deref().and_then(trimmed).is_some(),
            "object_key_present": input.object_key.as_deref().and_then(trimmed).is_some(),
            "image_url_present": input.image_url.as_deref().and_then(trimmed).is_some(),
            "content_type": input.content_type.as_deref().and_then(trimmed),
            "profile_seeded": profile_seeded,
        },
        "followup_plan": {
            "parse_queue": {
                "action": "enqueue_asset_parse",
                "task_kind": FASHION_DESIGN_IMAGE_PARSER_TASK_KIND,
                "parser_name": FASHION_DESIGN_IMAGE_PARSER_NAME,
                "parser_version": FASHION_DESIGN_IMAGE_PARSER_VERSION,
                "status": "pending",
                "source_ref_present": fashion_design_image_has_source(input),
                "source_kind": fashion_design_image_source_kind(input),
                "status_transition_policy": {
                    "pending_ready_next": "completed",
                    "pending_partial_next": "retrying",
                    "parsing_ready_next": "completed",
                    "parsing_partial_next": "retrying",
                    "failed_ready_next": "completed",
                    "failed_partial_next": "retrying",
                    "completed_next": "completed",
                },
            },
            "worker_input_contract": worker_input_contract,
            "retrieval_evidence": {
                "action": "upsert_retrieval_evidence",
                "mode": "profile_to_text_after_parse",
                "source_kind": "asset_profile",
                "profile_schema": FASHION_DESIGN_IMAGE_PROFILE_KIND,
                "dry_run_ready": profile_seeded,
                "write_policy": "after_profile_available",
                "idempotency_key_template": format!(
                    "asset-profile:{{asset_id}}:{}:{}:{}",
                    FASHION_DESIGN_IMAGE_PROFILE_KIND,
                    FASHION_DESIGN_IMAGE_PARSER_NAME,
                    FASHION_DESIGN_IMAGE_PARSER_VERSION
                ),
                "dedupe_scope": [
                    "tenant_id",
                    "dataset_id",
                    "asset_id",
                    "profile_kind",
                    "parser_name",
                    "parser_version"
                ],
            },
            "worker_commit_order": [
                {
                    "step": 1,
                    "action": "normalize_worker_output",
                    "writes": [],
                },
                {
                    "step": 2,
                    "action": "upsert_asset_profile",
                    "required_before": "materialize_retrieval_evidence_text",
                },
                {
                    "step": 3,
                    "action": "materialize_retrieval_evidence_text",
                    "requires": "asset_profile_upserted",
                },
                {
                    "step": 4,
                    "action": "upsert_retrieval_evidence",
                    "requires": "materialized_retrieval_evidence_text",
                    "idempotent": true,
                },
                {
                    "step": 5,
                    "action": "mark_parse_run_completed",
                    "requires": "retrieval_evidence_upserted",
                },
            ],
            "worker_commit_gate": {
                "mark_completed_after": [
                    "asset_profile_upserted",
                    "retrieval_evidence_upserted"
                ],
                "partial_failure_next_status": "retrying",
                "do_not_mark_completed_until_evidence_upsert_succeeds": true,
            },
        },
        "parse_method": "ocr_vlm_pending",
        "profile_schema": FASHION_DESIGN_IMAGE_PROFILE_KIND,
    })
}

fn fashion_design_image_has_source(input: &FashionDesignImageAssetImportInput) -> bool {
    input.external_id.as_deref().and_then(trimmed).is_some()
        || input.object_key.as_deref().and_then(trimmed).is_some()
        || input.image_url.as_deref().and_then(trimmed).is_some()
}

fn fashion_design_image_source_kind(
    input: &FashionDesignImageAssetImportInput,
) -> Option<&'static str> {
    if input.external_id.as_deref().and_then(trimmed).is_some() {
        Some("external_id")
    } else if input.object_key.as_deref().and_then(trimmed).is_some() {
        Some("object_key")
    } else if input.image_url.as_deref().and_then(trimmed).is_some() {
        Some("image_url")
    } else {
        None
    }
}

fn fashion_design_image_safe_metadata_map(metadata: &Value) -> Map<String, Value> {
    let mut safe = Map::new();
    let Some(object) = metadata.as_object() else {
        return safe;
    };
    for (key, value) in object {
        if fashion_design_image_metadata_key_is_sensitive(key) {
            continue;
        }
        if let Some(value) = fashion_design_image_safe_metadata_value(value) {
            safe.insert(key.clone(), value);
        }
    }
    safe
}

fn fashion_design_image_safe_metadata_value(value: &Value) -> Option<Value> {
    match value {
        Value::Object(object) => {
            let mut safe = Map::new();
            for (key, value) in object {
                if fashion_design_image_metadata_key_is_sensitive(key) {
                    continue;
                }
                if let Some(value) = fashion_design_image_safe_metadata_value(value) {
                    safe.insert(key.clone(), value);
                }
            }
            Some(Value::Object(safe))
        }
        Value::Array(items) => Some(Value::Array(
            items
                .iter()
                .filter_map(fashion_design_image_safe_metadata_value)
                .collect(),
        )),
        _ => Some(value.clone()),
    }
}

fn fashion_design_image_metadata_key_is_sensitive(key: &str) -> bool {
    matches!(
        key.trim().to_ascii_lowercase().as_str(),
        "source"
            | "url"
            | "image_url"
            | "imageurl"
            | "object_key"
            | "objectkey"
            | "package_source"
            | "packagesource"
            | "source_id"
            | "sourceid"
            | "filename"
            | "file_name"
            | "original_name"
            | "path"
            | "file_path"
            | "local_path"
            | "content_hash"
            | "sha256"
            | "raw_provider_payload"
            | "provider_payload"
            | "raw_payload"
            | "authorization"
            | "cookie"
    )
}

fn merge_batch_asset_metadata(batch_metadata: &Value, item_metadata: Value) -> Value {
    let mut metadata = match batch_metadata.as_object() {
        Some(object) => object.clone(),
        None => Map::new(),
    };
    if let Some(item_object) = item_metadata.as_object() {
        for (key, value) in item_object {
            metadata.insert(key.clone(), value.clone());
        }
    }
    Value::Object(metadata)
}

fn required_trimmed(field: &str, value: &str) -> std::result::Result<String, ApiError> {
    trimmed(value).ok_or_else(|| {
        ApiError::bad_request(
            &format!("missing_asset_import_{field}"),
            format!("asset import requires {field}"),
        )
    })
}

fn trimmed(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use domain_model::DatasetId;
    use serde_json::json;
    use storage::NewDataset;
    use test_fixtures::{
        local_postgres_storage, reset_local_postgres_storage, shared_local_postgres_test_lock,
    };
    use uuid::Uuid;

    use super::*;

    fn input() -> FashionDesignImageAssetImportInput {
        FashionDesignImageAssetImportInput {
            dataset_id: DatasetId(Uuid::from_u128(1)),
            asset_library_id: Some(Uuid::from_u128(2)),
            collection_id: Some(Uuid::from_u128(3)),
            stable_source_id_override: None,
            external_id: Some("img-001".to_string()),
            title: "春夏连衣裙灵感图".to_string(),
            image_url: Some("https://example.com/img-001.png".to_string()),
            object_key: Some("assets/fashion/img-001.png".to_string()),
            content_type: Some("image/png".to_string()),
            profile_payload: json!({
                "category": "dress",
                "season": "spring_summer",
                "style": ["commute"],
                "collar": "round_neck",
                "sleeve": "short_sleeve",
                "color": ["green", "white"],
                "caption": "春夏通勤连衣裙"
            }),
            metadata: json!({
                "source": "https://example.com/raw-source.png",
                "operator": "main-site",
                "raw_provider_payload": {"should_not_surface": true},
                "nested": {
                    "object_key": "objects/private/look-001.png",
                    "label": "safe-label"
                }
            }),
        }
    }

    fn request() -> CreateFashionDesignImageAssetImportRequest {
        CreateFashionDesignImageAssetImportRequest {
            dataset_id: DatasetId(Uuid::from_u128(1)),
            asset_library_id: Some(Uuid::from_u128(2).to_string()),
            collection_id: Some(Uuid::from_u128(3).to_string()),
            external_id: Some("img-001".to_string()),
            title: "春夏连衣裙灵感图".to_string(),
            image_url: Some("https://example.com/img-001.png".to_string()),
            object_key: Some("assets/fashion/img-001.png".to_string()),
            content_type: Some("image/png".to_string()),
            profile_payload: json!({"category": "dress"}),
            metadata: json!({"source": "operator_fixture"}),
        }
    }

    fn batch_request() -> CreateFashionDesignImageAssetImportBatchRequest {
        CreateFashionDesignImageAssetImportBatchRequest {
            dataset_id: DatasetId(Uuid::from_u128(1)),
            asset_library_id: Some(Uuid::from_u128(2).to_string()),
            collection_id: Some(Uuid::from_u128(3).to_string()),
            metadata: json!({"source_package": "fashion-zip-001", "operator": "main-site"}),
            packages: vec![],
            assets: vec![
                FashionDesignImageAssetImportItem {
                    external_id: Some("img-001".to_string()),
                    title: "春夏连衣裙灵感图".to_string(),
                    image_url: Some("https://example.com/img-001.png".to_string()),
                    object_key: None,
                    content_type: Some("image/png".to_string()),
                    profile_payload: json!({"category": "dress"}),
                    metadata: json!({"filename": "img-001.png"}),
                },
                FashionDesignImageAssetImportItem {
                    external_id: Some("img-002".to_string()),
                    title: "通勤外套灵感图".to_string(),
                    image_url: Some("https://example.com/img-002.jpg".to_string()),
                    object_key: None,
                    content_type: Some("image/jpeg".to_string()),
                    profile_payload: json!({"category": "jacket"}),
                    metadata: json!({"operator": "override"}),
                },
            ],
        }
    }

    #[test]
    fn asset_parse_enqueue_builds_one_ingest_task_after_successful_import() {
        let asset_id = Uuid::from_u128(11);
        let parse_run_id = Uuid::from_u128(12);
        let queued_at = Utc::now();

        let task = build_asset_parse_enqueue_task(
            true,
            asset_id,
            parse_run_id,
            FASHION_DESIGN_IMAGE_PARSER_NAME,
            FASHION_DESIGN_IMAGE_PARSER_VERSION,
            "pending",
            queued_at,
            2,
        )
        .expect("enabled pending import should enqueue asset parsing");

        assert_eq!(task.queue, contracts::ASSET_PROFILE_PARSE_QUEUE);
        assert_eq!(task.task_key, contracts::ASSET_PROFILE_PARSE_TASK_KEY);
        assert_eq!(task.max_attempts, 2);
        assert_eq!(task.payload["asset_id"], json!(asset_id.to_string()));
        assert_eq!(
            task.payload["parse_run_id"],
            json!(parse_run_id.to_string())
        );
        assert_eq!(
            task.payload["parser_version"],
            json!(FASHION_DESIGN_IMAGE_PARSER_VERSION)
        );
        assert!(task.payload.get("object_key").is_none());
        assert!(task.payload.get("image_url").is_none());
    }

    #[test]
    fn asset_parse_enqueue_is_disabled_by_default_and_skips_terminal_runs() {
        let queued_at = Utc::now();
        let args = (
            Uuid::from_u128(21),
            Uuid::from_u128(22),
            FASHION_DESIGN_IMAGE_PARSER_NAME,
            FASHION_DESIGN_IMAGE_PARSER_VERSION,
        );

        assert!(build_asset_parse_enqueue_task(
            false, args.0, args.1, args.2, args.3, "pending", queued_at, 2,
        )
        .is_none());
        assert!(build_asset_parse_enqueue_task(
            true,
            args.0,
            args.1,
            args.2,
            args.3,
            "completed",
            queued_at,
            2,
        )
        .is_none());
        assert!(build_asset_parse_enqueue_task(
            true, args.0, args.1, args.2, args.3, "partial", queued_at, 2,
        )
        .is_none());
    }

    #[test]
    fn asset_parse_enqueue_uses_registered_upload_ingest_workflow_version() {
        let workflow_catalog = workflow_definitions::catalog();
        let registered_version = workflow_catalog
            .find_definition(WorkflowKind::UploadIngest)
            .expect("upload ingest workflow should be registered")
            .version()
            .to_string();

        assert_eq!(
            asset_parse_workflow_version(&workflow_catalog)
                .expect("asset parser should reuse a registered workflow version"),
            registered_version
        );
        assert_ne!(registered_version, "asset-profile-parse-v1");
    }

    #[test]
    fn asset_parse_enqueue_fails_closed_without_registered_upload_ingest_workflow() {
        let workflow_catalog = workflow_engine::WorkflowCatalog::default();

        assert!(asset_parse_workflow_version(&workflow_catalog).is_err());
    }

    #[test]
    fn asset_parse_enqueue_dedupe_key_is_stable_per_asset_and_parser_version() {
        let tenant_id = TenantId(Uuid::from_u128(31));
        let asset_id = Uuid::from_u128(32);

        let first = asset_parse_runnable_dedupe_key(
            tenant_id,
            asset_id,
            FASHION_DESIGN_IMAGE_PARSER_NAME,
            FASHION_DESIGN_IMAGE_PARSER_VERSION,
        );
        let repeated = asset_parse_runnable_dedupe_key(
            tenant_id,
            asset_id,
            FASHION_DESIGN_IMAGE_PARSER_NAME,
            FASHION_DESIGN_IMAGE_PARSER_VERSION,
        );
        let next_version = asset_parse_runnable_dedupe_key(
            tenant_id,
            asset_id,
            FASHION_DESIGN_IMAGE_PARSER_NAME,
            "2026-07-11",
        );

        assert_eq!(first, repeated);
        assert_ne!(first, next_version);
        assert!(!first.contains(&tenant_id.to_string()));
        assert!(!first.contains(&asset_id.to_string()));
    }

    #[tokio::test]
    async fn asset_parse_enqueue_allows_at_most_one_runnable_task_per_dedupe_key() {
        let _guard = shared_local_postgres_test_lock().lock().await;
        let storage = match local_postgres_storage().await {
            Ok(storage) => storage,
            Err(reason) => {
                eprintln!("skipping asset parse enqueue dedupe test: {reason}");
                return;
            }
        };
        reset_local_postgres_storage(&storage)
            .await
            .expect("test storage should reset");
        let tenant = storage
            .ensure_tenant(
                &format!("asset-parse-enqueue-{}", Uuid::new_v4()),
                "Asset Parse Enqueue",
            )
            .await
            .expect("tenant should exist");
        let dataset = storage
            .datasets()
            .create(
                tenant.id,
                NewDataset {
                    key: format!("asset-parse-enqueue-{}", Uuid::new_v4()),
                    title: "Asset parse enqueue".to_string(),
                    description: None,
                    owner_user_id: None,
                },
            )
            .await
            .expect("dataset should exist");
        let asset_id = Uuid::new_v4();
        let parse_run_id = Uuid::new_v4();
        let dedupe_key = asset_parse_runnable_dedupe_key(
            tenant.id,
            asset_id,
            FASHION_DESIGN_IMAGE_PARSER_NAME,
            FASHION_DESIGN_IMAGE_PARSER_VERSION,
        );
        let now = Utc::now();
        let mut task = build_asset_parse_enqueue_task(
            true,
            asset_id,
            parse_run_id,
            FASHION_DESIGN_IMAGE_PARSER_NAME,
            FASHION_DESIGN_IMAGE_PARSER_VERSION,
            "pending",
            now,
            2,
        )
        .expect("task should build");
        task.payload["dedupe_key"] = json!(dedupe_key);

        let first_execution = test_asset_parse_execution(tenant.id, dataset.id, now);
        let first_event = test_asset_parse_event(&first_execution, now);
        let first = storage
            .workflow_tasks()
            .create_asset_parse_if_absent(&first_execution, &first_event, &task, &dedupe_key, now)
            .await
            .expect("first enqueue should succeed");
        assert!(first.is_some());

        let second_execution = test_asset_parse_execution(tenant.id, dataset.id, now);
        let second_event = test_asset_parse_event(&second_execution, now);
        let second = storage
            .workflow_tasks()
            .create_asset_parse_if_absent(&second_execution, &second_event, &task, &dedupe_key, now)
            .await
            .expect("duplicate enqueue should be handled");
        assert!(second.is_none());
        assert!(storage
            .workflow_executions()
            .get_by_id(tenant.id, second_execution.id)
            .await
            .expect("execution lookup should succeed")
            .is_none());
        let tasks = storage
            .workflow_tasks()
            .list_by_execution(first_execution.id)
            .await
            .expect("tasks should load");
        assert_eq!(tasks.len(), 1);
    }

    fn test_asset_parse_execution(
        tenant_id: TenantId,
        dataset_id: DatasetId,
        now: DateTime<Utc>,
    ) -> WorkflowExecution {
        WorkflowExecution {
            id: WorkflowExecutionId::new(),
            tenant_id,
            dataset_id: Some(dataset_id),
            report_plan_id: None,
            kind: WorkflowKind::UploadIngest,
            version: asset_parse_workflow_version(&workflow_definitions::catalog())
                .expect("upload ingest workflow should be registered"),
            stage: contracts::ASSET_PROFILE_PARSE_TASK_KEY.to_string(),
            status: WorkflowStatus::Running,
            attempt: 0,
            context: json!({"workflow_envelope": "asset_profile_parse"}),
            created_at: now,
            updated_at: now,
        }
    }

    fn test_asset_parse_event(
        execution: &WorkflowExecution,
        now: DateTime<Utc>,
    ) -> WorkflowEventRecord {
        WorkflowEventRecord {
            id: WorkflowEventId::new(),
            execution_id: execution.id,
            sequence_no: 1,
            event_name: "asset_profile_parse.queued".to_string(),
            payload: json!({}),
            created_at: now,
        }
    }

    #[test]
    fn asset_import_prepares_fashion_design_image_asset_profile_and_membership() {
        let prepared =
            prepare_fashion_design_image_asset_import(input()).expect("asset import should build");

        assert_eq!(prepared.asset.asset_kind, "image");
        assert_eq!(
            prepared.asset.source_kind,
            "fashion_design_image_import".to_string()
        );
        assert_eq!(prepared.asset.source_id.as_deref(), Some("img-001"));
        assert_eq!(prepared.asset.external_id.as_deref(), Some("img-001"));
        assert_eq!(prepared.asset.content_type.as_deref(), Some("image/png"));
        assert_eq!(prepared.dataset_membership.membership_kind, "imported");
        assert_eq!(
            prepared.parse_run.parser_name,
            "datamax-fashion-image-parser"
        );
        assert_eq!(prepared.parse_run.status, "pending");
        assert_eq!(
            prepared.parse_run.metadata["parse_method"],
            json!("ocr_vlm_pending")
        );
        assert_eq!(
            prepared.parse_run.metadata["asset_import"]["profile_seeded"],
            json!(true)
        );
        assert_eq!(
            prepared.profile.profile_kind,
            FASHION_DESIGN_IMAGE_PROFILE_KIND
        );
        assert_eq!(prepared.profile.attributes["category"], json!("dress"));
        assert_eq!(
            prepared.profile.attributes["seasons"],
            json!(["spring_summer"])
        );
        assert_eq!(
            prepared.profile.attributes["collars"],
            json!(["round_neck"])
        );
        assert_eq!(
            prepared.asset.metadata["asset_import"]["profile_schema"],
            json!("fashion_design_image_v1")
        );
        assert_eq!(
            prepared.asset.metadata["asset_import"]["source_kind"],
            json!("external_id")
        );
        assert_eq!(
            prepared.asset.metadata["asset_import"]["image_url_present"],
            json!(true)
        );
        assert_eq!(
            prepared.asset.metadata["asset_import"]["object_key_present"],
            json!(true)
        );
        assert_eq!(prepared.asset.metadata["operator"], json!("main-site"));
        assert_eq!(
            prepared.asset.metadata["nested"]["label"],
            json!("safe-label")
        );
        assert!(prepared.asset.metadata.get("source").is_none());
        assert!(prepared
            .asset
            .metadata
            .get("raw_provider_payload")
            .is_none());
        assert!(prepared.asset.metadata["nested"]
            .get("object_key")
            .is_none());
        assert!(prepared.parse_run.metadata["asset_import"]
            .get("source")
            .is_none());
        assert_eq!(
            prepared.parse_run.metadata["asset_import"]["source_kind"],
            json!("external_id")
        );
        assert_eq!(
            prepared.parse_run.metadata["followup_plan"]["parse_queue"]["task_kind"],
            json!("fashion_design_image_ocr_vlm")
        );
        assert_eq!(
            prepared.parse_run.metadata["followup_plan"]["parse_queue"]["source_kind"],
            json!("external_id")
        );
        assert_eq!(
            prepared.parse_run.metadata["followup_plan"]["parse_queue"]["status_transition_policy"]
                ["pending_ready_next"],
            json!("completed")
        );
        assert_eq!(
            prepared.parse_run.metadata["followup_plan"]["parse_queue"]["status_transition_policy"]
                ["pending_partial_next"],
            json!("retrying")
        );
        assert_eq!(
            prepared.parse_run.metadata["followup_plan"]["worker_input_contract"]["task_kind"],
            json!("fashion_design_image_ocr_vlm")
        );
        assert_eq!(
            prepared.parse_run.metadata["followup_plan"]["worker_input_contract"]
                ["requested_outputs"][1],
            json!("retrieval_evidence_text")
        );
        assert_eq!(
            prepared.parse_run.metadata["followup_plan"]["retrieval_evidence"]["source_kind"],
            json!("asset_profile")
        );
        assert_eq!(
            prepared.parse_run.metadata["followup_plan"]["retrieval_evidence"]["write_policy"],
            json!("after_profile_available")
        );
        assert_eq!(
            prepared.parse_run.metadata["followup_plan"]["retrieval_evidence"]["action"],
            json!("upsert_retrieval_evidence")
        );
        assert_eq!(
            prepared.parse_run.metadata["followup_plan"]["retrieval_evidence"]["idempotency_key_template"],
            json!(
                "asset-profile:{asset_id}:fashion_design_image_v1:datamax-fashion-image-parser:2026-06-17"
            )
        );
        assert_eq!(
            prepared.parse_run.metadata["followup_plan"]["retrieval_evidence"]["dedupe_scope"][2],
            json!("asset_id")
        );
        assert_eq!(
            prepared.parse_run.metadata["followup_plan"]["retrieval_evidence"]["dry_run_ready"],
            json!(true)
        );
        assert_eq!(
            prepared.parse_run.metadata["followup_plan"]["worker_commit_order"][1]["action"],
            json!("upsert_asset_profile")
        );
        assert_eq!(
            prepared.parse_run.metadata["followup_plan"]["worker_commit_order"][3]["action"],
            json!("upsert_retrieval_evidence")
        );
        assert_eq!(
            prepared.parse_run.metadata["followup_plan"]["worker_commit_order"][4]["requires"],
            json!("retrieval_evidence_upserted")
        );
        assert_eq!(
            prepared.parse_run.metadata["followup_plan"]["worker_commit_gate"]
                ["partial_failure_next_status"],
            json!("retrying")
        );
        assert_eq!(
            prepared.parse_run.metadata["followup_plan"]["worker_commit_gate"]
                ["do_not_mark_completed_until_evidence_upsert_succeeds"],
            json!(true)
        );
        let metadata_serialized = serde_json::to_string(&json!({
            "asset": prepared.asset.metadata,
            "parse_run": prepared.parse_run.metadata,
        }))
        .expect("metadata should serialize");
        assert!(!metadata_serialized.contains("https://example.com"));
        assert!(!metadata_serialized.contains("assets/fashion/img-001.png"));
        assert!(!metadata_serialized.contains("objects/private"));
        assert!(!metadata_serialized.contains("raw_provider_payload"));
        assert!(prepared
            .profile
            .attributes
            .get("unknown_provider_blob")
            .is_none());
    }

    #[test]
    fn live_execute_operator_manifest_dry_run_requires_reviewed_inputs_without_secrets() {
        let manifest = fashion_design_asset_import_live_execute_operator_manifest_dry_run(
            false, false, false, false, false, false,
        );

        assert_eq!(
            manifest["contract"],
            json!("fashion_design_asset_import_live_execute_operator_manifest_dry_run_v1")
        );
        assert_eq!(manifest["no_write"], json!(true));
        assert_eq!(manifest["execute_ready"], json!(false));
        assert_eq!(
            manifest["required_inputs"]["v3_user_session_required"],
            json!(true)
        );
        assert_eq!(
            manifest["required_inputs"]["v3_user_session_supplied"],
            json!(false)
        );
        assert_eq!(
            manifest["required_inputs"]["existing_dataset_required"],
            json!(true)
        );
        assert_eq!(
            manifest["required_inputs"]["existing_asset_library_required"],
            json!(true)
        );
        assert_eq!(
            manifest["review_gates"]["operator_review_required"],
            json!(true)
        );
        assert_eq!(
            manifest["rollback_plan"]["automatic_cleanup_allowed"],
            json!(false)
        );
        assert_eq!(manifest["production_write_allowed"], json!(false));

        let serialized = serde_json::to_string(&manifest).expect("manifest should serialize");
        assert!(!serialized.contains("Bearer "));
        assert!(!serialized.contains("cookie"));
        assert!(!serialized.contains("objects/private"));
        assert!(!serialized.contains("http://"));
        assert!(!serialized.contains("https://"));
    }

    #[test]
    fn live_execute_operator_manifest_dry_run_ready_still_requires_manual_review() {
        let manifest = fashion_design_asset_import_live_execute_operator_manifest_dry_run(
            true, true, true, true, true, true,
        );

        assert_eq!(manifest["execute_ready"], json!(true));
        assert_eq!(
            manifest["review_gates"]["manifest_must_be_reviewed_before_execute"],
            json!(true)
        );
        assert_eq!(
            manifest["review_gates"]["dataset_and_asset_library_must_already_exist"],
            json!(true)
        );
        assert_eq!(
            manifest["audit_requirements"]["record_approval_id_hash"],
            json!(true)
        );
        assert_eq!(
            manifest["audit_requirements"]["record_auth_material"],
            json!(false)
        );
        assert_eq!(
            manifest["redaction"]["raw_dataset_id_excluded"],
            json!(true)
        );
        assert_eq!(manifest["production_write_allowed"], json!(false));
    }

    #[test]
    fn post_execute_cleanup_manifest_dry_run_blocks_without_reviewed_receipt() {
        let manifest = fashion_design_asset_import_post_execute_cleanup_manifest_dry_run(
            false, false, 0, 0, 0, 0,
        );

        assert_eq!(
            manifest["contract"],
            json!("fashion_design_asset_import_post_execute_cleanup_manifest_dry_run_v1")
        );
        assert_eq!(manifest["no_write"], json!(true));
        assert_eq!(manifest["no_delete"], json!(true));
        assert_eq!(manifest["cleanup_manifest_ready"], json!(false));
        assert_eq!(
            manifest["required_inputs"]["approval_hash_required"],
            json!(true)
        );
        assert_eq!(
            manifest["required_inputs"]["live_execute_receipt_required"],
            json!(true)
        );
        assert_eq!(
            manifest["execution_controls"]["automatic_cleanup_allowed"],
            json!(false)
        );
        assert_eq!(
            manifest["receipt_shape"]["raw_object_locators_excluded_from_shared_receipt"],
            json!(true)
        );
        assert_eq!(manifest["production_write_allowed"], json!(false));

        let serialized = serde_json::to_string(&manifest).expect("manifest should serialize");
        assert!(!serialized.contains("Bearer "));
        assert!(!serialized.contains("cookie"));
        assert!(!serialized.contains("object_key"));
        assert!(!serialized.contains("objects/private"));
        assert!(!serialized.contains("http://"));
        assert!(!serialized.contains("https://"));
    }

    #[test]
    fn post_execute_cleanup_manifest_dry_run_ready_still_disables_cleanup_actions() {
        let manifest = fashion_design_asset_import_post_execute_cleanup_manifest_dry_run(
            true, true, 3, 3, 3, 3,
        );

        assert_eq!(manifest["cleanup_manifest_ready"], json!(true));
        assert_eq!(manifest["counts"]["asset_count"], json!(3));
        assert_eq!(
            manifest["selection_policy"]["match_by_approval_hash"],
            json!(true)
        );
        assert_eq!(
            manifest["selection_policy"]["shared_receipt_must_not_include_record_ids"],
            json!(true)
        );
        assert_eq!(
            manifest["execution_controls"]["cleanup_manifest_must_be_reviewed_before_action"],
            json!(true)
        );
        assert_eq!(
            manifest["execution_controls"]["automatic_cleanup_allowed"],
            json!(false)
        );
        assert_eq!(
            manifest["audit_requirements"]["record_object_locators"],
            json!(false)
        );
        assert_eq!(manifest["no_delete"], json!(true));
        assert_eq!(manifest["production_write_allowed"], json!(false));
    }

    #[test]
    fn operator_handoff_summary_dry_run_waits_for_inputs_without_side_effects() {
        let summary = fashion_design_asset_import_operator_handoff_summary_dry_run(
            false, false, false, false, true, 0,
        );

        assert_eq!(
            summary["contract"],
            json!("fashion_design_asset_import_operator_handoff_summary_dry_run_v1")
        );
        assert_eq!(summary["no_write"], json!(true));
        assert_eq!(summary["no_delete"], json!(true));
        assert_eq!(summary["no_callback"], json!(true));
        assert_eq!(summary["no_task_creation"], json!(true));
        assert_eq!(summary["public_contract_changed"], json!(false));
        assert_eq!(
            summary["task_card_summary"]["status"],
            json!("waiting_for_inputs")
        );
        assert_eq!(
            summary["task_card_summary"]["execute_state"],
            json!("blocked_missing_execute_inputs")
        );
        assert_eq!(
            summary["task_card_summary"]["cleanup_state"],
            json!("cleanup_waiting_for_live_receipt")
        );
        assert_eq!(
            summary["validation_summary"]["safe_for_shared_receipt"],
            json!(true)
        );

        let serialized = serde_json::to_string(&summary).expect("summary should serialize");
        assert!(!serialized.contains("Bearer "));
        assert!(!serialized.contains("cookie"));
        assert!(!serialized.contains("object_key"));
        assert!(!serialized.contains("objects/private"));
        assert!(!serialized.contains("http://"));
        assert!(!serialized.contains("https://"));
    }

    #[test]
    fn operator_handoff_summary_dry_run_reports_cleanup_review_without_auto_cleanup() {
        let summary = fashion_design_asset_import_operator_handoff_summary_dry_run(
            true, true, true, false, true, 4,
        );

        assert_eq!(summary["task_card_summary"]["status"], json!("executed"));
        assert_eq!(
            summary["task_card_summary"]["execute_state"],
            json!("executed_receipt_available")
        );
        assert_eq!(
            summary["task_card_summary"]["cleanup_state"],
            json!("cleanup_manifest_ready_for_review")
        );
        assert_eq!(
            summary["task_card_summary"]["next_operator_action"],
            json!("review_cleanup_manifest_before_any_cleanup")
        );
        assert_eq!(
            summary["operator_controls"]["cleanup_auto_allowed"],
            json!(false)
        );
        assert_eq!(
            summary["operator_controls"]["callback_allowed"],
            json!(false)
        );
        assert_eq!(
            summary["operator_controls"]["task_creation_allowed"],
            json!(false)
        );
        assert_eq!(
            summary["validation_summary"]["imported_asset_count"],
            json!(4)
        );
        assert_eq!(summary["production_write_allowed"], json!(false));
    }

    #[test]
    fn operator_handoff_contract_drift_guard_allows_detail_only_statuses() {
        let guard = fashion_design_asset_import_operator_handoff_contract_drift_guard_dry_run(
            "waiting_for_inputs",
            "blocked_missing_execute_inputs",
            "cleanup_waiting_for_live_receipt",
        );

        assert_eq!(
            guard["contract"],
            json!("fashion_design_asset_import_operator_handoff_contract_drift_guard_dry_run_v1")
        );
        assert_eq!(guard["surface"], json!("task_card_detail_panel_only"));
        assert_eq!(guard["no_write"], json!(true));
        assert_eq!(guard["no_delete"], json!(true));
        assert_eq!(guard["no_callback"], json!(true));
        assert_eq!(guard["no_task_creation"], json!(true));
        assert_eq!(
            guard["existing_task_card_status_enum_mutation_allowed"],
            json!(false)
        );
        assert_eq!(guard["new_task_card_creation_allowed"], json!(false));
        assert_eq!(guard["third_party_callback_allowed"], json!(false));
        assert_eq!(
            guard["validation"]["safe_for_task_card_detail"],
            json!(true)
        );
        assert_eq!(
            guard["field_contract"]["summary_must_not_replace_task_card_status"],
            json!(true)
        );
        assert_eq!(
            guard["operator_controls"]["task_status_mutation_allowed"],
            json!(false)
        );

        let serialized = serde_json::to_string(&guard).expect("guard should serialize");
        assert!(!serialized.contains("Bearer "));
        assert!(!serialized.contains("cookie"));
        assert!(!serialized.contains("object_key"));
        assert!(!serialized.contains("objects/private"));
        assert!(!serialized.contains("http://"));
        assert!(!serialized.contains("https://"));
    }

    #[test]
    fn operator_handoff_contract_drift_guard_rejects_unknown_detail_statuses() {
        let guard = fashion_design_asset_import_operator_handoff_contract_drift_guard_dry_run(
            "completed",
            "unexpected_execute_state",
            "unexpected_cleanup_state",
        );

        assert_eq!(guard["validation"]["detail_status_allowed"], json!(false));
        assert_eq!(guard["validation"]["execute_state_allowed"], json!(false));
        assert_eq!(guard["validation"]["cleanup_state_allowed"], json!(false));
        assert_eq!(
            guard["validation"]["safe_for_task_card_detail"],
            json!(false)
        );
        assert_eq!(guard["public_contract_changed"], json!(false));
        assert_eq!(guard["operator_controls"]["callback_allowed"], json!(false));
        assert_eq!(
            guard["operator_controls"]["task_creation_allowed"],
            json!(false)
        );
    }

    #[test]
    fn operator_readiness_rollup_dry_run_waits_for_execute_inputs_without_side_effects() {
        let execute_manifest = fashion_design_asset_import_live_execute_operator_manifest_dry_run(
            false, false, false, false, false, false,
        );
        let cleanup_manifest = fashion_design_asset_import_post_execute_cleanup_manifest_dry_run(
            false, false, 0, 0, 0, 0,
        );
        let guard = fashion_design_asset_import_operator_handoff_contract_drift_guard_dry_run(
            "waiting_for_inputs",
            "blocked_missing_execute_inputs",
            "cleanup_waiting_for_live_receipt",
        );
        let rollup = fashion_design_asset_import_operator_readiness_rollup_dry_run(
            &execute_manifest,
            &guard,
            &cleanup_manifest,
        );

        assert_eq!(
            rollup["contract"],
            json!("fashion_design_asset_import_operator_readiness_rollup_dry_run_v1")
        );
        assert_eq!(rollup["no_write"], json!(true));
        assert_eq!(rollup["no_delete"], json!(true));
        assert_eq!(rollup["no_callback"], json!(true));
        assert_eq!(rollup["no_task_creation"], json!(true));
        assert_eq!(rollup["inputs"]["input_contracts_ok"], json!(true));
        assert_eq!(
            rollup["readiness"]["overall_status"],
            json!("waiting_for_execute_inputs")
        );
        assert_eq!(
            rollup["readiness"]["ready_for_operator_execute"],
            json!(false)
        );
        assert_eq!(
            rollup["readiness"]["ready_for_operator_cleanup_review"],
            json!(false)
        );
        assert_eq!(
            rollup["release_gate_summary"]["safe_for_validation_receipt"],
            json!(true)
        );
        assert_eq!(
            rollup["operator_controls"]["callback_allowed"],
            json!(false)
        );
        assert_eq!(
            rollup["operator_controls"]["task_creation_allowed"],
            json!(false)
        );

        let serialized = serde_json::to_string(&rollup).expect("rollup should serialize");
        assert!(!serialized.contains("Bearer "));
        assert!(!serialized.contains("cookie"));
        assert!(!serialized.contains("object_key"));
        assert!(!serialized.contains("objects/private"));
        assert!(!serialized.contains("http://"));
        assert!(!serialized.contains("https://"));
    }

    #[test]
    fn operator_readiness_rollup_dry_run_marks_execute_review_ready() {
        let execute_manifest = fashion_design_asset_import_live_execute_operator_manifest_dry_run(
            true, true, true, true, true, true,
        );
        let cleanup_manifest = fashion_design_asset_import_post_execute_cleanup_manifest_dry_run(
            true, false, 0, 0, 0, 0,
        );
        let guard = fashion_design_asset_import_operator_handoff_contract_drift_guard_dry_run(
            "ready",
            "ready_for_controlled_execute",
            "cleanup_waiting_for_live_receipt",
        );
        let rollup = fashion_design_asset_import_operator_readiness_rollup_dry_run(
            &execute_manifest,
            &guard,
            &cleanup_manifest,
        );

        assert_eq!(
            rollup["readiness"]["overall_status"],
            json!("ready_for_execute_review")
        );
        assert_eq!(
            rollup["readiness"]["ready_for_operator_execute"],
            json!(true)
        );
        assert_eq!(
            rollup["readiness"]["ready_for_operator_cleanup_review"],
            json!(false)
        );
        assert_eq!(
            rollup["readiness"]["next_operator_action"],
            json!("review_rollup_and_manifest_then_run_controlled_execute")
        );
        assert_eq!(
            rollup["release_gate_summary"]["reviewed_execute_manifest_required"],
            json!(true)
        );
        assert_eq!(
            rollup["release_gate_summary"]["automatic_cleanup_allowed"],
            json!(false)
        );
        assert_eq!(rollup["production_write_allowed"], json!(false));
    }

    #[test]
    fn operator_readiness_rollup_dry_run_marks_cleanup_review_ready_without_auto_cleanup() {
        let execute_manifest = fashion_design_asset_import_live_execute_operator_manifest_dry_run(
            true, true, true, true, true, true,
        );
        let cleanup_manifest = fashion_design_asset_import_post_execute_cleanup_manifest_dry_run(
            true, true, 4, 4, 4, 4,
        );
        let guard = fashion_design_asset_import_operator_handoff_contract_drift_guard_dry_run(
            "executed",
            "executed_receipt_available",
            "cleanup_manifest_ready_for_review",
        );
        let rollup = fashion_design_asset_import_operator_readiness_rollup_dry_run(
            &execute_manifest,
            &guard,
            &cleanup_manifest,
        );

        assert_eq!(
            rollup["readiness"]["overall_status"],
            json!("ready_for_cleanup_review")
        );
        assert_eq!(
            rollup["readiness"]["ready_for_operator_execute"],
            json!(false)
        );
        assert_eq!(
            rollup["readiness"]["ready_for_operator_cleanup_review"],
            json!(true)
        );
        assert_eq!(rollup["inputs"]["live_receipt_supplied"], json!(true));
        assert_eq!(rollup["inputs"]["cleanup_manifest_ready"], json!(true));
        assert_eq!(
            rollup["operator_controls"]["cleanup_auto_allowed"],
            json!(false)
        );
        assert_eq!(
            rollup["release_gate_summary"]["record_counts_only"],
            json!(true)
        );
    }

    #[test]
    fn operator_readiness_contract_guard_allows_private_runbook_only_rollup() {
        let execute_manifest = fashion_design_asset_import_live_execute_operator_manifest_dry_run(
            false, false, false, false, false, false,
        );
        let cleanup_manifest = fashion_design_asset_import_post_execute_cleanup_manifest_dry_run(
            false, false, 0, 0, 0, 0,
        );
        let handoff_guard =
            fashion_design_asset_import_operator_handoff_contract_drift_guard_dry_run(
                "waiting_for_inputs",
                "blocked_missing_execute_inputs",
                "cleanup_waiting_for_live_receipt",
            );
        let rollup = fashion_design_asset_import_operator_readiness_rollup_dry_run(
            &execute_manifest,
            &handoff_guard,
            &cleanup_manifest,
        );
        let guard = fashion_design_asset_import_operator_readiness_contract_drift_guard_dry_run(
            &rollup, true, false,
        );

        assert_eq!(
            guard["contract"],
            json!("fashion_design_asset_import_operator_readiness_contract_drift_guard_dry_run_v1")
        );
        assert_eq!(
            guard["surface"],
            json!("private_operator_runbook_and_validation_only")
        );
        assert_eq!(guard["drift_status"], json!("safe_private_runbook_only"));
        assert_eq!(guard["no_write"], json!(true));
        assert_eq!(guard["no_delete"], json!(true));
        assert_eq!(guard["no_callback"], json!(true));
        assert_eq!(guard["no_task_creation"], json!(true));
        assert_eq!(guard["readiness_rollup_contract_ok"], json!(true));
        assert_eq!(guard["rollup_contains_command_material"], json!(false));
        assert_eq!(guard["command_template_redacted"], json!(true));
        assert_eq!(guard["public_docs_asset_imports_closed"], json!(true));
        assert_eq!(
            guard["validation"]["safe_for_shared_validation_receipt"],
            json!(true)
        );
        assert_eq!(
            guard["forbidden_public_material"]["raw_command_included"],
            json!(false)
        );
        assert_eq!(
            guard["forbidden_public_material"]["base_url_included"],
            json!(false)
        );
        assert_eq!(
            guard["operator_controls"]["private_runbook_only"],
            json!(true)
        );

        let serialized = serde_json::to_string(&guard).expect("guard should serialize");
        assert!(!serialized.contains("Bearer "));
        assert!(!serialized.contains("cookie"));
        assert!(!serialized.contains("object_key"));
        assert!(!serialized.contains("objects/private"));
        assert!(!serialized.contains("http://"));
        assert!(!serialized.contains("https://"));
    }

    #[test]
    fn operator_readiness_contract_guard_blocks_unredacted_command_template() {
        let execute_manifest = fashion_design_asset_import_live_execute_operator_manifest_dry_run(
            true, true, true, true, true, true,
        );
        let cleanup_manifest = fashion_design_asset_import_post_execute_cleanup_manifest_dry_run(
            true, false, 0, 0, 0, 0,
        );
        let handoff_guard =
            fashion_design_asset_import_operator_handoff_contract_drift_guard_dry_run(
                "ready",
                "ready_for_controlled_execute",
                "cleanup_waiting_for_live_receipt",
            );
        let rollup = fashion_design_asset_import_operator_readiness_rollup_dry_run(
            &execute_manifest,
            &handoff_guard,
            &cleanup_manifest,
        );
        let guard = fashion_design_asset_import_operator_readiness_contract_drift_guard_dry_run(
            &rollup, false, false,
        );

        assert_eq!(
            guard["drift_status"],
            json!("blocked_unredacted_command_template")
        );
        assert_eq!(guard["command_template_redacted"], json!(false));
        assert_eq!(
            guard["validation"]["safe_for_private_operator_review"],
            json!(false)
        );
        assert_eq!(
            guard["validation"]["safe_for_shared_validation_receipt"],
            json!(false)
        );
        assert_eq!(guard["operator_controls"]["callback_allowed"], json!(false));
        assert_eq!(
            guard["operator_controls"]["task_creation_allowed"],
            json!(false)
        );
    }

    #[test]
    fn operator_readiness_contract_guard_blocks_public_docs_asset_imports() {
        let execute_manifest = fashion_design_asset_import_live_execute_operator_manifest_dry_run(
            true, true, true, true, true, true,
        );
        let cleanup_manifest = fashion_design_asset_import_post_execute_cleanup_manifest_dry_run(
            true, false, 0, 0, 0, 0,
        );
        let handoff_guard =
            fashion_design_asset_import_operator_handoff_contract_drift_guard_dry_run(
                "ready",
                "ready_for_controlled_execute",
                "cleanup_waiting_for_live_receipt",
            );
        let rollup = fashion_design_asset_import_operator_readiness_rollup_dry_run(
            &execute_manifest,
            &handoff_guard,
            &cleanup_manifest,
        );
        let guard = fashion_design_asset_import_operator_readiness_contract_drift_guard_dry_run(
            &rollup, true, true,
        );

        assert_eq!(
            guard["drift_status"],
            json!("blocked_public_docs_expose_unreviewed_asset_imports")
        );
        assert_eq!(guard["public_docs_asset_imports_open"], json!(true));
        assert_eq!(guard["public_docs_asset_imports_closed"], json!(false));
        assert_eq!(guard["validation"]["safe_for_public_docs"], json!(false));
        assert_eq!(
            guard["validation"]["safe_for_shared_validation_receipt"],
            json!(false)
        );
        assert_eq!(guard["public_contract_changed"], json!(false));
    }

    #[test]
    fn private_runbook_receipt_package_splits_private_and_shared_receipts() {
        let execute_manifest = fashion_design_asset_import_live_execute_operator_manifest_dry_run(
            true, true, true, true, true, true,
        );
        let cleanup_manifest = fashion_design_asset_import_post_execute_cleanup_manifest_dry_run(
            true, false, 0, 0, 0, 0,
        );
        let handoff_guard =
            fashion_design_asset_import_operator_handoff_contract_drift_guard_dry_run(
                "ready",
                "ready_for_controlled_execute",
                "cleanup_waiting_for_live_receipt",
            );
        let rollup = fashion_design_asset_import_operator_readiness_rollup_dry_run(
            &execute_manifest,
            &handoff_guard,
            &cleanup_manifest,
        );
        let readiness_guard =
            fashion_design_asset_import_operator_readiness_contract_drift_guard_dry_run(
                &rollup, true, false,
            );
        let package =
            fashion_design_asset_import_live_execute_private_runbook_receipt_package_dry_run(
                &readiness_guard,
            );

        assert_eq!(
            package["contract"],
            json!(
                "fashion_design_asset_import_live_execute_private_runbook_receipt_package_dry_run_v1"
            )
        );
        assert_eq!(
            package["surface"],
            json!("private_runbook_and_shared_validation_receipt_split")
        );
        assert_eq!(
            package["package_status"],
            json!("ready_for_operator_review")
        );
        assert_eq!(package["no_write"], json!(true));
        assert_eq!(package["no_delete"], json!(true));
        assert_eq!(package["no_callback"], json!(true));
        assert_eq!(package["no_task_creation"], json!(true));
        assert_eq!(
            package["private_operator_runbook"]["included_in_shared_validation_receipt"],
            json!(false)
        );
        assert_eq!(
            package["private_operator_runbook"]["contains_raw_session"],
            json!(false)
        );
        assert_eq!(
            package["private_operator_runbook"]["contains_raw_dataset"],
            json!(false)
        );
        assert_eq!(
            package["private_operator_runbook"]["contains_raw_object_locator"],
            json!(false)
        );
        assert_eq!(
            package["shared_validation_receipt"]["contains_private_runbook"],
            json!(false)
        );
        assert_eq!(
            package["shared_validation_receipt"]["contains_command_template"],
            json!(false)
        );
        assert_eq!(
            package["shared_validation_receipt"]["contains_raw_session"],
            json!(false)
        );
        assert_eq!(
            package["shared_validation_receipt"]["contains_raw_asset_library"],
            json!(false)
        );
        assert_eq!(
            package["separation"]["private_runbook_separate_from_shared_receipt"],
            json!(true)
        );
        assert_eq!(
            package["validation"]["package_ready_for_review"],
            json!(true)
        );

        let serialized = serde_json::to_string(&package).expect("package should serialize");
        assert!(!serialized.contains("Bearer "));
        assert!(!serialized.contains("cookie"));
        assert!(!serialized.contains("object_key"));
        assert!(!serialized.contains("objects/private"));
        assert!(!serialized.contains("http://"));
        assert!(!serialized.contains("https://"));
    }

    #[test]
    fn private_runbook_receipt_package_blocks_guard_contract_mismatch() {
        let package =
            fashion_design_asset_import_live_execute_private_runbook_receipt_package_dry_run(
                &json!({
                    "contract": "unexpected",
                    "validation": {
                        "safe_for_shared_validation_receipt": true
                    },
                    "command_template_redacted": true,
                    "public_docs_asset_imports_closed": true,
                    "forbidden_public_material": {
                        "base_url_included": false,
                        "session_material_included": false,
                        "dataset_value_included": false,
                        "asset_library_value_included": false,
                        "approval_value_included": false,
                        "object_locator_value_included": false,
                        "executable_command_args_included": false,
                        "raw_command_included": false
                    },
                    "operator_controls": {
                        "private_runbook_only": true
                    }
                }),
            );

        assert_eq!(
            package["package_status"],
            json!("blocked_guard_contract_mismatch")
        );
        assert_eq!(
            package["validation"]["package_ready_for_review"],
            json!(false)
        );
        assert_eq!(
            package["shared_validation_receipt"]["contains_private_runbook"],
            json!(false)
        );
        assert_eq!(
            package["operator_controls"]["callback_allowed"],
            json!(false)
        );
    }

    #[test]
    fn private_runbook_receipt_package_blocks_public_docs_open_receipt() {
        let execute_manifest = fashion_design_asset_import_live_execute_operator_manifest_dry_run(
            true, true, true, true, true, true,
        );
        let cleanup_manifest = fashion_design_asset_import_post_execute_cleanup_manifest_dry_run(
            true, false, 0, 0, 0, 0,
        );
        let handoff_guard =
            fashion_design_asset_import_operator_handoff_contract_drift_guard_dry_run(
                "ready",
                "ready_for_controlled_execute",
                "cleanup_waiting_for_live_receipt",
            );
        let rollup = fashion_design_asset_import_operator_readiness_rollup_dry_run(
            &execute_manifest,
            &handoff_guard,
            &cleanup_manifest,
        );
        let readiness_guard =
            fashion_design_asset_import_operator_readiness_contract_drift_guard_dry_run(
                &rollup, true, true,
            );
        let package =
            fashion_design_asset_import_live_execute_private_runbook_receipt_package_dry_run(
                &readiness_guard,
            );

        assert_eq!(
            package["package_status"],
            json!("blocked_shared_validation_receipt_not_safe")
        );
        assert_eq!(
            package["validation"]["shared_validation_receipt_safe"],
            json!(false)
        );
        assert_eq!(
            package["shared_validation_receipt"]["public_docs_asset_imports_closed"],
            json!(false)
        );
        assert_eq!(
            package["separation"]["pre_execute_raw_material_excluded"],
            json!(true)
        );
        assert_eq!(package["public_contract_changed"], json!(false));
    }

    #[test]
    fn receipt_package_display_contract_guard_allows_detail_only_display() {
        let execute_manifest = fashion_design_asset_import_live_execute_operator_manifest_dry_run(
            true, true, true, true, true, true,
        );
        let cleanup_manifest = fashion_design_asset_import_post_execute_cleanup_manifest_dry_run(
            true, false, 0, 0, 0, 0,
        );
        let handoff_guard =
            fashion_design_asset_import_operator_handoff_contract_drift_guard_dry_run(
                "ready",
                "ready_for_controlled_execute",
                "cleanup_waiting_for_live_receipt",
            );
        let rollup = fashion_design_asset_import_operator_readiness_rollup_dry_run(
            &execute_manifest,
            &handoff_guard,
            &cleanup_manifest,
        );
        let readiness_guard =
            fashion_design_asset_import_operator_readiness_contract_drift_guard_dry_run(
                &rollup, true, false,
            );
        let package =
            fashion_design_asset_import_live_execute_private_runbook_receipt_package_dry_run(
                &readiness_guard,
            );
        let guard =
            fashion_design_asset_import_receipt_package_display_contract_drift_guard_dry_run(
                &package,
            );

        assert_eq!(
            guard["contract"],
            json!(
                "fashion_design_asset_import_receipt_package_display_contract_drift_guard_dry_run_v1"
            )
        );
        assert_eq!(
            guard["surface"],
            json!("task_card_detail_and_validation_receipt_only")
        );
        assert_eq!(guard["display_status"], json!("ready_for_detail_display"));
        assert_eq!(guard["no_write"], json!(true));
        assert_eq!(guard["no_delete"], json!(true));
        assert_eq!(guard["no_callback"], json!(true));
        assert_eq!(guard["no_task_creation"], json!(true));
        assert_eq!(
            guard["private_runbook_hidden_from_shared_receipt"],
            json!(true)
        );
        assert_eq!(
            guard["command_template_excluded_from_shared_receipt"],
            json!(true)
        );
        assert_eq!(
            guard["forbidden_display_behaviors"]["creates_task_card"],
            json!(false)
        );
        assert_eq!(
            guard["forbidden_display_behaviors"]["mutates_task_card_status_enum"],
            json!(false)
        );
        assert_eq!(
            guard["validation"]["safe_for_task_card_detail"],
            json!(true)
        );
        assert_eq!(
            guard["operator_controls"]["detail_display_only"],
            json!(true)
        );

        let serialized = serde_json::to_string(&guard).expect("guard should serialize");
        assert!(!serialized.contains("Bearer "));
        assert!(!serialized.contains("cookie"));
        assert!(!serialized.contains("object_key"));
        assert!(!serialized.contains("objects/private"));
        assert!(!serialized.contains("http://"));
        assert!(!serialized.contains("https://"));
    }

    #[test]
    fn receipt_package_display_contract_guard_blocks_package_contract_mismatch() {
        let guard =
            fashion_design_asset_import_receipt_package_display_contract_drift_guard_dry_run(
                &json!({
                    "contract": "unexpected",
                    "validation": {
                        "package_ready_for_review": true,
                        "shared_validation_receipt_safe": true
                    },
                    "separation": {
                        "private_runbook_separate_from_shared_receipt": true
                    },
                    "shared_validation_receipt": {
                        "contains_private_runbook": false,
                        "contains_command_template": false
                    },
                    "private_operator_runbook": {
                        "included_in_shared_validation_receipt": false
                    },
                    "no_write": true,
                    "no_delete": true,
                    "no_callback": true,
                    "no_task_creation": true,
                    "public_contract_changed": false,
                    "production_write_allowed": false
                }),
            );

        assert_eq!(
            guard["display_status"],
            json!("blocked_package_contract_mismatch")
        );
        assert_eq!(
            guard["validation"]["safe_for_task_card_detail"],
            json!(false)
        );
        assert_eq!(
            guard["forbidden_display_behaviors"]["creates_task_card"],
            json!(false)
        );
        assert_eq!(guard["operator_controls"]["callback_allowed"], json!(false));
    }

    #[test]
    fn receipt_package_display_contract_guard_blocks_shared_command_template_leak() {
        let execute_manifest = fashion_design_asset_import_live_execute_operator_manifest_dry_run(
            true, true, true, true, true, true,
        );
        let cleanup_manifest = fashion_design_asset_import_post_execute_cleanup_manifest_dry_run(
            true, false, 0, 0, 0, 0,
        );
        let handoff_guard =
            fashion_design_asset_import_operator_handoff_contract_drift_guard_dry_run(
                "ready",
                "ready_for_controlled_execute",
                "cleanup_waiting_for_live_receipt",
            );
        let rollup = fashion_design_asset_import_operator_readiness_rollup_dry_run(
            &execute_manifest,
            &handoff_guard,
            &cleanup_manifest,
        );
        let readiness_guard =
            fashion_design_asset_import_operator_readiness_contract_drift_guard_dry_run(
                &rollup, true, false,
            );
        let mut package =
            fashion_design_asset_import_live_execute_private_runbook_receipt_package_dry_run(
                &readiness_guard,
            );
        package["shared_validation_receipt"]["contains_command_template"] = json!(true);
        let guard =
            fashion_design_asset_import_receipt_package_display_contract_drift_guard_dry_run(
                &package,
            );

        assert_eq!(
            guard["display_status"],
            json!("blocked_command_template_shared_receipt_leak")
        );
        assert_eq!(
            guard["command_template_excluded_from_shared_receipt"],
            json!(false)
        );
        assert_eq!(
            guard["validation"]["safe_for_validation_ledger"],
            json!(false)
        );
        assert_eq!(guard["public_contract_changed"], json!(false));
    }

    #[test]
    fn operator_validation_rollup_allows_single_ready_gate() {
        let execute_manifest = fashion_design_asset_import_live_execute_operator_manifest_dry_run(
            true, true, true, true, true, true,
        );
        let cleanup_manifest = fashion_design_asset_import_post_execute_cleanup_manifest_dry_run(
            true, false, 0, 0, 0, 0,
        );
        let handoff_guard =
            fashion_design_asset_import_operator_handoff_contract_drift_guard_dry_run(
                "ready",
                "ready_for_controlled_execute",
                "cleanup_waiting_for_live_receipt",
            );
        let readiness_rollup = fashion_design_asset_import_operator_readiness_rollup_dry_run(
            &execute_manifest,
            &handoff_guard,
            &cleanup_manifest,
        );
        let readiness_guard =
            fashion_design_asset_import_operator_readiness_contract_drift_guard_dry_run(
                &readiness_rollup,
                true,
                false,
            );
        let receipt_package =
            fashion_design_asset_import_live_execute_private_runbook_receipt_package_dry_run(
                &readiness_guard,
            );
        let display_guard =
            fashion_design_asset_import_receipt_package_display_contract_drift_guard_dry_run(
                &receipt_package,
            );
        let rollup = fashion_design_asset_import_operator_validation_rollup_dry_run(
            &readiness_guard,
            &receipt_package,
            &display_guard,
        );

        assert_eq!(
            rollup["contract"],
            json!("fashion_design_asset_import_operator_validation_rollup_dry_run_v1")
        );
        assert_eq!(
            rollup["surface"],
            json!("operator_validation_release_gate_summary")
        );
        assert_eq!(
            rollup["rollup_status"],
            json!("ready_for_operator_validation_review")
        );
        assert_eq!(rollup["validation_rollup_ready"], json!(true));
        assert_eq!(rollup["release_blocker_count"], json!(0));
        assert_eq!(rollup["inputs"]["readiness_guard_ready"], json!(true));
        assert_eq!(rollup["inputs"]["receipt_package_ready"], json!(true));
        assert_eq!(rollup["inputs"]["display_guard_ready"], json!(true));
        assert_eq!(rollup["inputs"]["side_effect_guards_ok"], json!(true));
        assert_eq!(
            rollup["release_gate_summary"]["single_operator_gate_ready"],
            json!(true)
        );
        assert_eq!(
            rollup["release_gate_summary"]["ready_for_live_execute"],
            json!(false)
        );
        assert_eq!(
            rollup["release_gate_summary"]["live_execute_still_requires_private_inputs"],
            json!(true)
        );
        assert_eq!(
            rollup["forbidden_release_material"]["private_runbook_in_shared_receipt"],
            json!(false)
        );
        assert_eq!(
            rollup["forbidden_release_material"]["command_template_in_shared_receipt"],
            json!(false)
        );
        assert_eq!(
            rollup["operator_controls"]["task_creation_allowed"],
            json!(false)
        );

        let serialized = serde_json::to_string(&rollup).expect("rollup should serialize");
        assert!(!serialized.contains("Bearer "));
        assert!(!serialized.contains("cookie"));
        assert!(!serialized.contains("object_key"));
        assert!(!serialized.contains("objects/private"));
        assert!(!serialized.contains("http://"));
        assert!(!serialized.contains("https://"));
    }

    #[test]
    fn operator_validation_rollup_blocks_readiness_guard_failure() {
        let receipt_package =
            fashion_design_asset_import_live_execute_private_runbook_receipt_package_dry_run(
                &json!({
                    "contract": "fashion_design_asset_import_operator_readiness_contract_drift_guard_dry_run_v1",
                    "validation": {
                        "safe_for_shared_validation_receipt": true
                    },
                    "command_template_redacted": true,
                    "public_docs_asset_imports_closed": true,
                    "forbidden_public_material": {
                        "base_url_included": false,
                        "session_material_included": false,
                        "dataset_value_included": false,
                        "asset_library_value_included": false,
                        "approval_value_included": false,
                        "object_locator_value_included": false,
                        "executable_command_args_included": false,
                        "raw_command_included": false
                    },
                    "operator_controls": {
                        "private_runbook_only": true
                    }
                }),
            );
        let display_guard =
            fashion_design_asset_import_receipt_package_display_contract_drift_guard_dry_run(
                &receipt_package,
            );
        let bad_readiness_guard = json!({
            "contract": "unexpected",
            "validation": {
                "safe_for_shared_validation_receipt": true
            },
            "operator_controls": {
                "private_runbook_only": true
            },
            "command_template_redacted": true,
            "public_docs_asset_imports_closed": true,
            "no_write": true,
            "no_delete": true,
            "no_callback": true,
            "no_task_creation": true,
            "public_contract_changed": false,
            "production_write_allowed": false
        });
        let rollup = fashion_design_asset_import_operator_validation_rollup_dry_run(
            &bad_readiness_guard,
            &receipt_package,
            &display_guard,
        );

        assert_eq!(
            rollup["rollup_status"],
            json!("blocked_readiness_contract_guard")
        );
        assert_eq!(rollup["validation_rollup_ready"], json!(false));
        assert_eq!(
            rollup["inputs"]["readiness_guard_contract_ok"],
            json!(false)
        );
        assert_eq!(rollup["release_blocker_count"], json!(1));
        assert_eq!(
            rollup["operator_controls"]["callback_allowed"],
            json!(false)
        );
    }

    #[test]
    fn operator_validation_rollup_blocks_display_guard_failure() {
        let execute_manifest = fashion_design_asset_import_live_execute_operator_manifest_dry_run(
            true, true, true, true, true, true,
        );
        let cleanup_manifest = fashion_design_asset_import_post_execute_cleanup_manifest_dry_run(
            true, false, 0, 0, 0, 0,
        );
        let handoff_guard =
            fashion_design_asset_import_operator_handoff_contract_drift_guard_dry_run(
                "ready",
                "ready_for_controlled_execute",
                "cleanup_waiting_for_live_receipt",
            );
        let readiness_rollup = fashion_design_asset_import_operator_readiness_rollup_dry_run(
            &execute_manifest,
            &handoff_guard,
            &cleanup_manifest,
        );
        let readiness_guard =
            fashion_design_asset_import_operator_readiness_contract_drift_guard_dry_run(
                &readiness_rollup,
                true,
                false,
            );
        let receipt_package =
            fashion_design_asset_import_live_execute_private_runbook_receipt_package_dry_run(
                &readiness_guard,
            );
        let mut display_guard =
            fashion_design_asset_import_receipt_package_display_contract_drift_guard_dry_run(
                &receipt_package,
            );
        display_guard["forbidden_display_behaviors"]["creates_task_card"] = json!(true);
        let rollup = fashion_design_asset_import_operator_validation_rollup_dry_run(
            &readiness_guard,
            &receipt_package,
            &display_guard,
        );

        assert_eq!(
            rollup["rollup_status"],
            json!("blocked_display_contract_guard")
        );
        assert_eq!(rollup["validation_rollup_ready"], json!(false));
        assert_eq!(rollup["inputs"]["display_guard_ready"], json!(false));
        assert_eq!(rollup["release_blocker_count"], json!(1));
        assert_eq!(
            rollup["forbidden_release_material"]["base_url_included"],
            json!(false)
        );
    }

    fn validation_rollup_for_display_guard_tests() -> Value {
        let execute_manifest = fashion_design_asset_import_live_execute_operator_manifest_dry_run(
            true, true, true, true, true, true,
        );
        let cleanup_manifest = fashion_design_asset_import_post_execute_cleanup_manifest_dry_run(
            true, false, 0, 0, 0, 0,
        );
        let handoff_guard =
            fashion_design_asset_import_operator_handoff_contract_drift_guard_dry_run(
                "ready",
                "ready_for_controlled_execute",
                "cleanup_waiting_for_live_receipt",
            );
        let readiness_rollup = fashion_design_asset_import_operator_readiness_rollup_dry_run(
            &execute_manifest,
            &handoff_guard,
            &cleanup_manifest,
        );
        let readiness_guard =
            fashion_design_asset_import_operator_readiness_contract_drift_guard_dry_run(
                &readiness_rollup,
                true,
                false,
            );
        let receipt_package =
            fashion_design_asset_import_live_execute_private_runbook_receipt_package_dry_run(
                &readiness_guard,
            );
        let display_guard =
            fashion_design_asset_import_receipt_package_display_contract_drift_guard_dry_run(
                &receipt_package,
            );
        fashion_design_asset_import_operator_validation_rollup_dry_run(
            &readiness_guard,
            &receipt_package,
            &display_guard,
        )
    }

    #[test]
    fn operator_validation_rollup_display_contract_guard_allows_detail_only_gate() {
        let validation_rollup = validation_rollup_for_display_guard_tests();
        let guard =
            fashion_design_asset_import_operator_validation_rollup_display_contract_guard_dry_run(
                &validation_rollup,
            );

        assert_eq!(
            guard["contract"],
            json!(
                "fashion_design_asset_import_operator_validation_rollup_display_contract_guard_dry_run_v1"
            )
        );
        assert_eq!(
            guard["surface"],
            json!("validation_rollup_detail_and_markdown_only")
        );
        assert_eq!(
            guard["display_status"],
            json!("ready_for_validation_gate_display")
        );
        assert_eq!(guard["no_write"], json!(true));
        assert_eq!(guard["no_delete"], json!(true));
        assert_eq!(guard["no_callback"], json!(true));
        assert_eq!(guard["no_task_creation"], json!(true));
        assert_eq!(guard["not_live_execute_ready"], json!(true));
        assert_eq!(guard["live_private_inputs_required"], json!(true));
        assert_eq!(
            guard["forbidden_display_behaviors"]["auto_triggers_live_execute"],
            json!(false)
        );
        assert_eq!(
            guard["forbidden_display_behaviors"]["creates_task_card"],
            json!(false)
        );
        assert_eq!(
            guard["forbidden_display_behaviors"]["updates_task_card"],
            json!(false)
        );
        assert_eq!(
            guard["forbidden_display_behaviors"]["includes_private_runbook"],
            json!(false)
        );
        assert_eq!(
            guard["forbidden_display_behaviors"]["includes_command_template"],
            json!(false)
        );
        assert_eq!(
            guard["forbidden_display_behaviors"]["includes_raw_input_requirements"],
            json!(false)
        );
        assert_eq!(
            guard["validation"]["safe_for_task_card_detail"],
            json!(true)
        );
        assert_eq!(
            guard["validation"]["safe_for_validation_ledger"],
            json!(true)
        );
        assert_eq!(
            guard["validation"]["safe_for_markdown_summary"],
            json!(true)
        );

        let serialized = serde_json::to_string(&guard).expect("guard should serialize");
        assert!(!serialized.contains("Bearer "));
        assert!(!serialized.contains("cookie"));
        assert!(!serialized.contains("object_key"));
        assert!(!serialized.contains("objects/private"));
        assert!(!serialized.contains("http://"));
        assert!(!serialized.contains("https://"));
    }

    #[test]
    fn operator_validation_rollup_display_contract_guard_blocks_live_execute_confusion() {
        let mut validation_rollup = validation_rollup_for_display_guard_tests();
        validation_rollup["release_gate_summary"]["ready_for_live_execute"] = json!(true);
        let guard =
            fashion_design_asset_import_operator_validation_rollup_display_contract_guard_dry_run(
                &validation_rollup,
            );

        assert_eq!(
            guard["display_status"],
            json!("blocked_live_execute_gate_confusion")
        );
        assert_eq!(guard["not_live_execute_ready"], json!(false));
        assert_eq!(
            guard["validation"]["safe_for_validation_ledger"],
            json!(false)
        );
        assert_eq!(
            guard["forbidden_display_behaviors"]["auto_triggers_live_execute"],
            json!(false)
        );
        assert_eq!(guard["operator_controls"]["callback_allowed"], json!(false));
    }

    #[test]
    fn operator_validation_rollup_display_contract_guard_blocks_private_material_leak() {
        let mut validation_rollup = validation_rollup_for_display_guard_tests();
        validation_rollup["forbidden_release_material"]["private_runbook_in_shared_receipt"] =
            json!(true);
        let guard =
            fashion_design_asset_import_operator_validation_rollup_display_contract_guard_dry_run(
                &validation_rollup,
            );

        assert_eq!(
            guard["display_status"],
            json!("blocked_private_material_leak")
        );
        assert_eq!(guard["no_private_material"], json!(false));
        assert_eq!(
            guard["validation"]["safe_for_markdown_summary"],
            json!(false)
        );
        assert_eq!(
            guard["forbidden_display_behaviors"]["includes_private_runbook"],
            json!(false)
        );
        assert_eq!(guard["public_contract_changed"], json!(false));
    }

    #[test]
    fn operator_validation_release_summary_allows_private_input_review_gate() {
        let execute_manifest = fashion_design_asset_import_live_execute_operator_manifest_dry_run(
            false, false, false, false, false, false,
        );
        let validation_rollup = validation_rollup_for_display_guard_tests();
        let display_guard =
            fashion_design_asset_import_operator_validation_rollup_display_contract_guard_dry_run(
                &validation_rollup,
            );
        let summary = fashion_design_asset_import_operator_validation_release_summary_dry_run(
            &execute_manifest,
            &display_guard,
        );

        assert_eq!(
            summary["contract"],
            json!("fashion_design_asset_import_operator_validation_release_summary_dry_run_v1")
        );
        assert_eq!(
            summary["surface"],
            json!("operator_execute_preflight_release_summary")
        );
        assert_eq!(
            summary["release_status"],
            json!("ready_for_private_operator_input_review")
        );
        assert_eq!(
            summary["release_ready_for_operator_private_input_injection"],
            json!(true)
        );
        assert_eq!(summary["ready_for_live_execute"], json!(false));
        assert_eq!(
            summary["live_execute_still_requires_private_values"],
            json!(true)
        );
        assert_eq!(
            summary["missing_private_inputs"].as_array().unwrap().len(),
            5
        );
        assert_eq!(
            summary["unchanged_contracts"]["public_third_party_contract"],
            json!(true)
        );
        assert_eq!(
            summary["forbidden_release_behaviors"]["auto_execute"],
            json!(false)
        );
        assert_eq!(
            summary["operator_controls"]["manual_private_input_injection_required"],
            json!(true)
        );

        let serialized = serde_json::to_string(&summary).expect("summary should serialize");
        assert!(!serialized.contains("Bearer "));
        assert!(!serialized.contains("cookie"));
        assert!(!serialized.contains("object_key"));
        assert!(!serialized.contains("objects/private"));
        assert!(!serialized.contains("http://"));
        assert!(!serialized.contains("https://"));
    }

    #[test]
    fn operator_validation_release_summary_blocks_display_guard_failure() {
        let execute_manifest = fashion_design_asset_import_live_execute_operator_manifest_dry_run(
            false, false, false, false, false, false,
        );
        let validation_rollup = validation_rollup_for_display_guard_tests();
        let mut display_guard =
            fashion_design_asset_import_operator_validation_rollup_display_contract_guard_dry_run(
                &validation_rollup,
            );
        display_guard["display_status"] = json!("blocked_validation_rollup_not_ready");
        display_guard["validation"]["safe_for_validation_ledger"] = json!(false);
        let summary = fashion_design_asset_import_operator_validation_release_summary_dry_run(
            &execute_manifest,
            &display_guard,
        );

        assert_eq!(
            summary["release_status"],
            json!("blocked_validation_display_guard")
        );
        assert_eq!(summary["display_guard_ready"], json!(false));
        assert_eq!(
            summary["validation"]["safe_for_validation_ledger"],
            json!(false)
        );
        assert_eq!(summary["ready_for_live_execute"], json!(false));
    }

    #[test]
    fn operator_validation_release_summary_blocks_task_card_contract_drift() {
        let execute_manifest = fashion_design_asset_import_live_execute_operator_manifest_dry_run(
            true, true, true, true, true, true,
        );
        let validation_rollup = validation_rollup_for_display_guard_tests();
        let mut display_guard =
            fashion_design_asset_import_operator_validation_rollup_display_contract_guard_dry_run(
                &validation_rollup,
            );
        display_guard["forbidden_display_behaviors"]["creates_task_card"] = json!(true);
        let summary = fashion_design_asset_import_operator_validation_release_summary_dry_run(
            &execute_manifest,
            &display_guard,
        );

        assert_eq!(
            summary["release_status"],
            json!("blocked_task_card_contract_drift")
        );
        assert_eq!(
            summary["unchanged_contracts"]["task_card_status_enum"],
            json!(false)
        );
        assert_eq!(
            summary["forbidden_release_behaviors"]["task_card_create"],
            json!(false)
        );
        assert_eq!(
            summary["operator_controls"]["task_creation_allowed"],
            json!(false)
        );
    }

    fn release_summary_for_field_ledger_guard_tests() -> Value {
        let execute_manifest = fashion_design_asset_import_live_execute_operator_manifest_dry_run(
            false, false, false, false, false, false,
        );
        let validation_rollup = validation_rollup_for_display_guard_tests();
        let display_guard =
            fashion_design_asset_import_operator_validation_rollup_display_contract_guard_dry_run(
                &validation_rollup,
            );
        fashion_design_asset_import_operator_validation_release_summary_dry_run(
            &execute_manifest,
            &display_guard,
        )
    }

    #[test]
    fn operator_validation_release_summary_field_ledger_guard_allows_detail_only_fields() {
        let release_summary = release_summary_for_field_ledger_guard_tests();
        let guard =
            fashion_design_asset_import_operator_validation_release_summary_field_ledger_guard_dry_run(
                &release_summary,
            );

        assert_eq!(
            guard["contract"],
            json!(
                "fashion_design_asset_import_operator_validation_release_summary_field_ledger_guard_dry_run_v1"
            )
        );
        assert_eq!(
            guard["surface"],
            json!("task_detail_validation_ledger_field_allowlist")
        );
        assert_eq!(
            guard["ledger_status"],
            json!("ready_for_release_summary_field_ledger")
        );
        assert_eq!(guard["release_summary_ready"], json!(true));
        assert_eq!(guard["detail_display_only"], json!(true));
        assert_eq!(guard["no_live_execute"], json!(true));
        assert_eq!(guard["task_card_contract_stable"], json!(true));
        assert_eq!(guard["shared_validation_receipt_stable"], json!(true));
        assert_eq!(
            guard["validation"]["safe_for_task_card_detail"],
            json!(true)
        );
        assert_eq!(
            guard["forbidden_field_material"]["private_input_values"],
            json!(false)
        );
        assert_eq!(
            guard["forbidden_ledger_behaviors"]["creates_task_card"],
            json!(false)
        );
        assert_eq!(guard["operator_controls"]["field_ledger_only"], json!(true));

        let serialized = serde_json::to_string(&guard).expect("guard should serialize");
        assert!(serialized.contains("release_status"));
        assert!(!serialized.contains("Bearer "));
        assert!(!serialized.contains("cookie"));
        assert!(!serialized.contains("object_key"));
        assert!(!serialized.contains("objects/private"));
        assert!(!serialized.contains("http://"));
        assert!(!serialized.contains("https://"));
    }

    #[test]
    fn operator_validation_release_summary_field_ledger_guard_blocks_live_execute_risk() {
        let mut release_summary = release_summary_for_field_ledger_guard_tests();
        release_summary["ready_for_live_execute"] = json!(true);
        let guard =
            fashion_design_asset_import_operator_validation_release_summary_field_ledger_guard_dry_run(
                &release_summary,
            );

        assert_eq!(
            guard["ledger_status"],
            json!("blocked_live_execute_or_side_effect_risk")
        );
        assert_eq!(guard["no_live_execute"], json!(false));
        assert_eq!(
            guard["validation"]["safe_for_validation_ledger"],
            json!(false)
        );
        assert_eq!(
            guard["operator_controls"]["live_execute_allowed"],
            json!(false)
        );
    }

    #[test]
    fn operator_validation_release_summary_field_ledger_guard_blocks_shared_receipt_drift() {
        let mut release_summary = release_summary_for_field_ledger_guard_tests();
        release_summary["unchanged_contracts"]["shared_validation_receipt"] = json!(false);
        let guard =
            fashion_design_asset_import_operator_validation_release_summary_field_ledger_guard_dry_run(
                &release_summary,
            );

        assert_eq!(
            guard["ledger_status"],
            json!("blocked_shared_validation_receipt_drift")
        );
        assert_eq!(guard["shared_validation_receipt_stable"], json!(false));
        assert_eq!(
            guard["forbidden_ledger_behaviors"]["changes_shared_validation_receipt"],
            json!(false)
        );
        assert_eq!(guard["public_contract_changed"], json!(false));
    }

    fn operator_ready_handoff_receipt_inputs_for_tests() -> (Value, Value, Value) {
        let execute_manifest = fashion_design_asset_import_live_execute_operator_manifest_dry_run(
            false, false, false, false, false, false,
        );
        let validation_rollup = validation_rollup_for_display_guard_tests();
        let display_guard =
            fashion_design_asset_import_operator_validation_rollup_display_contract_guard_dry_run(
                &validation_rollup,
            );
        let release_summary =
            fashion_design_asset_import_operator_validation_release_summary_dry_run(
                &execute_manifest,
                &display_guard,
            );
        let field_ledger_guard =
            fashion_design_asset_import_operator_validation_release_summary_field_ledger_guard_dry_run(
                &release_summary,
            );
        (execute_manifest, release_summary, field_ledger_guard)
    }

    #[test]
    fn operator_ready_handoff_receipt_allows_private_value_handoff_only() {
        let (execute_manifest, release_summary, field_ledger_guard) =
            operator_ready_handoff_receipt_inputs_for_tests();
        let receipt = fashion_design_asset_import_operator_ready_handoff_receipt_dry_run(
            &execute_manifest,
            &release_summary,
            &field_ledger_guard,
        );

        assert_eq!(
            receipt["contract"],
            json!("fashion_design_asset_import_operator_ready_handoff_receipt_dry_run_v1")
        );
        assert_eq!(
            receipt["surface"],
            json!("operator_ready_private_execute_handoff_receipt")
        );
        assert_eq!(
            receipt["handoff_status"],
            json!("ready_for_operator_private_values")
        );
        assert_eq!(
            receipt["handoff_ready_for_operator_private_values"],
            json!(true)
        );
        assert_eq!(receipt["ready_for_live_execute"], json!(false));
        assert_eq!(
            receipt["live_execute_still_requires_private_values"],
            json!(true)
        );
        assert_eq!(
            receipt["operator_next_step"]["private_value_injection_required"],
            json!(true)
        );
        assert_eq!(
            receipt["operator_next_step"]["public_contract_change_required"],
            json!(false)
        );
        assert_eq!(
            receipt["forbidden_handoff_behaviors"]["auto_execute"],
            json!(false)
        );
        assert_eq!(
            receipt["forbidden_handoff_behaviors"]["changes_main_site_display_fields"],
            json!(false)
        );
        assert_eq!(
            receipt["unchanged_contracts"]["main_site_display_fields"],
            json!(true)
        );
        assert_eq!(
            receipt["validation"]["safe_for_operator_handoff_receipt"],
            json!(true)
        );
        assert_eq!(
            receipt["operator_private_input_labels"]
                .as_array()
                .expect("labels should be an array")
                .len(),
            5
        );

        let serialized = serde_json::to_string(&receipt).expect("receipt should serialize");
        assert!(!serialized.contains("Bearer "));
        assert!(!serialized.contains("cookie"));
        assert!(!serialized.contains("object_key"));
        assert!(!serialized.contains("objects/private"));
        assert!(!serialized.contains("http://"));
        assert!(!serialized.contains("https://"));
    }

    #[test]
    fn operator_ready_handoff_receipt_blocks_field_ledger_not_ready() {
        let (execute_manifest, release_summary, mut field_ledger_guard) =
            operator_ready_handoff_receipt_inputs_for_tests();
        field_ledger_guard["ledger_status"] = json!("blocked_task_card_contract_drift");
        field_ledger_guard["validation"]["safe_for_validation_ledger"] = json!(false);
        let receipt = fashion_design_asset_import_operator_ready_handoff_receipt_dry_run(
            &execute_manifest,
            &release_summary,
            &field_ledger_guard,
        );

        assert_eq!(
            receipt["handoff_status"],
            json!("blocked_field_ledger_not_ready")
        );
        assert_eq!(receipt["field_ledger_ready"], json!(false));
        assert_eq!(
            receipt["validation"]["safe_for_validation_ledger"],
            json!(false)
        );
        assert_eq!(receipt["ready_for_live_execute"], json!(false));
    }

    #[test]
    fn operator_ready_handoff_receipt_blocks_manifest_review_gate_risk() {
        let (mut execute_manifest, release_summary, field_ledger_guard) =
            operator_ready_handoff_receipt_inputs_for_tests();
        execute_manifest["review_gates"]["manifest_must_be_reviewed_before_execute"] = json!(false);
        let receipt = fashion_design_asset_import_operator_ready_handoff_receipt_dry_run(
            &execute_manifest,
            &release_summary,
            &field_ledger_guard,
        );

        assert_eq!(
            receipt["handoff_status"],
            json!("blocked_execute_manifest_review_gate")
        );
        assert_eq!(receipt["manifest_review_gate_ready"], json!(false));
        assert_eq!(
            receipt["operator_controls"]["controlled_execute_after_review_only"],
            json!(true)
        );
        assert_eq!(
            receipt["forbidden_handoff_behaviors"]["includes_private_input_values"],
            json!(false)
        );
    }

    fn final_validation_readiness_bundle_receipt_for_tests() -> Value {
        let (execute_manifest, release_summary, field_ledger_guard) =
            operator_ready_handoff_receipt_inputs_for_tests();
        fashion_design_asset_import_operator_ready_handoff_receipt_dry_run(
            &execute_manifest,
            &release_summary,
            &field_ledger_guard,
        )
    }

    #[test]
    fn final_validation_readiness_bundle_allows_single_review_gate() {
        let receipt = final_validation_readiness_bundle_receipt_for_tests();
        let bundle =
            fashion_design_asset_import_final_validation_readiness_bundle_dry_run(&receipt);

        assert_eq!(
            bundle["contract"],
            json!("fashion_design_asset_import_final_validation_readiness_bundle_dry_run_v1")
        );
        assert_eq!(
            bundle["surface"],
            json!("final_validation_readiness_bundle")
        );
        assert_eq!(
            bundle["bundle_status"],
            json!("ready_for_operator_review_bundle")
        );
        assert_eq!(bundle["bundle_ready_for_operator_review"], json!(true));
        assert_eq!(bundle["release_blocker_count"], json!(0));
        assert_eq!(bundle["ready_for_live_execute"], json!(false));
        assert_eq!(
            bundle["live_execute_still_requires_private_values"],
            json!(true)
        );
        assert_eq!(
            bundle["validation_readiness"]["single_gate_ready"],
            json!(true)
        );
        assert_eq!(
            bundle["operator_next_step"]["final_bundle_review_required"],
            json!(true)
        );
        assert_eq!(
            bundle["forbidden_bundle_behaviors"]["auto_execute"],
            json!(false)
        );
        assert_eq!(
            bundle["unchanged_contracts"]["main_site_display_fields"],
            json!(true)
        );
        assert_eq!(
            bundle["included_sections"]
                .as_array()
                .expect("sections should be an array")
                .len(),
            4
        );

        let serialized = serde_json::to_string(&bundle).expect("bundle should serialize");
        assert!(!serialized.contains("Bearer "));
        assert!(!serialized.contains("cookie"));
        assert!(!serialized.contains("object_key"));
        assert!(!serialized.contains("objects/private"));
        assert!(!serialized.contains("http://"));
        assert!(!serialized.contains("https://"));
    }

    #[test]
    fn final_validation_readiness_bundle_blocks_handoff_not_ready() {
        let mut receipt = final_validation_readiness_bundle_receipt_for_tests();
        receipt["handoff_status"] = json!("blocked_field_ledger_not_ready");
        receipt["handoff_ready_for_operator_private_values"] = json!(false);
        let bundle =
            fashion_design_asset_import_final_validation_readiness_bundle_dry_run(&receipt);

        assert_eq!(bundle["bundle_status"], json!("blocked_handoff_not_ready"));
        assert_eq!(bundle["bundle_ready_for_operator_review"], json!(false));
        assert_eq!(bundle["release_blocker_count"], json!(1));
        assert_eq!(
            bundle["validation"]["safe_for_final_validation_bundle"],
            json!(false)
        );
        assert_eq!(bundle["ready_for_live_execute"], json!(false));
    }

    #[test]
    fn final_validation_readiness_bundle_blocks_task_card_mutation_drift() {
        let mut receipt = final_validation_readiness_bundle_receipt_for_tests();
        receipt["forbidden_handoff_behaviors"]["creates_task_card"] = json!(true);
        let bundle =
            fashion_design_asset_import_final_validation_readiness_bundle_dry_run(&receipt);

        assert_eq!(bundle["bundle_status"], json!("blocked_mutation_guard"));
        assert_eq!(
            bundle["validation_readiness"]["no_task_card_or_shared_receipt_mutation"],
            json!(false)
        );
        assert_eq!(
            bundle["operator_controls"]["task_creation_allowed"],
            json!(false)
        );
        assert_eq!(
            bundle["forbidden_bundle_behaviors"]["creates_task_card"],
            json!(false)
        );
    }

    #[test]
    fn asset_retrieval_evidence_writer_readiness_allows_review_only_write_plan() {
        let readiness = fashion_design_asset_retrieval_evidence_live_writer_readiness_dry_run(
            true, true, true, true, true,
        );

        assert_eq!(
            readiness["contract"],
            json!("fashion_design_asset_retrieval_evidence_live_writer_readiness_dry_run_v1")
        );
        assert_eq!(
            readiness["writer_status"],
            json!("ready_for_controlled_writer_review")
        );
        assert_eq!(readiness["writer_ready_for_operator_review"], json!(true));
        assert_eq!(readiness["release_blocker_count"], json!(0));
        assert_eq!(readiness["no_write"], json!(true));
        assert_eq!(readiness["production_write_allowed"], json!(false));
        assert_eq!(
            readiness["live_write_still_requires_operator_ack"],
            json!(true)
        );
        assert_eq!(
            readiness["idempotency"]["strategy"],
            json!("upsert_by_dataset_asset_profile_parser_version")
        );
        assert_eq!(
            readiness["membership_guard"]["deny_hidden_assets"],
            json!(true)
        );
        assert_eq!(
            readiness["search_contract"]["returns_source_kind"],
            json!("asset_profile")
        );
        assert_eq!(
            readiness["operator_controls"]["live_write_allowed"],
            json!(false)
        );

        let serialized = serde_json::to_string(&readiness).expect("readiness should serialize");
        assert!(!serialized.contains("Bearer "));
        assert!(!serialized.contains("cookie"));
        assert!(!serialized.contains("object_key"));
        assert!(!serialized.contains("objects/private"));
        assert!(!serialized.contains("http://"));
        assert!(!serialized.contains("https://"));
    }

    #[test]
    fn asset_retrieval_evidence_writer_readiness_blocks_missing_membership_guard() {
        let readiness = fashion_design_asset_retrieval_evidence_live_writer_readiness_dry_run(
            true, true, false, true, true,
        );

        assert_eq!(
            readiness["writer_status"],
            json!("blocked_dataset_membership_guard")
        );
        assert_eq!(readiness["writer_ready_for_operator_review"], json!(false));
        assert_eq!(readiness["release_blocker_count"], json!(1));
        assert_eq!(
            readiness["input_readiness"]["dataset_membership_verified"],
            json!(false)
        );
        assert_eq!(
            readiness["membership_guard"]["tenant_scope_required"],
            json!(true)
        );
        assert_eq!(readiness["no_write"], json!(true));
    }

    #[test]
    fn asset_retrieval_evidence_writer_readiness_blocks_missing_table_before_live_write() {
        let readiness = fashion_design_asset_retrieval_evidence_live_writer_readiness_dry_run(
            true, true, true, false, true,
        );

        assert_eq!(
            readiness["writer_status"],
            json!("blocked_asset_evidence_table_missing")
        );
        assert_eq!(
            readiness["schema_migration_required_before_live_write"],
            json!(true)
        );
        assert_eq!(readiness["reviewed_migration_required"], json!(true));
        assert_eq!(
            readiness["operator_controls"]["controlled_write_after_review_only"],
            json!(true)
        );
        assert_eq!(
            readiness["validation"]["public_third_party_contract_changed"],
            json!(false)
        );
    }

    #[test]
    fn third_party_asset_import_private_endpoint_guard_allows_review_only_contract() {
        let guard = fashion_design_third_party_asset_import_private_endpoint_guard_dry_run(
            false, true, true, true, true, true, true,
        );

        assert_eq!(
            guard["contract"],
            json!("fashion_design_third_party_asset_import_private_endpoint_guard_dry_run_v1")
        );
        assert_eq!(
            guard["endpoint_status"],
            json!("ready_for_private_endpoint_review")
        );
        assert_eq!(guard["private_endpoint_ready"], json!(true));
        assert_eq!(guard["public_docs_asset_imports_closed"], json!(true));
        assert_eq!(
            guard["privacy_policy"]["third_party_assets_private_by_default"],
            json!(true)
        );
        assert_eq!(
            guard["response_contract"]["reply_shape"],
            json!("structured_json")
        );
        assert_eq!(
            guard["dataset_attachment_policy"]["dataset_external_ids_supported"],
            json!(true)
        );
        assert_eq!(guard["public_contract_changed"], json!(false));
        assert_eq!(guard["no_write"], json!(true));

        let serialized = serde_json::to_string(&guard).expect("guard should serialize");
        assert!(!serialized.contains("Bearer "));
        assert!(!serialized.contains("cookie"));
        assert!(!serialized.contains("objects/private"));
        assert!(!serialized.contains("http://"));
        assert!(!serialized.contains("https://"));
    }

    #[test]
    fn third_party_asset_import_private_endpoint_guard_blocks_public_docs_open() {
        let guard = fashion_design_third_party_asset_import_private_endpoint_guard_dry_run(
            true, true, true, true, true, true, true,
        );

        assert_eq!(
            guard["endpoint_status"],
            json!("blocked_public_docs_expose_unreviewed_asset_imports")
        );
        assert_eq!(guard["private_endpoint_ready"], json!(false));
        assert_eq!(guard["public_docs_asset_imports_closed"], json!(false));
        assert_eq!(guard["release_blocker_count"], json!(1));
        assert_eq!(guard["public_contract_changed"], json!(false));
    }

    #[test]
    fn third_party_asset_import_private_endpoint_guard_blocks_missing_asset_library_scope() {
        let guard = fashion_design_third_party_asset_import_private_endpoint_guard_dry_run(
            false, true, true, true, true, false, true,
        );

        assert_eq!(
            guard["endpoint_status"],
            json!("blocked_asset_library_scope_missing")
        );
        assert_eq!(guard["private_endpoint_ready"], json!(false));
        assert_eq!(
            guard["input_readiness"]["asset_library_scope_supplied"],
            json!(false)
        );
        assert_eq!(
            guard["validation"]["dataset_attachment_guard_ready"],
            json!(false)
        );
        assert_eq!(
            guard["operator_controls"]["controlled_execute_after_review_only"],
            json!(true)
        );
    }

    #[test]
    fn main_site_gallery_task_card_dry_run_allows_stable_detail_review() {
        let task_card =
            fashion_design_main_site_gallery_task_card_dry_run(true, true, true, true, true, true);

        assert_eq!(
            task_card["contract"],
            json!("fashion_design_main_site_gallery_task_card_dry_run_v1")
        );
        assert_eq!(
            task_card["task_status"],
            json!("ready_for_task_card_dry_run")
        );
        assert_eq!(task_card["task_card_ready"], json!(true));
        assert_eq!(
            task_card["planned_task_card"]["type"],
            json!("asset_gallery_import_task")
        );
        assert_eq!(
            task_card["refresh_policy"]["card_persists_after_creation"],
            json!(true)
        );
        assert_eq!(
            task_card["refresh_policy"]["selected_card_refresh_only"],
            json!(true)
        );
        assert_eq!(
            task_card["refresh_policy"]["no_unrelated_artifact_injection"],
            json!(true)
        );
        assert_eq!(task_card["no_chat_message"], json!(true));
        assert_eq!(task_card["no_artifact_creation"], json!(true));
        assert_eq!(
            task_card["detail_contract"]["shows_data_sources"],
            json!(true)
        );
        assert_eq!(
            task_card["detail_contract"]["shows_field_ledger"],
            json!(true)
        );

        let serialized = serde_json::to_string(&task_card).expect("task card should serialize");
        assert!(!serialized.contains("Bearer "));
        assert!(!serialized.contains("cookie"));
        assert!(!serialized.contains("object_key"));
        assert!(!serialized.contains("objects/private"));
        assert!(!serialized.contains("http://"));
        assert!(!serialized.contains("https://"));
    }

    #[test]
    fn main_site_gallery_task_card_dry_run_blocks_missing_field_ledger() {
        let task_card =
            fashion_design_main_site_gallery_task_card_dry_run(true, true, true, true, false, true);

        assert_eq!(
            task_card["task_status"],
            json!("blocked_field_ledger_missing")
        );
        assert_eq!(task_card["task_card_ready"], json!(false));
        assert_eq!(task_card["release_blocker_count"], json!(1));
        assert_eq!(
            task_card["validation"]["detail_contract_ready"],
            json!(false)
        );
        assert_eq!(task_card["no_task_creation"], json!(true));
    }

    #[test]
    fn main_site_gallery_task_card_dry_run_blocks_unstable_identity() {
        let task_card =
            fashion_design_main_site_gallery_task_card_dry_run(true, true, true, true, true, false);

        assert_eq!(
            task_card["task_status"],
            json!("blocked_unstable_task_identity")
        );
        assert_eq!(
            task_card["validation"]["task_card_contract_stable"],
            json!(false)
        );
        assert_eq!(
            task_card["planned_task_card"]["stable_identity_source"],
            json!("asset_library_id_dataset_id_import_batch_id")
        );
        assert_eq!(
            task_card["refresh_policy"]["no_global_polling"],
            json!(true)
        );
    }

    #[test]
    fn asset_import_uses_object_key_or_url_as_stable_source_when_external_id_is_missing() {
        let mut without_external_id = input();
        without_external_id.external_id = None;
        let prepared = prepare_fashion_design_image_asset_import(without_external_id)
            .expect("object key source should build");
        assert_eq!(
            prepared.asset.source_id.as_deref(),
            Some("assets/fashion/img-001.png")
        );

        let mut without_object_key = input();
        without_object_key.external_id = None;
        without_object_key.object_key = None;
        let prepared = prepare_fashion_design_image_asset_import(without_object_key)
            .expect("url source should build");
        assert_eq!(
            prepared.asset.source_id.as_deref(),
            Some("https://example.com/img-001.png")
        );
    }

    #[test]
    fn asset_import_rejects_missing_source_and_non_image_content_type() {
        let mut missing_source = input();
        missing_source.external_id = None;
        missing_source.object_key = None;
        missing_source.image_url = None;
        assert!(prepare_fashion_design_image_asset_import(missing_source).is_err());

        let mut bad_content_type = input();
        bad_content_type.content_type = Some("application/pdf".to_string());
        assert!(prepare_fashion_design_image_asset_import(bad_content_type).is_err());
    }

    #[test]
    fn asset_import_builds_input_from_contract_request_and_rejects_bad_refs() {
        let input = fashion_design_image_asset_import_input_from_request(request())
            .expect("contract request should build input");
        assert_eq!(input.dataset_id, DatasetId(Uuid::from_u128(1)));
        assert_eq!(input.asset_library_id, Some(Uuid::from_u128(2)));
        assert_eq!(input.collection_id, Some(Uuid::from_u128(3)));
        assert_eq!(input.profile_payload["category"], json!("dress"));

        let mut bad = request();
        bad.asset_library_id = Some("not-a-uuid".to_string());
        assert!(fashion_design_image_asset_import_input_from_request(bad).is_err());
    }

    #[test]
    fn asset_import_validates_collection_scope_against_asset_library() {
        let asset_library_id = Uuid::from_u128(2);
        let collection_id = Uuid::from_u128(3);

        assert!(validate_fashion_design_image_asset_import_collection_scope(
            Some(asset_library_id),
            asset_library_id,
            collection_id,
        )
        .is_ok());

        let missing_library = validate_fashion_design_image_asset_import_collection_scope(
            None,
            asset_library_id,
            collection_id,
        )
        .expect_err("collection without asset library should be rejected");
        assert_eq!(
            missing_library.payload.code,
            "asset_import_collection_requires_asset_library"
        );

        let mismatched_library = validate_fashion_design_image_asset_import_collection_scope(
            Some(Uuid::from_u128(4)),
            asset_library_id,
            collection_id,
        )
        .expect_err("collection from another asset library should be rejected");
        assert_eq!(
            mismatched_library.payload.code,
            "asset_import_collection_library_mismatch"
        );
    }

    #[test]
    fn asset_import_builds_inputs_from_batch_request_and_merges_metadata() {
        let inputs = fashion_design_image_asset_import_inputs_from_batch_request(batch_request())
            .expect("batch request should build inputs");

        assert_eq!(inputs.len(), 2);
        assert_eq!(inputs[0].dataset_id, DatasetId(Uuid::from_u128(1)));
        assert_eq!(inputs[0].asset_library_id, Some(Uuid::from_u128(2)));
        assert_eq!(inputs[0].collection_id, Some(Uuid::from_u128(3)));
        assert_eq!(inputs[0].external_id.as_deref(), Some("img-001"));
        assert_eq!(
            inputs[0].metadata["source_package"],
            json!("fashion-zip-001")
        );
        assert_eq!(inputs[0].metadata["operator"], json!("main-site"));
        assert_eq!(inputs[0].metadata["filename"], json!("img-001.png"));
        assert_eq!(inputs[1].metadata["operator"], json!("override"));

        let prepared_first = prepare_fashion_design_image_asset_import(inputs[0].clone())
            .expect("batch item should prepare");
        assert!(prepared_first.asset.metadata.get("filename").is_none());

        let prepared = prepare_fashion_design_image_asset_import(inputs[1].clone())
            .expect("batch item should prepare");
        assert_eq!(prepared.asset.source_id.as_deref(), Some("img-002"));
        assert_eq!(prepared.profile.attributes["category"], json!("jacket"));
    }

    #[test]
    fn asset_import_expands_zip_package_images_into_batch_inputs() {
        let zip_path = write_test_zip(
            "fashion-assets",
            &[
                ("looks/dress.png", b"png-bytes".as_slice()),
                ("docs/readme.txt", b"skip text".as_slice()),
                ("looks/coat.webp", b"webp-bytes".as_slice()),
            ],
        );
        let mut request = batch_request();
        request.assets.clear();
        request.packages = vec![FashionDesignImageAssetImportPackage {
            external_id: Some("zip-001".to_string()),
            title: Some("春夏图库".to_string()),
            object_key: zip_path.to_string_lossy().to_string(),
            content_type: Some("application/zip".to_string()),
            metadata: json!({
                "operator": "main-site",
                "original_name": "客户图库.zip",
                "path": zip_path.to_string_lossy(),
                "raw_provider_payload": {"should_not_surface": true}
            }),
        }];

        let inputs = fashion_design_image_asset_import_inputs_from_batch_request(request)
            .expect("zip package images should expand into asset inputs");

        assert_eq!(inputs.len(), 2);
        assert_eq!(inputs[0].title, "春夏图库 - dress.png");
        assert_eq!(inputs[0].content_type.as_deref(), Some("image/png"));
        assert!(inputs[0]
            .object_key
            .as_deref()
            .unwrap_or_default()
            .contains("_asset_zip_extracted"));
        assert_eq!(
            inputs[0].metadata["source_package"]["external_id"],
            json!("zip-001")
        );
        assert_eq!(
            inputs[0].metadata["source_package"]["object_key_present"],
            json!(true)
        );
        assert!(inputs[0].metadata["source_package"]
            .get("object_key")
            .is_none());
        assert_eq!(inputs[0].metadata["operator"], json!("main-site"));
        assert!(inputs[0].metadata.get("original_name").is_none());
        assert!(inputs[0].metadata.get("path").is_none());
        assert!(inputs[0].metadata.get("raw_provider_payload").is_none());
        assert_eq!(
            inputs[0].metadata["zip_entry"]["entry_name_present"],
            json!(true)
        );
        assert_eq!(inputs[0].metadata["zip_entry"]["extension"], json!("png"));
        assert!(inputs[0].metadata["zip_entry"].get("entry_name").is_none());

        let prepared = prepare_fashion_design_image_asset_import(inputs[0].clone())
            .expect("expanded zip image input should prepare");
        let source_id = prepared.asset.source_id.as_deref().unwrap_or_default();
        assert!(source_id.starts_with("zip-001:"));
        assert!(!source_id.contains("dress"));
        assert!(!source_id.contains("looks"));
        assert_eq!(prepared.parse_run.status, "pending");
    }

    #[test]
    fn asset_import_expanded_zip_source_id_uses_safe_package_ref_without_external_id() {
        let zip_path = write_test_zip(
            "fashion-assets-without-external-id",
            &[
                ("looks/dress.png", b"png-bytes".as_slice()),
                ("looks/coat.webp", b"webp-bytes".as_slice()),
            ],
        );
        let mut request = batch_request();
        request.assets.clear();
        request.packages = vec![FashionDesignImageAssetImportPackage {
            external_id: None,
            title: Some("匿名图库".to_string()),
            object_key: zip_path.to_string_lossy().to_string(),
            content_type: Some("application/zip".to_string()),
            metadata: json!({}),
        }];

        let inputs = fashion_design_image_asset_import_inputs_from_batch_request(request)
            .expect("zip package images should expand into asset inputs");

        let prepared = prepare_fashion_design_image_asset_import(inputs[0].clone())
            .expect("expanded zip image input should prepare");
        let source_id = prepared.asset.source_id.as_deref().unwrap_or_default();
        assert!(source_id.starts_with("zip-ref-"));
        assert!(!source_id.contains("dress"));
        assert!(!source_id.contains("looks"));
        assert!(!source_id.contains("fashion-assets"));
        assert!(!source_id.contains("_asset_zip_extracted"));
    }

    #[test]
    fn asset_import_rejects_bad_batch_refs() {
        let mut bad = batch_request();
        bad.asset_library_id = Some("not-a-uuid".to_string());

        assert!(fashion_design_image_asset_import_inputs_from_batch_request(bad).is_err());
    }

    fn write_test_zip(name: &str, entries: &[(&str, &[u8])]) -> std::path::PathBuf {
        use std::io::Write as _;
        use zip::{write::SimpleFileOptions, ZipWriter};

        let root = std::env::temp_dir().join(format!("datamax-asset-zip-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("zip temp root should be created");
        let path = root.join(format!("{name}.zip"));
        let file = std::fs::File::create(&path).expect("zip file should be created");
        let mut writer = ZipWriter::new(file);
        let options = SimpleFileOptions::default();
        for (entry_name, bytes) in entries {
            writer
                .start_file(entry_name, options)
                .expect("zip entry should start");
            writer.write_all(bytes).expect("zip entry should write");
        }
        writer.finish().expect("zip should finish");
        path
    }
}
