use contracts::{
    CreateFashionDesignImageAssetImportBatchRequest, FashionDesignImageAssetImportItem,
    FashionDesignImageAssetImportPackage,
};
use domain_model::{DatasetId, UserId};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sqlx::Row;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

use crate::{
    asset_import_support::{
        fashion_design_image_asset_import_inputs_from_batch_request,
        upsert_fashion_design_image_asset_import_for_tenant,
        validate_fashion_design_image_asset_import_scope_refs, SyncedFashionDesignImageAssetImport,
    },
    external_channel_image_idempotency_support::{
        external_asset_import_request_fingerprint, external_asset_import_source_id,
    },
    external_channel_support::{
        external_channel_database_source_allowed, external_channel_default_source_id_from_config,
    },
    external_document_object_support::{
        effective_external_document_parse_dataset_external_id,
        external_dataset_matches_external_document_parse_dataset,
    },
    fashion_postchain_adapter_support::FASHION_DESIGN_IMAGE_PROFILE_KIND,
    platform_env_flag, ApiError, AppState,
};

pub(crate) const EXTERNAL_ASSET_IMPORT_ENABLED_ENV: &str = "EXTERNAL_ASSET_IMPORT_ENABLED";
pub(crate) const EXTERNAL_ASSET_IMPORT_CONNECTION_ALLOWLIST_ENV: &str =
    "EXTERNAL_ASSET_IMPORT_CONNECTION_ALLOWLIST";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExternalAssetImportAccess {
    Allowed,
    FeatureDisabled,
    ConnectionNotAllowlisted,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateExternalAssetImportRequest {
    pub request_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_external_id: Option<String>,
    pub asset_library_external_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_collection_external_id: Option<String>,
    pub dataset_external_ids: Vec<String>,
    #[serde(default)]
    pub assets: Vec<FashionDesignImageAssetImportItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub packages: Vec<FashionDesignImageAssetImportPackage>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ExternalAssetImportSafeAssetSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_external_id: Option<String>,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    pub status: String,
    pub profile_schema: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct CreateExternalAssetImportResponse {
    pub request_id: String,
    pub task_ref: String,
    pub parse_status: String,
    pub assets: Vec<ExternalAssetImportSafeAssetSummary>,
}

pub(crate) fn external_asset_import_access(connection_id: &str) -> ExternalAssetImportAccess {
    external_asset_import_access_with(
        platform_env_flag(EXTERNAL_ASSET_IMPORT_ENABLED_ENV, false),
        &std::env::var(EXTERNAL_ASSET_IMPORT_CONNECTION_ALLOWLIST_ENV).unwrap_or_default(),
        connection_id,
    )
}

pub(crate) fn external_asset_import_access_with(
    enabled: bool,
    allowlist: &str,
    connection_id: &str,
) -> ExternalAssetImportAccess {
    if !enabled {
        return ExternalAssetImportAccess::FeatureDisabled;
    }
    let expected = connection_id.trim();
    if expected.is_empty()
        || !allowlist
            .split(',')
            .map(str::trim)
            .any(|value| !value.is_empty() && value.eq_ignore_ascii_case(expected))
    {
        return ExternalAssetImportAccess::ConnectionNotAllowlisted;
    }
    ExternalAssetImportAccess::Allowed
}

pub(crate) fn ensure_external_asset_import_access(
    connection_id: &str,
) -> std::result::Result<(), ApiError> {
    match external_asset_import_access(connection_id) {
        ExternalAssetImportAccess::Allowed => Ok(()),
        ExternalAssetImportAccess::FeatureDisabled => Err(ApiError::forbidden(
            "external_asset_import_feature_disabled",
            "private external asset import is disabled".to_string(),
        )),
        ExternalAssetImportAccess::ConnectionNotAllowlisted => Err(ApiError::forbidden(
            "external_asset_import_connection_not_allowlisted",
            "external channel connection is not allowlisted for private asset import".to_string(),
        )),
    }
}

pub(crate) fn resolve_external_asset_import_source_id(
    request: &CreateExternalAssetImportRequest,
    connection_config: &Value,
) -> std::result::Result<String, ApiError> {
    let source_id = request
        .source_external_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .or_else(|| external_channel_default_source_id_from_config(connection_config))
        .ok_or_else(|| {
            ApiError::bad_request(
                "external_asset_import_source_required",
                "source_external_id is required when the connection has no default source"
                    .to_string(),
            )
        })?;
    if !external_channel_database_source_allowed(connection_config, &source_id) {
        return Err(ApiError::forbidden(
            "external_asset_import_source_not_allowed",
            "external source is not allowed for this channel connection".to_string(),
        ));
    }
    Ok(source_id)
}

pub(crate) async fn create_external_asset_import_response(
    state: &AppState,
    connection_id: &str,
    source_id: &str,
    owner_user_id: UserId,
    request: CreateExternalAssetImportRequest,
) -> std::result::Result<CreateExternalAssetImportResponse, ApiError> {
    let request = normalize_external_asset_import_request(request)?;
    let fingerprint = external_asset_import_request_fingerprint(
        &serde_json::to_value(&request).map_err(|error| {
            ApiError::bad_request(
                "external_asset_import_request_invalid",
                format!("external asset import request could not be normalized: {error}"),
            )
        })?,
    );
    let datasets = resolve_external_asset_import_datasets(
        state,
        source_id,
        owner_user_id,
        &request.dataset_external_ids,
    )
    .await?;
    let asset_library_id = resolve_external_asset_import_library(
        state,
        connection_id,
        source_id,
        &request.asset_library_external_id,
        &datasets,
    )
    .await?;
    let collection_id = resolve_external_asset_import_collection(
        state,
        asset_library_id,
        request.asset_collection_external_id.as_deref(),
    )
    .await?;
    validate_fashion_design_image_asset_import_scope_refs(
        state,
        Some(asset_library_id),
        collection_id,
    )
    .await?;

    let first_dataset_id = datasets[0];
    let batch = CreateFashionDesignImageAssetImportBatchRequest {
        dataset_id: first_dataset_id,
        asset_library_id: Some(asset_library_id.to_string()),
        collection_id: collection_id.map(|value| value.to_string()),
        assets: request.assets,
        packages: request.packages,
        metadata: request.metadata,
    };
    let mut inputs = fashion_design_image_asset_import_inputs_from_batch_request(batch)?;
    let mut unique_assets = BTreeMap::<Uuid, SyncedFashionDesignImageAssetImport>::new();
    let mut stable_source_ids = BTreeSet::new();
    for input in &mut inputs {
        let stable_source_id = external_asset_import_source_id(
            connection_id,
            source_id,
            input.external_id.as_deref(),
            input.object_key.as_deref(),
            input.image_url.as_deref(),
        )
        .ok_or_else(|| {
            ApiError::bad_request(
                "external_asset_import_asset_source_required",
                "each external asset requires external_id, object_key, or image_url".to_string(),
            )
        })?;
        if !stable_source_ids.insert(stable_source_id.clone()) {
            return Err(ApiError::bad_request(
                "external_asset_import_duplicate_asset_identity",
                "private asset import contains duplicate stable asset identities".to_string(),
            ));
        }
        input.stable_source_id_override = Some(stable_source_id);
    }
    let stable_source_ids = stable_source_ids.into_iter().collect::<Vec<_>>();
    if external_asset_import_request_state(
        state,
        connection_id,
        &request.request_id,
        &fingerprint,
        &stable_source_ids,
    )
    .await?
        == ExternalAssetImportRequestState::Replay
    {
        return load_external_asset_import_replay_response(
            state,
            &request.request_id,
            &fingerprint,
            &stable_source_ids,
        )
        .await;
    }
    for input in &mut inputs {
        let stable_source_id = input
            .stable_source_id_override
            .as_deref()
            .expect("stable source id was assigned");
        let existing_metadata =
            load_external_asset_import_metadata(state, stable_source_id).await?;
        input.metadata = external_asset_import_metadata(
            input.metadata.clone(),
            existing_metadata.as_ref(),
            connection_id,
            &request.request_id,
            &fingerprint,
        )?;
    }

    for dataset_id in datasets {
        for base in &inputs {
            let mut input = base.clone();
            input.dataset_id = dataset_id;
            let synced =
                upsert_fashion_design_image_asset_import_for_tenant(state, state.tenant_id, input)
                    .await?;
            unique_assets.insert(synced.asset.id, synced);
        }
    }

    let parse_status = aggregate_external_asset_import_parse_status(unique_assets.values());
    let assets = unique_assets
        .into_values()
        .map(|synced| ExternalAssetImportSafeAssetSummary {
            asset_external_id: synced.asset.external_id,
            title: synced.asset.title,
            content_type: synced.asset.content_type,
            status: "accepted".to_string(),
            profile_schema: synced.profile.profile_kind,
        })
        .collect();
    Ok(CreateExternalAssetImportResponse {
        request_id: request.request_id,
        task_ref: format!("external-asset-import:{}", &fingerprint[..24]),
        parse_status,
        assets,
    })
}

fn normalize_external_asset_import_request(
    mut request: CreateExternalAssetImportRequest,
) -> std::result::Result<CreateExternalAssetImportRequest, ApiError> {
    request.request_id = required_external_asset_import_text("request_id", &request.request_id)?;
    request.asset_library_external_id = required_external_asset_import_text(
        "asset_library_external_id",
        &request.asset_library_external_id,
    )?;
    request.source_external_id = request
        .source_external_id
        .and_then(|value| non_empty_external_asset_import_text(&value));
    request.asset_collection_external_id = request
        .asset_collection_external_id
        .and_then(|value| non_empty_external_asset_import_text(&value));
    if request.request_id.len() > 128 {
        return Err(ApiError::bad_request(
            "external_asset_import_request_id_invalid",
            "request_id must not exceed 128 characters".to_string(),
        ));
    }
    let mut dataset_external_ids = BTreeSet::new();
    for value in request.dataset_external_ids {
        let value = required_external_asset_import_text("dataset_external_ids", &value)?;
        dataset_external_ids.insert(value);
    }
    if dataset_external_ids.is_empty() || dataset_external_ids.len() > 10 {
        return Err(ApiError::bad_request(
            "external_asset_import_dataset_scope_invalid",
            "dataset_external_ids must contain between 1 and 10 values".to_string(),
        ));
    }
    request.dataset_external_ids = dataset_external_ids.into_iter().collect();
    if request.assets.is_empty() && request.packages.is_empty() {
        return Err(ApiError::bad_request(
            "external_asset_import_assets_required",
            "private external asset import requires at least one asset or package".to_string(),
        ));
    }
    if request.assets.len() + request.packages.len() > 100 {
        return Err(ApiError::bad_request(
            "external_asset_import_batch_too_large",
            "private external asset import supports at most 100 direct assets and packages"
                .to_string(),
        ));
    }
    Ok(request)
}

fn required_external_asset_import_text(
    field: &str,
    value: &str,
) -> std::result::Result<String, ApiError> {
    non_empty_external_asset_import_text(value).ok_or_else(|| {
        ApiError::bad_request(
            "external_asset_import_scope_invalid",
            format!("{field} must be non-empty"),
        )
    })
}

fn non_empty_external_asset_import_text(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExternalAssetImportRequestState {
    New,
    Replay,
}

async fn external_asset_import_request_state(
    state: &AppState,
    connection_id: &str,
    request_id: &str,
    fingerprint: &str,
    stable_source_ids: &[String],
) -> std::result::Result<ExternalAssetImportRequestState, ApiError> {
    let conflicting = sqlx::query_scalar::<_, i64>(
        r#"
        select count(*)::bigint
        from asset_items
        where tenant_id = $1
          and metadata #>> '{external_asset_import,connection_id}' = $2
          and coalesce(
                (metadata #> '{external_asset_import,request_fingerprints}') ->> $3,
                case
                    when metadata #>> '{external_asset_import,request_id}' = $3
                    then metadata #>> '{external_asset_import,request_fingerprint}'
                    else null
                end
              ) is not null
          and coalesce(
                (metadata #> '{external_asset_import,request_fingerprints}') ->> $3,
                case
                    when metadata #>> '{external_asset_import,request_id}' = $3
                    then metadata #>> '{external_asset_import,request_fingerprint}'
                    else null
                end
              ) <> $4
        "#,
    )
    .bind(state.tenant_id.0)
    .bind(connection_id)
    .bind(request_id)
    .bind(fingerprint)
    .fetch_one(state.storage.pool())
    .await
    .map_err(|error| ApiError::from_storage(anyhow::Error::new(error)))?;
    if conflicting > 0 {
        return Err(ApiError::bad_request(
            "external_asset_import_idempotency_conflict",
            "request_id was already used with a different private asset import payload".to_string(),
        ));
    }
    let exact = sqlx::query_scalar::<_, i64>(
        r#"
        select count(distinct source_id)::bigint
        from asset_items
        where tenant_id = $1
          and metadata #>> '{external_asset_import,connection_id}' = $2
          and source_kind = 'fashion_design_image_import'
          and source_id = any($5)
          and coalesce(
                (metadata #> '{external_asset_import,request_fingerprints}') ->> $3,
                case
                    when metadata #>> '{external_asset_import,request_id}' = $3
                    then metadata #>> '{external_asset_import,request_fingerprint}'
                    else null
                end
              ) = $4
        "#,
    )
    .bind(state.tenant_id.0)
    .bind(connection_id)
    .bind(request_id)
    .bind(fingerprint)
    .bind(stable_source_ids)
    .fetch_one(state.storage.pool())
    .await
    .map_err(|error| ApiError::from_storage(anyhow::Error::new(error)))?;
    Ok(if exact == stable_source_ids.len() as i64 {
        ExternalAssetImportRequestState::Replay
    } else {
        ExternalAssetImportRequestState::New
    })
}

async fn load_external_asset_import_metadata(
    state: &AppState,
    stable_source_id: &str,
) -> std::result::Result<Option<Value>, ApiError> {
    sqlx::query_scalar::<_, Value>(
        r#"
        select metadata
        from asset_items
        where tenant_id = $1
          and source_kind = 'fashion_design_image_import'
          and source_id = $2
        "#,
    )
    .bind(state.tenant_id.0)
    .bind(stable_source_id)
    .fetch_optional(state.storage.pool())
    .await
    .map_err(|error| ApiError::from_storage(anyhow::Error::new(error)))
}

async fn load_external_asset_import_replay_response(
    state: &AppState,
    request_id: &str,
    fingerprint: &str,
    stable_source_ids: &[String],
) -> std::result::Result<CreateExternalAssetImportResponse, ApiError> {
    let rows = sqlx::query(
        r#"
        select a.external_id,
               a.title,
               a.content_type,
               coalesce(
                   (select r.status
                    from asset_parse_runs r
                    where r.tenant_id = a.tenant_id and r.asset_id = a.id
                    order by r.updated_at desc, r.created_at desc
                    limit 1),
                   'pending'
               ) as parse_status,
               coalesce(
                   (select p.profile_kind
                    from asset_profiles p
                    where p.tenant_id = a.tenant_id and p.asset_id = a.id
                    order by p.updated_at desc, p.created_at desc
                    limit 1),
                   $3
               ) as profile_schema
        from asset_items a
        where a.tenant_id = $1
          and a.source_kind = 'fashion_design_image_import'
          and a.source_id = any($2)
        order by a.external_id nulls last, a.title
        "#,
    )
    .bind(state.tenant_id.0)
    .bind(stable_source_ids)
    .bind(FASHION_DESIGN_IMAGE_PROFILE_KIND)
    .fetch_all(state.storage.pool())
    .await
    .map_err(|error| ApiError::from_storage(anyhow::Error::new(error)))?;
    if rows.len() != stable_source_ids.len() {
        return Err(ApiError::internal(
            "external_asset_import_replay_incomplete",
            "idempotent replay could not resolve every existing asset".to_string(),
        ));
    }
    let statuses = rows
        .iter()
        .map(|row| row.get::<String, _>("parse_status"))
        .collect::<BTreeSet<_>>();
    let parse_status = if statuses.len() == 1 {
        statuses
            .into_iter()
            .next()
            .unwrap_or_else(|| "pending".to_string())
    } else {
        "mixed".to_string()
    };
    let assets = rows
        .into_iter()
        .map(|row| ExternalAssetImportSafeAssetSummary {
            asset_external_id: row.get("external_id"),
            title: row.get("title"),
            content_type: row.get("content_type"),
            status: "accepted".to_string(),
            profile_schema: row.get("profile_schema"),
        })
        .collect();
    Ok(CreateExternalAssetImportResponse {
        request_id: request_id.to_string(),
        task_ref: format!("external-asset-import:{}", &fingerprint[..24]),
        parse_status,
        assets,
    })
}

async fn resolve_external_asset_import_datasets(
    state: &AppState,
    source_id: &str,
    owner_user_id: UserId,
    dataset_external_ids: &[String],
) -> std::result::Result<Vec<DatasetId>, ApiError> {
    let available = state
        .storage
        .datasets()
        .list_by_tenant(state.tenant_id)
        .await
        .map_err(ApiError::from_storage)?;
    let mut resolved = Vec::with_capacity(dataset_external_ids.len());
    for requested in dataset_external_ids {
        let effective = effective_external_document_parse_dataset_external_id(Some(requested))
            .ok_or_else(|| {
                ApiError::bad_request(
                    "external_asset_import_dataset_scope_invalid",
                    "dataset_external_ids must be non-empty".to_string(),
                )
            })?;
        let matches = available
            .iter()
            .filter(|dataset| {
                external_dataset_matches_external_document_parse_dataset(
                    dataset, source_id, &effective,
                )
            })
            .collect::<Vec<_>>();
        let dataset = match matches.as_slice() {
            [] => {
                return Err(ApiError::not_found(
                    "external_asset_import_dataset_not_found",
                    "external dataset scope was not found for the selected source".to_string(),
                ));
            }
            [dataset] => *dataset,
            _ => {
                return Err(ApiError::bad_request(
                    "external_asset_import_dataset_scope_ambiguous",
                    "external dataset scope matches more than one dataset".to_string(),
                ));
            }
        };
        if dataset.owner_user_id != Some(owner_user_id) {
            return Err(ApiError::forbidden(
                "external_asset_import_dataset_owner_mismatch",
                "external dataset belongs to another connection owner".to_string(),
            ));
        }
        resolved.push(dataset.id);
    }
    Ok(resolved)
}

async fn resolve_external_asset_import_library(
    state: &AppState,
    connection_id: &str,
    source_id: &str,
    external_id: &str,
    dataset_ids: &[DatasetId],
) -> std::result::Result<Uuid, ApiError> {
    let library = state
        .storage
        .asset_libraries()
        .list_by_tenant(state.tenant_id)
        .await
        .map_err(ApiError::from_storage)?
        .into_iter()
        .find(|library| library.external_id.as_deref() == Some(external_id))
        .ok_or_else(|| {
            ApiError::not_found(
                "external_asset_import_library_not_found",
                "private asset library external scope was not found".to_string(),
            )
        })?;
    if !library.visibility.eq_ignore_ascii_case("private") {
        return Err(ApiError::forbidden(
            "external_asset_import_library_not_private",
            "external asset imports require a private asset library".to_string(),
        ));
    }
    let binding = library.metadata.get("external_asset_import");
    let bound_connection_id = binding
        .and_then(|value| value.get("connection_id"))
        .and_then(Value::as_str);
    let bound_source_id = binding
        .and_then(|value| value.get("source_external_id"))
        .and_then(Value::as_str);
    if bound_connection_id != Some(connection_id) || bound_source_id != Some(source_id) {
        return Err(ApiError::forbidden(
            "external_asset_import_library_scope_mismatch",
            "asset library is not bound to the selected connection and source".to_string(),
        ));
    }
    let memberships = state
        .storage
        .asset_libraries()
        .list_dataset_memberships(state.tenant_id, library.id)
        .await
        .map_err(ApiError::from_storage)?;
    if dataset_ids.iter().any(|dataset_id| {
        !memberships
            .iter()
            .any(|membership| membership.dataset_id == *dataset_id)
    }) {
        return Err(ApiError::forbidden(
            "external_asset_import_library_dataset_mismatch",
            "asset library is not attached to every requested dataset".to_string(),
        ));
    }
    Ok(library.id)
}

async fn resolve_external_asset_import_collection(
    state: &AppState,
    asset_library_id: Uuid,
    external_id: Option<&str>,
) -> std::result::Result<Option<Uuid>, ApiError> {
    let Some(external_id) = external_id else {
        return Ok(None);
    };
    let collection_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        select id
        from asset_collections
        where tenant_id = $1
          and asset_library_id = $2
          and external_id = $3
        "#,
    )
    .bind(state.tenant_id.0)
    .bind(asset_library_id)
    .bind(external_id)
    .fetch_optional(state.storage.pool())
    .await
    .map_err(|error| ApiError::from_storage(anyhow::Error::new(error)))?
    .ok_or_else(|| {
        ApiError::not_found(
            "external_asset_import_collection_not_found",
            "asset collection external scope was not found in the selected library".to_string(),
        )
    })?;
    Ok(Some(collection_id))
}

fn external_asset_import_metadata(
    metadata: Value,
    existing_metadata: Option<&Value>,
    connection_id: &str,
    request_id: &str,
    fingerprint: &str,
) -> std::result::Result<Value, ApiError> {
    let mut metadata = metadata.as_object().cloned().unwrap_or_else(Map::new);
    let mut request_fingerprints = existing_metadata
        .and_then(|value| value.pointer("/external_asset_import/request_fingerprints"))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_else(Map::new);
    if let Some(existing) = existing_metadata.and_then(|value| value.get("external_asset_import")) {
        if let (Some(existing_request_id), Some(existing_fingerprint)) = (
            existing.get("request_id").and_then(Value::as_str),
            existing.get("request_fingerprint").and_then(Value::as_str),
        ) {
            request_fingerprints
                .entry(existing_request_id.to_string())
                .or_insert_with(|| json!(existing_fingerprint));
        }
    }
    if !request_fingerprints.contains_key(request_id) && request_fingerprints.len() >= 256 {
        return Err(ApiError::bad_request(
            "external_asset_import_request_history_full",
            "asset request history reached its safe limit; operator review is required".to_string(),
        ));
    }
    request_fingerprints.insert(request_id.to_string(), json!(fingerprint));
    metadata.insert(
        "external_asset_import".to_string(),
        json!({
            "connection_id": connection_id,
            "request_id": request_id,
            "request_fingerprint": fingerprint,
            "request_fingerprints": request_fingerprints,
            "private": true,
        }),
    );
    Ok(Value::Object(metadata))
}

fn aggregate_external_asset_import_parse_status<'a>(
    assets: impl Iterator<Item = &'a SyncedFashionDesignImageAssetImport>,
) -> String {
    let statuses = assets
        .map(|asset| asset.parse_run.status.as_str())
        .collect::<BTreeSet<_>>();
    if statuses.len() == 1 {
        statuses.into_iter().next().unwrap_or("pending").to_string()
    } else {
        "mixed".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use event_bus::EventBus;
    use storage::{NewAssetLibrary, NewAssetLibraryDatasetMembership, NewDataset};

    fn request() -> CreateExternalAssetImportRequest {
        CreateExternalAssetImportRequest {
            request_id: " request-001 ".to_string(),
            source_external_id: Some(" source-main ".to_string()),
            asset_library_external_id: " library-main ".to_string(),
            asset_collection_external_id: None,
            dataset_external_ids: vec![" group-b ".to_string(), "group-a".to_string()],
            assets: vec![FashionDesignImageAssetImportItem {
                external_id: Some("asset-001".to_string()),
                title: "Pilot image".to_string(),
                image_url: None,
                object_key: Some("objects/pilot.png".to_string()),
                content_type: Some("image/png".to_string()),
                profile_payload: json!({}),
                metadata: json!({}),
            }],
            packages: Vec::new(),
            metadata: json!({"pilot": true}),
        }
    }

    #[test]
    fn external_asset_import_access_is_default_closed_and_connection_scoped() {
        assert_eq!(
            external_asset_import_access_with(false, "pilot-connection", "pilot-connection"),
            ExternalAssetImportAccess::FeatureDisabled
        );
        assert_eq!(
            external_asset_import_access_with(true, "other-connection", "pilot-connection"),
            ExternalAssetImportAccess::ConnectionNotAllowlisted
        );
        assert_eq!(
            external_asset_import_access_with(
                true,
                " other-connection, PILOT-CONNECTION ",
                "pilot-connection"
            ),
            ExternalAssetImportAccess::Allowed
        );
    }

    #[test]
    fn external_asset_import_request_has_no_client_tenant_selector() {
        let error = serde_json::from_value::<CreateExternalAssetImportRequest>(json!({
            "request_id": "request-001",
            "tenant_id": "another-tenant",
            "asset_library_external_id": "library-main",
            "dataset_external_ids": ["group-a"],
            "assets": [{"external_id": "asset-001", "title": "Pilot image"}]
        }))
        .expect_err("tenant_id must be rejected as an unknown private-contract field");
        assert!(error.to_string().contains("tenant_id"));
    }

    #[test]
    fn external_asset_import_request_normalizes_scope_and_rejects_empty_assets() {
        let normalized = normalize_external_asset_import_request(request()).unwrap();
        assert_eq!(normalized.request_id, "request-001");
        assert_eq!(normalized.asset_library_external_id, "library-main");
        assert_eq!(normalized.dataset_external_ids, vec!["group-a", "group-b"]);

        let mut empty = request();
        empty.assets.clear();
        let error = normalize_external_asset_import_request(empty).unwrap_err();
        assert_eq!(error.payload.code, "external_asset_import_assets_required");
    }

    #[test]
    fn external_asset_import_source_requires_connection_scope() {
        let request = request();
        let allowed = resolve_external_asset_import_source_id(
            &request,
            &json!({"allowed_source_ids": ["source-main"]}),
        )
        .unwrap();
        assert_eq!(allowed, "source-main");

        let error = resolve_external_asset_import_source_id(
            &request,
            &json!({"allowed_source_ids": ["source-other"]}),
        )
        .unwrap_err();
        assert_eq!(
            error.payload.code,
            "external_asset_import_source_not_allowed"
        );
    }

    #[test]
    fn external_asset_import_metadata_excludes_locator_values() {
        let metadata = external_asset_import_metadata(
            json!({"operator": "pilot"}),
            None,
            "connection-main",
            "request-001",
            "fingerprint",
        )
        .unwrap();
        assert_eq!(
            metadata["external_asset_import"]["connection_id"],
            json!("connection-main")
        );
        assert!(metadata
            .pointer("/external_asset_import/object_key")
            .is_none());
        assert!(metadata
            .pointer("/external_asset_import/image_url")
            .is_none());
    }

    #[tokio::test]
    #[ignore = "requires EXTERNAL_ASSET_IMPORT_TEST_DATABASE_URL pointing to a disposable database"]
    async fn external_asset_import_database_gate_proves_idempotency_replacement_and_scope() {
        let database_url = std::env::var("EXTERNAL_ASSET_IMPORT_TEST_DATABASE_URL")
            .expect("EXTERNAL_ASSET_IMPORT_TEST_DATABASE_URL is required");
        assert!(
            database_url.contains("/aiv3_task11_"),
            "refusing non-disposable database"
        );
        std::env::set_var("ASSET_PARSE_ENABLED", "false");
        let storage = storage::PgStorage::connect(&database_url)
            .await
            .expect("connect disposable database");
        storage
            .migrate()
            .await
            .expect("migrate disposable database");
        let tenant = storage
            .ensure_tenant(
                &format!("task11-{}", Uuid::new_v4()),
                "Task 11 disposable tenant",
            )
            .await
            .expect("ensure tenant");
        let owner = storage
            .users()
            .ensure_by_email(
                tenant.id,
                "task11-owner@example.invalid",
                Some("Task 11 owner"),
            )
            .await
            .expect("ensure owner");
        let other_owner = storage
            .users()
            .ensure_by_email(
                tenant.id,
                "task11-other@example.invalid",
                Some("Task 11 other owner"),
            )
            .await
            .expect("ensure other owner");
        let source_id = "task11-source";
        let create_dataset = |external_id: &'static str| NewDataset {
            key: crate::external_document_object_support::external_document_parse_dataset_key(
                source_id,
                Some(external_id),
            ),
            title: format!("Task 11 {external_id}"),
            description: None,
            owner_user_id: Some(owner.id),
        };
        let dataset = storage
            .datasets()
            .create_with_metadata(
                tenant.id,
                create_dataset("group-main"),
                json!({
                    "visibility": "private",
                    "external_source": {
                        "source_id": source_id,
                        "dataset_external_id": "group-main"
                    }
                }),
            )
            .await
            .expect("create dataset");
        storage
            .datasets()
            .create_with_metadata(
                tenant.id,
                create_dataset("group-denied"),
                json!({
                    "visibility": "private",
                    "external_source": {
                        "source_id": source_id,
                        "dataset_external_id": "group-denied"
                    }
                }),
            )
            .await
            .expect("create denied dataset");
        let library = storage
            .asset_libraries()
            .create(
                tenant.id,
                NewAssetLibrary {
                    external_id: Some("library-main".to_string()),
                    name: "Task 11 library".to_string(),
                    domain: "fashion".to_string(),
                    description: None,
                    visibility: "private".to_string(),
                    metadata: json!({
                        "external_asset_import": {
                            "connection_id": "connection-main",
                            "source_external_id": source_id
                        }
                    }),
                },
            )
            .await
            .expect("create library");
        storage
            .asset_libraries()
            .upsert_dataset_membership(
                tenant.id,
                library.id,
                NewAssetLibraryDatasetMembership {
                    dataset_id: dataset.id,
                    role: "member".to_string(),
                    priority: 100,
                },
            )
            .await
            .expect("attach library to dataset");
        let state = AppState::new(
            storage.clone(),
            workflow_definitions::catalog(),
            tenant.id,
            EventBus::Disabled,
        );

        let mut first_request = request();
        first_request.dataset_external_ids = vec!["group-main".to_string()];
        let first = create_external_asset_import_response(
            &state,
            "connection-main",
            source_id,
            owner.id,
            first_request.clone(),
        )
        .await
        .expect("first import");
        let replay = create_external_asset_import_response(
            &state,
            "connection-main",
            source_id,
            owner.id,
            first_request.clone(),
        )
        .await
        .expect("idempotent replay");
        assert_eq!(first.task_ref, replay.task_ref);
        assert_eq!(first.assets.len(), 1);

        let mut conflict = first_request.clone();
        conflict.assets[0].title = "different payload".to_string();
        let error = create_external_asset_import_response(
            &state,
            "connection-main",
            source_id,
            owner.id,
            conflict,
        )
        .await
        .expect_err("same request id with different payload must fail");
        assert_eq!(
            error.payload.code,
            "external_asset_import_idempotency_conflict"
        );

        let mut replacement = first_request.clone();
        replacement.request_id = "request-002".to_string();
        replacement.assets[0].object_key = Some("objects/replacement.png".to_string());
        create_external_asset_import_response(
            &state,
            "connection-main",
            source_id,
            owner.id,
            replacement,
        )
        .await
        .expect("replacement import");
        let historical_replay = create_external_asset_import_response(
            &state,
            "connection-main",
            source_id,
            owner.id,
            first_request.clone(),
        )
        .await
        .expect("historical replay after replacement");
        assert_eq!(historical_replay.task_ref, first.task_ref);
        let asset_count = sqlx::query_scalar::<_, i64>(
            "select count(*)::bigint from asset_items where tenant_id = $1",
        )
        .bind(tenant.id.0)
        .fetch_one(storage.pool())
        .await
        .expect("count assets");
        let object_key = sqlx::query_scalar::<_, Option<String>>(
            "select object_key from asset_items where tenant_id = $1 limit 1",
        )
        .bind(tenant.id.0)
        .fetch_one(storage.pool())
        .await
        .expect("read replacement object key");
        assert_eq!(asset_count, 1);
        assert_eq!(object_key.as_deref(), Some("objects/replacement.png"));

        let mut denied = first_request.clone();
        denied.request_id = "request-denied".to_string();
        denied.dataset_external_ids = vec!["group-denied".to_string()];
        let error = create_external_asset_import_response(
            &state,
            "connection-main",
            source_id,
            owner.id,
            denied,
        )
        .await
        .expect_err("library/dataset mismatch must fail");
        assert_eq!(
            error.payload.code,
            "external_asset_import_library_dataset_mismatch"
        );

        let mut cross_library = first_request.clone();
        cross_library.request_id = "request-other-library-scope".to_string();
        let error = create_external_asset_import_response(
            &state,
            "connection-other",
            source_id,
            owner.id,
            cross_library,
        )
        .await
        .expect_err("another connection must not use the bound library");
        assert_eq!(
            error.payload.code,
            "external_asset_import_library_scope_mismatch"
        );

        let mut cross_connection = first_request;
        cross_connection.request_id = "request-other-connection".to_string();
        let error = create_external_asset_import_response(
            &state,
            "connection-other",
            source_id,
            other_owner.id,
            cross_connection,
        )
        .await
        .expect_err("another connection owner must not see the dataset");
        assert_eq!(
            error.payload.code,
            "external_asset_import_dataset_owner_mismatch"
        );
    }
}
