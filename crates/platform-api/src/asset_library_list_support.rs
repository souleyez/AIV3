use contracts::ListAssetLibrariesResponse;

use crate::{asset_library_view_support::asset_library_view, ApiError, AppState};

pub(crate) async fn list_asset_libraries_response(
    state: &AppState,
) -> std::result::Result<ListAssetLibrariesResponse, ApiError> {
    let asset_libraries = state
        .storage
        .asset_libraries()
        .list_by_tenant(state.tenant_id)
        .await
        .map_err(ApiError::from_storage)?
        .into_iter()
        .map(asset_library_view)
        .collect();

    Ok(ListAssetLibrariesResponse { asset_libraries })
}
