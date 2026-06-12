use crate::ApiError;
use domain_model::{Dataset, DatasetVisibility, SecretBindingId, UserId};
use serde_json::Value;

pub(crate) fn owner_user_id_is_visible(
    owner_user_id: Option<UserId>,
    current_user_id: Option<UserId>,
) -> bool {
    owner_user_id.is_none() || owner_user_id == current_user_id
}

pub(crate) fn ensure_owner_managed_resource(
    resource_kind: &'static str,
    resource_id: String,
    owner_user_id: Option<UserId>,
    current_user_id: Option<UserId>,
) -> std::result::Result<(), ApiError> {
    if owner_user_id.is_some() && owner_user_id != current_user_id {
        return Err(ApiError::not_found(
            &format!("{resource_kind}_not_found"),
            format!("{resource_kind} {resource_id} was not found"),
        ));
    }
    Ok(())
}

pub(crate) fn dataset_is_visible(
    dataset: &Dataset,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
) -> bool {
    dataset
        .owner_user_id
        .is_some_and(|owner_user_id| Some(owner_user_id) == current_user_id)
        || (dataset.owner_user_id.is_none() && dataset.visibility == DatasetVisibility::Public)
        || dataset.default_secret_binding_ids.iter().any(|secret_id| {
            active_secret_binding_ids
                .iter()
                .any(|active_id| active_id == secret_id)
        })
}

pub(crate) fn dataset_local_scope_is_visible(
    dataset: &Dataset,
    local_thread_id: Option<&str>,
) -> bool {
    let local_only = dataset
        .metadata
        .get("local_only")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !local_only || dataset.owner_user_id.is_some() {
        return true;
    }
    dataset
        .metadata
        .get("local_thread_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .is_some_and(|dataset_thread_id| Some(dataset_thread_id) == local_thread_id)
}

pub(crate) fn dataset_is_visible_by_local_thread_scope(
    dataset: &Dataset,
    local_thread_id: Option<&str>,
) -> bool {
    dataset.owner_user_id.is_none()
        && dataset
            .metadata
            .get("local_only")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        && dataset
            .metadata
            .get("local_thread_id")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .is_some_and(|dataset_thread_id| Some(dataset_thread_id) == local_thread_id)
}

pub(crate) fn dataset_is_visible_for_request(
    dataset: &Dataset,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
    local_thread_id: Option<&str>,
) -> bool {
    (dataset_is_visible(dataset, active_secret_binding_ids, current_user_id)
        && dataset_local_scope_is_visible(dataset, local_thread_id))
        || dataset_is_visible_by_local_thread_scope(dataset, local_thread_id)
}

pub(crate) fn dataset_is_hidden_from_standard_dataset_list(dataset: &Dataset) -> bool {
    dataset_is_external_temporary_scope(dataset)
        || dataset_is_system_external_document_parse_source(dataset)
}

pub(crate) fn dataset_is_system_external_document_parse_source(dataset: &Dataset) -> bool {
    if dataset.key.starts_with("external-parse-dataset-") {
        return true;
    }
    dataset
        .metadata
        .get("external_source")
        .or_else(|| dataset.metadata.get("externalSource"))
        .and_then(Value::as_object)
        .and_then(|external_source| {
            external_source
                .get("created_by")
                .or_else(|| external_source.get("createdBy"))
        })
        .and_then(Value::as_str)
        == Some("external_document_parse")
}

pub(crate) fn dataset_is_external_temporary_scope(dataset: &Dataset) -> bool {
    dataset.metadata.get("scope_kind").and_then(Value::as_str) == Some("external_temporary")
}

pub(crate) fn filter_visible_datasets(
    datasets: Vec<Dataset>,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
    local_thread_id: Option<&str>,
) -> Vec<Dataset> {
    datasets
        .into_iter()
        .filter(|dataset| {
            dataset_is_visible_for_request(
                dataset,
                active_secret_binding_ids,
                current_user_id,
                local_thread_id,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{DatasetId, DatasetLifecycle, TenantId};
    use serde_json::json;
    use std::collections::BTreeMap;
    use uuid::Uuid;

    fn test_dataset(
        visibility: DatasetVisibility,
        secret_binding_ids: Vec<SecretBindingId>,
    ) -> Dataset {
        Dataset {
            id: DatasetId::new(),
            tenant_id: TenantId::new(),
            owner_user_id: None,
            key: format!("dataset-{}", Uuid::new_v4()),
            title: "Dataset Visibility Test".to_string(),
            description: None,
            lifecycle: DatasetLifecycle::Active,
            visibility,
            default_secret_binding_ids: secret_binding_ids,
            metadata: BTreeMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn owner_visibility_preserves_public_and_owner_match_semantics() {
        let owner_user_id = UserId::new();

        assert!(owner_user_id_is_visible(None, None));
        assert!(owner_user_id_is_visible(None, Some(UserId::new())));
        assert!(owner_user_id_is_visible(
            Some(owner_user_id),
            Some(owner_user_id)
        ));
        assert!(!owner_user_id_is_visible(Some(owner_user_id), None));
        assert!(!owner_user_id_is_visible(
            Some(owner_user_id),
            Some(UserId::new())
        ));
    }

    #[test]
    fn owner_managed_resource_preserves_not_found_masking_semantics() {
        let owner_user_id = UserId::new();

        assert!(ensure_owner_managed_resource("dataset", "public".to_string(), None, None).is_ok());
        assert!(ensure_owner_managed_resource(
            "dataset",
            owner_user_id.to_string(),
            Some(owner_user_id),
            Some(owner_user_id)
        )
        .is_ok());

        let error = ensure_owner_managed_resource(
            "dataset",
            owner_user_id.to_string(),
            Some(owner_user_id),
            Some(UserId::new()),
        )
        .expect_err("non-owner should see a masked not_found error");
        assert_eq!(error.status, axum::http::StatusCode::NOT_FOUND);
        assert_eq!(error.payload.code, "dataset_not_found");
        assert!(error
            .payload
            .message
            .contains(&format!("dataset {owner_user_id} was not found")));
    }

    #[test]
    fn dataset_visibility_preserves_public_owner_and_secret_semantics() {
        let public_dataset = test_dataset(DatasetVisibility::Public, Vec::new());
        assert!(dataset_is_visible(&public_dataset, &[], None));

        let owner_user_id = UserId::new();
        let mut owned_private_dataset = test_dataset(DatasetVisibility::Private, Vec::new());
        owned_private_dataset.owner_user_id = Some(owner_user_id);
        assert!(dataset_is_visible(
            &owned_private_dataset,
            &[],
            Some(owner_user_id)
        ));
        assert!(!dataset_is_visible(
            &owned_private_dataset,
            &[],
            Some(UserId::new())
        ));

        let secret_binding_id = SecretBindingId::new();
        let private_dataset = test_dataset(DatasetVisibility::Private, vec![secret_binding_id]);
        assert!(!dataset_is_visible(&private_dataset, &[], None));
        assert!(dataset_is_visible(
            &private_dataset,
            &[secret_binding_id],
            None
        ));
    }

    #[test]
    fn dataset_request_visibility_preserves_local_thread_scope_bypass() {
        let mut dataset = test_dataset(DatasetVisibility::Private, Vec::new());
        dataset
            .metadata
            .insert("local_only".to_string(), json!(true));
        dataset
            .metadata
            .insert("local_thread_id".to_string(), json!("thread-a"));

        assert!(dataset_is_visible_for_request(
            &dataset,
            &[],
            None,
            Some("thread-a")
        ));
        assert!(!dataset_is_visible_for_request(
            &dataset,
            &[],
            None,
            Some("thread-b")
        ));
    }

    #[test]
    fn standard_dataset_list_hides_temporary_and_external_parse_sources() {
        let mut temporary_dataset = test_dataset(DatasetVisibility::Public, Vec::new());
        temporary_dataset
            .metadata
            .insert("scope_kind".to_string(), json!("external_temporary"));
        assert!(dataset_is_hidden_from_standard_dataset_list(
            &temporary_dataset
        ));

        let mut parse_dataset = test_dataset(DatasetVisibility::Public, Vec::new());
        parse_dataset.metadata.insert(
            "external_source".to_string(),
            json!({
                "created_by": "external_document_parse"
            }),
        );
        assert!(dataset_is_hidden_from_standard_dataset_list(&parse_dataset));

        let mut sync_dataset = test_dataset(DatasetVisibility::Public, Vec::new());
        sync_dataset.metadata.insert(
            "external_source".to_string(),
            json!({
                "created_by": "external_source_sync"
            }),
        );
        assert!(!dataset_is_hidden_from_standard_dataset_list(&sync_dataset));
    }
}
