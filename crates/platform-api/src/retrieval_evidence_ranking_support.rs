use domain_model::RetrievalEvidence;
use serde_json::Value;

pub(crate) fn sort_retrieval_evidences_by_relevance(evidences: &mut [RetrievalEvidence]) {
    evidences.sort_by(|left, right| {
        right
            .recall_score
            .partial_cmp(&left.recall_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                rank_hint_from_evidence_manifest(left)
                    .unwrap_or(usize::MAX)
                    .cmp(&rank_hint_from_evidence_manifest(right).unwrap_or(usize::MAX))
            })
            .then_with(|| left.chunk_index.cmp(&right.chunk_index))
            .then_with(|| right.created_at.cmp(&left.created_at))
    });
}

pub(crate) fn rank_hint_from_evidence_manifest(evidence: &RetrievalEvidence) -> Option<usize> {
    evidence
        .evidence_manifest
        .get("recall")
        .and_then(|value| value.get("rank_hint"))
        .and_then(Value::as_u64)
        .map(|value| value as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use domain_model::{
        DatasetId, DocumentChunkId, DocumentId, RetrievalEvidenceId, TenantId, WorkflowExecutionId,
    };
    use serde_json::json;

    fn evidence(
        label: &str,
        recall_score: f64,
        rank_hint: Option<u64>,
        chunk_index: i32,
        recency_seconds: i64,
    ) -> RetrievalEvidence {
        RetrievalEvidence {
            id: RetrievalEvidenceId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            execution_id: WorkflowExecutionId::new(),
            document_id: DocumentId::new(),
            document_chunk_id: DocumentChunkId::new(),
            chunk_index,
            source_locator: label.to_string(),
            content_excerpt: label.to_string(),
            summary: String::new(),
            payload_filter_key: format!("dataset/{label}"),
            embedding_model: "placeholder-embedding-v1".to_string(),
            recall_score,
            evidence_manifest: json!({
                "recall": {
                    "rank_hint": rank_hint,
                },
            }),
            created_at: Utc::now() + Duration::seconds(recency_seconds),
        }
    }

    #[test]
    fn rank_hint_reads_manifest_recall_rank_hint() {
        let evidence = evidence("ranked", 0.5, Some(7), 3, 0);

        assert_eq!(rank_hint_from_evidence_manifest(&evidence), Some(7));
    }

    #[test]
    fn sort_retrieval_evidences_keeps_existing_priority_order() {
        let mut evidences = vec![
            evidence("same_rank_newer", 0.8, Some(2), 4, 30),
            evidence("higher_recall", 0.9, Some(9), 9, 0),
            evidence("same_rank_lower_chunk", 0.8, Some(2), 3, -30),
            evidence("lower_rank_hint", 0.8, Some(1), 9, 0),
            evidence("missing_rank_hint", 0.8, None, 1, 60),
        ];

        sort_retrieval_evidences_by_relevance(&mut evidences);

        let order = evidences
            .iter()
            .map(|evidence| evidence.source_locator.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            order,
            vec![
                "higher_recall",
                "lower_rank_hint",
                "same_rank_lower_chunk",
                "same_rank_newer",
                "missing_rank_hint",
            ]
        );
    }
}
