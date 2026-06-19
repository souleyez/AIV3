use domain_model::{
    DatasetId, Document, DocumentId, DocumentLifecycle, RetrievalEvidence, SecretBindingId, UserId,
};
use serde_json::Value;
use std::collections::HashSet;

use crate::{
    assistant_run_scope_selection_support::selected_scope_temporary_dataset_id,
    load_visible_dataset_for_user_with_local_scope,
    not_found_errors::{dataset_not_found_error, document_not_found_error},
    resource_access::{dataset_is_external_temporary_scope, owner_user_id_is_visible},
    ApiError, AppState,
};

pub(crate) async fn load_visible_document_for_user(
    state: &AppState,
    document_id: DocumentId,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
) -> std::result::Result<Document, ApiError> {
    load_visible_document_for_user_with_local_scope(
        state,
        document_id,
        active_secret_binding_ids,
        current_user_id,
        None,
    )
    .await
}

pub(crate) async fn load_visible_document_for_assistant_scope(
    state: &AppState,
    document_id: DocumentId,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
    local_thread_id: Option<&str>,
    selected_scope: &Value,
) -> std::result::Result<Document, ApiError> {
    match load_visible_document_for_user_with_local_scope(
        state,
        document_id,
        active_secret_binding_ids,
        current_user_id,
        local_thread_id,
    )
    .await
    {
        Ok(document) => Ok(document),
        Err(error) => {
            let Some(temporary_dataset_id) = selected_scope_temporary_dataset_id(selected_scope)
            else {
                return Err(error);
            };
            let document = state
                .storage
                .documents()
                .get_by_id(state.tenant_id, document_id)
                .await
                .map_err(ApiError::from_storage)?
                .ok_or_else(|| document_not_found_error(document_id))?;
            if !owner_user_id_is_visible(document.owner_user_id, current_user_id) {
                return Err(error);
            }
            if !document_belongs_to_dataset_scope(state, &document, temporary_dataset_id).await? {
                return Err(error);
            }
            let dataset = state
                .storage
                .datasets()
                .get_by_id(state.tenant_id, temporary_dataset_id)
                .await
                .map_err(ApiError::from_storage)?
                .ok_or_else(|| dataset_not_found_error(temporary_dataset_id))?;
            if !dataset_is_external_temporary_scope(&dataset) {
                return Err(error);
            }
            Ok(document)
        }
    }
}

pub(crate) async fn load_visible_document_for_user_with_local_scope(
    state: &AppState,
    document_id: DocumentId,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
    local_thread_id: Option<&str>,
) -> std::result::Result<Document, ApiError> {
    let document = state
        .storage
        .documents()
        .get_by_id(state.tenant_id, document_id)
        .await
        .map_err(ApiError::from_storage)?
        .ok_or_else(|| document_not_found_error(document_id))?;
    if !owner_user_id_is_visible(document.owner_user_id, current_user_id) {
        return Err(document_not_found_error(document_id));
    }
    if document_has_visible_dataset_scope(
        state,
        &document,
        active_secret_binding_ids,
        current_user_id,
        local_thread_id,
    )
    .await?
    {
        return Ok(document);
    }
    Err(document_not_found_error(document_id))
}

async fn document_has_visible_dataset_scope(
    state: &AppState,
    document: &Document,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
    local_thread_id: Option<&str>,
) -> std::result::Result<bool, ApiError> {
    if load_visible_dataset_for_user_with_local_scope(
        state,
        document.dataset_id,
        active_secret_binding_ids,
        current_user_id,
        local_thread_id,
    )
    .await
    .is_ok()
    {
        return Ok(true);
    }

    let membership_dataset_ids = state
        .storage
        .dataset_document_memberships()
        .list_dataset_ids_by_document(state.tenant_id, document.id)
        .await
        .map_err(ApiError::from_storage)?;
    for dataset_id in membership_dataset_ids {
        if load_visible_dataset_for_user_with_local_scope(
            state,
            dataset_id,
            active_secret_binding_ids,
            current_user_id,
            local_thread_id,
        )
        .await
        .is_ok()
        {
            return Ok(true);
        }
    }
    Ok(false)
}

async fn document_belongs_to_dataset_scope(
    state: &AppState,
    document: &Document,
    dataset_id: DatasetId,
) -> std::result::Result<bool, ApiError> {
    if document.dataset_id == dataset_id {
        return Ok(true);
    }
    let membership_dataset_ids = state
        .storage
        .dataset_document_memberships()
        .list_dataset_ids_by_document(state.tenant_id, document.id)
        .await
        .map_err(ApiError::from_storage)?;
    Ok(membership_dataset_ids.contains(&dataset_id))
}

pub(crate) async fn list_documents_for_visible_dataset_scopes(
    state: &AppState,
    visible_dataset_ids: &HashSet<DatasetId>,
    current_user_id: Option<UserId>,
) -> std::result::Result<Vec<Document>, ApiError> {
    let mut documents = Vec::new();
    let mut seen = HashSet::new();
    for dataset_id in visible_dataset_ids {
        for document in list_documents_for_dataset_scope(state, *dataset_id)
            .await?
            .into_iter()
            .filter(|document| owner_user_id_is_visible(document.owner_user_id, current_user_id))
            .filter(|document| document.lifecycle != DocumentLifecycle::Archived)
        {
            if seen.insert(document.id) {
                documents.push(document);
            }
        }
    }
    documents.sort_by(|left, right| {
        right
            .created_at
            .cmp(&left.created_at)
            .then_with(|| left.title.cmp(&right.title))
    });
    Ok(documents)
}

pub(crate) async fn visible_document_ids_for_dataset(
    state: &AppState,
    dataset_id: DatasetId,
    current_user_id: Option<UserId>,
) -> std::result::Result<HashSet<DocumentId>, ApiError> {
    Ok(list_documents_for_dataset_scope(state, dataset_id)
        .await?
        .into_iter()
        .filter(|document| owner_user_id_is_visible(document.owner_user_id, current_user_id))
        .map(|document| document.id)
        .collect())
}

pub(crate) async fn list_documents_for_dataset_scope(
    state: &AppState,
    dataset_id: DatasetId,
) -> std::result::Result<Vec<Document>, ApiError> {
    state
        .storage
        .documents()
        .list_by_dataset_scope(state.tenant_id, dataset_id)
        .await
        .map_err(ApiError::from_storage)
}

pub(crate) async fn filter_retrieval_evidences_for_visible_documents(
    state: &AppState,
    dataset_id: DatasetId,
    evidences: Vec<RetrievalEvidence>,
    current_user_id: Option<UserId>,
) -> std::result::Result<Vec<RetrievalEvidence>, ApiError> {
    let visible_document_ids =
        visible_document_ids_for_dataset(state, dataset_id, current_user_id).await?;
    Ok(evidences
        .into_iter()
        .filter(|evidence| visible_document_ids.contains(&evidence.document_id))
        .collect())
}

pub(crate) fn document_is_visible_for_assistant_evidence_owner_scope(
    document: &Document,
    current_user_id: Option<UserId>,
    selected_document_ids: &[DocumentId],
    allow_selected_documents_without_acl_snapshot: bool,
) -> bool {
    owner_user_id_is_visible(document.owner_user_id, current_user_id)
        || external_acl_allows_missing_snapshot_for_selected_document(
            document.id,
            selected_document_ids,
            allow_selected_documents_without_acl_snapshot,
        )
}

async fn visible_document_ids_for_assistant_evidence_scope(
    state: &AppState,
    dataset_id: DatasetId,
    current_user_id: Option<UserId>,
    selected_document_ids: &[DocumentId],
    allow_selected_documents_without_acl_snapshot: bool,
) -> std::result::Result<HashSet<DocumentId>, ApiError> {
    Ok(list_documents_for_dataset_scope(state, dataset_id)
        .await?
        .into_iter()
        .filter(|document| {
            document_is_visible_for_assistant_evidence_owner_scope(
                document,
                current_user_id,
                selected_document_ids,
                allow_selected_documents_without_acl_snapshot,
            )
        })
        .map(|document| document.id)
        .collect())
}

pub(crate) async fn filter_retrieval_evidences_for_assistant_evidence_scope(
    state: &AppState,
    dataset_id: DatasetId,
    evidences: Vec<RetrievalEvidence>,
    current_user_id: Option<UserId>,
    selected_document_ids: &[DocumentId],
    allow_selected_documents_without_acl_snapshot: bool,
) -> std::result::Result<Vec<RetrievalEvidence>, ApiError> {
    let visible_document_ids = visible_document_ids_for_assistant_evidence_scope(
        state,
        dataset_id,
        current_user_id,
        selected_document_ids,
        allow_selected_documents_without_acl_snapshot,
    )
    .await?;
    Ok(evidences
        .into_iter()
        .filter(|evidence| visible_document_ids.contains(&evidence.document_id))
        .collect())
}

pub(crate) fn external_acl_allows_missing_snapshot_for_selected_document(
    document_id: DocumentId,
    selected_document_ids: &[DocumentId],
    allow_selected_documents_without_acl_snapshot: bool,
) -> bool {
    if !allow_selected_documents_without_acl_snapshot {
        return false;
    }
    selected_document_ids.is_empty() || selected_document_ids.contains(&document_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{TenantId, UserId};
    use std::collections::BTreeMap;
    use uuid::Uuid;

    fn document(id: u128, owner_user_id: Option<UserId>) -> Document {
        Document {
            id: DocumentId(Uuid::from_u128(id)),
            tenant_id: TenantId(Uuid::from_u128(2)),
            dataset_id: DatasetId(Uuid::from_u128(3)),
            owner_user_id,
            title: "Scoped document".to_string(),
            object_key: "documents/scoped.md".to_string(),
            content_type: "text/markdown".to_string(),
            lifecycle: DocumentLifecycle::Received,
            secret_binding_ids: Vec::new(),
            metadata: BTreeMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn selected_document_acl_fallback_requires_enabled_flag_and_matching_document() {
        let document_id = DocumentId(Uuid::from_u128(11));
        let other_document_id = DocumentId(Uuid::from_u128(12));

        assert!(!external_acl_allows_missing_snapshot_for_selected_document(
            document_id,
            &[document_id],
            false,
        ));
        assert!(external_acl_allows_missing_snapshot_for_selected_document(
            document_id,
            &[],
            true,
        ));
        assert!(external_acl_allows_missing_snapshot_for_selected_document(
            document_id,
            &[document_id],
            true,
        ));
        assert!(!external_acl_allows_missing_snapshot_for_selected_document(
            document_id,
            &[other_document_id],
            true,
        ));
    }

    #[test]
    fn assistant_evidence_owner_scope_allows_owner_or_selected_acl_fallback() {
        let owner = UserId(Uuid::from_u128(21));
        let owner_document = document(31, Some(owner));
        let selected_document = document(32, Some(UserId(Uuid::from_u128(22))));
        let hidden_document = document(33, Some(UserId(Uuid::from_u128(23))));

        assert!(document_is_visible_for_assistant_evidence_owner_scope(
            &owner_document,
            Some(owner),
            &[],
            false,
        ));
        assert!(document_is_visible_for_assistant_evidence_owner_scope(
            &selected_document,
            Some(owner),
            &[selected_document.id],
            true,
        ));
        assert!(!document_is_visible_for_assistant_evidence_owner_scope(
            &hidden_document,
            Some(owner),
            &[selected_document.id],
            true,
        ));
    }
}
