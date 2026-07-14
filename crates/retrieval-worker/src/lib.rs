use anyhow::{anyhow, Result};
use domain_model::{DatasetId, DocumentId, TenantId};
use platform_api::cross_dataset_semantic_graph::{
    match_cross_dataset_semantic_graph, CrossDatasetCanonicalIdentity,
    CrossDatasetSemanticEndpointInput, CrossDatasetSemanticMatchInput,
    CrossDatasetSemanticMatcherLimits, CrossDatasetSemanticMatcherOptions,
    DatasetSemanticGraphDataset, DatasetSemanticGraphNodeKind, DatasetSemanticGraphV1,
    DATASET_SEMANTIC_GRAPH_GENERATION_VERSION,
};
use platform_api::semantic_understanding::{
    stable_semantic_id, DatasetSemanticUnderstanding, SemanticEvidenceClass,
};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use storage::{DatasetSemanticLinkRun, DatasetSemanticSnapshot};

pub use platform_api::dataset_semantic_snapshot::{
    dataset_cross_semantic_graph_access, dataset_cross_semantic_graph_access_from_values,
    DatasetCrossSemanticGraphAccess, DATASET_CROSS_SEMANTIC_GRAPH_DATASET_ALLOWLIST_ENV,
    DATASET_CROSS_SEMANTIC_GRAPH_ENABLED_ENV, DATASET_CROSS_SEMANTIC_GRAPH_TENANT_ALLOWLIST_ENV,
};

const SIGNATURE_TERM_LIMIT: usize = 12;
const TERM_WEIGHT_LIMIT: usize = 16;
const CJK_NGRAM_MAX: usize = 6;

pub const DATASET_SEMANTIC_UNDERSTANDING_ENABLED_ENV: &str =
    "DATASET_SEMANTIC_UNDERSTANDING_ENABLED";
pub const DATASET_SEMANTIC_UNDERSTANDING_TENANT_ALLOWLIST_ENV: &str =
    "DATASET_SEMANTIC_UNDERSTANDING_TENANT_ALLOWLIST";
pub const DATASET_SEMANTIC_UNDERSTANDING_DATASET_ALLOWLIST_ENV: &str =
    "DATASET_SEMANTIC_UNDERSTANDING_DATASET_ALLOWLIST";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DatasetSemanticUnderstandingAccess {
    Allowed,
    FeatureDisabled,
    TenantNotAllowlisted,
    DatasetNotAllowlisted,
}

impl DatasetSemanticUnderstandingAccess {
    pub fn is_allowed(self) -> bool {
        self == Self::Allowed
    }

    pub fn safe_reason(self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::FeatureDisabled => "feature_disabled",
            Self::TenantNotAllowlisted => "tenant_not_allowlisted",
            Self::DatasetNotAllowlisted => "dataset_not_allowlisted",
        }
    }
}

pub fn dataset_semantic_understanding_access(
    tenant_id: TenantId,
    dataset_id: DatasetId,
) -> DatasetSemanticUnderstandingAccess {
    dataset_semantic_understanding_access_from_values(
        env_flag_value(DATASET_SEMANTIC_UNDERSTANDING_ENABLED_ENV, false),
        std::env::var(DATASET_SEMANTIC_UNDERSTANDING_TENANT_ALLOWLIST_ENV)
            .ok()
            .as_deref(),
        std::env::var(DATASET_SEMANTIC_UNDERSTANDING_DATASET_ALLOWLIST_ENV)
            .ok()
            .as_deref(),
        tenant_id,
        dataset_id,
    )
}

pub fn dataset_semantic_understanding_access_from_values(
    enabled: bool,
    tenant_allowlist: Option<&str>,
    dataset_allowlist: Option<&str>,
    tenant_id: TenantId,
    dataset_id: DatasetId,
) -> DatasetSemanticUnderstandingAccess {
    if !enabled {
        return DatasetSemanticUnderstandingAccess::FeatureDisabled;
    }
    if !uuid_csv_contains(tenant_allowlist, tenant_id.0) {
        return DatasetSemanticUnderstandingAccess::TenantNotAllowlisted;
    }
    if !uuid_csv_contains(dataset_allowlist, dataset_id.0) {
        return DatasetSemanticUnderstandingAccess::DatasetNotAllowlisted;
    }
    DatasetSemanticUnderstandingAccess::Allowed
}

pub fn dataset_cross_semantic_graph_worker_enabled(tenant_id: TenantId) -> bool {
    env_flag_value(DATASET_CROSS_SEMANTIC_GRAPH_ENABLED_ENV, false)
        && uuid_csv_contains(
            std::env::var(DATASET_CROSS_SEMANTIC_GRAPH_TENANT_ALLOWLIST_ENV)
                .ok()
                .as_deref(),
            tenant_id.0,
        )
}

pub fn dataset_cross_semantic_graph_allowed_dataset_ids() -> Vec<DatasetId> {
    dataset_cross_semantic_graph_allowed_dataset_ids_from_value(
        std::env::var(DATASET_CROSS_SEMANTIC_GRAPH_DATASET_ALLOWLIST_ENV)
            .ok()
            .as_deref(),
    )
}

pub fn dataset_cross_semantic_graph_allowed_dataset_ids_from_value(
    value: Option<&str>,
) -> Vec<DatasetId> {
    value
        .into_iter()
        .flat_map(|value| value.split(','))
        .filter_map(|value| value.trim().parse::<uuid::Uuid>().ok())
        .map(DatasetId)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn uuid_csv_contains(csv: Option<&str>, expected: uuid::Uuid) -> bool {
    csv.into_iter()
        .flat_map(|value| value.split(','))
        .filter_map(|value| value.trim().parse::<uuid::Uuid>().ok())
        .any(|value| value == expected)
}

fn env_flag_value(key: &str, default: bool) -> bool {
    std::env::var(key)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}

#[derive(Clone, Debug)]
pub struct DatasetSemanticLinkMatchInput {
    pub left: DatasetSemanticUnderstanding,
    pub right: DatasetSemanticUnderstanding,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DatasetSemanticLinkSafeSummary {
    pub pair_id: String,
    pub generation_version: String,
    pub node_count: usize,
    pub edge_count: usize,
    pub confirmed_edge_count: usize,
    pub observed_edge_count: usize,
    pub inferred_edge_count: usize,
    pub chinese_label_count: usize,
    pub generic_label_count: usize,
}

pub fn build_dataset_semantic_link_graph(
    input: &DatasetSemanticLinkMatchInput,
) -> Result<DatasetSemanticGraphV1> {
    let left_dataset_id = input.left.dataset.id;
    let right_dataset_id = input.right.dataset.id;
    if left_dataset_id == right_dataset_id {
        return Err(anyhow!(
            "semantic link snapshots must belong to distinct datasets"
        ));
    }
    if input.left.status != "ready" || input.right.status != "ready" {
        return Err(anyhow!(
            "semantic link matcher requires two ready snapshots"
        ));
    }

    let mut endpoints = Vec::new();
    append_snapshot_semantic_endpoints(&mut endpoints, &input.left);
    append_snapshot_semantic_endpoints(&mut endpoints, &input.right);
    for identity in shared_document_identities(&input.left, &input.right) {
        for dataset_id in [left_dataset_id, right_dataset_id] {
            endpoints.push(CrossDatasetSemanticEndpointInput {
                dataset_id,
                local_node_id: stable_semantic_id(
                    "document",
                    &[&dataset_id.to_string(), &identity],
                ),
                kind: DatasetSemanticGraphNodeKind::Document,
                display_label: "共享资料".to_string(),
                visible_provenance_count: 1,
                identities: vec![CrossDatasetCanonicalIdentity::SharedDocumentMembership {
                    shared_document_id: identity.clone(),
                }],
            });
        }
    }

    let graph = match_cross_dataset_semantic_graph(&CrossDatasetSemanticMatchInput {
        root_dataset_id: left_dataset_id.min(right_dataset_id),
        datasets: vec![
            DatasetSemanticGraphDataset {
                id: left_dataset_id,
                title: safe_semantic_link_label(&input.left.dataset.title, "数据集"),
                stale: input.left.stale,
            },
            DatasetSemanticGraphDataset {
                id: right_dataset_id,
                title: safe_semantic_link_label(&input.right.dataset.title, "数据集"),
                stale: input.right.stale,
            },
        ],
        endpoints,
        explicit_relations: Vec::new(),
        options: CrossDatasetSemanticMatcherOptions::default(),
        limits: CrossDatasetSemanticMatcherLimits::default(),
        stale: input.left.stale || input.right.stale,
    });
    let manifest = serde_json::to_value(&graph)?;
    if !dataset_semantic_link_manifest_is_safe(&manifest) {
        return Err(anyhow!("semantic link manifest failed safe-output audit"));
    }
    Ok(graph)
}

fn shared_document_identities(
    left: &DatasetSemanticUnderstanding,
    right: &DatasetSemanticUnderstanding,
) -> Vec<String> {
    let left = semantic_document_source_ids(left);
    let right = semantic_document_source_ids(right);
    left.intersection(&right).cloned().collect()
}

fn semantic_document_source_ids(snapshot: &DatasetSemanticUnderstanding) -> BTreeSet<String> {
    snapshot
        .objects
        .iter()
        .flat_map(|object| object.evidence_refs.iter())
        .chain(
            snapshot
                .fields
                .iter()
                .flat_map(|field| field.evidence_refs.iter()),
        )
        .chain(
            snapshot
                .relations
                .iter()
                .flat_map(|relation| relation.evidence_refs.iter()),
        )
        .filter(|evidence| !evidence.source_kind.eq_ignore_ascii_case("asset"))
        .filter_map(|evidence| {
            evidence
                .source_id
                .trim()
                .parse::<uuid::Uuid>()
                .ok()
                .map(|id| id.to_string())
        })
        .collect()
}

fn append_snapshot_semantic_endpoints(
    endpoints: &mut Vec<CrossDatasetSemanticEndpointInput>,
    snapshot: &DatasetSemanticUnderstanding,
) {
    for object in &snapshot.objects {
        endpoints.push(CrossDatasetSemanticEndpointInput {
            dataset_id: snapshot.dataset.id,
            local_node_id: object.id.clone(),
            kind: DatasetSemanticGraphNodeKind::Object,
            display_label: safe_semantic_link_label(&object.label, "业务对象"),
            visible_provenance_count: object.coverage_count,
            identities: Vec::new(),
        });
    }
    for field in &snapshot.fields {
        let identities = semantic_v3_opaque_field_identity(snapshot, &field.id)
            .into_iter()
            .collect();
        endpoints.push(CrossDatasetSemanticEndpointInput {
            dataset_id: snapshot.dataset.id,
            local_node_id: field.id.clone(),
            kind: DatasetSemanticGraphNodeKind::Field,
            display_label: safe_semantic_link_label(&field.label, "业务字段"),
            visible_provenance_count: field.non_empty_count.max(1),
            identities,
        });
    }
}

fn semantic_v3_opaque_field_identity(
    snapshot: &DatasetSemanticUnderstanding,
    field_id: &str,
) -> Option<CrossDatasetCanonicalIdentity> {
    let opaque = field_id.trim().strip_prefix("field:")?;
    if snapshot.generation_version
        != platform_api::semantic_understanding::DATASET_SEMANTIC_GENERATION_VERSION
        || opaque.len() != 32
        || !opaque
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return None;
    }
    Some(CrossDatasetCanonicalIdentity::ExactSourceField {
        source_system_key: "dataset-semantic-snapshot".to_string(),
        source_schema_key: "semantic-profile-v3".to_string(),
        source_object_key: "opaque-source-field".to_string(),
        source_field_key: field_id.trim().to_ascii_lowercase(),
    })
}

fn safe_semantic_link_label(value: &str, fallback: &str) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = compact.to_ascii_lowercase();
    let windows_path = lower.as_bytes().get(1) == Some(&b':')
        && matches!(lower.as_bytes().get(2), Some(b'\\') | Some(b'/'));
    let internal = windows_path
        || lower.starts_with("\\\\")
        || [
            "/home/",
            "/users/",
            "/root/",
            "/tmp/",
            "/var/",
            "file://",
            "postgres://",
            "mysql://",
            "jdbc:",
            "s3://",
            "oss://",
            "sha256",
            "hmac",
        ]
        .iter()
        .any(|marker| lower.contains(marker));
    if compact.is_empty()
        || compact.chars().count() > 48
        || compact.chars().any(char::is_control)
        || internal
        || looks_opaque_semantic_label(&compact)
    {
        fallback.to_string()
    } else {
        compact
    }
}

fn looks_opaque_semantic_label(value: &str) -> bool {
    if value.parse::<uuid::Uuid>().is_ok() {
        return true;
    }
    let compact = value
        .chars()
        .filter(|character| !matches!(character, '-' | '_' | ':' | ' '))
        .collect::<String>();
    compact.len() >= 16
        && compact
            .chars()
            .all(|character| character.is_ascii_hexdigit())
}

pub fn dataset_semantic_link_manifest_is_safe(manifest: &Value) -> bool {
    let serialized = match serde_json::to_string(manifest) {
        Ok(serialized) => serialized.to_ascii_lowercase(),
        Err(_) => return false,
    };
    ![
        "sha256",
        "hmac",
        "content_identity",
        "source_identity",
        "technical_name",
        "observed_values",
        "examples",
        "raw_value",
        "object_key",
        "file://",
        "postgres://",
        "mysql://",
        "jdbc:",
        "c:\\\\",
        "/home/",
        "/users/",
        "/root/",
    ]
    .iter()
    .any(|marker| serialized.contains(marker))
}

pub fn dataset_semantic_link_public_pair_id(
    left_dataset_id: DatasetId,
    right_dataset_id: DatasetId,
    left_snapshot_id: uuid::Uuid,
    right_snapshot_id: uuid::Uuid,
) -> String {
    let (left_dataset_id, right_dataset_id, left_snapshot_id, right_snapshot_id) =
        storage::canonical_dataset_semantic_link_pair(
            left_dataset_id,
            right_dataset_id,
            left_snapshot_id,
            right_snapshot_id,
        )
        .expect("semantic link public pair id requires distinct endpoints");
    stable_semantic_id(
        "pair",
        &[
            &left_dataset_id.to_string(),
            &right_dataset_id.to_string(),
            &left_snapshot_id.to_string(),
            &right_snapshot_id.to_string(),
            DATASET_SEMANTIC_GRAPH_GENERATION_VERSION,
        ],
    )
}

pub fn dataset_semantic_link_safe_summary(
    pair_id: &str,
    graph: &DatasetSemanticGraphV1,
) -> DatasetSemanticLinkSafeSummary {
    let mut confirmed_edge_count = 0;
    let mut observed_edge_count = 0;
    let mut inferred_edge_count = 0;
    for edge in &graph.edges {
        match edge.evidence_class {
            SemanticEvidenceClass::Confirmed => confirmed_edge_count += 1,
            SemanticEvidenceClass::Observed => observed_edge_count += 1,
            SemanticEvidenceClass::Inferred => inferred_edge_count += 1,
        }
    }
    let chinese_label_count = graph
        .nodes
        .iter()
        .filter(|node| node.display_label.chars().any(is_cjk_business_character))
        .count();
    let generic_label_count = graph
        .nodes
        .iter()
        .filter(|node| {
            matches!(
                node.display_label.as_str(),
                "数据集" | "资料" | "业务对象" | "业务字段" | "知识概念" | "数据结构"
            )
        })
        .count();
    DatasetSemanticLinkSafeSummary {
        pair_id: pair_id.to_string(),
        generation_version: DATASET_SEMANTIC_GRAPH_GENERATION_VERSION.to_string(),
        node_count: graph.nodes.len(),
        edge_count: graph.edges.len(),
        confirmed_edge_count,
        observed_edge_count,
        inferred_edge_count,
        chinese_label_count,
        generic_label_count,
    }
}

fn is_cjk_business_character(character: char) -> bool {
    matches!(
        character,
        '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{f900}'..='\u{faff}'
    )
}

pub fn build_dataset_semantic_link_graph_from_ready_snapshots(
    tenant_id: TenantId,
    left_snapshot: &DatasetSemanticSnapshot,
    right_snapshot: &DatasetSemanticSnapshot,
) -> Result<DatasetSemanticGraphV1> {
    if left_snapshot.tenant_id != tenant_id || right_snapshot.tenant_id != tenant_id {
        return Err(anyhow!("semantic link snapshot tenant mismatch"));
    }
    if left_snapshot.status != "ready" || right_snapshot.status != "ready" {
        return Err(anyhow!("semantic link inputs must both be ready"));
    }
    if left_snapshot.generation_version
        != platform_api::semantic_understanding::DATASET_SEMANTIC_GENERATION_VERSION
        || right_snapshot.generation_version
            != platform_api::semantic_understanding::DATASET_SEMANTIC_GENERATION_VERSION
    {
        return Err(anyhow!("semantic link input generation is unsupported"));
    }
    let left =
        serde_json::from_value::<DatasetSemanticUnderstanding>(left_snapshot.manifest.clone())?;
    let right =
        serde_json::from_value::<DatasetSemanticUnderstanding>(right_snapshot.manifest.clone())?;
    if left.dataset.id != left_snapshot.dataset_id || right.dataset.id != right_snapshot.dataset_id
    {
        return Err(anyhow!("semantic link manifest dataset identity mismatch"));
    }
    build_dataset_semantic_link_graph(&DatasetSemanticLinkMatchInput { left, right })
}

pub fn semantic_link_run_inputs_are_current(
    run: &DatasetSemanticLinkRun,
    left_latest: &DatasetSemanticSnapshot,
    right_latest: &DatasetSemanticSnapshot,
) -> bool {
    run.tenant_id == left_latest.tenant_id
        && run.tenant_id == right_latest.tenant_id
        && run.left_dataset_id == left_latest.dataset_id
        && run.right_dataset_id == right_latest.dataset_id
        && run.left_snapshot_id == left_latest.id
        && run.right_snapshot_id == right_latest.id
}

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
    use chrono::Utc;
    use platform_api::semantic_understanding::{
        DatasetSemanticIdentity, DatasetSemanticUnderstanding, SemanticCoverage,
        SemanticEvidenceRef, SemanticField, SemanticObject, SemanticStatus, SemanticSummary,
        SemanticTruncation, DATASET_SEMANTIC_GENERATION_VERSION, DATASET_SEMANTIC_SCHEMA_VERSION,
    };
    use serde_json::json;
    use uuid::Uuid;

    use super::*;

    fn semantic_snapshot(
        dataset_id: DatasetId,
        title: &str,
        object_id: &str,
        field_id: &str,
        field_label: &str,
        technical_name: &str,
    ) -> DatasetSemanticUnderstanding {
        DatasetSemanticUnderstanding {
            schema_version: DATASET_SEMANTIC_SCHEMA_VERSION.to_string(),
            generation_version: DATASET_SEMANTIC_GENERATION_VERSION.to_string(),
            status: "ready".to_string(),
            dataset: DatasetSemanticIdentity {
                id: dataset_id,
                title: title.to_string(),
            },
            coverage: SemanticCoverage::default(),
            summary: SemanticSummary::default(),
            objects: vec![SemanticObject {
                id: object_id.to_string(),
                kind: "database_table".to_string(),
                label: "经营对象".to_string(),
                technical_name: "C:\\private\\raw-table".to_string(),
                description: "不进入跨集快照".to_string(),
                label_source: "fixture".to_string(),
                confidence: 1.0,
                coverage_count: 1,
                status: SemanticStatus::Observed,
                evidence_refs: Vec::new(),
            }],
            fields: vec![SemanticField {
                id: field_id.to_string(),
                object_id: object_id.to_string(),
                label: field_label.to_string(),
                technical_name: technical_name.to_string(),
                semantic_role: "identifier".to_string(),
                value_type: "text".to_string(),
                non_empty_count: 1,
                distinct_count: 1,
                examples: vec!["客户原始值-不可泄漏".to_string()],
                status: SemanticStatus::Observed,
                label_source: "fixture".to_string(),
                confidence: 1.0,
                evidence_refs: Vec::new(),
            }],
            relations: Vec::new(),
            source_groups: Vec::new(),
            pipeline: Vec::new(),
            generated_at: Utc::now(),
            stale: false,
            truncated: SemanticTruncation::default(),
        }
    }

    fn with_document_evidence(
        mut snapshot: DatasetSemanticUnderstanding,
        document_id: Uuid,
    ) -> DatasetSemanticUnderstanding {
        snapshot.objects[0].evidence_refs.push(SemanticEvidenceRef {
            source_kind: "database".to_string(),
            source_id: document_id.to_string(),
            label: "来源结构".to_string(),
        });
        snapshot
    }

    #[test]
    fn semantic_link_rollout_is_fail_closed_and_wildcard_is_not_special() {
        let tenant_id = TenantId::new();
        let left_dataset_id = DatasetId::new();
        let right_dataset_id = DatasetId::new();
        let tenant_allowlist = tenant_id.to_string();
        let dataset_allowlist = format!("{left_dataset_id},{right_dataset_id}");

        assert_eq!(
            dataset_cross_semantic_graph_access_from_values(
                false,
                Some(&tenant_allowlist),
                Some(&dataset_allowlist),
                tenant_id,
                left_dataset_id,
                right_dataset_id,
            ),
            DatasetCrossSemanticGraphAccess::FeatureDisabled
        );
        assert_eq!(
            dataset_cross_semantic_graph_access_from_values(
                true,
                Some("*"),
                Some("*"),
                tenant_id,
                left_dataset_id,
                right_dataset_id,
            ),
            DatasetCrossSemanticGraphAccess::TenantNotAllowlisted
        );
        assert_eq!(
            dataset_cross_semantic_graph_access_from_values(
                true,
                Some(&tenant_allowlist),
                Some(&left_dataset_id.to_string()),
                tenant_id,
                left_dataset_id,
                right_dataset_id,
            ),
            DatasetCrossSemanticGraphAccess::RightDatasetNotAllowlisted
        );
        assert_eq!(
            dataset_cross_semantic_graph_access_from_values(
                true,
                Some(&tenant_allowlist),
                Some(&dataset_allowlist),
                tenant_id,
                left_dataset_id,
                right_dataset_id,
            ),
            DatasetCrossSemanticGraphAccess::Allowed
        );
        assert!(dataset_cross_semantic_graph_allowed_dataset_ids_from_value(Some("*")).is_empty());
        let mut expected_dataset_ids = vec![left_dataset_id, right_dataset_id];
        expected_dataset_ids.sort();
        assert_eq!(
            dataset_cross_semantic_graph_allowed_dataset_ids_from_value(Some(&dataset_allowlist)),
            expected_dataset_ids
        );
    }

    #[test]
    fn semantic_link_matcher_shares_membership_documents_and_exact_opaque_fields_without_leaks() {
        let left_dataset_id = DatasetId(Uuid::from_u128(10));
        let right_dataset_id = DatasetId(Uuid::from_u128(20));
        let shared_field_id = "field:0123456789abcdef0123456789abcdef";
        let shared_document_id = Uuid::from_u128(99);
        let graph = build_dataset_semantic_link_graph(&DatasetSemanticLinkMatchInput {
            left: with_document_evidence(
                semantic_snapshot(
                    left_dataset_id,
                    "项目资料",
                    "object:left",
                    shared_field_id,
                    "合同编号",
                    "raw_hmac_private_field",
                ),
                shared_document_id,
            ),
            right: with_document_evidence(
                semantic_snapshot(
                    right_dataset_id,
                    "经营分析",
                    "object:right",
                    shared_field_id,
                    "合同编号",
                    "postgres://private-host/raw",
                ),
                shared_document_id,
            ),
        })
        .expect("safe cross-dataset graph");
        let serialized = serde_json::to_string(&graph).expect("serializable graph");

        assert_eq!(
            graph
                .nodes
                .iter()
                .filter(|node| node.id.starts_with("shared:document:"))
                .count(),
            1
        );
        assert_eq!(
            graph
                .nodes
                .iter()
                .filter(|node| node.id.starts_with("shared:field:"))
                .count(),
            1
        );
        for forbidden in [
            &shared_document_id.to_string(),
            shared_field_id,
            "raw_hmac_private_field",
            "postgres://private-host/raw",
            "客户原始值-不可泄漏",
            "C:\\private\\raw-table",
        ] {
            assert!(!serialized.contains(forbidden), "leaked {forbidden}");
        }
        assert!(dataset_semantic_link_manifest_is_safe(
            &serde_json::to_value(&graph).unwrap()
        ));
    }

    #[test]
    fn semantic_link_safe_summary_has_only_approved_receipt_fields() {
        let left_dataset_id = DatasetId(Uuid::from_u128(10));
        let right_dataset_id = DatasetId(Uuid::from_u128(20));
        let graph = build_dataset_semantic_link_graph(&DatasetSemanticLinkMatchInput {
            left: semantic_snapshot(
                left_dataset_id,
                "项目资料",
                "object:left",
                "field:left",
                "合同金额",
                "amount",
            ),
            right: semantic_snapshot(
                right_dataset_id,
                "经营分析",
                "object:right",
                "field:right",
                "租赁面积",
                "area",
            ),
        })
        .unwrap();
        let summary = dataset_semantic_link_safe_summary("pair:opaque", &graph);
        let value = serde_json::to_value(summary).unwrap();
        let keys = value
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();

        assert_eq!(
            keys,
            BTreeSet::from([
                "chinese_label_count".to_string(),
                "confirmed_edge_count".to_string(),
                "edge_count".to_string(),
                "generation_version".to_string(),
                "generic_label_count".to_string(),
                "inferred_edge_count".to_string(),
                "node_count".to_string(),
                "observed_edge_count".to_string(),
                "pair_id".to_string(),
            ])
        );
        assert_eq!(value["pair_id"], json!("pair:opaque"));
    }

    #[test]
    fn semantic_link_labels_drop_opaque_ids_and_internal_paths() {
        assert_eq!(
            safe_semantic_link_label("0123456789abcdef0123456789abcdef", "资料"),
            "资料"
        );
        assert_eq!(
            safe_semantic_link_label("\\\\internal-server\\share\\report.xlsx", "资料"),
            "资料"
        );
        assert_eq!(safe_semantic_link_label("合同金额", "业务字段"), "合同金额");
    }

    #[test]
    fn semantic_understanding_access_is_fail_closed_and_requires_both_allowlists() {
        let tenant_id = TenantId::new();
        let dataset_id = DatasetId::new();

        assert_eq!(
            dataset_semantic_understanding_access_from_values(
                false,
                Some(&tenant_id.to_string()),
                Some(&dataset_id.to_string()),
                tenant_id,
                dataset_id,
            ),
            DatasetSemanticUnderstandingAccess::FeatureDisabled
        );
        assert_eq!(
            dataset_semantic_understanding_access_from_values(
                true,
                None,
                Some(&dataset_id.to_string()),
                tenant_id,
                dataset_id,
            ),
            DatasetSemanticUnderstandingAccess::TenantNotAllowlisted
        );
        assert_eq!(
            dataset_semantic_understanding_access_from_values(
                true,
                Some(&tenant_id.to_string()),
                None,
                tenant_id,
                dataset_id,
            ),
            DatasetSemanticUnderstandingAccess::DatasetNotAllowlisted
        );
    }

    #[test]
    fn semantic_understanding_access_accepts_exact_uuid_csv_entries_only() {
        let tenant_id = TenantId::new();
        let dataset_id = DatasetId::new();
        let other_tenant_id = TenantId::new();
        let other_dataset_id = DatasetId::new();
        let tenant_allowlist = format!("invalid, {}, {}", other_tenant_id, tenant_id);
        let dataset_allowlist = format!("{},{}", other_dataset_id, dataset_id);

        assert_eq!(
            dataset_semantic_understanding_access_from_values(
                true,
                Some(&tenant_allowlist),
                Some(&dataset_allowlist),
                tenant_id,
                dataset_id,
            ),
            DatasetSemanticUnderstandingAccess::Allowed
        );
        assert_eq!(
            dataset_semantic_understanding_access_from_values(
                true,
                Some("*"),
                Some("*"),
                tenant_id,
                dataset_id,
            ),
            DatasetSemanticUnderstandingAccess::TenantNotAllowlisted
        );
    }

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
