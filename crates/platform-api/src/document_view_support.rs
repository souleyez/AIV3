use std::collections::BTreeSet;

use contracts::{DocumentChunkView, DocumentEnrichmentRunView};
use domain_model::{DatasetId, DocumentChunk};
use serde_json::{json, Map, Value};
use storage::DocumentEnrichmentRun;

use crate::document_chunk_support::document_chunk_section_title_hints;

pub(crate) fn normalize_document_dataset_ids(
    canonical_dataset_id: DatasetId,
    dataset_ids: Vec<DatasetId>,
) -> Vec<DatasetId> {
    let mut seen = BTreeSet::new();
    let mut normalized = Vec::new();
    if seen.insert(canonical_dataset_id) {
        normalized.push(canonical_dataset_id);
    }
    for dataset_id in dataset_ids {
        if seen.insert(dataset_id) {
            normalized.push(dataset_id);
        }
    }
    normalized
}

pub(crate) fn infer_media_kind_from_content_type(content_type: &str) -> &'static str {
    let lower = content_type.trim().to_ascii_lowercase();
    if lower.starts_with("audio/") {
        "audio"
    } else if lower.starts_with("video/") {
        "video"
    } else {
        "unknown"
    }
}

pub(crate) fn to_document_chunk_view(chunk: DocumentChunk) -> DocumentChunkView {
    let section_title_hints = document_chunk_section_title_hints(&chunk);
    let mut metadata = Map::from_iter(chunk.metadata);
    if !section_title_hints.is_empty() {
        metadata.insert(
            "section_title_hints".to_string(),
            json!(section_title_hints),
        );
    }

    DocumentChunkView {
        id: chunk.id,
        document_id: chunk.document_id,
        chunk_index: chunk.chunk_index,
        token_count: chunk.token_count,
        state: contracts::DocumentChunkStateView::from_domain(chunk.state),
        content: chunk.content,
        metadata: Value::Object(metadata),
        created_at: chunk.created_at,
        updated_at: chunk.updated_at,
    }
}

pub(crate) fn to_document_enrichment_run_view(
    run: DocumentEnrichmentRun,
) -> DocumentEnrichmentRunView {
    DocumentEnrichmentRunView {
        id: run.id.to_string(),
        document_id: run.document_id,
        enrichment_kind: run.enrichment_kind,
        parse_version: run.parse_version,
        input_fingerprint: run.input_fingerprint,
        status: run.status,
        priority: run.priority,
        attempt_count: run.attempt_count,
        max_attempts: run.max_attempts,
        available_at: run.available_at,
        started_at: run.started_at,
        finished_at: run.finished_at,
        error_message: run.error_message,
        output_summary: run.output_summary,
        created_at: run.created_at,
        updated_at: run.updated_at,
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use domain_model::{DocumentChunkId, DocumentChunkState, DocumentId, TenantId};
    use serde_json::json;
    use uuid::Uuid;

    use super::*;

    #[test]
    fn document_dataset_ids_keep_canonical_first_and_deduplicate() {
        let canonical_dataset_id = DatasetId::new();
        let secondary_dataset_id = DatasetId::new();
        let tertiary_dataset_id = DatasetId::new();

        let normalized = normalize_document_dataset_ids(
            canonical_dataset_id,
            vec![
                secondary_dataset_id,
                canonical_dataset_id,
                tertiary_dataset_id,
                secondary_dataset_id,
            ],
        );

        assert_eq!(
            normalized,
            vec![
                canonical_dataset_id,
                secondary_dataset_id,
                tertiary_dataset_id
            ]
        );
    }

    #[test]
    fn media_kind_inference_keeps_existing_audio_video_prefix_rules() {
        assert_eq!(infer_media_kind_from_content_type("audio/mpeg"), "audio");
        assert_eq!(
            infer_media_kind_from_content_type("  VIDEO/MP4; charset=utf-8"),
            "video"
        );
        assert_eq!(
            infer_media_kind_from_content_type("application/pdf"),
            "unknown"
        );
        assert_eq!(infer_media_kind_from_content_type(""), "unknown");
    }

    #[test]
    fn document_chunk_view_preserves_fields_and_adds_section_title_hints() {
        let now = Utc::now();
        let document_id = DocumentId::new();
        let chunk = DocumentChunk {
            id: DocumentChunkId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            document_id,
            chunk_index: 7,
            content: "## 项目背景\nalpha beta gamma".to_string(),
            token_count: 3,
            state: DocumentChunkState::Extracted,
            metadata: std::collections::BTreeMap::from_iter([(
                "source".to_string(),
                json!("fixture"),
            )]),
            created_at: now,
            updated_at: now,
        };
        let id = chunk.id;

        let view = to_document_chunk_view(chunk);

        assert_eq!(view.id, id);
        assert_eq!(view.document_id, document_id);
        assert_eq!(view.chunk_index, 7);
        assert_eq!(view.token_count, 3);
        assert_eq!(view.state, contracts::DocumentChunkStateView::Extracted);
        assert_eq!(view.content, "## 项目背景\nalpha beta gamma");
        assert_eq!(view.metadata["source"], json!("fixture"));
        assert_eq!(view.metadata["section_title_hints"][0], json!("项目背景"));
        assert_eq!(view.created_at, now);
        assert_eq!(view.updated_at, now);
    }

    #[test]
    fn document_enrichment_run_view_preserves_run_fields() {
        let now = Utc::now();
        let document_id = DocumentId::new();
        let run = DocumentEnrichmentRun {
            id: Uuid::new_v4(),
            tenant_id: TenantId::new(),
            document_id,
            enrichment_kind: "parse_quality".to_string(),
            parse_version: Some("v1".to_string()),
            input_fingerprint: "fingerprint-1".to_string(),
            status: "completed".to_string(),
            priority: 5,
            attempt_count: 2,
            max_attempts: 3,
            available_at: now,
            started_at: Some(now),
            finished_at: Some(now),
            error_message: None,
            output_summary: json!({"status": "ok"}),
            created_at: now,
            updated_at: now,
        };
        let id = run.id;

        let view = to_document_enrichment_run_view(run);

        assert_eq!(view.id, id.to_string());
        assert_eq!(view.document_id, document_id);
        assert_eq!(view.enrichment_kind, "parse_quality");
        assert_eq!(view.parse_version.as_deref(), Some("v1"));
        assert_eq!(view.input_fingerprint, "fingerprint-1");
        assert_eq!(view.status, "completed");
        assert_eq!(view.priority, 5);
        assert_eq!(view.attempt_count, 2);
        assert_eq!(view.max_attempts, 3);
        assert_eq!(view.available_at, now);
        assert_eq!(view.started_at, Some(now));
        assert_eq!(view.finished_at, Some(now));
        assert!(view.error_message.is_none());
        assert_eq!(view.output_summary, json!({"status": "ok"}));
        assert_eq!(view.created_at, now);
        assert_eq!(view.updated_at, now);
    }
}
