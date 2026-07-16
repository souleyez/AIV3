use domain_model::{
    Dataset, DatasetId, DatasetVisibility, Document, DocumentId, DocumentLifecycle,
    RetrievalEvidence, SecretBindingId, UserId,
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

use crate::{
    assistant_run_scope_selection_support::selected_scope_temporary_dataset_id,
    load_visible_dataset_for_user_with_local_scope,
    not_found_errors::{dataset_not_found_error, document_not_found_error},
    resource_access::{dataset_is_external_temporary_scope, owner_user_id_is_visible},
    ApiError, AppState,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DocumentContributionScopeMismatch {
    Tenant,
    CanonicalDataset,
    Owner,
    Visibility,
    SecretBinding,
    LocalThread,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExcludedDocumentContribution {
    pub(crate) document_id: DocumentId,
    pub(crate) reason: DocumentContributionScopeMismatch,
}

#[derive(Clone, Debug)]
pub(crate) struct DatasetContributionDocuments {
    pub(crate) documents: Vec<Document>,
    pub(crate) excluded: Vec<ExcludedDocumentContribution>,
}

pub(crate) fn document_scope_can_contribute_to_dataset(
    document: &Document,
    canonical_dataset: &Dataset,
    target_dataset: &Dataset,
) -> bool {
    document_contribution_scope_mismatch(document, canonical_dataset, target_dataset).is_none()
}

fn document_contribution_scope_mismatch(
    document: &Document,
    canonical_dataset: &Dataset,
    target_dataset: &Dataset,
) -> Option<DocumentContributionScopeMismatch> {
    if document.tenant_id != canonical_dataset.tenant_id
        || document.tenant_id != target_dataset.tenant_id
    {
        return Some(DocumentContributionScopeMismatch::Tenant);
    }
    if document.dataset_id != canonical_dataset.id {
        return Some(DocumentContributionScopeMismatch::CanonicalDataset);
    }

    if let Some(document_owner) = document.owner_user_id {
        let canonical_scope_allows_owner = canonical_dataset.owner_user_id == Some(document_owner)
            || (canonical_dataset.owner_user_id.is_none()
                && canonical_dataset.visibility == DatasetVisibility::Public
                && matches!(dataset_local_thread_scope(canonical_dataset), Ok(None)));
        if !canonical_scope_allows_owner
            || target_dataset.owner_user_id != Some(document_owner)
            || !target_dataset.default_secret_binding_ids.is_empty()
        {
            return Some(DocumentContributionScopeMismatch::Owner);
        }
    } else if let Some(source_owner) = canonical_dataset.owner_user_id {
        if target_dataset.owner_user_id != Some(source_owner) {
            return Some(DocumentContributionScopeMismatch::Owner);
        }
    } else if canonical_dataset.visibility != DatasetVisibility::Public
        && target_dataset.owner_user_id.is_some()
    {
        // A secret-scoped source cannot prove that an arbitrary target owner is
        // one of the source secret holders. Fail closed instead of widening it.
        return Some(DocumentContributionScopeMismatch::Owner);
    }

    let source_local_thread = match dataset_local_thread_scope(canonical_dataset) {
        Ok(value) => value,
        Err(reason) => return Some(reason),
    };
    let document_local_thread = match document_local_thread_scope(document) {
        Ok(value) => value,
        Err(reason) => return Some(reason),
    };
    if source_local_thread.is_some()
        && document_local_thread.is_some()
        && source_local_thread != document_local_thread
    {
        return Some(DocumentContributionScopeMismatch::LocalThread);
    }
    let effective_source_local_thread = document_local_thread.or(source_local_thread);
    let target_local_thread = match dataset_local_thread_scope(target_dataset) {
        Ok(value) => value,
        Err(reason) => return Some(reason),
    };
    if let Some(source_thread) = effective_source_local_thread {
        if target_local_thread != Some(source_thread) {
            return Some(DocumentContributionScopeMismatch::LocalThread);
        }
    } else if target_local_thread.is_some()
        && !source_scope_is_unrestricted_public(document, canonical_dataset)
    {
        // A local-thread bypass is a new principal unless the source was
        // already public to every principal.
        return Some(DocumentContributionScopeMismatch::LocalThread);
    }

    if target_scope_is_unrestricted_public(target_dataset)
        && !source_scope_is_unrestricted_public(document, canonical_dataset)
    {
        return Some(DocumentContributionScopeMismatch::Visibility);
    }

    if canonical_dataset.visibility != DatasetVisibility::Public
        && !secret_set_is_subset(
            &target_dataset.default_secret_binding_ids,
            &canonical_dataset.default_secret_binding_ids,
        )
    {
        return Some(DocumentContributionScopeMismatch::SecretBinding);
    }
    if !document.secret_binding_ids.is_empty()
        && !secret_set_is_subset(
            &target_dataset.default_secret_binding_ids,
            &document.secret_binding_ids,
        )
    {
        return Some(DocumentContributionScopeMismatch::SecretBinding);
    }

    None
}

fn dataset_local_thread_scope(
    dataset: &Dataset,
) -> Result<Option<&str>, DocumentContributionScopeMismatch> {
    let local_only = dataset
        .metadata
        .get("local_only")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !local_only || dataset.owner_user_id.is_some() {
        return Ok(None);
    }
    dataset
        .metadata
        .get("local_thread_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(Some)
        .ok_or(DocumentContributionScopeMismatch::LocalThread)
}

fn document_local_thread_scope(
    document: &Document,
) -> Result<Option<&str>, DocumentContributionScopeMismatch> {
    let local_only = document
        .metadata
        .get("local_only")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !local_only || document.owner_user_id.is_some() {
        return Ok(None);
    }
    document
        .metadata
        .get("local_thread_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(Some)
        .ok_or(DocumentContributionScopeMismatch::LocalThread)
}

fn target_scope_is_unrestricted_public(dataset: &Dataset) -> bool {
    dataset.owner_user_id.is_none()
        && dataset.visibility == DatasetVisibility::Public
        && matches!(dataset_local_thread_scope(dataset), Ok(None))
}

fn source_scope_is_unrestricted_public(document: &Document, dataset: &Dataset) -> bool {
    document.owner_user_id.is_none()
        && document.secret_binding_ids.is_empty()
        && matches!(document_local_thread_scope(document), Ok(None))
        && target_scope_is_unrestricted_public(dataset)
}

fn secret_set_is_subset(
    candidate_subset: &[SecretBindingId],
    candidate_superset: &[SecretBindingId],
) -> bool {
    candidate_subset
        .iter()
        .all(|candidate| candidate_superset.contains(candidate))
}

pub(crate) fn filter_documents_for_dataset_contribution(
    target_dataset: &Dataset,
    documents: Vec<Document>,
    canonical_datasets: &[Dataset],
) -> DatasetContributionDocuments {
    let canonical_datasets = canonical_datasets
        .iter()
        .map(|dataset| (dataset.id, dataset))
        .collect::<HashMap<_, _>>();
    let mut allowed = Vec::new();
    let mut excluded = Vec::new();

    for document in documents {
        let mismatch = canonical_datasets
            .get(&document.dataset_id)
            .and_then(|canonical_dataset| {
                document_contribution_scope_mismatch(&document, canonical_dataset, target_dataset)
            })
            .or_else(|| {
                (!canonical_datasets.contains_key(&document.dataset_id))
                    .then_some(DocumentContributionScopeMismatch::CanonicalDataset)
            });
        if let Some(reason) = mismatch {
            excluded.push(ExcludedDocumentContribution {
                document_id: document.id,
                reason,
            });
        } else {
            allowed.push(document);
        }
    }

    DatasetContributionDocuments {
        documents: allowed,
        excluded,
    }
}

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
    use domain_model::{
        Dataset, DatasetLifecycle, DatasetVisibility, SecretBindingId, TenantId, UserId,
    };
    use serde_json::json;
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

    fn dataset(
        id: u128,
        tenant_id: TenantId,
        owner_user_id: Option<UserId>,
        visibility: DatasetVisibility,
        secret_binding_ids: Vec<SecretBindingId>,
        local_thread_id: Option<&str>,
    ) -> Dataset {
        let mut metadata = BTreeMap::new();
        if let Some(local_thread_id) = local_thread_id {
            metadata.insert("local_only".to_string(), json!(true));
            metadata.insert("local_thread_id".to_string(), json!(local_thread_id));
        }
        Dataset {
            id: DatasetId(Uuid::from_u128(id)),
            tenant_id,
            owner_user_id,
            key: format!("scope-{id}"),
            title: format!("Scope {id}"),
            description: None,
            lifecycle: DatasetLifecycle::Active,
            visibility,
            default_secret_binding_ids: secret_binding_ids,
            metadata,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn scoped_document(
        id: u128,
        tenant_id: TenantId,
        canonical_dataset_id: DatasetId,
        owner_user_id: Option<UserId>,
        secret_binding_ids: Vec<SecretBindingId>,
    ) -> Document {
        let mut document = document(id, owner_user_id);
        document.tenant_id = tenant_id;
        document.dataset_id = canonical_dataset_id;
        document.secret_binding_ids = secret_binding_ids;
        document
    }

    #[test]
    fn document_contribution_scope_accepts_public_ownerless_and_matching_private_owner() {
        let tenant_id = TenantId(Uuid::from_u128(100));
        let public_source = dataset(
            101,
            tenant_id,
            None,
            DatasetVisibility::Public,
            Vec::new(),
            None,
        );
        let public_target = dataset(
            102,
            tenant_id,
            None,
            DatasetVisibility::Public,
            Vec::new(),
            None,
        );
        let public_document = scoped_document(103, tenant_id, public_source.id, None, Vec::new());
        assert!(document_scope_can_contribute_to_dataset(
            &public_document,
            &public_source,
            &public_target,
        ));

        let owner = UserId(Uuid::from_u128(104));
        let private_source = dataset(
            105,
            tenant_id,
            Some(owner),
            DatasetVisibility::Private,
            Vec::new(),
            None,
        );
        let private_target = dataset(
            106,
            tenant_id,
            Some(owner),
            DatasetVisibility::Private,
            Vec::new(),
            None,
        );
        let private_document =
            scoped_document(107, tenant_id, private_source.id, Some(owner), Vec::new());
        assert!(document_scope_can_contribute_to_dataset(
            &private_document,
            &private_source,
            &private_target,
        ));
    }

    #[test]
    fn document_contribution_scope_rejects_owner_and_public_scope_widening() {
        let tenant_id = TenantId(Uuid::from_u128(110));
        let owner_a = UserId(Uuid::from_u128(111));
        let owner_b = UserId(Uuid::from_u128(112));
        let private_source = dataset(
            113,
            tenant_id,
            Some(owner_a),
            DatasetVisibility::Private,
            Vec::new(),
            None,
        );
        let public_target = dataset(
            114,
            tenant_id,
            None,
            DatasetVisibility::Public,
            Vec::new(),
            None,
        );
        let wrong_owner_target = dataset(
            115,
            tenant_id,
            Some(owner_b),
            DatasetVisibility::Private,
            Vec::new(),
            None,
        );
        let document =
            scoped_document(116, tenant_id, private_source.id, Some(owner_a), Vec::new());

        assert!(!document_scope_can_contribute_to_dataset(
            &document,
            &private_source,
            &public_target,
        ));
        assert!(!document_scope_can_contribute_to_dataset(
            &document,
            &private_source,
            &wrong_owner_target,
        ));
    }

    #[test]
    fn document_contribution_scope_requires_compatible_secret_and_local_thread_scope() {
        let tenant_id = TenantId(Uuid::from_u128(120));
        let secret_a = SecretBindingId(Uuid::from_u128(121));
        let secret_b = SecretBindingId(Uuid::from_u128(122));
        let source = dataset(
            123,
            tenant_id,
            None,
            DatasetVisibility::Private,
            vec![secret_a],
            Some("thread-a"),
        );
        let matching = dataset(
            124,
            tenant_id,
            None,
            DatasetVisibility::Private,
            vec![secret_a],
            Some("thread-a"),
        );
        let wrong_secret = dataset(
            125,
            tenant_id,
            None,
            DatasetVisibility::Private,
            vec![secret_b],
            Some("thread-a"),
        );
        let wrong_thread = dataset(
            126,
            tenant_id,
            None,
            DatasetVisibility::Private,
            vec![secret_a],
            Some("thread-b"),
        );
        let non_local_target = dataset(
            127,
            tenant_id,
            None,
            DatasetVisibility::Private,
            vec![secret_a],
            None,
        );
        let document = scoped_document(128, tenant_id, source.id, None, vec![secret_a]);

        assert!(document_scope_can_contribute_to_dataset(
            &document, &source, &matching,
        ));
        assert!(!document_scope_can_contribute_to_dataset(
            &document,
            &source,
            &wrong_secret,
        ));
        assert!(!document_scope_can_contribute_to_dataset(
            &document,
            &source,
            &wrong_thread,
        ));
        assert!(!document_scope_can_contribute_to_dataset(
            &document,
            &source,
            &non_local_target,
        ));
    }

    #[test]
    fn document_contribution_scope_is_tenant_bound_and_fail_closed() {
        let source_tenant = TenantId(Uuid::from_u128(140));
        let target_tenant = TenantId(Uuid::from_u128(141));
        let source = dataset(
            142,
            source_tenant,
            None,
            DatasetVisibility::Public,
            Vec::new(),
            None,
        );
        let target = dataset(
            143,
            target_tenant,
            None,
            DatasetVisibility::Public,
            Vec::new(),
            None,
        );
        let document = scoped_document(144, source_tenant, source.id, None, Vec::new());

        assert!(!document_scope_can_contribute_to_dataset(
            &document, &source, &target,
        ));
    }

    #[test]
    fn historical_incompatible_memberships_are_filtered_without_mutating_documents() {
        let tenant_id = TenantId(Uuid::from_u128(130));
        let owner = UserId(Uuid::from_u128(131));
        let public_target = dataset(
            132,
            tenant_id,
            None,
            DatasetVisibility::Public,
            Vec::new(),
            None,
        );
        let public_source = dataset(
            133,
            tenant_id,
            None,
            DatasetVisibility::Public,
            Vec::new(),
            None,
        );
        let private_source = dataset(
            134,
            tenant_id,
            Some(owner),
            DatasetVisibility::Private,
            Vec::new(),
            None,
        );
        let public_document = scoped_document(135, tenant_id, public_source.id, None, Vec::new());
        let private_document =
            scoped_document(136, tenant_id, private_source.id, Some(owner), Vec::new());

        let filtered = filter_documents_for_dataset_contribution(
            &public_target,
            vec![public_document.clone(), private_document.clone()],
            &[public_source, private_source],
        );

        assert_eq!(filtered.documents.len(), 1);
        assert_eq!(filtered.documents[0].id, public_document.id);
        assert_eq!(filtered.excluded.len(), 1);
        assert_eq!(filtered.excluded[0].document_id, private_document.id);
        assert_eq!(
            filtered.excluded[0].reason,
            DocumentContributionScopeMismatch::Owner
        );
        assert_eq!(private_document.dataset_id, DatasetId(Uuid::from_u128(134)));
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
