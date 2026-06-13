use domain_model::{DocumentChunk, RetrievalEvidence};
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

pub(crate) fn retrieval_evidence_search_text(evidence: &RetrievalEvidence) -> String {
    let mut parts = Vec::new();
    for value in [
        evidence.summary.trim(),
        evidence.content_excerpt.trim(),
        evidence.source_locator.trim(),
        evidence.payload_filter_key.trim(),
    ] {
        if !value.is_empty() {
            parts.push(value.to_string());
        }
    }
    let mut section_title_hints = Vec::new();
    for pointer in [
        "/evidence/section_title_hints",
        "/section_title_hints",
        "/metadata/section_title_hints",
    ] {
        if let Some(value) = evidence.evidence_manifest.pointer(pointer) {
            crate::collect_string_list(value, &mut section_title_hints);
        }
    }
    parts.extend(section_title_hints);
    parts.join("\n")
}

pub(crate) fn document_chunk_looks_like_toc_or_index(chunk: &DocumentChunk) -> bool {
    let content = chunk.content.trim();
    if content.is_empty() {
        return false;
    }
    let dotted_leader_count = content.matches("....").count();
    let has_page_tail = content
        .split_whitespace()
        .last()
        .is_some_and(|tail| tail.chars().all(|ch| ch.is_ascii_digit()) && tail.len() <= 4);
    let short_line_count = content
        .lines()
        .filter(|line| line.trim().len() <= 80)
        .count();
    let line_count = content.lines().count().max(1);
    dotted_leader_count >= 1
        || (content.contains("目录") && short_line_count >= line_count.saturating_sub(1))
        || (has_page_tail && content.contains('…'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use domain_model::{
        DatasetId, DocumentChunkId, DocumentChunkState, DocumentId, RetrievalEvidenceId, TenantId,
        WorkflowExecutionId,
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

    fn chunk(content: &str) -> DocumentChunk {
        let now = Utc::now();
        DocumentChunk {
            id: DocumentChunkId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            document_id: DocumentId::new(),
            chunk_index: 0,
            content: content.to_string(),
            token_count: content.split_whitespace().count() as i32,
            state: DocumentChunkState::Extracted,
            metadata: BTreeMap::new(),
            created_at: now,
            updated_at: now,
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

    #[test]
    fn retrieval_evidence_search_text_includes_manifest_section_hints() {
        let mut evidence = evidence("ranked", 0.5, None, 0, 0);
        evidence.summary = "summary".to_string();
        evidence.content_excerpt = "excerpt".to_string();
        evidence.source_locator = "source.pdf#chunk=1".to_string();
        evidence.payload_filter_key = "dataset/demo".to_string();
        evidence.evidence_manifest = json!({
            "evidence": {
                "section_title_hints": ["经营分析", "取高机会"]
            },
            "section_title_hints": ["经营分析"],
            "metadata": {
                "section_title_hints": ["低活跃风险"]
            }
        });

        let search_text = retrieval_evidence_search_text(&evidence);

        assert!(search_text.contains("summary"));
        assert!(search_text.contains("excerpt"));
        assert!(search_text.contains("source.pdf#chunk=1"));
        assert!(search_text.contains("dataset/demo"));
        assert!(search_text.contains("经营分析"));
        assert!(search_text.contains("取高机会"));
        assert!(search_text.contains("低活跃风险"));
    }

    #[test]
    fn document_chunk_toc_detection_keeps_existing_signals() {
        assert!(document_chunk_looks_like_toc_or_index(&chunk(
            "一、总则 .... 1\n二、细则 .... 2"
        )));
        assert!(document_chunk_looks_like_toc_or_index(&chunk(
            "目录\n一、总则\n二、细则"
        )));
        assert!(document_chunk_looks_like_toc_or_index(&chunk(
            "经营分析 … 12"
        )));
        assert!(!document_chunk_looks_like_toc_or_index(&chunk(
            "这里是一段普通正文，描述合同续签流程和审批动作。"
        )));
    }
}
