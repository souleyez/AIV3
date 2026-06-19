use storage::AssetLibraryRecord;
use uuid::Uuid;

use crate::{asset_library_validation_support::asset_library_not_found_error, ApiError, AppState};

pub(crate) async fn load_asset_library(
    state: &AppState,
    asset_library_id: Uuid,
) -> std::result::Result<AssetLibraryRecord, ApiError> {
    let asset_library = state
        .storage
        .asset_libraries()
        .get_by_id(state.tenant_id, asset_library_id)
        .await
        .map_err(ApiError::from_storage)?;

    asset_library_record_or_not_found(asset_library_id, asset_library)
}

fn asset_library_record_or_not_found(
    asset_library_id: Uuid,
    asset_library: Option<AssetLibraryRecord>,
) -> std::result::Result<AssetLibraryRecord, ApiError> {
    asset_library.ok_or_else(|| asset_library_not_found_error(asset_library_id))
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use domain_model::TenantId;
    use serde_json::json;

    use super::*;

    fn timestamp() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 6, 19, 11, 0, 0)
            .single()
            .expect("valid timestamp")
    }

    fn tenant_id() -> TenantId {
        TenantId(Uuid::from_u128(1))
    }

    fn asset_library_record(id: Uuid) -> AssetLibraryRecord {
        AssetLibraryRecord {
            id,
            tenant_id: tenant_id(),
            external_id: Some("fashion-library".to_string()),
            name: "服装设计资产库".to_string(),
            domain: "fashion".to_string(),
            description: None,
            visibility: "private".to_string(),
            metadata: json!({}),
            dataset_count: 0,
            created_at: timestamp(),
            updated_at: timestamp(),
        }
    }

    #[test]
    fn asset_library_load_support_returns_existing_record() {
        let id = Uuid::from_u128(2);
        let asset_library = asset_library_record_or_not_found(id, Some(asset_library_record(id)))
            .expect("existing asset library should load");

        assert_eq!(asset_library.id, id);
        assert_eq!(asset_library.name, "服装设计资产库");
    }

    #[test]
    fn asset_library_load_support_maps_missing_record_to_not_found() {
        let id = Uuid::from_u128(3);
        let error =
            asset_library_record_or_not_found(id, None).expect_err("missing record should fail");

        assert_eq!(error.payload.code, "asset_library_not_found");
        assert!(error.payload.message.contains(&id.to_string()));
    }
}
