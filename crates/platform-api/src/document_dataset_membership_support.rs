use chrono::{DateTime, Utc};
use contracts::DocumentSummary;
use domain_model::{DatasetId, Document, DocumentId, SecretBindingId, UserId};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashSet;
use storage::NewDatasetDocumentMembership;

use crate::{
    document_scope_can_contribute_to_dataset, hydrate_document_summary_dataset_ids,
    load_visible_dataset_for_user, load_visible_document_for_user,
    not_found_errors::dataset_not_found_error, resource_access::ensure_owner_managed_resource,
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
        let canonical_dataset = state
            .storage
            .datasets()
            .get_by_id(state.tenant_id, document.dataset_id)
            .await
            .map_err(ApiError::from_storage)?
            .ok_or_else(|| dataset_not_found_error(document.dataset_id))?;
        ensure_document_dataset_membership_scope_compatible(
            &document,
            &canonical_dataset,
            &dataset,
        )?;
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
    let canonical_dataset = state
        .storage
        .datasets()
        .get_by_id(state.tenant_id, document.dataset_id)
        .await
        .map_err(ApiError::from_storage)?
        .ok_or_else(|| dataset_not_found_error(document.dataset_id))?;
    let membership_dataset_ids = state
        .storage
        .dataset_document_memberships()
        .list_dataset_ids_by_document(state.tenant_id, document.id)
        .await
        .map_err(ApiError::from_storage)?;
    let candidate_dataset_ids =
        candidate_membership_dataset_ids_after_removal(membership_dataset_ids, removed_dataset_id);

    let mut visible_candidate_datasets = Vec::new();
    for membership_dataset_id in candidate_dataset_ids {
        if let Ok(candidate_dataset) = load_visible_dataset_for_user(
            state,
            membership_dataset_id,
            active_secret_binding_ids,
            current_user_id,
        )
        .await
        {
            visible_candidate_datasets.push(candidate_dataset);
        }
    }
    let promote_to = select_compatible_promotion_dataset(
        &document,
        &canonical_dataset,
        &visible_candidate_datasets,
    )?;
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

fn ensure_document_dataset_membership_scope_compatible(
    document: &Document,
    canonical_dataset: &domain_model::Dataset,
    target_dataset: &domain_model::Dataset,
) -> std::result::Result<(), ApiError> {
    if document_scope_can_contribute_to_dataset(document, canonical_dataset, target_dataset) {
        Ok(())
    } else {
        Err(document_dataset_scope_incompatible_error())
    }
}

fn select_compatible_promotion_dataset(
    document: &Document,
    canonical_dataset: &domain_model::Dataset,
    visible_candidate_datasets: &[domain_model::Dataset],
) -> std::result::Result<DatasetId, ApiError> {
    visible_candidate_datasets
        .iter()
        .find(|candidate_dataset| {
            document_scope_can_contribute_to_dataset(document, canonical_dataset, candidate_dataset)
        })
        .map(|dataset| dataset.id)
        .ok_or_else(|| {
            if visible_candidate_datasets.is_empty() {
                document_dataset_membership_required_error()
            } else {
                document_dataset_scope_incompatible_error()
            }
        })
}

fn document_dataset_scope_incompatible_error() -> ApiError {
    ApiError::bad_request(
        "document_dataset_scope_incompatible",
        "document scope is incompatible with the target dataset".to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use domain_model::{
        Dataset, DatasetLifecycle, DatasetVisibility, DocumentLifecycle, TenantId, UserId,
    };
    use std::collections::BTreeMap;
    use uuid::Uuid;

    fn dataset_id(value: u128) -> DatasetId {
        DatasetId(Uuid::from_u128(value))
    }

    fn document_id(value: u128) -> DocumentId {
        DocumentId(Uuid::from_u128(value))
    }

    fn dataset(
        id: u128,
        tenant_id: TenantId,
        owner_user_id: Option<UserId>,
        visibility: DatasetVisibility,
    ) -> Dataset {
        Dataset {
            id: dataset_id(id),
            tenant_id,
            owner_user_id,
            key: format!("membership-{id}"),
            title: format!("Membership {id}"),
            description: None,
            lifecycle: DatasetLifecycle::Active,
            visibility,
            default_secret_binding_ids: Vec::new(),
            metadata: BTreeMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn document(
        id: u128,
        tenant_id: TenantId,
        canonical_dataset_id: DatasetId,
        owner_user_id: Option<UserId>,
    ) -> Document {
        Document {
            id: document_id(id),
            tenant_id,
            dataset_id: canonical_dataset_id,
            owner_user_id,
            title: "Membership document".to_string(),
            object_key: "documents/membership.md".to_string(),
            content_type: "text/markdown".to_string(),
            lifecycle: DocumentLifecycle::Received,
            secret_binding_ids: Vec::new(),
            metadata: BTreeMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn membership_add_guard_rejects_scope_widening_with_safe_error() {
        let tenant_id = TenantId(Uuid::from_u128(40));
        let owner = UserId(Uuid::from_u128(41));
        let source = dataset(42, tenant_id, Some(owner), DatasetVisibility::Private);
        let target = dataset(43, tenant_id, None, DatasetVisibility::Public);
        let document = document(44, tenant_id, source.id, Some(owner));

        let error =
            ensure_document_dataset_membership_scope_compatible(&document, &source, &target)
                .expect_err("private document must not be attached to a public dataset");

        assert_eq!(error.payload.code, "document_dataset_scope_incompatible");
        assert_eq!(
            error.payload.message,
            "document scope is incompatible with the target dataset"
        );
        assert!(error.payload.details.is_none());
    }

    #[test]
    fn canonical_move_rejects_incompatible_visible_promotion_candidates() {
        let tenant_id = TenantId(Uuid::from_u128(50));
        let owner = UserId(Uuid::from_u128(51));
        let source = dataset(52, tenant_id, Some(owner), DatasetVisibility::Private);
        let public_candidate = dataset(53, tenant_id, None, DatasetVisibility::Public);
        let wrong_owner_candidate = dataset(
            54,
            tenant_id,
            Some(UserId(Uuid::from_u128(55))),
            DatasetVisibility::Private,
        );
        let document = document(56, tenant_id, source.id, Some(owner));

        let error = select_compatible_promotion_dataset(
            &document,
            &source,
            &[public_candidate, wrong_owner_candidate],
        )
        .expect_err("canonical move must reject every incompatible promotion target");

        assert_eq!(error.payload.code, "document_dataset_scope_incompatible");
    }

    #[test]
    fn canonical_move_preserves_order_and_selects_first_compatible_candidate() {
        let tenant_id = TenantId(Uuid::from_u128(60));
        let owner = UserId(Uuid::from_u128(61));
        let source = dataset(62, tenant_id, Some(owner), DatasetVisibility::Private);
        let incompatible = dataset(63, tenant_id, None, DatasetVisibility::Public);
        let compatible = dataset(64, tenant_id, Some(owner), DatasetVisibility::Private);
        let later_compatible = dataset(65, tenant_id, Some(owner), DatasetVisibility::Private);
        let document = document(66, tenant_id, source.id, Some(owner));

        let selected = select_compatible_promotion_dataset(
            &document,
            &source,
            &[incompatible, compatible.clone(), later_compatible],
        )
        .expect("one compatible promotion target should be selected");

        assert_eq!(selected, compatible.id);
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
