use std::collections::{BTreeMap, BTreeSet};

use contracts::{
    AssetItemView, AssetLibraryDatasetMembershipView, AssetLibraryScopeSummaryResponse,
    AssetLibraryScopeSummaryView, AssetLibraryView, AssetProfileSupplyHintView, DatasetSummary,
};
use domain_model::{Dataset, DatasetId, DatasetLifecycle, SecretBindingId, UserId};
use serde_json::{json, Value};
use storage::{AssetLibraryDatasetMembershipRecord, AssetLibraryRecord};
use uuid::Uuid;

use crate::{
    asset_library_asset_supply_support::load_asset_library_authorized_asset_supply,
    asset_library_load_support::load_asset_library,
    asset_library_scope_support,
    asset_library_validation_support::parse_asset_library_id,
    asset_library_view_support::{asset_library_membership_view, asset_library_view},
    dataset_summary_support::dataset_summary,
    resource_access::filter_visible_datasets,
    ApiError, AppState,
};

pub(crate) const ASSET_LIBRARY_SCOPE_POLICY: &str =
    "asset_library_memberships_intersect_authorized_datasets";

pub(crate) struct AssetLibraryScopeSummaryResponseInput {
    pub(crate) asset_library: AssetLibraryView,
    pub(crate) memberships: Vec<AssetLibraryDatasetMembershipView>,
    pub(crate) datasets: Vec<DatasetSummary>,
    pub(crate) dataset_ids: Vec<DatasetId>,
    pub(crate) assets: Vec<AssetItemView>,
    pub(crate) asset_profile_hints: Vec<AssetProfileSupplyHintView>,
    pub(crate) asset_parse_status_counts: BTreeMap<String, usize>,
    pub(crate) asset_parse_run_count: usize,
    pub(crate) denied_dataset_count: usize,
    pub(crate) membership_count: usize,
    pub(crate) authorized_dataset_count: usize,
}

pub(crate) fn asset_library_scope_summary_response(
    input: AssetLibraryScopeSummaryResponseInput,
) -> AssetLibraryScopeSummaryResponse {
    let assets = asset_library_scope_summary_safe_asset_views(input.assets);
    let asset_count = assets.len();
    let asset_profile_hint_count = input.asset_profile_hints.len();

    AssetLibraryScopeSummaryResponse {
        summary: AssetLibraryScopeSummaryView {
            asset_library: input.asset_library,
            memberships: input.memberships,
            datasets: input.datasets,
            dataset_ids: input.dataset_ids,
            assets,
            asset_profile_hints: input.asset_profile_hints,
            denied_dataset_count: input.denied_dataset_count,
            membership_count: input.membership_count,
            authorized_dataset_count: input.authorized_dataset_count,
            asset_count,
            asset_profile_hint_count,
            asset_parse_status_counts: input.asset_parse_status_counts,
            asset_parse_run_count: input.asset_parse_run_count,
            scope_policy: ASSET_LIBRARY_SCOPE_POLICY.to_string(),
        },
    }
}

fn asset_library_scope_summary_safe_asset_views(assets: Vec<AssetItemView>) -> Vec<AssetItemView> {
    assets
        .into_iter()
        .map(asset_library_scope_summary_safe_asset_view)
        .collect()
}

fn asset_library_scope_summary_safe_asset_view(mut asset: AssetItemView) -> AssetItemView {
    let metadata = asset_library_scope_summary_safe_asset_metadata(&asset);
    asset.source_id = None;
    asset.object_key = None;
    asset.metadata = metadata;
    asset
}

fn asset_library_scope_summary_safe_asset_metadata(asset: &AssetItemView) -> Value {
    json!({
        "storage_locator_present": asset.object_key.as_ref().is_some_and(|value| !value.trim().is_empty()),
        "source_id_present": asset.source_id.as_ref().is_some_and(|value| !value.trim().is_empty()),
        "metadata_present": asset_library_scope_summary_metadata_present(&asset.metadata),
    })
}

fn asset_library_scope_summary_metadata_present(metadata: &Value) -> bool {
    match metadata {
        Value::Null => false,
        Value::Object(object) => !object.is_empty(),
        _ => true,
    }
}

pub(crate) async fn load_asset_library_scope_summary_response(
    state: &AppState,
    asset_library_id: Uuid,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: UserId,
    local_thread_id: Option<&str>,
) -> std::result::Result<AssetLibraryScopeSummaryResponse, ApiError> {
    let asset_library = load_asset_library(state, asset_library_id).await?;
    let memberships = state
        .storage
        .asset_libraries()
        .list_dataset_memberships(state.tenant_id, asset_library_id)
        .await
        .map_err(ApiError::from_storage)?;
    let visible_datasets = filter_visible_datasets(
        state
            .storage
            .datasets()
            .list_by_tenant(state.tenant_id)
            .await
            .map_err(ApiError::from_storage)?,
        active_secret_binding_ids,
        Some(current_user_id),
        local_thread_id,
    );
    let authorized_dataset_ids =
        authorized_dataset_ids_for_scope_summary(&memberships, &visible_datasets);
    let supply = load_asset_library_authorized_asset_supply(
        state,
        asset_library_id,
        &authorized_dataset_ids,
    )
    .await?;

    Ok(asset_library_scope_summary_response(
        asset_library_scope_summary_input_from_records(
            asset_library,
            memberships,
            visible_datasets,
            supply.assets,
            supply.asset_profile_hints,
            supply.asset_parse_status_counts,
            supply.asset_parse_run_count,
        ),
    ))
}

fn authorized_dataset_ids_for_scope_summary(
    memberships: &[AssetLibraryDatasetMembershipRecord],
    visible_datasets: &[Dataset],
) -> BTreeSet<DatasetId> {
    let visible_dataset_ids = visible_datasets
        .iter()
        .filter(|dataset| dataset.lifecycle != DatasetLifecycle::Archived)
        .map(|dataset| dataset.id)
        .collect::<Vec<_>>();
    let scope_memberships = memberships
        .iter()
        .map(
            |membership| asset_library_scope_support::AssetLibraryDatasetMembership {
                dataset_id: membership.dataset_id,
                role: membership.role.clone(),
                priority: membership.priority,
            },
        )
        .collect::<Vec<_>>();
    asset_library_scope_support::resolve_asset_library_dataset_scope(
        &scope_memberships,
        &visible_dataset_ids,
    )
    .dataset_ids
    .into_iter()
    .collect()
}

fn asset_library_scope_summary_input_from_records(
    asset_library: AssetLibraryRecord,
    memberships: Vec<AssetLibraryDatasetMembershipRecord>,
    visible_datasets: Vec<Dataset>,
    assets: Vec<AssetItemView>,
    asset_profile_hints: Vec<AssetProfileSupplyHintView>,
    asset_parse_status_counts: BTreeMap<String, usize>,
    asset_parse_run_count: usize,
) -> AssetLibraryScopeSummaryResponseInput {
    let visible_datasets = visible_datasets
        .into_iter()
        .filter(|dataset| dataset.lifecycle != DatasetLifecycle::Archived)
        .collect::<Vec<_>>();
    let visible_dataset_ids = visible_datasets
        .iter()
        .map(|dataset| dataset.id)
        .collect::<Vec<_>>();
    let scope_memberships = memberships
        .iter()
        .map(
            |membership| asset_library_scope_support::AssetLibraryDatasetMembership {
                dataset_id: membership.dataset_id,
                role: membership.role.clone(),
                priority: membership.priority,
            },
        )
        .collect::<Vec<_>>();
    let scope = asset_library_scope_support::resolve_asset_library_dataset_scope(
        &scope_memberships,
        &visible_dataset_ids,
    );
    let authorized_dataset_ids = scope.dataset_ids.iter().copied().collect::<BTreeSet<_>>();
    let visible_memberships = memberships
        .into_iter()
        .filter(|membership| authorized_dataset_ids.contains(&membership.dataset_id))
        .map(asset_library_membership_view)
        .collect::<Vec<_>>();
    let datasets = visible_datasets
        .into_iter()
        .filter(|dataset| authorized_dataset_ids.contains(&dataset.id))
        .map(|dataset| dataset_summary(dataset, None))
        .collect::<Vec<_>>();

    AssetLibraryScopeSummaryResponseInput {
        asset_library: asset_library_view(asset_library),
        memberships: visible_memberships,
        datasets,
        dataset_ids: scope.dataset_ids,
        assets,
        asset_profile_hints,
        asset_parse_status_counts,
        asset_parse_run_count,
        denied_dataset_count: scope.denied_dataset_ids.len(),
        membership_count: scope.membership_count,
        authorized_dataset_count: authorized_dataset_ids.len(),
    }
}

pub(crate) fn parse_asset_library_scope_summary_path(
    asset_library_id: &str,
) -> std::result::Result<Uuid, ApiError> {
    parse_asset_library_id(asset_library_id)
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use domain_model::{DatasetId, DatasetLifecycle, DatasetVisibility, TenantId};
    use serde_json::json;
    use std::collections::BTreeMap;

    use super::*;

    fn timestamp() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 6, 19, 10, 0, 0)
            .single()
            .expect("valid timestamp")
    }

    fn dataset_id(value: u128) -> DatasetId {
        DatasetId(Uuid::from_u128(value))
    }

    fn tenant_id() -> TenantId {
        TenantId(Uuid::from_u128(10))
    }

    fn asset_library_record() -> AssetLibraryRecord {
        AssetLibraryRecord {
            id: Uuid::from_u128(1),
            tenant_id: tenant_id(),
            external_id: Some("fashion-main".to_string()),
            name: "服装设计资产库".to_string(),
            domain: "fashion".to_string(),
            description: Some("图库、视频、PPT".to_string()),
            visibility: "private".to_string(),
            metadata: json!({"owner": "design"}),
            dataset_count: 3,
            created_at: timestamp(),
            updated_at: timestamp(),
        }
    }

    fn dataset(id: DatasetId, lifecycle: DatasetLifecycle, title: &str) -> Dataset {
        Dataset {
            id,
            tenant_id: tenant_id(),
            owner_user_id: None,
            key: format!("dataset-{}", id.0),
            title: title.to_string(),
            description: None,
            lifecycle,
            visibility: DatasetVisibility::Private,
            default_secret_binding_ids: Vec::new(),
            metadata: BTreeMap::new(),
            created_at: timestamp(),
            updated_at: timestamp(),
        }
    }

    fn membership(
        asset_library_id: Uuid,
        dataset_id: DatasetId,
    ) -> AssetLibraryDatasetMembershipRecord {
        AssetLibraryDatasetMembershipRecord {
            tenant_id: tenant_id(),
            asset_library_id,
            dataset_id,
            role: "member".to_string(),
            priority: 100,
            created_at: timestamp(),
        }
    }

    fn library_view() -> AssetLibraryView {
        AssetLibraryView {
            id: Uuid::from_u128(1).to_string(),
            external_id: Some("fashion-main".to_string()),
            name: "服装设计资产库".to_string(),
            domain: "fashion".to_string(),
            description: Some("图库、视频、PPT".to_string()),
            visibility: "private".to_string(),
            metadata: json!({"owner": "design"}),
            dataset_count: 2,
            created_at: timestamp(),
            updated_at: timestamp(),
        }
    }

    #[test]
    fn scope_summary_input_filters_archived_and_denied_memberships() {
        let asset_library_id = Uuid::from_u128(1);
        let visible_dataset_id = dataset_id(2);
        let hidden_dataset_id = dataset_id(3);
        let archived_dataset_id = dataset_id(4);

        let input = asset_library_scope_summary_input_from_records(
            asset_library_record(),
            vec![
                membership(asset_library_id, visible_dataset_id),
                membership(asset_library_id, hidden_dataset_id),
                membership(asset_library_id, archived_dataset_id),
            ],
            vec![
                dataset(visible_dataset_id, DatasetLifecycle::Active, "可见资料"),
                dataset(
                    archived_dataset_id,
                    DatasetLifecycle::Archived,
                    "已归档资料",
                ),
            ],
            Vec::new(),
            Vec::new(),
            BTreeMap::new(),
            0,
        );
        let response = asset_library_scope_summary_response(input);

        assert_eq!(response.summary.membership_count, 3);
        assert_eq!(response.summary.authorized_dataset_count, 1);
        assert_eq!(response.summary.denied_dataset_count, 2);
        assert_eq!(response.summary.dataset_ids, vec![visible_dataset_id]);
        assert_eq!(response.summary.datasets.len(), 1);
        assert_eq!(response.summary.datasets[0].title, "可见资料");
        assert_eq!(response.summary.memberships.len(), 1);
        assert_eq!(
            response.summary.memberships[0].dataset_id,
            visible_dataset_id
        );
    }

    #[test]
    fn scope_summary_response_sets_counts_and_policy() {
        let response =
            asset_library_scope_summary_response(AssetLibraryScopeSummaryResponseInput {
                asset_library: library_view(),
                memberships: vec![AssetLibraryDatasetMembershipView {
                    asset_library_id: Uuid::from_u128(1).to_string(),
                    dataset_id: dataset_id(2),
                    role: "source".to_string(),
                    priority: 10,
                    created_at: timestamp(),
                }],
                datasets: vec![DatasetSummary {
                    id: dataset_id(2),
                    key: "fashion-dataset".to_string(),
                    title: "服装数据集".to_string(),
                    lifecycle: DatasetLifecycle::Active,
                    visibility: DatasetVisibility::Private,
                    secret_binding_ids: vec![],
                    document_count: Some(1),
                    documents_count: Some(1),
                    estimated_word_count: None,
                    parse_status_summary: None,
                    content_type_summary: None,
                    latest_upload: None,
                    document_title_hints: vec![],
                    material_hints: vec![],
                    noun_term_hints: vec![],
                    section_title_hints: vec![],
                    document_understanding_strategies: vec![],
                    access_warning: None,
                }],
                dataset_ids: vec![dataset_id(2)],
                assets: vec![AssetItemView {
                    id: Uuid::from_u128(4).to_string(),
                    asset_library_id: Some(Uuid::from_u128(1).to_string()),
                    collection_id: None,
                    external_id: None,
                    title: "连衣裙灵感图".to_string(),
                    asset_kind: "image".to_string(),
                    source_kind: "upload".to_string(),
                    source_id: Some("objects/private/source-doc-1.png".to_string()),
                    content_type: Some("image/png".to_string()),
                    object_key: Some("objects/private/look-001.png".to_string()),
                    metadata: json!({"raw_provider_payload": {"should_not_surface": true}}),
                    profile_count: 1,
                    created_at: timestamp(),
                    updated_at: timestamp(),
                }],
                asset_profile_hints: vec![AssetProfileSupplyHintView {
                    asset_id: Uuid::from_u128(4).to_string(),
                    title: "连衣裙灵感图".to_string(),
                    asset_kind: "image".to_string(),
                    source_kind: "upload".to_string(),
                    profile_kind: "image_semantic".to_string(),
                    summary: "春夏连衣裙".to_string(),
                    noun_terms: vec!["连衣裙".to_string()],
                    facets: vec!["季节: 春夏".to_string()],
                }],
                asset_parse_status_counts: BTreeMap::from([
                    ("completed".to_string(), 1usize),
                    ("pending".to_string(), 2usize),
                ]),
                asset_parse_run_count: 3,
                denied_dataset_count: 1,
                membership_count: 2,
                authorized_dataset_count: 1,
            });

        assert_eq!(response.summary.asset_count, 1);
        assert_eq!(response.summary.assets.len(), 1);
        assert_eq!(response.summary.assets[0].source_id, None);
        assert_eq!(response.summary.assets[0].object_key, None);
        assert_eq!(
            response.summary.assets[0].metadata["storage_locator_present"],
            json!(true)
        );
        assert_eq!(
            response.summary.assets[0].metadata["source_id_present"],
            json!(true)
        );
        assert_eq!(
            response.summary.assets[0].metadata["metadata_present"],
            json!(true)
        );
        let serialized_asset =
            serde_json::to_string(&response.summary.assets[0]).expect("asset should serialize");
        assert!(!serialized_asset.contains("objects/private"));
        assert!(!serialized_asset.contains("raw_provider_payload"));
        assert_eq!(response.summary.asset_profile_hint_count, 1);
        assert_eq!(response.summary.asset_parse_run_count, 3);
        assert_eq!(
            response.summary.asset_parse_status_counts.get("pending"),
            Some(&2)
        );
        assert_eq!(response.summary.denied_dataset_count, 1);
        assert_eq!(response.summary.membership_count, 2);
        assert_eq!(response.summary.authorized_dataset_count, 1);
        assert_eq!(response.summary.scope_policy, ASSET_LIBRARY_SCOPE_POLICY);
    }

    #[test]
    fn scope_summary_path_parser_keeps_asset_library_id_validation() {
        let asset_library_id = Uuid::from_u128(9);

        assert_eq!(
            parse_asset_library_scope_summary_path(&asset_library_id.to_string())
                .expect("valid asset library id should parse"),
            asset_library_id
        );

        let error = parse_asset_library_scope_summary_path("not-a-uuid")
            .expect_err("invalid asset library id should fail");
        assert_eq!(error.payload.code, "invalid_asset_library_id");
    }
}
