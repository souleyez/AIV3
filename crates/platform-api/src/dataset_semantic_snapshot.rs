use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use domain_model::{DatasetId, DocumentId, TenantId};
use serde_json::{json, Map, Value};
use storage::{NewDatasetSemanticSnapshot, PgStorage};

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

#[derive(Clone, Debug)]
pub struct DatasetSemanticRebuildOutcome {
    pub status: String,
    pub source_fingerprint: String,
    pub snapshot: Option<DatasetSemanticUnderstanding>,
    pub failure_code: Option<String>,
}

pub async fn rebuild_dataset_semantic_snapshot_from_storage(
    storage: &PgStorage,
    tenant_id: TenantId,
    dataset_id: DatasetId,
    generated_at: DateTime<Utc>,
) -> Result<DatasetSemanticRebuildOutcome> {
    let dataset = storage
        .datasets()
        .get_by_id(tenant_id, dataset_id)
        .await?
        .ok_or_else(|| anyhow!("dataset {dataset_id} not found"))?;
    let documents = storage
        .documents()
        .list_by_dataset_scope_bounded(tenant_id, dataset_id, 10_000)
        .await?;
    let document_ids = documents.iter().map(|item| item.id).collect::<Vec<_>>();
    let chunks = storage
        .document_chunks()
        .list_by_documents_bounded(tenant_id, &document_ids, 50_000)
        .await?;
    let facts = storage
        .document_facts()
        .list_by_document_ids_bounded(tenant_id, &document_ids, 50_000)
        .await?;
    let retrieval_evidences = storage
        .retrieval_evidences()
        .list_latest_by_document_ids(tenant_id, &document_ids, 50_000)
        .await?;
    let assets = storage
        .asset_items()
        .list_by_dataset_scope(tenant_id, dataset_id, 10_000)
        .await?;
    let asset_ids = assets.iter().map(|asset| asset.id).collect::<Vec<_>>();
    let profiles = storage
        .asset_items()
        .list_accepted_profiles_by_asset_ids(tenant_id, &asset_ids, 20_000)
        .await?;
    let dictionary = storage
        .semantic_dictionary_entries()
        .list_for_resolution(tenant_id, 10_000)
        .await?;
    let fact_snapshot = storage
        .dataset_fact_snapshots()
        .load_latest_dataset_fact_snapshot(tenant_id, dataset_id, "entity_rows_by_type")
        .await?;

    let chunks_by_document = chunks.iter().fold(
        BTreeMap::<DocumentId, Vec<&domain_model::DocumentChunk>>::new(),
        |mut grouped, chunk| {
            grouped.entry(chunk.document_id).or_default().push(chunk);
            grouped
        },
    );
    let facts_by_document = facts.iter().fold(
        BTreeMap::<DocumentId, Vec<&storage::DocumentFact>>::new(),
        |mut grouped, fact| {
            grouped.entry(fact.document_id).or_default().push(fact);
            grouped
        },
    );
    let mut observations = Vec::new();
    let mut source_keys = BTreeSet::new();
    let mut document_versions = Vec::new();
    let mut record_count = 0u64;
    for document in &documents {
        let document_chunks = chunks_by_document
            .get(&document.id)
            .cloned()
            .unwrap_or_default();
        let parse_versions = document_chunks
            .iter()
            .filter_map(|chunk| metadata_string(&chunk.metadata, "parse_version"))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let fact_versions = facts_by_document
            .get(&document.id)
            .into_iter()
            .flatten()
            .filter_map(|fact| fact.parse_version.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        document_versions.push(
            crate::dataset_semantic_source_support::SemanticDocumentSourceVersion {
                document_id: document.id,
                updated_at: document.updated_at,
                parse_versions,
                fact_snapshot_versions: fact_versions,
            },
        );
        let source_kind = semantic_document_source_kind(document, &document_chunks);
        let source_metadata = document_chunks
            .first()
            .map(|chunk| map_to_value(&chunk.metadata))
            .unwrap_or_else(|| map_to_value(&document.metadata));
        let source_key = source_metadata
            .pointer("/parse_metadata/source_table")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| document.id.to_string());
        if source_kind == "database" {
            record_count += 1;
        }
        source_keys.insert(format!("{source_kind}:{source_key}"));
        let fact_values = facts_by_document
            .get(&document.id)
            .into_iter()
            .flatten()
            .map(|fact| {
                json!({
                    "name": fact.name,
                    "fact_type": fact.fact_type,
                    "value_type": document_fact_value_type(fact),
                    "parse_version": fact.parse_version,
                })
            })
            .collect();
        observations.extend(crate::semantic_profile_adapters::adapt_semantic_profile(
            &crate::semantic_profile_adapters::SemanticProfileInput {
                source_id: document.id.to_string(),
                source_kind,
                title: document.title.clone(),
                metadata: source_metadata,
                facts: fact_values,
                evidence_labels: document_chunks
                    .iter()
                    .take(5)
                    .map(|chunk| format!("document_chunk:{}", chunk.id))
                    .collect(),
            },
        ));
    }

    let profiles_by_asset = profiles.iter().fold(
        BTreeMap::<uuid::Uuid, Vec<&storage::AssetProfileRecord>>::new(),
        |mut grouped, profile| {
            grouped.entry(profile.asset_id).or_default().push(profile);
            grouped
        },
    );
    let mut asset_versions = Vec::new();
    for asset in &assets {
        let asset_profiles = profiles_by_asset
            .get(&asset.id)
            .cloned()
            .unwrap_or_default();
        asset_versions.push(
            crate::dataset_semantic_source_support::SemanticAssetSourceVersion {
                asset_id: asset.id,
                updated_at: asset.updated_at,
                profile_versions: asset_profiles
                    .iter()
                    .map(|profile| profile.profile_version.clone())
                    .collect(),
            },
        );
        source_keys.insert(format!("asset:{}", asset.id));
        for profile in asset_profiles {
            observations.extend(crate::semantic_profile_adapters::adapt_semantic_profile(
                &crate::semantic_profile_adapters::SemanticProfileInput {
                    source_id: asset.id.to_string(),
                    source_kind: "asset".to_string(),
                    title: asset.title.clone(),
                    metadata: json!({
                        "profile_kind": profile.profile_kind,
                        "safe_profile": profile.attributes,
                        "collection": asset.metadata.get("collection_name"),
                    }),
                    facts: Vec::new(),
                    evidence_labels: vec![format!("asset_profile:{}", profile.id)],
                },
            ));
        }
    }

    let fact_snapshot_version = fact_snapshot.as_ref().map(|item| item.snapshot_key.clone());
    let source_fingerprint = crate::dataset_semantic_source_support::source_fingerprint(
        &crate::dataset_semantic_source_support::SemanticSourceFingerprintInput {
            documents: document_versions,
            assets: asset_versions,
            dataset_fact_snapshot_version: fact_snapshot_version.clone(),
        },
    );
    let latest_ready = storage
        .dataset_semantic_snapshots()
        .load_latest_ready(tenant_id, dataset_id)
        .await?;
    if latest_ready.as_ref().is_some_and(|snapshot| {
        snapshot.source_fingerprint == source_fingerprint
            && snapshot.generation_version == DATASET_SEMANTIC_GENERATION_VERSION
    }) {
        let snapshot = latest_ready.and_then(|item| serde_json::from_value(item.manifest).ok());
        return Ok(DatasetSemanticRebuildOutcome {
            status: "skipped".to_string(),
            source_fingerprint,
            snapshot,
            failure_code: None,
        });
    }

    let new_snapshot = NewDatasetSemanticSnapshot {
        dataset_id,
        schema_version: DATASET_SEMANTIC_SCHEMA_VERSION.to_string(),
        generation_version: DATASET_SEMANTIC_GENERATION_VERSION.to_string(),
        source_fingerprint: source_fingerprint.clone(),
        manifest: json!({}),
        source_document_count: documents.len() as i64,
        source_asset_count: assets.len() as i64,
        source_record_count: record_count as i64,
    };
    let Some(build_record) = storage
        .dataset_semantic_snapshots()
        .try_begin_build(tenant_id, &new_snapshot, generated_at)
        .await?
    else {
        let snapshot = latest_ready.and_then(|item| serde_json::from_value(item.manifest).ok());
        return Ok(DatasetSemanticRebuildOutcome {
            status: "skipped".to_string(),
            source_fingerprint,
            snapshot,
            failure_code: Some("build_in_progress_or_complete".to_string()),
        });
    };

    let dictionary_entries = dictionary
        .into_iter()
        .map(|entry| SnapshotDictionaryEntry {
            source_kind: entry.source_kind,
            source_object_key: entry.source_object_key,
            raw_field_key: entry.raw_field_key,
            candidate: LabelCandidate {
                label: entry.display_name,
                description: entry.description,
                status: entry.status,
                confidence: entry.confidence,
            },
        })
        .collect();
    let fact_snapshot_refs = fact_snapshot
        .map(|item| DatasetFactSnapshotRef {
            snapshot_kind: item.snapshot_kind,
            snapshot_key: item.snapshot_key,
            source_fact_count: item.source_fact_count.max(0) as u64,
        })
        .into_iter()
        .collect::<Vec<_>>();
    let semantic_snapshot = build_dataset_semantic_snapshot(&DatasetSemanticSnapshotBuildInput {
        dataset: DatasetSemanticIdentity {
            id: dataset.id,
            title: dataset.title,
        },
        source_fingerprint: source_fingerprint.clone(),
        generated_at,
        coverage: SemanticCoverage {
            source_count: source_keys.len() as u64,
            document_count: documents.len() as u64,
            record_count,
            asset_count: assets.len() as u64,
            retrieval_evidence_count: retrieval_evidences.len() as u64,
            confirmed_fact_count: facts.len() as u64,
            unresolved_field_count: 0,
        },
        observations,
        dictionary_entries,
        explicit_relations: Vec::new(),
        fact_snapshot_refs,
        limitations: Vec::new(),
    });
    let manifest = serde_json::to_value(&semantic_snapshot)?;
    let manifest_bytes = serde_json::to_vec(&manifest)?.len();
    if manifest_bytes > 2 * 1024 * 1024 {
        storage
            .dataset_semantic_snapshots()
            .mark_failed(
                tenant_id,
                dataset_id,
                build_record.id,
                "manifest_too_large",
                generated_at,
            )
            .await?;
        return Ok(DatasetSemanticRebuildOutcome {
            status: "failed".to_string(),
            source_fingerprint,
            snapshot: latest_ready
                .and_then(|item| serde_json::from_value(item.manifest).ok())
                .and_then(|item| {
                    semantic_snapshot_failure_fallback(Some(item), "manifest_too_large")
                }),
            failure_code: Some("manifest_too_large".to_string()),
        });
    }
    storage
        .dataset_semantic_snapshots()
        .mark_ready(
            tenant_id,
            dataset_id,
            build_record.id,
            &manifest,
            (semantic_snapshot.objects.len() + semantic_snapshot.fields.len()) as i32,
            semantic_snapshot.relations.len() as i32,
            generated_at,
        )
        .await?
        .ok_or_else(|| anyhow!("semantic snapshot build record is no longer claimable"))?;
    Ok(DatasetSemanticRebuildOutcome {
        status: "ready".to_string(),
        source_fingerprint,
        snapshot: Some(semantic_snapshot),
        failure_code: None,
    })
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
        let source_count = group
            .iter()
            .map(|item| item.source_id.as_str())
            .collect::<BTreeSet<_>>()
            .len() as u64;
        objects.push(SemanticObject {
            id: object_id,
            kind: representative.object_kind.clone(),
            label,
            technical_name: representative.object_key.clone(),
            description: format!("由 {source_count} 个来源记录的可追溯结构观察归纳。"),
            label_source: representative.label_source.clone(),
            confidence: representative.confidence,
            coverage_count: source_count,
            status: representative.status,
            evidence_refs,
        });
    }

    let mut fields = Vec::new();
    let mut relation_fields = Vec::new();
    let mut containment = Vec::new();
    let mut field_ids = BTreeMap::<(String, String), String>::new();
    let mut field_observations = BTreeMap::<(String, String), Vec<&SemanticObservation>>::new();
    for observation in observations
        .iter()
        .filter(|item| semantic_field_observation(item))
    {
        field_observations
            .entry((
                object_group_key(observation),
                observation.technical_name.trim().to_ascii_lowercase(),
            ))
            .or_default()
            .push(observation);
    }
    for ((group_key, _), group) in field_observations {
        let observation = group[0];
        let Some(object_id) = object_ids.get(&group_key) else {
            continue;
        };
        let dictionary_key = (
            observation.source_kind.trim().to_ascii_lowercase(),
            observation.object_key.trim().to_ascii_lowercase(),
            observation.technical_name.trim().to_ascii_lowercase(),
        );
        let observed_values = group
            .iter()
            .flat_map(|item| item.observed_values.iter().cloned())
            .collect::<Vec<_>>();
        let source_comment = group
            .iter()
            .find(|item| item.label_source == "source_comment")
            .and_then(|item| item.label_hint.clone());
        let reviewed_template = group
            .iter()
            .find(|item| item.label_source == "reviewed_template")
            .and_then(|item| item.label_hint.clone());
        let model_suggestion = group
            .iter()
            .filter(|item| item.status == SemanticStatus::Inferred)
            .max_by(|left, right| {
                left.confidence
                    .partial_cmp(&right.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| right.id.cmp(&left.id))
            })
            .and_then(|item| {
                item.label_hint.as_ref().map(|label| LabelCandidate {
                    label: label.clone(),
                    description: None,
                    status: "suggested".to_string(),
                    confidence: item.confidence,
                })
            });
        let declared_value_type = dominant_value_type(&group);
        let resolution = resolve_semantic_label(&SemanticLabelInput {
            raw_field_key: observation.technical_name.clone(),
            confirmed_dictionary: dictionary.get(&dictionary_key).cloned(),
            source_comment,
            reviewed_template,
            model_suggestion,
            observed_values: observed_values.clone(),
            declared_value_type,
        });
        let field_id = stable_semantic_id(
            "field",
            &[object_id, &observation.technical_name.to_ascii_lowercase()],
        );
        field_ids.insert(
            (
                group_key.clone(),
                observation.technical_name.trim().to_ascii_lowercase(),
            ),
            field_id.clone(),
        );
        let distinct_count = observed_values
            .iter()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .collect::<BTreeSet<_>>()
            .len() as u64;
        let non_empty_count = observed_values
            .iter()
            .filter(|value| !value.trim().is_empty())
            .count() as u64;
        let total_count = group
            .iter()
            .map(|item| item.source_id.as_str())
            .collect::<BTreeSet<_>>()
            .len() as u64;
        let is_fact = group.iter().any(|item| item.observation_kind == "fact");
        let observed_label = group.iter().find_map(|item| {
            (item.status == SemanticStatus::Observed)
                .then_some(item)
                .and_then(|item| {
                    item.label_hint
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(|label| {
                            (
                                label.to_string(),
                                SemanticStatus::Observed,
                                item.label_source.clone(),
                                item.confidence,
                            )
                        })
                })
        });
        let (display_name, status, label_source, confidence) = if is_fact {
            (
                resolution.display_name.clone(),
                SemanticStatus::Confirmed,
                "document_fact".to_string(),
                1.0,
            )
        } else if resolution.status == SemanticStatus::Unresolved {
            observed_label.unwrap_or((
                resolution.display_name.clone(),
                resolution.status,
                resolution.label_source.clone(),
                resolution.confidence,
            ))
        } else {
            (
                resolution.display_name.clone(),
                resolution.status,
                resolution.label_source.clone(),
                resolution.confidence,
            )
        };
        let evidence_refs = group
            .iter()
            .flat_map(|item| item.evidence_refs.clone())
            .collect::<Vec<_>>();
        fields.push(SemanticField {
            id: field_id.clone(),
            object_id: object_id.clone(),
            label: display_name,
            technical_name: resolution.technical_name.clone(),
            semantic_role: resolution.semantic_role.as_str().to_string(),
            value_type: resolution.value_type.clone(),
            non_empty_count,
            distinct_count,
            examples: resolution.examples.clone(),
            status,
            label_source,
            confidence,
            evidence_refs: evidence_refs.clone(),
        });
        relation_fields.push(RelationFieldProfile {
            id: field_id.clone(),
            object_id: object_id.clone(),
            technical_name: resolution.technical_name,
            value_type: resolution.value_type,
            semantic_role: resolution.semantic_role,
            non_empty_count,
            total_count: total_count.max(1),
            distinct_count,
            examples: resolution.examples,
            sensitive: false,
            evidence_refs: evidence_refs.clone(),
        });
        containment.push(ExplicitRelationEvidence::new(
            object_id,
            &field_id,
            ExplicitRelationKind::Contains,
            "解析结构直接表明该字段或事实属于此业务对象。",
            evidence_refs,
        ));
    }

    let mut explicit_relations = input.explicit_relations.clone();
    explicit_relations.extend(constraint_relations(
        &observations,
        &object_observations,
        &field_ids,
    ));
    explicit_relations.extend(containment);
    let relations = build_semantic_relations(&explicit_relations, &relation_fields);

    let mut coverage = input.coverage.clone();
    coverage.unresolved_field_count = fields
        .iter()
        .filter(|field| field.status == SemanticStatus::Unresolved)
        .count() as u64;
    let mut limitations = input.limitations.clone();
    if coverage.confirmed_fact_count == 0 {
        limitations.push("暂无已确认事实；关系仅来自结构观察或明确标注的推断。".to_string());
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
    if observation.object_kind == "database_table" {
        format!("{}|{}", observation.object_kind, observation.object_key)
    } else {
        format!(
            "{}|{}|{}",
            observation.object_kind, observation.source_id, observation.object_key
        )
    }
}

fn semantic_field_observation(observation: &SemanticObservation) -> bool {
    !matches!(
        observation.observation_kind.as_str(),
        "object" | "constraint"
    )
}

fn dominant_value_type(observations: &[&SemanticObservation]) -> Option<String> {
    let mut counts = BTreeMap::<String, usize>::new();
    for value_type in observations
        .iter()
        .filter_map(|item| item.value_type.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase)
    {
        *counts.entry(value_type).or_default() += 1;
    }
    let mut selected = None;
    for (value_type, count) in counts {
        if selected
            .as_ref()
            .is_none_or(|(_, selected_count)| count > *selected_count)
        {
            selected = Some((value_type, count));
        }
    }
    selected.map(|(value_type, _)| value_type)
}

fn constraint_relations(
    observations: &[SemanticObservation],
    object_observations: &BTreeMap<String, Vec<&SemanticObservation>>,
    field_ids: &BTreeMap<(String, String), String>,
) -> Vec<ExplicitRelationEvidence> {
    let database_groups = object_observations
        .iter()
        .filter_map(|(group_key, group)| {
            let observation = group.first()?;
            (observation.object_kind == "database_table").then_some((
                group_key.as_str(),
                observation.object_key.trim().to_ascii_lowercase(),
            ))
        })
        .collect::<Vec<_>>();

    observations
        .iter()
        .filter(|item| item.observation_kind == "constraint")
        .filter_map(|constraint| {
            let reference = constraint
                .attributes
                .get("references")
                .and_then(Value::as_str)
                .or(constraint.label_hint.as_deref())?
                .trim();
            let (target_object_key, target_field_key) = reference.rsplit_once('.')?;
            let target_table = target_object_key
                .rsplit('.')
                .next()
                .unwrap_or(target_object_key)
                .trim()
                .to_ascii_lowercase();
            let target_group = database_groups
                .iter()
                .find(|(_, object_key)| {
                    object_key.as_str() == target_object_key.trim().to_ascii_lowercase()
                })
                .or_else(|| {
                    database_groups
                        .iter()
                        .find(|(_, object_key)| object_key.as_str() == target_table)
                })?
                .0;
            let source_group = object_group_key(constraint);
            let source_id = field_ids.get(&(
                source_group,
                constraint.technical_name.trim().to_ascii_lowercase(),
            ))?;
            let target_id = field_ids.get(&(
                target_group.to_string(),
                target_field_key.trim().to_ascii_lowercase(),
            ))?;
            Some(ExplicitRelationEvidence::new(
                source_id,
                target_id,
                ExplicitRelationKind::ForeignKey,
                "源数据库外键约束明确关联这两个字段。",
                constraint.evidence_refs.clone(),
            ))
        })
        .collect()
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

fn map_to_value(values: &BTreeMap<String, Value>) -> Value {
    Value::Object(
        values
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<Map<_, _>>(),
    )
}

fn metadata_string(values: &BTreeMap<String, Value>, key: &str) -> Option<String> {
    values
        .get(key)
        .or_else(|| {
            values
                .get("parse_metadata")
                .and_then(|value| value.get(key))
        })
        .or_else(|| values.get("ingest").and_then(|value| value.get(key)))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn semantic_document_source_kind(
    document: &domain_model::Document,
    chunks: &[&domain_model::DocumentChunk],
) -> String {
    if chunks.iter().any(|chunk| {
        chunk
            .metadata
            .get("parse_metadata")
            .and_then(|value| value.get("source_table"))
            .and_then(Value::as_str)
            .is_some()
    }) {
        return "database".to_string();
    }
    let content_type = document.content_type.to_ascii_lowercase();
    if content_type.contains("spreadsheet")
        || content_type.contains("excel")
        || content_type.contains("csv")
    {
        "spreadsheet".to_string()
    } else if content_type.starts_with("audio/")
        || content_type.starts_with("video/")
        || content_type.contains("presentation")
        || content_type.contains("powerpoint")
    {
        "media".to_string()
    } else if content_type.contains("html") || content_type.contains("json") {
        "web_api".to_string()
    } else {
        "document".to_string()
    }
}

fn document_fact_value_type(fact: &storage::DocumentFact) -> &'static str {
    if fact.value_number.is_some() {
        "number"
    } else if fact.value_date.is_some() {
        "date"
    } else if fact.value_text.is_some() {
        "text"
    } else {
        "unknown"
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
        assert_eq!(
            left.pipeline
                .iter()
                .map(|stage| stage.id.as_str())
                .collect::<Vec<_>>(),
            vec!["source", "structure", "labels", "relations", "facts"]
        );
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
    fn database_rows_collapse_into_one_business_object_and_aggregate_field_coverage() {
        let mut input = fixture_input();
        input.observations = [
            (
                "row-1",
                json!({"contract_id": "C-001", "rent_amount": 1200}),
            ),
            (
                "row-2",
                json!({"contract_id": "C-002", "rent_amount": 1800}),
            ),
            ("row-3", json!({"contract_id": null, "rent_amount": 1800})),
        ]
        .into_iter()
        .flat_map(|(source_id, fields)| {
            adapt_semantic_profile(&SemanticProfileInput {
                source_id: source_id.to_string(),
                source_kind: "database".to_string(),
                title: "租赁合同".to_string(),
                metadata: json!({"parse_metadata": {
                    "source_table": "lease_contract",
                    "fields": fields,
                    "field_comments": {
                        "contract_id": "合同编号",
                        "rent_amount": "租金金额"
                    }
                }}),
                facts: Vec::new(),
                evidence_labels: vec![format!("结构解析:{source_id}")],
            })
        })
        .collect();

        let snapshot = build_dataset_semantic_snapshot(&input);
        let contract_id = snapshot
            .fields
            .iter()
            .find(|field| field.technical_name == "contract_id")
            .expect("aggregated contract id field");
        let rent_amount = snapshot
            .fields
            .iter()
            .find(|field| field.technical_name == "rent_amount")
            .expect("aggregated rent field");

        assert_eq!(snapshot.objects.len(), 1);
        assert_eq!(snapshot.objects[0].coverage_count, 3);
        assert_eq!(snapshot.fields.len(), 2);
        assert_eq!(contract_id.non_empty_count, 2);
        assert_eq!(contract_id.distinct_count, 2);
        assert_eq!(contract_id.examples, vec!["C-001", "C-002"]);
        assert_eq!(rent_amount.non_empty_count, 3);
        assert_eq!(rent_amount.distinct_count, 2);
        assert_eq!(snapshot.relations.len(), 2);
    }

    #[test]
    fn database_foreign_key_constraint_becomes_a_confirmed_field_relation() {
        let mut input = fixture_input();
        let contract = SemanticProfileInput {
            source_id: "db:lease_contract".to_string(),
            source_kind: "database".to_string(),
            title: "租赁合同".to_string(),
            metadata: json!({"parse_metadata": {
                "source_table": "lease_contract",
                "fields": {"contract_id": "C-001", "store_id": "S-01"},
                "field_comments": {"contract_id": "合同编号", "store_id": "门店编号"},
                "foreign_keys": [{"field": "store_id", "references": "store.id"}]
            }}),
            facts: Vec::new(),
            evidence_labels: vec!["租赁合同约束".to_string()],
        };
        let store = SemanticProfileInput {
            source_id: "db:store".to_string(),
            source_kind: "database".to_string(),
            title: "门店".to_string(),
            metadata: json!({"parse_metadata": {
                "source_table": "store",
                "fields": {"id": "S-01", "name": "中环店"},
                "field_comments": {"id": "门店编号", "name": "门店名称"}
            }}),
            facts: Vec::new(),
            evidence_labels: vec!["门店结构".to_string()],
        };
        input.observations = adapt_semantic_profile(&contract)
            .into_iter()
            .chain(adapt_semantic_profile(&store))
            .collect();

        let snapshot = build_dataset_semantic_snapshot(&input);
        let relation = snapshot
            .relations
            .iter()
            .find(|relation| relation.relation_type == "foreign_key")
            .expect("confirmed foreign-key relation");
        let source = snapshot
            .fields
            .iter()
            .find(|field| field.id == relation.source_id)
            .expect("source field");
        let target = snapshot
            .fields
            .iter()
            .find(|field| field.id == relation.target_id)
            .expect("target field");

        assert_eq!(
            relation.evidence_class,
            crate::semantic_understanding::SemanticEvidenceClass::Confirmed
        );
        assert_eq!(relation.confidence, 1.0);
        assert_eq!(source.technical_name, "store_id");
        assert_eq!(target.technical_name, "id");
    }

    #[test]
    fn no_credential_smoke_all_source_kinds_share_one_business_safe_contract() {
        let sources = vec![
            SemanticProfileInput {
                source_id: "db:contract".to_string(),
                source_kind: "database".to_string(),
                title: "租赁合同".to_string(),
                metadata: json!({"parse_metadata": {
                    "source_table": "lease_contract",
                    "fields": {"rent_amount": 1200},
                    "field_comments": {"rent_amount": "租金金额"}
                }}),
                facts: Vec::new(),
                evidence_labels: vec!["数据库结构".to_string()],
            },
            SemanticProfileInput {
                source_id: "sheet:sales".to_string(),
                source_kind: "spreadsheet".to_string(),
                title: "销售经营表".to_string(),
                metadata: json!({
                    "sheet_name": "销售经营表",
                    "headers": ["交易日期", "销售金额", "品类"]
                }),
                facts: Vec::new(),
                evidence_labels: vec!["表头结构".to_string()],
            },
            SemanticProfileInput {
                source_id: "doc:policy".to_string(),
                source_kind: "document".to_string(),
                title: "企业问答制度".to_string(),
                metadata: json!({"sections": ["适用范围"], "entities": ["审批负责人"]}),
                facts: Vec::new(),
                evidence_labels: vec!["文档结构".to_string()],
            },
            SemanticProfileInput {
                source_id: "asset:product".to_string(),
                source_kind: "asset".to_string(),
                title: "商品视觉".to_string(),
                metadata: json!({
                    "profile_kind": "product_visual",
                    "safe_profile": {"labels": ["主色调", "版型特征"]}
                }),
                facts: Vec::new(),
                evidence_labels: vec!["资产画像".to_string()],
            },
            SemanticProfileInput {
                source_id: "media:training".to_string(),
                source_kind: "media".to_string(),
                title: "培训视频".to_string(),
                metadata: json!({"pages": [{"title": "服务流程"}]}),
                facts: Vec::new(),
                evidence_labels: vec!["媒体分段".to_string()],
            },
            SemanticProfileInput {
                source_id: "api:store".to_string(),
                source_kind: "web_api".to_string(),
                title: "门店接口".to_string(),
                metadata: json!({
                    "resource_type": "store_resource",
                    "title": "门店资源",
                    "fields": ["store_name"]
                }),
                facts: Vec::new(),
                evidence_labels: vec!["接口结构".to_string()],
            },
        ];

        for source in sources {
            let mut input = fixture_input();
            input.coverage.confirmed_fact_count = 0;
            input.fact_snapshot_refs.clear();
            input.observations = adapt_semantic_profile(&source);
            let snapshot = build_dataset_semantic_snapshot(&input);

            assert_eq!(snapshot.schema_version, DATASET_SEMANTIC_SCHEMA_VERSION);
            assert_eq!(snapshot.status, "ready");
            assert!(
                !snapshot.objects.is_empty(),
                "{} object",
                source.source_kind
            );
            assert!(!snapshot.fields.is_empty(), "{} field", source.source_kind);
            if source.source_kind == "database" {
                assert_eq!(snapshot.objects[0].label, "租赁合同");
                assert!(!snapshot.objects[0].label.contains("rent_amount"));
            }
            assert!(snapshot
                .summary
                .limitations
                .iter()
                .any(|item| item.contains("暂无已确认事实")));
        }
    }

    #[test]
    fn empty_unknown_fixture_keeps_technical_field_unresolved_and_out_of_headline() {
        let mut input = fixture_input();
        input.coverage.confirmed_fact_count = 0;
        input.fact_snapshot_refs.clear();
        input.observations = adapt_semantic_profile(&SemanticProfileInput {
            source_id: "db:unknown".to_string(),
            source_kind: "database".to_string(),
            title: "未知来源".to_string(),
            metadata: json!({"parse_metadata": {
                "source_table": "TABLE_X",
                "fields": {"cardparentname": "opaque"}
            }}),
            facts: Vec::new(),
            evidence_labels: vec!["隔离结构".to_string()],
        });

        let snapshot = build_dataset_semantic_snapshot(&input);

        assert_eq!(snapshot.fields.len(), 1);
        assert_eq!(snapshot.fields[0].status, SemanticStatus::Unresolved);
        assert!(!snapshot.summary.headline.contains("cardparentname"));
        assert!(snapshot
            .summary
            .limitations
            .iter()
            .any(|item| item.contains("暂无已确认事实")));
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
