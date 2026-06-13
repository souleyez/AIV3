use domain_model::RetrievalEvidence;
use serde_json::Value;
use std::collections::BTreeMap;

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

pub(crate) fn lexical_query_score(
    evidence: &RetrievalEvidence,
    query_weights: &BTreeMap<String, f64>,
    query_norm: f64,
) -> f64 {
    if query_weights.is_empty() || query_norm <= 0.0 {
        return 0.0;
    }

    let evidence_weights = evidence_term_weights_from_manifest(evidence);
    let evidence_norm = vector_norm(&evidence_weights);
    if evidence_weights.is_empty() || evidence_norm <= 0.0 {
        return 0.0;
    }

    let dot_product = query_weights
        .iter()
        .filter_map(|(term, query_weight)| {
            evidence_weights
                .get(term)
                .map(|evidence_weight| query_weight * evidence_weight)
        })
        .sum::<f64>();
    if dot_product <= 0.0 {
        return 0.0;
    }

    (dot_product / (query_norm * evidence_norm) * 10_000.0).round() / 10_000.0
}

pub(crate) fn evidence_term_weights_from_manifest(
    evidence: &RetrievalEvidence,
) -> BTreeMap<String, f64> {
    evidence
        .evidence_manifest
        .get("embedding")
        .and_then(|value| value.get("term_weights"))
        .and_then(Value::as_object)
        .map(|weights| {
            weights
                .iter()
                .filter_map(|(term, weight)| weight.as_f64().map(|value| (term.clone(), value)))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default()
}

pub(crate) fn vector_norm(weights: &BTreeMap<String, f64>) -> f64 {
    weights
        .values()
        .map(|weight| weight * weight)
        .sum::<f64>()
        .sqrt()
}

pub(crate) fn lexical_text_score(
    content: &str,
    query_weights: &BTreeMap<String, f64>,
    query_norm: f64,
) -> f64 {
    if query_weights.is_empty() || query_norm <= 0.0 {
        return 0.0;
    }

    let content_weights = crate::lexical_query_term_weights(content);
    let content_norm = vector_norm(&content_weights);
    if content_weights.is_empty() || content_norm <= 0.0 {
        return 0.0;
    }

    let dot_product = query_weights
        .iter()
        .filter_map(|(term, query_weight)| {
            content_weights
                .get(term)
                .map(|content_weight| query_weight * content_weight)
        })
        .sum::<f64>();
    if dot_product <= 0.0 {
        return 0.0;
    }

    (dot_product / (query_norm * content_norm) * 10_000.0).round() / 10_000.0
}

pub(crate) fn limited_lexical_term_weights(content: &str, limit: usize) -> BTreeMap<String, f64> {
    let mut weights = crate::lexical_query_term_weights(content)
        .into_iter()
        .collect::<Vec<_>>();
    weights.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.0.cmp(&right.0))
    });
    weights.into_iter().take(limit).collect()
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
                "embedding": {
                    "term_weights": {
                        "alpha": 3.0,
                        "beta": 4.0,
                        "ignored": null,
                        "not_number": "5",
                    },
                },
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

    #[test]
    fn evidence_term_weights_reads_numeric_embedding_terms_only() {
        let evidence = evidence("weighted", 0.5, None, 0, 0);

        assert_eq!(
            evidence_term_weights_from_manifest(&evidence)
                .into_iter()
                .collect::<Vec<_>>(),
            vec![("alpha".to_string(), 3.0), ("beta".to_string(), 4.0)]
        );
    }

    #[test]
    fn lexical_query_score_uses_cosine_score_with_existing_rounding() {
        let evidence = evidence("weighted", 0.5, None, 0, 0);
        let query_weights = BTreeMap::from([("alpha".to_string(), 3.0), ("beta".to_string(), 4.0)]);

        assert_eq!(vector_norm(&query_weights), 5.0);
        assert_eq!(lexical_query_score(&evidence, &query_weights, 5.0), 1.0);
        assert_eq!(lexical_query_score(&evidence, &BTreeMap::new(), 5.0), 0.0);
        assert_eq!(lexical_query_score(&evidence, &query_weights, 0.0), 0.0);
    }

    #[test]
    fn lexical_text_score_uses_existing_query_term_weights() {
        let query_weights = BTreeMap::from([("alpha".to_string(), 1.0)]);

        assert_eq!(lexical_text_score("alpha", &query_weights, 1.0), 1.0);
        assert_eq!(lexical_text_score("beta", &query_weights, 1.0), 0.0);
        assert_eq!(lexical_text_score("alpha", &BTreeMap::new(), 1.0), 0.0);
        assert_eq!(lexical_text_score("alpha", &query_weights, 0.0), 0.0);
    }

    #[test]
    fn limited_lexical_term_weights_keeps_highest_terms_only() {
        let weights = limited_lexical_term_weights("alpha alpha beta", 1);

        assert_eq!(weights.len(), 1);
        assert!(weights.contains_key("alpha"));
        assert!(limited_lexical_term_weights("alpha beta", 0).is_empty());
    }
}
