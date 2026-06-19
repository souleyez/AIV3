use axum::http::HeaderMap;
use contracts::{
    AssetLibraryDatasetMembershipResponse, RemoveAssetLibraryDatasetMembershipResponse,
    UpsertAssetLibraryDatasetMembershipRequest,
};
use domain_model::{DatasetId, UserId};
use storage::{AssetLibraryRecord, NewAssetLibraryDatasetMembership};
use uuid::Uuid;

use crate::{
    asset_library_validation_support::parse_asset_library_id,
    asset_library_view_support::{asset_library_membership_view, asset_library_view},
    id_parse_support::parse_dataset_id,
    load_asset_library, load_visible_dataset_for_user_with_local_scope,
    request_scope_headers::active_secret_binding_ids_from_headers,
    resource_access::ensure_owner_managed_resource,
    text_normalization::trim_optional,
    validate_required, ApiError, AppState,
};

pub(crate) fn new_asset_library_dataset_membership_from_request(
    dataset_id: DatasetId,
    request: UpsertAssetLibraryDatasetMembershipRequest,
) -> std::result::Result<NewAssetLibraryDatasetMembership, ApiError> {
    let role = trim_optional(request.role).unwrap_or_else(|| "member".to_string());
    validate_required("role", &role)?;

    Ok(NewAssetLibraryDatasetMembership {
        dataset_id,
        role,
        priority: request.priority.unwrap_or(100),
    })
}

async fn load_asset_library_for_dataset_membership(
    state: &AppState,
    headers: &HeaderMap,
    current_user_id: UserId,
    asset_library_id: Uuid,
    dataset_id: DatasetId,
) -> std::result::Result<AssetLibraryRecord, ApiError> {
    let asset_library = load_asset_library(state, asset_library_id).await?;
    let active_secret_binding_ids = active_secret_binding_ids_from_headers(headers)?;
    let dataset = load_visible_dataset_for_user_with_local_scope(
        state,
        dataset_id,
        &active_secret_binding_ids,
        Some(current_user_id),
        None,
    )
    .await?;
    ensure_owner_managed_resource(
        "dataset",
        dataset.id.to_string(),
        dataset.owner_user_id,
        Some(current_user_id),
    )?;

    Ok(asset_library)
}

pub(crate) async fn upsert_asset_library_dataset_membership_and_load_response(
    state: &AppState,
    headers: &HeaderMap,
    current_user_id: UserId,
    asset_library_id: Uuid,
    dataset_id: DatasetId,
    request: UpsertAssetLibraryDatasetMembershipRequest,
) -> std::result::Result<AssetLibraryDatasetMembershipResponse, ApiError> {
    let asset_library = load_asset_library_for_dataset_membership(
        state,
        headers,
        current_user_id,
        asset_library_id,
        dataset_id,
    )
    .await?;
    let membership = state
        .storage
        .asset_libraries()
        .upsert_dataset_membership(
            state.tenant_id,
            asset_library_id,
            new_asset_library_dataset_membership_from_request(dataset_id, request)?,
        )
        .await
        .map_err(ApiError::from_storage)?;
    let asset_library = state
        .storage
        .asset_libraries()
        .get_by_id(state.tenant_id, asset_library.id)
        .await
        .map_err(ApiError::from_storage)?
        .unwrap_or(asset_library);

    Ok(AssetLibraryDatasetMembershipResponse {
        asset_library: asset_library_view(asset_library),
        membership: asset_library_membership_view(membership),
    })
}

pub(crate) async fn upsert_asset_library_dataset_membership_path_and_load_response(
    state: &AppState,
    headers: &HeaderMap,
    current_user_id: UserId,
    asset_library_id: &str,
    dataset_id: &str,
    request: UpsertAssetLibraryDatasetMembershipRequest,
) -> std::result::Result<AssetLibraryDatasetMembershipResponse, ApiError> {
    let (asset_library_id, dataset_id) =
        parse_asset_library_dataset_membership_path(asset_library_id, dataset_id)?;
    upsert_asset_library_dataset_membership_and_load_response(
        state,
        headers,
        current_user_id,
        asset_library_id,
        dataset_id,
        request,
    )
    .await
}

pub(crate) async fn remove_asset_library_dataset_membership_and_load_response(
    state: &AppState,
    headers: &HeaderMap,
    current_user_id: UserId,
    asset_library_id: Uuid,
    dataset_id: DatasetId,
) -> std::result::Result<RemoveAssetLibraryDatasetMembershipResponse, ApiError> {
    let asset_library = load_asset_library_for_dataset_membership(
        state,
        headers,
        current_user_id,
        asset_library_id,
        dataset_id,
    )
    .await?;
    let removed = state
        .storage
        .asset_libraries()
        .remove_dataset_membership(state.tenant_id, asset_library_id, dataset_id)
        .await
        .map_err(ApiError::from_storage)?
        .is_some();
    let asset_library = state
        .storage
        .asset_libraries()
        .get_by_id(state.tenant_id, asset_library.id)
        .await
        .map_err(ApiError::from_storage)?
        .unwrap_or(asset_library);

    Ok(RemoveAssetLibraryDatasetMembershipResponse {
        asset_library: asset_library_view(asset_library),
        dataset_id,
        removed,
    })
}

pub(crate) async fn remove_asset_library_dataset_membership_path_and_load_response(
    state: &AppState,
    headers: &HeaderMap,
    current_user_id: UserId,
    asset_library_id: &str,
    dataset_id: &str,
) -> std::result::Result<RemoveAssetLibraryDatasetMembershipResponse, ApiError> {
    let (asset_library_id, dataset_id) =
        parse_asset_library_dataset_membership_path(asset_library_id, dataset_id)?;
    remove_asset_library_dataset_membership_and_load_response(
        state,
        headers,
        current_user_id,
        asset_library_id,
        dataset_id,
    )
    .await
}

fn parse_asset_library_dataset_membership_path(
    asset_library_id: &str,
    dataset_id: &str,
) -> std::result::Result<(Uuid, DatasetId), ApiError> {
    Ok((
        parse_asset_library_id(asset_library_id)?,
        parse_dataset_id(dataset_id)?,
    ))
}

#[cfg(test)]
mod tests {
    use domain_model::DatasetId;
    use uuid::Uuid;

    use super::*;

    fn dataset_id() -> DatasetId {
        DatasetId(Uuid::from_u128(42))
    }

    #[test]
    fn asset_library_membership_support_builds_storage_request() {
        let membership = new_asset_library_dataset_membership_from_request(
            dataset_id(),
            UpsertAssetLibraryDatasetMembershipRequest {
                role: Some(" source ".to_string()),
                priority: Some(20),
            },
        )
        .expect("membership request should build");

        assert_eq!(membership.dataset_id, dataset_id());
        assert_eq!(membership.role, "source");
        assert_eq!(membership.priority, 20);
    }

    #[test]
    fn asset_library_membership_support_defaults_role_and_priority() {
        let membership = new_asset_library_dataset_membership_from_request(
            dataset_id(),
            UpsertAssetLibraryDatasetMembershipRequest {
                role: None,
                priority: None,
            },
        )
        .expect("defaults should build");

        assert_eq!(membership.role, "member");
        assert_eq!(membership.priority, 100);
    }

    #[test]
    fn asset_library_membership_support_defaults_blank_role() {
        let membership = new_asset_library_dataset_membership_from_request(
            dataset_id(),
            UpsertAssetLibraryDatasetMembershipRequest {
                role: Some("   ".to_string()),
                priority: None,
            },
        )
        .expect("blank role should default");

        assert_eq!(membership.role, "member");
        assert_eq!(membership.priority, 100);
    }

    #[test]
    fn asset_library_membership_support_parses_path_ids_in_order() {
        let asset_library_id = Uuid::from_u128(7);
        let dataset_id = Uuid::from_u128(8);

        let parsed = parse_asset_library_dataset_membership_path(
            &asset_library_id.to_string(),
            &dataset_id.to_string(),
        )
        .expect("valid path ids should parse");

        assert_eq!(parsed.0, asset_library_id);
        assert_eq!(parsed.1, DatasetId(dataset_id));

        let asset_error =
            parse_asset_library_dataset_membership_path("not-a-uuid", &dataset_id.to_string())
                .expect_err("invalid asset library id should fail first");
        assert_eq!(asset_error.payload.code, "invalid_asset_library_id");

        let dataset_error = parse_asset_library_dataset_membership_path(
            &asset_library_id.to_string(),
            "not-a-uuid",
        )
        .expect_err("invalid dataset id should fail");
        assert_eq!(dataset_error.payload.code, "invalid_dataset_id");
    }
}
