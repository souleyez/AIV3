use anyhow::{anyhow, Result};
use chrono::Utc;
use domain_model::{DatasetId, DocumentId, TenantId};
use retrieval_worker::dataset_semantic_understanding_access;
use serde_json::{json, Value};
use sqlx::Row;
use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
};
use storage::{NewDocumentEnrichmentRun, PgStorage, DEFAULT_LOCAL_DATABASE_URL};
use uuid::Uuid;

const ALL_ENRICHMENT_KINDS: &[&str] = &[
    "structure_outline_v1",
    "fact_index_v2",
    "qa_seed_v1",
    "entity_relation_v1",
    "table_structure_v1",
    "entity_terms_v1",
    "procedure_steps_v1",
    "resume_profile_v1",
    "spreadsheet_metrics_v1",
    "semantic_profile_v1",
];

#[derive(Clone, Debug, PartialEq, Eq)]
struct BackfillArgs {
    dataset_id: DatasetId,
    document_id: Option<DocumentId>,
    enrichment_kinds: Vec<&'static str>,
    limit: usize,
    explicit_limit: bool,
    dry_run: bool,
    confirm_real_run: bool,
    summary_only: bool,
    pretty: bool,
    priority: i32,
    max_attempts: i32,
}

#[derive(Clone, Debug)]
struct DocumentCandidate {
    id: DocumentId,
    dataset_id: DatasetId,
    title: String,
    content_sha256: Option<String>,
    parse_version: Option<String>,
}

#[derive(Clone, Debug)]
struct DocumentPlan {
    document: DocumentCandidate,
    existing_kinds: Vec<String>,
    missing_kinds: Vec<String>,
    skipped_reason: Option<String>,
    enqueued_count: usize,
}

fn usage(program: &str) -> String {
    format!(
        "Usage: {program} --dataset-id <uuid> --kind <kind[,kind]|all> [--document-id <uuid>] [--limit <n>] [--dry-run] [--summary-only] [--confirm-real-run] [--priority <n>] [--max-attempts <n>] [--pretty]"
    )
}

#[tokio::main]
async fn main() -> Result<()> {
    observability::install("document_enrichment_backfill")?;

    let program = std::env::args()
        .next()
        .unwrap_or_else(|| "document-enrichment-backfill".to_string());
    let args = parse_args(&program, std::env::args().skip(1).collect())?;
    let database_url = std::env::var("PLATFORM_DATABASE_URL")
        .unwrap_or_else(|_| DEFAULT_LOCAL_DATABASE_URL.into());
    let storage = PgStorage::connect(&database_url).await?;
    storage.migrate().await?;

    let tenant_key = std::env::var("PLATFORM_TENANT_KEY")
        .unwrap_or_else(|_| storage::DEFAULT_LOCAL_TENANT_KEY.to_string());
    let tenant_name = std::env::var("PLATFORM_TENANT_NAME")
        .unwrap_or_else(|_| storage::DEFAULT_LOCAL_TENANT_NAME.to_string());
    let tenant = storage.ensure_tenant(&tenant_key, &tenant_name).await?;
    if !args.dry_run && args.enrichment_kinds.contains(&"semantic_profile_v1") {
        let access = dataset_semantic_understanding_access(tenant.id, args.dataset_id);
        if !access.is_allowed() {
            anyhow::bail!(
                "semantic_profile_v1 denied by semantic rollout gate: {}",
                access.safe_reason()
            );
        }
    }

    let summary = run_document_enrichment_backfill(&storage, tenant.id, &args).await?;
    if args.pretty {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        println!("{}", serde_json::to_string(&summary)?);
    }

    Ok(())
}

fn parse_args(program: &str, mut args: Vec<String>) -> Result<BackfillArgs> {
    let pretty = remove_flag(&mut args, "--pretty");
    let dry_run = remove_flag(&mut args, "--dry-run");
    let confirm_real_run = remove_flag(&mut args, "--confirm-real-run");
    let summary_only = remove_flag(&mut args, "--summary-only");
    let dataset_id = take_option(&mut args, "--dataset-id")
        .ok_or_else(|| anyhow!(usage(program)))
        .and_then(|raw| parse_uuid_arg("dataset_id", &raw).map(DatasetId))?;
    let document_id = take_option(&mut args, "--document-id")
        .map(|raw| parse_uuid_arg("document_id", &raw).map(DocumentId))
        .transpose()?;
    let kind_csv = take_option(&mut args, "--kind").ok_or_else(|| anyhow!(usage(program)))?;
    let enrichment_kinds = enrichment_kinds_from_csv(&kind_csv);
    if enrichment_kinds.is_empty() {
        anyhow::bail!("--kind must include at least one supported enrichment kind");
    }
    let raw_limit = take_option(&mut args, "--limit");
    let explicit_limit = raw_limit.is_some();
    let limit = raw_limit
        .map(|raw| raw.parse::<usize>())
        .transpose()?
        .unwrap_or(50)
        .max(1);
    let priority = take_option(&mut args, "--priority")
        .map(|raw| raw.parse::<i32>())
        .transpose()?
        .unwrap_or(100)
        .clamp(1, 10_000);
    let max_attempts = take_option(&mut args, "--max-attempts")
        .map(|raw| raw.parse::<i32>())
        .transpose()?
        .unwrap_or(3)
        .clamp(1, 10);

    if !args.is_empty() {
        anyhow::bail!(usage(program));
    }
    if !dry_run && !confirm_real_run {
        anyhow::bail!(
            "real document enrichment backfill enqueue requires --confirm-real-run; rerun with --dry-run --summary-only first"
        );
    }
    if !dry_run && document_id.is_none() && (!explicit_limit || limit > 5) {
        anyhow::bail!(
            "dataset-level real document enrichment enqueue requires an explicit --limit no greater than 5"
        );
    }
    if !dry_run && document_id.is_none() && enrichment_kinds.len() != 1 {
        anyhow::bail!("dataset-level real document enrichment enqueue requires exactly one --kind");
    }

    Ok(BackfillArgs {
        dataset_id,
        document_id,
        enrichment_kinds,
        limit,
        explicit_limit,
        dry_run,
        confirm_real_run,
        summary_only,
        pretty,
        priority,
        max_attempts,
    })
}

fn remove_flag(args: &mut Vec<String>, flag: &str) -> bool {
    if let Some(index) = args.iter().position(|arg| arg == flag) {
        args.remove(index);
        true
    } else {
        false
    }
}

fn take_option(args: &mut Vec<String>, name: &str) -> Option<String> {
    let index = args.iter().position(|arg| arg == name)?;
    if index + 1 >= args.len() {
        return Some(String::new());
    }
    let value = args.remove(index + 1);
    args.remove(index);
    Some(value)
}

fn parse_uuid_arg(name: &str, raw: &str) -> Result<Uuid> {
    Uuid::from_str(raw).map_err(|error| anyhow!("invalid {name} {raw}: {error}"))
}

async fn run_document_enrichment_backfill(
    storage: &PgStorage,
    tenant_id: TenantId,
    args: &BackfillArgs,
) -> Result<Value> {
    let documents = load_backfill_documents(storage, tenant_id, args).await?;
    let mut plans = Vec::new();
    let mut missing_fingerprint_count = 0usize;
    let mut already_exists_count = 0usize;
    let mut would_enqueue_count = 0usize;
    let mut enqueued_count = 0usize;
    let mut existing_count_by_kind = BTreeMap::<String, usize>::new();
    let mut enqueue_count_by_kind = BTreeMap::<String, usize>::new();
    let now = Utc::now();

    for document in documents {
        let Some(input_fingerprint) = document.content_sha256.clone() else {
            missing_fingerprint_count += 1;
            plans.push(DocumentPlan {
                document,
                existing_kinds: Vec::new(),
                missing_kinds: Vec::new(),
                skipped_reason: Some("missing_content_fingerprint".to_string()),
                enqueued_count: 0,
            });
            continue;
        };
        let existing =
            load_existing_run_kinds(storage, tenant_id, document.id, &input_fingerprint).await?;
        let mut existing_kinds = Vec::new();
        let mut missing_kinds = Vec::new();
        let mut document_enqueued_count = 0usize;

        for kind in &args.enrichment_kinds {
            if existing.contains(*kind) {
                already_exists_count += 1;
                *existing_count_by_kind
                    .entry((*kind).to_string())
                    .or_insert(0) += 1;
                existing_kinds.push((*kind).to_string());
                continue;
            }

            would_enqueue_count += 1;
            *enqueue_count_by_kind
                .entry((*kind).to_string())
                .or_insert(0) += 1;
            missing_kinds.push((*kind).to_string());
            if !args.dry_run {
                storage
                    .document_enrichment_runs()
                    .create_or_get(
                        tenant_id,
                        &NewDocumentEnrichmentRun {
                            document_id: document.id,
                            enrichment_kind: (*kind).to_string(),
                            parse_version: document.parse_version.clone(),
                            input_fingerprint: input_fingerprint.clone(),
                            priority: args.priority,
                            max_attempts: args.max_attempts,
                            available_at: now,
                        },
                        now,
                    )
                    .await?;
                enqueued_count += 1;
                document_enqueued_count += 1;
            }
        }

        plans.push(DocumentPlan {
            document,
            existing_kinds,
            missing_kinds,
            skipped_reason: None,
            enqueued_count: document_enqueued_count,
        });
    }

    let mut output = json!({
        "dry_run": args.dry_run,
        "confirm_real_run": args.confirm_real_run,
        "dataset_id": args.dataset_id,
        "document_id": args.document_id,
        "limit": args.limit,
        "summary_only": args.summary_only,
        "enrichment_kinds": args.enrichment_kinds,
        "document_count": plans.len(),
        "missing_fingerprint_count": missing_fingerprint_count,
        "already_exists_count": already_exists_count,
        "would_enqueue_count": would_enqueue_count,
        "enqueued_count": enqueued_count,
        "existing_count_by_kind": existing_count_by_kind,
        "enqueue_count_by_kind": enqueue_count_by_kind,
        "priority": args.priority,
        "max_attempts": args.max_attempts,
    });
    if !args.summary_only {
        if let Some(object) = output.as_object_mut() {
            object.insert(
                "documents".to_string(),
                Value::Array(plans.iter().map(document_plan_to_json).collect()),
            );
        }
    }
    Ok(output)
}

async fn load_backfill_documents(
    storage: &PgStorage,
    tenant_id: TenantId,
    args: &BackfillArgs,
) -> Result<Vec<DocumentCandidate>> {
    let rows = sqlx::query(
        r#"
        select id, dataset_id, title, content_sha256, metadata
        from documents
        where tenant_id = $1
          and dataset_id = $2
          and ($3::uuid is null or id = $3)
        order by created_at desc, title asc
        limit $4
        "#,
    )
    .bind(tenant_id.0)
    .bind(args.dataset_id.0)
    .bind(args.document_id.map(|id| id.0))
    .bind(args.limit as i64)
    .fetch_all(storage.pool())
    .await?;
    if args.document_id.is_some() && rows.is_empty() {
        anyhow::bail!(
            "document {} not found in dataset {}",
            args.document_id.unwrap(),
            args.dataset_id
        );
    }

    Ok(rows
        .iter()
        .map(|row| {
            let metadata = row.get::<Value, _>("metadata");
            DocumentCandidate {
                id: DocumentId(row.get::<Uuid, _>("id")),
                dataset_id: DatasetId(row.get::<Uuid, _>("dataset_id")),
                title: row.get("title"),
                content_sha256: row
                    .get::<Option<String>, _>("content_sha256")
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty()),
                parse_version: document_enrichment_parse_version(&metadata),
            }
        })
        .collect())
}

async fn load_existing_run_kinds(
    storage: &PgStorage,
    tenant_id: TenantId,
    document_id: DocumentId,
    input_fingerprint: &str,
) -> Result<BTreeSet<String>> {
    let rows = sqlx::query(
        r#"
        select enrichment_kind
        from document_enrichment_runs
        where tenant_id = $1
          and document_id = $2
          and input_fingerprint = $3
        "#,
    )
    .bind(tenant_id.0)
    .bind(document_id.0)
    .bind(input_fingerprint)
    .fetch_all(storage.pool())
    .await?;

    Ok(rows
        .iter()
        .map(|row| row.get::<String, _>("enrichment_kind"))
        .collect())
}

fn document_plan_to_json(plan: &DocumentPlan) -> Value {
    json!({
        "document_id": plan.document.id,
        "dataset_id": plan.document.dataset_id,
        "title": plan.document.title,
        "has_content_fingerprint": plan.document.content_sha256.is_some(),
        "parse_version": plan.document.parse_version,
        "existing_kinds": plan.existing_kinds,
        "missing_kinds": plan.missing_kinds,
        "skipped_reason": plan.skipped_reason,
        "enqueued_count": plan.enqueued_count,
    })
}

fn enrichment_kinds_from_csv(value: &str) -> Vec<&'static str> {
    let mut kinds = Vec::new();
    for raw in value.split(',') {
        if raw.trim().eq_ignore_ascii_case("all") {
            for kind in ALL_ENRICHMENT_KINDS {
                if !kinds.contains(kind) {
                    kinds.push(*kind);
                }
            }
            continue;
        }
        let Some(kind) = normalize_enrichment_kind(raw) else {
            continue;
        };
        if !kinds.contains(&kind) {
            kinds.push(kind);
        }
    }
    kinds
}

fn normalize_enrichment_kind(raw: &str) -> Option<&'static str> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "structure_outline_v1" | "section_outline_v1" | "section_outline" => {
            Some("structure_outline_v1")
        }
        "fact_index_v2" => Some("fact_index_v2"),
        "qa_seed_v1" => Some("qa_seed_v1"),
        "entity_relation_v1" => Some("entity_relation_v1"),
        "table_structure_v1" | "table_structure" => Some("table_structure_v1"),
        "entity_terms_v1" | "entity_terms" => Some("entity_terms_v1"),
        "procedure_steps_v1" | "procedure_steps" => Some("procedure_steps_v1"),
        "resume_profile_v1" | "resume_profile" => Some("resume_profile_v1"),
        "spreadsheet_metrics_v1" | "spreadsheet_metrics" => Some("spreadsheet_metrics_v1"),
        "semantic_profile_v1" | "semantic_profile" => Some("semantic_profile_v1"),
        _ => None,
    }
}

fn document_enrichment_parse_version(metadata: &Value) -> Option<String> {
    metadata
        .get("ingest")
        .and_then(|value| {
            value
                .get("parse_version")
                .or_else(|| value.get("parseVersion"))
                .or_else(|| value.get("parse_method"))
                .or_else(|| value.get("parseMethod"))
        })
        .or_else(|| metadata.get("parse_version"))
        .or_else(|| metadata.get("parseVersion"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_args_requires_dataset_and_kind() {
        let err = parse_args(
            "document-enrichment-backfill",
            vec!["--dry-run".to_string()],
        )
        .expect_err("dataset id is required");

        assert!(err.to_string().contains("--dataset-id"));

        let dataset_id = Uuid::new_v4();
        let err = parse_args(
            "document-enrichment-backfill",
            vec!["--dataset-id".to_string(), dataset_id.to_string()],
        )
        .expect_err("kind is required");

        assert!(err.to_string().contains("--kind"));
    }

    #[test]
    fn parse_args_supports_dry_run_all_kinds() {
        let dataset_id = Uuid::new_v4();
        let args = parse_args(
            "document-enrichment-backfill",
            vec![
                "--dataset-id".to_string(),
                dataset_id.to_string(),
                "--kind".to_string(),
                "all".to_string(),
                "--dry-run".to_string(),
                "--summary-only".to_string(),
                "--pretty".to_string(),
            ],
        )
        .expect("args should parse");

        assert_eq!(args.dataset_id, DatasetId(dataset_id));
        assert_eq!(args.enrichment_kinds, ALL_ENRICHMENT_KINDS);
        assert!(args.dry_run);
        assert!(args.summary_only);
        assert!(args.pretty);
    }

    #[test]
    fn parse_args_dedupes_and_normalizes_kinds() {
        let dataset_id = Uuid::new_v4();
        let args = parse_args(
            "document-enrichment-backfill",
            vec![
                "--dataset-id".to_string(),
                dataset_id.to_string(),
                "--kind".to_string(),
                "table_structure,table_structure_v1,procedure_steps".to_string(),
                "--dry-run".to_string(),
            ],
        )
        .expect("args should parse");

        assert_eq!(
            args.enrichment_kinds,
            vec!["table_structure_v1", "procedure_steps_v1"]
        );
    }

    #[test]
    fn parse_args_rejects_real_run_without_confirmation() {
        let dataset_id = Uuid::new_v4();
        let err = parse_args(
            "document-enrichment-backfill",
            vec![
                "--dataset-id".to_string(),
                dataset_id.to_string(),
                "--kind".to_string(),
                "procedure_steps".to_string(),
            ],
        )
        .expect_err("real run should require confirmation");

        assert!(err.to_string().contains("--confirm-real-run"));
    }

    #[test]
    fn parse_args_rejects_broad_dataset_real_run() {
        let dataset_id = Uuid::new_v4();
        let err = parse_args(
            "document-enrichment-backfill",
            vec![
                "--dataset-id".to_string(),
                dataset_id.to_string(),
                "--kind".to_string(),
                "procedure_steps".to_string(),
                "--limit".to_string(),
                "6".to_string(),
                "--confirm-real-run".to_string(),
            ],
        )
        .expect_err("dataset real run should reject broad limits");

        assert!(err.to_string().contains("no greater than 5"));
    }

    #[test]
    fn parse_args_rejects_multi_kind_dataset_real_run() {
        let dataset_id = Uuid::new_v4();
        let err = parse_args(
            "document-enrichment-backfill",
            vec![
                "--dataset-id".to_string(),
                dataset_id.to_string(),
                "--kind".to_string(),
                "procedure_steps,table_structure".to_string(),
                "--limit".to_string(),
                "5".to_string(),
                "--confirm-real-run".to_string(),
            ],
        )
        .expect_err("dataset real run should require one kind");

        assert!(err.to_string().contains("exactly one"));
    }

    #[test]
    fn parse_args_allows_confirmed_single_document_multi_kind_run() {
        let dataset_id = Uuid::new_v4();
        let document_id = Uuid::new_v4();
        let args = parse_args(
            "document-enrichment-backfill",
            vec![
                "--dataset-id".to_string(),
                dataset_id.to_string(),
                "--document-id".to_string(),
                document_id.to_string(),
                "--kind".to_string(),
                "procedure_steps,table_structure".to_string(),
                "--confirm-real-run".to_string(),
            ],
        )
        .expect("single document real run should parse");

        assert_eq!(args.document_id, Some(DocumentId(document_id)));
        assert!(!args.dry_run);
        assert!(args.confirm_real_run);
        assert_eq!(args.enrichment_kinds.len(), 2);
    }

    #[test]
    fn parse_version_reads_ingest_metadata() {
        let metadata = json!({
            "ingest": {
                "parse_version": "parser-2026-06"
            }
        });

        assert_eq!(
            document_enrichment_parse_version(&metadata),
            Some("parser-2026-06".to_string())
        );
    }
}
