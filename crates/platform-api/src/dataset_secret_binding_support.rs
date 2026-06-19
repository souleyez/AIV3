use contracts::{
    CreateDatasetSecretBindingRequest, CreateDatasetSecretBindingResponse, DatasetSummary,
    ResolveDatasetSecretBindingsRequest, ResolveDatasetSecretBindingsResponse,
};
use domain_model::{DatasetVisibility, SecretBindingId, SecretScopeLevel, UserId};
use serde_json::json;
use storage::NewSecretBinding;

use crate::{
    dataset_summary_support::dataset_summary, load_visible_dataset_for_user,
    request_scope_headers::merge_secret_binding_ids, text_normalization::trim_optional,
    validate_required, ApiError, AppState,
};

pub(crate) async fn create_dataset_secret_binding_response(
    state: &AppState,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
    request: CreateDatasetSecretBindingRequest,
) -> std::result::Result<CreateDatasetSecretBindingResponse, ApiError> {
    let fingerprint = dataset_secret_binding_fingerprint(&request.fingerprint)?;
    let dataset = load_visible_dataset_for_user(
        state,
        request.dataset_id,
        active_secret_binding_ids,
        current_user_id,
    )
    .await?;
    let label = dataset_secret_binding_provider_key(request.label);
    let binding = state
        .storage
        .secret_bindings()
        .create(
            state.tenant_id,
            NewSecretBinding {
                dataset_id: dataset.id,
                document_id: None,
                scope_level: SecretScopeLevel::Dataset,
                provider_key: label,
                cipher_text: "local-only".to_string(),
                fingerprint,
            },
        )
        .await
        .map_err(ApiError::from_storage)?;
    let next_secret_binding_ids =
        merge_secret_binding_ids(&dataset.default_secret_binding_ids, &[binding.id]);
    let updated_dataset = state
        .storage
        .datasets()
        .update_metadata(
            state.tenant_id,
            dataset.id,
            &json!({
                "visibility": DatasetVisibility::Private.as_str(),
                "default_secret_binding_ids": next_secret_binding_ids,
            }),
        )
        .await
        .map_err(ApiError::from_storage)?;
    let active_secret_binding_ids =
        merge_secret_binding_ids(active_secret_binding_ids, &[binding.id]);

    Ok(CreateDatasetSecretBindingResponse {
        dataset: dataset_summary(updated_dataset, None),
        secret_binding_id: binding.id,
        active_secret_binding_ids,
    })
}

pub(crate) async fn resolve_dataset_secret_bindings_response(
    state: &AppState,
    request: ResolveDatasetSecretBindingsRequest,
) -> std::result::Result<ResolveDatasetSecretBindingsResponse, ApiError> {
    let fingerprint = dataset_secret_binding_fingerprint(&request.fingerprint)?;
    let bindings = state
        .storage
        .secret_bindings()
        .list_by_fingerprint(state.tenant_id, &fingerprint)
        .await
        .map_err(ApiError::from_storage)?;
    let secret_binding_ids = bindings.iter().map(|binding| binding.id).collect();
    let mut datasets = Vec::new();
    for binding in bindings {
        if datasets
            .iter()
            .any(|dataset: &DatasetSummary| dataset.id == binding.dataset_id)
        {
            continue;
        }
        if let Some(dataset) = state
            .storage
            .datasets()
            .get_by_id(state.tenant_id, binding.dataset_id)
            .await
            .map_err(ApiError::from_storage)?
        {
            datasets.push(dataset_summary(dataset, None));
        }
    }

    Ok(ResolveDatasetSecretBindingsResponse {
        secret_binding_ids,
        datasets,
    })
}

pub(crate) fn dataset_secret_binding_fingerprint(
    raw: &str,
) -> std::result::Result<String, ApiError> {
    validate_required("fingerprint", raw)?;
    Ok(raw.trim().to_string())
}

pub(crate) fn dataset_secret_binding_provider_key(label: Option<String>) -> String {
    trim_optional(label).unwrap_or_else(|| "local-browser-key".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_dataset_secret_binding_request_uses_default_label_for_blank_input() {
        assert_eq!(
            dataset_secret_binding_provider_key(Some("   ".to_string())),
            "local-browser-key"
        );
    }

    #[test]
    fn create_dataset_secret_binding_request_trims_fingerprint_for_storage() {
        assert_eq!(
            dataset_secret_binding_fingerprint(" local-fingerprint ")
                .expect("fingerprint should normalize"),
            "local-fingerprint"
        );
    }

    #[test]
    fn create_dataset_secret_binding_request_rejects_blank_fingerprint() {
        let error =
            dataset_secret_binding_fingerprint("   ").expect_err("blank fingerprint should fail");

        assert_eq!(error.payload.code, "validation_error");
        assert!(error.payload.message.contains("fingerprint"));
    }
}
