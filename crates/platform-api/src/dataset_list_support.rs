use contracts::DatasetSummary;
use domain_model::{Dataset, DatasetLifecycle, SecretBindingId, UserId};

use crate::{
    dataset_summary_support::dataset_summary,
    enrich_visible_datasets_for_scope_planning, ensure_default_public_datasets,
    resource_access::{dataset_is_hidden_from_standard_dataset_list, filter_visible_datasets},
    ApiError, AppState,
};

pub(crate) async fn list_visible_dataset_summaries(
    state: &AppState,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
    local_thread_id: Option<&str>,
) -> std::result::Result<Vec<DatasetSummary>, ApiError> {
    ensure_default_public_datasets(&state.storage, state.tenant_id).await?;
    let datasets = state
        .storage
        .datasets()
        .list_by_tenant(state.tenant_id)
        .await
        .map_err(ApiError::from_storage)?;
    let visible_datasets = filter_standard_dataset_list_candidates(filter_visible_datasets(
        datasets,
        active_secret_binding_ids,
        current_user_id,
        local_thread_id,
    ));
    let visible_datasets =
        enrich_visible_datasets_for_scope_planning(state, visible_datasets, current_user_id)
            .await?;

    Ok(visible_datasets
        .into_iter()
        .map(|dataset| dataset_summary(dataset, None))
        .collect())
}

fn filter_standard_dataset_list_candidates(datasets: Vec<Dataset>) -> Vec<Dataset> {
    datasets
        .into_iter()
        .filter(|dataset| dataset.lifecycle != DatasetLifecycle::Archived)
        .filter(|dataset| !dataset_is_hidden_from_standard_dataset_list(dataset))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use domain_model::{DatasetId, DatasetVisibility, TenantId};
    use serde_json::json;
    use uuid::Uuid;

    use super::*;

    fn dataset(
        title: &str,
        lifecycle: DatasetLifecycle,
        metadata: BTreeMap<String, serde_json::Value>,
    ) -> Dataset {
        Dataset {
            id: DatasetId::new(),
            tenant_id: TenantId(Uuid::from_u128(1)),
            owner_user_id: None,
            key: format!("dataset-{}", Uuid::new_v4()),
            title: title.to_string(),
            description: None,
            lifecycle,
            visibility: DatasetVisibility::Public,
            default_secret_binding_ids: Vec::new(),
            metadata,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn standard_dataset_list_candidates_drop_archived_and_system_parse_sources() {
        let mut hidden_metadata = BTreeMap::new();
        hidden_metadata.insert(
            "external_source".to_string(),
            json!({
                "created_by": "external_document_parse"
            }),
        );

        let visible = dataset("visible", DatasetLifecycle::Active, BTreeMap::new());
        let archived = dataset("archived", DatasetLifecycle::Archived, BTreeMap::new());
        let hidden = dataset("hidden", DatasetLifecycle::Active, hidden_metadata);

        let filtered =
            filter_standard_dataset_list_candidates(vec![visible.clone(), archived, hidden]);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, visible.id);
    }
}
