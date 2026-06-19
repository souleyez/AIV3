use chrono::{DateTime, Utc};
use contracts::UpdateDocumentRequest;
use domain_model::{Document, DocumentId, DocumentLifecycle, SecretBindingId, UserId};
use serde_json::{json, Value};

use crate::{
    lifecycle_updates::parse_document_lifecycle_update, load_visible_document_for_user,
    resource_access::ensure_owner_managed_resource, static_page_payload_support::set_payload_value,
    text_normalization::trim_optional, validate_required, ApiError, AppState,
};

pub(crate) async fn update_document_for_user(
    state: &AppState,
    document_id: DocumentId,
    request: UpdateDocumentRequest,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
) -> std::result::Result<Document, ApiError> {
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

    let prepared = prepare_document_update(request, document.lifecycle.clone(), Utc::now())?;

    state
        .storage
        .documents()
        .update_state(
            state.tenant_id,
            document_id,
            prepared.lifecycle,
            prepared.title.as_deref(),
            &prepared.metadata,
            Utc::now(),
        )
        .await
        .map_err(ApiError::from_storage)
}

#[derive(Debug, PartialEq)]
struct PreparedDocumentUpdate {
    lifecycle: DocumentLifecycle,
    title: Option<String>,
    metadata: Value,
}

fn prepare_document_update(
    request: UpdateDocumentRequest,
    current_lifecycle: DocumentLifecycle,
    archived_at: DateTime<Utc>,
) -> std::result::Result<PreparedDocumentUpdate, ApiError> {
    let title = trim_optional(request.title);
    if let Some(title) = title.as_deref() {
        validate_required("title", title)?;
    }
    let lifecycle = parse_document_lifecycle_update(request.lifecycle)?;
    let mut metadata = if request.metadata.is_object() {
        request.metadata
    } else {
        json!({})
    };
    if lifecycle == Some(DocumentLifecycle::Archived) {
        set_payload_value(&mut metadata, "archived_at", json!(archived_at));
    }

    Ok(PreparedDocumentUpdate {
        lifecycle: lifecycle.unwrap_or(current_lifecycle),
        title,
        metadata,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn update_request(
        title: Option<&str>,
        lifecycle: Option<&str>,
        metadata: Value,
    ) -> UpdateDocumentRequest {
        UpdateDocumentRequest {
            title: title.map(str::to_string),
            lifecycle: lifecycle.map(str::to_string),
            metadata,
        }
    }

    #[test]
    fn prepare_document_update_trims_title_and_ignores_non_object_metadata() {
        let prepared = prepare_document_update(
            update_request(Some("  Updated title  "), None, json!("ignored")),
            DocumentLifecycle::Indexed,
            Utc::now(),
        )
        .expect("document update should prepare");

        assert_eq!(prepared.lifecycle, DocumentLifecycle::Indexed);
        assert_eq!(prepared.title.as_deref(), Some("Updated title"));
        assert_eq!(prepared.metadata, json!({}));
    }

    #[test]
    fn prepare_document_update_adds_archived_at_to_object_metadata() {
        let archived_at = Utc
            .with_ymd_and_hms(2026, 6, 19, 10, 30, 0)
            .single()
            .expect("fixed timestamp should be valid");

        let prepared = prepare_document_update(
            update_request(
                None,
                Some(" Archived "),
                json!({
                    "source": "manual"
                }),
            ),
            DocumentLifecycle::Indexed,
            archived_at,
        )
        .expect("archive update should prepare");

        assert_eq!(prepared.lifecycle, DocumentLifecycle::Archived);
        assert_eq!(prepared.title, None);
        assert_eq!(prepared.metadata.get("source"), Some(&json!("manual")));
        assert_eq!(
            prepared.metadata.get("archived_at"),
            Some(&json!(archived_at))
        );
    }

    #[test]
    fn prepare_document_update_rejects_unsupported_lifecycle() {
        let error = prepare_document_update(
            update_request(None, Some("deleted"), json!({})),
            DocumentLifecycle::Indexed,
            Utc::now(),
        )
        .expect_err("unsupported lifecycle should fail");

        assert_eq!(error.payload.code, "validation_error");
        assert!(error
            .payload
            .message
            .contains("unsupported document lifecycle: deleted"));
    }
}
