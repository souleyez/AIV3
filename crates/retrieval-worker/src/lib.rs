use domain_model::{DatasetId, DocumentId};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const SIGNATURE_TERM_LIMIT: usize = 12;
const TERM_WEIGHT_LIMIT: usize = 16;
const CJK_NGRAM_MAX: usize = 6;

#[derive(Clone, Debug)]
pub struct RetrievalChunkInput {
    pub chunk_index: i32,
    pub content: String,
    pub token_count: usize,
}

#[derive(Clone, Debug)]
pub struct RetrievalIndexJob {
    pub dataset_id: DatasetId,
    pub document_id: DocumentId,
    pub chunks: Vec<RetrievalChunkInput>,
}

#[derive(Clone, Debug)]
pub struct RetrievalChunkProfile {
    pub chunk_index: i32,
    pub recall_score: f64,
    pub rank_hint: usize,
    pub signature_terms: Vec<String>,
    pub term_weights: BTreeMap<String, f64>,
    pub vector_norm: f64,
    pub token_count: usize,
}

#[derive(Clone, Debug)]
pub struct RetrievalIndexOutcome {
    pub embedded_chunks: u32,
    pub payload_filter_key: String,
    pub embedding_model: String,
    pub chunk_profiles: Vec<RetrievalChunkProfile>,
}

pub trait RetrievalIndexer {
    fn index(&self, job: &RetrievalIndexJob) -> RetrievalIndexOutcome;
}

#[derive(Clone, Debug)]
pub struct LocalLexicalRetrievalIndexer;

impl RetrievalIndexer for LocalLexicalRetrievalIndexer {
    fn index(&self, job: &RetrievalIndexJob) -> RetrievalIndexOutcome {
        let chunk_term_frequencies = job
            .chunks
            .iter()
            .map(|chunk| term_frequencies(&chunk.content))
            .collect::<Vec<_>>();
        let document_frequencies = document_frequencies(&chunk_term_frequencies);
        let chunk_count = job.chunks.len().max(1) as f64;

        let mut draft_profiles = job
            .chunks
            .iter()
            .zip(chunk_term_frequencies.iter())
            .map(|(chunk, frequencies)| {
                let weighted_terms =
                    weighted_terms_for_chunk(frequencies, &document_frequencies, chunk_count);
                let vector_norm = weighted_terms
                    .iter()
                    .map(|(_, weight)| weight * weight)
                    .sum::<f64>()
                    .sqrt();
                let signature_terms = weighted_terms
                    .iter()
                    .take(SIGNATURE_TERM_LIMIT)
                    .map(|(term, _)| term.clone())
                    .collect::<Vec<_>>();
                let term_weights = weighted_terms
                    .iter()
                    .take(TERM_WEIGHT_LIMIT)
                    .map(|(term, weight)| (term.clone(), round_metric(*weight)))
                    .collect::<BTreeMap<_, _>>();
                let lexical_salience =
                    lexical_salience_score(vector_norm, chunk.token_count, weighted_terms.len());

                (
                    chunk.chunk_index,
                    chunk.token_count,
                    signature_terms,
                    term_weights,
                    vector_norm,
                    lexical_salience,
                )
            })
            .collect::<Vec<_>>();

        let max_salience = draft_profiles
            .iter()
            .map(|(_, _, _, _, _, salience)| *salience)
            .fold(0.0_f64, f64::max);

        let mut ranked = draft_profiles
            .iter()
            .map(|(chunk_index, _, _, _, _, salience)| (*chunk_index, *salience))
            .collect::<Vec<_>>();
        ranked.sort_by(|left, right| {
            right
                .1
                .partial_cmp(&left.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.0.cmp(&right.0))
        });
        let rank_hints = ranked
            .into_iter()
            .enumerate()
            .map(|(index, (chunk_index, _))| (chunk_index, index + 1))
            .collect::<BTreeMap<_, _>>();

        let chunk_profiles = draft_profiles
            .drain(..)
            .map(
                |(
                    chunk_index,
                    token_count,
                    signature_terms,
                    term_weights,
                    vector_norm,
                    lexical_salience,
                )| RetrievalChunkProfile {
                    chunk_index,
                    recall_score: if max_salience > 0.0 {
                        round_metric(lexical_salience / max_salience)
                    } else {
                        0.0
                    },
                    rank_hint: rank_hints.get(&chunk_index).copied().unwrap_or(1),
                    signature_terms,
                    term_weights,
                    vector_norm: round_metric(vector_norm),
                    token_count,
                },
            )
            .collect::<Vec<_>>();

        RetrievalIndexOutcome {
            embedded_chunks: job.chunks.len() as u32,
            payload_filter_key: format!("dataset/{}", job.dataset_id),
            embedding_model: "local-lexical-v1".to_string(),
            chunk_profiles,
        }
    }
}

fn document_frequencies(
    chunk_term_frequencies: &[BTreeMap<String, usize>],
) -> BTreeMap<String, usize> {
    let mut frequencies = BTreeMap::new();

    for terms in chunk_term_frequencies {
        for term in terms.keys().collect::<BTreeSet<_>>() {
            *frequencies.entry((*term).clone()).or_insert(0) += 1;
        }
    }

    frequencies
}

fn weighted_terms_for_chunk(
    term_frequencies: &BTreeMap<String, usize>,
    document_frequencies: &BTreeMap<String, usize>,
    chunk_count: f64,
) -> Vec<(String, f64)> {
    let mut weighted_terms = term_frequencies
        .iter()
        .map(|(term, count)| {
            let tf_weight = 1.0 + (*count as f64).ln();
            let document_frequency = *document_frequencies.get(term).unwrap_or(&1) as f64;
            let inverse_document_frequency =
                ((chunk_count + 1.0) / (document_frequency + 1.0)).ln() + 1.0;
            (
                term.clone(),
                tf_weight * inverse_document_frequency * cjk_phrase_boost(term),
            )
        })
        .collect::<Vec<_>>();
    weighted_terms.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.0.cmp(&right.0))
    });
    weighted_terms
}

fn lexical_salience_score(vector_norm: f64, token_count: usize, weighted_term_count: usize) -> f64 {
    if vector_norm <= 0.0 {
        return 0.0;
    }

    let token_factor = (token_count.max(1) as f64).ln() + 1.0;
    let diversity_factor = (weighted_term_count.max(1) as f64).sqrt();

    vector_norm * token_factor * diversity_factor
}

fn term_frequencies(content: &str) -> BTreeMap<String, usize> {
    let mut frequencies = BTreeMap::new();
    for token in tokenize(content) {
        *frequencies.entry(token).or_insert(0) += 1;
    }
    frequencies
}

fn tokenize(content: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut ascii_token = String::new();
    let mut cjk_chars = Vec::new();

    for value in content.chars() {
        if value.is_ascii_alphanumeric() {
            flush_cjk_terms(&mut tokens, &mut cjk_chars);
            ascii_token.push(value.to_ascii_lowercase());
            continue;
        }
        flush_ascii_token(&mut tokens, &mut ascii_token);
        if is_cjk_token_char(value) {
            cjk_chars.push(value);
        } else {
            flush_cjk_terms(&mut tokens, &mut cjk_chars);
        }
    }
    flush_ascii_token(&mut tokens, &mut ascii_token);
    flush_cjk_terms(&mut tokens, &mut cjk_chars);
    tokens
}

pub fn lexical_search_terms(content: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    tokenize(content)
        .into_iter()
        .filter(|term| seen.insert(term.clone()))
        .collect()
}

pub fn lexical_content_hash(content: &str) -> String {
    let digest = Sha256::digest(content.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn flush_ascii_token(tokens: &mut Vec<String>, ascii_token: &mut String) {
    if let Some(token) = normalize_token(ascii_token) {
        tokens.push(token);
    }
    ascii_token.clear();
}

fn flush_cjk_terms(tokens: &mut Vec<String>, cjk_chars: &mut Vec<char>) {
    if cjk_chars.is_empty() {
        return;
    }

    for value in cjk_chars.iter() {
        tokens.push(value.to_string());
    }
    for ngram_size in 2..=CJK_NGRAM_MAX.min(cjk_chars.len()) {
        for window in cjk_chars.windows(ngram_size) {
            tokens.push(window.iter().collect::<String>());
        }
    }
    cjk_chars.clear();
}

fn is_cjk_token_char(value: char) -> bool {
    matches!(
        value as u32,
        0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0xF900..=0xFAFF
    )
}

fn cjk_phrase_boost(term: &str) -> f64 {
    let mut char_count = 0;
    let mut cjk_count = 0;
    for value in term.chars() {
        char_count += 1;
        if is_cjk_token_char(value) {
            cjk_count += 1;
        }
    }
    if char_count >= 2 && char_count == cjk_count {
        1.0 + ((char_count - 1) as f64 * 0.15).min(0.75)
    } else {
        1.0
    }
}

fn normalize_token(token: &str) -> Option<String> {
    if token.len() < 2 || is_stop_word(token) {
        return None;
    }

    Some(token.to_string())
}

fn is_stop_word(token: &str) -> bool {
    matches!(
        token,
        "a" | "an"
            | "and"
            | "are"
            | "as"
            | "at"
            | "be"
            | "by"
            | "for"
            | "from"
            | "in"
            | "into"
            | "is"
            | "it"
            | "of"
            | "on"
            | "or"
            | "that"
            | "the"
            | "this"
            | "to"
            | "was"
            | "were"
            | "with"
    )
}

fn round_metric(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_lexical_retrieval_indexer_uses_chunk_content() {
        let dataset_id = DatasetId::new();
        let outcome = LocalLexicalRetrievalIndexer.index(&RetrievalIndexJob {
            dataset_id,
            document_id: DocumentId::new(),
            chunks: vec![
                RetrievalChunkInput {
                    chunk_index: 0,
                    content: "Revenue revenue margin margin operating profit".to_string(),
                    token_count: 6,
                },
                RetrievalChunkInput {
                    chunk_index: 1,
                    content: "Product roadmap design system mobile navigation".to_string(),
                    token_count: 6,
                },
            ],
        });

        assert_eq!(outcome.embedded_chunks, 2);
        assert_eq!(outcome.payload_filter_key, format!("dataset/{dataset_id}"));
        assert_eq!(outcome.embedding_model, "local-lexical-v1");
        assert_eq!(outcome.chunk_profiles.len(), 2);
        assert_eq!(outcome.chunk_profiles[0].chunk_index, 0);
        assert_eq!(outcome.chunk_profiles[1].chunk_index, 1);
        assert_eq!(outcome.chunk_profiles[0].token_count, 6);
        assert!(!outcome.chunk_profiles[0].signature_terms.is_empty());
        assert!(outcome.chunk_profiles[0]
            .term_weights
            .contains_key("revenue"));
        assert!(outcome.chunk_profiles[1]
            .term_weights
            .contains_key("roadmap"));
        assert!(outcome
            .chunk_profiles
            .iter()
            .all(|profile| profile.rank_hint >= 1));
        assert!(outcome
            .chunk_profiles
            .iter()
            .all(|profile| (0.0..=1.0).contains(&profile.recall_score)));
    }

    #[test]
    fn tokenize_drops_stop_words_and_normalizes_terms() {
        assert_eq!(
            tokenize("The revenue, margin, and growth plan."),
            vec![
                "revenue".to_string(),
                "margin".to_string(),
                "growth".to_string(),
                "plan".to_string(),
            ]
        );
    }

    #[test]
    fn tokenize_keeps_cjk_terms_for_chinese_materials() {
        let tokens = tokenize("订单金额增长 revenue");

        assert!(tokens.contains(&"订".to_string()));
        assert!(tokens.contains(&"单".to_string()));
        assert!(tokens.contains(&"金".to_string()));
        assert!(tokens.contains(&"额".to_string()));
        assert!(tokens.contains(&"订单".to_string()));
        assert!(tokens.contains(&"金额".to_string()));
        assert!(tokens.contains(&"增长".to_string()));
        assert!(tokens.contains(&"订单金".to_string()));
        assert!(tokens.contains(&"金额增".to_string()));
        assert!(tokens.contains(&"订单金额增长".to_string()));
        assert!(tokens.contains(&"revenue".to_string()));
    }

    #[test]
    fn lexical_search_terms_are_unique_and_hash_is_stable() {
        let terms = lexical_search_terms("订单延期风险 订单延期风险 revenue");

        assert_eq!(
            terms.iter().filter(|term| *term == "订单延期风险").count(),
            1
        );
        assert!(terms.contains(&"revenue".to_string()));
        assert_eq!(
            lexical_content_hash("订单延期风险"),
            lexical_content_hash("订单延期风险")
        );
        assert_ne!(
            lexical_content_hash("订单延期风险"),
            lexical_content_hash("订单延期")
        );
    }

    #[test]
    fn local_lexical_retrieval_indexer_profiles_cjk_business_phrases() {
        let outcome = LocalLexicalRetrievalIndexer.index(&RetrievalIndexJob {
            dataset_id: DatasetId::new(),
            document_id: DocumentId::new(),
            chunks: vec![
                RetrievalChunkInput {
                    chunk_index: 0,
                    content: "订单延期风险 订单延期风险 仓库交接赔付".to_string(),
                    token_count: 8,
                },
                RetrievalChunkInput {
                    chunk_index: 1,
                    content: "客服满意度提升 会员复购增长".to_string(),
                    token_count: 7,
                },
            ],
        });

        let phrase_profile = outcome
            .chunk_profiles
            .iter()
            .find(|profile| profile.chunk_index == 0)
            .expect("phrase chunk profile should exist");

        assert!(phrase_profile.term_weights.contains_key("订单"));
        assert!(phrase_profile.term_weights.contains_key("延期"));
        assert!(phrase_profile.term_weights.contains_key("风险"));
        assert!(phrase_profile.term_weights.contains_key("订单延期风险"));
        assert!(phrase_profile.recall_score > 0.0);
    }

    #[test]
    fn local_lexical_retrieval_indexer_preserves_realistic_business_phrases() {
        let outcome = LocalLexicalRetrievalIndexer.index(&RetrievalIndexJob {
            dataset_id: DatasetId::new(),
            document_id: DocumentId::new(),
            chunks: vec![
                RetrievalChunkInput {
                    chunk_index: 0,
                    content: "订单延期风险集中在华东仓库交接，超过两天需要赔付提醒。".to_string(),
                    token_count: 22,
                },
                RetrievalChunkInput {
                    chunk_index: 1,
                    content: "客户满意度下降主要来自客服响应慢，工单需要升级处理。".to_string(),
                    token_count: 23,
                },
                RetrievalChunkInput {
                    chunk_index: 2,
                    content: "企业问答手册记录了员工报销流程和审批制度。".to_string(),
                    token_count: 18,
                },
            ],
        });

        let order_profile = outcome
            .chunk_profiles
            .iter()
            .find(|profile| profile.chunk_index == 0)
            .expect("order profile should exist");
        let support_profile = outcome
            .chunk_profiles
            .iter()
            .find(|profile| profile.chunk_index == 1)
            .expect("support profile should exist");

        assert!(order_profile.term_weights.contains_key("订单延期风险"));
        assert!(order_profile
            .signature_terms
            .contains(&"订单延期风险".to_string()));
        assert!(support_profile
            .term_weights
            .keys()
            .any(|term| term.contains("客户满意度")));
        assert!(support_profile
            .signature_terms
            .iter()
            .any(|term| term.contains("客户满意度")));
        assert!(order_profile.vector_norm > 0.0);
        assert!(support_profile.vector_norm > 0.0);
    }

    #[test]
    fn tokenize_keeps_cjk_ngrams_separate_across_ascii_boundaries() {
        let tokens = tokenize("订单risk取消");

        assert!(tokens.contains(&"订单".to_string()));
        assert!(tokens.contains(&"risk".to_string()));
        assert!(tokens.contains(&"取消".to_string()));
        assert!(!tokens.contains(&"单取".to_string()));
    }
}
