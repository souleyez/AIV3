use contracts::{RegisterDocumentRequest, RegisterDocumentResponse};
use domain_model::{SecretBindingId, UserId};
use storage::NewDocument;

use crate::{
    load_visible_dataset_for_user_with_local_scope,
    record_local_document_content_fingerprint_if_available, to_document_summary, validate_required,
    ApiError, AppState,
};

pub(crate) fn validate_register_document_request(
    request: &RegisterDocumentRequest,
) -> std::result::Result<(), ApiError> {
    validate_required("title", &request.title)?;
    validate_required("object_key", &request.object_key)?;
    validate_required("content_type", &request.content_type)
}

pub(crate) async fn register_document_for_user(
    state: &AppState,
    request: RegisterDocumentRequest,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
    local_thread_id: Option<&str>,
) -> std::result::Result<RegisterDocumentResponse, ApiError> {
    load_visible_dataset_for_user_with_local_scope(
        state,
        request.dataset_id,
        active_secret_binding_ids,
        current_user_id,
        local_thread_id,
    )
    .await?;

    let document = state
        .storage
        .documents()
        .create(
            state.tenant_id,
            new_document_from_register_request(request, current_user_id),
        )
        .await
        .map_err(ApiError::from_storage)?;
    let document =
        record_local_document_content_fingerprint_if_available(state, document, chrono::Utc::now())
            .await;
    if let Err(error) = state
        .storage
        .asset_items()
        .sync_document_asset_profile(state.tenant_id, &document, &[])
        .await
    {
        tracing::warn!(
            error = ?error,
            document_id = %document.id,
            dataset_id = %document.dataset_id,
            "document asset profile sync failed after register; upload response will continue"
        );
    }

    Ok(RegisterDocumentResponse {
        document: to_document_summary(document),
    })
}

fn new_document_from_register_request(
    request: RegisterDocumentRequest,
    owner_user_id: Option<UserId>,
) -> NewDocument {
    NewDocument {
        dataset_id: request.dataset_id,
        title: request.title.trim().to_string(),
        object_key: request.object_key.trim().to_string(),
        content_type: request.content_type.trim().to_string(),
        secret_binding_ids: request.secret_binding_ids,
        owner_user_id,
        metadata: request.metadata,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain_model::DatasetId;
    use serde_json::{json, Value};
    use uuid::Uuid;

    fn register_request(
        title: &str,
        object_key: &str,
        content_type: &str,
        metadata: Value,
    ) -> RegisterDocumentRequest {
        RegisterDocumentRequest {
            dataset_id: DatasetId(Uuid::from_u128(1)),
            title: title.to_string(),
            object_key: object_key.to_string(),
            content_type: content_type.to_string(),
            secret_binding_ids: vec![SecretBindingId(Uuid::from_u128(2))],
            metadata,
        }
    }

    #[test]
    fn register_document_request_validation_keeps_existing_required_fields() {
        assert!(validate_register_document_request(&register_request(
            "Title",
            "objects/doc.pdf",
            "application/pdf",
            json!({})
        ))
        .is_ok());

        let title_error = validate_register_document_request(&register_request(
            " ",
            "objects/doc.pdf",
            "text/plain",
            json!({}),
        ))
        .expect_err("blank title should fail");
        assert_eq!(title_error.payload.code, "validation_error");
        assert!(title_error
            .payload
            .message
            .contains("title must not be empty"));

        let object_key_error = validate_register_document_request(&register_request(
            "Title",
            " ",
            "text/plain",
            json!({}),
        ))
        .expect_err("blank object key should fail");
        assert_eq!(object_key_error.payload.code, "validation_error");
        assert!(object_key_error
            .payload
            .message
            .contains("object_key must not be empty"));

        let content_type_error = validate_register_document_request(&register_request(
            "Title",
            "objects/doc.pdf",
            " ",
            json!({}),
        ))
        .expect_err("blank content type should fail");
        assert_eq!(content_type_error.payload.code, "validation_error");
        assert!(content_type_error
            .payload
            .message
            .contains("content_type must not be empty"));
    }

    #[test]
    fn new_document_from_register_request_trims_storage_fields_and_preserves_payload() {
        let owner_user_id = UserId(Uuid::from_u128(3));
        let metadata = json!({
            "source": "upload"
        });
        let request = register_request(
            "  Upload title  ",
            "  C:/tmp/upload.pdf  ",
            "  application/pdf  ",
            metadata.clone(),
        );
        let dataset_id = request.dataset_id;
        let secret_binding_ids = request.secret_binding_ids.clone();

        let document = new_document_from_register_request(request, Some(owner_user_id));

        assert_eq!(document.dataset_id, dataset_id);
        assert_eq!(document.title, "Upload title");
        assert_eq!(document.object_key, "C:/tmp/upload.pdf");
        assert_eq!(document.content_type, "application/pdf");
        assert_eq!(document.secret_binding_ids, secret_binding_ids);
        assert_eq!(document.owner_user_id, Some(owner_user_id));
        assert_eq!(document.metadata, metadata);
    }
}
