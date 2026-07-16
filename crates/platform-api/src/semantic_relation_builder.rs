use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use crate::semantic_label_resolver::{safe_public_examples, SemanticRole};
use crate::semantic_understanding::{
    stable_semantic_id, SemanticEvidenceClass, SemanticEvidenceRef, SemanticRelation,
    MAX_EVIDENCE_REFS,
};

const MAX_RELATIONS_PER_PAIR: usize = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplicitRelationKind {
    ForeignKey,
    ParentChild,
    ExplicitReference,
    Contains,
    CollectionMembership,
    Cooccurrence,
    TextSimilarity,
}

impl ExplicitRelationKind {
    fn relation_type(self) -> &'static str {
        match self {
            Self::ForeignKey => "foreign_key",
            Self::ParentChild => "parent_child",
            Self::ExplicitReference => "explicit_reference",
            Self::Contains => "contains",
            Self::CollectionMembership => "collection_membership",
            Self::Cooccurrence => "cooccurrence",
            Self::TextSimilarity => "text_similarity",
        }
    }

    fn evidence_class(self) -> SemanticEvidenceClass {
        match self {
            Self::ForeignKey | Self::ParentChild | Self::ExplicitReference => {
                SemanticEvidenceClass::Confirmed
            }
            Self::Contains | Self::CollectionMembership => SemanticEvidenceClass::Observed,
            Self::Cooccurrence | Self::TextSimilarity => SemanticEvidenceClass::Inferred,
        }
    }

    fn confidence(self) -> f64 {
        match self {
            Self::ForeignKey => 1.0,
            Self::ParentChild | Self::ExplicitReference => 0.98,
            Self::Contains | Self::CollectionMembership => 0.95,
            Self::Cooccurrence => 0.45,
            Self::TextSimilarity => 0.35,
        }
    }

    fn symmetric(self) -> bool {
        matches!(self, Self::Cooccurrence | Self::TextSimilarity)
    }
}

#[derive(Clone, Debug)]
pub struct ExplicitRelationEvidence {
    pub source_id: String,
    pub target_id: String,
    pub kind: ExplicitRelationKind,
    pub reason: String,
    pub evidence_refs: Vec<SemanticEvidenceRef>,
}

impl ExplicitRelationEvidence {
    pub fn new(
        source_id: &str,
        target_id: &str,
        kind: ExplicitRelationKind,
        reason: &str,
        evidence_refs: Vec<SemanticEvidenceRef>,
    ) -> Self {
        Self {
            source_id: source_id.to_string(),
            target_id: target_id.to_string(),
            kind,
            reason: reason.to_string(),
            evidence_refs,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RelationFieldProfile {
    pub id: String,
    pub object_id: String,
    pub technical_name: String,
    pub value_type: String,
    pub semantic_role: SemanticRole,
    pub non_empty_count: u64,
    pub total_count: u64,
    pub distinct_count: u64,
    pub examples: Vec<String>,
    pub sensitive: bool,
    pub evidence_refs: Vec<SemanticEvidenceRef>,
}

pub fn build_semantic_relations(
    explicit: &[ExplicitRelationEvidence],
    fields: &[RelationFieldProfile],
) -> Vec<SemanticRelation> {
    let mut candidates = Vec::new();
    for evidence in explicit {
        if evidence.source_id == evidence.target_id
            || evidence.reason.trim().is_empty()
            || evidence.evidence_refs.is_empty()
        {
            continue;
        }
        let (source_id, target_id) = canonical_endpoints(
            &evidence.source_id,
            &evidence.target_id,
            evidence.kind.symmetric(),
        );
        let relation_type = evidence.kind.relation_type();
        candidates.push(SemanticRelation {
            id: stable_semantic_id("relation", &[&source_id, &target_id, relation_type]),
            source_id,
            target_id,
            relation_type: relation_type.to_string(),
            label: relation_label(evidence.kind).to_string(),
            evidence_class: evidence.kind.evidence_class(),
            confidence: evidence.kind.confidence(),
            reason: evidence.reason.trim().to_string(),
            evidence_refs: normalized_evidence(&evidence.evidence_refs),
        });
    }

    for left_index in 0..fields.len() {
        for right_index in (left_index + 1)..fields.len() {
            if let Some(relation) = inferred_shared_key(&fields[left_index], &fields[right_index]) {
                candidates.push(relation);
            }
        }
    }

    let mut deduped = BTreeMap::<(String, String, String), SemanticRelation>::new();
    for relation in candidates {
        let key = (
            relation.source_id.clone(),
            relation.target_id.clone(),
            relation.relation_type.clone(),
        );
        match deduped.get(&key) {
            Some(current) if compare_relation_priority(&relation, current) != Ordering::Less => {}
            _ => {
                deduped.insert(key, relation);
            }
        }
    }
    let mut relations = deduped.into_values().collect::<Vec<_>>();
    relations.sort_by(compare_relation_priority);

    let mut pair_counts = BTreeMap::<(String, String), usize>::new();
    relations.retain(|relation| {
        let pair = canonical_endpoints(&relation.source_id, &relation.target_id, true);
        let count = pair_counts.entry(pair).or_default();
        if *count >= MAX_RELATIONS_PER_PAIR {
            false
        } else {
            *count += 1;
            true
        }
    });
    relations
}

fn inferred_shared_key(
    left: &RelationFieldProfile,
    right: &RelationFieldProfile,
) -> Option<SemanticRelation> {
    if left.object_id == right.object_id
        || left.sensitive
        || right.sensitive
        || left.semantic_role != SemanticRole::Identifier
        || right.semantic_role != SemanticRole::Identifier
        || is_pseudo_identifier(&left.technical_name)
        || is_pseudo_identifier(&right.technical_name)
        || !compatible_types(&left.value_type, &right.value_type)
        || coverage(left) < 0.8
        || coverage(right) < 0.8
        || left.distinct_count < 2
        || right.distinct_count < 2
        || left.distinct_count > left.non_empty_count
        || right.distinct_count > right.non_empty_count
    {
        return None;
    }

    let left_values = safe_public_examples(&left.examples)
        .into_iter()
        .collect::<BTreeSet<_>>();
    let right_values = safe_public_examples(&right.examples)
        .into_iter()
        .collect::<BTreeSet<_>>();
    let overlap_count = left_values.intersection(&right_values).count();
    let comparison_count = left_values.len().min(right_values.len());
    if overlap_count < 2 || comparison_count < 2 {
        return None;
    }
    let overlap_ratio = overlap_count as f64 / comparison_count as f64;
    if overlap_ratio < 0.5 {
        return None;
    }

    let (source_id, target_id) = canonical_endpoints(&left.id, &right.id, true);
    let mut evidence_refs = left.evidence_refs.clone();
    evidence_refs.extend(right.evidence_refs.clone());
    let confidence =
        (0.65 + overlap_ratio.min(1.0) * 0.15 + coverage(left).min(coverage(right)) * 0.1).min(0.9);
    Some(SemanticRelation {
        id: stable_semantic_id("relation", &[&source_id, &target_id, "shared_key"]),
        source_id,
        target_id,
        relation_type: "shared_key".to_string(),
        label: "可能通过共享键关联".to_string(),
        evidence_class: SemanticEvidenceClass::Inferred,
        confidence,
        reason: format!(
            "字段类型兼容，非空覆盖率均不低于 80%，受控样本重合 {overlap_count}/{comparison_count}；未发现已确认外键。"
        ),
        evidence_refs: normalized_evidence(&evidence_refs),
    })
}

fn coverage(field: &RelationFieldProfile) -> f64 {
    if field.total_count == 0 {
        0.0
    } else {
        field.non_empty_count as f64 / field.total_count as f64
    }
}

fn compatible_types(left: &str, right: &str) -> bool {
    let family = |value: &str| match value.trim().to_ascii_lowercase().as_str() {
        "int" | "integer" | "long" | "float" | "double" | "decimal" | "number" => "number",
        "date" | "datetime" | "timestamp" => "date",
        "string" | "varchar" | "char" | "text" => "text",
        "bool" | "boolean" => "boolean",
        _ => "unknown",
    };
    let left = family(left);
    left != "unknown" && left == family(right)
}

fn is_pseudo_identifier(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "id" | "key" | "code" | "no" | "number" | "identifier"
    )
}

fn canonical_endpoints(source_id: &str, target_id: &str, symmetric: bool) -> (String, String) {
    if symmetric && source_id > target_id {
        (target_id.to_string(), source_id.to_string())
    } else {
        (source_id.to_string(), target_id.to_string())
    }
}

fn relation_label(kind: ExplicitRelationKind) -> &'static str {
    match kind {
        ExplicitRelationKind::ForeignKey => "外键关联",
        ExplicitRelationKind::ParentChild => "父子结构",
        ExplicitRelationKind::ExplicitReference => "明确引用",
        ExplicitRelationKind::Contains => "包含",
        ExplicitRelationKind::CollectionMembership => "属于集合",
        ExplicitRelationKind::Cooccurrence => "同组共现",
        ExplicitRelationKind::TextSimilarity => "文本相似",
    }
}

fn normalized_evidence(values: &[SemanticEvidenceRef]) -> Vec<SemanticEvidenceRef> {
    let mut unique = BTreeMap::new();
    for value in values {
        if value.source_kind.trim().is_empty() || value.source_id.trim().is_empty() {
            continue;
        }
        unique
            .entry((
                value.source_kind.clone(),
                value.source_id.clone(),
                value.label.clone(),
            ))
            .or_insert_with(|| value.clone());
    }
    unique.into_values().take(MAX_EVIDENCE_REFS).collect()
}

fn compare_relation_priority(left: &SemanticRelation, right: &SemanticRelation) -> Ordering {
    evidence_rank(left.evidence_class)
        .cmp(&evidence_rank(right.evidence_class))
        .then_with(|| {
            right
                .confidence
                .partial_cmp(&left.confidence)
                .unwrap_or(Ordering::Equal)
        })
        .then_with(|| left.source_id.cmp(&right.source_id))
        .then_with(|| left.target_id.cmp(&right.target_id))
        .then_with(|| left.relation_type.cmp(&right.relation_type))
}

fn evidence_rank(class: SemanticEvidenceClass) -> u8 {
    match class {
        SemanticEvidenceClass::Confirmed => 0,
        SemanticEvidenceClass::Observed => 1,
        SemanticEvidenceClass::Inferred => 2,
    }
}

pub fn cap_semantic_evidence_class(
    requested: SemanticEvidenceClass,
    strongest_allowed: SemanticEvidenceClass,
) -> SemanticEvidenceClass {
    if evidence_rank(requested) < evidence_rank(strongest_allowed) {
        strongest_allowed
    } else {
        requested
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic_label_resolver::SemanticRole;
    use crate::semantic_understanding::{SemanticEvidenceClass, SemanticEvidenceRef};

    fn evidence(label: &str) -> Vec<SemanticEvidenceRef> {
        vec![SemanticEvidenceRef {
            source_kind: "fixture".to_string(),
            source_id: label.to_string(),
            label: label.to_string(),
        }]
    }

    fn field(id: &str, object_id: &str, key: &str, values: &[&str]) -> RelationFieldProfile {
        RelationFieldProfile {
            id: id.to_string(),
            object_id: object_id.to_string(),
            technical_name: key.to_string(),
            value_type: "text".to_string(),
            semantic_role: SemanticRole::Identifier,
            non_empty_count: 100,
            total_count: 100,
            distinct_count: 90,
            examples: values.iter().map(|value| (*value).to_string()).collect(),
            sensitive: false,
            evidence_refs: evidence(id),
        }
    }

    #[test]
    fn explicit_fk_parent_reference_and_observed_containment_keep_evidence_classes() {
        let inputs = vec![
            ExplicitRelationEvidence::new(
                "contract.store_id",
                "store.id",
                ExplicitRelationKind::ForeignKey,
                "外键指向门店",
                evidence("fk"),
            ),
            ExplicitRelationEvidence::new(
                "document",
                "section",
                ExplicitRelationKind::Contains,
                "文档包含章节",
                evidence("structure"),
            ),
        ];
        let relations = build_semantic_relations(&inputs, &[]);

        assert_eq!(relations.len(), 2);
        assert!(relations.iter().any(|relation| {
            relation.evidence_class == SemanticEvidenceClass::Confirmed
                && relation.relation_type == "foreign_key"
        }));
        assert!(relations.iter().any(|relation| {
            relation.evidence_class == SemanticEvidenceClass::Observed
                && relation.relation_type == "contains"
        }));
        assert!(relations
            .iter()
            .all(|relation| { !relation.reason.is_empty() && !relation.evidence_refs.is_empty() }));
    }

    #[test]
    fn shared_key_requires_compatible_type_coverage_overlap_and_cardinality() {
        let left = field("field-a", "object-a", "contract_id", &["C1", "C2", "C3"]);
        let right = field("field-b", "object-b", "contract_no", &["C2", "C3", "C4"]);

        let relations = build_semantic_relations(&[], &[left, right]);

        assert_eq!(relations.len(), 1);
        assert_eq!(relations[0].evidence_class, SemanticEvidenceClass::Inferred);
        assert_eq!(relations[0].relation_type, "shared_key");
        assert!(relations[0].confidence < 1.0);
    }

    #[test]
    fn unsafe_empty_constant_free_text_and_pseudo_id_fields_do_not_create_shared_keys() {
        let mut cases = vec![
            field("constant-a", "a", "status", &["active", "active"]),
            field("constant-b", "b", "status", &["active", "active"]),
            field("empty-a", "c", "contract_id", &[]),
            field("empty-b", "d", "contract_id", &[]),
            field("pseudo-a", "e", "id", &["1", "2"]),
            field("pseudo-b", "f", "id", &["1", "2"]),
            field("secret-a", "g", "api_key", &["sk-secret"]),
            field("secret-b", "h", "api_key", &["sk-secret"]),
            field("text-a", "i", "description", &["long text", "same text"]),
            field("text-b", "j", "description", &["same text", "other text"]),
        ];
        cases[0].distinct_count = 1;
        cases[1].distinct_count = 1;
        cases[2].non_empty_count = 0;
        cases[3].non_empty_count = 0;
        cases[6].sensitive = true;
        cases[7].sensitive = true;
        cases[8].semantic_role = SemanticRole::Text;
        cases[9].semantic_role = SemanticRole::Text;

        assert!(build_semantic_relations(&[], &cases).is_empty());
    }

    #[test]
    fn symmetric_relations_are_canonicalized_deduplicated_and_pair_limited() {
        let relations = build_semantic_relations(
            &[
                ExplicitRelationEvidence::new(
                    "b",
                    "a",
                    ExplicitRelationKind::TextSimilarity,
                    "文本相似",
                    evidence("similarity-a"),
                ),
                ExplicitRelationEvidence::new(
                    "a",
                    "b",
                    ExplicitRelationKind::TextSimilarity,
                    "重复文本相似",
                    evidence("similarity-b"),
                ),
                ExplicitRelationEvidence::new(
                    "a",
                    "b",
                    ExplicitRelationKind::Cooccurrence,
                    "同组共现",
                    evidence("cooccurrence"),
                ),
            ],
            &[],
        );

        assert_eq!(relations.len(), 2);
        assert!(relations.iter().all(|relation| relation.source_id == "a"));
        assert!(relations.iter().all(|relation| relation.target_id == "b"));
        assert!(relations
            .iter()
            .all(|relation| relation.evidence_class == SemanticEvidenceClass::Inferred));
    }

    #[test]
    fn evidence_class_cap_never_upgrades_inferred_relationships() {
        assert_eq!(
            cap_semantic_evidence_class(
                SemanticEvidenceClass::Inferred,
                SemanticEvidenceClass::Confirmed,
            ),
            SemanticEvidenceClass::Inferred
        );
        assert_eq!(
            cap_semantic_evidence_class(
                SemanticEvidenceClass::Confirmed,
                SemanticEvidenceClass::Inferred,
            ),
            SemanticEvidenceClass::Inferred
        );
    }
}
