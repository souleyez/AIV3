use contracts::{CreateDatasetRequest, DatasetSummary};
use domain_model::{DatasetVisibility, SecretScopeLevel, UserId};
use serde_json::{json, Map, Value};
use storage::{NewDataset, NewSecretBinding};

use crate::{
    dataset_secret_binding_support::{
        dataset_secret_binding_fingerprint, dataset_secret_binding_provider_key,
    },
    dataset_summary_support::dataset_summary,
    request_scope_headers::merge_secret_binding_ids,
    text_normalization::trim_optional,
    validate_required, ApiError, AppState, PUBLIC_DATASET_WARNING,
};

pub(crate) fn validate_create_dataset_required_fields(
    request: &CreateDatasetRequest,
) -> std::result::Result<(), ApiError> {
    validate_required("key", &request.key)?;
    validate_required("title", &request.title)?;
    Ok(())
}

pub(crate) async fn create_dataset_and_load_summary(
    state: &AppState,
    current_user_id: Option<UserId>,
    request: CreateDatasetRequest,
) -> std::result::Result<DatasetSummary, ApiError> {
    let input = create_dataset_input_from_request(request, current_user_id)?;
    let dataset = state
        .storage
        .datasets()
        .create_with_metadata(
            state.tenant_id,
            NewDataset {
                key: input.key,
                title: input.title,
                description: input.description,
                owner_user_id: current_user_id,
            },
            Value::Object(input.metadata),
        )
        .await
        .map_err(ApiError::from_storage)?;
    let dataset = if let Some(secret) = input.secret_binding {
        let binding = state
            .storage
            .secret_bindings()
            .create(
                state.tenant_id,
                NewSecretBinding {
                    dataset_id: dataset.id,
                    document_id: None,
                    scope_level: SecretScopeLevel::Dataset,
                    provider_key: secret.provider_key,
                    cipher_text: "local-only".to_string(),
                    fingerprint: secret.fingerprint,
                },
            )
            .await
            .map_err(ApiError::from_storage)?;
        let next_secret_binding_ids =
            merge_secret_binding_ids(&dataset.default_secret_binding_ids, &[binding.id]);
        state
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
            .map_err(ApiError::from_storage)?
    } else {
        dataset
    };

    Ok(dataset_summary(dataset, input.access_warning))
}

#[derive(Debug)]
struct CreateDatasetInput {
    key: String,
    title: String,
    description: Option<String>,
    metadata: Map<String, Value>,
    secret_binding: Option<CreateDatasetSecretBindingInput>,
    access_warning: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct CreateDatasetSecretBindingInput {
    fingerprint: String,
    provider_key: String,
}

fn create_dataset_input_from_request(
    request: CreateDatasetRequest,
    current_user_id: Option<UserId>,
) -> std::result::Result<CreateDatasetInput, ApiError> {
    validate_create_dataset_required_fields(&request)?;
    let secret_fingerprint = trim_optional(request.secret_fingerprint);
    let requested_secret_binding_ids = request.secret_binding_ids.clone();
    let visibility = if requested_secret_binding_ids.is_empty() && secret_fingerprint.is_none() {
        request.visibility.unwrap_or(if current_user_id.is_some() {
            DatasetVisibility::Private
        } else {
            DatasetVisibility::Public
        })
    } else {
        request.visibility.unwrap_or(DatasetVisibility::Private)
    };
    let anonymous_local_only = current_user_id.is_none() && request.local_only;
    let access_warning = (visibility == DatasetVisibility::Public && !anonymous_local_only)
        .then(|| PUBLIC_DATASET_WARNING.to_string());
    let local_thread_id = trim_optional(request.local_thread_id);
    let mut metadata = Map::new();
    metadata.insert("visibility".to_string(), json!(visibility.as_str()));
    metadata.insert(
        "default_secret_binding_ids".to_string(),
        json!(requested_secret_binding_ids),
    );
    if anonymous_local_only {
        metadata.insert("local_only".to_string(), json!(true));
        if let Some(local_thread_id) = local_thread_id.as_deref() {
            metadata.insert("local_thread_id".to_string(), json!(local_thread_id));
        }
    }
    let secret_binding = secret_fingerprint
        .map(|fingerprint| {
            Ok(CreateDatasetSecretBindingInput {
                fingerprint: dataset_secret_binding_fingerprint(&fingerprint)?,
                provider_key: dataset_secret_binding_provider_key(request.secret_label),
            })
        })
        .transpose()?;

    Ok(CreateDatasetInput {
        key: request.key.trim().to_string(),
        title: request.title.trim().to_string(),
        description: trim_optional(request.description),
        metadata,
        secret_binding,
        access_warning,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> CreateDatasetRequest {
        CreateDatasetRequest {
            key: " dataset-key ".to_string(),
            title: " Dataset Title ".to_string(),
            description: Some(" Dataset description ".to_string()),
            visibility: None,
            local_only: false,
            local_thread_id: None,
            secret_binding_ids: Vec::new(),
            secret_fingerprint: None,
            secret_label: None,
        }
    }

    #[test]
    fn create_dataset_input_defaults_logged_in_dataset_to_private() {
        let input = create_dataset_input_from_request(request(), Some(UserId::new()))
            .expect("input should build");

        assert_eq!(input.key, "dataset-key");
        assert_eq!(input.title, "Dataset Title");
        assert_eq!(input.description.as_deref(), Some("Dataset description"));
        assert_eq!(input.metadata["visibility"], json!("private"));
        assert_eq!(input.metadata["default_secret_binding_ids"], json!([]));
        assert_eq!(input.access_warning, None);
        assert_eq!(input.secret_binding, None);
    }

    #[test]
    fn create_dataset_input_keeps_anonymous_local_only_thread_scope_without_warning() {
        let mut request = request();
        request.local_only = true;
        request.local_thread_id = Some(" thread-1 ".to_string());

        let input = create_dataset_input_from_request(request, None).expect("input should build");

        assert_eq!(input.metadata["visibility"], json!("public"));
        assert_eq!(input.metadata["local_only"], json!(true));
        assert_eq!(input.metadata["local_thread_id"], json!("thread-1"));
        assert_eq!(input.access_warning, None);
    }

    #[test]
    fn create_dataset_input_prepares_secret_binding_and_private_default() {
        let mut request = request();
        request.secret_fingerprint = Some(" local-fingerprint ".to_string());
        request.secret_label = Some(" local-label ".to_string());

        let input = create_dataset_input_from_request(request, None).expect("input should build");

        assert_eq!(input.metadata["visibility"], json!("private"));
        assert_eq!(
            input.secret_binding,
            Some(CreateDatasetSecretBindingInput {
                fingerprint: "local-fingerprint".to_string(),
                provider_key: "local-label".to_string(),
            })
        );
    }
}
