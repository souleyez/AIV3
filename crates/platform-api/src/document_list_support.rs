use contracts::DocumentSummary;
use domain_model::{Dataset, DatasetId, SecretBindingId, UserId};
use std::collections::HashSet;

use crate::{
    filter_visible_datasets, list_documents_for_visible_dataset_scopes,
    to_document_summaries_with_dataset_ids, ApiError, AppState,
};

pub(crate) async fn list_document_summaries_for_user(
    state: &AppState,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
    local_thread_id: Option<&str>,
) -> std::result::Result<Vec<DocumentSummary>, ApiError> {
    let visible_dataset_ids = visible_document_list_dataset_ids(
        state
            .storage
            .datasets()
            .list_by_tenant(state.tenant_id)
            .await
            .map_err(ApiError::from_storage)?,
        active_secret_binding_ids,
        current_user_id,
        local_thread_id,
    );
    let documents =
        list_documents_for_visible_dataset_scopes(state, &visible_dataset_ids, current_user_id)
            .await?;

    to_document_summaries_with_dataset_ids(state, documents, Some(&visible_dataset_ids)).await
}

fn visible_document_list_dataset_ids(
    datasets: Vec<Dataset>,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
    local_thread_id: Option<&str>,
) -> HashSet<DatasetId> {
    filter_visible_datasets(
        datasets,
        active_secret_binding_ids,
        current_user_id,
        local_thread_id,
    )
    .into_iter()
    .map(|dataset| dataset.id)
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{DatasetLifecycle, DatasetVisibility, TenantId};
    use serde_json::json;
    use std::collections::BTreeMap;
    use uuid::Uuid;

    fn dataset(id: u128, visibility: DatasetVisibility) -> Dataset {
        Dataset {
            id: DatasetId(Uuid::from_u128(id)),
            tenant_id: TenantId(Uuid::from_u128(100)),
            owner_user_id: None,
            key: format!("dataset-{id}"),
            title: format!("Dataset {id}"),
            description: None,
            lifecycle: DatasetLifecycle::Active,
            visibility,
            default_secret_binding_ids: Vec::new(),
            metadata: BTreeMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn visible_document_list_dataset_ids_keep_public_owner_secret_and_local_scope() {
        let owner_user_id = UserId(Uuid::from_u128(10));
        let secret_binding_id = SecretBindingId(Uuid::from_u128(20));

        let public_dataset = dataset(1, DatasetVisibility::Public);
        let public_id = public_dataset.id;

        let mut owned_private_dataset = dataset(2, DatasetVisibility::Private);
        owned_private_dataset.owner_user_id = Some(owner_user_id);
        let owned_private_id = owned_private_dataset.id;

        let mut secret_dataset = dataset(3, DatasetVisibility::Private);
        secret_dataset.default_secret_binding_ids = vec![secret_binding_id];
        let secret_dataset_id = secret_dataset.id;

        let mut local_only_dataset = dataset(4, DatasetVisibility::Private);
        local_only_dataset
            .metadata
            .insert("local_only".to_string(), json!(true));
        local_only_dataset
            .metadata
            .insert("local_thread_id".to_string(), json!("thread-a"));
        let local_only_id = local_only_dataset.id;

        let visible_ids = visible_document_list_dataset_ids(
            vec![
                public_dataset,
                owned_private_dataset,
                secret_dataset,
                local_only_dataset,
            ],
            &[secret_binding_id],
            Some(owner_user_id),
            Some("thread-a"),
        );

        assert!(visible_ids.contains(&public_id));
        assert!(visible_ids.contains(&owned_private_id));
        assert!(visible_ids.contains(&secret_dataset_id));
        assert!(visible_ids.contains(&local_only_id));
    }
}
