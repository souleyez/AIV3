use anyhow::{anyhow, Result};
use chrono::Utc;
use domain_model::{DatasetId, Document, DocumentId, TenantId};
use serde_json::{json, Map, Value};
use std::{collections::BTreeMap, str::FromStr};
use storage::{NewDocumentFact, PgStorage};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
struct BackfillArgs {
    dataset_id: DatasetId,
    document_id: Option<DocumentId>,
    limit: usize,
    explicit_limit: bool,
    dry_run: bool,
    confirm_real_run: bool,
    summary_only: bool,
    pretty: bool,
}

#[derive(Clone, Debug)]
struct DocumentFactBackfillSummary {
    document: Document,
    chunk_count: usize,
    facts: Vec<NewDocumentFact>,
    skipped_reason: Option<String>,
}

fn usage(program: &str) -> String {
    format!(
        "Usage: {program} --dataset-id <uuid> [--document-id <uuid>] [--limit <n>] [--dry-run] [--summary-only] [--confirm-real-run] [--pretty]"
    )
}

#[tokio::main]
async fn main() -> Result<()> {
    observability::install("fact_index_backfill")?;

    let program = std::env::args()
        .next()
        .unwrap_or_else(|| "fact-index-backfill".to_string());
    let args = parse_args(&program, std::env::args().skip(1).collect())?;
    let database_url = std::env::var("PLATFORM_DATABASE_URL")
        .unwrap_or_else(|_| storage::DEFAULT_LOCAL_DATABASE_URL.to_string());
    let storage = PgStorage::connect(&database_url).await?;
    storage.migrate().await?;

    let tenant_key = std::env::var("PLATFORM_TENANT_KEY")
        .unwrap_or_else(|_| storage::DEFAULT_LOCAL_TENANT_KEY.to_string());
    let tenant_name = std::env::var("PLATFORM_TENANT_NAME")
        .unwrap_or_else(|_| storage::DEFAULT_LOCAL_TENANT_NAME.to_string());
    let tenant = storage.ensure_tenant(&tenant_key, &tenant_name).await?;

    let summary = run_fact_index_backfill(&storage, tenant.id, &args).await?;
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
    let raw_limit = take_option(&mut args, "--limit");
    let explicit_limit = raw_limit.is_some();
    let limit = raw_limit
        .map(|raw| raw.parse::<usize>())
        .transpose()?
        .unwrap_or(50)
        .max(1);

    if !args.is_empty() {
        anyhow::bail!(usage(program));
    }
    if !dry_run && !confirm_real_run {
        anyhow::bail!(
            "real fact-index backfill requires --confirm-real-run; rerun with --dry-run --summary-only first"
        );
    }
    if !dry_run && document_id.is_none() && (!explicit_limit || limit > 5) {
        anyhow::bail!(
            "dataset-level real fact-index backfill requires an explicit --limit no greater than 5"
        );
    }

    Ok(BackfillArgs {
        dataset_id,
        document_id,
        limit,
        explicit_limit,
        dry_run,
        confirm_real_run,
        summary_only,
        pretty,
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

async fn run_fact_index_backfill(
    storage: &PgStorage,
    tenant_id: TenantId,
    args: &BackfillArgs,
) -> Result<Value> {
    let documents = load_backfill_documents(storage, tenant_id, args).await?;
    let mut summaries = Vec::new();
    let mut fact_type_counts = BTreeMap::<String, usize>::new();
    let mut derived_fact_count = 0usize;
    let mut inserted_fact_count = 0usize;
    let mut skipped_document_count = 0usize;
    let mut parse_quality_warnings = Vec::<Value>::new();
    let mut snapshot_summary = Value::Null;

    for document in documents {
        let chunks = storage
            .document_chunks()
            .list_by_document(tenant_id, document.id)
            .await?;
        let facts = platform_api::fact_index::build_document_fact_candidates(
            &document,
            &chunks,
            Utc::now(),
        );
        for fact in &facts {
            *fact_type_counts.entry(fact.fact_type.clone()).or_insert(0) += 1;
        }
        derived_fact_count += facts.len();

        let skipped_reason = if chunks.is_empty() {
            Some("no_chunks".to_string())
        } else if facts.is_empty() {
            Some("no_facts".to_string())
        } else {
            None
        };
        if skipped_reason.is_some() {
            skipped_document_count += 1;
        }
        if let Some(warning) = document_parse_quality_warning(&document) {
            parse_quality_warnings.push(warning);
        }

        if !args.dry_run && !facts.is_empty() {
            let persisted = storage
                .document_facts()
                .replace_document_facts(tenant_id, document.id, &facts)
                .await?;
            inserted_fact_count += persisted.len();
        }

        summaries.push(DocumentFactBackfillSummary {
            document,
            chunk_count: chunks.len(),
            facts,
            skipped_reason,
        });
    }

    if !args.dry_run {
        let snapshot = platform_api::fact_index::rebuild_dataset_entity_rows_snapshot(
            storage,
            tenant_id,
            args.dataset_id,
            args.limit as i64,
            Utc::now(),
        )
        .await?;
        snapshot_summary = json!({
            "snapshot_kind": snapshot.snapshot_kind,
            "snapshot_key": snapshot.snapshot_key,
            "source_fact_count": snapshot.source_fact_count,
            "source_document_count": snapshot.source_document_count,
            "created_at": snapshot.created_at,
        });
    }

    let document_report_count = summaries.len();
    let mut output = json!({
        "dry_run": args.dry_run,
        "confirm_real_run": args.confirm_real_run,
        "dataset_id": args.dataset_id,
        "document_id": args.document_id,
        "limit": args.limit,
        "summary_only": args.summary_only,
        "document_count": summaries.len(),
        "document_report_count": document_report_count,
        "derived_fact_count": derived_fact_count,
        "inserted_fact_count": inserted_fact_count,
        "skipped_document_count": skipped_document_count,
        "snapshot_updated": !args.dry_run,
        "snapshot": snapshot_summary,
        "parse_quality_warning_count": parse_quality_warnings.len(),
        "fact_type_counts": fact_type_counts_to_json(&fact_type_counts),
    });
    if !args.summary_only {
        if let Some(object) = output.as_object_mut() {
            object.insert(
                "parse_quality_warnings".to_string(),
                Value::Array(parse_quality_warnings),
            );
            object.insert(
                "documents".to_string(),
                Value::Array(
                    summaries
                        .iter()
                        .map(document_summary_to_json)
                        .collect::<Vec<_>>(),
                ),
            );
        }
    }
    Ok(output)
}

async fn load_backfill_documents(
    storage: &PgStorage,
    tenant_id: TenantId,
    args: &BackfillArgs,
) -> Result<Vec<Document>> {
    if let Some(document_id) = args.document_id {
        let document = storage
            .documents()
            .get_by_id(tenant_id, document_id)
            .await?
            .ok_or_else(|| anyhow!("document {} not found", document_id))?;
        if document.dataset_id != args.dataset_id {
            anyhow::bail!(
                "document {} belongs to dataset {}, not {}",
                document.id,
                document.dataset_id,
                args.dataset_id
            );
        }
        return Ok(vec![document]);
    }

    let mut documents = storage
        .documents()
        .list_by_dataset(tenant_id, args.dataset_id)
        .await?;
    documents.truncate(args.limit);
    Ok(documents)
}

fn document_parse_quality_warning(document: &Document) -> Option<Value> {
    let parse_status = document
        .metadata
        .get("parse_status")
        .and_then(Value::as_str)
        .or_else(|| {
            document
                .metadata
                .get("ingest")
                .and_then(|value| value.get("parse_status"))
                .and_then(Value::as_str)
        });
    let quality_status = document
        .metadata
        .get("ingest")
        .and_then(|value| value.get("parse_quality_status"))
        .and_then(Value::as_str);
    let degraded = parse_status
        .map(|status| status.contains("degraded") || status == "placeholder" || status == "failed")
        .unwrap_or(false)
        || quality_status
            .map(|status| status.contains("low_text_coverage"))
            .unwrap_or(false);

    degraded.then(|| {
        json!({
            "document_id": document.id,
            "title": document.title,
            "parse_status": parse_status,
            "parse_quality_status": quality_status,
        })
    })
}

fn document_summary_to_json(summary: &DocumentFactBackfillSummary) -> Value {
    let mut fact_type_counts = BTreeMap::<String, usize>::new();
    for fact in &summary.facts {
        *fact_type_counts.entry(fact.fact_type.clone()).or_insert(0) += 1;
    }

    json!({
        "document_id": summary.document.id,
        "title": summary.document.title,
        "chunk_count": summary.chunk_count,
        "derived_fact_count": summary.facts.len(),
        "skipped_reason": summary.skipped_reason,
        "fact_type_counts": fact_type_counts_to_json(&fact_type_counts),
    })
}

fn fact_type_counts_to_json(counts: &BTreeMap<String, usize>) -> Value {
    Value::Object(
        counts
            .iter()
            .map(|(key, count)| (key.clone(), Value::from(*count)))
            .collect::<Map<_, _>>(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_args_requires_dataset_id() {
        let err = parse_args("fact-index-backfill", vec!["--dry-run".to_string()])
            .expect_err("dataset id is required");

        assert!(err.to_string().contains("--dataset-id"));
    }

    #[test]
    fn parse_args_supports_dry_run_document_and_limit() {
        let dataset_id = Uuid::new_v4();
        let document_id = Uuid::new_v4();
        let args = parse_args(
            "fact-index-backfill",
            vec![
                "--dataset-id".to_string(),
                dataset_id.to_string(),
                "--document-id".to_string(),
                document_id.to_string(),
                "--limit".to_string(),
                "25".to_string(),
                "--dry-run".to_string(),
                "--pretty".to_string(),
            ],
        )
        .expect("args should parse");

        assert_eq!(args.dataset_id, DatasetId(dataset_id));
        assert_eq!(args.document_id, Some(DocumentId(document_id)));
        assert_eq!(args.limit, 25);
        assert!(args.explicit_limit);
        assert!(args.dry_run);
        assert!(!args.confirm_real_run);
        assert!(!args.summary_only);
        assert!(args.pretty);
    }

    #[test]
    fn parse_args_supports_summary_only() {
        let dataset_id = Uuid::new_v4();
        let args = parse_args(
            "fact-index-backfill",
            vec![
                "--dataset-id".to_string(),
                dataset_id.to_string(),
                "--dry-run".to_string(),
                "--summary-only".to_string(),
            ],
        )
        .expect("args should parse");

        assert_eq!(args.dataset_id, DatasetId(dataset_id));
        assert!(args.summary_only);
        assert!(args.dry_run);
    }

    #[test]
    fn parse_args_requires_confirmation_for_real_run() {
        let dataset_id = Uuid::new_v4();
        let err = parse_args(
            "fact-index-backfill",
            vec!["--dataset-id".to_string(), dataset_id.to_string()],
        )
        .expect_err("real run should require confirmation");

        assert!(err.to_string().contains("--confirm-real-run"));
    }

    #[test]
    fn parse_args_requires_small_explicit_limit_for_dataset_real_run() {
        let dataset_id = Uuid::new_v4();
        let err = parse_args(
            "fact-index-backfill",
            vec![
                "--dataset-id".to_string(),
                dataset_id.to_string(),
                "--confirm-real-run".to_string(),
            ],
        )
        .expect_err("dataset real run should require explicit tiny limit");

        assert!(err.to_string().contains("--limit"));

        let err = parse_args(
            "fact-index-backfill",
            vec![
                "--dataset-id".to_string(),
                dataset_id.to_string(),
                "--limit".to_string(),
                "6".to_string(),
                "--confirm-real-run".to_string(),
            ],
        )
        .expect_err("dataset real run should reject broad limits");

        assert!(err.to_string().contains("no greater than 5"));
    }

    #[test]
    fn parse_args_allows_confirmed_single_document_real_run() {
        let dataset_id = Uuid::new_v4();
        let document_id = Uuid::new_v4();
        let args = parse_args(
            "fact-index-backfill",
            vec![
                "--dataset-id".to_string(),
                dataset_id.to_string(),
                "--document-id".to_string(),
                document_id.to_string(),
                "--confirm-real-run".to_string(),
            ],
        )
        .expect("confirmed single-document real run should parse");

        assert_eq!(args.dataset_id, DatasetId(dataset_id));
        assert_eq!(args.document_id, Some(DocumentId(document_id)));
        assert!(!args.dry_run);
        assert!(args.confirm_real_run);
    }

    #[test]
    fn fact_type_counts_are_stable_json_object() {
        let mut counts = BTreeMap::new();
        counts.insert("organization".to_string(), 2);
        counts.insert("role_position".to_string(), 1);

        let value = fact_type_counts_to_json(&counts);

        assert_eq!(value["organization"], json!(2));
        assert_eq!(value["role_position"], json!(1));
    }
}
