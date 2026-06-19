use contracts::DatasetOutputView;
use domain_model::{DatasetId, DatasetOutput, SecretBindingId, UserId};

use crate::{
    hydrate_dataset_output_view, load_visible_dataset_for_user,
    resource_access::owner_user_id_is_visible, ApiError, AppState,
};

pub(crate) async fn list_dataset_output_views_for_user(
    state: &AppState,
    dataset_id: DatasetId,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
) -> std::result::Result<Vec<DatasetOutputView>, ApiError> {
    load_visible_dataset_for_user(
        state,
        dataset_id,
        active_secret_binding_ids,
        current_user_id,
    )
    .await?;

    let outputs = state
        .storage
        .dataset_outputs()
        .list_by_dataset(state.tenant_id, dataset_id)
        .await
        .map_err(ApiError::from_storage)?;

    let outputs = filter_dataset_outputs_visible_to_user(outputs, current_user_id);
    let mut views = Vec::with_capacity(outputs.len());
    for output in outputs {
        views.push(hydrate_dataset_output_view(state, output).await?);
    }
    Ok(views)
}

fn filter_dataset_outputs_visible_to_user(
    outputs: Vec<DatasetOutput>,
    current_user_id: Option<UserId>,
) -> Vec<DatasetOutput> {
    outputs
        .into_iter()
        .filter(|output| owner_user_id_is_visible(output.owner_user_id, current_user_id))
        .collect()
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use domain_model::{DatasetOutputId, TenantId, WorkflowExecutionId};
    use serde_json::json;

    use super::*;

    fn dataset_output(
        dataset_id: DatasetId,
        owner_user_id: Option<UserId>,
        prompt: &str,
    ) -> DatasetOutput {
        DatasetOutput {
            id: DatasetOutputId::new(),
            tenant_id: TenantId::new(),
            execution_id: WorkflowExecutionId::new(),
            dataset_id,
            owner_user_id,
            prompt: prompt.to_string(),
            output_text: String::new(),
            memory_directory_id: None,
            retrieval_evidence_ids: Vec::new(),
            output_manifest: json!({}),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn dataset_output_list_filter_keeps_public_and_owned_outputs_only() {
        let dataset_id = DatasetId::new();
        let current_user_id = UserId::new();
        let other_user_id = UserId::new();
        let public = dataset_output(dataset_id, None, "public");
        let public_id = public.id;
        let owned = dataset_output(dataset_id, Some(current_user_id), "owned");
        let owned_id = owned.id;
        let other = dataset_output(dataset_id, Some(other_user_id), "other");

        let visible = filter_dataset_outputs_visible_to_user(
            vec![public, other, owned],
            Some(current_user_id),
        );

        assert_eq!(visible.len(), 2);
        assert_eq!(visible[0].id, public_id);
        assert_eq!(visible[1].id, owned_id);
    }
}
