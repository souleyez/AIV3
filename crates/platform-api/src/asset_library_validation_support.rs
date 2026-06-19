use serde_json::{json, Value};
use uuid::Uuid;

use crate::{validate_required, ApiError};

pub(crate) fn parse_asset_library_id(raw: &str) -> std::result::Result<Uuid, ApiError> {
    Uuid::parse_str(raw).map_err(|_| {
        ApiError::bad_request(
            "invalid_asset_library_id",
            format!("{raw} is not a valid UUID"),
        )
    })
}

pub(crate) fn asset_library_not_found_error(asset_library_id: Uuid) -> ApiError {
    ApiError::not_found(
        "asset_library_not_found",
        format!("asset library {asset_library_id} was not found"),
    )
}

pub(crate) fn normalize_asset_library_visibility(
    raw: &str,
) -> std::result::Result<String, ApiError> {
    let visibility = raw.trim().to_ascii_lowercase();
    validate_required("visibility", &visibility)?;
    match visibility.as_str() {
        "private" | "internal" | "public" => Ok(visibility),
        _ => Err(ApiError::bad_request(
            "invalid_asset_library_visibility",
            "visibility must be private, internal, or public".to_string(),
        )),
    }
}

pub(crate) fn normalize_asset_library_metadata(
    metadata: Value,
) -> std::result::Result<Value, ApiError> {
    if metadata.is_null() {
        return Ok(json!({}));
    }
    if metadata.is_object() {
        return Ok(metadata);
    }
    Err(ApiError::bad_request(
        "invalid_asset_library_metadata",
        "metadata must be a JSON object".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_library_validation_parses_uuid_and_masks_bad_ids() {
        let id = Uuid::from_u128(0x1234);

        assert_eq!(parse_asset_library_id(&id.to_string()).unwrap(), id);

        let error = parse_asset_library_id("not-a-uuid").expect_err("invalid UUID should fail");
        assert_eq!(error.payload.code, "invalid_asset_library_id");
        assert!(error.payload.message.contains("not a valid UUID"));
    }

    #[test]
    fn asset_library_validation_builds_not_found_error() {
        let id = Uuid::from_u128(0x5678);
        let error = asset_library_not_found_error(id);

        assert_eq!(error.payload.code, "asset_library_not_found");
        assert!(error.payload.message.contains(&id.to_string()));
    }

    #[test]
    fn asset_library_validation_normalizes_allowed_visibility_values() {
        assert_eq!(
            normalize_asset_library_visibility(" PRIVATE ").unwrap(),
            "private"
        );
        assert_eq!(
            normalize_asset_library_visibility("internal").unwrap(),
            "internal"
        );
        assert_eq!(
            normalize_asset_library_visibility("Public").unwrap(),
            "public"
        );
    }

    #[test]
    fn asset_library_validation_rejects_empty_or_unknown_visibility() {
        let empty_error =
            normalize_asset_library_visibility("   ").expect_err("empty visibility should fail");
        assert_eq!(empty_error.payload.code, "validation_error");

        let unknown_error =
            normalize_asset_library_visibility("partner").expect_err("unknown should fail");
        assert_eq!(
            unknown_error.payload.code,
            "invalid_asset_library_visibility"
        );
    }

    #[test]
    fn asset_library_validation_normalizes_metadata_objects() {
        assert_eq!(
            normalize_asset_library_metadata(Value::Null).unwrap(),
            json!({})
        );
        assert_eq!(
            normalize_asset_library_metadata(json!({"owner": "design"})).unwrap(),
            json!({"owner": "design"})
        );
    }

    #[test]
    fn asset_library_validation_rejects_non_object_metadata() {
        let error =
            normalize_asset_library_metadata(json!(["bad"])).expect_err("arrays should fail");

        assert_eq!(error.payload.code, "invalid_asset_library_metadata");
    }
}
