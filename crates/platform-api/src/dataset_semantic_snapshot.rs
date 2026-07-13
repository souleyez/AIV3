use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};

use crate::semantic_label_resolver::{resolve_semantic_label, LabelCandidate, SemanticLabelInput};
use crate::semantic_relation_builder::{
    build_semantic_relations, ExplicitRelationEvidence, ExplicitRelationKind, RelationFieldProfile,
};
use crate::semantic_understanding::{
    build_semantic_summary_headline, normalize_semantic_understanding, stable_semantic_id,
    DatasetSemanticIdentity, DatasetSemanticUnderstanding, SemanticCoverage, SemanticField,
    SemanticObject, SemanticObservation, SemanticPipelineStage, SemanticSourceGroup,
    SemanticStatus, SemanticSummary, SemanticTruncation, DATASET_SEMANTIC_GENERATION_VERSION,
    DATASET_SEMANTIC_SCHEMA_VERSION,
};

#[derive(Clone, Debug)]
pub struct SnapshotDictionaryEntry {
    pub source_kind: String,
    pub source_object_key: String,
    pub raw_field_key: String,
    pub candidate: LabelCandidate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DatasetFactSnapshotRef {
    pub snapshot_kind: String,
    pub snapshot_key: String,
    pub source_fact_count: u64,
}

#[derive(Clone, Debug)]
pub struct DatasetSemanticSnapshotBuildInput {
    pub dataset: DatasetSemanticIdentity,
    pub source_fingerprint: String,
    pub generated_at: DateTime<Utc>,
    pub coverage: SemanticCoverage,
    pub observations: Vec<SemanticObservation>,
    pub dictionary_entries: Vec<SnapshotDictionaryEntry>,
    pub explicit_relations: Vec<ExplicitRelationEvidence>,
    pub fact_snapshot_refs: Vec<DatasetFactSnapshotRef>,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadySemanticSnapshotIdentity {
    pub source_fingerprint: String,
    pub generation_version: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticSnapshotBuildAction {
    UseExisting,
    Rebuild,
}

pub fn semantic_snapshot_build_action(
    existing: Option<&ReadySemanticSnapshotIdentity>,
    source_fingerprint: &str,
    generation_version: &str,
) -> SemanticSnapshotBuildAction {
    match existing {
        Some(existing)
            if existing.source_fingerprint == source_fingerprint
                && existing.generation_version == generation_version =>
        {
            SemanticSnapshotBuildAction::UseExisting
        }
        _ => SemanticSnapshotBuildAction::Rebuild,
    }
}

pub fn build_dataset_semantic_snapshot(
    input: &DatasetSemanticSnapshotBuildInput,
) -> DatasetSemanticUnderstanding {
    let mut observations = input.observations.clone();
    observations.sort_by(|left, right| left.id.cmp(&right.id));
    observations.dedup_by(|left, right| left.id == right.id);

    let dictionary = input
        .dictionary_entries
        .iter()
        .map(|entry| {
            (
                (
                    entry.source_kind.trim().to_ascii_lowercase(),
                    entry.source_object_key.trim().to_ascii_lowercase(),
                    entry.raw_field_key.trim().to_ascii_lowercase(),
                ),
                entry.candidate.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();

    let mut object_observations = BTreeMap::<String, Vec<&SemanticObservation>>::new();
    for observation in &observations {
        let key = object_group_key(observation);
        object_observations
            .entry(key)
            .or_default()
            .push(observation);
    }

    let mut objects = Vec::new();
    let mut object_ids = BTreeMap::new();
    for (group_key, group) in &object_observations {
        let representative = group
            .iter()
            .copied()
            .find(|item| item.observation_kind == "object")
            .unwrap_or(group[0]);
        let object_id = stable_semantic_id("object", &[group_key]);
        object_ids.insert(group_key.clone(), object_id.clone());
        let label = representative
            .label_hint
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(&representative.technical_name)
            .trim()
            .to_string();
        let evidence_refs = group
            .iter()
            .flat_map(|item| item.evidence_refs.clone())
            .collect();
        objects.push(SemanticObject {
            id: object_id,
            kind: representative.object_kind.clone(),
            label,
            technical_name: representative.object_key.clone(),
            description: format!("由 {} 条可追溯结构观察归纳。", group.len()),
            label_source: representative.label_source.clone(),
            confidence: representative.confidence,
            coverage_count: group.len() as u64,
            status: representative.status,
            evidence_refs,
        });
    }

    let mut fields = Vec::new();
    let mut relation_fields = Vec::new();
    let mut containment = Vec::new();
    for observation in observations
        .iter()
        .filter(|item| matches!(item.observation_kind.as_str(), "field" | "fact"))
    {
        let group_key = object_group_key(observation);
        let Some(object_id) = object_ids.get(&group_key) else {
            continue;
        };
        let dictionary_key = (
            observation.source_kind.trim().to_ascii_lowercase(),
            observation.object_key.trim().to_ascii_lowercase(),
            observation.technical_name.trim().to_ascii_lowercase(),
        );
        let resolution = resolve_semantic_label(&SemanticLabelInput {
            raw_field_key: observation.technical_name.clone(),
            confirmed_dictionary: dictionary.get(&dictionary_key).cloned(),
            source_comment: if observation.label_source == "source_comment" {
                observation.label_hint.clone()
            } else {
                None
            },
            reviewed_template: if observation.label_source == "reviewed_template" {
                observation.label_hint.clone()
            } else {
                None
            },
            model_suggestion: if observation.status == SemanticStatus::Inferred {
                observation.label_hint.as_ref().map(|label| LabelCandidate {
                    label: label.clone(),
                    description: None,
                    status: "suggested".to_string(),
                    confidence: observation.confidence,
                })
            } else {
                None
            },
            observed_values: observation.observed_values.clone(),
            declared_value_type: observation.value_type.clone(),
        });
        let field_id = stable_semantic_id(
            "field",
            &[object_id, &observation.technical_name.to_ascii_lowercase()],
        );
        let distinct_count = resolution.examples.iter().collect::<BTreeSet<_>>().len() as u64;
        fields.push(SemanticField {
            id: field_id.clone(),
            object_id: object_id.clone(),
            label: resolution.display_name.clone(),
            technical_name: resolution.technical_name.clone(),
            semantic_role: resolution.semantic_role.as_str().to_string(),
            value_type: resolution.value_type.clone(),
            non_empty_count: observation.observed_values.len() as u64,
            distinct_count,
            examples: resolution.examples.clone(),
            status: if observation.observation_kind == "fact" {
                SemanticStatus::Confirmed
            } else {
                resolution.status
            },
            label_source: if observation.observation_kind == "fact" {
                "document_fact".to_string()
            } else {
                resolution.label_source.clone()
            },
            confidence: if observation.observation_kind == "fact" {
                1.0
            } else {
                resolution.confidence
            },
            evidence_refs: observation.evidence_refs.clone(),
        });
        relation_fields.push(RelationFieldProfile {
            id: field_id.clone(),
            object_id: object_id.clone(),
            technical_name: resolution.technical_name,
            value_type: resolution.value_type,
            semantic_role: resolution.semantic_role,
            non_empty_count: observation.observed_values.len() as u64,
            total_count: observation.observed_values.len().max(1) as u64,
            distinct_count,
            examples: resolution.examples,
            sensitive: false,
            evidence_refs: observation.evidence_refs.clone(),
        });
        containment.push(ExplicitRelationEvidence::new(
            object_id,
            &field_id,
            ExplicitRelationKind::Contains,
            "解析结构直接表明该字段或事实属于此业务对象。",
            observation.evidence_refs.clone(),
        ));
    }

    let mut explicit_relations = input.explicit_relations.clone();
    explicit_relations.extend(containment);
    let relations = build_semantic_relations(&explicit_relations, &relation_fields);

    let mut coverage = input.coverage.clone();
    coverage.unresolved_field_count = fields
        .iter()
        .filter(|field| field.status == SemanticStatus::Unresolved)
        .count() as u64;
    let mut limitations = input.limitations.clone();
    if coverage.confirmed_fact_count == 0 {
        limitations.push("当前没有已确认事实；关系仅来自结构观察或明确标注的推断。".to_string());
    }
    if input.fact_snapshot_refs.is_empty() && coverage.confirmed_fact_count > 0 {
        limitations.push("已确认事实计数存在，但尚未关联可追溯事实快照版本。".to_string());
    }

    let source_groups = build_source_groups(&observations, &object_ids);
    let pipeline = vec![
        pipeline_stage("source", "来源归属", observations.len() as u64),
        pipeline_stage("structure", "结构识别", objects.len() as u64),
        pipeline_stage("labels", "业务解释", fields.len() as u64),
        pipeline_stage("relations", "关系构建", relations.len() as u64),
        pipeline_stage(
            "facts",
            "事实引用",
            input
                .fact_snapshot_refs
                .iter()
                .map(|item| item.source_fact_count)
                .sum(),
        ),
    ];
    let headline = build_semantic_summary_headline(&objects, coverage.source_count);
    normalize_semantic_understanding(DatasetSemanticUnderstanding {
        schema_version: DATASET_SEMANTIC_SCHEMA_VERSION.to_string(),
        generation_version: DATASET_SEMANTIC_GENERATION_VERSION.to_string(),
        status: "ready".to_string(),
        dataset: input.dataset.clone(),
        coverage,
        summary: SemanticSummary {
            headline,
            limitations,
        },
        objects,
        fields,
        relations,
        source_groups,
        pipeline,
        generated_at: input.generated_at,
        stale: false,
        truncated: SemanticTruncation::default(),
    })
}

pub fn semantic_snapshot_failure_fallback(
    latest_ready: Option<DatasetSemanticUnderstanding>,
    failure_code: &str,
) -> Option<DatasetSemanticUnderstanding> {
    let mut snapshot = latest_ready?;
    snapshot.stale = true;
    let safe_code = failure_code
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
        .take(64)
        .collect::<String>();
    snapshot.summary.limitations.push(format!(
        "最新重建失败，当前展示上一版结果（失败码：{}）。",
        if safe_code.is_empty() {
            "semantic_build_failed"
        } else {
            &safe_code
        }
    ));
    Some(snapshot)
}

fn object_group_key(observation: &SemanticObservation) -> String {
    format!(
        "{}|{}|{}",
        observation.object_kind, observation.source_id, observation.object_key
    )
}

fn build_source_groups(
    observations: &[SemanticObservation],
    object_ids: &BTreeMap<String, String>,
) -> Vec<SemanticSourceGroup> {
    let mut groups = BTreeMap::<String, BTreeSet<String>>::new();
    for observation in observations {
        if let Some(object_id) = object_ids.get(&object_group_key(observation)) {
            groups
                .entry(observation.source_kind.clone())
                .or_default()
                .insert(object_id.clone());
        }
    }
    groups
        .into_iter()
        .map(|(kind, object_ids)| SemanticSourceGroup {
            id: stable_semantic_id("source_group", &[&kind]),
            label: kind.clone(),
            kind,
            coverage_count: object_ids.len() as u64,
            object_ids: object_ids.into_iter().collect(),
        })
        .collect()
}

fn pipeline_stage(id: &str, label: &str, evidence_count: u64) -> SemanticPipelineStage {
    SemanticPipelineStage {
        id: id.to_string(),
        label: label.to_string(),
        status: "completed".to_string(),
        detail: format!("处理 {evidence_count} 条可追溯信号。"),
        evidence_count,
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use domain_model::{DatasetId, DocumentId};
    use serde_json::json;
    use uuid::Uuid;

    use super::*;
    use crate::semantic_profile_adapters::{adapt_semantic_profile, SemanticProfileInput};
    use crate::semantic_understanding::{
        DatasetSemanticIdentity, SemanticCoverage, SemanticStatus,
        DATASET_SEMANTIC_GENERATION_VERSION,
    };

    fn fixture_input() -> DatasetSemanticSnapshotBuildInput {
        let source = SemanticProfileInput {
            source_id: "db:lease_contract".to_string(),
            source_kind: "database".to_string(),
            title: "租赁合同".to_string(),
            metadata: json!({"parse_metadata": {
                "source_table": "lease_contract",
                "fields": {"contract_id": "C-001", "rent_amount": 1200},
                "field_comments": {"contract_id": "合同编号", "rent_amount": "租金金额"}
            }}),
            facts: Vec::new(),
            evidence_labels: vec!["结构解析".to_string()],
        };
        DatasetSemanticSnapshotBuildInput {
            dataset: DatasetSemanticIdentity {
                id: DatasetId(Uuid::from_u128(1)),
                title: "经营分析".to_string(),
            },
            source_fingerprint: "a".repeat(64),
            generated_at: Utc
                .with_ymd_and_hms(2026, 7, 13, 21, 0, 0)
                .single()
                .expect("fixed time"),
            coverage: SemanticCoverage {
                source_count: 1,
                document_count: 486,
                record_count: 485,
                retrieval_evidence_count: 485,
                confirmed_fact_count: 7,
                ..SemanticCoverage::default()
            },
            observations: adapt_semantic_profile(&source),
            dictionary_entries: Vec::new(),
            explicit_relations: Vec::new(),
            fact_snapshot_refs: vec![DatasetFactSnapshotRef {
                snapshot_kind: "answer_facts".to_string(),
                snapshot_key: "v7".to_string(),
                source_fact_count: 7,
            }],
            limitations: Vec::new(),
        }
    }

    #[test]
    fn snapshot_builder_is_deterministic_and_uses_authoritative_fact_coverage() {
        let input = fixture_input();
        let mut reversed = input.clone();
        reversed.observations.reverse();

        let left = build_dataset_semantic_snapshot(&input);
        let right = build_dataset_semantic_snapshot(&reversed);

        assert_eq!(
            serde_json::to_value(&left).unwrap(),
            serde_json::to_value(&right).unwrap()
        );
        assert_eq!(left.coverage.confirmed_fact_count, 7);
        assert_eq!(left.coverage.record_count, 485);
        assert!(left.summary.headline.contains("租赁合同"));
        assert!(!left.summary.headline.contains("lease_contract"));
        assert!(!left.objects.is_empty());
        assert!(!left.fields.is_empty());
        assert!(!left.relations.is_empty());
    }

    #[test]
    fn snapshot_builder_does_not_promote_unresolved_english_identifier_to_headline() {
        let mut input = fixture_input();
        for observation in &mut input.observations {
            observation.label_hint = Some("cardparentname".to_string());
            observation.status = SemanticStatus::Unresolved;
        }

        let snapshot = build_dataset_semantic_snapshot(&input);

        assert!(!snapshot.summary.headline.contains("cardparentname"));
        assert!(snapshot.summary.headline.contains("1 类来源"));
    }

    #[test]
    fn same_fingerprint_and_generation_version_skip_rebuild_but_version_changes_rebuild() {
        let existing = ReadySemanticSnapshotIdentity {
            source_fingerprint: "a".repeat(64),
            generation_version: DATASET_SEMANTIC_GENERATION_VERSION.to_string(),
        };

        assert_eq!(
            semantic_snapshot_build_action(
                Some(&existing),
                &"a".repeat(64),
                DATASET_SEMANTIC_GENERATION_VERSION
            ),
            SemanticSnapshotBuildAction::UseExisting
        );
        assert_eq!(
            semantic_snapshot_build_action(Some(&existing), &"a".repeat(64), "semantic_profile_v2"),
            SemanticSnapshotBuildAction::Rebuild
        );
    }

    #[test]
    fn failed_build_returns_previous_ready_as_stale_without_mutating_it() {
        let ready = build_dataset_semantic_snapshot(&fixture_input());
        let fallback =
            semantic_snapshot_failure_fallback(Some(ready.clone()), "source_parse_failed")
                .expect("ready fallback");

        assert!(fallback.stale);
        assert_eq!(fallback.status, "ready");
        assert_eq!(fallback.dataset, ready.dataset);
        assert!(fallback
            .summary
            .limitations
            .iter()
            .any(|item| item.contains("source_parse_failed")));
    }

    #[test]
    fn fingerprint_input_references_versions_and_ids_without_document_content() {
        let source = crate::dataset_semantic_source_support::SemanticSourceFingerprintInput {
            documents: vec![
                crate::dataset_semantic_source_support::SemanticDocumentSourceVersion {
                    document_id: DocumentId(Uuid::from_u128(2)),
                    updated_at: fixture_input().generated_at,
                    parse_versions: vec!["parser-v1".to_string()],
                    fact_snapshot_versions: vec!["facts-v7".to_string()],
                },
            ],
            assets: Vec::new(),
            dataset_fact_snapshot_version: Some("v7".to_string()),
        };
        assert_eq!(
            crate::dataset_semantic_source_support::source_fingerprint(&source).len(),
            64
        );
    }
}
