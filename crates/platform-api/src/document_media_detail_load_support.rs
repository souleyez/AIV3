use contracts::DocumentMediaDetailView;
use domain_model::{DocumentId, SecretBindingId, UserId};

use crate::{
    load_visible_document_for_user_with_local_scope, to_document_media_detail_view, ApiError,
    AppState,
};

pub(crate) async fn load_document_media_detail_view_for_user(
    state: &AppState,
    document_id: DocumentId,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
    local_thread_id: Option<&str>,
) -> std::result::Result<DocumentMediaDetailView, ApiError> {
    let document = load_visible_document_for_user_with_local_scope(
        state,
        document_id,
        active_secret_binding_ids,
        current_user_id,
        local_thread_id,
    )
    .await?;
    let chunks = state
        .storage
        .document_chunks()
        .list_by_document_or_canonical(state.tenant_id, document_id)
        .await
        .map_err(ApiError::from_storage)?;

    Ok(to_document_media_detail_view(document, chunks))
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use domain_model::{
        DatasetId, Document, DocumentChunk, DocumentChunkId, DocumentChunkState, DocumentId,
        DocumentLifecycle, TenantId,
    };
    use serde_json::json;
    use std::collections::BTreeMap;

    use super::*;

    fn document(document_id: DocumentId, dataset_id: DatasetId) -> Document {
        let now = Utc::now();
        Document {
            id: document_id,
            tenant_id: TenantId::new(),
            dataset_id,
            owner_user_id: None,
            title: "Media detail fixture".to_string(),
            object_key: "/tmp/media-detail-fixture.mp4".to_string(),
            content_type: "video/mp4".to_string(),
            lifecycle: DocumentLifecycle::Indexed,
            secret_binding_ids: Vec::new(),
            metadata: BTreeMap::new(),
            created_at: now,
            updated_at: now,
        }
    }

    fn media_chunk(document_id: DocumentId, dataset_id: DatasetId) -> DocumentChunk {
        let now = Utc::now();
        DocumentChunk {
            id: DocumentChunkId::new(),
            tenant_id: TenantId::new(),
            dataset_id,
            document_id,
            chunk_index: 0,
            content: "media metadata".to_string(),
            token_count: 2,
            state: DocumentChunkState::Extracted,
            metadata: BTreeMap::from_iter([(
                "media".to_string(),
                json!({
                    "kind": "video",
                    "parse_status": "completed",
                    "transcript_segments": [
                        {
                            "start_seconds": 0.0,
                            "end_seconds": 3.0,
                            "text": "hello",
                            "source": "fixture"
                        }
                    ],
                    "scenes": [],
                    "keyframe_ocr_snippets": [],
                    "provider_evidence": []
                }),
            )]),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn media_detail_view_assembly_preserves_media_metadata() {
        let document_id = DocumentId::new();
        let dataset_id = DatasetId::new();
        let view = to_document_media_detail_view(
            document(document_id, dataset_id),
            vec![media_chunk(document_id, dataset_id)],
        );

        assert_eq!(view.document.id, document_id);
        assert_eq!(view.media_kind, "video");
        assert_eq!(view.parse_status, "completed");
        assert_eq!(view.transcript_segments.len(), 1);
        assert_eq!(view.transcript_segments[0].text, "hello");
        assert!(view.model_facing.is_some());
    }
}
