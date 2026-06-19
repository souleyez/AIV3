use contracts::CreateAssetLibraryRequest;
use storage::NewAssetLibrary;

use crate::{
    asset_library_validation_support::{
        normalize_asset_library_metadata, normalize_asset_library_visibility,
    },
    text_normalization::trim_optional,
    ApiError,
};

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

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::*;

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
}
