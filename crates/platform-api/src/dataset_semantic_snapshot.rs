use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use domain_model::{DatasetId, DocumentId, TenantId};
use serde::Serialize;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use storage::{
    NewDatasetSemanticLinkRun, NewDatasetSemanticLinkSnapshot, NewDatasetSemanticSnapshot,
    PgStorage,
};

use crate::semantic_label_resolver::{
    classify_semantic_primary_label, resolve_semantic_label, safe_semantic_business_label,
    LabelCandidate, SemanticLabelInput, SemanticPrimaryLabelClass, SAFE_GENERIC_OBJECT_LABEL,
};
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
    pub source_system_key: String,
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
    pub tenant_id: TenantId,
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

#[derive(Clone, Debug)]
pub struct DatasetSemanticSnapshotPreview {
    pub source_fingerprint: String,
    pub source_document_count: i64,
    pub source_asset_count: i64,
    pub source_record_count: i64,
    pub snapshot: DatasetSemanticUnderstanding,
}

pub const DATASET_CROSS_SEMANTIC_GRAPH_ENABLED_ENV: &str = "DATASET_CROSS_SEMANTIC_GRAPH_ENABLED";
pub const DATASET_CROSS_SEMANTIC_GRAPH_TENANT_ALLOWLIST_ENV: &str =
    "DATASET_CROSS_SEMANTIC_GRAPH_TENANT_ALLOWLIST";
pub const DATASET_CROSS_SEMANTIC_GRAPH_DATASET_ALLOWLIST_ENV: &str =
    "DATASET_CROSS_SEMANTIC_GRAPH_DATASET_ALLOWLIST";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DatasetCrossSemanticGraphAccess {
    Allowed,
    FeatureDisabled,
    TenantNotAllowlisted,
    LeftDatasetNotAllowlisted,
    RightDatasetNotAllowlisted,
}

impl DatasetCrossSemanticGraphAccess {
    pub fn is_allowed(self) -> bool {
        self == Self::Allowed
    }

    pub fn safe_reason(self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::FeatureDisabled => "feature_disabled",
            Self::TenantNotAllowlisted => "tenant_not_allowlisted",
            Self::LeftDatasetNotAllowlisted => "left_dataset_not_allowlisted",
            Self::RightDatasetNotAllowlisted => "right_dataset_not_allowlisted",
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ReadyDatasetSemanticLinkEndpoint {
    pub tenant_id: TenantId,
    pub dataset_id: DatasetId,
    pub snapshot_id: uuid::Uuid,
    pub source_fingerprint: String,
}

pub fn dataset_cross_semantic_graph_access(
    tenant_id: TenantId,
    left_dataset_id: DatasetId,
    right_dataset_id: DatasetId,
) -> DatasetCrossSemanticGraphAccess {
    dataset_cross_semantic_graph_access_from_values(
        semantic_env_flag(DATASET_CROSS_SEMANTIC_GRAPH_ENABLED_ENV, false),
        std::env::var(DATASET_CROSS_SEMANTIC_GRAPH_TENANT_ALLOWLIST_ENV)
            .ok()
            .as_deref(),
        std::env::var(DATASET_CROSS_SEMANTIC_GRAPH_DATASET_ALLOWLIST_ENV)
            .ok()
            .as_deref(),
        tenant_id,
        left_dataset_id,
        right_dataset_id,
    )
}

pub fn dataset_cross_semantic_graph_access_from_values(
    enabled: bool,
    tenant_allowlist: Option<&str>,
    dataset_allowlist: Option<&str>,
    tenant_id: TenantId,
    left_dataset_id: DatasetId,
    right_dataset_id: DatasetId,
) -> DatasetCrossSemanticGraphAccess {
    if !enabled {
        return DatasetCrossSemanticGraphAccess::FeatureDisabled;
    }
    if !semantic_uuid_csv_contains(tenant_allowlist, tenant_id.0) {
        return DatasetCrossSemanticGraphAccess::TenantNotAllowlisted;
    }
    if !semantic_uuid_csv_contains(dataset_allowlist, left_dataset_id.0) {
        return DatasetCrossSemanticGraphAccess::LeftDatasetNotAllowlisted;
    }
    if !semantic_uuid_csv_contains(dataset_allowlist, right_dataset_id.0) {
        return DatasetCrossSemanticGraphAccess::RightDatasetNotAllowlisted;
    }
    DatasetCrossSemanticGraphAccess::Allowed
}

pub fn plan_dataset_semantic_link_runs_from_values(
    enabled: bool,
    tenant_allowlist: Option<&str>,
    dataset_allowlist: Option<&str>,
    current: &ReadyDatasetSemanticLinkEndpoint,
    candidates: &[ReadyDatasetSemanticLinkEndpoint],
    available_at: DateTime<Utc>,
) -> Vec<NewDatasetSemanticLinkRun> {
    let unique_candidates = candidates
        .iter()
        .filter(|candidate| {
            candidate.tenant_id == current.tenant_id
                && candidate.dataset_id != current.dataset_id
                && dataset_cross_semantic_graph_access_from_values(
                    enabled,
                    tenant_allowlist,
                    dataset_allowlist,
                    current.tenant_id,
                    current.dataset_id,
                    candidate.dataset_id,
                )
                .is_allowed()
        })
        .cloned()
        .map(|candidate| ((candidate.dataset_id, candidate.snapshot_id), candidate))
        .collect::<BTreeMap<_, _>>()
        .into_values()
        .collect::<Vec<_>>();

    unique_candidates
        .into_iter()
        .filter_map(|candidate| {
            let (left_dataset_id, right_dataset_id, left_snapshot_id, right_snapshot_id) =
                storage::canonical_dataset_semantic_link_pair(
                    current.dataset_id,
                    candidate.dataset_id,
                    current.snapshot_id,
                    candidate.snapshot_id,
                )
                .ok()?;
            let (left_source_fingerprint, right_source_fingerprint) =
                if current.dataset_id < candidate.dataset_id {
                    (
                        current.source_fingerprint.as_str(),
                        candidate.source_fingerprint.as_str(),
                    )
                } else {
                    (
                        candidate.source_fingerprint.as_str(),
                        current.source_fingerprint.as_str(),
                    )
                };
            Some(NewDatasetSemanticLinkRun {
                left_dataset_id,
                right_dataset_id,
                left_snapshot_id,
                right_snapshot_id,
                generation_version:
                    crate::cross_dataset_semantic_graph::DATASET_SEMANTIC_GRAPH_GENERATION_VERSION
                        .to_string(),
                source_fingerprint: dataset_semantic_link_source_fingerprint(
                    left_dataset_id,
                    right_dataset_id,
                    left_snapshot_id,
                    right_snapshot_id,
                    left_source_fingerprint,
                    right_source_fingerprint,
                ),
                priority: 100,
                max_attempts: 3,
                available_at,
            })
        })
        .collect()
}

pub fn dataset_semantic_link_source_fingerprint(
    left_dataset_id: DatasetId,
    right_dataset_id: DatasetId,
    left_snapshot_id: uuid::Uuid,
    right_snapshot_id: uuid::Uuid,
    left_source_fingerprint: &str,
    right_source_fingerprint: &str,
) -> String {
    let (left_source_fingerprint, right_source_fingerprint) = if left_dataset_id < right_dataset_id
    {
        (left_source_fingerprint, right_source_fingerprint)
    } else {
        (right_source_fingerprint, left_source_fingerprint)
    };
    let (left_dataset_id, right_dataset_id, left_snapshot_id, right_snapshot_id) =
        storage::canonical_dataset_semantic_link_pair(
            left_dataset_id,
            right_dataset_id,
            left_snapshot_id,
            right_snapshot_id,
        )
        .expect("semantic link fingerprint requires distinct endpoints");
    let mut digest = Sha256::new();
    for value in [
        left_dataset_id.to_string(),
        right_dataset_id.to_string(),
        left_snapshot_id.to_string(),
        right_snapshot_id.to_string(),
        left_source_fingerprint.trim().to_string(),
        right_source_fingerprint.trim().to_string(),
        crate::cross_dataset_semantic_graph::DATASET_SEMANTIC_GRAPH_GENERATION_VERSION.to_string(),
    ] {
        digest.update(value.as_bytes());
        digest.update([0]);
    }
    let mut encoded = String::with_capacity(64);
    for byte in digest.finalize() {
        write!(&mut encoded, "{byte:02x}").expect("writing to a string cannot fail");
    }
    encoded
}

pub async fn enqueue_dataset_semantic_link_runs_for_ready_snapshot(
    storage: &PgStorage,
    ready_snapshot: &storage::DatasetSemanticSnapshot,
    now: DateTime<Utc>,
) -> Result<usize> {
    let enabled = semantic_env_flag(DATASET_CROSS_SEMANTIC_GRAPH_ENABLED_ENV, false);
    let tenant_allowlist = std::env::var(DATASET_CROSS_SEMANTIC_GRAPH_TENANT_ALLOWLIST_ENV).ok();
    let dataset_allowlist = std::env::var(DATASET_CROSS_SEMANTIC_GRAPH_DATASET_ALLOWLIST_ENV).ok();
    let current = ReadyDatasetSemanticLinkEndpoint {
        tenant_id: ready_snapshot.tenant_id,
        dataset_id: ready_snapshot.dataset_id,
        snapshot_id: ready_snapshot.id,
        source_fingerprint: ready_snapshot.source_fingerprint.clone(),
    };
    let candidates = storage
        .dataset_semantic_snapshots()
        .list_latest_ready_by_tenant(ready_snapshot.tenant_id, 10_000)
        .await?
        .into_iter()
        .map(|snapshot| ReadyDatasetSemanticLinkEndpoint {
            tenant_id: snapshot.tenant_id,
            dataset_id: snapshot.dataset_id,
            snapshot_id: snapshot.id,
            source_fingerprint: snapshot.source_fingerprint,
        })
        .collect::<Vec<_>>();
    let runs = plan_dataset_semantic_link_runs_from_values(
        enabled,
        tenant_allowlist.as_deref(),
        dataset_allowlist.as_deref(),
        &current,
        &candidates,
        now,
    );
    for run in &runs {
        ensure_dataset_semantic_link_run(storage, ready_snapshot.tenant_id, run, now).await?;
    }
    Ok(runs.len())
}

pub async fn ensure_dataset_semantic_link_run(
    storage: &PgStorage,
    tenant_id: TenantId,
    run: &NewDatasetSemanticLinkRun,
    now: DateTime<Utc>,
) -> Result<storage::DatasetSemanticLinkRun> {
    let links = storage.dataset_semantic_links();
    let persisted = links.create_or_get_run(tenant_id, run, now).await?;
    if persisted.status != "dead_letter" {
        links
            .try_begin_build(
                tenant_id,
                &NewDatasetSemanticLinkSnapshot {
                    left_dataset_id: run.left_dataset_id,
                    right_dataset_id: run.right_dataset_id,
                    left_snapshot_id: run.left_snapshot_id,
                    right_snapshot_id: run.right_snapshot_id,
                    schema_version:
                        crate::cross_dataset_semantic_graph::DATASET_SEMANTIC_GRAPH_SCHEMA_VERSION
                            .to_string(),
                    generation_version: run.generation_version.clone(),
                    source_fingerprint: run.source_fingerprint.clone(),
                    manifest: json!({}),
                },
                now,
            )
            .await?;
    }
    Ok(persisted)
}

fn semantic_uuid_csv_contains(csv: Option<&str>, expected: uuid::Uuid) -> bool {
    csv.into_iter()
        .flat_map(|value| value.split(','))
        .filter_map(|value| value.trim().parse::<uuid::Uuid>().ok())
        .any(|value| value == expected)
}

fn semantic_env_flag(key: &str, default: bool) -> bool {
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

pub const MAX_SEMANTIC_SNAPSHOT_MANIFEST_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SemanticSnapshotQualityReport {
    pub business_label_count: usize,
    pub chinese_business_label_count: usize,
    pub chinese_label_ratio: f64,
    pub raw_row_hit_count: usize,
    pub sql_or_mime_hit_count: usize,
    pub strategy_hit_count: usize,
    pub path_or_connection_hit_count: usize,
    pub technical_filename_hit_count: usize,
    pub numeric_identifier_hit_count: usize,
    pub manifest_bytes: usize,
    pub quality_gate_passed: bool,
}

pub fn audit_semantic_snapshot_quality(
    snapshot: &DatasetSemanticUnderstanding,
) -> SemanticSnapshotQualityReport {
    let mut report = SemanticSnapshotQualityReport {
        business_label_count: 0,
        chinese_business_label_count: 0,
        chinese_label_ratio: 0.0,
        raw_row_hit_count: 0,
        sql_or_mime_hit_count: 0,
        strategy_hit_count: 0,
        path_or_connection_hit_count: 0,
        technical_filename_hit_count: 0,
        numeric_identifier_hit_count: 0,
        manifest_bytes: serde_json::to_vec(snapshot)
            .map(|manifest| manifest.len())
            .unwrap_or(usize::MAX),
        quality_gate_passed: false,
    };

    for (label, technical_name) in snapshot
        .objects
        .iter()
        .map(|object| (object.label.as_str(), object.technical_name.as_str()))
        .chain(
            snapshot
                .fields
                .iter()
                .map(|field| (field.label.as_str(), field.technical_name.as_str())),
        )
    {
        let quality = classify_semantic_primary_label(label);
        if quality.business_label {
            report.business_label_count += 1;
            if quality.chinese_business_label {
                report.chinese_business_label_count += 1;
            }
        }
        record_snapshot_noise_hit(&mut report, quality.class);
        if technical_name != label {
            let technical_class = classify_semantic_primary_label(technical_name).class;
            if crate::semantic_label_resolver::semantic_technical_name_is_sensitive(technical_name)
            {
                record_snapshot_noise_hit(&mut report, SemanticPrimaryLabelClass::PathOrConnection);
            } else if !matches!(
                technical_class,
                SemanticPrimaryLabelClass::NumericIdentifier
                    | SemanticPrimaryLabelClass::TechnicalFilename
                    | SemanticPrimaryLabelClass::TechnicalIdentifier
            ) {
                record_snapshot_noise_hit(&mut report, technical_class);
            }
        }
    }
    report.chinese_label_ratio = if report.business_label_count == 0 {
        0.0
    } else {
        report.chinese_business_label_count as f64 / report.business_label_count as f64
    };
    let noise_free = report.raw_row_hit_count == 0
        && report.sql_or_mime_hit_count == 0
        && report.strategy_hit_count == 0
        && report.path_or_connection_hit_count == 0
        && report.technical_filename_hit_count == 0
        && report.numeric_identifier_hit_count == 0;
    // This gate protects the public manifest. Sparse sources such as a single
    // document or asset can be valid without fields or relations; canaries that
    // require a fully structured graph enforce those counts separately.
    report.quality_gate_passed = !snapshot.objects.is_empty()
        && report.business_label_count > 0
        && report.chinese_label_ratio >= 0.95
        && noise_free
        && report.manifest_bytes < MAX_SEMANTIC_SNAPSHOT_MANIFEST_BYTES;
    report
}

fn record_snapshot_noise_hit(
    report: &mut SemanticSnapshotQualityReport,
    class: SemanticPrimaryLabelClass,
) {
    match class {
        SemanticPrimaryLabelClass::RawRow => report.raw_row_hit_count += 1,
        SemanticPrimaryLabelClass::SqlOrMime => report.sql_or_mime_hit_count += 1,
        SemanticPrimaryLabelClass::Strategy | SemanticPrimaryLabelClass::OperationalInstruction => {
            report.strategy_hit_count += 1
        }
        SemanticPrimaryLabelClass::PathOrConnection => {
            report.path_or_connection_hit_count += 1;
        }
        SemanticPrimaryLabelClass::TechnicalFilename => {
            report.technical_filename_hit_count += 1;
        }
        SemanticPrimaryLabelClass::NumericIdentifier => {
            report.numeric_identifier_hit_count += 1;
        }
        SemanticPrimaryLabelClass::Business
        | SemanticPrimaryLabelClass::SafeGenericFallback
        | SemanticPrimaryLabelClass::Empty
        | SemanticPrimaryLabelClass::TechnicalIdentifier => {}
    }
}

pub async fn preview_dataset_semantic_snapshot_from_storage(
    storage: &PgStorage,
    tenant_id: TenantId,
    dataset_id: DatasetId,
    generated_at: DateTime<Utc>,
    limit: usize,
) -> Result<DatasetSemanticSnapshotPreview> {
    let dataset = storage
        .datasets()
        .get_by_id(tenant_id, dataset_id)
        .await?
        .ok_or_else(|| anyhow!("dataset {dataset_id} not found"))?;
    let memberships = storage
        .dataset_document_memberships()
        .list_active_by_dataset(tenant_id, dataset_id, generated_at)
        .await?;
    let raw_documents = storage
        .documents()
        .list_by_dataset_scope_bounded(tenant_id, dataset_id, generated_at, 10_000)
        .await?;
    let canonical_datasets = storage.datasets().list_by_tenant(tenant_id).await?;
    let contribution =
        crate::document_visibility_support::filter_documents_for_dataset_contribution(
            &dataset,
            raw_documents,
            &canonical_datasets,
        );
    if !contribution.excluded.is_empty() {
        tracing::warn!(
            target_dataset_id = %dataset_id,
            excluded = ?contribution.excluded,
            "historical documents excluded from semantic snapshot due to incompatible scope"
        );
    }
    let mut documents = contribution.documents;
    documents.truncate(limit.clamp(1, 10_000));
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
    let raw_assets = storage
        .asset_items()
        .list_by_dataset_scope(tenant_id, dataset_id, limit.clamp(1, 10_000))
        .await?;
    let scoped_document_ids = documents
        .iter()
        .map(|document| document.id.to_string())
        .collect::<BTreeSet<_>>();
    let assets = raw_assets
        .into_iter()
        .filter(|asset| !asset_is_derived_from_documents(asset, &scoped_document_ids))
        .collect::<Vec<_>>();
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
                source_dataset_id: document.dataset_id,
                owner_user_id: document.owner_user_id,
                secret_binding_ids: document.secret_binding_ids.clone(),
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
        let source_metadata = if source_kind == "database" {
            normalize_database_row_metadata_with_fallback(
                &source_metadata,
                &database_source_system_fallback(
                    document.dataset_id,
                    &document.object_key,
                    &map_to_value(&document.metadata),
                ),
            )
        } else {
            source_metadata
        };
        let source_key =
            semantic_source_scope_key(&source_kind, &source_metadata, &document.id.to_string());
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
    let dictionary_versions = dictionary
        .iter()
        .map(|entry| {
            format!(
                "{}|{}|{}|{}|{}@{}",
                entry.id,
                entry.source_kind.trim(),
                entry.source_system_key.trim(),
                entry.source_object_key.trim(),
                entry.raw_field_key.trim(),
                entry
                    .updated_at
                    .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
            )
        })
        .collect::<Vec<_>>();
    let source_fingerprint = crate::dataset_semantic_source_support::source_fingerprint(
        &crate::dataset_semantic_source_support::SemanticSourceFingerprintInput {
            dataset_scope: Some(
                crate::dataset_semantic_source_support::SemanticDatasetScopeVersion {
                    tenant_id,
                    dataset_id,
                    owner_user_id: dataset.owner_user_id,
                    visibility: dataset.visibility.as_str().to_string(),
                    default_secret_binding_ids: dataset.default_secret_binding_ids.clone(),
                    updated_at: dataset.updated_at,
                },
            ),
            documents: document_versions,
            assets: asset_versions,
            memberships: memberships
                .into_iter()
                .map(|membership| {
                    crate::dataset_semantic_source_support::SemanticDatasetMembershipVersion {
                        document_id: membership.document_id,
                        membership_kind: membership.membership_kind,
                        source: membership.source,
                        expires_at: membership.expires_at,
                        created_at: membership.created_at,
                    }
                })
                .collect(),
            source_identities: observations
                .iter()
                .filter_map(|observation| {
                    let identity = observation.source_identity.as_ref()?;
                    Some(
                        crate::dataset_semantic_source_support::SemanticSourceIdentityVersion {
                            source_kind: observation.source_kind.clone(),
                            source_system_key: identity.source_system_key.clone(),
                            source_schema_key: identity.source_schema_key.clone(),
                            source_object_key: identity.source_object_key.clone(),
                        },
                    )
                })
                .collect(),
            dataset_fact_snapshot_version: fact_snapshot_version.clone(),
            dictionary_versions,
        },
    );
    let dictionary_entries = dictionary
        .into_iter()
        .map(|entry| SnapshotDictionaryEntry {
            source_kind: entry.source_kind,
            source_system_key: entry.source_system_key,
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
        tenant_id,
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
    Ok(DatasetSemanticSnapshotPreview {
        source_fingerprint,
        source_document_count: documents.len() as i64,
        source_asset_count: assets.len() as i64,
        source_record_count: record_count as i64,
        snapshot: semantic_snapshot,
    })
}

pub async fn rebuild_dataset_semantic_snapshot_from_storage(
    storage: &PgStorage,
    tenant_id: TenantId,
    dataset_id: DatasetId,
    generated_at: DateTime<Utc>,
) -> Result<DatasetSemanticRebuildOutcome> {
    rebuild_dataset_semantic_snapshot_from_storage_with_limit(
        storage,
        tenant_id,
        dataset_id,
        generated_at,
        10_000,
    )
    .await
}

pub async fn rebuild_dataset_semantic_snapshot_from_storage_with_limit(
    storage: &PgStorage,
    tenant_id: TenantId,
    dataset_id: DatasetId,
    generated_at: DateTime<Utc>,
    limit: usize,
) -> Result<DatasetSemanticRebuildOutcome> {
    let preview = preview_dataset_semantic_snapshot_from_storage(
        storage,
        tenant_id,
        dataset_id,
        generated_at,
        limit,
    )
    .await?;
    let latest_ready = storage
        .dataset_semantic_snapshots()
        .load_latest_ready(tenant_id, dataset_id)
        .await?;
    if latest_ready.as_ref().is_some_and(|snapshot| {
        snapshot.source_fingerprint == preview.source_fingerprint
            && snapshot.generation_version == DATASET_SEMANTIC_GENERATION_VERSION
    }) {
        if let Some(snapshot) = latest_ready.as_ref() {
            if let Err(_) = enqueue_dataset_semantic_link_runs_for_ready_snapshot(
                storage,
                snapshot,
                generated_at,
            )
            .await
            {
                tracing::warn!(
                    tenant_id = %tenant_id,
                    dataset_id = %dataset_id,
                    snapshot_id = %snapshot.id,
                    failure_code = "link_enqueue_repair_failed",
                    "existing ready semantic snapshot could not repair cross-dataset link enqueue"
                );
            }
        }
        let snapshot = latest_ready.and_then(|item| serde_json::from_value(item.manifest).ok());
        return Ok(DatasetSemanticRebuildOutcome {
            status: "skipped".to_string(),
            source_fingerprint: preview.source_fingerprint,
            snapshot,
            failure_code: None,
        });
    }

    let new_snapshot = NewDatasetSemanticSnapshot {
        dataset_id,
        schema_version: DATASET_SEMANTIC_SCHEMA_VERSION.to_string(),
        generation_version: DATASET_SEMANTIC_GENERATION_VERSION.to_string(),
        source_fingerprint: preview.source_fingerprint.clone(),
        manifest: json!({}),
        source_document_count: preview.source_document_count,
        source_asset_count: preview.source_asset_count,
        source_record_count: preview.source_record_count,
    };
    let Some(build_record) = storage
        .dataset_semantic_snapshots()
        .try_begin_build(tenant_id, &new_snapshot, generated_at)
        .await?
    else {
        let snapshot = latest_ready.and_then(|item| serde_json::from_value(item.manifest).ok());
        return Ok(DatasetSemanticRebuildOutcome {
            status: "skipped".to_string(),
            source_fingerprint: preview.source_fingerprint,
            snapshot,
            failure_code: Some("build_in_progress_or_complete".to_string()),
        });
    };

    let quality_report = audit_semantic_snapshot_quality(&preview.snapshot);
    if !quality_report.quality_gate_passed {
        let failure_code = if quality_report.manifest_bytes >= MAX_SEMANTIC_SNAPSHOT_MANIFEST_BYTES
        {
            "manifest_too_large"
        } else {
            "semantic_quality_gate_failed"
        };
        storage
            .dataset_semantic_snapshots()
            .mark_failed(
                tenant_id,
                dataset_id,
                build_record.id,
                failure_code,
                generated_at,
            )
            .await?;
        return Ok(DatasetSemanticRebuildOutcome {
            status: "failed".to_string(),
            source_fingerprint: preview.source_fingerprint,
            snapshot: latest_ready
                .and_then(|item| serde_json::from_value(item.manifest).ok())
                .and_then(|item| semantic_snapshot_failure_fallback(Some(item), failure_code)),
            failure_code: Some(failure_code.to_string()),
        });
    }
    let manifest = serde_json::to_value(&preview.snapshot)?;
    let ready_record = storage
        .dataset_semantic_snapshots()
        .mark_ready(
            tenant_id,
            dataset_id,
            build_record.id,
            &manifest,
            (preview.snapshot.objects.len() + preview.snapshot.fields.len()) as i32,
            preview.snapshot.relations.len() as i32,
            generated_at,
        )
        .await?
        .ok_or_else(|| anyhow!("semantic snapshot build record is no longer claimable"))?;
    if let Err(_) =
        enqueue_dataset_semantic_link_runs_for_ready_snapshot(storage, &ready_record, generated_at)
            .await
    {
        tracing::warn!(
            tenant_id = %tenant_id,
            dataset_id = %dataset_id,
            snapshot_id = %ready_record.id,
            failure_code = "link_enqueue_failed",
            "dataset semantic snapshot is ready but cross-dataset link enqueue failed"
        );
    }
    Ok(DatasetSemanticRebuildOutcome {
        status: "ready".to_string(),
        source_fingerprint: preview.source_fingerprint,
        snapshot: Some(preview.snapshot),
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
        let public_technical_name = public_object_technical_name(representative);
        let object_id = stable_semantic_id("object", &[&input.tenant_id.to_string(), group_key]);
        object_ids.insert(group_key.clone(), object_id.clone());
        let confirmed_object_label =
            dictionary_candidate(&input.dictionary_entries, representative, "*")
                .filter(|candidate| candidate.status == "confirmed")
                .and_then(|candidate| {
                    safe_semantic_business_label(&candidate.label)
                        .map(|safe_label| (candidate, safe_label))
                });
        let (label, label_source, status, confidence, dictionary_description) =
            if let Some((candidate, safe_label)) = confirmed_object_label.as_ref() {
                (
                    safe_label.clone(),
                    "confirmed_dictionary".to_string(),
                    SemanticStatus::Confirmed,
                    candidate.confidence.clamp(0.0, 1.0),
                    candidate.description.clone(),
                )
            } else if let Some(safe_label) = representative
                .label_hint
                .as_deref()
                .and_then(safe_semantic_business_label)
                .or_else(|| safe_semantic_business_label(&public_technical_name))
            {
                (
                    safe_label,
                    representative.label_source.clone(),
                    representative.status,
                    representative.confidence,
                    None,
                )
            } else {
                (
                    SAFE_GENERIC_OBJECT_LABEL.to_string(),
                    "safe_generic_fallback".to_string(),
                    SemanticStatus::Unresolved,
                    0.0,
                    None,
                )
            };
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
            technical_name: public_technical_name,
            description: dictionary_description
                .unwrap_or_else(|| format!("由 {source_count} 个来源记录的可追溯结构观察归纳。")),
            label_source,
            confidence,
            coverage_count: source_count,
            status,
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
            confirmed_dictionary: dictionary_candidate(
                &input.dictionary_entries,
                observation,
                &observation.technical_name,
            ),
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
                        .and_then(safe_semantic_business_label)
                        .map(|label| {
                            (
                                label,
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
        let (source_system_key, source_schema_key, source_object_key) = observation
            .source_identity
            .as_ref()
            .map(|identity| {
                (
                    identity.source_system_key.as_str(),
                    identity.source_schema_key.as_str(),
                    identity.source_object_key.as_str(),
                )
            })
            .unwrap_or((
                observation.source_id.as_str(),
                "",
                observation.object_key.as_str(),
            ));
        format!(
            "{}|{}|{}|{}|{}",
            normalized_identity_key(&observation.object_kind),
            normalized_identity_key(&observation.source_kind),
            normalized_identity_key(source_system_key),
            normalized_identity_key(source_schema_key),
            normalized_identity_key(source_object_key),
        )
    } else {
        format!(
            "{}|{}|{}",
            observation.object_kind, observation.source_id, observation.object_key
        )
    }
}

fn public_object_technical_name(observation: &SemanticObservation) -> String {
    if observation.object_kind != "database_table" {
        return observation.technical_name.trim().to_string();
    }
    observation
        .source_identity
        .as_ref()
        .map(|identity| identity.source_object_key.trim())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            observation
                .object_key
                .trim()
                .rsplit('.')
                .next()
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
        .unwrap_or("database_object")
        .to_string()
}

fn dictionary_candidate(
    entries: &[SnapshotDictionaryEntry],
    observation: &SemanticObservation,
    raw_field_key: &str,
) -> Option<LabelCandidate> {
    let source_kind = normalized_identity_key(&observation.source_kind);
    let (source_system_key, source_object_key, bare_source_object_key) =
        dictionary_source_scope(observation);
    let raw_field_key = normalized_identity_key(raw_field_key);
    entries
        .iter()
        .filter(|entry| normalized_identity_key(&entry.source_kind) == source_kind)
        .filter(|entry| {
            let candidate = normalized_identity_key(&entry.source_system_key);
            candidate == "*" || candidate == source_system_key
        })
        .filter(|entry| {
            let candidate = normalized_identity_key(&entry.source_object_key);
            let candidate_system = normalized_identity_key(&entry.source_system_key);
            candidate == "*"
                || candidate == source_object_key
                || (candidate_system == source_system_key && candidate == bare_source_object_key)
        })
        .filter(|entry| normalized_identity_key(&entry.raw_field_key) == raw_field_key)
        .min_by(|left, right| {
            dictionary_candidate_rank(
                left,
                &source_system_key,
                &source_object_key,
                &bare_source_object_key,
            )
            .cmp(&dictionary_candidate_rank(
                right,
                &source_system_key,
                &source_object_key,
                &bare_source_object_key,
            ))
            .then_with(|| {
                right
                    .candidate
                    .confidence
                    .partial_cmp(&left.candidate.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| left.candidate.label.cmp(&right.candidate.label))
        })
        .map(|entry| entry.candidate.clone())
}

fn dictionary_candidate_rank(
    entry: &SnapshotDictionaryEntry,
    source_system_key: &str,
    source_object_key: &str,
    bare_source_object_key: &str,
) -> (u8, u8, u8) {
    let candidate_object_key = normalized_identity_key(&entry.source_object_key);
    let object_scope_rank = if candidate_object_key == source_object_key {
        0
    } else if candidate_object_key == bare_source_object_key {
        1
    } else {
        2
    };
    (
        u8::from(entry.candidate.status != "confirmed"),
        u8::from(normalized_identity_key(&entry.source_system_key) != source_system_key),
        object_scope_rank,
    )
}

fn dictionary_source_scope(observation: &SemanticObservation) -> (String, String, String) {
    let Some(identity) = observation.source_identity.as_ref() else {
        return (
            normalized_identity_key(&observation.source_id),
            normalized_identity_key(&observation.object_key),
            normalized_identity_key(&observation.object_key),
        );
    };
    let schema = normalized_identity_key(&identity.source_schema_key);
    let object = normalized_identity_key(&identity.source_object_key);
    let scoped_object = if schema.is_empty() {
        object.clone()
    } else {
        format!("{schema}.{object}")
    };
    (
        normalized_identity_key(&identity.source_system_key),
        scoped_object,
        object,
    )
}

fn normalized_identity_key(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn asset_is_derived_from_documents(
    asset: &storage::AssetItemRecord,
    scoped_document_ids: &BTreeSet<String>,
) -> bool {
    if asset.asset_kind.eq_ignore_ascii_case("document")
        || asset.source_kind.eq_ignore_ascii_case("document")
    {
        return true;
    }
    let metadata_document_id = asset
        .metadata
        .get("document_id")
        .and_then(Value::as_str)
        .map(str::trim);
    metadata_document_id.is_some_and(|document_id| scoped_document_ids.contains(document_id))
        || (asset.source_kind.eq_ignore_ascii_case("document")
            && asset
                .source_id
                .as_deref()
                .map(str::trim)
                .is_some_and(|document_id| scoped_document_ids.contains(document_id)))
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
            if observation.object_kind != "database_table" {
                return None;
            }
            let (source_system_key, source_schema_key, source_object_key) =
                normalized_database_source_identity(observation);
            Some(DatabaseRelationGroup {
                group_key: group_key.as_str(),
                source_system_key,
                source_schema_key,
                source_object_key,
            })
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
            let target_group =
                resolve_database_reference_group(constraint, target_object_key, &database_groups)?;
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

struct DatabaseRelationGroup<'a> {
    group_key: &'a str,
    source_system_key: String,
    source_schema_key: String,
    source_object_key: String,
}

fn normalized_database_source_identity(
    observation: &SemanticObservation,
) -> (String, String, String) {
    if let Some(identity) = observation.source_identity.as_ref() {
        return (
            normalized_identity_key(&identity.source_system_key),
            normalized_identity_key(&identity.source_schema_key),
            normalized_identity_key(&identity.source_object_key),
        );
    }
    let object_key = normalized_identity_key(&observation.object_key);
    let (schema, object) = object_key
        .rsplit_once('.')
        .map(|(schema, object)| (schema.to_string(), object.to_string()))
        .unwrap_or_else(|| (String::new(), object_key));
    (
        normalized_identity_key(&observation.source_id),
        schema,
        object,
    )
}

fn resolve_database_reference_group<'a>(
    constraint: &SemanticObservation,
    target_object_key: &str,
    database_groups: &'a [DatabaseRelationGroup<'a>],
) -> Option<&'a str> {
    let (source_system_key, source_schema_key, _) = normalized_database_source_identity(constraint);
    let normalized_target = normalized_identity_key(target_object_key);
    let (explicit_schema, target_object) = normalized_target
        .rsplit_once('.')
        .map(|(schema, object)| (Some(schema), object))
        .unwrap_or((None, normalized_target.as_str()));
    let candidates = database_groups
        .iter()
        .filter(|candidate| candidate.source_system_key == source_system_key)
        .filter(|candidate| candidate.source_object_key == target_object)
        .filter(|candidate| {
            explicit_schema.is_none_or(|schema| candidate.source_schema_key == schema)
        })
        .collect::<Vec<_>>();

    if explicit_schema.is_some() {
        return unique_database_group(&candidates);
    }
    if !source_schema_key.is_empty() {
        let same_schema = candidates
            .iter()
            .copied()
            .filter(|candidate| candidate.source_schema_key == source_schema_key)
            .collect::<Vec<_>>();
        return unique_database_group(&same_schema);
    }
    unique_database_group(&candidates)
}

fn unique_database_group<'a>(candidates: &[&'a DatabaseRelationGroup<'a>]) -> Option<&'a str> {
    (candidates.len() == 1).then(|| candidates[0].group_key)
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

fn normalize_database_row_metadata(metadata: &Value) -> Value {
    let parse = metadata
        .get("parse_metadata")
        .or_else(|| metadata.get("external_metadata"))
        .unwrap_or(metadata);
    let Some(parse_object) = parse.as_object() else {
        return metadata.clone();
    };
    let mut fields = parse_object
        .get("fields")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for (key, value) in parse_object {
        if database_metadata_control_key(key) || sensitive_semantic_field_key(key) {
            continue;
        }
        fields.entry(key.clone()).or_insert_with(|| value.clone());
    }

    let mut normalized = Map::new();
    for key in [
        "source_table",
        "source_system_key",
        "source_system",
        "source_schema",
        "schema",
        "table_comment",
        "field_comments",
        "foreign_keys",
    ] {
        if let Some(value) = parse_object.get(key) {
            normalized.insert(key.to_string(), value.clone());
        }
    }
    if !normalized.contains_key("source_system_key") {
        if let Some(source_system_key) = metadata
            .get("external_source")
            .and_then(|value| value.get("source_id"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            normalized.insert(
                "source_system_key".to_string(),
                Value::String(source_system_key.to_string()),
            );
        }
    }
    let primary_key = parse_object
        .get("primary_key")
        .or_else(|| parse_object.get("source_primary_key_columns"))
        .or_else(|| parse_object.get("source_primary_key"));
    if let Some(value) = primary_key {
        normalized.insert("primary_key".to_string(), normalized_string_array(value));
    }
    normalized.insert("fields".to_string(), Value::Object(fields));
    json!({"parse_metadata": normalized})
}

fn normalize_database_row_metadata_with_fallback(metadata: &Value, fallback: &str) -> Value {
    let mut normalized = normalize_database_row_metadata(metadata);
    let Some(parse) = normalized
        .get_mut("parse_metadata")
        .and_then(Value::as_object_mut)
    else {
        return normalized;
    };
    if !parse.contains_key("source_system_key") {
        parse.insert(
            "source_system_key".to_string(),
            Value::String(fallback.trim().to_string()),
        );
    }
    normalized
}

fn database_source_system_fallback(
    source_dataset_id: DatasetId,
    object_key: &str,
    document_metadata: &Value,
) -> String {
    if let Some(source_id) = document_metadata
        .get("external_source")
        .and_then(|value| value.get("source_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return source_id.to_string();
    }
    let mut segments = object_key.trim().split('/');
    if segments.next() == Some("external") {
        if let Some(source_id) = segments
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return source_id.to_string();
        }
    }
    stable_semantic_id("legacy_source", &[&source_dataset_id.to_string()])
}

fn semantic_source_scope_key(source_kind: &str, metadata: &Value, fallback_id: &str) -> String {
    if source_kind != "database" {
        return stable_semantic_id("source_scope", &[source_kind, fallback_id]);
    }
    let parse = metadata
        .get("parse_metadata")
        .or_else(|| metadata.get("external_metadata"))
        .unwrap_or(metadata);
    let source_system = parse
        .get("source_system_key")
        .or_else(|| parse.get("source_system"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback_id);
    let source_schema = parse
        .get("source_schema")
        .or_else(|| parse.get("schema"))
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    let source_object = parse
        .get("source_table")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback_id);
    stable_semantic_id(
        "database_source_scope",
        &[source_system, source_schema, source_object],
    )
}

fn database_metadata_control_key(key: &str) -> bool {
    matches!(
        key.trim().to_ascii_lowercase().as_str(),
        "source_kind"
            | "source_table"
            | "source_system_key"
            | "source_system"
            | "source_schema"
            | "source_primary_key"
            | "source_primary_key_columns"
            | "source_updated_at"
            | "schema"
            | "table_comment"
            | "field_comments"
            | "foreign_keys"
            | "primary_key"
            | "fields"
    )
}

fn sensitive_semantic_field_key(key: &str) -> bool {
    let normalized = key.trim().to_ascii_lowercase().replace(['-', '.'], "_");
    [
        "password",
        "passwd",
        "secret",
        "token",
        "api_key",
        "access_key",
        "private_key",
        "credential",
        "connection_string",
        "database_url",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
}

fn normalized_string_array(value: &Value) -> Value {
    let mut values = match value {
        Value::Array(items) => items
            .iter()
            .filter_map(|item| item.as_str())
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>(),
        Value::String(item) => item
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    };
    values.sort();
    values.dedup();
    Value::Array(values.into_iter().map(Value::String).collect())
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
    }) || document
        .metadata
        .get("external_metadata")
        .or_else(|| document.metadata.get("parse_metadata"))
        .and_then(|value| value.get("source_table"))
        .and_then(Value::as_str)
        .is_some()
    {
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
            tenant_id: TenantId(Uuid::from_u128(2)),
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
            dictionary_entries: vec![SnapshotDictionaryEntry {
                source_kind: "database".to_string(),
                source_system_key: "*".to_string(),
                source_object_key: "lease_contract".to_string(),
                raw_field_key: "*".to_string(),
                candidate: LabelCandidate::confirmed("租赁合同", "租赁合同主数据及其可追溯结构。"),
            }],
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
    fn database_row_metadata_becomes_structured_fields_and_drops_sensitive_keys() {
        let normalized = normalize_database_row_metadata(&json!({
            "parse_metadata": {
                "source_kind": "database_row",
                "source_table": "lease_contract",
                "source_primary_key_columns": "contract_id, store_id",
                "source_updated_at": "2026-07-14T00:00:00Z",
                "contract_id": "C-001",
                "rent_amount": 1200,
                "api_token": "must-not-appear"
            }
        }));
        let parse = normalized
            .get("parse_metadata")
            .expect("normalized parse metadata");

        assert_eq!(parse["source_table"], json!("lease_contract"));
        assert_eq!(parse["primary_key"], json!(["contract_id", "store_id"]));
        assert_eq!(parse["fields"]["contract_id"], json!("C-001"));
        assert_eq!(parse["fields"]["rent_amount"], json!(1200));
        assert!(parse["fields"].get("source_updated_at").is_none());
        assert!(parse["fields"].get("api_token").is_none());

        let observations = adapt_semantic_profile(&SemanticProfileInput {
            source_id: "row-1".to_string(),
            source_kind: "database".to_string(),
            title: "row".to_string(),
            metadata: normalized,
            facts: Vec::new(),
            evidence_labels: vec!["database row".to_string()],
        });
        assert!(observations
            .iter()
            .any(|item| item.technical_name == "rent_amount"));
        assert!(!observations
            .iter()
            .any(|item| item.technical_name == "api_token"));
    }

    #[test]
    fn database_source_system_fallback_is_stable_per_canonical_dataset() {
        let dataset_id = DatasetId(Uuid::from_u128(11));
        let other_dataset_id = DatasetId(Uuid::from_u128(12));
        let first = database_source_system_fallback(dataset_id, "legacy/row-1", &json!({}));
        let second = database_source_system_fallback(dataset_id, "legacy/row-2", &json!({}));
        let other = database_source_system_fallback(other_dataset_id, "legacy/row-1", &json!({}));

        assert_eq!(first, second);
        assert_ne!(first, other);
        assert_eq!(
            database_source_system_fallback(dataset_id, "external/connector-a/row-1", &json!({})),
            "connector-a"
        );
        assert_eq!(
            database_source_system_fallback(
                dataset_id,
                "legacy/row-1",
                &json!({"external_source": {"source_id": "connector-b"}})
            ),
            "connector-b"
        );

        let normalized = normalize_database_row_metadata_with_fallback(
            &json!({"external_metadata": {
                "schema": "finance",
                "source_table": "lease_contract",
                "fields": {"contract_id": "C-001"}
            }}),
            &first,
        );
        assert_eq!(normalized["parse_metadata"]["source_system_key"], first);
        assert_eq!(normalized["parse_metadata"]["schema"], "finance");
        assert_eq!(
            normalized["parse_metadata"]["source_table"],
            "lease_contract"
        );

        let erp_a = semantic_source_scope_key(
            "database",
            &json!({"parse_metadata": {
                "source_system_key": "erp-a",
                "schema": "finance",
                "source_table": "lease_contract"
            }}),
            "row-a",
        );
        let erp_a_again = semantic_source_scope_key(
            "database",
            &json!({"parse_metadata": {
                "source_system_key": "erp-a",
                "schema": "finance",
                "source_table": "lease_contract"
            }}),
            "row-b",
        );
        let erp_b = semantic_source_scope_key(
            "database",
            &json!({"parse_metadata": {
                "source_system_key": "erp-b",
                "schema": "finance",
                "source_table": "lease_contract"
            }}),
            "row-c",
        );
        assert_eq!(erp_a, erp_a_again);
        assert_ne!(erp_a, erp_b);
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
    fn semantic_snapshot_quality_report_counts_noise_and_enforces_gate() {
        let clean = build_dataset_semantic_snapshot(&fixture_input());
        let clean_report = audit_semantic_snapshot_quality(&clean);
        assert!(clean_report.business_label_count >= 3);
        assert_eq!(
            clean_report.business_label_count,
            clean_report.chinese_business_label_count
        );
        assert_eq!(clean_report.chinese_label_ratio, 1.0);
        assert_eq!(clean_report.raw_row_hit_count, 0);
        assert_eq!(clean_report.sql_or_mime_hit_count, 0);
        assert_eq!(clean_report.strategy_hit_count, 0);
        assert_eq!(clean_report.path_or_connection_hit_count, 0);
        assert_eq!(clean_report.technical_filename_hit_count, 0);
        assert!(clean_report.manifest_bytes > 0);
        assert!(clean_report.quality_gate_passed);

        let mut sparse_but_safe = clean.clone();
        sparse_but_safe.fields.clear();
        sparse_but_safe.relations.clear();
        let sparse_report = audit_semantic_snapshot_quality(&sparse_but_safe);
        assert!(sparse_report.quality_gate_passed);

        let mut technical_only = clean.clone();
        technical_only.fields[0].technical_name = "2026".to_string();
        technical_only.fields[1].technical_name = "technical_report_alpha.xlsx".to_string();
        let technical_only_report = audit_semantic_snapshot_quality(&technical_only);
        assert_eq!(technical_only_report.numeric_identifier_hit_count, 0);
        assert_eq!(technical_only_report.technical_filename_hit_count, 0);
        assert!(technical_only_report.quality_gate_passed);

        technical_only.fields[0].technical_name = "a".repeat(64);
        let sensitive_technical_report = audit_semantic_snapshot_quality(&technical_only);
        assert_eq!(sensitive_technical_report.path_or_connection_hit_count, 1);
        assert!(!sensitive_technical_report.quality_gate_passed);

        let mut noisy = clean;
        noisy.objects[0].label = "1001,新街口门店,2026,123456.78".to_string();
        noisy.fields[0].label = "select * from lease_contract".to_string();
        noisy.fields[1].label =
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string();
        for (id, label) in [
            ("strategy", "paragraph_aware_noun_terms_v1"),
            ("path", "C:\\internal\\newbai\\source.xlsx"),
            ("filename", "technical_report_alpha.xlsx"),
            ("identifier", "HT-2026-000001"),
        ] {
            let mut field = noisy.fields[0].clone();
            field.id = format!("field:{id}");
            field.label = label.to_string();
            noisy.fields.push(field);
        }
        let report = audit_semantic_snapshot_quality(&noisy);
        assert_eq!(report.raw_row_hit_count, 1);
        assert_eq!(report.sql_or_mime_hit_count, 2);
        assert_eq!(report.strategy_hit_count, 1);
        assert_eq!(report.path_or_connection_hit_count, 1);
        assert_eq!(report.technical_filename_hit_count, 1);
        assert_eq!(report.numeric_identifier_hit_count, 1);
        assert!(!report.quality_gate_passed);

        let json = serde_json::to_value(report).expect("quality report serializable");
        for key in [
            "business_label_count",
            "chinese_business_label_count",
            "chinese_label_ratio",
            "raw_row_hit_count",
            "sql_or_mime_hit_count",
            "strategy_hit_count",
            "path_or_connection_hit_count",
            "technical_filename_hit_count",
            "manifest_bytes",
            "quality_gate_passed",
        ] {
            assert!(json.get(key).is_some(), "missing report key {key}");
        }
    }

    #[test]
    fn sanitized_newbai_fixture_builds_ready_quality_contract_without_noise() {
        let source = SemanticProfileInput {
            source_id: "newbai:sanitized:workbook".to_string(),
            source_kind: "spreadsheet".to_string(),
            title: "Data_Buddy_AI新百经营分析5个重点场景.xlsx".to_string(),
            metadata: json!({"parse_metadata": {}}),
            facts: vec![
                json!({"name": "项目名称", "value_type": "text"}),
                json!({"name": "合同金额", "value_type": "number"}),
                json!({"name": "经营状态", "value_type": "text"}),
                json!({"name": "租赁面积", "fact_type": "metric", "value_type": "number"}),
                json!({"name": "固定与提成取高预警V1", "value_type": "text"}),
                json!({"name": "2026", "value_type": "number"}),
                json!({"name": "HT-2026-000001", "value_type": "text"}),
                json!({"name": "select * from lease_contract", "value_type": "text"}),
                json!({
                    "name": "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                    "value_type": "text"
                }),
                json!({"name": "paragraph_aware_noun_terms_v1", "value_type": "text"}),
                json!({"name": "C:\\internal\\newbai\\source.xlsx", "value_type": "text"}),
                json!({"name": "technical_report_alpha.xlsx", "value_type": "text"}),
                json!({"name": "1001,新街口门店,2026,123456.78", "value_type": "text"}),
            ],
            evidence_labels: vec!["新百脱敏结构".to_string()],
        };
        let mut input = fixture_input();
        input.dataset.title = "新百项目资料".to_string();
        input.observations = adapt_semantic_profile(&source);
        input.dictionary_entries.clear();
        input.coverage.document_count = 1;
        input.coverage.record_count = 0;
        input.coverage.confirmed_fact_count = 1;

        let snapshot = build_dataset_semantic_snapshot(&input);
        let report = audit_semantic_snapshot_quality(&snapshot);
        assert!(!snapshot.objects.is_empty());
        assert!(!snapshot.fields.is_empty());
        assert!(!snapshot.relations.is_empty());
        assert!(report.quality_gate_passed, "report={report:?}");

        let public_manifest = serde_json::to_string(&snapshot).expect("serializable snapshot");
        for noise in [
            "1001,新街口门店,2026,123456.78",
            "HT-2026-000001",
            "select * from lease_contract",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "paragraph_aware_noun_terms_v1",
            "C:\\internal\\newbai\\source.xlsx",
            "technical_report_alpha.xlsx",
        ] {
            assert!(!public_manifest.contains(noise), "leaked noise: {noise}");
        }
        assert!(public_manifest.contains("新百经营分析重点场景"));
        assert!(public_manifest.contains("合同金额"));
        assert!(public_manifest.contains("租赁面积"));
        assert!(public_manifest.contains("固定与提成取高预警"));
    }

    #[test]
    fn snapshot_builder_does_not_promote_unresolved_english_identifier_to_headline() {
        let mut input = fixture_input();
        input.dictionary_entries.clear();
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
                metadata: normalize_database_row_metadata(&json!({
                    "external_source": {"source_id": "fixture-db"},
                    "parse_metadata": {
                        "source_table": "lease_contract",
                        "fields": fields,
                        "field_comments": {
                            "contract_id": "合同编号",
                            "rent_amount": "租金金额"
                        }
                    }
                })),
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
    fn dictionary_resolution_prefers_schema_scope_but_keeps_same_system_table_compatibility() {
        let mut input = fixture_input();
        input.observations = adapt_semantic_profile(&SemanticProfileInput {
            source_id: "row-a".to_string(),
            source_kind: "database".to_string(),
            title: "租赁合同".to_string(),
            metadata: json!({"parse_metadata": {
                "source_system_key": "erp-a",
                "schema": "finance",
                "source_table": "lease_contract",
                "fields": {"contract_id": "C-001"}
            }}),
            facts: Vec::new(),
            evidence_labels: vec!["结构解析".to_string()],
        });
        input.dictionary_entries = vec![
            SnapshotDictionaryEntry {
                source_kind: "database".to_string(),
                source_system_key: "*".to_string(),
                source_object_key: "*".to_string(),
                raw_field_key: "contract_id".to_string(),
                candidate: LabelCandidate::confirmed("通用合同号", "wildcard"),
            },
            SnapshotDictionaryEntry {
                source_kind: "database".to_string(),
                source_system_key: "erp-a".to_string(),
                source_object_key: "lease_contract".to_string(),
                raw_field_key: "contract_id".to_string(),
                candidate: LabelCandidate::confirmed("兼容合同号", "legacy table scope"),
            },
            SnapshotDictionaryEntry {
                source_kind: "database".to_string(),
                source_system_key: "erp-a".to_string(),
                source_object_key: "finance.lease_contract".to_string(),
                raw_field_key: "contract_id".to_string(),
                candidate: LabelCandidate::confirmed("财务合同号", "schema scope"),
            },
        ];

        let exact = build_dataset_semantic_snapshot(&input);
        assert_eq!(exact.fields[0].label, "财务合同号");

        input
            .dictionary_entries
            .retain(|entry| entry.source_object_key != "finance.lease_contract");
        let compatible = build_dataset_semantic_snapshot(&input);
        assert_eq!(compatible.fields[0].label, "兼容合同号");
    }

    #[test]
    fn same_named_database_objects_are_scoped_by_tenant_system_and_schema() {
        let mut input = fixture_input();
        input.observations = [
            ("row-a", "erp-internal-a", "甲系统合同", "甲合同号"),
            ("row-b", "erp-internal-b", "乙系统合同", "乙合同号"),
        ]
        .into_iter()
        .flat_map(|(source_id, source_system_key, _, _)| {
            adapt_semantic_profile(&SemanticProfileInput {
                source_id: source_id.to_string(),
                source_kind: "database".to_string(),
                title: "租赁合同".to_string(),
                metadata: json!({"parse_metadata": {
                    "source_system_key": source_system_key,
                    "schema": "finance_private",
                    "source_table": "lease_contract",
                    "fields": {"contract_id": source_id}
                }}),
                facts: Vec::new(),
                evidence_labels: vec![format!("结构解析:{source_id}")],
            })
        })
        .collect();
        input.dictionary_entries = [
            ("erp-internal-a", "*", "甲系统合同"),
            ("erp-internal-b", "*", "乙系统合同"),
            ("erp-internal-a", "contract_id", "甲合同号"),
            ("erp-internal-b", "contract_id", "乙合同号"),
        ]
        .into_iter()
        .map(
            |(source_system_key, raw_field_key, display_name)| SnapshotDictionaryEntry {
                source_kind: "database".to_string(),
                source_system_key: source_system_key.to_string(),
                source_object_key: "finance_private.lease_contract".to_string(),
                raw_field_key: raw_field_key.to_string(),
                candidate: LabelCandidate::confirmed(display_name, "fixture dictionary"),
            },
        )
        .collect();

        let snapshot = build_dataset_semantic_snapshot(&input);
        assert_eq!(snapshot.objects.len(), 2);
        assert_eq!(snapshot.fields.len(), 2);
        assert_eq!(
            snapshot
                .objects
                .iter()
                .map(|object| object.label.as_str())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["甲系统合同", "乙系统合同"])
        );
        assert_eq!(
            snapshot
                .fields
                .iter()
                .map(|field| field.label.as_str())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["甲合同号", "乙合同号"])
        );
        assert_ne!(snapshot.objects[0].id, snapshot.objects[1].id);
        assert_ne!(snapshot.fields[0].id, snapshot.fields[1].id);

        let public_manifest = serde_json::to_string(&snapshot).expect("serializable snapshot");
        assert!(!public_manifest.contains("erp-internal-a"));
        assert!(!public_manifest.contains("erp-internal-b"));
        assert!(!public_manifest.contains("finance_private"));

        let mut other_tenant_input = input.clone();
        other_tenant_input.tenant_id = TenantId(Uuid::from_u128(999));
        let other_tenant = build_dataset_semantic_snapshot(&other_tenant_input);
        assert!(snapshot
            .fields
            .iter()
            .all(|field| other_tenant.fields.iter().all(|other| other.id != field.id)));
    }

    #[test]
    fn schema_qualified_database_table_does_not_expose_schema_in_public_manifest() {
        let mut input = fixture_input();
        input.observations = adapt_semantic_profile(&SemanticProfileInput {
            source_id: "row-a".to_string(),
            source_kind: "database".to_string(),
            title: "租赁合同".to_string(),
            metadata: json!({"parse_metadata": {
                "source_system_key": "erp-internal-a",
                "source_table": "finance_private.lease_contract",
                "fields": {"contract_id": "C-001"}
            }}),
            facts: Vec::new(),
            evidence_labels: vec!["结构解析".to_string()],
        });
        input.dictionary_entries.clear();

        let snapshot = build_dataset_semantic_snapshot(&input);
        let public_manifest = serde_json::to_string(&snapshot).expect("serializable snapshot");

        assert_eq!(snapshot.objects[0].technical_name, "lease_contract");
        assert!(!public_manifest.contains("finance_private"));
        assert!(!public_manifest.contains("erp-internal-a"));
    }

    #[test]
    fn document_public_technical_name_does_not_expose_opaque_source_id() {
        let mut input = fixture_input();
        let source_id = "d4923d83-6053-4feb-8005-b22ee51e0227";
        input.observations = adapt_semantic_profile(&SemanticProfileInput {
            source_id: source_id.to_string(),
            source_kind: "document".to_string(),
            title: "新百项目.zip".to_string(),
            metadata: json!({"sections": ["经营摘要"]}),
            facts: vec![json!({"name": "项目负责人", "value_type": "text"})],
            evidence_labels: vec!["文档结构".to_string()],
        });
        input.dictionary_entries.clear();

        let snapshot = build_dataset_semantic_snapshot(&input);
        let object = snapshot.objects.first().expect("document object");

        assert_eq!(object.technical_name, "新百项目");
        assert_ne!(object.technical_name, source_id);
        assert!(audit_semantic_snapshot_quality(&snapshot).quality_gate_passed);
    }

    #[test]
    fn foreign_keys_do_not_cross_same_named_tables_from_different_source_systems() {
        let mut input = fixture_input();
        input.observations = [
            (
                "erp-a-contract",
                "erp-a",
                "lease_contract",
                "甲合同",
                json!({"store_id": "A-01"}),
                json!({"store_id": "甲门店编号"}),
                json!([{"field": "store_id", "references": "store.id"}]),
            ),
            (
                "erp-a-store",
                "erp-a",
                "store",
                "甲门店",
                json!({"id": "A-01"}),
                json!({"id": "甲门店主键"}),
                json!([]),
            ),
            (
                "erp-b-contract",
                "erp-b",
                "lease_contract",
                "乙合同",
                json!({"store_id": "B-01"}),
                json!({"store_id": "乙门店编号"}),
                json!([{"field": "store_id", "references": "store.id"}]),
            ),
            (
                "erp-b-store",
                "erp-b",
                "store",
                "乙门店",
                json!({"id": "B-01"}),
                json!({"id": "乙门店主键"}),
                json!([]),
            ),
        ]
        .into_iter()
        .flat_map(
            |(
                source_id,
                source_system,
                table,
                table_comment,
                fields,
                field_comments,
                foreign_keys,
            )| {
                adapt_semantic_profile(&SemanticProfileInput {
                    source_id: source_id.to_string(),
                    source_kind: "database".to_string(),
                    title: table_comment.to_string(),
                    metadata: json!({"parse_metadata": {
                        "source_system_key": source_system,
                        "schema": "retail",
                        "source_table": table,
                        "table_comment": table_comment,
                        "fields": fields,
                        "field_comments": field_comments,
                        "foreign_keys": foreign_keys
                    }}),
                    facts: Vec::new(),
                    evidence_labels: vec![format!("结构解析:{source_id}")],
                })
            },
        )
        .collect();
        input.dictionary_entries.clear();

        let snapshot = build_dataset_semantic_snapshot(&input);
        let field_labels = snapshot
            .fields
            .iter()
            .map(|field| (field.id.as_str(), field.label.as_str()))
            .collect::<BTreeMap<_, _>>();
        let relation_pairs = snapshot
            .relations
            .iter()
            .filter(|relation| relation.relation_type == "foreign_key")
            .map(|relation| {
                (
                    *field_labels.get(relation.source_id.as_str()).unwrap(),
                    *field_labels.get(relation.target_id.as_str()).unwrap(),
                )
            })
            .collect::<BTreeSet<_>>();

        assert_eq!(
            relation_pairs,
            BTreeSet::from([("甲门店编号", "甲门店主键"), ("乙门店编号", "乙门店主键"),])
        );
    }

    #[test]
    fn database_foreign_key_constraint_becomes_a_confirmed_field_relation() {
        let mut input = fixture_input();
        let contract = SemanticProfileInput {
            source_id: "db:lease_contract".to_string(),
            source_kind: "database".to_string(),
            title: "租赁合同".to_string(),
            metadata: json!({"parse_metadata": {
                "source_system_key": "fixture-db",
                "schema": "public",
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
                "source_system_key": "fixture-db",
                "schema": "public",
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
    fn document_derived_assets_are_not_counted_as_independent_semantic_sources() {
        let document_id = Uuid::from_u128(42).to_string();
        let asset = storage::AssetItemRecord {
            id: Uuid::from_u128(43),
            tenant_id: TenantId(Uuid::from_u128(2)),
            asset_library_id: None,
            collection_id: None,
            external_id: None,
            title: "派生文档画像".to_string(),
            asset_kind: "document".to_string(),
            source_kind: "document".to_string(),
            source_id: Some(document_id.clone()),
            content_type: Some("application/pdf".to_string()),
            object_key: None,
            metadata: json!({"document_id": document_id}),
            profile_count: 1,
            created_at: fixture_input().generated_at,
            updated_at: fixture_input().generated_at,
        };
        let scoped = BTreeSet::from([Uuid::from_u128(42).to_string()]);

        assert!(asset_is_derived_from_documents(&asset, &scoped));

        let mut standalone = asset;
        standalone.asset_kind = "image".to_string();
        standalone.source_kind = "asset_import".to_string();
        standalone.source_id = None;
        standalone.metadata = json!({});
        assert!(!asset_is_derived_from_documents(&standalone, &scoped));
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
            semantic_snapshot_build_action(Some(&existing), &"a".repeat(64), "semantic_profile_v4"),
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
            dataset_scope: None,
            documents: vec![
                crate::dataset_semantic_source_support::SemanticDocumentSourceVersion {
                    document_id: DocumentId(Uuid::from_u128(2)),
                    source_dataset_id: DatasetId(Uuid::from_u128(1)),
                    owner_user_id: None,
                    secret_binding_ids: Vec::new(),
                    updated_at: fixture_input().generated_at,
                    parse_versions: vec!["parser-v1".to_string()],
                    fact_snapshot_versions: vec!["facts-v7".to_string()],
                },
            ],
            assets: Vec::new(),
            memberships: Vec::new(),
            source_identities: Vec::new(),
            dataset_fact_snapshot_version: Some("v7".to_string()),
            dictionary_versions: vec!["dictionary-v1".to_string()],
        };
        assert_eq!(
            crate::dataset_semantic_source_support::source_fingerprint(&source).len(),
            64
        );
    }

    #[test]
    fn semantic_link_enqueue_plan_is_same_tenant_exact_allowlist_and_pair_idempotent() {
        let tenant_id = TenantId(Uuid::from_u128(1));
        let current = ReadyDatasetSemanticLinkEndpoint {
            tenant_id,
            dataset_id: DatasetId(Uuid::from_u128(10)),
            snapshot_id: Uuid::from_u128(100),
            source_fingerprint: "left-source-v1".to_string(),
        };
        let allowed_peer = ReadyDatasetSemanticLinkEndpoint {
            tenant_id,
            dataset_id: DatasetId(Uuid::from_u128(20)),
            snapshot_id: Uuid::from_u128(200),
            source_fingerprint: "right-source-v1".to_string(),
        };
        let denied_peer = ReadyDatasetSemanticLinkEndpoint {
            tenant_id,
            dataset_id: DatasetId(Uuid::from_u128(30)),
            snapshot_id: Uuid::from_u128(300),
            source_fingerprint: "denied-source-v1".to_string(),
        };
        let other_tenant_peer = ReadyDatasetSemanticLinkEndpoint {
            tenant_id: TenantId(Uuid::from_u128(2)),
            ..allowed_peer.clone()
        };
        let tenant_allowlist = tenant_id.to_string();
        let dataset_allowlist = format!("{},{}", current.dataset_id, allowed_peer.dataset_id);
        let planned = plan_dataset_semantic_link_runs_from_values(
            true,
            Some(&tenant_allowlist),
            Some(&dataset_allowlist),
            &current,
            &[
                denied_peer,
                allowed_peer.clone(),
                other_tenant_peer,
                allowed_peer.clone(),
                current.clone(),
            ],
            fixture_input().generated_at,
        );

        assert_eq!(planned.len(), 1);
        assert_eq!(planned[0].left_dataset_id, current.dataset_id);
        assert_eq!(planned[0].right_dataset_id, allowed_peer.dataset_id);
        assert_eq!(planned[0].left_snapshot_id, current.snapshot_id);
        assert_eq!(planned[0].right_snapshot_id, allowed_peer.snapshot_id);
        assert_eq!(planned[0].max_attempts, 3);
    }

    #[test]
    fn semantic_link_pair_identity_changes_with_any_single_snapshot_identity() {
        let left_dataset_id = DatasetId(Uuid::from_u128(10));
        let right_dataset_id = DatasetId(Uuid::from_u128(20));
        let left_snapshot_id = Uuid::from_u128(100);
        let right_snapshot_id = Uuid::from_u128(200);
        let original = dataset_semantic_link_source_fingerprint(
            left_dataset_id,
            right_dataset_id,
            left_snapshot_id,
            right_snapshot_id,
            "left-source-v1",
            "right-source-v1",
        );
        let reversed = dataset_semantic_link_source_fingerprint(
            right_dataset_id,
            left_dataset_id,
            right_snapshot_id,
            left_snapshot_id,
            "right-source-v1",
            "left-source-v1",
        );
        let changed = dataset_semantic_link_source_fingerprint(
            left_dataset_id,
            right_dataset_id,
            Uuid::from_u128(101),
            right_snapshot_id,
            "left-source-v1",
            "right-source-v1",
        );
        let source_changed = dataset_semantic_link_source_fingerprint(
            left_dataset_id,
            right_dataset_id,
            left_snapshot_id,
            right_snapshot_id,
            "left-source-v2",
            "right-source-v1",
        );

        assert_eq!(original, reversed);
        assert_ne!(original, changed);
        assert_ne!(original, source_changed);
        assert_eq!(original.len(), 64);
    }
}
