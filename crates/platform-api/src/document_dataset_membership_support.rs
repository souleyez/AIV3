use chrono::{DateTime, Utc};
use contracts::DocumentSummary;
use domain_model::{DatasetId, Document, DocumentId, SecretBindingId, UserId};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashSet;
use storage::NewDatasetDocumentMembership;

use crate::{
    hydrate_document_summary_dataset_ids, load_visible_dataset_for_user,
    load_visible_document_for_user, resource_access::ensure_owner_managed_resource,
    to_document_summary, ApiError, AppState,
};

#[derive(Debug, Serialize)]
pub(crate) struct DocumentDatasetMembershipResponse {
    pub(crate) document: DocumentSummary,
    pub(crate) dataset_ids: Vec<DatasetId>,
    #[serde(rename = "datasetIds")]
    pub(crate) dataset_ids_camel: Vec<DatasetId>,
    pub(crate) canonical_dataset_id: DatasetId,
    #[serde(rename = "canonicalDatasetId")]
    pub(crate) canonical_dataset_id_camel: DatasetId,
}

pub(crate) async fn add_document_dataset_membership_for_user(
    state: &AppState,
    document_id: DocumentId,
    dataset_id: DatasetId,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
) -> std::result::Result<DocumentDatasetMembershipResponse, ApiError> {
    let document = load_visible_document_for_user(
        state,
        document_id,
        active_secret_binding_ids,
        current_user_id,
    )
    .await?;
    ensure_owner_managed_resource(
        "document",
        document.id.to_string(),
        document.owner_user_id,
        current_user_id,
    )?;
    let dataset = load_visible_dataset_for_user(
        state,
        dataset_id,
        active_secret_binding_ids,
        current_user_id,
    )
    .await?;
    ensure_owner_managed_resource(
        "dataset",
        dataset.id.to_string(),
        dataset.owner_user_id,
        current_user_id,
    )?;

    if should_create_secondary_dataset_membership(document.dataset_id, dataset_id) {
        state
            .storage
            .dataset_document_memberships()
            .create_or_update(
                state.tenant_id,
                new_manual_dataset_document_membership(dataset_id, document_id),
            )
            .await
            .map_err(ApiError::from_storage)?;
    }

    document_dataset_membership_response(state, document, None).await
}

pub(crate) async fn remove_document_dataset_membership_for_user(
    state: &AppState,
    document_id: DocumentId,
    dataset_id: DatasetId,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
) -> std::result::Result<DocumentDatasetMembershipResponse, ApiError> {
    let document = load_visible_document_for_user(
        state,
        document_id,
        active_secret_binding_ids,
        current_user_id,
    )
    .await?;
    ensure_owner_managed_resource(
        "document",
        document.id.to_string(),
        document.owner_user_id,
        current_user_id,
    )?;
    load_visible_dataset_for_user(
        state,
        dataset_id,
        active_secret_binding_ids,
        current_user_id,
    )
    .await?;

    let updated_document = if document.dataset_id == dataset_id {
        remove_canonical_dataset_membership(
            state,
            document,
            dataset_id,
            active_secret_binding_ids,
            current_user_id,
        )
        .await?
    } else {
        state
            .storage
            .dataset_document_memberships()
            .delete(state.tenant_id, dataset_id, document.id)
            .await
            .map_err(ApiError::from_storage)?;
        document
    };

    document_dataset_membership_response(state, updated_document, None).await
}

async fn remove_canonical_dataset_membership(
    state: &AppState,
    document: Document,
    removed_dataset_id: DatasetId,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
) -> std::result::Result<Document, ApiError> {
    let membership_dataset_ids = state
        .storage
        .dataset_document_memberships()
        .list_dataset_ids_by_document(state.tenant_id, document.id)
        .await
        .map_err(ApiError::from_storage)?;
    let candidate_dataset_ids =
        candidate_membership_dataset_ids_after_removal(membership_dataset_ids, removed_dataset_id);

    let mut promote_to = None;
    for membership_dataset_id in candidate_dataset_ids {
        if load_visible_dataset_for_user(
            state,
            membership_dataset_id,
            active_secret_binding_ids,
            current_user_id,
        )
        .await
        .is_ok()
        {
            promote_to = Some(membership_dataset_id);
            break;
        }
    }
    let promote_to = promote_to.ok_or_else(document_dataset_membership_required_error)?;
    let now = Utc::now();
    let moved = state
        .storage
        .documents()
        .move_to_dataset(
            state.tenant_id,
            document.id,
            promote_to,
            &document_dataset_membership_update_metadata(removed_dataset_id, promote_to, now),
            now,
        )
        .await
        .map_err(ApiError::from_storage)?;
    state
        .storage
        .dataset_document_memberships()
        .delete(state.tenant_id, promote_to, document.id)
        .await
        .map_err(ApiError::from_storage)?;
    Ok(moved)
}

async fn document_dataset_membership_response(
    state: &AppState,
    document: Document,
    visible_dataset_ids: Option<&HashSet<DatasetId>>,
) -> std::result::Result<DocumentDatasetMembershipResponse, ApiError> {
    let document = hydrate_document_summary_dataset_ids(
        state,
        to_document_summary(document),
        visible_dataset_ids,
    )
    .await?;
    let dataset_ids = document.dataset_ids.clone();
    Ok(DocumentDatasetMembershipResponse {
        canonical_dataset_id: document.dataset_id,
        canonical_dataset_id_camel: document.dataset_id,
        document,
        dataset_ids: dataset_ids.clone(),
        dataset_ids_camel: dataset_ids,
    })
}

fn new_manual_dataset_document_membership(
    dataset_id: DatasetId,
    document_id: DocumentId,
) -> NewDatasetDocumentMembership {
    NewDatasetDocumentMembership {
        dataset_id,
        document_id,
        membership_kind: "curated".to_string(),
        source: "manual".to_string(),
        expires_at: None,
    }
}

fn should_create_secondary_dataset_membership(
    canonical_dataset_id: DatasetId,
    target_dataset_id: DatasetId,
) -> bool {
    canonical_dataset_id != target_dataset_id
}

fn candidate_membership_dataset_ids_after_removal(
    membership_dataset_ids: Vec<DatasetId>,
    removed_dataset_id: DatasetId,
) -> Vec<DatasetId> {
    membership_dataset_ids
        .into_iter()
        .filter(|dataset_id| *dataset_id != removed_dataset_id)
        .collect()
}

fn document_dataset_membership_update_metadata(
    removed_dataset_id: DatasetId,
    promoted_dataset_id: DatasetId,
    updated_at: DateTime<Utc>,
) -> Value {
    json!({
        "dataset_membership_update": {
            "removed_dataset_id": removed_dataset_id,
            "promoted_dataset_id": promoted_dataset_id,
            "updated_at": updated_at,
        }
    })
}

fn document_dataset_membership_required_error() -> ApiError {
    ApiError::bad_request(
        "document_dataset_membership_required",
        "document must belong to at least one visible dataset".to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use uuid::Uuid;

    fn dataset_id(value: u128) -> DatasetId {
        DatasetId(Uuid::from_u128(value))
    }

    fn document_id(value: u128) -> DocumentId {
        DocumentId(Uuid::from_u128(value))
    }

    #[test]
    fn manual_dataset_document_membership_keeps_existing_storage_shape() {
        let dataset_id = dataset_id(10);
        let document_id = document_id(20);

        let membership = new_manual_dataset_document_membership(dataset_id, document_id);

        assert_eq!(membership.dataset_id, dataset_id);
        assert_eq!(membership.document_id, document_id);
        assert_eq!(membership.membership_kind, "curated");
        assert_eq!(membership.source, "manual");
        assert_eq!(membership.expires_at, None);
    }

    #[test]
    fn secondary_membership_guard_skips_canonical_dataset() {
        let canonical = dataset_id(1);
        let secondary = dataset_id(2);

        assert!(!should_create_secondary_dataset_membership(
            canonical, canonical
        ));
        assert!(should_create_secondary_dataset_membership(
            canonical, secondary
        ));
    }

    #[test]
    fn membership_promotion_candidates_preserve_order_and_skip_removed_dataset() {
        let removed = dataset_id(1);
        let second = dataset_id(2);
        let third = dataset_id(3);

        let candidates =
            candidate_membership_dataset_ids_after_removal(vec![removed, second, third], removed);

        assert_eq!(candidates, vec![second, third]);
    }

    #[test]
    fn membership_update_metadata_keeps_existing_keys() {
        let removed = dataset_id(1);
        let promoted = dataset_id(2);
        let updated_at = Utc
            .with_ymd_and_hms(2026, 6, 19, 12, 0, 0)
            .single()
            .expect("fixed timestamp should be valid");

        let metadata = document_dataset_membership_update_metadata(removed, promoted, updated_at);

        assert_eq!(
            metadata.pointer("/dataset_membership_update/removed_dataset_id"),
            Some(&json!(removed))
        );
        assert_eq!(
            metadata.pointer("/dataset_membership_update/promoted_dataset_id"),
            Some(&json!(promoted))
        );
        assert_eq!(
            metadata.pointer("/dataset_membership_update/updated_at"),
            Some(&json!(updated_at))
        );
    }

    #[test]
    fn membership_required_error_keeps_existing_code_and_message() {
        let error = document_dataset_membership_required_error();

        assert_eq!(error.payload.code, "document_dataset_membership_required");
        assert_eq!(
            error.payload.message,
            "document must belong to at least one visible dataset"
        );
    }
}
