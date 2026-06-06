use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use domain_model::{DatasetId, DocumentId, TenantId};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    str::FromStr,
};
use storage::PgStorage;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
struct BackfillArgs {
    dataset_id: Option<DatasetId>,
    document_id: Option<DocumentId>,
    limit: usize,
    dry_run: bool,
    include_existing: bool,
    summary_only: bool,
    pretty: bool,
}

#[derive(Clone, Debug)]
struct DocumentCandidate {
    id: DocumentId,
    dataset_id: DatasetId,
    title: String,
    object_key: String,
    content_type: String,
    existing_content_sha256: Option<String>,
}

#[derive(Clone, Debug)]
struct LocalFingerprint {
    path: PathBuf,
    content_sha256: String,
    content_size_bytes: i64,
}

fn usage(program: &str) -> String {
    format!(
        "Usage: {program} [--dataset-id <uuid>] [--document-id <uuid>] [--limit <n>] [--dry-run] [--include-existing] [--summary-only] [--pretty]"
    )
}

#[tokio::main]
async fn main() -> Result<()> {
    observability::install("document_fingerprint_backfill")?;

    let program = std::env::args()
        .next()
        .unwrap_or_else(|| "document-fingerprint-backfill".to_string());
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

    let summary = run_document_fingerprint_backfill(&storage, tenant.id, &args).await?;
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
    let include_existing = remove_flag(&mut args, "--include-existing");
    let summary_only = remove_flag(&mut args, "--summary-only");
    let dataset_id = take_option(&mut args, "--dataset-id")
        .map(|raw| parse_uuid_arg("dataset_id", &raw).map(DatasetId))
        .transpose()?;
    let document_id = take_option(&mut args, "--document-id")
        .map(|raw| parse_uuid_arg("document_id", &raw).map(DocumentId))
        .transpose()?;
    let limit = take_option(&mut args, "--limit")
        .map(|raw| raw.parse::<usize>())
        .transpose()?
        .unwrap_or(100)
        .max(1);

    if !args.is_empty() {
        anyhow::bail!(usage(program));
    }

    Ok(BackfillArgs {
        dataset_id,
        document_id,
        limit,
        dry_run,
        include_existing,
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

async fn run_document_fingerprint_backfill(
    storage: &PgStorage,
    tenant_id: TenantId,
    args: &BackfillArgs,
) -> Result<Value> {
    let candidates = load_document_candidates(storage, tenant_id, args).await?;
    let mut documents = Vec::new();
    let mut would_record_count = 0usize;
    let mut recorded_count = 0usize;
    let mut duplicate_count = 0usize;
    let mut skipped_count = 0usize;

    for document in candidates {
        let Some(fingerprint) = local_fingerprint_for_object_key(&document.object_key) else {
            skipped_count += 1;
            documents.push(document_report(
                &document,
                "skipped",
                Some("local_file_not_found"),
                None,
            ));
            continue;
        };
        let existing_canonical =
            load_existing_canonical_document_id(storage, tenant_id, &fingerprint.content_sha256)
                .await?;
        let would_duplicate = existing_canonical
            .map(|canonical_document_id| canonical_document_id != document.id)
            .unwrap_or(false);
        if would_duplicate {
            duplicate_count += 1;
        }

        if args.dry_run {
            would_record_count += 1;
            documents.push(document_report(
                &document,
                if would_duplicate {
                    "would_record_duplicate"
                } else {
                    "would_record"
                },
                None,
                Some(fingerprint_report(&fingerprint, existing_canonical, None)),
            ));
            continue;
        }

        let _updated = storage
            .documents()
            .record_content_fingerprint(
                tenant_id,
                document.id,
                &fingerprint.content_sha256,
                fingerprint.content_size_bytes,
                Utc::now(),
            )
            .await?;
        let final_state = load_document_fingerprint_state(storage, tenant_id, document.id).await?;
        recorded_count += 1;
        documents.push(document_report(
            &document,
            if would_duplicate {
                "recorded_duplicate"
            } else {
                "recorded"
            },
            None,
            Some(fingerprint_report(
                &fingerprint,
                existing_canonical,
                final_state,
            )),
        ));
    }

    let document_report_count = documents.len();
    let mut summary = json!({
        "dry_run": args.dry_run,
        "dataset_id": args.dataset_id,
        "document_id": args.document_id,
        "limit": args.limit,
        "include_existing": args.include_existing,
        "summary_only": args.summary_only,
        "candidate_count": documents.len(),
        "document_report_count": document_report_count,
        "would_record_count": would_record_count,
        "recorded_count": recorded_count,
        "duplicate_count": duplicate_count,
        "skipped_count": skipped_count,
    });
    if !args.summary_only {
        if let Some(object) = summary.as_object_mut() {
            object.insert("documents".to_string(), Value::Array(documents));
        }
    }
    Ok(summary)
}

async fn load_document_candidates(
    storage: &PgStorage,
    tenant_id: TenantId,
    args: &BackfillArgs,
) -> Result<Vec<DocumentCandidate>> {
    let rows = sqlx::query(
        r#"
        select id, dataset_id, title, object_key, content_type, content_sha256
        from documents
        where tenant_id = $1
          and ($2::uuid is null or dataset_id = $2)
          and ($3::uuid is null or id = $3)
          and ($4::boolean or $3::uuid is not null or content_sha256 is null)
        order by created_at desc, title asc
        limit $5
        "#,
    )
    .bind(tenant_id.0)
    .bind(args.dataset_id.map(|id| id.0))
    .bind(args.document_id.map(|id| id.0))
    .bind(args.include_existing)
    .bind(args.limit as i64)
    .fetch_all(storage.pool())
    .await?;

    Ok(rows
        .iter()
        .map(|row| DocumentCandidate {
            id: DocumentId(row.get::<Uuid, _>("id")),
            dataset_id: DatasetId(row.get::<Uuid, _>("dataset_id")),
            title: row.get("title"),
            object_key: row.get("object_key"),
            content_type: row.get("content_type"),
            existing_content_sha256: row.get("content_sha256"),
        })
        .collect())
}

fn local_fingerprint_for_object_key(object_key: &str) -> Option<LocalFingerprint> {
    let path = resolve_local_object_path(object_key)?;
    let metadata = fs::metadata(&path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    let max_bytes = configured_max_fingerprint_bytes();
    if metadata.len() > max_bytes || metadata.len() > i64::MAX as u64 {
        return None;
    }

    let mut file = File::open(&path).ok()?;
    let mut hasher = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        total = total.saturating_add(read as u64);
        if total > max_bytes || total > i64::MAX as u64 {
            return None;
        }
        hasher.update(&buffer[..read]);
    }

    Some(LocalFingerprint {
        path,
        content_sha256: format!("{:x}", hasher.finalize()),
        content_size_bytes: total as i64,
    })
}

fn resolve_local_object_path(object_key: &str) -> Option<PathBuf> {
    let raw = object_key.trim().trim_start_matches("file://");
    if raw.is_empty() {
        return None;
    }

    let direct = PathBuf::from(raw);
    if direct.is_file() {
        return Some(direct);
    }

    if cfg!(windows) {
        if let Some(rest) = raw.strip_prefix("/mnt/") {
            let mut parts = rest.splitn(2, '/');
            if let (Some(drive), Some(path)) = (parts.next(), parts.next()) {
                if drive.len() == 1 {
                    let windows_path = format!("{}:\\{}", drive, path.replace('/', "\\"));
                    let candidate = PathBuf::from(windows_path);
                    if candidate.is_file() {
                        return Some(candidate);
                    }
                }
            }
        }
    }

    let root = std::env::var("PLATFORM_LOCAL_OBJECT_ROOT").ok()?;
    let rooted = Path::new(&root).join(raw);
    rooted.is_file().then_some(rooted)
}

fn configured_max_fingerprint_bytes() -> u64 {
    std::env::var("DOCUMENT_FINGERPRINT_MAX_BYTES")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(300 * 1024 * 1024)
        .max(1)
}

async fn load_existing_canonical_document_id(
    storage: &PgStorage,
    tenant_id: TenantId,
    content_sha256: &str,
) -> Result<Option<DocumentId>> {
    let row = sqlx::query_scalar::<_, Uuid>(
        r#"
        select canonical_document_id
        from document_content_fingerprints
        where tenant_id = $1 and content_sha256 = $2
        "#,
    )
    .bind(tenant_id.0)
    .bind(content_sha256)
    .fetch_optional(storage.pool())
    .await?;
    Ok(row.map(DocumentId))
}

async fn load_document_fingerprint_state(
    storage: &PgStorage,
    tenant_id: TenantId,
    document_id: DocumentId,
) -> Result<Option<Value>> {
    let row = sqlx::query(
        r#"
        select canonical_document_id, dedup_state, deduped_at
        from documents
        where tenant_id = $1 and id = $2
        "#,
    )
    .bind(tenant_id.0)
    .bind(document_id.0)
    .fetch_optional(storage.pool())
    .await?;

    Ok(row.map(|row| {
        json!({
            "canonical_document_id": row.get::<Option<Uuid>, _>("canonical_document_id").map(DocumentId),
            "dedup_state": row.get::<Option<String>, _>("dedup_state"),
            "deduped_at": row.get::<Option<DateTime<Utc>>, _>("deduped_at"),
        })
    }))
}

fn document_report(
    document: &DocumentCandidate,
    action: &str,
    skipped_reason: Option<&str>,
    fingerprint: Option<Value>,
) -> Value {
    json!({
        "document_id": document.id,
        "dataset_id": document.dataset_id,
        "title": document.title,
        "content_type": document.content_type,
        "existing_content_sha256": document.existing_content_sha256,
        "action": action,
        "skipped_reason": skipped_reason,
        "fingerprint": fingerprint,
    })
}

fn fingerprint_report(
    fingerprint: &LocalFingerprint,
    existing_canonical_document_id: Option<DocumentId>,
    final_state: Option<Value>,
) -> Value {
    json!({
        "path": fingerprint.path,
        "content_sha256": fingerprint.content_sha256,
        "content_size_bytes": fingerprint.content_size_bytes,
        "existing_canonical_document_id": existing_canonical_document_id,
        "final_state": final_state,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_args_defaults_to_dry_run_false_and_limited_scope() {
        let dataset_id = Uuid::new_v4();
        let args = parse_args(
            "document-fingerprint-backfill",
            vec![
                "--dataset-id".to_string(),
                dataset_id.to_string(),
                "--dry-run".to_string(),
                "--pretty".to_string(),
            ],
        )
        .expect("args should parse");

        assert_eq!(args.dataset_id, Some(DatasetId(dataset_id)));
        assert_eq!(args.document_id, None);
        assert_eq!(args.limit, 100);
        assert!(args.dry_run);
        assert!(!args.include_existing);
        assert!(!args.summary_only);
        assert!(args.pretty);
    }

    #[test]
    fn parse_args_supports_document_limit_and_include_existing() {
        let document_id = Uuid::new_v4();
        let args = parse_args(
            "document-fingerprint-backfill",
            vec![
                "--document-id".to_string(),
                document_id.to_string(),
                "--limit".to_string(),
                "25".to_string(),
                "--include-existing".to_string(),
                "--summary-only".to_string(),
            ],
        )
        .expect("args should parse");

        assert_eq!(args.dataset_id, None);
        assert_eq!(args.document_id, Some(DocumentId(document_id)));
        assert_eq!(args.limit, 25);
        assert!(args.include_existing);
        assert!(args.summary_only);
    }

    #[test]
    fn local_fingerprint_reads_existing_file() {
        let root = std::env::temp_dir().join(format!(
            "datamax-fingerprint-backfill-test-{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(&root).expect("temp root should be created");
        let path = root.join("fixture.txt");
        let body = b"fingerprint backfill fixture\n";
        fs::write(&path, body).expect("fixture should be written");
        let expected_sha256 = {
            let mut hasher = Sha256::new();
            hasher.update(body);
            format!("{:x}", hasher.finalize())
        };

        let fingerprint = local_fingerprint_for_object_key(&path.to_string_lossy())
            .expect("fingerprint should be computed");

        assert_eq!(fingerprint.content_sha256, expected_sha256);
        assert_eq!(fingerprint.content_size_bytes, body.len() as i64);
    }
}
