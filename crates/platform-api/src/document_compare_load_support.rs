use contracts::{CompareDocumentsRequest, CompareDocumentsView, DocumentDetailView};
use domain_model::{DatasetId, DocumentId, SecretBindingId, UserId};

use crate::{
    document_detail_load_support::load_document_detail_view_for_user,
    load_visible_document_for_user, to_compare_documents_view, ApiError, AppState,
};

pub(crate) async fn compare_documents_for_user(
    state: &AppState,
    request: CompareDocumentsRequest,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
) -> std::result::Result<CompareDocumentsView, ApiError> {
    let document_ids = unique_compare_document_ids(request.document_ids);
    ensure_compare_document_count(&document_ids)?;
    ensure_compare_documents_share_dataset(
        state,
        &document_ids,
        active_secret_binding_ids,
        current_user_id,
    )
    .await?;
    let documents = load_compare_document_details(
        state,
        document_ids,
        active_secret_binding_ids,
        current_user_id,
    )
    .await?;

    Ok(to_compare_documents_view(documents))
}

fn unique_compare_document_ids(document_ids: Vec<DocumentId>) -> Vec<DocumentId> {
    let mut unique_document_ids = Vec::new();
    for document_id in document_ids {
        if !unique_document_ids
            .iter()
            .any(|existing| *existing == document_id)
        {
            unique_document_ids.push(document_id);
        }
    }
    unique_document_ids
}

fn ensure_compare_document_count(document_ids: &[DocumentId]) -> std::result::Result<(), ApiError> {
    if document_ids.len() < 2 {
        return Err(ApiError::bad_request(
            "compare_documents_requires_multiple_documents",
            "compare_documents requires at least 2 distinct document_ids".to_string(),
        ));
    }
    Ok(())
}

async fn ensure_compare_documents_share_dataset(
    state: &AppState,
    document_ids: &[DocumentId],
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
) -> std::result::Result<(), ApiError> {
    let mut expected_dataset: Option<(DocumentId, DatasetId)> = None;
    for document_id in document_ids {
        let document = load_visible_document_for_user(
            state,
            *document_id,
            active_secret_binding_ids,
            current_user_id,
        )
        .await?;

        if let Some((expected_document_id, expected_dataset_id)) = expected_dataset {
            if document.dataset_id != expected_dataset_id {
                return Err(ApiError::bad_request(
                    "compare_documents_requires_same_dataset",
                    format!(
                        "document {} belongs to dataset {}, which does not match document {} in dataset {}",
                        document.id, document.dataset_id, expected_document_id, expected_dataset_id
                    ),
                ));
            }
        } else {
            expected_dataset = Some((document.id, document.dataset_id));
        }
    }
    Ok(())
}

async fn load_compare_document_details(
    state: &AppState,
    document_ids: Vec<DocumentId>,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
) -> std::result::Result<Vec<DocumentDetailView>, ApiError> {
    let mut documents = Vec::with_capacity(document_ids.len());
    for document_id in document_ids {
        documents.push(
            load_document_detail_view_for_user(
                state,
                document_id,
                active_secret_binding_ids,
                current_user_id,
            )
            .await?,
        );
    }
    Ok(documents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_compare_document_ids_preserve_first_seen_order() {
        let first = DocumentId::new();
        let second = DocumentId::new();
        let third = DocumentId::new();

        let unique = unique_compare_document_ids(vec![first, second, first, third, second]);

        assert_eq!(unique, vec![first, second, third]);
    }

    #[test]
    fn compare_document_count_requires_two_distinct_documents() {
        let only = DocumentId::new();

        assert!(ensure_compare_document_count(&[]).is_err());
        assert!(ensure_compare_document_count(&[only]).is_err());
        assert!(ensure_compare_document_count(&[only, DocumentId::new()]).is_ok());
    }
}
