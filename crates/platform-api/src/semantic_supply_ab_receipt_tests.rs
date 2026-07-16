//! Test-only exporter for offline semantic-supply A/B/C retrieval receipts.
//!
//! The exporter consumes only the independent runtime-input corpus. It accepts
//! the oracle's SHA-256 digest as an environment value for experiment linkage,
//! but never opens the oracle file. It calls the production semantic planner
//! and retrieval ranker, records only supplied evidence, and never produces or
//! evaluates an answer, intent, route, action, workflow, or artifact.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::{self, BufWriter, Write as _};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use chrono::{DateTime, Utc};
use domain_model::{
    DatasetId, DocumentChunkId, DocumentId, RetrievalEvidence, RetrievalEvidenceId, TenantId,
    WorkflowExecutionId,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::assistant_run_model_supply_item_support::assistant_run_model_supply_item_for_context;
use crate::assistant_run_semantic_supply_support::{
    build_assistant_semantic_supply_plan, semantic_supply_visible_document_ids,
    AssistantSemanticSupplyMode,
};
use crate::semantic_understanding::{
    DatasetSemanticIdentity, DatasetSemanticUnderstanding, SemanticCoverage, SemanticEvidenceRef,
    SemanticObject, SemanticStatus, SemanticSummary, SemanticTruncation,
    DATASET_SEMANTIC_GENERATION_VERSION, DATASET_SEMANTIC_SCHEMA_VERSION,
};

const RUNTIME_INPUT_SCHEMA: &str = "semantic-supply-runtime-input.v2";
const RESULT_SCHEMA: &str = "semantic-supply-ab-result.v2";
const EXPORTER_VERSION: &str = "platform-api-rust-offline-v2";
const EXECUTION_KIND: &str = "rust_offline_retrieval";
const EXPECTED_RUNTIME_INPUT_NAME: &str = "runtime-inputs.jsonl";
const RANKING_LIMIT: usize = 5;
const SUPPLEMENT_LIMIT: usize = 2;

const RUNTIME_INPUT_ENV: &str = "SEMANTIC_SUPPLY_AB_RUNTIME_INPUT";
const OUTPUT_DIR_ENV: &str = "SEMANTIC_SUPPLY_AB_OUTPUT_DIR";
const ORACLE_SHA_ENV: &str = "SEMANTIC_SUPPLY_AB_ORACLE_SHA256";
const RUN_NONCE_ENV: &str = "SEMANTIC_SUPPLY_AB_RUN_NONCE";
const GIT_HEAD_ENV: &str = "SEMANTIC_SUPPLY_AB_GIT_HEAD";

type ExportResult<T> = Result<T, Box<dyn Error>>;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum RuntimeSupplyKind {
    Document,
    StructuredQuery,
    StructuredRowScan,
    ParseStatus,
    OrdinaryKnowledge,
}

impl RuntimeSupplyKind {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Document => "document",
            Self::StructuredQuery => "structured_query",
            Self::StructuredRowScan => "structured_row_scan",
            Self::ParseStatus => "parse_status",
            Self::OrdinaryKnowledge => "ordinary_knowledge",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(transparent)]
struct RequiredNullableString(Option<String>);

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeCandidateInput {
    candidate_id: String,
    source_id: String,
    supply_kind: RuntimeSupplyKind,
    document_id: RequiredNullableString,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum RuntimeEvidenceClass {
    Confirmed,
    Observed,
}

impl RuntimeEvidenceClass {
    fn semantic_status(self) -> SemanticStatus {
        match self {
            Self::Confirmed => SemanticStatus::Confirmed,
            Self::Observed => SemanticStatus::Observed,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum RuntimeSemanticRole {
    Field,
    RelationKey,
    DocumentTopic,
    GuardEvidence,
}

impl RuntimeSemanticRole {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Field => "field",
            Self::RelationKey => "relation_key",
            Self::DocumentTopic => "document_topic",
            Self::GuardEvidence => "guard_evidence",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeSemanticNodeInput {
    node_id: String,
    canonical_label: String,
    technical_names: Vec<String>,
    semantic_role: RuntimeSemanticRole,
    evidence_class: RuntimeEvidenceClass,
    source_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeSemanticSnapshotInput {
    snapshot_key: String,
    version: u64,
    status: String,
    stale: bool,
    compatible: bool,
    source_fingerprint_sha256: String,
    nodes: Vec<RuntimeSemanticNodeInput>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeDatasetInput {
    schema_version: String,
    record_kind: String,
    dataset_key: String,
    candidate_pool: Vec<RuntimeCandidateInput>,
    semantic_snapshot: RuntimeSemanticSnapshotInput,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeSelectedScope {
    selected_dataset_keys: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimePermission {
    tenant_key: String,
    visible_dataset_keys: Vec<String>,
    allowed_evidence_classes: Vec<RuntimeEvidenceClass>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum RuntimeEvaluationKind {
    RetrievalEvidence,
    StructuredQuery,
    StructuredRowScan,
    ParseStatus,
    OrdinaryChat,
}

impl RuntimeEvaluationKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::RetrievalEvidence => "retrieval_evidence",
            Self::StructuredQuery => "structured_query",
            Self::StructuredRowScan => "structured_row_scan",
            Self::ParseStatus => "parse_status",
            Self::OrdinaryChat => "ordinary_chat",
        }
    }

    fn required_supply_kind(self) -> Option<RuntimeSupplyKind> {
        match self {
            Self::RetrievalEvidence => None,
            Self::StructuredQuery => Some(RuntimeSupplyKind::StructuredQuery),
            Self::StructuredRowScan => Some(RuntimeSupplyKind::StructuredRowScan),
            Self::ParseStatus => Some(RuntimeSupplyKind::ParseStatus),
            Self::OrdinaryChat => Some(RuntimeSupplyKind::OrdinaryKnowledge),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeCaseInput {
    schema_version: String,
    record_kind: String,
    case_id: String,
    question: String,
    selected_scope: RuntimeSelectedScope,
    permission: RuntimePermission,
    evaluation_kind: RuntimeEvaluationKind,
}

#[derive(Debug)]
struct LoadedRuntimeInput {
    raw_records: Vec<Value>,
    datasets: BTreeMap<String, RuntimeDatasetInput>,
    cases: Vec<RuntimeCaseInput>,
}

#[derive(Clone, Debug)]
struct PreparedCandidate {
    runtime: RuntimeCandidateInput,
    evidence: RetrievalEvidence,
}

#[derive(Clone, Debug)]
struct PreparedDataset {
    tenant_id: TenantId,
    dataset_id: DatasetId,
    snapshot_id: Uuid,
    semantic_understanding: DatasetSemanticUnderstanding,
    candidates: Vec<PreparedCandidate>,
}

#[derive(Clone, Debug)]
struct PreparedCase {
    case_input: RuntimeCaseInput,
    datasets: Vec<PreparedDataset>,
    candidate_ids: Vec<String>,
    candidate_snapshot_sha256: String,
    semantic_snapshot_sha256: String,
    scope_sha256: String,
    permission_sha256: String,
    evaluation_kind_sha256: String,
    runtime_input_sha256: String,
    question_sha256: String,
}

#[derive(Clone, Debug)]
struct HitCore {
    candidate_id: String,
    source_id: String,
    dataset_id: String,
    retrieval_evidence_id: String,
    document_id: String,
    document_chunk_id: String,
    source_locator: String,
    model_visible_item: Value,
}

#[derive(Debug)]
struct ArmExecution {
    hits: Vec<HitCore>,
    latency_ms: f64,
    rank_change_count: usize,
    supplement_count: usize,
    same_run_off_path_self_consistency: bool,
}

fn invalid_data(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::new(io::ErrorKind::InvalidData, message.into()))
}

fn required_env(name: &str) -> ExportResult<String> {
    let value = std::env::var(name)
        .map_err(|_| invalid_data(format!("required environment variable {name} is missing")))?;
    let value = value.trim();
    if value.is_empty() {
        return Err(invalid_data(format!(
            "required environment variable {name} is empty"
        )));
    }
    Ok(value.to_string())
}

fn validate_hex(value: &str, length: usize, label: &str) -> ExportResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.len() != length || !normalized.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid_data(format!(
            "{label} must be exactly {length} hexadecimal characters"
        )));
    }
    Ok(normalized)
}

fn canonical_json(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => serde_json::to_string(value)
            .expect("serializing a JSON string into a String cannot fail"),
        Value::Array(values) => {
            let values = values
                .iter()
                .map(canonical_json)
                .collect::<Vec<_>>()
                .join(",");
            format!("[{values}]")
        }
        Value::Object(values) => {
            let mut keys = values.keys().collect::<Vec<_>>();
            keys.sort();
            let fields = keys
                .into_iter()
                .map(|key| {
                    let encoded_key = serde_json::to_string(key)
                        .expect("serializing a JSON object key cannot fail");
                    format!("{encoded_key}:{}", canonical_json(&values[key]))
                })
                .collect::<Vec<_>>()
                .join(",");
            format!("{{{fields}}}")
        }
    }
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(64);
    for byte in digest {
        write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    encoded
}

fn sha256_text(value: &str) -> String {
    sha256_bytes(value.as_bytes())
}

fn sha256_json(value: &Value) -> String {
    sha256_text(&canonical_json(value))
}

fn stable_uuid(namespace: &str, value: &str) -> Uuid {
    let mut digest = Sha256::new();
    digest.update(b"semantic-supply-ab-offline");
    digest.update([0]);
    digest.update(namespace.as_bytes());
    digest.update([0]);
    digest.update(value.as_bytes());
    let digest = digest.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

fn runtime_key_is_forbidden(key: &str) -> bool {
    let key = key.trim().to_ascii_lowercase();
    key.starts_with("expected_")
        || key.starts_with("forbidden_")
        || matches!(
            key.as_str(),
            "answer"
                | "answer_pattern"
                | "answer_template"
                | "artifact"
                | "artifacts"
                | "conversation"
                | "graph_target"
                | "intent"
                | "messages"
                | "prompt"
                | "route"
                | "action"
                | "actions"
                | "split"
                | "split_group"
                | "overlap_group"
                | "system_prompt"
        )
}

fn reject_oracle_or_orchestration_fields(value: &Value, path: &str) -> ExportResult<()> {
    match value {
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                reject_oracle_or_orchestration_fields(value, &format!("{path}[{index}]"))?;
            }
        }
        Value::Object(values) => {
            for (key, value) in values {
                if runtime_key_is_forbidden(key) {
                    return Err(invalid_data(format!(
                        "runtime input contains forbidden oracle/orchestration field {path}.{key}"
                    )));
                }
                reject_oracle_or_orchestration_fields(value, &format!("{path}.{key}"))?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn repository_root() -> ExportResult<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| invalid_data("cannot resolve repository root from CARGO_MANIFEST_DIR"))
}

fn git_stdout(repo_root: &Path, args: &[&str]) -> ExportResult<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(args)
        .output()
        .map_err(|error| {
            invalid_data(format!("cannot execute git for receipt binding: {error}"))
        })?;
    if !output.status.success() {
        return Err(invalid_data(format!(
            "git receipt binding command failed: git {}",
            args.join(" ")
        )));
    }
    String::from_utf8(output.stdout)
        .map_err(|_| invalid_data("git receipt binding output is not valid UTF-8"))
}

/// Derives the receipt source revision from Git itself and refuses to export
/// from a dirty worktree. `SEMANTIC_SUPPLY_AB_GIT_HEAD`, when present, is only
/// an additional equality assertion; it can never supply or override the
/// recorded revision.
fn clean_repository_git_head(repo_root: &Path) -> ExportResult<String> {
    let discovered_root = git_stdout(repo_root, &["rev-parse", "--show-toplevel"])?;
    let discovered_root = fs::canonicalize(discovered_root.trim()).map_err(|error| {
        invalid_data(format!(
            "cannot canonicalize Git-discovered repository root: {error}"
        ))
    })?;
    let expected_root = fs::canonicalize(repo_root).map_err(|error| {
        invalid_data(format!(
            "cannot canonicalize expected repository root: {error}"
        ))
    })?;
    if discovered_root != expected_root {
        return Err(invalid_data(
            "Git-discovered repository root does not match CARGO_MANIFEST_DIR",
        ));
    }

    let git_head = validate_hex(
        git_stdout(repo_root, &["rev-parse", "HEAD"])?.trim(),
        40,
        "actual repository HEAD",
    )?;
    let status = git_stdout(
        repo_root,
        &["status", "--porcelain=v1", "--untracked-files=normal"],
    )?;
    if !status.trim().is_empty() {
        return Err(invalid_data(
            "semantic supply receipt export requires a clean Git worktree",
        ));
    }

    if let Ok(asserted_head) = std::env::var(GIT_HEAD_ENV) {
        if !asserted_head.trim().is_empty() {
            let asserted_head = validate_hex(&asserted_head, 40, GIT_HEAD_ENV)?;
            if asserted_head != git_head {
                return Err(invalid_data(format!(
                    "{GIT_HEAD_ENV} does not match the actual repository HEAD"
                )));
            }
        }
    }
    Ok(git_head)
}

fn reject_parent_components(path: &Path, label: &str) -> ExportResult<()> {
    if path
        .components()
        .any(|component| component == Component::ParentDir)
    {
        return Err(invalid_data(format!(
            "{label} must not contain parent-directory components"
        )));
    }
    Ok(())
}

fn resolve_runtime_input_path(repo_root: &Path) -> ExportResult<PathBuf> {
    let raw = PathBuf::from(required_env(RUNTIME_INPUT_ENV)?);
    reject_parent_components(&raw, RUNTIME_INPUT_ENV)?;
    let path = if raw.is_absolute() {
        raw
    } else {
        repo_root.join(raw)
    };
    let canonical = fs::canonicalize(&path).map_err(|error| {
        invalid_data(format!(
            "cannot canonicalize runtime input {}: {error}",
            path.display()
        ))
    })?;
    let allowed_root = fs::canonicalize(repo_root.join("fixtures").join("semantic-supply-ab"))
        .map_err(|error| {
            invalid_data(format!(
                "cannot canonicalize semantic-supply fixture root: {error}"
            ))
        })?;
    if !canonical.starts_with(&allowed_root)
        || canonical.file_name().and_then(|value| value.to_str())
            != Some(EXPECTED_RUNTIME_INPUT_NAME)
    {
        return Err(invalid_data(format!(
            "runtime input must be the dedicated {EXPECTED_RUNTIME_INPUT_NAME} under {}",
            allowed_root.display()
        )));
    }
    Ok(canonical)
}

fn resolve_output_dir(repo_root: &Path) -> ExportResult<PathBuf> {
    let raw = PathBuf::from(required_env(OUTPUT_DIR_ENV)?);
    reject_parent_components(&raw, OUTPUT_DIR_ENV)?;
    let path = if raw.is_absolute() {
        raw
    } else {
        repo_root.join(raw)
    };
    let allowed_root = repo_root.join("target").join("semantic-supply-ab");
    fs::create_dir_all(&allowed_root)?;
    if path.parent() != Some(allowed_root.as_path()) || path.file_name().is_none() {
        return Err(invalid_data(format!(
            "output directory must be a unique direct child of {}",
            allowed_root.display()
        )));
    }
    Ok(path)
}

fn load_runtime_input(path: &Path) -> ExportResult<LoadedRuntimeInput> {
    let content = fs::read_to_string(path)?;
    let mut raw_records = Vec::new();
    let mut datasets = BTreeMap::new();
    let mut cases = Vec::new();
    let mut case_ids = BTreeSet::new();

    for (offset, raw_line) in content.lines().enumerate() {
        if raw_line.trim().is_empty() {
            continue;
        }
        let line = offset + 1;
        let value = serde_json::from_str::<Value>(raw_line).map_err(|error| {
            invalid_data(format!(
                "runtime input line {line} is invalid JSON: {error}"
            ))
        })?;
        reject_oracle_or_orchestration_fields(&value, &format!("line[{line}]"))?;
        let record_kind = value
            .get("record_kind")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid_data(format!("runtime input line {line} has no record_kind")))?;

        match record_kind {
            "dataset" => {
                let dataset = serde_json::from_value::<RuntimeDatasetInput>(value.clone())
                    .map_err(|error| {
                        invalid_data(format!(
                            "runtime dataset line {line} does not match the strict schema: {error}"
                        ))
                    })?;
                validate_dataset_input(&dataset, line)?;
                if datasets
                    .insert(dataset.dataset_key.clone(), dataset)
                    .is_some()
                {
                    return Err(invalid_data(format!(
                        "runtime input line {line} duplicates a dataset_key"
                    )));
                }
            }
            "case" => {
                let case =
                    serde_json::from_value::<RuntimeCaseInput>(value.clone()).map_err(|error| {
                        invalid_data(format!(
                            "runtime case line {line} does not match the strict schema: {error}"
                        ))
                    })?;
                validate_case_input(&case, line)?;
                if !case_ids.insert(case.case_id.clone()) {
                    return Err(invalid_data(format!(
                        "runtime input line {line} duplicates case_id {}",
                        case.case_id
                    )));
                }
                cases.push(case);
            }
            other => {
                return Err(invalid_data(format!(
                    "runtime input line {line} has unsupported record_kind {other}"
                )));
            }
        }
        raw_records.push(value);
    }

    if datasets.is_empty() || cases.is_empty() {
        return Err(invalid_data(
            "runtime input must contain dataset and case records",
        ));
    }
    Ok(LoadedRuntimeInput {
        raw_records,
        datasets,
        cases,
    })
}

fn validate_dataset_input(dataset: &RuntimeDatasetInput, line: usize) -> ExportResult<()> {
    if dataset.schema_version != RUNTIME_INPUT_SCHEMA || dataset.record_kind != "dataset" {
        return Err(invalid_data(format!(
            "runtime dataset line {line} has an invalid schema or record kind"
        )));
    }
    if !dataset.dataset_key.starts_with("synthetic-") || dataset.candidate_pool.is_empty() {
        return Err(invalid_data(format!(
            "runtime dataset line {line} requires a synthetic key and candidates"
        )));
    }
    let mut candidate_ids = BTreeSet::new();
    let mut source_ids = BTreeSet::new();
    for candidate in &dataset.candidate_pool {
        if candidate.candidate_id.trim().is_empty()
            || !candidate_ids.insert(candidate.candidate_id.clone())
            || !candidate.source_id.starts_with("synthetic://visible/")
            || !source_ids.insert(candidate.source_id.clone())
        {
            return Err(invalid_data(format!(
                "runtime dataset line {line} has an invalid or duplicate candidate/source"
            )));
        }
        match (&candidate.supply_kind, &candidate.document_id.0) {
            (RuntimeSupplyKind::Document, Some(document_id)) if !document_id.trim().is_empty() => {}
            (RuntimeSupplyKind::Document, _) => {
                return Err(invalid_data(format!(
                    "runtime dataset line {line} document supply requires document_id"
                )));
            }
            (_, None) => {}
            (_, Some(_)) => {
                return Err(invalid_data(format!(
                    "runtime dataset line {line} non-document supply must use null document_id"
                )));
            }
        }
    }

    let snapshot = &dataset.semantic_snapshot;
    validate_hex(
        &snapshot.source_fingerprint_sha256,
        64,
        "semantic source fingerprint",
    )?;
    if snapshot.snapshot_key.trim().is_empty()
        || snapshot.version == 0
        || snapshot.status != "ready"
        || snapshot.stale
        || !snapshot.compatible
    {
        return Err(invalid_data(format!(
            "runtime dataset line {line} semantic snapshot is not ready and compatible"
        )));
    }
    let mut node_ids = BTreeSet::new();
    for node in &snapshot.nodes {
        if node.node_id.trim().is_empty()
            || !node_ids.insert(node.node_id.clone())
            || node.canonical_label.trim().is_empty()
            || node
                .technical_names
                .iter()
                .any(|name| name.trim().is_empty())
            || node.source_ids.is_empty()
            || node
                .source_ids
                .iter()
                .any(|source_id| !source_ids.contains(source_id))
        {
            return Err(invalid_data(format!(
                "runtime dataset line {line} contains an invalid semantic node"
            )));
        }
    }
    Ok(())
}

fn validate_case_input(case: &RuntimeCaseInput, line: usize) -> ExportResult<()> {
    if case.schema_version != RUNTIME_INPUT_SCHEMA || case.record_kind != "case" {
        return Err(invalid_data(format!(
            "runtime case line {line} has an invalid schema or record kind"
        )));
    }
    if case.case_id.trim().is_empty()
        || case.question.trim().is_empty()
        || !case.permission.tenant_key.starts_with("synthetic-")
        || case.selected_scope.selected_dataset_keys.is_empty()
    {
        return Err(invalid_data(format!(
            "runtime case line {line} has invalid identity, question, tenant, or scope"
        )));
    }
    let mut dataset_keys = BTreeSet::new();
    if case
        .selected_scope
        .selected_dataset_keys
        .iter()
        .any(|key| !key.starts_with("synthetic-") || !dataset_keys.insert(key.clone()))
    {
        return Err(invalid_data(format!(
            "runtime case line {line} has an invalid or duplicate selected dataset"
        )));
    }
    if case.permission.visible_dataset_keys != case.selected_scope.selected_dataset_keys
        || case.permission.allowed_evidence_classes
            != [
                RuntimeEvidenceClass::Confirmed,
                RuntimeEvidenceClass::Observed,
            ]
    {
        return Err(invalid_data(format!(
            "runtime case line {line} permission does not exactly match the selected safe scope"
        )));
    }
    Ok(())
}

fn build_retrieval_evidence(
    tenant_id: TenantId,
    dataset_id: DatasetId,
    dataset_key: &str,
    candidate: &RuntimeCandidateInput,
    index: usize,
) -> RetrievalEvidence {
    let document_material = candidate
        .document_id
        .0
        .as_deref()
        .unwrap_or(&candidate.source_id);
    let document_id = DocumentId(stable_uuid(
        "document",
        &format!("{dataset_key}\0{document_material}"),
    ));
    let source_terms = candidate
        .source_id
        .trim_start_matches("synthetic://visible/")
        .replace(['/', '-', '_'], " ");
    RetrievalEvidence {
        id: RetrievalEvidenceId(stable_uuid("evidence", &candidate.candidate_id)),
        tenant_id,
        dataset_id,
        execution_id: WorkflowExecutionId(stable_uuid("execution", dataset_key)),
        document_id,
        document_chunk_id: DocumentChunkId(stable_uuid("chunk", &candidate.candidate_id)),
        chunk_index: index as i32,
        source_locator: candidate.source_id.clone(),
        content_excerpt: source_terms.clone(),
        summary: format!(
            "synthetic visible {} supply: {source_terms}",
            candidate.supply_kind.as_str()
        ),
        payload_filter_key: candidate.candidate_id.clone(),
        embedding_model: "offline-runtime-input".to_string(),
        recall_score: (10_000usize.saturating_sub(index) as f64) / 10_000.0,
        evidence_manifest: json!({
            "recall": { "rank_hint": index },
            "runtime_supply_kind": candidate.supply_kind.as_str(),
        }),
        created_at: DateTime::<Utc>::from_timestamp(index as i64, 0)
            .expect("small non-negative fixture timestamps are valid"),
    }
}

fn build_semantic_understanding(
    dataset: &RuntimeDatasetInput,
    dataset_id: DatasetId,
    candidates: &[PreparedCandidate],
) -> DatasetSemanticUnderstanding {
    let document_by_source = candidates
        .iter()
        .map(|candidate| {
            (
                candidate.runtime.source_id.as_str(),
                candidate.evidence.document_id,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let objects = dataset
        .semantic_snapshot
        .nodes
        .iter()
        .map(|node| SemanticObject {
            id: node.node_id.clone(),
            kind: node.semantic_role.as_str().to_string(),
            label: node.canonical_label.clone(),
            technical_name: node.technical_names.first().cloned().unwrap_or_default(),
            description: String::new(),
            label_source: "offline_runtime_input".to_string(),
            confidence: match node.evidence_class {
                RuntimeEvidenceClass::Confirmed => 1.0,
                RuntimeEvidenceClass::Observed => 0.9,
            },
            coverage_count: node.source_ids.len() as u64,
            status: node.evidence_class.semantic_status(),
            evidence_refs: node
                .source_ids
                .iter()
                .filter_map(|source_id| document_by_source.get(source_id.as_str()).copied())
                .map(|document_id| SemanticEvidenceRef {
                    source_kind: "document".to_string(),
                    source_id: document_id.to_string(),
                    label: node.canonical_label.clone(),
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    DatasetSemanticUnderstanding {
        schema_version: DATASET_SEMANTIC_SCHEMA_VERSION.to_string(),
        generation_version: DATASET_SEMANTIC_GENERATION_VERSION.to_string(),
        status: "ready".to_string(),
        dataset: DatasetSemanticIdentity {
            id: dataset_id,
            title: dataset.dataset_key.clone(),
        },
        coverage: SemanticCoverage {
            source_count: candidates.len() as u64,
            document_count: candidates.len() as u64,
            retrieval_evidence_count: candidates.len() as u64,
            confirmed_fact_count: dataset
                .semantic_snapshot
                .nodes
                .iter()
                .filter(|node| node.evidence_class == RuntimeEvidenceClass::Confirmed)
                .count() as u64,
            ..SemanticCoverage::default()
        },
        summary: SemanticSummary {
            headline: String::new(),
            limitations: Vec::new(),
        },
        objects,
        fields: Vec::new(),
        relations: Vec::new(),
        source_groups: Vec::new(),
        pipeline: Vec::new(),
        generated_at: DateTime::<Utc>::from_timestamp(0, 0)
            .expect("Unix epoch is a valid fixture timestamp"),
        stale: false,
        truncated: SemanticTruncation::default(),
    }
}

fn prepare_case(
    case: &RuntimeCaseInput,
    datasets: &BTreeMap<String, RuntimeDatasetInput>,
) -> ExportResult<PreparedCase> {
    let tenant_id = TenantId(stable_uuid("tenant", &case.permission.tenant_key));
    let required_supply_kind = case.evaluation_kind.required_supply_kind();
    let mut prepared_datasets = Vec::new();
    let mut candidate_ids = Vec::new();
    let mut seen_candidate_ids = BTreeSet::new();
    let mut candidate_snapshot = Vec::new();
    let mut semantic_snapshot = Vec::new();

    for dataset_key in &case.selected_scope.selected_dataset_keys {
        let dataset = datasets.get(dataset_key).ok_or_else(|| {
            invalid_data(format!(
                "case {} selects unknown dataset_key {dataset_key}",
                case.case_id
            ))
        })?;
        let dataset_id = DatasetId(stable_uuid("dataset", dataset_key));
        let all_candidates = dataset
            .candidate_pool
            .iter()
            .enumerate()
            .map(|(index, candidate)| PreparedCandidate {
                runtime: candidate.clone(),
                evidence: build_retrieval_evidence(
                    tenant_id,
                    dataset_id,
                    dataset_key,
                    candidate,
                    index,
                ),
            })
            .collect::<Vec<_>>();

        for candidate in &dataset.candidate_pool {
            if !seen_candidate_ids.insert(candidate.candidate_id.clone()) {
                return Err(invalid_data(format!(
                    "case {} has duplicate candidate_id {} across selected datasets",
                    case.case_id, candidate.candidate_id
                )));
            }
            candidate_ids.push(candidate.candidate_id.clone());
            candidate_snapshot.push(json!({
                "dataset_key": dataset_key,
                "candidate_id": &candidate.candidate_id,
                "source_id": &candidate.source_id,
                "supply_kind": candidate.supply_kind.as_str(),
                "document_id": &candidate.document_id,
            }));
        }
        semantic_snapshot.push(json!({
            "dataset_key": dataset_key,
            "semantic_snapshot": &dataset.semantic_snapshot,
        }));
        let semantic_understanding =
            build_semantic_understanding(dataset, dataset_id, &all_candidates);
        let candidates = all_candidates
            .into_iter()
            .filter(|candidate| {
                required_supply_kind
                    .as_ref()
                    .is_none_or(|required| &candidate.runtime.supply_kind == required)
            })
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            return Err(invalid_data(format!(
                "case {} has no visible candidate for evaluation kind {}",
                case.case_id,
                case.evaluation_kind.as_str()
            )));
        }
        prepared_datasets.push(PreparedDataset {
            tenant_id,
            dataset_id,
            snapshot_id: stable_uuid("snapshot", &dataset.semantic_snapshot.snapshot_key),
            semantic_understanding,
            candidates,
        });
    }

    let question_sha256 = sha256_text(&case.question);
    let scope_sha256 = sha256_json(&serde_json::to_value(&case.selected_scope)?);
    let permission_sha256 = sha256_json(&serde_json::to_value(&case.permission)?);
    let evaluation_kind_sha256 = sha256_text(case.evaluation_kind.as_str());
    let candidate_snapshot_sha256 = sha256_json(&Value::Array(candidate_snapshot));
    let semantic_snapshot_sha256 = sha256_json(&Value::Array(semantic_snapshot));
    let runtime_input_sha256 = sha256_json(&json!({
        "schema_version": RUNTIME_INPUT_SCHEMA,
        "case_id": &case.case_id,
        "question_sha256": &question_sha256,
        "scope_sha256": &scope_sha256,
        "permission_sha256": &permission_sha256,
        "evaluation_kind": case.evaluation_kind.as_str(),
        "evaluation_kind_sha256": &evaluation_kind_sha256,
        "candidate_snapshot_sha256": &candidate_snapshot_sha256,
        "semantic_snapshot_sha256": &semantic_snapshot_sha256,
    }));

    Ok(PreparedCase {
        case_input: case.clone(),
        datasets: prepared_datasets,
        candidate_ids,
        candidate_snapshot_sha256,
        semantic_snapshot_sha256,
        scope_sha256,
        permission_sha256,
        evaluation_kind_sha256,
        runtime_input_sha256,
        question_sha256,
    })
}

fn run_arm(
    prepared: &PreparedCase,
    requested_mode: AssistantSemanticSupplyMode,
) -> ExportResult<ArmExecution> {
    let started_at = Instant::now();
    let effective_mode =
        if prepared.case_input.evaluation_kind == RuntimeEvaluationKind::RetrievalEvidence {
            requested_mode
        } else {
            AssistantSemanticSupplyMode::Off
        };
    let mut hits = Vec::new();
    let mut seen_source_ids = BTreeSet::new();
    let mut rank_change_count = 0usize;
    let mut supplement_remaining = SUPPLEMENT_LIMIT;
    let mut supplement_count = 0usize;
    let mut same_run_off_path_self_consistency = true;

    for dataset in &prepared.datasets {
        let evidences = dataset
            .candidates
            .iter()
            .map(|candidate| candidate.evidence.clone())
            .collect::<Vec<_>>();
        let candidate_by_evidence_id = dataset
            .candidates
            .iter()
            .map(|candidate| (candidate.evidence.id.to_string(), candidate))
            .collect::<BTreeMap<_, _>>();
        let visible_document_ids =
            semantic_supply_visible_document_ids(dataset.tenant_id, dataset.dataset_id, &evidences);
        let plan = if effective_mode == AssistantSemanticSupplyMode::Off {
            None
        } else {
            build_assistant_semantic_supply_plan(
                dataset.snapshot_id,
                &prepared.case_input.question,
                &dataset.semantic_understanding,
                &visible_document_ids,
            )
        };
        let (ranked, changes, dataset_supplement_count) =
            if effective_mode == AssistantSemanticSupplyMode::Supplement {
                // Runtime input v2 exposes one already-authorized candidate universe. Passing it
                // independently as the base and recovery universe exercises the production
                // supplement path without inventing an oracle-derived pool split.
                super::rank_retrieval_evidences_for_semantic_supply_mode_with_supplement_candidates(
                    &evidences,
                    &evidences,
                    &prepared.case_input.question,
                    RANKING_LIMIT,
                    effective_mode,
                    plan.as_ref(),
                    supplement_remaining,
                )
            } else {
                let (ranked, changes) = super::rank_retrieval_evidences_for_semantic_supply_mode(
                    &evidences,
                    &prepared.case_input.question,
                    RANKING_LIMIT,
                    effective_mode,
                    plan.as_ref(),
                );
                (ranked, changes, 0)
            };
        rank_change_count += changes;
        supplement_remaining = supplement_remaining.saturating_sub(dataset_supplement_count);
        supplement_count += dataset_supplement_count;

        if requested_mode == AssistantSemanticSupplyMode::Off {
            let direct = super::rank_retrieval_evidences_for_prompt(
                &evidences,
                &prepared.case_input.question,
                RANKING_LIMIT,
            );
            same_run_off_path_self_consistency &= direct
                .iter()
                .map(|ranked| ranked.evidence.id)
                .eq(ranked.iter().map(|ranked| ranked.evidence.id));
        }

        for ranked in ranked {
            let evidence = ranked.evidence;
            let candidate = candidate_by_evidence_id
                .get(&evidence.id.to_string())
                .ok_or_else(|| {
                    invalid_data(format!(
                        "ranker returned evidence outside candidate snapshot for case {}",
                        prepared.case_input.case_id
                    ))
                })?;
            if !seen_source_ids.insert(candidate.runtime.source_id.clone()) {
                continue;
            }
            let ordinary_evidence = json!({
                "type": "retrieval_evidence",
                "dataset_id": evidence.dataset_id,
                "document_id": evidence.document_id,
                "document_chunk_id": evidence.document_chunk_id,
                "retrieval_evidence_id": evidence.id,
                "chunk_index": evidence.chunk_index,
                "source_locator": &evidence.source_locator,
                "summary": &evidence.summary,
                "content_excerpt": &evidence.content_excerpt,
                "payload_filter_key": &evidence.payload_filter_key,
                "score": ranked.score,
                "lexical_score": ranked.lexical_score,
                "recall_score": ranked.recall_score,
                "evidence_manifest": &evidence.evidence_manifest,
            });
            hits.push(HitCore {
                candidate_id: candidate.runtime.candidate_id.clone(),
                source_id: candidate.runtime.source_id.clone(),
                dataset_id: evidence.dataset_id.to_string(),
                retrieval_evidence_id: evidence.id.to_string(),
                document_id: candidate
                    .runtime
                    .document_id
                    .0
                    .clone()
                    .unwrap_or_else(|| evidence.document_id.to_string()),
                document_chunk_id: evidence.document_chunk_id.to_string(),
                source_locator: evidence.source_locator.clone(),
                model_visible_item: assistant_run_model_supply_item_for_context(&ordinary_evidence),
            });
        }
    }

    Ok(ArmExecution {
        hits,
        latency_ms: started_at.elapsed().as_secs_f64() * 1000.0,
        rank_change_count,
        supplement_count,
        same_run_off_path_self_consistency,
    })
}

fn grounding_receipt(prepared: &PreparedCase) -> Value {
    if prepared.case_input.evaluation_kind != RuntimeEvaluationKind::RetrievalEvidence {
        return not_applicable_grounding();
    }
    let mut matched_node_count = 0usize;
    for dataset in &prepared.datasets {
        let evidences = dataset
            .candidates
            .iter()
            .map(|candidate| candidate.evidence.clone())
            .collect::<Vec<_>>();
        let visible_document_ids =
            semantic_supply_visible_document_ids(dataset.tenant_id, dataset.dataset_id, &evidences);
        if let Some(plan) = build_assistant_semantic_supply_plan(
            dataset.snapshot_id,
            &prepared.case_input.question,
            &dataset.semantic_understanding,
            &visible_document_ids,
        ) {
            matched_node_count += plan.trace.matched_node_count;
        }
    }
    if matched_node_count == 0 {
        not_applicable_grounding()
    } else {
        json!({
            "status": "measured",
            "snapshot_status": "ready",
            "snapshot_stale": false,
            "snapshot_compatible": true,
            "target_node_count": matched_node_count,
            "resolved_target_node_count": matched_node_count,
            "confirmed_or_observed_used_count": matched_node_count,
            "inferred_used_count": 0,
            "unresolved_used_count": 0,
            "stale_used_count": 0,
            "hidden_source_use_count": 0,
            "unresolvable_source_use_count": 0,
        })
    }
}

fn not_applicable_grounding() -> Value {
    json!({
        "status": "not_applicable",
        "snapshot_status": "not_applicable",
        "snapshot_stale": false,
        "snapshot_compatible": true,
        "target_node_count": 0,
        "resolved_target_node_count": 0,
        "confirmed_or_observed_used_count": 0,
        "inferred_used_count": 0,
        "unresolved_used_count": 0,
        "stale_used_count": 0,
        "hidden_source_use_count": 0,
        "unresolvable_source_use_count": 0,
    })
}

#[allow(clippy::too_many_arguments)]
fn result_row(
    prepared: &PreparedCase,
    arm: &str,
    execution: &ArmExecution,
    rerank_candidate_ids: &BTreeSet<String>,
    experiment_id: &str,
    fixture_sha256: &str,
    producer: &Value,
    grounding: &Value,
) -> ExportResult<Value> {
    let retrieval_case =
        prepared.case_input.evaluation_kind == RuntimeEvaluationKind::RetrievalEvidence;
    let mut supplement_count = 0usize;
    let mut model_visible_items = Vec::new();
    let hits = execution
        .hits
        .iter()
        .enumerate()
        .map(|(index, hit)| {
            let selection_kind = if !retrieval_case || arm == "A" {
                "baseline"
            } else if arm == "C" && !rerank_candidate_ids.contains(&hit.candidate_id) {
                supplement_count += 1;
                "supplement"
            } else {
                "reranked"
            };
            model_visible_items.push(hit.model_visible_item.clone());
            json!({
                "rank": index + 1,
                "candidate_id": &hit.candidate_id,
                "source_id": &hit.source_id,
                "dataset_id": &hit.dataset_id,
                "selection_kind": selection_kind,
                "retrieval_evidence_id": &hit.retrieval_evidence_id,
                "document_id": &hit.document_id,
                "document_chunk_id": &hit.document_chunk_id,
                "source_locator": &hit.source_locator,
            })
        })
        .collect::<Vec<_>>();
    if supplement_count > SUPPLEMENT_LIMIT {
        return Err(invalid_data(format!(
            "case {} produced {supplement_count} supplements, exceeding the global limit {SUPPLEMENT_LIMIT}",
            prepared.case_input.case_id
        )));
    }
    if arm == "C" && supplement_count != execution.supplement_count {
        return Err(invalid_data(format!(
            "case {} receipt classified {supplement_count} supplements but the production ranker reported {}",
            prepared.case_input.case_id, execution.supplement_count
        )));
    }

    let model_visible_evidence = Value::Array(model_visible_items);
    let model_visible_evidence_json = canonical_json(&model_visible_evidence);
    let route = json!({
        "status": "not_run",
        "reason": "retrieval_only_exporter_does_not_compute_route"
    });
    let grounding_sha256 = sha256_json(grounding);
    let system_prompt_sha256 = sha256_text("not_run:retrieval_only_exporter");
    let answer_policy_sha256 = sha256_text("not_run:model_answers_not_evaluated");
    let provider_config_sha256 = sha256_text("not_run:no_provider_instantiated");
    let action_catalog_sha256 = sha256_text("not_run:no_actions_evaluated");
    let route_sha256 = sha256_json(&route);
    let immutable_components = json!({
        "question_sha256": &prepared.question_sha256,
        "system_prompt_sha256": &system_prompt_sha256,
        "answer_policy_sha256": &answer_policy_sha256,
        "provider_config_sha256": &provider_config_sha256,
        "selected_scope_sha256": &prepared.scope_sha256,
        "permission_sha256": &prepared.permission_sha256,
        "evaluation_kind_sha256": &prepared.evaluation_kind_sha256,
        "runtime_input_sha256": &prepared.runtime_input_sha256,
        "action_catalog_sha256": &action_catalog_sha256,
        "route_sha256": &route_sha256,
    });

    Ok(json!({
        "schema_version": RESULT_SCHEMA,
        "case_id": &prepared.case_input.case_id,
        "evaluation_kind": prepared.case_input.evaluation_kind.as_str(),
        "experiment_id": experiment_id,
        "arm": arm,
        "execution_kind": EXECUTION_KIND,
        "fixture_sha256": fixture_sha256,
        "runtime_input_sha256": &prepared.runtime_input_sha256,
        "question_sha256": &prepared.question_sha256,
        "candidate_snapshot_sha256": &prepared.candidate_snapshot_sha256,
        "semantic_snapshot_sha256": &prepared.semantic_snapshot_sha256,
        "scope_sha256": &prepared.scope_sha256,
        "permission_sha256": &prepared.permission_sha256,
        "grounding_sha256": grounding_sha256,
        "producer": producer,
        "latency_ms": execution.latency_ms,
        "rank_change_count": execution.rank_change_count,
        "candidate_ids": &prepared.candidate_ids,
        "input_candidate_count": prepared.candidate_ids.len(),
        "hits": hits,
        "citations": [],
        "route": route,
        "artifacts": [],
        "contract": {
            "question_sha256": &prepared.question_sha256,
            "system_prompt_sha256": system_prompt_sha256,
            "answer_policy_sha256": answer_policy_sha256,
            "provider_config_sha256": provider_config_sha256,
            "selected_scope_sha256": &prepared.scope_sha256,
            "permission_sha256": &prepared.permission_sha256,
            "evaluation_kind_sha256": &prepared.evaluation_kind_sha256,
            "runtime_input_sha256": &prepared.runtime_input_sha256,
            "action_catalog_sha256": action_catalog_sha256,
            "route_sha256": route_sha256,
            "immutable_component_sha256": sha256_json(&immutable_components),
            "model_visible_evidence_sha256": sha256_text(&model_visible_evidence_json),
            "model_visible_evidence_bytes": model_visible_evidence_json.len(),
            "provider_input_sha256": sha256_text("not_run:no_provider_input_built"),
            "provider_input_bytes": 0,
            "provider_call_count": 0,
            "provider_retry_count": 0,
            "workflow_delta_count": 0,
            "artifact_delta_count": 0,
            "report_delta_count": 0,
            "template_delta_count": 0,
            "static_page_delta_count": 0,
            "same_run_off_path_self_consistency": arm == "A"
                && execution.same_run_off_path_self_consistency,
        },
        "grounding": grounding,
        "permission_leaks": [],
        "answer_evaluation": {
            "status": "not_run",
            "reason": "offline_retrieval_only_no_provider"
        }
    }))
}

fn create_new_jsonl(path: &Path, rows: &[Value]) -> ExportResult<()> {
    let file = OpenOptions::new().write(true).create_new(true).open(path)?;
    let mut writer = BufWriter::new(file);
    for row in rows {
        serde_json::to_writer(&mut writer, row)?;
        writer.write_all(b"\n")?;
    }
    writer.flush()?;
    Ok(())
}

fn create_new_json(path: &Path, value: &Value) -> ExportResult<()> {
    let file = OpenOptions::new().write(true).create_new(true).open(path)?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, value)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

fn file_sha256(path: &Path) -> ExportResult<String> {
    Ok(sha256_bytes(&fs::read(path)?))
}

#[test]
fn canonical_json_hash_is_stable_and_key_order_independent() {
    let left = json!({"b": [2, 3], "a": "中文"});
    let right = json!({"a": "中文", "b": [2, 3]});
    assert_eq!(canonical_json(&left), canonical_json(&right));
    assert_eq!(sha256_json(&left), sha256_json(&right));
    assert_eq!(
        sha256_text("abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn runtime_input_rejects_oracle_and_orchestration_fields_recursively() {
    for value in [
        json!({"expected_sources": []}),
        json!({"nested": {"graph_target": true}}),
        json!({"nested": [{"route": "database"}]}),
        json!({"system_prompt": "do not accept"}),
    ] {
        assert!(reject_oracle_or_orchestration_fields(&value, "$runtime").is_err());
    }
    assert!(reject_oracle_or_orchestration_fields(
        &json!({"question": "普通问题", "source_id": "synthetic://visible/a"}),
        "$runtime"
    )
    .is_ok());
}

#[test]
fn stable_uuid_is_deterministic_and_namespaced() {
    assert_eq!(stable_uuid("dataset", "a"), stable_uuid("dataset", "a"));
    assert_ne!(stable_uuid("dataset", "a"), stable_uuid("document", "a"));
    assert_ne!(stable_uuid("dataset", "a"), stable_uuid("dataset", "b"));
}

#[test]
fn cross_dataset_receipt_keeps_one_global_supplement_budget() -> ExportResult<()> {
    let runtime_input_path = repository_root()?
        .join("fixtures")
        .join("semantic-supply-ab")
        .join(EXPECTED_RUNTIME_INPUT_NAME);
    let loaded = load_runtime_input(&runtime_input_path)?;
    let producer = json!({
        "kind": "platform-api-rust-fixture-export",
        "exporter_version": "cross-dataset-budget-test",
        "git_head": "0000000000000000000000000000000000000000",
    });
    let fixture_sha256 = "0".repeat(64);
    let mut checked_case_count = 0usize;

    for case in loaded
        .cases
        .iter()
        .filter(|case| case.selected_scope.selected_dataset_keys.len() == 2)
    {
        let prepared = prepare_case(case, &loaded.datasets)?;
        let rerank = run_arm(&prepared, AssistantSemanticSupplyMode::Rerank)?;
        let supplement = run_arm(&prepared, AssistantSemanticSupplyMode::Supplement)?;
        let rerank_candidate_ids = rerank
            .hits
            .iter()
            .map(|hit| hit.candidate_id.clone())
            .collect::<BTreeSet<_>>();
        let new_candidate_count = supplement
            .hits
            .iter()
            .filter(|hit| !rerank_candidate_ids.contains(&hit.candidate_id))
            .count();
        let grounding = grounding_receipt(&prepared);
        let receipt = result_row(
            &prepared,
            "C",
            &supplement,
            &rerank_candidate_ids,
            "cross-dataset-budget-test",
            &fixture_sha256,
            &producer,
            &grounding,
        )?;
        let classified_supplement_count = receipt["hits"]
            .as_array()
            .expect("receipt hits are an array")
            .iter()
            .filter(|hit| hit["selection_kind"] == "supplement")
            .count();

        assert_eq!(classified_supplement_count, new_candidate_count);
        assert_eq!(classified_supplement_count, supplement.supplement_count);
        assert!(classified_supplement_count <= SUPPLEMENT_LIMIT);
        checked_case_count += 1;
    }

    assert!(
        checked_case_count > 0,
        "runtime input has cross-dataset cases"
    );
    Ok(())
}

#[test]
#[ignore = "writes Task 8 offline retrieval-only A/B/C receipts"]
fn export_semantic_supply_ab_offline_receipts() -> ExportResult<()> {
    let repo_root = repository_root()?;
    let runtime_input_path = resolve_runtime_input_path(&repo_root)?;
    let output_dir = resolve_output_dir(&repo_root)?;
    let fixture_sha256 = validate_hex(&required_env(ORACLE_SHA_ENV)?, 64, ORACLE_SHA_ENV)?;
    let git_head = clean_repository_git_head(&repo_root)?;
    let run_nonce = required_env(RUN_NONCE_ENV)?;
    let loaded = load_runtime_input(&runtime_input_path)?;
    let runtime_input_file_sha256 = file_sha256(&runtime_input_path)?;
    let runtime_input_canonical_sha256 = sha256_json(&Value::Array(loaded.raw_records.clone()));
    let experiment_id = sha256_json(&json!({
        "exporter_version": EXPORTER_VERSION,
        "fixture_sha256": &fixture_sha256,
        "git_head": &git_head,
        "run_nonce_sha256": sha256_text(&run_nonce),
        "runtime_input_file_sha256": &runtime_input_file_sha256,
        "runtime_input_canonical_sha256": &runtime_input_canonical_sha256,
    }));
    let producer = json!({
        "kind": "platform-api-rust-fixture-export",
        "exporter_version": EXPORTER_VERSION,
        "git_head": git_head,
        "git_head_source": "git_rev_parse_head",
        "worktree_clean": true,
        "runtime_input_file_sha256": runtime_input_file_sha256,
        "runtime_input_canonical_sha256": runtime_input_canonical_sha256,
        "oracle_access": "sha256_only",
        "provider_execution": "not_run",
        "answer_evaluation": "not_run",
    });

    let mut a_rows = Vec::new();
    let mut b_rows = Vec::new();
    let mut c_rows = Vec::new();
    for case in &loaded.cases {
        let prepared = prepare_case(case, &loaded.datasets)?;
        let grounding = grounding_receipt(&prepared);
        let a = run_arm(&prepared, AssistantSemanticSupplyMode::Off)?;
        let b = run_arm(&prepared, AssistantSemanticSupplyMode::Rerank)?;
        let c = run_arm(&prepared, AssistantSemanticSupplyMode::Supplement)?;
        let rerank_candidate_ids = b
            .hits
            .iter()
            .map(|hit| hit.candidate_id.clone())
            .collect::<BTreeSet<_>>();

        a_rows.push(result_row(
            &prepared,
            "A",
            &a,
            &rerank_candidate_ids,
            &experiment_id,
            &fixture_sha256,
            &producer,
            &grounding,
        )?);
        b_rows.push(result_row(
            &prepared,
            "B",
            &b,
            &rerank_candidate_ids,
            &experiment_id,
            &fixture_sha256,
            &producer,
            &grounding,
        )?);
        c_rows.push(result_row(
            &prepared,
            "C",
            &c,
            &rerank_candidate_ids,
            &experiment_id,
            &fixture_sha256,
            &producer,
            &grounding,
        )?);
    }

    fs::create_dir_all(&output_dir)?;
    let a_path = output_dir.join("A.jsonl");
    let b_path = output_dir.join("B.jsonl");
    let c_path = output_dir.join("C.jsonl");
    create_new_jsonl(&a_path, &a_rows)?;
    create_new_jsonl(&b_path, &b_rows)?;
    create_new_jsonl(&c_path, &c_rows)?;
    let manifest_path = output_dir.join("experiment.json");
    create_new_json(
        &manifest_path,
        &json!({
            "schema_version": "semantic-supply-ab-offline-experiment.v1",
            "experiment_id": experiment_id,
            "fixture_sha256": fixture_sha256,
            "runtime_input_file_sha256": runtime_input_file_sha256,
            "runtime_input_canonical_sha256": runtime_input_canonical_sha256,
            "producer": producer,
            "case_count": loaded.cases.len(),
            "files": {
                "A.jsonl": file_sha256(&a_path)?,
                "B.jsonl": file_sha256(&b_path)?,
                "C.jsonl": file_sha256(&c_path)?,
            },
            "answer_evaluation": {
                "status": "not_run",
                "reason": "offline_retrieval_only_no_provider"
            },
            "decision_eligible": false,
        }),
    )?;

    println!(
        "semantic supply offline receipts exported: experiment_id={} cases={} output={}",
        experiment_id,
        loaded.cases.len(),
        output_dir.display()
    );
    Ok(())
}
