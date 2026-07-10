use std::collections::{BTreeMap, BTreeSet};

use contracts::{AssetItemView, AssetProfileSupplyHintView};
use domain_model::DatasetId;
use uuid::Uuid;

use crate::{
    asset_library_view_support::{
        asset_item_view, asset_profile_supply_hint_view, asset_profile_supply_inputs,
    },
    asset_profile_supply_support, ApiError, AppState,
};

pub(crate) struct AssetLibraryAuthorizedAssetSupply {
    pub(crate) assets: Vec<AssetItemView>,
    pub(crate) asset_profile_hints: Vec<AssetProfileSupplyHintView>,
    pub(crate) asset_parse_status_counts: BTreeMap<String, usize>,
    pub(crate) asset_parse_run_count: usize,
}

pub(crate) async fn load_asset_library_authorized_asset_supply(
    state: &AppState,
    asset_library_id: Uuid,
    authorized_dataset_ids: &BTreeSet<DatasetId>,
) -> std::result::Result<AssetLibraryAuthorizedAssetSupply, ApiError> {
    if authorized_dataset_ids.is_empty() {
        return Ok(AssetLibraryAuthorizedAssetSupply {
            assets: vec![],
            asset_profile_hints: vec![],
            asset_parse_status_counts: BTreeMap::new(),
            asset_parse_run_count: 0,
        });
    }

    let mut asset_views = Vec::new();
    let mut profile_inputs = Vec::new();
    let mut asset_parse_status_counts = BTreeMap::new();
    let mut asset_parse_run_count = 0usize;
    let direct_assets = state
        .storage
        .asset_items()
        .list_by_asset_library(state.tenant_id, asset_library_id, 100)
        .await
        .map_err(ApiError::from_storage)?;
    let dataset_assets = state
        .storage
        .asset_items()
        .list_by_dataset_ids(
            state.tenant_id,
            &asset_library_authorized_dataset_ids_for_query(authorized_dataset_ids),
            100,
        )
        .await
        .map_err(ApiError::from_storage)?;
    let mut assets_by_id = BTreeMap::new();
    for asset in direct_assets.into_iter().chain(dataset_assets.into_iter()) {
        assets_by_id.entry(asset.id).or_insert(asset);
    }

    let mut authorized_assets = Vec::new();
    for asset in assets_by_id.into_values().take(100) {
        let dataset_memberships = state
            .storage
            .asset_items()
            .list_dataset_memberships(state.tenant_id, asset.id)
            .await
            .map_err(ApiError::from_storage)?;
        if !dataset_memberships
            .iter()
            .any(|membership| authorized_dataset_ids.contains(&membership.dataset_id))
        {
            continue;
        }
        authorized_assets.push(asset);
    }

    let asset_ids = authorized_assets
        .iter()
        .map(|asset| asset.id)
        .collect::<Vec<_>>();
    let mut profiles_by_asset_id = BTreeMap::new();
    for profile in state
        .storage
        .asset_items()
        .list_profiles_by_asset_ids(
            state.tenant_id,
            &asset_ids,
            asset_ids.len().saturating_mul(4),
        )
        .await
        .map_err(ApiError::from_storage)?
    {
        profiles_by_asset_id
            .entry(profile.asset_id)
            .or_insert_with(Vec::new)
            .push(profile);
    }

    for asset in authorized_assets {
        for parse_run in state
            .storage
            .asset_items()
            .list_parse_runs(state.tenant_id, asset.id)
            .await
            .map_err(ApiError::from_storage)?
        {
            asset_parse_run_count = asset_parse_run_count.saturating_add(1);
            let status = parse_run.status.trim();
            if !status.is_empty() {
                *asset_parse_status_counts
                    .entry(status.to_string())
                    .or_insert(0) += 1;
            }
        }
        let profiles = profiles_by_asset_id
            .get(&asset.id)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        profile_inputs.extend(asset_profile_supply_inputs(&asset, profiles));
        asset_views.push(asset_item_view(asset));
    }
    let asset_profile_hints =
        asset_profile_supply_support::build_asset_profile_supply_hints(&profile_inputs, 100)
            .into_iter()
            .map(asset_profile_supply_hint_view)
            .collect::<Vec<_>>();

    Ok(AssetLibraryAuthorizedAssetSupply {
        assets: asset_views,
        asset_profile_hints,
        asset_parse_status_counts,
        asset_parse_run_count,
    })
}

fn asset_library_authorized_dataset_ids_for_query(
    authorized_dataset_ids: &BTreeSet<DatasetId>,
) -> Vec<DatasetId> {
    authorized_dataset_ids.iter().copied().collect()
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    fn dataset_id(value: u128) -> DatasetId {
        DatasetId(Uuid::from_u128(value))
    }

    #[test]
    fn asset_library_asset_supply_query_ids_are_stable_and_sorted() {
        let ids = BTreeSet::from([dataset_id(3), dataset_id(1), dataset_id(2)]);

        assert_eq!(
            asset_library_authorized_dataset_ids_for_query(&ids),
            vec![dataset_id(1), dataset_id(2), dataset_id(3)]
        );
    }

    #[test]
    fn asset_library_asset_supply_query_ids_can_be_empty() {
        assert!(asset_library_authorized_dataset_ids_for_query(&BTreeSet::new()).is_empty());
    }
}
