use std::collections::{BTreeMap, BTreeSet};

use domain_model::DatasetId;
use serde::{Deserialize, Serialize};

use crate::semantic_relation_builder::cap_semantic_evidence_class;
use crate::semantic_understanding::{
    stable_semantic_id, SemanticEvidenceClass, SemanticRelationSemantics,
};

pub const DATASET_SEMANTIC_GRAPH_SCHEMA_VERSION: &str = "1.0.0";
pub const DATASET_SEMANTIC_GRAPH_GENERATION_VERSION: &str = "dataset_semantic_graph_v1";

const ABSOLUTE_MAX_DATASETS: usize = 8;
const ABSOLUTE_MAX_ENDPOINTS: usize = 360;
const ABSOLUTE_MAX_EDGES: usize = 600;
const MAX_INTERNAL_IDENTITY_PART_LENGTH: usize = 512;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DatasetSemanticGraphNodeKind {
    Dataset,
    Document,
    Object,
    Field,
    Concept,
    Structure,
}

impl DatasetSemanticGraphNodeKind {
    fn shared_id_kind(self) -> &'static str {
        match self {
            Self::Dataset => "dataset",
            Self::Document => "document",
            Self::Object => "object",
            Self::Field => "field",
            Self::Concept => "concept",
            Self::Structure => "structure",
        }
    }

    fn fallback_label(self) -> &'static str {
        match self {
            Self::Dataset => "数据集",
            Self::Document => "资料",
            Self::Object => "业务对象",
            Self::Field => "业务字段",
            Self::Concept => "知识概念",
            Self::Structure => "数据结构",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DatasetSemanticGraphDataset {
    pub id: DatasetId,
    pub title: String,
    pub stale: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DatasetSemanticGraphNode {
    pub id: String,
    pub kind: DatasetSemanticGraphNodeKind,
    pub display_label: String,
    pub dataset_refs: Vec<DatasetId>,
    pub visible_provenance_count: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DatasetSemanticGraphEdge {
    pub id: String,
    pub source_id: String,
    pub target_id: String,
    pub relation_type: String,
    pub label: String,
    pub relation_semantics: SemanticRelationSemantics,
    pub evidence_class: SemanticEvidenceClass,
    pub confidence: f64,
    pub reason: String,
    pub cross_dataset: bool,
    pub supporting_dataset_ids: Vec<DatasetId>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct DatasetSemanticGraphTruncation {
    pub datasets: usize,
    pub nodes: usize,
    pub edges: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DatasetSemanticGraphV1 {
    pub schema_version: String,
    pub root_dataset_id: DatasetId,
    pub datasets: Vec<DatasetSemanticGraphDataset>,
    pub nodes: Vec<DatasetSemanticGraphNode>,
    pub edges: Vec<DatasetSemanticGraphEdge>,
    pub truncated: DatasetSemanticGraphTruncation,
    pub stale: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CrossDatasetCanonicalIdentity {
    SharedDocumentMembership {
        shared_document_id: String,
    },
    CanonicalDocument {
        canonical_document_id: String,
    },
    ExactContent {
        content_identity: String,
    },
    ExactSourceField {
        source_system_key: String,
        source_schema_key: String,
        source_object_key: String,
        source_field_key: String,
    },
    ConfirmedConcept {
        concept_id: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrossDatasetSemanticEndpointInput {
    pub dataset_id: DatasetId,
    pub local_node_id: String,
    pub kind: DatasetSemanticGraphNodeKind,
    pub display_label: String,
    pub visible_provenance_count: u64,
    pub identities: Vec<CrossDatasetCanonicalIdentity>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CrossDatasetExplicitRelationKind {
    ForeignKey,
    ExplicitReference,
}

impl CrossDatasetExplicitRelationKind {
    fn relation_type(self) -> &'static str {
        match self {
            Self::ForeignKey => "foreign_key",
            Self::ExplicitReference => "explicit_reference",
        }
    }

    fn label(self, evidence_class: SemanticEvidenceClass) -> &'static str {
        match (self, evidence_class) {
            (Self::ForeignKey, SemanticEvidenceClass::Confirmed) => "已确认外键",
            (Self::ForeignKey, SemanticEvidenceClass::Observed) => "外键关系线索",
            (Self::ForeignKey, SemanticEvidenceClass::Inferred) => "可能外键关联",
            (Self::ExplicitReference, SemanticEvidenceClass::Confirmed) => "明确引用",
            (Self::ExplicitReference, SemanticEvidenceClass::Observed) => "引用关系线索",
            (Self::ExplicitReference, SemanticEvidenceClass::Inferred) => "可能引用",
        }
    }

    fn reason(self, evidence_class: SemanticEvidenceClass) -> &'static str {
        match (self, evidence_class) {
            (Self::ForeignKey, SemanticEvidenceClass::Confirmed) => {
                "两个节点存在已登记的外键关系。"
            }
            (Self::ForeignKey, SemanticEvidenceClass::Observed) => {
                "现有证据观察到外键关系线索，尚未确认。"
            }
            (Self::ForeignKey, SemanticEvidenceClass::Inferred) => {
                "现有证据提示可能存在外键关系，尚未确认。"
            }
            (Self::ExplicitReference, SemanticEvidenceClass::Confirmed) => {
                "两个节点存在已登记的明确引用关系。"
            }
            (Self::ExplicitReference, SemanticEvidenceClass::Observed) => {
                "现有证据观察到引用关系线索，尚未确认。"
            }
            (Self::ExplicitReference, SemanticEvidenceClass::Inferred) => {
                "现有证据提示可能存在引用关系，尚未确认。"
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrossDatasetExplicitRelationInput {
    pub source_dataset_id: DatasetId,
    pub source_local_node_id: String,
    pub target_dataset_id: DatasetId,
    pub target_local_node_id: String,
    pub kind: CrossDatasetExplicitRelationKind,
    pub evidence_class: SemanticEvidenceClass,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CrossDatasetSemanticMatcherOptions {
    pub enable_label_similarity: bool,
    pub enable_structure_similarity: bool,
    pub enable_jaccard_similarity: bool,
    pub enable_sample_overlap: bool,
}

impl Default for CrossDatasetSemanticMatcherOptions {
    fn default() -> Self {
        Self {
            enable_label_similarity: false,
            enable_structure_similarity: false,
            enable_jaccard_similarity: false,
            enable_sample_overlap: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CrossDatasetSemanticMatcherLimits {
    pub max_datasets: usize,
    pub max_nodes: usize,
    pub max_edges: usize,
}

impl Default for CrossDatasetSemanticMatcherLimits {
    fn default() -> Self {
        Self {
            max_datasets: ABSOLUTE_MAX_DATASETS,
            max_nodes: 160,
            max_edges: 240,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CrossDatasetSemanticMatchInput {
    pub root_dataset_id: DatasetId,
    pub datasets: Vec<DatasetSemanticGraphDataset>,
    pub endpoints: Vec<CrossDatasetSemanticEndpointInput>,
    pub explicit_relations: Vec<CrossDatasetExplicitRelationInput>,
    pub options: CrossDatasetSemanticMatcherOptions,
    pub limits: CrossDatasetSemanticMatcherLimits,
    pub stale: bool,
}

pub fn scoped_semantic_endpoint_id(dataset_id: DatasetId, local_node_id: &str) -> String {
    let local_node_id = safe_local_node_id(dataset_id, local_node_id);
    format!("d:{dataset_id}:{local_node_id}")
}

pub fn match_cross_dataset_semantic_graph(
    input: &CrossDatasetSemanticMatchInput,
) -> DatasetSemanticGraphV1 {
    let (datasets, datasets_truncated) = bounded_datasets(input);
    let included_dataset_ids = datasets
        .iter()
        .map(|dataset| dataset.id)
        .collect::<BTreeSet<_>>();

    let mut endpoints = input
        .endpoints
        .iter()
        .filter(|endpoint| {
            included_dataset_ids.contains(&endpoint.dataset_id)
                && !endpoint.local_node_id.trim().is_empty()
        })
        .cloned()
        .collect::<Vec<_>>();
    endpoints.sort_by(|left, right| {
        left.dataset_id
            .cmp(&right.dataset_id)
            .then_with(|| left.local_node_id.trim().cmp(right.local_node_id.trim()))
            .then_with(|| left.kind.cmp(&right.kind))
    });
    endpoints.dedup_by(|left, right| {
        left.dataset_id == right.dataset_id
            && left.local_node_id.trim() == right.local_node_id.trim()
    });
    let endpoints_truncated = endpoints.len().saturating_sub(ABSOLUTE_MAX_ENDPOINTS);
    endpoints.truncate(ABSOLUTE_MAX_ENDPOINTS);

    let mut identities = BTreeMap::<String, Vec<usize>>::new();
    let mut endpoint_identity_keys = vec![Vec::<String>::new(); endpoints.len()];
    for (index, endpoint) in endpoints.iter().enumerate() {
        for identity in &endpoint.identities {
            if let Some(key) = exact_identity_key(endpoint.kind, identity) {
                identities.entry(key.clone()).or_default().push(index);
                endpoint_identity_keys[index].push(key);
            }
        }
        endpoint_identity_keys[index].sort();
        endpoint_identity_keys[index].dedup();
    }

    let mut union_find = UnionFind::new(endpoints.len());
    for indices in identities.values() {
        let dataset_ids = indices
            .iter()
            .map(|index| endpoints[*index].dataset_id)
            .collect::<BTreeSet<_>>();
        if dataset_ids.len() < 2 {
            continue;
        }
        if let Some(first) = indices.first() {
            for index in &indices[1..] {
                union_find.union(*first, *index);
            }
        }
    }

    let mut components = BTreeMap::<usize, Vec<usize>>::new();
    for index in 0..endpoints.len() {
        components
            .entry(union_find.find(index))
            .or_default()
            .push(index);
    }

    let mut endpoint_node_ids = vec![String::new(); endpoints.len()];
    let mut nodes_by_id = BTreeMap::<String, DatasetSemanticGraphNode>::new();
    for indices in components.values() {
        let dataset_refs = indices
            .iter()
            .map(|index| endpoints[*index].dataset_id)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        if dataset_refs.len() >= 2 {
            let kind = endpoints[indices[0]].kind;
            let shared_id = shared_node_id(kind, indices, &endpoint_identity_keys, &endpoints);
            for index in indices {
                endpoint_node_ids[*index] = shared_id.clone();
            }
            nodes_by_id.insert(
                shared_id.clone(),
                DatasetSemanticGraphNode {
                    id: shared_id,
                    kind,
                    display_label: preferred_display_label(indices, &endpoints, kind),
                    dataset_refs,
                    visible_provenance_count: indices.iter().fold(0_u64, |count, index| {
                        count.saturating_add(endpoints[*index].visible_provenance_count)
                    }),
                },
            );
        } else {
            for index in indices {
                let endpoint = &endpoints[*index];
                let node_id =
                    scoped_semantic_endpoint_id(endpoint.dataset_id, endpoint.local_node_id.trim());
                endpoint_node_ids[*index] = node_id.clone();
                nodes_by_id.insert(
                    node_id.clone(),
                    DatasetSemanticGraphNode {
                        id: node_id,
                        kind: endpoint.kind,
                        display_label: safe_display_label(&endpoint.display_label, endpoint.kind),
                        dataset_refs: vec![endpoint.dataset_id],
                        visible_provenance_count: endpoint.visible_provenance_count,
                    },
                );
            }
        }
    }

    let all_node_dataset_refs = nodes_by_id
        .iter()
        .map(|(id, node)| (id.clone(), node.dataset_refs.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut all_nodes = nodes_by_id.into_values().collect::<Vec<_>>();
    all_nodes.sort_by(|left, right| {
        node_sort_rank(&left.id)
            .cmp(&node_sort_rank(&right.id))
            .then_with(|| left.id.cmp(&right.id))
    });
    let total_node_count = all_nodes.len().saturating_add(endpoints_truncated);
    let max_nodes = input.limits.max_nodes.min(ABSOLUTE_MAX_ENDPOINTS);
    all_nodes.truncate(max_nodes);
    let retained_node_ids = all_nodes
        .iter()
        .map(|node| node.id.clone())
        .collect::<BTreeSet<_>>();

    let endpoint_lookup = endpoints
        .iter()
        .enumerate()
        .map(|(index, endpoint)| {
            (
                (
                    endpoint.dataset_id,
                    endpoint.local_node_id.trim().to_string(),
                ),
                index,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut edge_candidates =
        BTreeMap::<(String, String, String, u8), DatasetSemanticGraphEdge>::new();

    for relation in &input.explicit_relations {
        let source_key = (
            relation.source_dataset_id,
            relation.source_local_node_id.trim().to_string(),
        );
        let target_key = (
            relation.target_dataset_id,
            relation.target_local_node_id.trim().to_string(),
        );
        let (Some(source_index), Some(target_index)) = (
            endpoint_lookup.get(&source_key),
            endpoint_lookup.get(&target_key),
        ) else {
            continue;
        };
        let source_id = endpoint_node_ids[*source_index].clone();
        let target_id = endpoint_node_ids[*target_index].clone();
        if source_id == target_id {
            continue;
        }
        let evidence_class =
            cap_semantic_evidence_class(relation.evidence_class, SemanticEvidenceClass::Confirmed);
        let edge = build_edge(
            source_id,
            target_id,
            relation.kind.relation_type(),
            relation.kind.label(evidence_class),
            SemanticRelationSemantics::Reference,
            evidence_class,
            evidence_confidence(evidence_class),
            relation.kind.reason(evidence_class),
            &all_node_dataset_refs,
        );
        insert_edge_candidate(&mut edge_candidates, edge);
    }

    if input.options.enable_label_similarity {
        for left_index in 0..endpoints.len() {
            for right_index in (left_index + 1)..endpoints.len() {
                let left = &endpoints[left_index];
                let right = &endpoints[right_index];
                if left.dataset_id == right.dataset_id || left.kind != right.kind {
                    continue;
                }
                let (Some(left_label), Some(right_label)) = (
                    comparable_chinese_label(&left.display_label),
                    comparable_chinese_label(&right.display_label),
                ) else {
                    continue;
                };
                if left_label != right_label {
                    continue;
                }
                let (source_id, target_id) = canonical_node_pair(
                    &endpoint_node_ids[left_index],
                    &endpoint_node_ids[right_index],
                );
                if source_id == target_id {
                    continue;
                }
                let evidence_class = cap_semantic_evidence_class(
                    SemanticEvidenceClass::Inferred,
                    SemanticEvidenceClass::Inferred,
                );
                let edge = build_edge(
                    source_id,
                    target_id,
                    "label_similarity",
                    "名称相近",
                    SemanticRelationSemantics::Similarity,
                    evidence_class,
                    0.45,
                    "两个节点使用相同中文名称，仅表示可能相关，不代表同一实体。",
                    &all_node_dataset_refs,
                );
                insert_edge_candidate(&mut edge_candidates, edge);
            }
        }
    }

    let total_edge_count = edge_candidates.len();
    let mut edges = edge_candidates
        .into_values()
        .filter(|edge| {
            retained_node_ids.contains(&edge.source_id)
                && retained_node_ids.contains(&edge.target_id)
        })
        .collect::<Vec<_>>();
    edges.sort_by(|left, right| {
        evidence_rank(left.evidence_class)
            .cmp(&evidence_rank(right.evidence_class))
            .then_with(|| left.source_id.cmp(&right.source_id))
            .then_with(|| left.target_id.cmp(&right.target_id))
            .then_with(|| left.relation_type.cmp(&right.relation_type))
    });
    let max_edges = input.limits.max_edges.min(ABSOLUTE_MAX_EDGES);
    edges.truncate(max_edges);

    DatasetSemanticGraphV1 {
        schema_version: DATASET_SEMANTIC_GRAPH_SCHEMA_VERSION.to_string(),
        root_dataset_id: input.root_dataset_id,
        stale: input.stale || datasets.iter().any(|dataset| dataset.stale),
        datasets,
        truncated: DatasetSemanticGraphTruncation {
            datasets: datasets_truncated,
            nodes: total_node_count.saturating_sub(all_nodes.len()),
            edges: total_edge_count.saturating_sub(edges.len()),
        },
        nodes: all_nodes,
        edges,
    }
}

fn bounded_datasets(
    input: &CrossDatasetSemanticMatchInput,
) -> (Vec<DatasetSemanticGraphDataset>, usize) {
    let mut unique = input
        .datasets
        .iter()
        .cloned()
        .map(|dataset| (dataset.id, dataset))
        .collect::<BTreeMap<_, _>>();
    let root =
        unique
            .remove(&input.root_dataset_id)
            .unwrap_or_else(|| DatasetSemanticGraphDataset {
                id: input.root_dataset_id,
                title: "当前数据集".to_string(),
                stale: input.stale,
            });
    let total = unique.len() + 1;
    let limit = input.limits.max_datasets.clamp(1, ABSOLUTE_MAX_DATASETS);
    let mut datasets = vec![root];
    datasets.extend(unique.into_values().take(limit.saturating_sub(1)));
    let truncated = total.saturating_sub(datasets.len());
    (datasets, truncated)
}

fn exact_identity_key(
    kind: DatasetSemanticGraphNodeKind,
    identity: &CrossDatasetCanonicalIdentity,
) -> Option<String> {
    match (kind, identity) {
        (
            DatasetSemanticGraphNodeKind::Document,
            CrossDatasetCanonicalIdentity::SharedDocumentMembership { shared_document_id },
        ) => bounded_identity_parts("shared_document", &[shared_document_id]),
        (
            DatasetSemanticGraphNodeKind::Document,
            CrossDatasetCanonicalIdentity::CanonicalDocument {
                canonical_document_id,
            },
        ) => bounded_identity_parts("canonical_document", &[canonical_document_id]),
        (
            DatasetSemanticGraphNodeKind::Document,
            CrossDatasetCanonicalIdentity::ExactContent { content_identity },
        ) => bounded_identity_parts("exact_content", &[content_identity]),
        (
            DatasetSemanticGraphNodeKind::Field,
            CrossDatasetCanonicalIdentity::ExactSourceField {
                source_system_key,
                source_schema_key,
                source_object_key,
                source_field_key,
            },
        ) => bounded_identity_parts(
            "exact_source_field",
            &[
                source_system_key,
                source_schema_key,
                source_object_key,
                source_field_key,
            ],
        ),
        (
            DatasetSemanticGraphNodeKind::Concept,
            CrossDatasetCanonicalIdentity::ConfirmedConcept { concept_id },
        ) => bounded_identity_parts("confirmed_concept", &[concept_id]),
        _ => None,
    }
}

fn bounded_identity_parts(prefix: &str, parts: &[&String]) -> Option<String> {
    if parts.iter().any(|part| {
        let value = part.trim();
        value.is_empty() || value.len() > MAX_INTERNAL_IDENTITY_PART_LENGTH
    }) {
        return None;
    }
    let mut key = prefix.to_string();
    for part in parts {
        key.push('\0');
        key.push_str(part.trim());
    }
    Some(key)
}

fn shared_node_id(
    kind: DatasetSemanticGraphNodeKind,
    indices: &[usize],
    endpoint_identity_keys: &[Vec<String>],
    endpoints: &[CrossDatasetSemanticEndpointInput],
) -> String {
    let mut identity_dataset_refs = BTreeMap::<String, BTreeSet<DatasetId>>::new();
    for index in indices {
        for key in &endpoint_identity_keys[*index] {
            identity_dataset_refs
                .entry(key.clone())
                .or_default()
                .insert(endpoints[*index].dataset_id);
        }
    }
    let identity_digests = identity_dataset_refs
        .into_iter()
        .filter(|(_, dataset_refs)| dataset_refs.len() >= 2)
        .map(|(key, _)| stable_semantic_id("identity", &[key.as_str()]))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let digest_refs = identity_digests
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let digest = stable_semantic_id("shared", &digest_refs);
    let opaque = digest
        .split_once(':')
        .map(|(_, value)| value)
        .unwrap_or(&digest);
    format!("shared:{}:{opaque}", kind.shared_id_kind())
}

fn safe_local_node_id(dataset_id: DatasetId, local_node_id: &str) -> String {
    let value = local_node_id.trim();
    let lower = value.to_ascii_lowercase();
    let semantic_namespace = [
        "dataset:",
        "document:",
        "doc:",
        "object:",
        "field:",
        "concept:",
        "structure:",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix));
    let hash_like =
        value.len() >= 24 && value.chars().all(|character| character.is_ascii_hexdigit());
    let named_secret = ["sha256", "hmac", "content_identity", "source_system"]
        .iter()
        .any(|marker| lower.contains(marker));
    let safe = !value.is_empty()
        && value.len() <= 128
        && semantic_namespace
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "_:-".contains(character))
        && !hash_like
        && !named_secret;
    if safe {
        return value.to_string();
    }
    let digest = stable_semantic_id("endpoint", &[&dataset_id.to_string(), value]);
    let opaque = digest
        .split_once(':')
        .map(|(_, value)| value)
        .unwrap_or(&digest);
    format!("opaque:{opaque}")
}

fn safe_display_label(value: &str, kind: DatasetSemanticGraphNodeKind) -> String {
    comparable_chinese_label(value).unwrap_or_else(|| kind.fallback_label().to_string())
}

fn comparable_chinese_label(value: &str) -> Option<String> {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let cjk_count = compact
        .chars()
        .filter(|character| is_cjk(*character))
        .count();
    if compact.is_empty()
        || cjk_count < 2
        || compact
            .chars()
            .any(|character| character.is_ascii_alphabetic())
    {
        return None;
    }
    Some(compact.chars().take(24).collect())
}

fn is_cjk(character: char) -> bool {
    matches!(
        character,
        '\u{3400}'..='\u{4dbf}'
            | '\u{4e00}'..='\u{9fff}'
            | '\u{f900}'..='\u{faff}'
            | '\u{20000}'..='\u{2fa1f}'
    )
}

fn preferred_display_label(
    indices: &[usize],
    endpoints: &[CrossDatasetSemanticEndpointInput],
    kind: DatasetSemanticGraphNodeKind,
) -> String {
    let mut counts = BTreeMap::<String, usize>::new();
    for index in indices {
        let label = safe_display_label(&endpoints[*index].display_label, kind);
        *counts.entry(label).or_default() += 1;
    }
    counts
        .into_iter()
        .min_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)))
        .map(|(label, _)| label)
        .unwrap_or_else(|| kind.fallback_label().to_string())
}

fn canonical_node_pair(left: &str, right: &str) -> (String, String) {
    if left <= right {
        (left.to_string(), right.to_string())
    } else {
        (right.to_string(), left.to_string())
    }
}

#[allow(clippy::too_many_arguments)]
fn build_edge(
    source_id: String,
    target_id: String,
    relation_type: &str,
    label: &str,
    relation_semantics: SemanticRelationSemantics,
    evidence_class: SemanticEvidenceClass,
    confidence: f64,
    reason: &str,
    node_dataset_refs: &BTreeMap<String, Vec<DatasetId>>,
) -> DatasetSemanticGraphEdge {
    let supporting_dataset_ids = node_dataset_refs
        .get(&source_id)
        .into_iter()
        .chain(node_dataset_refs.get(&target_id))
        .flat_map(|dataset_ids| dataset_ids.iter().copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let evidence_name = match evidence_class {
        SemanticEvidenceClass::Confirmed => "confirmed",
        SemanticEvidenceClass::Observed => "observed",
        SemanticEvidenceClass::Inferred => "inferred",
    };
    DatasetSemanticGraphEdge {
        id: stable_semantic_id(
            "cross_relation",
            &[&source_id, &target_id, relation_type, evidence_name],
        ),
        source_id,
        target_id,
        relation_type: relation_type.to_string(),
        label: label.to_string(),
        relation_semantics,
        evidence_class,
        confidence,
        reason: reason.to_string(),
        cross_dataset: supporting_dataset_ids.len() >= 2,
        supporting_dataset_ids,
    }
}

fn insert_edge_candidate(
    candidates: &mut BTreeMap<(String, String, String, u8), DatasetSemanticGraphEdge>,
    edge: DatasetSemanticGraphEdge,
) {
    let key = (
        edge.source_id.clone(),
        edge.target_id.clone(),
        edge.relation_type.clone(),
        evidence_rank(edge.evidence_class),
    );
    candidates.entry(key).or_insert(edge);
}

fn evidence_confidence(class: SemanticEvidenceClass) -> f64 {
    match class {
        SemanticEvidenceClass::Confirmed => 1.0,
        SemanticEvidenceClass::Observed => 0.72,
        SemanticEvidenceClass::Inferred => 0.45,
    }
}

fn evidence_rank(class: SemanticEvidenceClass) -> u8 {
    match class {
        SemanticEvidenceClass::Confirmed => 0,
        SemanticEvidenceClass::Observed => 1,
        SemanticEvidenceClass::Inferred => 2,
    }
}

fn node_sort_rank(id: &str) -> u8 {
    if id.starts_with("shared:") {
        0
    } else {
        1
    }
}

#[derive(Debug)]
struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<u8>,
}

impl UnionFind {
    fn new(length: usize) -> Self {
        Self {
            parent: (0..length).collect(),
            rank: vec![0; length],
        }
    }

    fn find(&mut self, index: usize) -> usize {
        if self.parent[index] != index {
            self.parent[index] = self.find(self.parent[index]);
        }
        self.parent[index]
    }

    fn union(&mut self, left: usize, right: usize) {
        let left_root = self.find(left);
        let right_root = self.find(right);
        if left_root == right_root {
            return;
        }
        match self.rank[left_root].cmp(&self.rank[right_root]) {
            std::cmp::Ordering::Less => self.parent[left_root] = right_root,
            std::cmp::Ordering::Greater => self.parent[right_root] = left_root,
            std::cmp::Ordering::Equal => {
                self.parent[right_root] = left_root;
                self.rank[left_root] = self.rank[left_root].saturating_add(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use domain_model::DatasetId;
    use uuid::Uuid;

    use super::*;
    use crate::semantic_understanding::{SemanticEvidenceClass, SemanticRelationSemantics};

    fn dataset(value: u128, title: &str) -> DatasetSemanticGraphDataset {
        DatasetSemanticGraphDataset {
            id: DatasetId(Uuid::from_u128(value)),
            title: title.to_string(),
            stale: false,
        }
    }

    fn endpoint(
        dataset_id: DatasetId,
        local_node_id: &str,
        kind: DatasetSemanticGraphNodeKind,
        display_label: &str,
        identities: Vec<CrossDatasetCanonicalIdentity>,
    ) -> CrossDatasetSemanticEndpointInput {
        CrossDatasetSemanticEndpointInput {
            dataset_id,
            local_node_id: local_node_id.to_string(),
            kind,
            display_label: display_label.to_string(),
            visible_provenance_count: 1,
            identities,
        }
    }

    fn input(
        datasets: Vec<DatasetSemanticGraphDataset>,
        endpoints: Vec<CrossDatasetSemanticEndpointInput>,
    ) -> CrossDatasetSemanticMatchInput {
        CrossDatasetSemanticMatchInput {
            root_dataset_id: datasets[0].id,
            datasets,
            endpoints,
            explicit_relations: Vec::new(),
            options: CrossDatasetSemanticMatcherOptions::default(),
            limits: CrossDatasetSemanticMatcherLimits::default(),
            stale: false,
        }
    }

    #[test]
    fn dataset_semantic_graph_v1_contract_uses_scoped_and_opaque_shared_ids() {
        let left = dataset(1, "项目资料");
        let right = dataset(2, "经营分析");
        let raw_content_identity = "raw-content-sha256-should-never-be-public";
        let raw_hmac_identity = "raw-hmac-v1-should-never-be-public";
        let raw_source_system = "oracle-badw-private";
        let graph_input = input(
            vec![left.clone(), right.clone()],
            vec![
                endpoint(
                    left.id,
                    "document:one",
                    DatasetSemanticGraphNodeKind::Document,
                    "经营资料",
                    vec![
                        CrossDatasetCanonicalIdentity::ExactContent {
                            content_identity: raw_content_identity.to_string(),
                        },
                        CrossDatasetCanonicalIdentity::ExactContent {
                            content_identity: raw_hmac_identity.to_string(),
                        },
                    ],
                ),
                endpoint(
                    right.id,
                    "document:two",
                    DatasetSemanticGraphNodeKind::Document,
                    "经营资料",
                    vec![
                        CrossDatasetCanonicalIdentity::ExactContent {
                            content_identity: raw_content_identity.to_string(),
                        },
                        CrossDatasetCanonicalIdentity::ExactContent {
                            content_identity: raw_hmac_identity.to_string(),
                        },
                    ],
                ),
                endpoint(
                    left.id,
                    "field:one",
                    DatasetSemanticGraphNodeKind::Field,
                    "合同编号",
                    vec![CrossDatasetCanonicalIdentity::ExactSourceField {
                        source_system_key: raw_source_system.to_string(),
                        source_schema_key: "private_schema".to_string(),
                        source_object_key: "private_table".to_string(),
                        source_field_key: "private_field".to_string(),
                    }],
                ),
                endpoint(
                    right.id,
                    "field:two",
                    DatasetSemanticGraphNodeKind::Field,
                    "合同编号",
                    vec![CrossDatasetCanonicalIdentity::ExactSourceField {
                        source_system_key: raw_source_system.to_string(),
                        source_schema_key: "private_schema".to_string(),
                        source_object_key: "private_table".to_string(),
                        source_field_key: "private_field".to_string(),
                    }],
                ),
            ],
        );

        let graph = match_cross_dataset_semantic_graph(&graph_input);
        let serialized = serde_json::to_string(&graph).expect("serializable graph contract");

        assert_eq!(graph.schema_version, DATASET_SEMANTIC_GRAPH_SCHEMA_VERSION);
        assert_eq!(graph.root_dataset_id, left.id);
        assert_eq!(graph.nodes.len(), 2);
        assert!(graph
            .nodes
            .iter()
            .any(|node| node.id.starts_with("shared:document:")
                && node.dataset_refs == vec![left.id, right.id]
                && node.visible_provenance_count == 2));
        assert!(graph
            .nodes
            .iter()
            .any(|node| node.id.starts_with("shared:field:")
                && node.dataset_refs == vec![left.id, right.id]
                && node.visible_provenance_count == 2));
        assert!(!serialized.contains(raw_content_identity));
        assert!(!serialized.contains(raw_hmac_identity));
        assert!(!serialized.contains(raw_source_system));
        assert!(!serialized.contains("private_schema"));
        assert!(!serialized.contains("private_table"));
        assert!(!serialized.contains("private_field"));
        assert_eq!(
            scoped_semantic_endpoint_id(left.id, "field:one"),
            format!("d:{}:field:one", left.id)
        );
    }

    #[test]
    fn shared_document_membership_folds_by_shared_document_id_not_membership_row() {
        let left = dataset(3, "项目资料");
        let right = dataset(4, "经营分析");
        let shared_document_id = "document-identity-1";
        let graph = match_cross_dataset_semantic_graph(&input(
            vec![left.clone(), right.clone()],
            vec![
                endpoint(
                    left.id,
                    "document:left-membership",
                    DatasetSemanticGraphNodeKind::Document,
                    "经营资料",
                    vec![CrossDatasetCanonicalIdentity::SharedDocumentMembership {
                        shared_document_id: shared_document_id.to_string(),
                    }],
                ),
                endpoint(
                    right.id,
                    "document:right-membership",
                    DatasetSemanticGraphNodeKind::Document,
                    "经营资料",
                    vec![CrossDatasetCanonicalIdentity::SharedDocumentMembership {
                        shared_document_id: shared_document_id.to_string(),
                    }],
                ),
            ],
        ));

        assert_eq!(graph.nodes.len(), 1);
        assert!(graph.nodes[0].id.starts_with("shared:document:"));
        assert_eq!(graph.nodes[0].dataset_refs, vec![left.id, right.id]);
    }

    #[test]
    fn shared_node_id_ignores_identity_supported_by_only_one_dataset() {
        let left = dataset(5, "项目资料");
        let right = dataset(6, "经营分析");
        let shared_content = CrossDatasetCanonicalIdentity::ExactContent {
            content_identity: "same-content".to_string(),
        };
        let base_endpoints = vec![
            endpoint(
                left.id,
                "document:left",
                DatasetSemanticGraphNodeKind::Document,
                "经营资料",
                vec![shared_content.clone()],
            ),
            endpoint(
                right.id,
                "document:right",
                DatasetSemanticGraphNodeKind::Document,
                "经营资料",
                vec![shared_content.clone()],
            ),
        ];
        let base_graph = match_cross_dataset_semantic_graph(&input(
            vec![left.clone(), right.clone()],
            base_endpoints.clone(),
        ));
        let mut enhanced_endpoints = base_endpoints;
        enhanced_endpoints[0]
            .identities
            .push(CrossDatasetCanonicalIdentity::CanonicalDocument {
                canonical_document_id: "left-only-internal-identity".to_string(),
            });
        let enhanced_graph =
            match_cross_dataset_semantic_graph(&input(vec![left, right], enhanced_endpoints));

        assert_eq!(base_graph.nodes.len(), 1);
        assert_eq!(enhanced_graph.nodes.len(), 1);
        assert_eq!(base_graph.nodes[0].id, enhanced_graph.nodes[0].id);
    }

    #[test]
    fn exact_document_field_and_confirmed_concept_identities_fold_but_source_name_does_not() {
        let left = dataset(11, "左数据集");
        let right = dataset(12, "右数据集");
        let exact_field =
            |system: &str, field: &str| CrossDatasetCanonicalIdentity::ExactSourceField {
                source_system_key: system.to_string(),
                source_schema_key: "finance".to_string(),
                source_object_key: "contract".to_string(),
                source_field_key: field.to_string(),
            };
        let graph = match_cross_dataset_semantic_graph(&input(
            vec![left.clone(), right.clone()],
            vec![
                endpoint(
                    left.id,
                    "doc:left",
                    DatasetSemanticGraphNodeKind::Document,
                    "同一资料",
                    vec![CrossDatasetCanonicalIdentity::CanonicalDocument {
                        canonical_document_id: "canonical-document-1".to_string(),
                    }],
                ),
                endpoint(
                    right.id,
                    "doc:right",
                    DatasetSemanticGraphNodeKind::Document,
                    "同一资料",
                    vec![CrossDatasetCanonicalIdentity::CanonicalDocument {
                        canonical_document_id: "canonical-document-1".to_string(),
                    }],
                ),
                endpoint(
                    left.id,
                    "field:exact-left",
                    DatasetSemanticGraphNodeKind::Field,
                    "门店编号",
                    vec![exact_field("erp-a", "store_id")],
                ),
                endpoint(
                    right.id,
                    "field:exact-right",
                    DatasetSemanticGraphNodeKind::Field,
                    "门店编号",
                    vec![exact_field("erp-a", "store_id")],
                ),
                endpoint(
                    left.id,
                    "concept:left",
                    DatasetSemanticGraphNodeKind::Concept,
                    "合同概念",
                    vec![CrossDatasetCanonicalIdentity::ConfirmedConcept {
                        concept_id: "confirmed-concept-1".to_string(),
                    }],
                ),
                endpoint(
                    right.id,
                    "concept:right",
                    DatasetSemanticGraphNodeKind::Concept,
                    "合同概念",
                    vec![CrossDatasetCanonicalIdentity::ConfirmedConcept {
                        concept_id: "confirmed-concept-1".to_string(),
                    }],
                ),
                endpoint(
                    left.id,
                    "field:different-left",
                    DatasetSemanticGraphNodeKind::Field,
                    "合同编号",
                    vec![exact_field("erp-a", "contract_id")],
                ),
                endpoint(
                    right.id,
                    "field:different-right",
                    DatasetSemanticGraphNodeKind::Field,
                    "合同编号",
                    vec![exact_field("erp-b", "contract_id")],
                ),
            ],
        ));

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
        assert_eq!(
            graph
                .nodes
                .iter()
                .filter(|node| node.id.starts_with("shared:concept:"))
                .count(),
            1
        );
        assert!(graph.nodes.iter().any(|node| {
            node.id == scoped_semantic_endpoint_id(left.id, "field:different-left")
        }));
        assert!(graph.nodes.iter().any(|node| {
            node.id == scoped_semantic_endpoint_id(right.id, "field:different-right")
        }));
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn equal_chinese_labels_are_inferred_only_when_explicitly_enabled_and_never_fold() {
        let left = dataset(21, "左数据集");
        let right = dataset(22, "右数据集");
        let endpoints = vec![
            endpoint(
                left.id,
                "field:left",
                DatasetSemanticGraphNodeKind::Field,
                "合同编号",
                vec![CrossDatasetCanonicalIdentity::ExactSourceField {
                    source_system_key: "erp-a".to_string(),
                    source_schema_key: "finance".to_string(),
                    source_object_key: "contract".to_string(),
                    source_field_key: "contract_id".to_string(),
                }],
            ),
            endpoint(
                right.id,
                "field:right",
                DatasetSemanticGraphNodeKind::Field,
                "合同编号",
                vec![CrossDatasetCanonicalIdentity::ExactSourceField {
                    source_system_key: "erp-b".to_string(),
                    source_schema_key: "finance".to_string(),
                    source_object_key: "contract".to_string(),
                    source_field_key: "contract_id".to_string(),
                }],
            ),
        ];
        let default_graph = match_cross_dataset_semantic_graph(&input(
            vec![left.clone(), right.clone()],
            endpoints.clone(),
        ));
        assert_eq!(default_graph.nodes.len(), 2);
        assert!(default_graph.edges.is_empty());

        let mut enabled_input = input(vec![left, right], endpoints);
        enabled_input.options.enable_label_similarity = true;
        let graph = match_cross_dataset_semantic_graph(&enabled_input);

        assert_eq!(graph.nodes.len(), 2);
        assert!(graph.nodes.iter().all(|node| node.id.starts_with("d:")));
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(
            graph.edges[0].relation_semantics,
            SemanticRelationSemantics::Similarity
        );
        assert_eq!(
            graph.edges[0].evidence_class,
            SemanticEvidenceClass::Inferred
        );
        assert!(graph.edges[0].confidence < 1.0);
        assert!(graph.edges[0].reason.contains("不代表同一实体"));
    }

    #[test]
    fn explicit_fk_and_reference_edges_preserve_requested_evidence_class() {
        let left = dataset(31, "左数据集");
        let right = dataset(32, "右数据集");
        let endpoints = vec![
            endpoint(
                left.id,
                "field:left",
                DatasetSemanticGraphNodeKind::Field,
                "门店编号",
                Vec::new(),
            ),
            endpoint(
                right.id,
                "field:right",
                DatasetSemanticGraphNodeKind::Field,
                "门店编号",
                Vec::new(),
            ),
        ];
        let mut graph_input = input(vec![left.clone(), right.clone()], endpoints);
        graph_input.explicit_relations = vec![
            CrossDatasetExplicitRelationInput {
                source_dataset_id: left.id,
                source_local_node_id: "field:left".to_string(),
                target_dataset_id: right.id,
                target_local_node_id: "field:right".to_string(),
                kind: CrossDatasetExplicitRelationKind::ForeignKey,
                evidence_class: SemanticEvidenceClass::Confirmed,
            },
            CrossDatasetExplicitRelationInput {
                source_dataset_id: right.id,
                source_local_node_id: "field:right".to_string(),
                target_dataset_id: left.id,
                target_local_node_id: "field:left".to_string(),
                kind: CrossDatasetExplicitRelationKind::ExplicitReference,
                evidence_class: SemanticEvidenceClass::Inferred,
            },
        ];

        let graph = match_cross_dataset_semantic_graph(&graph_input);

        assert_eq!(graph.edges.len(), 2);
        assert!(graph.edges.iter().all(|edge| {
            edge.relation_semantics == SemanticRelationSemantics::Reference && edge.cross_dataset
        }));
        assert!(graph.edges.iter().any(|edge| {
            edge.relation_type == "foreign_key"
                && edge.evidence_class == SemanticEvidenceClass::Confirmed
        }));
        assert!(graph.edges.iter().any(|edge| {
            edge.relation_type == "explicit_reference"
                && edge.evidence_class == SemanticEvidenceClass::Inferred
        }));
    }

    #[test]
    fn non_confirmed_fk_never_uses_confirmed_public_wording() {
        let left = dataset(33, "左数据集");
        let right = dataset(34, "右数据集");
        let endpoints = vec![
            endpoint(
                left.id,
                "field:left",
                DatasetSemanticGraphNodeKind::Field,
                "门店编号",
                Vec::new(),
            ),
            endpoint(
                right.id,
                "field:right",
                DatasetSemanticGraphNodeKind::Field,
                "门店编号",
                Vec::new(),
            ),
        ];
        let mut graph_input = input(vec![left.clone(), right.clone()], endpoints);
        graph_input.explicit_relations = vec![CrossDatasetExplicitRelationInput {
            source_dataset_id: left.id,
            source_local_node_id: "field:left".to_string(),
            target_dataset_id: right.id,
            target_local_node_id: "field:right".to_string(),
            kind: CrossDatasetExplicitRelationKind::ForeignKey,
            evidence_class: SemanticEvidenceClass::Inferred,
        }];

        let graph = match_cross_dataset_semantic_graph(&graph_input);

        assert_eq!(graph.edges.len(), 1);
        assert_eq!(
            graph.edges[0].evidence_class,
            SemanticEvidenceClass::Inferred
        );
        assert!(!graph.edges[0].label.contains("已确认"));
        assert!(graph.edges[0].reason.contains("尚未确认"));
    }

    #[test]
    fn matcher_is_bounded_and_second_phase_matchers_default_off() {
        let left = dataset(41, "左数据集");
        let right = dataset(42, "右数据集");
        let endpoints = (0..12)
            .map(|index| {
                let dataset_id = if index % 2 == 0 { left.id } else { right.id };
                endpoint(
                    dataset_id,
                    &format!("field:{index}"),
                    DatasetSemanticGraphNodeKind::Field,
                    "相同字段",
                    Vec::new(),
                )
            })
            .collect();
        let mut graph_input = input(vec![left, right], endpoints);
        graph_input.limits.max_nodes = 4;
        graph_input.limits.max_edges = 2;

        assert!(!graph_input.options.enable_label_similarity);
        assert!(!graph_input.options.enable_structure_similarity);
        assert!(!graph_input.options.enable_jaccard_similarity);
        assert!(!graph_input.options.enable_sample_overlap);

        let graph = match_cross_dataset_semantic_graph(&graph_input);
        assert_eq!(graph.nodes.len(), 4);
        assert!(graph.edges.len() <= 2);
        assert_eq!(graph.truncated.nodes, 8);
    }
}
