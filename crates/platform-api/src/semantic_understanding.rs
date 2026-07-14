use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use chrono::{DateTime, Utc};
use domain_model::DatasetId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const DATASET_SEMANTIC_SCHEMA_VERSION: &str = "1.0.0";
pub const DATASET_SEMANTIC_GENERATION_VERSION: &str = "semantic_profile_v1";
pub const MAX_OBJECTS: usize = 40;
pub const MAX_FIELDS: usize = 160;
pub const MAX_RELATIONS: usize = 240;
pub const MAX_EXAMPLES: usize = 5;
pub const MAX_EVIDENCE_REFS: usize = 5;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticStatus {
    Confirmed,
    Observed,
    Inferred,
    Unresolved,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticEvidenceClass {
    Confirmed,
    Observed,
    Inferred,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SemanticEvidenceRef {
    pub source_kind: String,
    pub source_id: String,
    pub label: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DatasetSemanticIdentity {
    pub id: DatasetId,
    pub title: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct SemanticCoverage {
    pub source_count: u64,
    pub document_count: u64,
    pub record_count: u64,
    pub asset_count: u64,
    pub retrieval_evidence_count: u64,
    pub confirmed_fact_count: u64,
    pub unresolved_field_count: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct SemanticSummary {
    pub headline: String,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SemanticObject {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub technical_name: String,
    pub description: String,
    pub label_source: String,
    pub confidence: f64,
    pub coverage_count: u64,
    pub status: SemanticStatus,
    pub evidence_refs: Vec<SemanticEvidenceRef>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SemanticField {
    pub id: String,
    pub object_id: String,
    pub label: String,
    pub technical_name: String,
    pub semantic_role: String,
    pub value_type: String,
    pub non_empty_count: u64,
    pub distinct_count: u64,
    pub examples: Vec<String>,
    pub status: SemanticStatus,
    pub label_source: String,
    pub confidence: f64,
    pub evidence_refs: Vec<SemanticEvidenceRef>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SemanticRelation {
    pub id: String,
    pub source_id: String,
    pub target_id: String,
    pub relation_type: String,
    pub label: String,
    pub evidence_class: SemanticEvidenceClass,
    pub confidence: f64,
    pub reason: String,
    pub evidence_refs: Vec<SemanticEvidenceRef>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SemanticObservation {
    pub id: String,
    pub source_kind: String,
    pub source_id: String,
    pub observation_kind: String,
    pub object_kind: String,
    pub object_key: String,
    pub technical_name: String,
    pub label_hint: Option<String>,
    pub label_source: String,
    pub value_type: Option<String>,
    pub semantic_role_hint: Option<String>,
    pub observed_values: Vec<String>,
    pub attributes: Value,
    pub status: SemanticStatus,
    pub confidence: f64,
    pub evidence_refs: Vec<SemanticEvidenceRef>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct SemanticSourceGroup {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub object_ids: Vec<String>,
    pub coverage_count: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct SemanticPipelineStage {
    pub id: String,
    pub label: String,
    pub status: String,
    pub detail: String,
    pub evidence_count: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct SemanticTruncation {
    pub objects: usize,
    pub fields: usize,
    pub relations: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DatasetSemanticUnderstanding {
    pub schema_version: String,
    pub generation_version: String,
    pub status: String,
    pub dataset: DatasetSemanticIdentity,
    pub coverage: SemanticCoverage,
    pub summary: SemanticSummary,
    pub objects: Vec<SemanticObject>,
    pub fields: Vec<SemanticField>,
    pub relations: Vec<SemanticRelation>,
    pub source_groups: Vec<SemanticSourceGroup>,
    pub pipeline: Vec<SemanticPipelineStage>,
    pub generated_at: DateTime<Utc>,
    pub stale: bool,
    pub truncated: SemanticTruncation,
}

pub fn stable_semantic_id(prefix: &str, parts: &[&str]) -> String {
    let mut digest = Sha256::new();
    digest.update(prefix.trim().as_bytes());
    for part in parts {
        digest.update([0]);
        digest.update(part.trim().as_bytes());
    }
    let digest = digest.finalize();
    let mut encoded = String::with_capacity(32);
    for byte in &digest[..16] {
        write!(&mut encoded, "{byte:02x}").expect("writing to a string cannot fail");
    }
    format!("{}:{encoded}", prefix.trim())
}

pub fn build_semantic_summary_headline(objects: &[SemanticObject], source_count: u64) -> String {
    let labels = objects
        .iter()
        .filter(|object| {
            matches!(
                object.status,
                SemanticStatus::Confirmed | SemanticStatus::Observed
            )
        })
        .map(|object| object.label.trim())
        .filter(|label| !label.is_empty() && label.chars().any(is_cjk))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .take(4)
        .collect::<Vec<_>>();

    if labels.is_empty() {
        return format!("系统已整理 {source_count} 类来源，业务含义仍待确认。");
    }

    format!(
        "系统识别到 {source_count} 类业务数据，覆盖{}。",
        labels.join("、")
    )
}

fn is_cjk(character: char) -> bool {
    matches!(
        character,
        '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{f900}'..='\u{faff}'
    )
}

pub fn normalize_semantic_understanding(
    mut model: DatasetSemanticUnderstanding,
) -> DatasetSemanticUnderstanding {
    model.schema_version = DATASET_SEMANTIC_SCHEMA_VERSION.to_string();
    model.generation_version = DATASET_SEMANTIC_GENERATION_VERSION.to_string();
    model.summary.limitations = normalized_strings(model.summary.limitations, usize::MAX);

    for object in &mut model.objects {
        object.confidence = normalized_confidence(object.confidence);
        object.evidence_refs = normalized_evidence(std::mem::take(&mut object.evidence_refs));
    }
    model.objects = dedupe_by_id(model.objects, |item| &item.id, compare_objects);
    model.truncated.objects = model.objects.len().saturating_sub(MAX_OBJECTS);
    model.objects.truncate(MAX_OBJECTS);

    let object_ids = model
        .objects
        .iter()
        .map(|object| object.id.as_str())
        .collect::<BTreeSet<_>>();
    for field in &mut model.fields {
        field.confidence = normalized_confidence(field.confidence);
        field.examples = normalized_strings(std::mem::take(&mut field.examples), MAX_EXAMPLES);
        field.evidence_refs = normalized_evidence(std::mem::take(&mut field.evidence_refs));
    }
    model
        .fields
        .retain(|field| object_ids.contains(field.object_id.as_str()));
    model.fields = dedupe_by_id(model.fields, |item| &item.id, compare_fields);
    model.truncated.fields = model.fields.len().saturating_sub(MAX_FIELDS);
    model.fields.truncate(MAX_FIELDS);

    let valid_node_ids = object_ids
        .into_iter()
        .map(str::to_string)
        .chain(model.fields.iter().map(|field| field.id.clone()))
        .collect::<BTreeSet<_>>();
    for relation in &mut model.relations {
        relation.confidence = normalized_confidence(relation.confidence);
        relation.evidence_refs = normalized_evidence(std::mem::take(&mut relation.evidence_refs));
    }
    model.relations.retain(|relation| {
        valid_node_ids.contains(&relation.source_id)
            && valid_node_ids.contains(&relation.target_id)
            && relation.source_id != relation.target_id
    });
    model.relations = dedupe_by_id(model.relations, |item| &item.id, compare_relations);
    model.truncated.relations = model.relations.len().saturating_sub(MAX_RELATIONS);
    model.relations.truncate(MAX_RELATIONS);

    for group in &mut model.source_groups {
        group.object_ids = normalized_strings(std::mem::take(&mut group.object_ids), MAX_OBJECTS);
        group
            .object_ids
            .retain(|object_id| valid_node_ids.contains(object_id));
    }
    model
        .source_groups
        .sort_by(|left, right| left.id.cmp(&right.id));
    model
        .source_groups
        .dedup_by(|left, right| left.id == right.id);
    model.pipeline.sort_by(|left, right| {
        pipeline_stage_rank(&left.id)
            .cmp(&pipeline_stage_rank(&right.id))
            .then_with(|| left.id.cmp(&right.id))
    });
    model.pipeline.dedup_by(|left, right| left.id == right.id);
    model
}

fn pipeline_stage_rank(id: &str) -> u8 {
    match id {
        "source" => 0,
        "structure" => 1,
        "labels" => 2,
        "relations" => 3,
        "facts" => 4,
        _ => 5,
    }
}

fn normalized_confidence(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn normalized_strings(values: Vec<String>, limit: usize) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .take(limit)
        .collect()
}

fn normalized_evidence(values: Vec<SemanticEvidenceRef>) -> Vec<SemanticEvidenceRef> {
    let mut keyed = BTreeMap::new();
    for mut value in values {
        value.source_kind = value.source_kind.trim().to_string();
        value.source_id = value.source_id.trim().to_string();
        value.label = value.label.trim().to_string();
        if value.source_kind.is_empty() || value.source_id.is_empty() {
            continue;
        }
        let key = (
            value.source_kind.clone(),
            value.source_id.clone(),
            value.label.clone(),
        );
        keyed.entry(key).or_insert(value);
    }
    keyed.into_values().take(MAX_EVIDENCE_REFS).collect()
}

fn dedupe_by_id<T, I, C>(items: Vec<T>, id: I, compare: C) -> Vec<T>
where
    I: Fn(&T) -> &str,
    C: Fn(&T, &T) -> Ordering + Copy,
{
    let mut unique = BTreeMap::new();
    for item in items {
        let key = id(&item).to_string();
        match unique.get(&key) {
            Some(current) if compare(&item, current) != Ordering::Less => {}
            _ => {
                unique.insert(key, item);
            }
        }
    }
    let mut values = unique.into_values().collect::<Vec<_>>();
    values.sort_by(compare);
    values
}

fn compare_objects(left: &SemanticObject, right: &SemanticObject) -> Ordering {
    status_rank(left.status)
        .cmp(&status_rank(right.status))
        .then_with(|| right.coverage_count.cmp(&left.coverage_count))
        .then_with(|| compare_confidence(right.confidence, left.confidence))
        .then_with(|| left.label.cmp(&right.label))
        .then_with(|| left.id.cmp(&right.id))
}

fn compare_fields(left: &SemanticField, right: &SemanticField) -> Ordering {
    status_rank(left.status)
        .cmp(&status_rank(right.status))
        .then_with(|| right.non_empty_count.cmp(&left.non_empty_count))
        .then_with(|| compare_confidence(right.confidence, left.confidence))
        .then_with(|| left.label.cmp(&right.label))
        .then_with(|| left.id.cmp(&right.id))
}

fn compare_relations(left: &SemanticRelation, right: &SemanticRelation) -> Ordering {
    evidence_rank(left.evidence_class)
        .cmp(&evidence_rank(right.evidence_class))
        .then_with(|| compare_confidence(right.confidence, left.confidence))
        .then_with(|| left.source_id.cmp(&right.source_id))
        .then_with(|| left.target_id.cmp(&right.target_id))
        .then_with(|| left.id.cmp(&right.id))
}

fn status_rank(status: SemanticStatus) -> u8 {
    match status {
        SemanticStatus::Confirmed => 0,
        SemanticStatus::Observed => 1,
        SemanticStatus::Inferred => 2,
        SemanticStatus::Unresolved => 3,
    }
}

fn evidence_rank(class: SemanticEvidenceClass) -> u8 {
    match class {
        SemanticEvidenceClass::Confirmed => 0,
        SemanticEvidenceClass::Observed => 1,
        SemanticEvidenceClass::Inferred => 2,
    }
}

fn compare_confidence(left: f64, right: f64) -> Ordering {
    left.partial_cmp(&right).unwrap_or(Ordering::Equal)
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use domain_model::DatasetId;

    use super::*;

    fn evidence(index: usize) -> SemanticEvidenceRef {
        SemanticEvidenceRef {
            source_kind: "document_chunk".to_string(),
            source_id: format!("chunk-{index}"),
            label: format!("证据 {index}"),
        }
    }

    fn object(kind: &str, index: usize, status: SemanticStatus) -> SemanticObject {
        SemanticObject {
            id: stable_semantic_id("object", &[kind, &index.to_string()]),
            kind: kind.to_string(),
            label: format!("业务对象 {index}"),
            technical_name: format!("technical_{index}"),
            description: "来自结构化解析证据".to_string(),
            label_source: "source_metadata".to_string(),
            confidence: 0.9,
            coverage_count: 10,
            status,
            evidence_refs: (0..8).map(evidence).collect(),
        }
    }

    fn contract(
        objects: Vec<SemanticObject>,
        relations: Vec<SemanticRelation>,
    ) -> DatasetSemanticUnderstanding {
        DatasetSemanticUnderstanding {
            schema_version: DATASET_SEMANTIC_SCHEMA_VERSION.to_string(),
            generation_version: DATASET_SEMANTIC_GENERATION_VERSION.to_string(),
            status: "ready".to_string(),
            dataset: DatasetSemanticIdentity {
                id: DatasetId(uuid::Uuid::from_u128(1)),
                title: "通用语义测试".to_string(),
            },
            coverage: SemanticCoverage::default(),
            summary: SemanticSummary {
                headline: "系统识别到多个可解释业务对象。".to_string(),
                limitations: Vec::new(),
            },
            objects,
            fields: Vec::new(),
            relations,
            source_groups: Vec::new(),
            pipeline: Vec::new(),
            generated_at: Utc
                .with_ymd_and_hms(2026, 7, 13, 12, 0, 0)
                .single()
                .expect("fixed time"),
            stale: false,
            truncated: SemanticTruncation::default(),
        }
    }

    #[test]
    fn semantic_understanding_contract_supports_all_source_kinds_and_bounds_evidence() {
        let source_kinds = [
            "database_table",
            "spreadsheet_table",
            "document_section",
            "asset_profile",
            "media_segment",
            "api_resource",
        ];
        let model = normalize_semantic_understanding(contract(
            source_kinds
                .iter()
                .enumerate()
                .map(|(index, kind)| object(kind, index, SemanticStatus::Observed))
                .collect(),
            Vec::new(),
        ));

        assert_eq!(model.objects.len(), source_kinds.len());
        assert!(source_kinds
            .iter()
            .all(|kind| model.objects.iter().any(|object| object.kind == *kind)));
        assert!(model.objects.iter().all(|object| {
            !object.label_source.is_empty()
                && object.confidence > 0.0
                && object.evidence_refs.len() <= MAX_EVIDENCE_REFS
        }));
    }

    #[test]
    fn semantic_understanding_normalization_is_deterministic_and_bounded() {
        let mut objects = (0..(MAX_OBJECTS + 8))
            .map(|index| object("database_table", index, SemanticStatus::Observed))
            .collect::<Vec<_>>();
        objects.push(objects[0].clone());
        let mut reversed = objects.clone();
        reversed.reverse();

        let left = normalize_semantic_understanding(contract(objects, Vec::new()));
        let right = normalize_semantic_understanding(contract(reversed, Vec::new()));

        assert_eq!(left.objects, right.objects);
        assert_eq!(left.objects.len(), MAX_OBJECTS);
        assert_eq!(left.truncated.objects, 8);
        assert_eq!(
            serde_json::to_value(left).unwrap(),
            serde_json::to_value(right).unwrap()
        );
    }

    #[test]
    fn semantic_summary_headline_excludes_unresolved_technical_identifiers() {
        let objects = vec![
            SemanticObject {
                label: "租赁合同".to_string(),
                ..object("database_table", 1, SemanticStatus::Confirmed)
            },
            SemanticObject {
                label: "cardparentname".to_string(),
                ..object("database_table", 2, SemanticStatus::Unresolved)
            },
        ];

        let headline = build_semantic_summary_headline(&objects, 2);

        assert!(headline.contains("租赁合同"));
        assert!(!headline.contains("cardparentname"));
        assert!(!headline.contains("technical_2"));
    }

    #[test]
    fn semantic_relationship_contract_keeps_confirmed_observed_and_inferred_distinct() {
        let objects = vec![
            object("database_table", 1, SemanticStatus::Confirmed),
            object("database_table", 2, SemanticStatus::Observed),
        ];
        let source_id = objects[0].id.clone();
        let target_id = objects[1].id.clone();
        let relation = |index, evidence_class| SemanticRelation {
            id: format!("relation-{index}"),
            source_id: source_id.clone(),
            target_id: target_id.clone(),
            relation_type: "shared_key".to_string(),
            label: format!("关系 {index}"),
            evidence_class,
            confidence: 0.8,
            reason: "fixture evidence".to_string(),
            evidence_refs: vec![evidence(index)],
        };
        let model = normalize_semantic_understanding(contract(
            objects,
            vec![
                relation(1, SemanticEvidenceClass::Confirmed),
                relation(2, SemanticEvidenceClass::Observed),
                relation(3, SemanticEvidenceClass::Inferred),
            ],
        ));

        assert_eq!(model.relations.len(), 3);
        assert!(model
            .relations
            .iter()
            .any(|relation| relation.evidence_class == SemanticEvidenceClass::Confirmed));
        assert!(model
            .relations
            .iter()
            .any(|relation| relation.evidence_class == SemanticEvidenceClass::Observed));
        assert!(model
            .relations
            .iter()
            .any(|relation| relation.evidence_class == SemanticEvidenceClass::Inferred));
    }
}
