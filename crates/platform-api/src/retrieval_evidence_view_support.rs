use chrono::{DateTime, Utc};
use contracts::{RetrievalEvidenceManifestView, RetrievalEvidenceView};
use domain_model::{DatasetId, DocumentChunkId, DocumentId, RetrievalEvidence};
use serde_json::Value;
use uuid::Uuid;

pub(crate) fn to_retrieval_evidence_view(evidence: RetrievalEvidence) -> RetrievalEvidenceView {
    let evidence_manifest_view = parse_retrieval_evidence_manifest(&evidence);

    RetrievalEvidenceView {
        id: evidence.id,
        dataset_id: evidence.dataset_id,
        document_id: evidence.document_id,
        document_chunk_id: evidence.document_chunk_id,
        execution_id: evidence.execution_id,
        chunk_index: evidence.chunk_index,
        source_locator: evidence.source_locator,
        content_excerpt: evidence.content_excerpt,
        summary: evidence.summary,
        payload_filter_key: evidence.payload_filter_key,
        embedding_model: evidence.embedding_model,
        recall_score: evidence.recall_score,
        evidence_manifest: evidence.evidence_manifest,
        evidence_manifest_view,
        created_at: evidence.created_at,
    }
}

fn parse_manifest_timestamp(value: &Value) -> Option<DateTime<Utc>> {
    let timestamp = value.as_str()?;
    DateTime::parse_from_rfc3339(timestamp)
        .ok()
        .map(|value| value.with_timezone(&Utc))
}

fn parse_retrieval_embedding_status(
    value: &str,
) -> Option<contracts::RetrievalEmbeddingStatusView> {
    match value {
        "pending" => Some(contracts::RetrievalEmbeddingStatusView::Pending),
        "indexed" => Some(contracts::RetrievalEmbeddingStatusView::Indexed),
        "failed" => Some(contracts::RetrievalEmbeddingStatusView::Failed),
        _ => None,
    }
}

fn parse_retrieval_recall_status(value: &str) -> Option<contracts::RetrievalRecallStatusView> {
    match value {
        "pending" => Some(contracts::RetrievalRecallStatusView::Pending),
        "ready" => Some(contracts::RetrievalRecallStatusView::Ready),
        "failed" => Some(contracts::RetrievalRecallStatusView::Failed),
        _ => None,
    }
}

fn parse_retrieval_evidence_manifest(
    evidence: &RetrievalEvidence,
) -> Option<RetrievalEvidenceManifestView> {
    let object = evidence.evidence_manifest.as_object()?;
    let embedding = object.get("embedding")?.as_object()?;
    let recall = object.get("recall")?.as_object()?;
    let evidence_object = object.get("evidence")?.as_object()?;

    let parse_dataset_id = |key: &str| {
        object
            .get(key)
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .map(DatasetId::from)
    };
    let parse_document_id = |key: &str| {
        object
            .get(key)
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .map(DocumentId::from)
    };
    let parse_document_chunk_id = |container: &serde_json::Map<String, Value>, key: &str| {
        container
            .get(key)
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .map(DocumentChunkId::from)
    };

    Some(RetrievalEvidenceManifestView {
        schema_version: object
            .get("schema_version")
            .and_then(Value::as_str)
            .unwrap_or("0.2.0")
            .to_string(),
        generator: object
            .get("generator")
            .and_then(Value::as_str)
            .unwrap_or("retrieval-worker")
            .to_string(),
        dataset_id: parse_dataset_id("dataset_id").unwrap_or(evidence.dataset_id),
        document_id: parse_document_id("document_id").unwrap_or(evidence.document_id),
        document_chunk_id: parse_document_chunk_id(object, "document_chunk_id")
            .or_else(|| parse_document_chunk_id(evidence_object, "document_chunk_id"))
            .unwrap_or(evidence.document_chunk_id),
        chunk_index: object
            .get("chunk_index")
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok())
            .unwrap_or(evidence.chunk_index),
        indexed_at: object
            .get("indexed_at")
            .and_then(parse_manifest_timestamp)
            .unwrap_or(evidence.created_at),
        embedding: contracts::RetrievalEmbeddingManifestView {
            status: parse_retrieval_embedding_status(embedding.get("status")?.as_str()?)?,
            model: embedding
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or(evidence.embedding_model.as_str())
                .to_string(),
            token_count: embedding.get("token_count")?.as_u64()? as usize,
        },
        recall: contracts::RetrievalRecallManifestView {
            status: parse_retrieval_recall_status(recall.get("status")?.as_str()?)?,
            score: recall
                .get("score")
                .and_then(Value::as_f64)
                .unwrap_or(evidence.recall_score),
            rank_hint: recall
                .get("rank_hint")
                .and_then(Value::as_u64)
                .map(|value| value as usize)
                .or_else(|| {
                    usize::try_from(evidence.chunk_index)
                        .ok()
                        .map(|value| value + 1)
                })?,
        },
        evidence: contracts::RetrievalEvidenceLocatorManifestView {
            document_chunk_id: parse_document_chunk_id(object, "document_chunk_id")
                .or_else(|| parse_document_chunk_id(evidence_object, "document_chunk_id"))
                .unwrap_or(evidence.document_chunk_id),
            payload_filter_key: evidence_object
                .get("payload_filter_key")
                .and_then(Value::as_str)
                .unwrap_or(evidence.payload_filter_key.as_str())
                .to_string(),
            source_locator: evidence_object
                .get("source_locator")
                .and_then(Value::as_str)
                .unwrap_or(evidence.source_locator.as_str())
                .to_string(),
        },
    })
}

#[cfg(test)]
mod tests {
    use domain_model::{RetrievalEvidenceId, TenantId, WorkflowExecutionId};
    use serde_json::json;

    use super::*;

    fn fixed_time() -> DateTime<Utc> {
        "2026-06-14T00:00:00Z"
            .parse()
            .expect("fixed timestamp should parse")
    }

    #[test]
    fn retrieval_evidence_view_preserves_fields_and_manifest_view() {
        let now = fixed_time();
        let dataset_id = DatasetId::new();
        let document_id = DocumentId::new();
        let document_chunk_id = DocumentChunkId::new();
        let evidence = RetrievalEvidence {
            id: RetrievalEvidenceId::new(),
            tenant_id: TenantId::new(),
            dataset_id,
            execution_id: WorkflowExecutionId::new(),
            document_id,
            document_chunk_id,
            chunk_index: 4,
            source_locator: "documents/demo.pdf#chunk=4".to_string(),
            content_excerpt: "alpha beta gamma".to_string(),
            summary: "demo summary".to_string(),
            payload_filter_key: "dataset/demo".to_string(),
            embedding_model: "placeholder-embedding-v1".to_string(),
            recall_score: 0.91,
            evidence_manifest: json!({
                "schema_version": "0.2.0",
                "embedding": {
                    "status": "indexed",
                    "model": "placeholder-embedding-v1",
                    "token_count": 42,
                },
                "recall": {
                    "status": "ready",
                    "score": 0.91,
                    "rank_hint": 1,
                },
                "evidence": {
                    "document_chunk_id": document_chunk_id,
                    "payload_filter_key": "dataset/demo",
                    "source_locator": "documents/demo.pdf#chunk=4",
                },
            }),
            created_at: now,
        };
        let id = evidence.id;

        let view = to_retrieval_evidence_view(evidence);

        assert_eq!(view.id, id);
        assert_eq!(view.dataset_id, dataset_id);
        assert_eq!(view.document_id, document_id);
        assert_eq!(view.document_chunk_id, document_chunk_id);
        assert_eq!(view.chunk_index, 4);
        assert_eq!(view.payload_filter_key, "dataset/demo");
        assert_eq!(view.embedding_model, "placeholder-embedding-v1");
        assert_eq!(view.recall_score, 0.91);
        let manifest = view.evidence_manifest_view.expect("manifest should parse");
        assert_eq!(manifest.generator, "retrieval-worker");
        assert_eq!(manifest.dataset_id, dataset_id);
        assert_eq!(manifest.document_id, document_id);
        assert_eq!(manifest.document_chunk_id, document_chunk_id);
        assert_eq!(manifest.embedding.token_count, 42);
        assert_eq!(manifest.recall.rank_hint, 1);
        assert_eq!(manifest.evidence.payload_filter_key, "dataset/demo");
        assert_eq!(
            manifest.evidence.source_locator,
            "documents/demo.pdf#chunk=4"
        );
    }

    #[test]
    fn retrieval_evidence_manifest_falls_back_to_domain_fields() {
        let now = fixed_time();
        let evidence = RetrievalEvidence {
            id: RetrievalEvidenceId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            execution_id: WorkflowExecutionId::new(),
            document_id: DocumentId::new(),
            document_chunk_id: DocumentChunkId::new(),
            chunk_index: 2,
            source_locator: "documents/fallback.pdf#chunk=2".to_string(),
            content_excerpt: "fallback excerpt".to_string(),
            summary: "fallback summary".to_string(),
            payload_filter_key: "dataset/fallback".to_string(),
            embedding_model: "fallback-embedding-v1".to_string(),
            recall_score: 0.7,
            evidence_manifest: json!({
                "embedding": {
                    "status": "indexed",
                    "token_count": 12,
                },
                "recall": {
                    "status": "ready"
                },
                "evidence": {}
            }),
            created_at: now,
        };

        let view = to_retrieval_evidence_view(evidence);
        let manifest = view
            .evidence_manifest_view
            .expect("fallback manifest should parse");

        assert_eq!(manifest.dataset_id, view.dataset_id);
        assert_eq!(manifest.document_id, view.document_id);
        assert_eq!(manifest.document_chunk_id, view.document_chunk_id);
        assert_eq!(manifest.chunk_index, 2);
        assert_eq!(manifest.indexed_at, now);
        assert_eq!(manifest.embedding.model, "fallback-embedding-v1");
        assert_eq!(manifest.recall.score, 0.7);
        assert_eq!(manifest.recall.rank_hint, 3);
        assert_eq!(manifest.evidence.payload_filter_key, "dataset/fallback");
        assert_eq!(
            manifest.evidence.source_locator,
            "documents/fallback.pdf#chunk=2"
        );
    }
}
