use contracts::{CreateAssetLibraryRequest, CreateAssetLibraryResponse};
use storage::{AssetLibraryRecord, NewAssetLibrary};

use crate::{
    asset_library_validation_support::{
        normalize_asset_library_metadata, normalize_asset_library_visibility,
    },
    asset_library_view_support::asset_library_view,
    text_normalization::trim_optional,
    validate_required, ApiError, AppState,
};

pub(crate) async fn create_asset_library_and_load_response(
    state: &AppState,
    request: CreateAssetLibraryRequest,
) -> std::result::Result<CreateAssetLibraryResponse, ApiError> {
    validate_create_asset_library_request(&request)?;
    let new_asset_library = new_asset_library_from_request(request)?;

    let asset_library = state
        .storage
        .asset_libraries()
        .create(state.tenant_id, new_asset_library)
        .await
        .map_err(ApiError::from_storage)?;

    Ok(create_asset_library_response(asset_library))
}

pub(crate) fn new_asset_library_from_request(
    request: CreateAssetLibraryRequest,
) -> std::result::Result<NewAssetLibrary, ApiError> {
    let domain = trim_optional(request.domain).unwrap_or_else(|| "general".to_string());
    let visibility =
        normalize_asset_library_visibility(request.visibility.as_deref().unwrap_or("private"))?;
    let metadata = normalize_asset_library_metadata(request.metadata)?;

    Ok(NewAssetLibrary {
        external_id: trim_optional(request.external_id),
        name: request.name.trim().to_string(),
        domain,
        description: trim_optional(request.description),
        visibility,
        metadata,
    })
}

fn validate_create_asset_library_request(
    request: &CreateAssetLibraryRequest,
) -> std::result::Result<(), ApiError> {
    validate_required("name", &request.name)
}

fn create_asset_library_response(asset_library: AssetLibraryRecord) -> CreateAssetLibraryResponse {
    CreateAssetLibraryResponse {
        asset_library: asset_library_view(asset_library),
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use domain_model::TenantId;
    use serde_json::{json, Value};
    use uuid::Uuid;

    use super::*;

    fn timestamp() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 6, 19, 12, 0, 0)
            .single()
            .expect("valid timestamp")
    }

    fn tenant_id() -> TenantId {
        TenantId(Uuid::from_u128(1))
    }

    fn request() -> CreateAssetLibraryRequest {
        CreateAssetLibraryRequest {
            external_id: Some(" fashion-main ".to_string()),
            name: " 服装设计资产库 ".to_string(),
            domain: Some(" fashion_design ".to_string()),
            description: Some(" 设计图库、PPT、视频 ".to_string()),
            visibility: Some(" Internal ".to_string()),
            metadata: json!({"owner": "design"}),
        }
    }

    #[test]
    fn asset_library_create_support_builds_storage_request() {
        let new_asset_library =
            new_asset_library_from_request(request()).expect("asset library request should build");

        assert_eq!(
            new_asset_library.external_id.as_deref(),
            Some("fashion-main")
        );
        assert_eq!(new_asset_library.name, "服装设计资产库");
        assert_eq!(new_asset_library.domain, "fashion_design");
        assert_eq!(
            new_asset_library.description.as_deref(),
            Some("设计图库、PPT、视频")
        );
        assert_eq!(new_asset_library.visibility, "internal");
        assert_eq!(new_asset_library.metadata, json!({"owner": "design"}));
    }

    #[test]
    fn asset_library_create_support_defaults_optional_fields() {
        let mut request = request();
        request.external_id = Some("   ".to_string());
        request.domain = None;
        request.description = Some(" ".to_string());
        request.visibility = None;
        request.metadata = Value::Null;

        let new_asset_library =
            new_asset_library_from_request(request).expect("defaults should build");

        assert!(new_asset_library.external_id.is_none());
        assert_eq!(new_asset_library.domain, "general");
        assert!(new_asset_library.description.is_none());
        assert_eq!(new_asset_library.visibility, "private");
        assert_eq!(new_asset_library.metadata, json!({}));
    }

    #[test]
    fn asset_library_create_support_rejects_bad_metadata() {
        let mut request = request();
        request.metadata = json!(["bad"]);

        let error =
            new_asset_library_from_request(request).expect_err("non-object metadata should fail");

        assert_eq!(error.payload.code, "invalid_asset_library_metadata");
    }

    #[test]
    fn asset_library_create_support_builds_response_view() {
        let id = Uuid::from_u128(2);
        let response = create_asset_library_response(AssetLibraryRecord {
            id,
            tenant_id: tenant_id(),
            external_id: Some("fashion-library".to_string()),
            name: "服装设计资产库".to_string(),
            domain: "fashion".to_string(),
            description: Some("图库".to_string()),
            visibility: "private".to_string(),
            metadata: json!({"owner": "design"}),
            dataset_count: 0,
            created_at: timestamp(),
            updated_at: timestamp(),
        });

        assert_eq!(response.asset_library.id, id.to_string());
        assert_eq!(
            response.asset_library.external_id.as_deref(),
            Some("fashion-library")
        );
        assert_eq!(response.asset_library.name, "服装设计资产库");
        assert_eq!(response.asset_library.metadata["owner"], "design");
    }

    #[test]
    fn asset_library_create_support_keeps_name_required_validation() {
        let mut request = request();
        request.name = "   ".to_string();

        let error =
            validate_create_asset_library_request(&request).expect_err("blank name should fail");

        assert_eq!(error.payload.code, "validation_error");
    }
}
