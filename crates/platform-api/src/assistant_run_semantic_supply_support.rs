use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[cfg(test)]
use domain_model::RetrievalEvidence;
use domain_model::{DatasetId, DocumentId, TenantId, UserId};
use uuid::Uuid;

use crate::assistant_run_lexical_query_support::{
    is_ascii_connector_token_char, is_cjk_query_token_char, lexical_surface_term_weights,
};
use crate::retrieval_evidence_ranking_support::vector_norm;
use crate::semantic_label_resolver::semantic_evidence_label_is_safe;
use crate::semantic_understanding::{
    DatasetSemanticUnderstanding, SemanticEvidenceClass, SemanticEvidenceRef, SemanticStatus,
    DATASET_SEMANTIC_GENERATION_VERSION, DATASET_SEMANTIC_SCHEMA_VERSION,
};

pub(crate) const ASSISTANT_SEMANTIC_SUPPLY_MATCH_LIMIT: usize = 8;
pub(crate) const ASSISTANT_SEMANTIC_SUPPLY_ALIAS_LIMIT: usize = 12;
pub(crate) const ASSISTANT_SEMANTIC_SUPPLY_SOURCE_LIMIT: usize = 8;
pub(crate) const ASSISTANT_SEMANTIC_SUPPLY_SUPPLEMENT_LIMIT: usize = 2;

const SEMANTIC_MATCH_MIN_SCORE: f64 = 0.08;
const SEMANTIC_DOCUMENT_BOOST_MAX: f64 = 0.30;
const SEMANTIC_ALIAS_COVERAGE_MAX_SCORE: f64 = 0.25;

pub(crate) const ASSISTANT_SEMANTIC_SUPPLY_MODE_ENV: &str = "ASSISTANT_RUN_SEMANTIC_SUPPLY_MODE";
pub(crate) const ASSISTANT_SEMANTIC_SUPPLY_TENANT_ALLOWLIST_ENV: &str =
    "ASSISTANT_RUN_SEMANTIC_SUPPLY_TENANT_ALLOWLIST";
pub(crate) const ASSISTANT_SEMANTIC_SUPPLY_DATASET_ALLOWLIST_ENV: &str =
    "ASSISTANT_RUN_SEMANTIC_SUPPLY_DATASET_ALLOWLIST";
pub(crate) const ASSISTANT_SEMANTIC_SUPPLY_USER_ALLOWLIST_ENV: &str =
    "ASSISTANT_RUN_SEMANTIC_SUPPLY_USER_ALLOWLIST";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum AssistantSemanticSupplyMode {
    #[default]
    Off,
    Shadow,
    Rerank,
    Supplement,
}

impl AssistantSemanticSupplyMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Shadow => "shadow",
            Self::Rerank => "rerank",
            Self::Supplement => "supplement",
        }
    }

    pub(crate) fn is_enabled(self) -> bool {
        self != Self::Off
    }

    pub(crate) fn changes_supply(self) -> bool {
        matches!(self, Self::Rerank | Self::Supplement)
    }
}

pub(crate) fn assistant_semantic_supply_mode_from_env(
    tenant_id: TenantId,
    dataset_id: DatasetId,
    current_user_id: Option<UserId>,
) -> AssistantSemanticSupplyMode {
    assistant_semantic_supply_mode_from_values(
        &std::env::var(ASSISTANT_SEMANTIC_SUPPLY_MODE_ENV).unwrap_or_default(),
        &std::env::var(ASSISTANT_SEMANTIC_SUPPLY_TENANT_ALLOWLIST_ENV).unwrap_or_default(),
        &std::env::var(ASSISTANT_SEMANTIC_SUPPLY_DATASET_ALLOWLIST_ENV).unwrap_or_default(),
        &std::env::var(ASSISTANT_SEMANTIC_SUPPLY_USER_ALLOWLIST_ENV).unwrap_or_default(),
        tenant_id,
        dataset_id,
        current_user_id,
    )
}

pub(crate) fn assistant_semantic_supply_effective_mode(
    configured_mode: AssistantSemanticSupplyMode,
    external_channel_scope: bool,
) -> AssistantSemanticSupplyMode {
    match (external_channel_scope, configured_mode) {
        (true, AssistantSemanticSupplyMode::Shadow) => AssistantSemanticSupplyMode::Shadow,
        (true, _) => AssistantSemanticSupplyMode::Off,
        (false, mode) => mode,
    }
}

pub(crate) fn assistant_semantic_supply_latest_attempt_invalidates_ready(
    latest_status: &str,
    newer_than_ready: bool,
) -> bool {
    newer_than_ready && matches!(latest_status.trim(), "building" | "failed")
}

fn assistant_semantic_supply_mode_from_values(
    mode: &str,
    tenant_allowlist: &str,
    dataset_allowlist: &str,
    user_allowlist: &str,
    tenant_id: TenantId,
    dataset_id: DatasetId,
    current_user_id: Option<UserId>,
) -> AssistantSemanticSupplyMode {
    let requested = match mode.trim().to_ascii_lowercase().as_str() {
        "shadow" => AssistantSemanticSupplyMode::Shadow,
        "rerank" => AssistantSemanticSupplyMode::Rerank,
        "supplement" => AssistantSemanticSupplyMode::Supplement,
        _ => AssistantSemanticSupplyMode::Off,
    };
    if !requested.is_enabled() {
        return AssistantSemanticSupplyMode::Off;
    }
    let Some(current_user_id) = current_user_id else {
        return AssistantSemanticSupplyMode::Off;
    };
    if !exact_allowlist_contains(tenant_allowlist, &tenant_id.to_string())
        || !exact_allowlist_contains(dataset_allowlist, &dataset_id.to_string())
        || !exact_allowlist_contains(user_allowlist, &current_user_id.to_string())
    {
        return AssistantSemanticSupplyMode::Off;
    }
    requested
}

fn exact_allowlist_contains(allowlist: &str, expected: &str) -> bool {
    allowlist
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .any(|entry| entry == expected)
}

/// Converts a candidate set that has already passed the normal retrieval ACL
/// pipeline into a per-dataset provenance boundary. The tenant and dataset
/// predicates intentionally prevent callers from unioning visible sources
/// across independently selected datasets.
#[cfg(test)]
pub(crate) fn semantic_supply_visible_document_ids(
    tenant_id: TenantId,
    dataset_id: DatasetId,
    scoped_evidences: &[RetrievalEvidence],
) -> BTreeSet<DocumentId> {
    scoped_evidences
        .iter()
        .filter(|evidence| evidence.tenant_id == tenant_id && evidence.dataset_id == dataset_id)
        .map(|evidence| evidence.document_id)
        .collect()
}

/// Internal-only hints for selecting existing, attributable supplies.
///
/// This contract intentionally contains no answer, intent, action, instruction,
/// example value, or free-form graph description. It is never serialized into
/// model-facing or public evidence state.
#[derive(Clone, PartialEq)]
pub(crate) struct AssistantSemanticSupplyPlan {
    pub(crate) snapshot_id: Uuid,
    pub(crate) matched_node_ids: Vec<String>,
    pub(crate) query_aliases: Vec<String>,
    pub(crate) visible_source_document_ids: Vec<DocumentId>,
    pub(crate) evidence_boosts: Vec<SemanticEvidenceBoost>,
    pub(crate) supplement_document_ids: Vec<DocumentId>,
    pub(crate) trace: SemanticSupplyTrace,
}

#[derive(Clone, PartialEq)]
pub(crate) struct SemanticEvidenceBoost {
    pub(crate) document_id: DocumentId,
    pub(crate) semantic_score: f64,
    pub(crate) match_ids: Vec<String>,
    pub(crate) attributable_aliases: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct SemanticSupplyTrace {
    pub(crate) eligible_node_count: usize,
    pub(crate) matched_node_count: usize,
    pub(crate) unresolved_provenance_count: usize,
}

impl AssistantSemanticSupplyPlan {
    pub(crate) fn is_empty(&self) -> bool {
        self.matched_node_ids.is_empty()
            || self.visible_source_document_ids.is_empty()
            || self.evidence_boosts.is_empty()
    }

    pub(crate) fn semantic_score_for_attributable_text(
        &self,
        document_id: DocumentId,
        evidence_text: &str,
    ) -> f64 {
        self.evidence_boosts
            .iter()
            .find(|boost| boost.document_id == document_id)
            .filter(|boost| semantic_boost_has_attributable_text(boost, evidence_text))
            .map(|boost| boost.semantic_score)
            .unwrap_or(0.0)
    }

    pub(crate) fn has_attributable_text(
        &self,
        document_id: DocumentId,
        evidence_text: &str,
    ) -> bool {
        self.evidence_boosts
            .iter()
            .find(|boost| boost.document_id == document_id)
            .is_some_and(|boost| semantic_boost_has_attributable_text(boost, evidence_text))
    }
}

fn semantic_boost_has_attributable_text(
    boost: &SemanticEvidenceBoost,
    evidence_text: &str,
) -> bool {
    boost
        .attributable_aliases
        .iter()
        .any(|alias| semantic_binding_alias_matches_text(alias, evidence_text))
}

impl fmt::Debug for AssistantSemanticSupplyPlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AssistantSemanticSupplyPlan")
            .field("matched_node_count", &self.matched_node_ids.len())
            .field("query_alias_count", &self.query_aliases.len())
            .field(
                "visible_source_document_count",
                &self.visible_source_document_ids.len(),
            )
            .field("evidence_boost_count", &self.evidence_boosts.len())
            .field(
                "supplement_document_count",
                &self.supplement_document_ids.len(),
            )
            .field("trace", &self.trace)
            .finish()
    }
}

#[derive(Clone, Debug)]
struct SemanticSupplyCandidate {
    node_id: String,
    aliases: Vec<String>,
    document_ids: Vec<DocumentId>,
    score: f64,
    status_rank: u8,
    kind_rank: u8,
}

/// Builds a deterministic, bounded semantic hint plan from an already-visible
/// snapshot. Storage and permission checks deliberately live outside this pure
/// function; every node is still rejected unless all of its document
/// provenance resolves inside `visible_document_ids`.
pub(crate) fn build_assistant_semantic_supply_plan(
    snapshot_id: Uuid,
    question: &str,
    understanding: &DatasetSemanticUnderstanding,
    visible_document_ids: &BTreeSet<DocumentId>,
) -> Option<AssistantSemanticSupplyPlan> {
    if !semantic_understanding_is_eligible(understanding)
        || question.trim().is_empty()
        || visible_document_ids.is_empty()
    {
        return None;
    }

    let query_weights = lexical_surface_term_weights(question);
    let query_norm = vector_norm(&query_weights);
    let normalized_query = semantic_normalize_exact_surface(question);
    if query_weights.is_empty() || query_norm <= 0.0 {
        return None;
    }

    let mut candidates = Vec::new();
    let mut trace = SemanticSupplyTrace::default();
    let mut node_documents = BTreeMap::<String, Vec<DocumentId>>::new();

    for object in &understanding.objects {
        if !semantic_status_is_supply_safe(object.status) {
            continue;
        }
        trace.eligible_node_count += 1;
        let Some(document_ids) =
            resolve_visible_document_provenance(&object.evidence_refs, visible_document_ids)
        else {
            trace.unresolved_provenance_count += 1;
            continue;
        };
        let aliases = sanitize_aliases([&object.label, &object.technical_name]);
        node_documents.insert(object.id.clone(), document_ids.clone());
        if let Some(score) =
            semantic_alias_match_score(&aliases, &normalized_query, &query_weights, query_norm)
        {
            candidates.push(SemanticSupplyCandidate {
                node_id: object.id.clone(),
                aliases,
                document_ids,
                score,
                status_rank: semantic_status_rank(object.status),
                kind_rank: 1,
            });
        }
    }

    for field in &understanding.fields {
        if !semantic_status_is_supply_safe(field.status) {
            continue;
        }
        trace.eligible_node_count += 1;
        let Some(parent_document_ids) = node_documents.get(&field.object_id) else {
            trace.unresolved_provenance_count += 1;
            continue;
        };
        let Some(field_document_ids) =
            resolve_visible_document_provenance(&field.evidence_refs, visible_document_ids)
        else {
            trace.unresolved_provenance_count += 1;
            continue;
        };
        let document_ids = intersect_document_ids(parent_document_ids, &field_document_ids);
        if document_ids.is_empty() {
            continue;
        }
        node_documents.insert(field.id.clone(), document_ids.clone());
        let mut aliases = sanitize_aliases([&field.label, &field.technical_name]);
        if semantic_role_alias_is_allowlisted(&field.semantic_role) {
            aliases.push(field.semantic_role.trim().to_ascii_lowercase());
            aliases.sort();
            aliases.dedup();
        }
        if let Some(score) =
            semantic_alias_match_score(&aliases, &normalized_query, &query_weights, query_norm)
        {
            candidates.push(SemanticSupplyCandidate {
                node_id: field.id.clone(),
                aliases,
                document_ids,
                score,
                status_rank: semantic_status_rank(field.status),
                kind_rank: 0,
            });
        }
    }

    for relation in &understanding.relations {
        if !semantic_evidence_class_is_supply_safe(relation.evidence_class) {
            continue;
        }
        trace.eligible_node_count += 1;
        let (Some(source_documents), Some(target_documents)) = (
            node_documents.get(&relation.source_id),
            node_documents.get(&relation.target_id),
        ) else {
            trace.unresolved_provenance_count += 1;
            continue;
        };
        let Some(relation_documents) =
            resolve_visible_document_provenance(&relation.evidence_refs, visible_document_ids)
        else {
            trace.unresolved_provenance_count += 1;
            continue;
        };
        let endpoint_documents = union_document_ids(source_documents, target_documents);
        let document_ids = intersect_document_ids(&endpoint_documents, &relation_documents);
        if document_ids.is_empty() {
            continue;
        }
        let mut aliases = sanitize_aliases([&relation.label]);
        if semantic_relation_type_alias_is_allowlisted(&relation.relation_type) {
            aliases.push(relation.relation_type.trim().to_ascii_lowercase());
            aliases.sort();
            aliases.dedup();
        }
        if let Some(score) =
            semantic_alias_match_score(&aliases, &normalized_query, &query_weights, query_norm)
        {
            candidates.push(SemanticSupplyCandidate {
                node_id: relation.id.clone(),
                aliases,
                document_ids,
                score,
                status_rank: semantic_evidence_class_rank(relation.evidence_class),
                kind_rank: 2,
            });
        }
    }

    candidates.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.status_rank.cmp(&right.status_rank))
            .then_with(|| left.kind_rank.cmp(&right.kind_rank))
            .then_with(|| left.node_id.cmp(&right.node_id))
    });
    candidates.truncate(ASSISTANT_SEMANTIC_SUPPLY_MATCH_LIMIT);
    if candidates.is_empty() {
        return None;
    }

    trace.matched_node_count = candidates.len();
    let mut matched_node_ids = Vec::new();
    let mut alias_scores = BTreeMap::<String, f64>::new();
    let mut document_scores = BTreeMap::<DocumentId, f64>::new();
    let mut match_ids_by_document = BTreeMap::<DocumentId, Vec<String>>::new();
    let mut aliases_by_document = BTreeMap::<DocumentId, BTreeSet<String>>::new();
    for candidate in &candidates {
        matched_node_ids.push(candidate.node_id.clone());
        for alias in &candidate.aliases {
            let score = semantic_single_alias_match_score(
                alias,
                &normalized_query,
                &query_weights,
                query_norm,
            );
            if score >= SEMANTIC_MATCH_MIN_SCORE {
                alias_scores
                    .entry(alias.clone())
                    .and_modify(|current| *current = current.max(score))
                    .or_insert(score);
            }
        }
        for document_id in &candidate.document_ids {
            document_scores
                .entry(*document_id)
                .and_modify(|current| *current = current.max(candidate.score))
                .or_insert(candidate.score);
            let ids = match_ids_by_document.entry(*document_id).or_default();
            if ids.len() < ASSISTANT_SEMANTIC_SUPPLY_MATCH_LIMIT
                && !ids.contains(&candidate.node_id)
            {
                ids.push(candidate.node_id.clone());
            }
            let binding_aliases = aliases_by_document.entry(*document_id).or_default();
            for alias in candidate
                .aliases
                .iter()
                .filter(|alias| semantic_alias_is_specific_binding(alias))
            {
                if binding_aliases.len() < ASSISTANT_SEMANTIC_SUPPLY_ALIAS_LIMIT {
                    binding_aliases.insert(alias.clone());
                }
            }
        }
    }

    let mut aliases = alias_scores.into_iter().collect::<Vec<_>>();
    aliases.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.0.cmp(&right.0))
    });
    let query_aliases = aliases
        .into_iter()
        .map(|(alias, _)| alias)
        .take(ASSISTANT_SEMANTIC_SUPPLY_ALIAS_LIMIT)
        .collect();

    let mut documents = document_scores.into_iter().collect::<Vec<_>>();
    documents.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.0.cmp(&right.0))
    });
    documents.truncate(ASSISTANT_SEMANTIC_SUPPLY_SOURCE_LIMIT);
    let visible_source_document_ids = documents
        .iter()
        .map(|(document_id, _)| *document_id)
        .collect::<Vec<_>>();
    let evidence_boosts = documents
        .into_iter()
        .map(|(document_id, score)| SemanticEvidenceBoost {
            document_id,
            semantic_score: (score * SEMANTIC_DOCUMENT_BOOST_MAX)
                .clamp(0.0, SEMANTIC_DOCUMENT_BOOST_MAX),
            match_ids: match_ids_by_document
                .remove(&document_id)
                .unwrap_or_default(),
            attributable_aliases: aliases_by_document
                .remove(&document_id)
                .unwrap_or_default()
                .into_iter()
                .collect(),
        })
        .collect::<Vec<_>>();
    let supplement_document_ids = visible_source_document_ids
        .iter()
        .copied()
        .take(ASSISTANT_SEMANTIC_SUPPLY_SUPPLEMENT_LIMIT)
        .collect();

    let plan = AssistantSemanticSupplyPlan {
        snapshot_id,
        matched_node_ids,
        query_aliases,
        visible_source_document_ids,
        evidence_boosts,
        supplement_document_ids,
        trace,
    };

    (!plan.is_empty()).then_some(plan)
}

fn semantic_understanding_is_eligible(understanding: &DatasetSemanticUnderstanding) -> bool {
    understanding.status == "ready"
        && !understanding.stale
        && understanding.schema_version == DATASET_SEMANTIC_SCHEMA_VERSION
        && understanding.generation_version == DATASET_SEMANTIC_GENERATION_VERSION
}

fn semantic_status_is_supply_safe(status: SemanticStatus) -> bool {
    matches!(status, SemanticStatus::Confirmed | SemanticStatus::Observed)
}

fn semantic_evidence_class_is_supply_safe(evidence_class: SemanticEvidenceClass) -> bool {
    matches!(
        evidence_class,
        SemanticEvidenceClass::Confirmed | SemanticEvidenceClass::Observed
    )
}

fn semantic_status_rank(status: SemanticStatus) -> u8 {
    match status {
        SemanticStatus::Confirmed => 0,
        SemanticStatus::Observed => 1,
        SemanticStatus::Inferred => 2,
        SemanticStatus::Unresolved => 3,
    }
}

fn semantic_evidence_class_rank(evidence_class: SemanticEvidenceClass) -> u8 {
    match evidence_class {
        SemanticEvidenceClass::Confirmed => 0,
        SemanticEvidenceClass::Observed => 1,
        SemanticEvidenceClass::Inferred => 2,
    }
}

fn resolve_visible_document_provenance(
    evidence_refs: &[SemanticEvidenceRef],
    visible_document_ids: &BTreeSet<DocumentId>,
) -> Option<Vec<DocumentId>> {
    if evidence_refs.is_empty() {
        return None;
    }
    let mut resolved = BTreeSet::new();
    for evidence_ref in evidence_refs {
        if !semantic_source_kind_is_document_backed(&evidence_ref.source_kind) {
            return None;
        }
        let document_id = Uuid::parse_str(evidence_ref.source_id.trim())
            .ok()
            .map(DocumentId)?;
        if !visible_document_ids.contains(&document_id) {
            return None;
        }
        resolved.insert(document_id);
    }
    (!resolved.is_empty()).then(|| resolved.into_iter().collect())
}

fn semantic_source_kind_is_document_backed(source_kind: &str) -> bool {
    matches!(
        source_kind.trim().to_ascii_lowercase().as_str(),
        "document"
            | "pdf"
            | "word"
            | "doc"
            | "docx"
            | "text"
            | "txt"
            | "markdown"
            | "spreadsheet"
            | "excel"
            | "xlsx"
            | "xls"
            | "csv"
    )
}

fn sanitize_aliases<const N: usize>(values: [&str; N]) -> Vec<String> {
    values
        .into_iter()
        .map(str::trim)
        .filter(|value| semantic_alias_is_safe(value))
        .map(ToOwned::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn semantic_alias_is_safe(value: &str) -> bool {
    let char_count = value.chars().count();
    (2..=96).contains(&char_count)
        && value.chars().any(|value| value.is_alphanumeric())
        && !value.contains('\n')
        && !value.contains('\r')
        && !semantic_alias_looks_like_structured_value(value)
        && semantic_evidence_label_is_safe(value)
}

fn semantic_alias_looks_like_structured_value(value: &str) -> bool {
    let value = value.trim();
    (value.starts_with('{') && value.ends_with('}'))
        || (value.starts_with('[') && value.ends_with(']'))
        || (value.contains("\":") && (value.contains('{') || value.contains('[')))
}

fn semantic_alias_is_specific_binding(alias: &str) -> bool {
    let normalized = semantic_normalize_binding_text(alias);
    normalized.chars().count() >= 4
        && !semantic_role_alias_is_allowlisted(alias)
        && !semantic_relation_type_alias_is_allowlisted(alias)
}

fn semantic_binding_alias_matches_text(alias: &str, evidence_text: &str) -> bool {
    if !semantic_alias_is_specific_binding(alias) {
        return false;
    }
    let normalized_alias = semantic_normalize_binding_text(alias);
    let normalized_evidence = semantic_normalize_binding_text(evidence_text);
    !normalized_alias.is_empty() && normalized_evidence.contains(&normalized_alias)
}

fn semantic_normalize_binding_text(value: &str) -> String {
    value
        .chars()
        .filter(|value| value.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn semantic_role_alias_is_allowlisted(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "identifier"
            | "name"
            | "date"
            | "amount"
            | "quantity"
            | "category"
            | "status"
            | "location"
            | "text"
            | "unknown"
            | "metric"
    )
}

fn semantic_relation_type_alias_is_allowlisted(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "identity"
            | "same_identity"
            | "foreign_key"
            | "explicit_reference"
            | "reference"
            | "parent_child"
            | "contains"
            | "collection_membership"
            | "membership"
            | "structure"
            | "label_similarity"
            | "shared_key"
            | "cooccurrence"
            | "text_similarity"
            | "similarity"
    )
}

fn semantic_alias_match_score(
    aliases: &[String],
    normalized_query: &str,
    query_weights: &BTreeMap<String, f64>,
    query_norm: f64,
) -> Option<f64> {
    let score = aliases
        .iter()
        .map(|alias| {
            semantic_single_alias_match_score(alias, normalized_query, query_weights, query_norm)
        })
        .fold(0.0, f64::max);
    (score >= SEMANTIC_MATCH_MIN_SCORE).then_some(score)
}

fn semantic_single_alias_match_score(
    alias: &str,
    normalized_query: &str,
    query_weights: &BTreeMap<String, f64>,
    query_norm: f64,
) -> f64 {
    let coverage_score = semantic_alias_query_coverage_score(alias, query_weights);
    let long_cjk_alias = alias
        .chars()
        .filter(|value| is_cjk_query_token_char(*value))
        .count()
        >= 5;
    let exact_surface_match =
        long_cjk_alias && semantic_normalized_query_contains_exact_alias(alias, normalized_query);
    if long_cjk_alias && coverage_score == 0.0 && !exact_surface_match {
        return 0.0;
    }
    semantic_surface_text_score(alias, query_weights, query_norm)
        .max(coverage_score)
        .max(if exact_surface_match {
            SEMANTIC_MATCH_MIN_SCORE
        } else {
            0.0
        })
        .clamp(0.0, 1.0)
}

fn semantic_surface_text_score(
    alias: &str,
    query_weights: &BTreeMap<String, f64>,
    query_norm: f64,
) -> f64 {
    if query_weights.is_empty() || query_norm <= 0.0 {
        return 0.0;
    }
    let alias_weights = lexical_surface_term_weights(alias);
    let alias_norm = vector_norm(&alias_weights);
    if alias_weights.is_empty() || alias_norm <= 0.0 {
        return 0.0;
    }
    let dot_product = query_weights
        .iter()
        .filter_map(|(term, query_weight)| {
            alias_weights
                .get(term)
                .map(|alias_weight| query_weight * alias_weight)
        })
        .sum::<f64>();
    if dot_product <= 0.0 {
        return 0.0;
    }
    (dot_product / (query_norm * alias_norm) * 10_000.0).round() / 10_000.0
}

fn semantic_normalized_query_contains_exact_alias(alias: &str, normalized_query: &str) -> bool {
    if !alias.chars().any(|value| !value.is_alphanumeric()) {
        return false;
    }
    let normalized_alias = semantic_normalize_exact_surface(alias);
    !normalized_alias.is_empty() && normalized_query.contains(&normalized_alias)
}

fn semantic_normalize_exact_surface(value: &str) -> String {
    value
        .chars()
        .filter(|value| value.is_alphanumeric() || is_ascii_connector_token_char(*value))
        .flat_map(char::to_lowercase)
        .collect()
}

/// Keeps a short, specific business label usable inside a much longer question.
/// Cosine similarity alone penalizes the label because the question norm grows
/// with every unrelated term. Coverage is measured from the bounded alias side
/// and still requires exact lexical overlap; it never expands synonyms or adds
/// answer, intent, route, or action semantics.
fn semantic_alias_query_coverage_score(alias: &str, query_weights: &BTreeMap<String, f64>) -> f64 {
    if alias.chars().count() < 5 {
        return 0.0;
    }
    let alias_weights = lexical_surface_term_weights(alias);
    let signal_terms = alias_weights
        .iter()
        .filter(|(term, _)| term.chars().count() >= 2)
        .collect::<Vec<_>>();
    if signal_terms.len() < 2 {
        return 0.0;
    }

    let matching_terms = signal_terms
        .iter()
        .filter(|(term, _)| query_weights.contains_key(term.as_str()))
        .collect::<Vec<_>>();
    let has_specific_term = matching_terms
        .iter()
        .any(|(term, _)| term.chars().count() >= 4);
    let has_proximate_pair = semantic_alias_query_has_proximate_term_pair(
        &alias_weights,
        query_weights,
        &matching_terms
            .iter()
            .map(|(term, _)| term.as_str())
            .collect::<Vec<_>>(),
    );
    if !has_specific_term && !has_proximate_pair {
        return 0.0;
    }

    let total_weight = signal_terms.iter().map(|(_, weight)| **weight).sum::<f64>();
    if total_weight <= 0.0 {
        return 0.0;
    }
    let matching_weight = matching_terms
        .into_iter()
        .map(|(_, weight)| **weight)
        .sum::<f64>();
    (matching_weight / total_weight).clamp(0.0, SEMANTIC_ALIAS_COVERAGE_MAX_SCORE)
}

fn semantic_alias_query_has_proximate_term_pair(
    alias_weights: &BTreeMap<String, f64>,
    query_weights: &BTreeMap<String, f64>,
    matching_terms: &[&str],
) -> bool {
    matching_terms.iter().enumerate().any(|(left_index, left)| {
        matching_terms
            .iter()
            .enumerate()
            .any(|(right_index, right)| {
                left_index != right_index
                    && left.chars().count() == 2
                    && right.chars().count() == 2
                    && semantic_term_set_contains_ordered_pair(alias_weights, left, right)
                    && semantic_term_set_contains_ordered_pair(query_weights, left, right)
            })
    })
}

fn semantic_term_set_contains_ordered_pair(
    weights: &BTreeMap<String, f64>,
    left: &str,
    right: &str,
) -> bool {
    weights.keys().any(|container| {
        let char_count = container.chars().count();
        if !(4..=6).contains(&char_count) {
            return false;
        }
        let Some(left_index) = container.find(left) else {
            return false;
        };
        let right_search_start = left_index + left.len();
        container
            .get(right_search_start..)
            .is_some_and(|tail| tail.contains(right))
    })
}

fn intersect_document_ids(left: &[DocumentId], right: &[DocumentId]) -> Vec<DocumentId> {
    let right = right.iter().copied().collect::<BTreeSet<_>>();
    left.iter()
        .copied()
        .filter(|document_id| right.contains(document_id))
        .collect()
}

fn union_document_ids(left: &[DocumentId], right: &[DocumentId]) -> Vec<DocumentId> {
    left.iter()
        .chain(right)
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use domain_model::{DocumentChunkId, RetrievalEvidenceId, WorkflowExecutionId};
    use serde_json::json;

    use super::*;
    use crate::semantic_understanding::{
        DatasetSemanticIdentity, SemanticCoverage, SemanticField, SemanticObject, SemanticRelation,
        SemanticSummary, SemanticTruncation,
    };

    fn document_id(value: u128) -> DocumentId {
        DocumentId(Uuid::from_u128(value))
    }

    fn evidence(document_id: DocumentId) -> Vec<SemanticEvidenceRef> {
        vec![SemanticEvidenceRef {
            source_kind: "spreadsheet".to_string(),
            source_id: document_id.to_string(),
            label: "document_chunk:opaque".to_string(),
        }]
    }

    fn retrieval_evidence(
        tenant_id: TenantId,
        dataset_id: DatasetId,
        document_id: DocumentId,
        value: u128,
    ) -> RetrievalEvidence {
        RetrievalEvidence {
            id: RetrievalEvidenceId(Uuid::from_u128(value)),
            tenant_id,
            dataset_id,
            execution_id: WorkflowExecutionId(Uuid::from_u128(value + 100)),
            document_id,
            document_chunk_id: DocumentChunkId(Uuid::from_u128(value + 200)),
            chunk_index: value as i32,
            source_locator: format!("chunk:{value}"),
            content_excerpt: "脱敏资料".to_string(),
            summary: "脱敏摘要".to_string(),
            payload_filter_key: format!("fixture:{value}"),
            embedding_model: "fixture".to_string(),
            recall_score: 0.5,
            evidence_manifest: json!({"rank_hint": value}),
            created_at: Utc::now(),
        }
    }

    fn build_plan(
        question: &str,
        understanding: &DatasetSemanticUnderstanding,
        visible_document_ids: &BTreeSet<DocumentId>,
    ) -> Option<AssistantSemanticSupplyPlan> {
        build_assistant_semantic_supply_plan(
            Uuid::from_u128(99),
            question,
            understanding,
            visible_document_ids,
        )
    }

    fn manual_plan(
        boosts: &[(DocumentId, f64)],
        supplement_document_ids: Vec<DocumentId>,
    ) -> AssistantSemanticSupplyPlan {
        AssistantSemanticSupplyPlan {
            snapshot_id: Uuid::from_u128(98),
            matched_node_ids: vec!["node:opaque".to_string()],
            query_aliases: Vec::new(),
            visible_source_document_ids: boosts
                .iter()
                .map(|(document_id, _)| *document_id)
                .collect(),
            evidence_boosts: boosts
                .iter()
                .map(|(document_id, semantic_score)| SemanticEvidenceBoost {
                    document_id: *document_id,
                    semantic_score: *semantic_score,
                    match_ids: vec!["node:opaque".to_string()],
                    attributable_aliases: vec!["脱敏资料".to_string()],
                })
                .collect(),
            supplement_document_ids,
            trace: SemanticSupplyTrace {
                eligible_node_count: 1,
                matched_node_count: 1,
                unresolved_provenance_count: 0,
            },
        }
    }

    fn ranked_evidence_state(ranked: &[crate::RankedRetrievalEvidence<'_>]) -> serde_json::Value {
        let supplied_items = ranked
            .iter()
            .map(|ranked| {
                json!({
                    "type": "retrieval_evidence",
                    "dataset_id": ranked.evidence.dataset_id,
                    "document_id": ranked.evidence.document_id,
                    "document_chunk_id": ranked.evidence.document_chunk_id,
                    "retrieval_evidence_id": ranked.evidence.id,
                    "chunk_index": ranked.evidence.chunk_index,
                    "source_locator": ranked.evidence.source_locator,
                    "summary": ranked.evidence.summary,
                    "content_excerpt": ranked.evidence.content_excerpt,
                    "payload_filter_key": ranked.evidence.payload_filter_key,
                    "score": ranked.score,
                    "lexical_score": ranked.lexical_score,
                    "recall_score": ranked.recall_score,
                    "evidence_manifest": ranked.evidence.evidence_manifest,
                })
            })
            .collect::<Vec<_>>();
        json!({
            "status": "supplied",
            "supplied_items": supplied_items,
        })
    }

    #[test]
    fn semantic_supply_shadow_mode_requires_all_three_exact_allowlists() {
        let tenant_id = TenantId(Uuid::from_u128(1001));
        let dataset_id = DatasetId(Uuid::from_u128(1002));
        let user_id = UserId(Uuid::from_u128(1003));
        let exact = assistant_semantic_supply_mode_from_values(
            "shadow",
            &tenant_id.to_string(),
            &dataset_id.to_string(),
            &user_id.to_string(),
            tenant_id,
            dataset_id,
            Some(user_id),
        );
        assert_eq!(exact, AssistantSemanticSupplyMode::Shadow);
        assert_eq!(exact.as_str(), "shadow");
        assert!(!exact.changes_supply());

        for (tenants, datasets, users) in [
            ("*".to_string(), dataset_id.to_string(), user_id.to_string()),
            (tenant_id.to_string(), "*".to_string(), user_id.to_string()),
            (
                tenant_id.to_string(),
                dataset_id.to_string(),
                "*".to_string(),
            ),
            (
                format!("prefix-{tenant_id}"),
                dataset_id.to_string(),
                user_id.to_string(),
            ),
            (String::new(), dataset_id.to_string(), user_id.to_string()),
        ] {
            assert_eq!(
                assistant_semantic_supply_mode_from_values(
                    "rerank",
                    &tenants,
                    &datasets,
                    &users,
                    tenant_id,
                    dataset_id,
                    Some(user_id),
                ),
                AssistantSemanticSupplyMode::Off
            );
        }
        assert_eq!(
            assistant_semantic_supply_mode_from_values(
                "supplement",
                &tenant_id.to_string(),
                &dataset_id.to_string(),
                &user_id.to_string(),
                tenant_id,
                dataset_id,
                None,
            ),
            AssistantSemanticSupplyMode::Off
        );
        assert_eq!(
            assistant_semantic_supply_mode_from_values(
                "rerank-now",
                &tenant_id.to_string(),
                &dataset_id.to_string(),
                &user_id.to_string(),
                tenant_id,
                dataset_id,
                Some(user_id),
            ),
            AssistantSemanticSupplyMode::Off
        );
        assert_eq!(
            assistant_semantic_supply_effective_mode(exact, true),
            AssistantSemanticSupplyMode::Shadow,
            "external ACL scopes may compute only byte-inert shadow receipts"
        );
        assert_eq!(
            assistant_semantic_supply_effective_mode(AssistantSemanticSupplyMode::Rerank, true),
            AssistantSemanticSupplyMode::Off,
            "external ACL scopes cannot change supply in the first release"
        );
    }

    #[test]
    fn assistant_run_evidence_scope_for_semantic_supply_permission_scope_stays_inside_current_tenant_and_dataset(
    ) {
        let tenant_id = TenantId(Uuid::from_u128(2001));
        let other_tenant_id = TenantId(Uuid::from_u128(2002));
        let dataset_id = DatasetId(Uuid::from_u128(2003));
        let other_dataset_id = DatasetId(Uuid::from_u128(2004));
        let allowed = document_id(21);
        let other_dataset = document_id(22);
        let other_tenant = document_id(23);
        let candidates = vec![
            retrieval_evidence(tenant_id, dataset_id, allowed, 1),
            retrieval_evidence(tenant_id, other_dataset_id, other_dataset, 2),
            retrieval_evidence(other_tenant_id, dataset_id, other_tenant, 3),
        ];

        assert_eq!(
            semantic_supply_visible_document_ids(tenant_id, dataset_id, &candidates),
            BTreeSet::from([allowed])
        );
    }

    #[test]
    fn semantic_supply_shadow_preserves_baseline_ranking_bytes() {
        let tenant_id = TenantId(Uuid::from_u128(3001));
        let dataset_id = DatasetId(Uuid::from_u128(1));
        let mut first = retrieval_evidence(tenant_id, dataset_id, document_id(11), 31);
        let mut second = retrieval_evidence(tenant_id, dataset_id, document_id(12), 32);
        first.recall_score = 0.20;
        second.recall_score = 0.10;
        second.content_excerpt = "inventory_ledger detail".to_string();
        second.source_locator = "documents/inventory-ledger#chunk=1".to_string();
        first.created_at = Utc::now();
        second.created_at = first.created_at;
        let evidences = vec![first, second];
        let understanding = fixture_understanding();
        let plan = build_plan(
            "库存台账",
            &understanding,
            &BTreeSet::from([document_id(11), document_id(12)]),
        )
        .expect("inventory object should produce a semantic plan");

        let (off, _) = crate::rank_retrieval_evidences_for_semantic_supply_mode(
            &evidences,
            "库存台账",
            2,
            AssistantSemanticSupplyMode::Off,
            Some(&plan),
        );
        let (shadow, hypothetical_changes) =
            crate::rank_retrieval_evidences_for_semantic_supply_mode(
                &evidences,
                "库存台账",
                2,
                AssistantSemanticSupplyMode::Shadow,
                Some(&plan),
            );
        let signature = |ranked: &[crate::RankedRetrievalEvidence<'_>]| {
            ranked
                .iter()
                .map(|ranked| {
                    (
                        ranked.evidence.id,
                        ranked.score.to_bits(),
                        ranked.lexical_score.to_bits(),
                        ranked.recall_score.to_bits(),
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(signature(&off), signature(&shadow));
        assert!(hypothetical_changes > 0);

        let off_state = ranked_evidence_state(&off);
        let shadow_state = ranked_evidence_state(&shadow);
        assert_eq!(
            serde_json::to_vec(&off_state).unwrap(),
            serde_json::to_vec(&shadow_state).unwrap(),
            "shadow must preserve final supplied evidence bytes"
        );
        let off_model_state = crate::assistant_run_model_evidence_state(&off_state);
        let shadow_model_state = crate::assistant_run_model_evidence_state(&shadow_state);
        assert_eq!(
            serde_json::to_vec(&off_model_state).unwrap(),
            serde_json::to_vec(&shadow_model_state).unwrap(),
            "shadow must preserve budgeted model evidence bytes"
        );
        let request = crate::CreateAssistantRunRequest {
            prompt: "库存台账".to_string(),
            local_thread_id: None,
            startup_briefing: None,
            selected_scope: Some(json!({
                "mode": "user_selected",
                "datasets": [dataset_id],
                "intent": "data_analysis",
            })),
            scope_candidates: Vec::new(),
            context_policy_hint: None,
            current_artifact: None,
            messages: Vec::new(),
        };
        assert_eq!(
            crate::build_assistant_run_provider_input_with_evidence(&request, Some(&off_state)),
            crate::build_assistant_run_provider_input_with_evidence(&request, Some(&shadow_state)),
            "shadow must preserve final provider input bytes"
        );

        let (reranked, _) = crate::rank_retrieval_evidences_for_semantic_supply_mode(
            &evidences,
            "库存台账",
            2,
            AssistantSemanticSupplyMode::Rerank,
            Some(&plan),
        );
        assert_eq!(reranked[0].evidence.document_id, document_id(12));
        assert_eq!(reranked[0].score.to_bits(), 0.10_f64.to_bits());
    }

    #[test]
    fn semantic_supply_rerank_boosts_exact_provenance_and_caps_documents() {
        let tenant_id = TenantId(Uuid::from_u128(3101));
        let dataset_id = DatasetId(Uuid::from_u128(1));
        let sales = document_id(11);
        let other = document_id(12);
        let mut evidences = vec![
            retrieval_evidence(tenant_id, dataset_id, sales, 41),
            retrieval_evidence(tenant_id, dataset_id, sales, 42),
            retrieval_evidence(tenant_id, dataset_id, sales, 43),
            retrieval_evidence(tenant_id, dataset_id, other, 44),
        ];
        for (evidence, recall_score) in evidences
            .iter_mut()
            .zip([0.10_f64, 0.09_f64, 0.08_f64, 0.25_f64])
        {
            evidence.recall_score = recall_score;
            evidence.created_at = Utc::now();
        }
        for evidence in evidences
            .iter_mut()
            .filter(|evidence| evidence.document_id == sales)
        {
            evidence.content_excerpt = "QUEKOU 字段资料".to_string();
            evidence.source_locator = format!("documents/quekou#chunk={}", evidence.chunk_index);
        }
        let plan = build_plan(
            "销售缺口",
            &fixture_understanding(),
            &BTreeSet::from([sales, other]),
        )
        .expect("technical field alias should resolve to the exact sales document");

        let (baseline, _) = crate::rank_retrieval_evidences_for_semantic_supply_mode(
            &evidences,
            "销售缺口",
            3,
            AssistantSemanticSupplyMode::Off,
            Some(&plan),
        );
        assert_eq!(baseline[0].evidence.document_id, other);

        let (reranked, changes) = crate::rank_retrieval_evidences_for_semantic_supply_mode(
            &evidences,
            "销售缺口",
            3,
            AssistantSemanticSupplyMode::Rerank,
            Some(&plan),
        );
        assert!(changes > 0);
        assert_eq!(reranked[0].evidence.document_id, sales);
        assert_eq!(
            reranked
                .iter()
                .filter(|ranked| ranked.evidence.document_id == sales)
                .count(),
            2
        );
        assert!(reranked
            .iter()
            .any(|ranked| ranked.evidence.document_id == other));

        let (no_match, _) = crate::rank_retrieval_evidences_for_semantic_supply_mode(
            &evidences,
            "销售缺口",
            3,
            AssistantSemanticSupplyMode::Rerank,
            None,
        );
        assert_eq!(
            baseline
                .iter()
                .map(|ranked| ranked.evidence.id)
                .collect::<Vec<_>>(),
            no_match
                .iter()
                .map(|ranked| ranked.evidence.id)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn semantic_supply_boost_requires_the_actual_chunk_to_match_a_node_alias() {
        let tenant_id = TenantId(Uuid::from_u128(3111));
        let dataset_id = DatasetId(Uuid::from_u128(1));
        let sales = document_id(11);
        let mut attributable = retrieval_evidence(tenant_id, dataset_id, sales, 45);
        attributable.content_excerpt = "QUEKOU 字段定义".to_string();
        attributable.summary = "销售缺口字段".to_string();
        attributable.source_locator = "documents/quekou#chunk=1".to_string();
        let attributable_id = attributable.id;
        let mut unrelated_same_document = retrieval_evidence(tenant_id, dataset_id, sales, 46);
        unrelated_same_document.content_excerpt = "欢迎页和目录".to_string();
        unrelated_same_document.summary = "通用介绍".to_string();
        unrelated_same_document.source_locator = "documents/quekou#chunk=2".to_string();
        unrelated_same_document.payload_filter_key = "QUEKOU".to_string();
        unrelated_same_document.evidence_manifest = json!({
            "evidence": {"section_title_hints": ["销售缺口", "QUEKOU"]},
        });
        let unrelated_id = unrelated_same_document.id;
        let evidences = vec![attributable, unrelated_same_document];
        let plan = build_plan(
            "销售缺口是什么字段？",
            &fixture_understanding(),
            &BTreeSet::from([sales]),
        )
        .expect("the technical alias should create a document-backed plan");

        let (ranked, _) = crate::rank_retrieval_evidences_for_semantic_supply_mode(
            &evidences,
            "销售缺口是什么字段？",
            2,
            AssistantSemanticSupplyMode::Rerank,
            Some(&plan),
        );
        assert!(ranked
            .iter()
            .any(|item| { item.evidence.id == attributable_id && item.semantic_score > 0.0 }));
        assert!(ranked
            .iter()
            .any(|item| { item.evidence.id == unrelated_id && item.semantic_score == 0.0 }));
    }

    #[test]
    fn semantic_supply_rerank_preserves_a_weak_lexical_target() {
        let tenant_id = TenantId(Uuid::from_u128(3121));
        let dataset_id = DatasetId(Uuid::from_u128(3122));
        let target_document = document_id(51);
        let decoy_document = document_id(52);
        let mut target = retrieval_evidence(tenant_id, dataset_id, target_document, 51);
        target.content_excerpt = "gap".to_string();
        target.summary = "field gap evidence".to_string();
        target.recall_score = 0.99;
        let mut decoy = retrieval_evidence(tenant_id, dataset_id, decoy_document, 52);
        decoy.content_excerpt = "unrelated".to_string();
        decoy.summary = "generic overview".to_string();
        decoy.recall_score = 1.0;
        let evidences = vec![target, decoy];
        let plan = manual_plan(&[(target_document, 0.30)], Vec::new());
        let prompt = "字段 gap_amt 在完整经营分析资料里代表什么？";

        let (baseline, _) = crate::rank_retrieval_evidences_for_semantic_supply_mode(
            &evidences,
            prompt,
            2,
            AssistantSemanticSupplyMode::Off,
            Some(&plan),
        );
        assert_eq!(
            baseline[0].evidence.document_id, target_document,
            "the baseline lexical-first ranker recognizes the target"
        );

        let (reranked, _) = crate::rank_retrieval_evidences_for_semantic_supply_mode(
            &evidences,
            prompt,
            2,
            AssistantSemanticSupplyMode::Rerank,
            Some(&plan),
        );
        assert!(
            reranked
                .iter()
                .find(|item| item.evidence.document_id == target_document)
                .is_some_and(|item| item.lexical_score > 0.0),
            "fixture must exercise the weak-lexical branch"
        );
        assert_eq!(
            reranked[0].evidence.document_id, target_document,
            "semantic reranking must not demote a baseline lexical target merely because its weak lexical score replaced its stronger recall score"
        );
    }

    #[test]
    fn semantic_supply_rerank_does_not_restore_unmatched_decoy_recall() {
        let tenant_id = TenantId(Uuid::from_u128(3131));
        let dataset_id = DatasetId(Uuid::from_u128(3132));
        let target_document = document_id(53);
        let decoy_document = document_id(54);
        let mut target = retrieval_evidence(tenant_id, dataset_id, target_document, 53);
        target.content_excerpt = "gap amt".to_string();
        target.summary = "gap amt".to_string();
        target.recall_score = 0.10;
        let mut decoy = retrieval_evidence(tenant_id, dataset_id, decoy_document, 54);
        decoy.content_excerpt = "gap".to_string();
        decoy.summary = "gap".to_string();
        decoy.recall_score = 1.0;
        let evidences = vec![target, decoy];
        let plan = manual_plan(&[(target_document, 0.075)], Vec::new());
        let prompt =
            "gap_amt alpha beta gamma delta epsilon zeta theta lambda metric context question";

        let (baseline, _) = crate::rank_retrieval_evidences_for_semantic_supply_mode(
            &evidences,
            prompt,
            2,
            AssistantSemanticSupplyMode::Off,
            Some(&plan),
        );
        assert_eq!(
            baseline[0].evidence.document_id, target_document,
            "the stronger lexical target must lead the baseline"
        );

        let (reranked, _) = crate::rank_retrieval_evidences_for_semantic_supply_mode(
            &evidences,
            prompt,
            2,
            AssistantSemanticSupplyMode::Rerank,
            Some(&plan),
        );
        assert_eq!(
            reranked[0].evidence.document_id, target_document,
            "an unmatched weak-lexical decoy must not recover its high recall score only because another document matched the graph"
        );
    }

    #[test]
    fn semantic_supply_document_diversity_preserves_single_document_detail_and_refills_topk() {
        let tenant_id = TenantId(Uuid::from_u128(3151));
        let dataset_id = DatasetId(Uuid::from_u128(3152));
        let primary = document_id(61);
        let secondary = document_id(62);
        let tertiary = document_id(63);
        let created_at = Utc::now();

        let mut single_document = (0..5)
            .map(|index| {
                let mut evidence = retrieval_evidence(tenant_id, dataset_id, primary, 100 + index);
                evidence.recall_score = 1.0 - index as f64 * 0.01;
                evidence.created_at = created_at;
                evidence
            })
            .collect::<Vec<_>>();
        let plan = manual_plan(&[(primary, 0.20)], Vec::new());
        let (single_ranked, _) = crate::rank_retrieval_evidences_for_semantic_supply_mode(
            &single_document,
            "明细",
            5,
            AssistantSemanticSupplyMode::Rerank,
            Some(&plan),
        );
        assert_eq!(
            single_ranked.len(),
            5,
            "single-document detail evidence must not be collapsed to two chunks"
        );

        let mut multi_document = std::mem::take(&mut single_document);
        for (document_id, value, score) in [(secondary, 201, 0.20), (tertiary, 202, 0.10)] {
            let mut evidence = retrieval_evidence(tenant_id, dataset_id, document_id, value);
            evidence.recall_score = score;
            evidence.created_at = created_at;
            multi_document.push(evidence);
        }
        let plan = manual_plan(
            &[(primary, 0.20), (secondary, 0.0), (tertiary, 0.0)],
            Vec::new(),
        );
        let (multi_ranked, _) = crate::rank_retrieval_evidences_for_semantic_supply_mode(
            &multi_document,
            "明细",
            6,
            AssistantSemanticSupplyMode::Rerank,
            Some(&plan),
        );
        assert_eq!(
            multi_ranked.len(),
            6,
            "multi-document diversity must refill unused TopK slots"
        );
        assert!(multi_ranked
            .iter()
            .take(4)
            .any(|ranked| ranked.evidence.document_id == secondary));
        assert!(multi_ranked
            .iter()
            .take(4)
            .any(|ranked| ranked.evidence.document_id == tertiary));
    }

    #[test]
    fn semantic_supply_supplement_requires_citable_provenance_and_is_bounded() {
        let tenant_id = TenantId(Uuid::from_u128(3201));
        let dataset_id = DatasetId(Uuid::from_u128(3202));
        let baseline_document = document_id(31);
        let supplement_document = document_id(32);
        let mut baseline = retrieval_evidence(tenant_id, dataset_id, baseline_document, 51);
        let mut supplement_one = retrieval_evidence(tenant_id, dataset_id, supplement_document, 52);
        let mut invalid = retrieval_evidence(tenant_id, dataset_id, supplement_document, 53);
        let mut supplement_two = retrieval_evidence(tenant_id, dataset_id, supplement_document, 54);
        baseline.recall_score = 0.90;
        supplement_one.recall_score = 0.80;
        invalid.recall_score = 0.75;
        invalid.source_locator.clear();
        invalid.document_chunk_id = DocumentChunkId(Uuid::nil());
        supplement_two.recall_score = 0.70;
        let created_at = Utc::now();
        for evidence in [
            &mut baseline,
            &mut supplement_one,
            &mut invalid,
            &mut supplement_two,
        ] {
            evidence.created_at = created_at;
        }
        let evidences = vec![baseline, supplement_one, invalid, supplement_two];
        let plan = manual_plan(
            &[(baseline_document, 0.0), (supplement_document, 0.0)],
            vec![supplement_document],
        );

        let (supplied, _) = crate::rank_retrieval_evidences_for_semantic_supply_mode(
            &evidences,
            "无词法命中问题",
            1,
            AssistantSemanticSupplyMode::Supplement,
            Some(&plan),
        );
        assert_eq!(
            supplied.len(),
            1,
            "supplements must occupy, not extend, the caller's final retrieval budget"
        );
        assert!(supplied.iter().all(|ranked| {
            ranked.evidence.document_id == supplement_document
                && !ranked.evidence.document_chunk_id.0.is_nil()
                && !ranked.evidence.source_locator.trim().is_empty()
        }));
        assert!(!supplied
            .iter()
            .any(|ranked| ranked.evidence.id == evidences[2].id));
    }

    #[test]
    fn semantic_supply_recovers_authorized_alias_document_missing_from_lexical_pool() {
        let tenant_id = TenantId(Uuid::from_u128(3221));
        let dataset_id = DatasetId(Uuid::from_u128(3222));
        let baseline_document = document_id(91);
        let recovered_document = document_id(11);
        let mut baseline = retrieval_evidence(tenant_id, dataset_id, baseline_document, 61);
        baseline.recall_score = 0.95;
        let initial = vec![baseline];
        let visible_document_ids = BTreeSet::from([baseline_document, recovered_document]);
        let plan = build_plan("销售缺口", &fixture_understanding(), &visible_document_ids)
            .expect("authorized alias provenance should produce a plan");
        assert!(plan.supplement_document_ids.contains(&recovered_document));

        let recovery_document_ids = crate::assistant_semantic_supply_recovery_document_ids(
            &plan,
            &visible_document_ids,
            &initial,
            ASSISTANT_SEMANTIC_SUPPLY_SUPPLEMENT_LIMIT,
        );
        assert_eq!(recovery_document_ids, vec![recovered_document]);

        let mut recovered_one = retrieval_evidence(tenant_id, dataset_id, recovered_document, 62);
        let mut invalid = retrieval_evidence(tenant_id, dataset_id, recovered_document, 63);
        let mut recovered_two = retrieval_evidence(tenant_id, dataset_id, recovered_document, 64);
        let mut metadata_only = retrieval_evidence(tenant_id, dataset_id, recovered_document, 65);
        recovered_one.content_excerpt = "销售缺口字段资料".to_string();
        recovered_two.content_excerpt = "QUEKOU 技术字段资料".to_string();
        metadata_only.content_excerpt = "欢迎页和通用介绍".to_string();
        metadata_only.summary = "通用目录".to_string();
        metadata_only.source_locator = "documents/quekou#chunk=metadata-only".to_string();
        metadata_only.payload_filter_key = "QUEKOU".to_string();
        metadata_only.evidence_manifest = json!({
            "section_title_hints": ["销售缺口", "QUEKOU"],
        });
        metadata_only.recall_score = 1.0;
        let metadata_only_id = metadata_only.id;
        invalid.id = RetrievalEvidenceId(Uuid::nil());
        invalid.document_chunk_id = DocumentChunkId(Uuid::nil());
        invalid.source_locator.clear();
        let supplement_candidates = crate::merge_assistant_semantic_supplement_candidates(
            &initial,
            vec![metadata_only, recovered_one, invalid, recovered_two],
        );

        let (arm_a, _, _) =
            crate::rank_retrieval_evidences_for_semantic_supply_mode_with_supplement_candidates(
                &initial,
                &supplement_candidates,
                "销售缺口",
                3,
                AssistantSemanticSupplyMode::Off,
                Some(&plan),
                ASSISTANT_SEMANTIC_SUPPLY_SUPPLEMENT_LIMIT,
            );
        let (arm_b, _, _) =
            crate::rank_retrieval_evidences_for_semantic_supply_mode_with_supplement_candidates(
                &initial,
                &supplement_candidates,
                "销售缺口",
                3,
                AssistantSemanticSupplyMode::Rerank,
                Some(&plan),
                ASSISTANT_SEMANTIC_SUPPLY_SUPPLEMENT_LIMIT,
            );
        let (without_persisted_recovery, _, missing_count) =
            crate::rank_retrieval_evidences_for_semantic_supply_mode_with_supplement_candidates(
                &initial,
                &initial,
                "销售缺口",
                3,
                AssistantSemanticSupplyMode::Supplement,
                Some(&plan),
                ASSISTANT_SEMANTIC_SUPPLY_SUPPLEMENT_LIMIT,
            );
        let (arm_c, _, supplement_count) =
            crate::rank_retrieval_evidences_for_semantic_supply_mode_with_supplement_candidates(
                &initial,
                &supplement_candidates,
                "销售缺口",
                3,
                AssistantSemanticSupplyMode::Supplement,
                Some(&plan),
                ASSISTANT_SEMANTIC_SUPPLY_SUPPLEMENT_LIMIT,
            );

        assert_eq!(
            arm_a
                .iter()
                .map(|ranked| ranked.evidence.id)
                .collect::<Vec<_>>(),
            arm_b
                .iter()
                .map(|ranked| ranked.evidence.id)
                .collect::<Vec<_>>(),
            "an out-of-pool graph match must not perturb Arm B"
        );
        assert_eq!(missing_count, 0);
        assert_eq!(
            without_persisted_recovery
                .iter()
                .map(|ranked| ranked.evidence.id)
                .collect::<Vec<_>>(),
            arm_b
                .iter()
                .map(|ranked| ranked.evidence.id)
                .collect::<Vec<_>>()
        );
        assert_eq!(supplement_count, 2);
        assert!(arm_c.len() <= 3);
        let recovered = arm_c
            .iter()
            .filter(|ranked| ranked.evidence.document_id == recovered_document)
            .collect::<Vec<_>>();
        assert_eq!(recovered.len(), 2);
        assert!(!arm_c
            .iter()
            .any(|ranked| ranked.evidence.id == metadata_only_id));
        assert!(recovered.iter().all(|ranked| {
            !ranked.evidence.id.0.is_nil()
                && !ranked.evidence.document_chunk_id.0.is_nil()
                && !ranked.evidence.source_locator.trim().is_empty()
        }));
    }

    #[test]
    fn semantic_supply_recovery_ids_require_current_authorized_scope() {
        let tenant_id = TenantId(Uuid::from_u128(3231));
        let dataset_id = DatasetId(Uuid::from_u128(3232));
        let already_retrieved = document_id(71);
        let authorized_missing = document_id(72);
        let other_dataset_or_hidden = document_id(73);
        let plan = manual_plan(
            &[(authorized_missing, 0.2), (other_dataset_or_hidden, 0.2)],
            vec![
                already_retrieved,
                other_dataset_or_hidden,
                authorized_missing,
            ],
        );
        let initial = vec![retrieval_evidence(
            tenant_id,
            dataset_id,
            already_retrieved,
            65,
        )];
        let authorized = BTreeSet::from([already_retrieved, authorized_missing]);

        assert_eq!(
            crate::assistant_semantic_supply_recovery_document_ids(
                &plan,
                &authorized,
                &initial,
                ASSISTANT_SEMANTIC_SUPPLY_SUPPLEMENT_LIMIT,
            ),
            vec![authorized_missing]
        );
        assert!(crate::assistant_semantic_supply_recovery_document_ids(
            &plan,
            &authorized,
            &initial,
            0,
        )
        .is_empty());
    }

    #[test]
    fn semantic_supply_plan_with_only_out_of_pool_boost_preserves_baseline_order() {
        let tenant_id = TenantId(Uuid::from_u128(3241));
        let dataset_id = DatasetId(Uuid::from_u128(3242));
        let mut lexical = retrieval_evidence(tenant_id, dataset_id, document_id(81), 66);
        lexical.content_excerpt = "销售缺口".to_string();
        lexical.recall_score = 0.01;
        let mut recalled = retrieval_evidence(tenant_id, dataset_id, document_id(82), 67);
        recalled.content_excerpt = "其他资料".to_string();
        recalled.recall_score = 0.99;
        let evidences = vec![lexical, recalled];
        let plan = manual_plan(&[(document_id(83), 0.30)], vec![document_id(83)]);

        let (baseline, _) = crate::rank_retrieval_evidences_for_semantic_supply_mode(
            &evidences,
            "销售缺口",
            evidences.len(),
            AssistantSemanticSupplyMode::Off,
            Some(&plan),
        );
        let (reranked, _) = crate::rank_retrieval_evidences_for_semantic_supply_mode(
            &evidences,
            "销售缺口",
            evidences.len(),
            AssistantSemanticSupplyMode::Rerank,
            Some(&plan),
        );
        let receipt = |items: &[crate::RankedRetrievalEvidence<'_>]| {
            items
                .iter()
                .map(|item| {
                    (
                        item.evidence.id,
                        item.score.to_bits(),
                        item.lexical_score.to_bits(),
                        item.recall_score.to_bits(),
                        item.semantic_score.to_bits(),
                        item.ranking_score.to_bits(),
                        item.rank_hint,
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(receipt(&reranked), receipt(&baseline));
    }

    #[test]
    fn semantic_supply_supplement_survives_model_retrieval_budget() {
        let tenant_id = TenantId(Uuid::from_u128(3251));
        let dataset_id = DatasetId(Uuid::from_u128(3252));
        let supplement_document = document_id(90);
        let created_at = Utc::now();
        let mut evidences = (0..9)
            .map(|index| {
                let mut evidence = retrieval_evidence(
                    tenant_id,
                    dataset_id,
                    document_id(100 + index),
                    300 + index,
                );
                evidence.recall_score = 1.0 - index as f64 * 0.05;
                evidence.created_at = created_at;
                evidence
            })
            .collect::<Vec<_>>();
        let mut supplement = retrieval_evidence(tenant_id, dataset_id, supplement_document, 400);
        supplement.recall_score = 0.01;
        supplement.created_at = created_at;
        let supplement_id = supplement.id;
        evidences.push(supplement);
        let plan = manual_plan(&[(supplement_document, 0.0)], vec![supplement_document]);

        let (supplied, _) = crate::rank_retrieval_evidences_for_semantic_supply_mode(
            &evidences,
            "无词法命中问题",
            crate::ASSISTANT_RUN_MODEL_CONTEXT_RETRIEVAL_LIMIT,
            AssistantSemanticSupplyMode::Supplement,
            Some(&plan),
        );
        assert_eq!(
            supplied.len(),
            crate::ASSISTANT_RUN_MODEL_CONTEXT_RETRIEVAL_LIMIT
        );
        let mut evidence_state = ranked_evidence_state(&supplied);
        let retrieval_items = evidence_state["supplied_items"]
            .as_array_mut()
            .expect("supplied retrieval items should exist")
            .split_off(0);
        let model_saturating_prefix = (0..4)
            .map(|index| json!({"type": "document_parse_status", "title": index}))
            .chain((0..8).map(|index| json!({"type": "dataset_fact_snapshot", "title": index})))
            .chain((0..6).map(|index| json!({"type": "spreadsheet_row_analysis", "title": index})))
            .collect::<Vec<_>>();
        evidence_state["supplied_items"] = json!(model_saturating_prefix
            .into_iter()
            .chain(retrieval_items)
            .collect::<Vec<_>>());
        let model_state = crate::assistant_run_model_evidence_state(&evidence_state);
        let model_items = model_state["supplied_items"]
            .as_array()
            .expect("model supplied items should exist");
        let retrieval_count = model_items
            .iter()
            .filter(|item| item["type"] == json!("retrieval_evidence"))
            .count();
        assert!(retrieval_count > 0);
        assert!(retrieval_count <= crate::ASSISTANT_RUN_MODEL_CONTEXT_RETRIEVAL_LIMIT);
        assert!(model_items
            .iter()
            .any(|item| item["retrieval_evidence_id"] == json!(supplement_id)));
    }

    #[test]
    fn semantic_supply_supplement_budget_is_global_across_selected_datasets() {
        let tenant_id = TenantId(Uuid::from_u128(3271));
        let created_at = Utc::now();
        let mut remaining = ASSISTANT_SEMANTIC_SUPPLY_SUPPLEMENT_LIMIT;
        let mut total_supplements = 0usize;

        for dataset_index in 0..2_u128 {
            let dataset_id = DatasetId(Uuid::from_u128(3280 + dataset_index));
            let baseline_one = document_id(200 + dataset_index * 10);
            let baseline_two = document_id(201 + dataset_index * 10);
            let supplement_document = document_id(202 + dataset_index * 10);
            let mut evidences = vec![
                retrieval_evidence(
                    tenant_id,
                    dataset_id,
                    baseline_one,
                    500 + dataset_index * 10,
                ),
                retrieval_evidence(
                    tenant_id,
                    dataset_id,
                    baseline_two,
                    501 + dataset_index * 10,
                ),
                retrieval_evidence(
                    tenant_id,
                    dataset_id,
                    supplement_document,
                    502 + dataset_index * 10,
                ),
                retrieval_evidence(
                    tenant_id,
                    dataset_id,
                    supplement_document,
                    503 + dataset_index * 10,
                ),
            ];
            for (evidence, score) in evidences.iter_mut().zip([0.9, 0.8, 0.1, 0.05]) {
                evidence.recall_score = score;
                evidence.created_at = created_at;
            }
            let plan = manual_plan(&[(supplement_document, 0.0)], vec![supplement_document]);
            let (_, _, added) =
                crate::rank_retrieval_evidences_for_semantic_supply_mode_with_supplement_limit(
                    &evidences,
                    "无词法命中问题",
                    2,
                    AssistantSemanticSupplyMode::Supplement,
                    Some(&plan),
                    remaining,
                );
            remaining = remaining.saturating_sub(added);
            total_supplements += added;
        }

        assert_eq!(
            total_supplements,
            ASSISTANT_SEMANTIC_SUPPLY_SUPPLEMENT_LIMIT
        );
        assert_eq!(remaining, 0);
    }

    #[test]
    fn semantic_supply_newer_failed_or_building_attempt_invalidates_previous_ready() {
        for status in ["building", "failed"] {
            assert!(assistant_semantic_supply_latest_attempt_invalidates_ready(
                status, true
            ));
            assert!(!assistant_semantic_supply_latest_attempt_invalidates_ready(
                status, false
            ));
        }
        assert!(!assistant_semantic_supply_latest_attempt_invalidates_ready(
            "ready", true
        ));
    }

    #[tokio::test]
    async fn semantic_supply_snapshot_lookup_timeout_fails_open() {
        let result = crate::assistant_semantic_supply_with_timeout(
            std::time::Duration::from_millis(1),
            async {
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                7usize
            },
        )
        .await;
        assert_eq!(result, Err("snapshot_lookup_timeout"));
    }

    fn fixture_understanding() -> DatasetSemanticUnderstanding {
        let sales = document_id(11);
        let inventory = document_id(12);
        DatasetSemanticUnderstanding {
            schema_version: DATASET_SEMANTIC_SCHEMA_VERSION.to_string(),
            generation_version: DATASET_SEMANTIC_GENERATION_VERSION.to_string(),
            status: "ready".to_string(),
            dataset: DatasetSemanticIdentity {
                id: DatasetId(Uuid::from_u128(1)),
                title: "脱敏经营资料".to_string(),
            },
            coverage: SemanticCoverage::default(),
            summary: SemanticSummary::default(),
            objects: vec![
                SemanticObject {
                    id: "object:sales".to_string(),
                    kind: "spreadsheet_table".to_string(),
                    label: "销售经营".to_string(),
                    technical_name: "sales_operation".to_string(),
                    description: "must never enter the plan".to_string(),
                    label_source: "fixture".to_string(),
                    confidence: 0.96,
                    coverage_count: 10,
                    status: SemanticStatus::Confirmed,
                    evidence_refs: evidence(sales),
                },
                SemanticObject {
                    id: "object:inventory".to_string(),
                    kind: "spreadsheet_table".to_string(),
                    label: "库存台账".to_string(),
                    technical_name: "inventory_ledger".to_string(),
                    description: String::new(),
                    label_source: "fixture".to_string(),
                    confidence: 0.88,
                    coverage_count: 8,
                    status: SemanticStatus::Observed,
                    evidence_refs: evidence(inventory),
                },
            ],
            fields: vec![
                SemanticField {
                    id: "field:sales-gap".to_string(),
                    object_id: "object:sales".to_string(),
                    label: "销售缺口".to_string(),
                    technical_name: "QUEKOU".to_string(),
                    semantic_role: "metric".to_string(),
                    value_type: "decimal".to_string(),
                    non_empty_count: 10,
                    distinct_count: 8,
                    examples: vec!["private raw value".to_string()],
                    status: SemanticStatus::Observed,
                    label_source: "fixture".to_string(),
                    confidence: 0.91,
                    evidence_refs: evidence(sales),
                },
                SemanticField {
                    id: "field:inferred".to_string(),
                    object_id: "object:sales".to_string(),
                    label: "预测销售".to_string(),
                    technical_name: "forecast_sales".to_string(),
                    semantic_role: "metric".to_string(),
                    value_type: "decimal".to_string(),
                    non_empty_count: 1,
                    distinct_count: 1,
                    examples: vec!["secret".to_string()],
                    status: SemanticStatus::Inferred,
                    label_source: "fixture".to_string(),
                    confidence: 0.4,
                    evidence_refs: evidence(sales),
                },
            ],
            relations: vec![SemanticRelation {
                id: "relation:sales-inventory".to_string(),
                source_id: "object:sales".to_string(),
                target_id: "object:inventory".to_string(),
                relation_type: "reference".to_string(),
                label: "销售关联库存".to_string(),
                evidence_class: SemanticEvidenceClass::Observed,
                confidence: 0.82,
                reason: "must never enter the plan".to_string(),
                evidence_refs: evidence(sales),
            }],
            source_groups: Vec::new(),
            pipeline: Vec::new(),
            generated_at: Utc::now(),
            stale: false,
            truncated: SemanticTruncation::default(),
        }
    }

    #[test]
    fn assistant_run_semantic_supply_support_matches_chinese_labels_with_visible_provenance() {
        let understanding = fixture_understanding();
        let visible = BTreeSet::from([document_id(11), document_id(12)]);
        let plan = build_plan("销售缺口是多少", &understanding, &visible)
            .expect("matching visible semantic field should produce a plan");

        assert_eq!(plan.matched_node_ids[0], "field:sales-gap");
        assert_eq!(plan.snapshot_id, Uuid::from_u128(99));
        assert_eq!(plan.visible_source_document_ids, vec![document_id(11)]);
        assert!(plan.query_aliases.contains(&"销售缺口".to_string()));
        assert!(
            plan.semantic_score_for_attributable_text(document_id(11), "销售缺口字段资料") > 0.0
        );
        assert!(!format!("{plan:?}").contains("private raw value"));
        assert!(!format!("{plan:?}").contains("must never enter the plan"));
    }

    #[test]
    fn semantic_supply_short_chinese_alias_survives_long_question_norm() {
        for (alias, question) in [
            (
                "缺勤处理制度建议",
                "找出缺勤最多的部门，并引用制度中的处理建议。",
            ),
            (
                "员工关联键",
                "按员工编号关联考勤和培训资料，找出缺勤人员的培训完成情况。",
            ),
            (
                "合同关联键",
                "按合同编号关联账单和风险资料，说明逾期账单对应的合同风险。",
            ),
            (
                "设备关联键",
                "按设备编号关联资产台账和维护记录，说明故障设备的维护状态。",
            ),
            (
                "销售-库存-合同",
                "请根据当前选中的全部资料和既有经营分析结论，围绕销售-库存-合同进行全面深入分析，并分别说明各项经营指标、历史变化趋势、区域差异、部门表现、异常原因、责任归属、风险影响、处理优先级以及后续改善建议和实施计划。",
            ),
        ] {
            let query_weights = lexical_surface_term_weights(question);
            assert!(
                semantic_single_alias_match_score(
                    alias,
                    &semantic_normalize_exact_surface(question),
                    &query_weights,
                    vector_norm(&query_weights),
                ) >= SEMANTIC_MATCH_MIN_SCORE,
                "specific business alias should survive question norm: {alias}"
            );
        }
        for alias in [
            "销售",
            "合同",
            "部门",
            "制度",
            "资料",
            "状态",
            "数据",
            "合同号",
            "销售数据",
            "合同风险",
        ] {
            let question = format!("请结合全部已选资料详细分析{alias}对当前业务的影响。");
            let query_weights = lexical_surface_term_weights(&question);
            assert_eq!(
                semantic_alias_query_coverage_score(alias, &query_weights),
                0.0,
                "short common aliases must not receive coverage fallback: {alias}"
            );
        }
        for (alias, question) in [
            ("销售数据表", "请比较销售、合同和历史数据。"),
            ("销售数据表", "请输出数据表"),
            ("合同风险表", "请按合同编号找资料，再说明当前经营风险。"),
            ("合同风险表", "请生成风险表"),
            ("销售-库存-合同", "请比较销售、库存、合同"),
            (
                "预算差异说明",
                "预算偏差最大的指标是什么，说明材料如何解释该偏差？",
            ),
            ("药品委托发放记录", "服药禁忌"),
            ("A B", "table"),
        ] {
            let query_weights = lexical_surface_term_weights(question);
            assert_eq!(
                semantic_alias_query_coverage_score(alias, &query_weights),
                0.0,
                "dispersed terms or unstated synonyms must not create a semantic match: {alias}"
            );
            assert!(
                semantic_single_alias_match_score(
                    alias,
                    &semantic_normalize_exact_surface(question),
                    &query_weights,
                    vector_norm(&query_weights),
                ) < SEMANTIC_MATCH_MIN_SCORE,
                "long CJK aliases without a specific or proximate overlap must not match: {alias}"
            );
        }

        let mut understanding = fixture_understanding();
        understanding.objects.truncate(1);
        understanding.objects[0].id = "object:attendance-policy".to_string();
        understanding.objects[0].label = "缺勤处理制度建议".to_string();
        understanding.objects[0].technical_name = "attendance_policy".to_string();
        understanding.fields.clear();
        understanding.relations.clear();

        let plan = build_plan(
            "找出缺勤最多的部门，并引用制度中的处理建议。",
            &understanding,
            &BTreeSet::from([document_id(11)]),
        )
        .expect("specific overlapping Chinese business phrases should match a short alias");

        assert_eq!(plan.matched_node_ids, vec!["object:attendance-policy"]);
        assert!(build_plan(
            "请简单总结一下这份资料。",
            &understanding,
            &BTreeSet::from([document_id(11)]),
        )
        .is_none());
    }

    #[test]
    fn assistant_run_semantic_supply_support_matches_technical_names_and_roles() {
        let understanding = fixture_understanding();
        let visible = BTreeSet::from([document_id(11), document_id(12)]);
        let technical = build_plan("QUEKOU field", &understanding, &visible)
            .expect("technical field name should match");
        assert_eq!(technical.matched_node_ids[0], "field:sales-gap");

        let role =
            build_plan("metric", &understanding, &visible).expect("semantic role should match");
        assert_eq!(role.matched_node_ids[0], "field:sales-gap");
        assert!(!role
            .matched_node_ids
            .contains(&"field:inferred".to_string()));
    }

    #[test]
    fn assistant_run_semantic_supply_support_filters_untrusted_statuses_and_provenance() {
        let mut understanding = fixture_understanding();
        let visible = BTreeSet::from([document_id(11)]);
        assert!(build_plan("库存台账", &understanding, &visible).is_none());
        let inferred = build_plan("预测销售", &understanding, &visible)
            .expect("the visible parent object may still match the question");
        assert!(!inferred
            .matched_node_ids
            .contains(&"field:inferred".to_string()));

        understanding.fields[0].evidence_refs[0].source_kind = "database".to_string();
        let unsupported = build_plan("销售缺口", &understanding, &visible)
            .expect("the visible parent object may still match the question");
        assert!(!unsupported
            .matched_node_ids
            .contains(&"field:sales-gap".to_string()));

        understanding.fields[0].evidence_refs[0].source_kind = "spreadsheet".to_string();
        understanding.fields[0].evidence_refs[0].source_id = "not-a-document-id".to_string();
        let unresolved = build_plan("销售缺口", &understanding, &visible)
            .expect("the visible parent object may still match the question");
        assert!(!unresolved
            .matched_node_ids
            .contains(&"field:sales-gap".to_string()));
    }

    #[test]
    fn semantic_supply_provenance_requires_every_reference_to_be_visible() {
        let mut understanding = fixture_understanding();
        understanding.objects.truncate(1);
        understanding.fields.clear();
        understanding.relations.clear();
        understanding.objects[0]
            .evidence_refs
            .push(SemanticEvidenceRef {
                source_kind: "spreadsheet".to_string(),
                source_id: document_id(12).to_string(),
                label: "document_chunk:second".to_string(),
            });

        assert!(build_plan(
            "销售经营",
            &understanding,
            &BTreeSet::from([document_id(11)])
        )
        .is_none());
        let resolved = build_plan(
            "销售经营",
            &understanding,
            &BTreeSet::from([document_id(11), document_id(12)]),
        )
        .expect("all source documents are independently visible");
        assert_eq!(
            resolved.visible_source_document_ids,
            vec![document_id(11), document_id(12)]
        );
        assert_eq!(resolved.trace.unresolved_provenance_count, 0);
    }

    #[test]
    fn semantic_supply_provenance_supports_observed_field_to_field_relations() {
        let mut understanding = fixture_understanding();
        let mut inventory_field = understanding.fields[0].clone();
        inventory_field.id = "field:inventory-quantity".to_string();
        inventory_field.object_id = "object:inventory".to_string();
        inventory_field.label = "库存数量".to_string();
        inventory_field.technical_name = "inventory_quantity".to_string();
        inventory_field.evidence_refs = evidence(document_id(12));
        understanding.fields.push(inventory_field);
        understanding.relations = vec![SemanticRelation {
            id: "relation:gap-inventory".to_string(),
            source_id: "field:sales-gap".to_string(),
            target_id: "field:inventory-quantity".to_string(),
            relation_type: "reference".to_string(),
            label: "销售缺口关联库存数量".to_string(),
            evidence_class: SemanticEvidenceClass::Observed,
            confidence: 0.9,
            reason: "must never enter the plan".to_string(),
            evidence_refs: vec![
                evidence(document_id(11)).remove(0),
                evidence(document_id(12)).remove(0),
            ],
        }];

        let plan = build_plan(
            "销售缺口关联库存数量",
            &understanding,
            &BTreeSet::from([document_id(11), document_id(12)]),
        )
        .expect("observed relation with visible field endpoints should be usable");
        assert!(plan
            .matched_node_ids
            .contains(&"relation:gap-inventory".to_string()));
        assert_eq!(
            plan.visible_source_document_ids,
            vec![document_id(11), document_id(12)]
        );
    }

    #[test]
    fn assistant_run_semantic_supply_support_rejects_polluted_aliases_and_open_ended_types() {
        for polluted in [
            "postgres://user:secret@example.invalid/db",
            "select * from private_table",
            "password=customer-secret",
            r"C:\private\raw.xlsx",
            r#"{"raw_value":"customer-secret"}"#,
        ] {
            assert!(
                !semantic_alias_is_safe(polluted),
                "polluted alias must be rejected: {polluted}"
            );
        }
        assert!(semantic_alias_is_safe("销售缺口"));
        assert!(semantic_alias_is_safe("QUEKOU"));
        assert!(semantic_role_alias_is_allowlisted("amount"));
        assert!(!semantic_role_alias_is_allowlisted("customer-secret"));
        assert!(semantic_relation_type_alias_is_allowlisted("reference"));
        assert!(!semantic_relation_type_alias_is_allowlisted(
            "select private_table"
        ));

        let mut understanding = fixture_understanding();
        understanding.objects[0].label = "password=customer-secret".to_string();
        understanding.objects[0].technical_name = r"C:\private\raw.xlsx".to_string();
        understanding.fields.clear();
        understanding.relations.clear();
        let visible = BTreeSet::from([document_id(11)]);
        assert!(build_plan("customer-secret", &understanding, &visible).is_none());
    }

    #[test]
    fn assistant_run_semantic_supply_support_rejects_stale_invalid_and_empty_snapshots() {
        let visible = BTreeSet::from([document_id(11), document_id(12)]);
        let mut understanding = fixture_understanding();
        understanding.stale = true;
        assert!(build_plan("销售缺口", &understanding, &visible).is_none());

        understanding = fixture_understanding();
        understanding.status = "empty".to_string();
        assert!(build_plan("销售缺口", &understanding, &visible).is_none());

        understanding = fixture_understanding();
        understanding.schema_version = "0".to_string();
        assert!(build_plan("销售缺口", &understanding, &visible).is_none());

        understanding = fixture_understanding();
        understanding.generation_version = "future".to_string();
        assert!(build_plan("销售缺口", &understanding, &visible).is_none());

        understanding = fixture_understanding();
        assert!(build_plan("", &understanding, &visible).is_none());
    }

    #[test]
    fn assistant_run_semantic_supply_support_is_bounded_and_deterministic() {
        let mut understanding = fixture_understanding();
        let source = document_id(11);
        understanding.fields = (0..32)
            .map(|index| SemanticField {
                id: format!("field:{index:02}"),
                object_id: "object:sales".to_string(),
                label: format!("销售指标{index:02}"),
                technical_name: format!("sales_metric_{index:02}"),
                semantic_role: "metric".to_string(),
                value_type: "decimal".to_string(),
                non_empty_count: 10,
                distinct_count: 8,
                examples: vec![format!("raw-{index}")],
                status: SemanticStatus::Observed,
                label_source: "fixture".to_string(),
                confidence: 0.9,
                evidence_refs: evidence(source),
            })
            .collect();
        let visible = BTreeSet::from([source]);

        let left = build_plan("销售指标 metric", &understanding, &visible).expect("bounded plan");
        understanding.fields.reverse();
        let right =
            build_plan("销售指标 metric", &understanding, &visible).expect("deterministic plan");

        assert_eq!(left, right);
        assert!(left.matched_node_ids.len() <= ASSISTANT_SEMANTIC_SUPPLY_MATCH_LIMIT);
        assert!(left.query_aliases.len() <= ASSISTANT_SEMANTIC_SUPPLY_ALIAS_LIMIT);
        assert!(left.visible_source_document_ids.len() <= ASSISTANT_SEMANTIC_SUPPLY_SOURCE_LIMIT);
        assert!(left.supplement_document_ids.len() <= ASSISTANT_SEMANTIC_SUPPLY_SUPPLEMENT_LIMIT);
    }
}
