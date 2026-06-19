use axum::http::HeaderMap;
use domain_model::{DatasetId, UserId};
use uuid::Uuid;

use crate::{
    load_asset_library, load_visible_dataset_for_user_with_local_scope,
    request_scope_headers::{active_secret_binding_ids_from_headers, local_thread_id_from_headers},
    ApiError, AppState,
};

pub(crate) async fn validate_v3_client_scope_refs(
    state: &AppState,
    headers: &HeaderMap,
    current_user_id: Option<UserId>,
    dataset_ids: &[String],
    asset_library_ids: &[String],
) -> std::result::Result<(), ApiError> {
    let active_secret_binding_ids = active_secret_binding_ids_from_headers(headers)?;
    let local_thread_id = local_thread_id_from_headers(headers);
    for dataset_uuid in v3_client_scope_uuid_refs(dataset_ids) {
        load_visible_dataset_for_user_with_local_scope(
            state,
            DatasetId(dataset_uuid),
            &active_secret_binding_ids,
            current_user_id,
            local_thread_id.as_deref(),
        )
        .await?;
    }
    for asset_library_id in v3_client_scope_uuid_refs(asset_library_ids) {
        load_asset_library(state, asset_library_id).await?;
    }
    Ok(())
}

pub(crate) fn v3_client_scope_uuid_refs(values: &[String]) -> Vec<Uuid> {
    values
        .iter()
        .filter_map(|value| Uuid::parse_str(value.trim()).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_artifact_scope_support_extracts_trimmed_uuid_refs() {
        let uuid =
            Uuid::parse_str("00000000-0000-0000-0000-000000000042").expect("valid fixture uuid");

        assert_eq!(
            v3_client_scope_uuid_refs(&[
                " 00000000-0000-0000-0000-000000000042 ".to_string(),
                "dataset-external-id".to_string(),
                "".to_string(),
            ]),
            vec![uuid]
        );
    }

    #[test]
    fn client_artifact_scope_support_ignores_non_uuid_external_refs() {
        assert!(v3_client_scope_uuid_refs(&[
            "asset-library-external-id".to_string(),
            "third-party-source-main".to_string(),
        ])
        .is_empty());
    }
}
