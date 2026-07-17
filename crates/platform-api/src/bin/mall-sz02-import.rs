use anyhow::{anyhow, bail, Context, Result};
use chrono::NaiveDate;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{postgres::PgPoolOptions, PgPool, Postgres, QueryBuilder, Row, Transaction};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    str::FromStr,
};
use uuid::Uuid;

const MALL_CODE: &str = "SZ02";
const TRAFFIC_SOURCE_KIND: &str = "aibee_traffic_hourly";
const PROFILE_SOURCE_KIND: &str = "aibee_profile_aggregate";
const SOURCE_DOCUMENT_LIFECYCLE: &str = "indexed";
const MAX_SOURCE_BYTES: u64 = 64 * 1024 * 1024;
const INSERT_CHUNK_SIZE: usize = 1_000;

const TRAFFIC_TITLES: [&str; 2] = ["traffic_hourly.csv", "point_inventory.csv"];
const PROFILE_TITLES: [&str; 7] = [
    "daily_summary.csv",
    "age_distribution_daily.csv",
    "age_distribution_total.csv",
    "gender_distribution_daily.csv",
    "gender_distribution_total.csv",
    "group_type_distribution_daily.csv",
    "group_type_distribution_total.csv",
];

const TRAFFIC_HEADER: [&str; 15] = [
    "source_mall_id",
    "mallId",
    "entityType",
    "entityTypeName",
    "entityName",
    "entityId",
    "aibeeEntityId",
    "day",
    "hour",
    "minute",
    "interval",
    "trafficIn",
    "trafficOut",
    "visitors",
    "averageStay",
];
const INVENTORY_HEADER: [&str; 11] = [
    "mallId",
    "entityType",
    "entityName",
    "entityId",
    "aibeeEntityId",
    "floor",
    "floorName",
    "entityStatus",
    "area",
    "l1RetailFormat",
    "l2RetailFormat",
];
const SUMMARY_HEADER: [&str; 7] = [
    "date",
    "record_count",
    "unique_pid_count",
    "duplicate_record_count",
    "missing_pid_count",
    "date_mismatch_count",
    "invalid_age_count",
];

#[derive(Clone, Debug, PartialEq, Eq)]
struct Args {
    traffic_dataset_id: Uuid,
    profile_dataset_id: Uuid,
    expected_date_from: NaiveDate,
    expected_date_to: NaiveDate,
    dry_run: bool,
}

#[derive(Clone, Debug)]
struct CsvRecord {
    record_number: i64,
    fields: Vec<String>,
}

#[derive(Clone, Debug)]
struct ParsedCsv {
    header: Vec<String>,
    records: Vec<CsvRecord>,
}

#[derive(Clone, Debug)]
struct SourceDocument {
    id: Uuid,
    title: String,
    sha256: String,
    csv: ParsedCsv,
}

#[derive(Clone, Debug)]
struct BatchMetadata {
    combined_sha256: String,
    manifest: Value,
    source_file_count: i32,
    source_row_count: i64,
}

#[derive(Clone, Debug)]
struct PointRow {
    source_document_id: Uuid,
    source_row_number: i64,
    source_inventory_mall_id: Option<String>,
    entity_type: i16,
    entity_type_name: Option<String>,
    entity_name: String,
    source_entity_id: String,
    aibee_entity_id: String,
    floor_code: Option<String>,
    floor_name: Option<String>,
    entity_status: Option<i16>,
    area: Option<String>,
    l1_retail_format: Option<String>,
    l2_retail_format: Option<String>,
    inventory_present: bool,
}

#[derive(Clone, Debug)]
struct TrafficRow {
    source_document_id: Uuid,
    source_row_number: i64,
    source_mall_id: String,
    source_internal_mall_id: String,
    entity_type: i16,
    entity_type_name: Option<String>,
    entity_name: String,
    source_entity_id: String,
    aibee_entity_id: String,
    business_date: NaiveDate,
    hour: i16,
    minute: i16,
    traffic_in: i64,
    traffic_out: i64,
    visitors: i64,
    average_stay: String,
    visitors_metric_available: bool,
    average_stay_metric_available: bool,
}

#[derive(Clone, Debug)]
struct DailySummaryRow {
    source_document_id: Uuid,
    source_row_number: i64,
    business_date: NaiveDate,
    record_count: i64,
    unique_pid_count: i64,
    duplicate_record_count: i64,
    missing_pid_count: i64,
    date_mismatch_count: i64,
    invalid_age_count: i64,
}

#[derive(Clone, Debug)]
struct DailyDistributionRow {
    source_document_id: Uuid,
    source_row_number: i64,
    business_date: NaiveDate,
    dimension_key: &'static str,
    bucket_key: String,
    visitor_count: i64,
    share: String,
}

#[derive(Clone, Debug)]
struct PeriodDistributionRow {
    source_document_id: Uuid,
    source_row_number: i64,
    dimension_key: &'static str,
    bucket_key: String,
    visitor_count: i64,
    share: String,
}

#[derive(Debug)]
struct PreparedTraffic {
    batch: BatchMetadata,
    facts: Vec<TrafficRow>,
    points: Vec<PointRow>,
    business_date_from: NaiveDate,
    business_date_to: NaiveDate,
    synthesized_point_count: usize,
}

#[derive(Debug)]
struct PreparedProfile {
    batch: BatchMetadata,
    summaries: Vec<DailySummaryRow>,
    daily_distributions: Vec<DailyDistributionRow>,
    period_distributions: Vec<PeriodDistributionRow>,
    business_date_from: NaiveDate,
    business_date_to: NaiveDate,
}

fn usage(program: &str) -> String {
    format!(
        "Usage: {program} --traffic-dataset-id <uuid> --profile-dataset-id <uuid> --expected-date-from <YYYY-MM-DD> --expected-date-to <YYYY-MM-DD> [--dry-run]"
    )
}

fn parse_args(program: &str, args: Vec<String>) -> Result<Args> {
    let mut traffic_dataset_id = None;
    let mut profile_dataset_id = None;
    let mut expected_date_from = None;
    let mut expected_date_to = None;
    let mut dry_run = false;
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--traffic-dataset-id" | "--profile-dataset-id" => {
                let name = args[index].clone();
                let raw = args.get(index + 1).ok_or_else(|| anyhow!(usage(program)))?;
                let value = Uuid::from_str(raw)
                    .map_err(|_| anyhow!("{name} must be a UUID; {}", usage(program)))?;
                let target = if name == "--traffic-dataset-id" {
                    &mut traffic_dataset_id
                } else {
                    &mut profile_dataset_id
                };
                if target.replace(value).is_some() {
                    bail!("{name} may only be provided once");
                }
                index += 2;
            }
            "--expected-date-from" | "--expected-date-to" => {
                let name = args[index].clone();
                let raw = args.get(index + 1).ok_or_else(|| anyhow!(usage(program)))?;
                let value = NaiveDate::parse_from_str(raw, "%Y-%m-%d")
                    .map_err(|_| anyhow!("{name} must be YYYY-MM-DD; {}", usage(program)))?;
                let target = if name == "--expected-date-from" {
                    &mut expected_date_from
                } else {
                    &mut expected_date_to
                };
                if target.replace(value).is_some() {
                    bail!("{name} may only be provided once");
                }
                index += 2;
            }
            "--dry-run" => {
                if dry_run {
                    bail!("--dry-run may only be provided once");
                }
                dry_run = true;
                index += 1;
            }
            "--help" | "-h" => bail!(usage(program)),
            _ => bail!(usage(program)),
        }
    }
    let traffic_dataset_id = traffic_dataset_id.ok_or_else(|| anyhow!(usage(program)))?;
    let profile_dataset_id = profile_dataset_id.ok_or_else(|| anyhow!(usage(program)))?;
    let expected_date_from = expected_date_from.ok_or_else(|| anyhow!(usage(program)))?;
    let expected_date_to = expected_date_to.ok_or_else(|| anyhow!(usage(program)))?;
    if traffic_dataset_id == profile_dataset_id {
        bail!("traffic and profile dataset IDs must differ");
    }
    if expected_date_from > expected_date_to {
        bail!("expected date range must not be inverted");
    }
    Ok(Args {
        traffic_dataset_id,
        profile_dataset_id,
        expected_date_from,
        expected_date_to,
        dry_run,
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let program = std::env::args()
        .next()
        .unwrap_or_else(|| "mall-sz02-import".to_string());
    let args = parse_args(&program, std::env::args().skip(1).collect())?;
    let database_url = std::env::var("PLATFORM_DATABASE_URL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("PLATFORM_DATABASE_URL must be configured"))?;
    let tenant_key = std::env::var("PLATFORM_TENANT_KEY")
        .unwrap_or_else(|_| storage::DEFAULT_LOCAL_TENANT_KEY.to_string());
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url)
        .await
        .context("connect to platform PostgreSQL")?;
    let tenant_id = lookup_tenant_id(&pool, &tenant_key).await?;
    let roots = configured_object_roots()?;
    let traffic_documents = load_source_documents(
        &pool,
        tenant_id,
        args.traffic_dataset_id,
        &TRAFFIC_TITLES,
        &roots,
    )
    .await?;
    let profile_documents = load_source_documents(
        &pool,
        tenant_id,
        args.profile_dataset_id,
        &PROFILE_TITLES,
        &roots,
    )
    .await?;
    let traffic = prepare_traffic(&traffic_documents)?;
    let profile = prepare_profile(&profile_documents)?;
    validate_matching_snapshot_ranges(
        traffic.business_date_from,
        traffic.business_date_to,
        profile.business_date_from,
        profile.business_date_to,
    )?;
    validate_expected_snapshot_range(
        traffic.business_date_from,
        traffic.business_date_to,
        args.expected_date_from,
        args.expected_date_to,
    )?;

    let output = if args.dry_run {
        build_output(&args, &traffic, &profile, None)
    } else {
        let batch_ids = import_all(&pool, tenant_id, &args, &traffic, &profile).await?;
        build_output(&args, &traffic, &profile, Some(batch_ids))
    };
    println!("{}", serde_json::to_string(&output)?);
    Ok(())
}

async fn lookup_tenant_id(pool: &PgPool, tenant_key: &str) -> Result<Uuid> {
    let tenant_id = sqlx::query_scalar::<_, Uuid>("select id from tenants where key = $1")
        .bind(tenant_key)
        .fetch_optional(pool)
        .await
        .context("look up configured tenant")?;
    tenant_id.ok_or_else(|| anyhow!("configured tenant does not exist"))
}

fn configured_object_roots() -> Result<Vec<PathBuf>> {
    let mut roots = Vec::new();
    for name in ["PLATFORM_LOCAL_OBJECT_ROOT", "PLATFORM_LEGACY_UPLOAD_ROOT"] {
        if let Some(raw) = std::env::var_os(name) {
            if raw.is_empty() {
                continue;
            }
            let root = fs::canonicalize(PathBuf::from(raw))
                .with_context(|| format!("canonicalize configured object root {name}"))?;
            if !root.is_dir() {
                bail!("configured object root {name} is not a directory");
            }
            if !roots.contains(&root) {
                roots.push(root);
            }
        }
    }
    if roots.is_empty() {
        bail!("no local object root is configured");
    }
    Ok(roots)
}

async fn load_source_documents<const N: usize>(
    pool: &PgPool,
    tenant_id: Uuid,
    dataset_id: Uuid,
    expected_titles: &[&str; N],
    roots: &[PathBuf],
) -> Result<Vec<SourceDocument>> {
    let dataset_exists = sqlx::query_scalar::<_, bool>(
        "select exists(select 1 from datasets where tenant_id = $1 and id = $2 and lifecycle = 'active')",
    )
    .bind(tenant_id)
    .bind(dataset_id)
    .fetch_one(pool)
    .await
    .context("validate dataset scope")?;
    if !dataset_exists {
        bail!("dataset is not part of the configured tenant");
    }

    let title_values = expected_titles
        .iter()
        .map(|title| (*title).to_string())
        .collect::<Vec<_>>();
    let rows = sqlx::query(
        r#"
        select d.id, d.title, d.object_key
        from documents d
        where d.tenant_id = $1
          and d.lifecycle = $4
          and d.title = any($3::text[])
          and (
                d.dataset_id = $2
                or exists (
                    select 1
                    from dataset_document_memberships m
                    where m.tenant_id = d.tenant_id
                      and m.dataset_id = $2
                      and m.document_id = d.id
                      and (m.expires_at is null or m.expires_at > now())
                )
          )
        order by d.title, d.id
        "#,
    )
    .bind(tenant_id)
    .bind(dataset_id)
    .bind(&title_values)
    .bind(SOURCE_DOCUMENT_LIFECYCLE)
    .fetch_all(pool)
    .await
    .context("load exact dataset source documents")?;

    let mut candidates = BTreeMap::<String, Vec<(Uuid, String)>>::new();
    for row in rows {
        candidates
            .entry(row.get::<String, _>("title"))
            .or_default()
            .push((row.get("id"), row.get("object_key")));
    }

    let mut documents = Vec::with_capacity(N);
    for title in expected_titles {
        let matches = candidates.remove(*title).unwrap_or_default();
        if matches.len() != 1 {
            bail!("expected exactly one source document named {title}");
        }
        let (id, object_key) = matches.into_iter().next().expect("one match");
        let path = resolve_object_path(&object_key, roots)
            .with_context(|| format!("resolve source document {title}"))?;
        let metadata = fs::metadata(&path)
            .with_context(|| format!("read metadata for source document {title}"))?;
        if !metadata.is_file() || metadata.len() > MAX_SOURCE_BYTES {
            bail!("source document {title} is not a supported regular file");
        }
        let bytes = fs::read(&path).with_context(|| format!("read source document {title}"))?;
        let sha256 = sha256_hex(&bytes);
        let csv = parse_csv(&bytes).with_context(|| format!("parse source document {title}"))?;
        documents.push(SourceDocument {
            id,
            title: (*title).to_string(),
            sha256,
            csv,
        });
    }
    Ok(documents)
}

fn resolve_object_path(object_key: &str, roots: &[PathBuf]) -> Result<PathBuf> {
    let trimmed = object_key.trim();
    if trimmed.is_empty() || looks_remote(trimmed) {
        bail!("object key is empty or remote");
    }
    let raw = trimmed.strip_prefix("file://").unwrap_or(trimmed);
    let raw_path = Path::new(raw);
    let mut matches = Vec::new();
    for root in roots {
        let candidate = if raw_path.is_absolute() {
            raw_path.to_path_buf()
        } else {
            root.join(raw_path)
        };
        let Ok(candidate) = fs::canonicalize(candidate) else {
            continue;
        };
        if candidate.starts_with(root) && candidate.is_file() {
            if !matches.contains(&candidate) {
                matches.push(candidate);
            }
        }
    }
    match matches.len() {
        1 => Ok(matches.remove(0)),
        0 => bail!("object key does not resolve inside a configured root"),
        _ => bail!("relative object key resolves ambiguously across configured roots"),
    }
}

fn looks_remote(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    ["http://", "https://", "s3://", "cos://", "oss://", "gs://"]
        .iter()
        .any(|prefix| lower.starts_with(prefix))
}

fn parse_csv(bytes: &[u8]) -> Result<ParsedCsv> {
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    std::str::from_utf8(bytes).context("CSV must be UTF-8")?;
    let mut logical_rows = Vec::<Vec<String>>::new();
    let mut row = Vec::<String>::new();
    let mut field = Vec::<u8>::new();
    let mut in_quotes = false;
    let mut after_quote = false;
    let mut index = 0usize;

    while index < bytes.len() {
        let byte = bytes[index];
        if in_quotes {
            if byte == b'"' {
                if bytes.get(index + 1) == Some(&b'"') {
                    field.push(b'"');
                    index += 2;
                    continue;
                }
                in_quotes = false;
                after_quote = true;
            } else {
                field.push(byte);
            }
            index += 1;
            continue;
        }
        if after_quote {
            match byte {
                b',' => {
                    row.push(csv_field(&field)?);
                    field.clear();
                    after_quote = false;
                }
                b'\r' | b'\n' => {
                    row.push(csv_field(&field)?);
                    field.clear();
                    logical_rows.push(std::mem::take(&mut row));
                    after_quote = false;
                    if byte == b'\r' && bytes.get(index + 1) == Some(&b'\n') {
                        index += 1;
                    }
                }
                _ => bail!("malformed CSV: data follows a closing quote"),
            }
            index += 1;
            continue;
        }
        match byte {
            b'"' if field.is_empty() => in_quotes = true,
            b'"' => bail!("malformed CSV: quote inside an unquoted field"),
            b',' => {
                row.push(csv_field(&field)?);
                field.clear();
            }
            b'\r' | b'\n' => {
                row.push(csv_field(&field)?);
                field.clear();
                logical_rows.push(std::mem::take(&mut row));
                if byte == b'\r' && bytes.get(index + 1) == Some(&b'\n') {
                    index += 1;
                }
            }
            _ => field.push(byte),
        }
        index += 1;
    }
    if in_quotes {
        bail!("malformed CSV: unterminated quoted field");
    }
    if after_quote || !field.is_empty() || !row.is_empty() {
        row.push(csv_field(&field)?);
        logical_rows.push(row);
    }
    if logical_rows.len() < 2 {
        bail!("CSV must contain a header and at least one data row");
    }
    let header = logical_rows.remove(0);
    if header.iter().any(|cell| cell.is_empty()) {
        bail!("CSV header contains an empty column name");
    }
    let width = header.len();
    let records = logical_rows
        .into_iter()
        .enumerate()
        .map(|(index, fields)| {
            if fields.len() != width {
                bail!("CSV record {} has an unexpected field count", index + 2);
            }
            Ok(CsvRecord {
                record_number: (index + 2) as i64,
                fields,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(ParsedCsv { header, records })
}

fn csv_field(bytes: &[u8]) -> Result<String> {
    Ok(std::str::from_utf8(bytes)
        .context("CSV field must be UTF-8")?
        .to_string())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn batch_metadata(documents: &[SourceDocument]) -> BatchMetadata {
    let mut ordered = documents.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| left.title.cmp(&right.title));
    let mut combined = Sha256::new();
    combined.update(b"mall-sz02-import-v1\0");
    let mut files = Vec::with_capacity(ordered.len());
    let mut source_row_count = 0i64;
    for document in ordered {
        combined.update(document.title.as_bytes());
        combined.update(b"\0");
        combined.update(document.sha256.as_bytes());
        combined.update(b"\0");
        let row_count = document.csv.records.len() as i64;
        source_row_count += row_count;
        files.push(json!({
            "document_id": document.id,
            "title": document.title,
            "sha256": document.sha256,
            "row_count": row_count,
        }));
    }
    BatchMetadata {
        combined_sha256: combined
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        manifest: json!({"files": files}),
        source_file_count: documents.len() as i32,
        source_row_count,
    }
}

fn document<'a>(documents: &'a [SourceDocument], title: &str) -> &'a SourceDocument {
    documents
        .iter()
        .find(|document| document.title == title)
        .expect("expected source document was loaded")
}

fn expect_header<const N: usize>(csv: &ParsedCsv, expected: &[&str; N]) -> Result<()> {
    if csv.header.len() != N
        || csv
            .header
            .iter()
            .zip(expected)
            .any(|(actual, expected)| actual != expected)
    {
        bail!("CSV header does not match the required schema");
    }
    Ok(())
}

fn required_text(record: &CsvRecord, index: usize, field_name: &str) -> Result<String> {
    let value = record.fields[index].trim();
    if value.is_empty() {
        bail!(
            "record {} field {field_name} must not be empty",
            record.record_number
        );
    }
    Ok(value.to_string())
}

fn optional_text(record: &CsvRecord, index: usize) -> Option<String> {
    let value = record.fields[index].trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn parse_date(record: &CsvRecord, index: usize, field_name: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(record.fields[index].trim(), "%Y-%m-%d").map_err(|_| {
        anyhow!(
            "record {} field {field_name} must be YYYY-MM-DD",
            record.record_number
        )
    })
}

fn parse_compact_date(record: &CsvRecord, index: usize, field_name: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(record.fields[index].trim(), "%Y%m%d").map_err(|_| {
        anyhow!(
            "record {} field {field_name} must be YYYYMMDD",
            record.record_number
        )
    })
}

fn parse_nonnegative_i64(record: &CsvRecord, index: usize, field_name: &str) -> Result<i64> {
    let value = record.fields[index].trim().parse::<i64>().map_err(|_| {
        anyhow!(
            "record {} field {field_name} must be an integer",
            record.record_number
        )
    })?;
    if value < 0 {
        bail!(
            "record {} field {field_name} must be non-negative",
            record.record_number
        );
    }
    Ok(value)
}

fn parse_i16_range(
    record: &CsvRecord,
    index: usize,
    field_name: &str,
    minimum: i16,
    maximum: i16,
) -> Result<i16> {
    let value = record.fields[index].trim().parse::<i16>().map_err(|_| {
        anyhow!(
            "record {} field {field_name} must be an integer",
            record.record_number
        )
    })?;
    if !(minimum..=maximum).contains(&value) {
        bail!(
            "record {} field {field_name} is outside the allowed range",
            record.record_number
        );
    }
    Ok(value)
}

fn parse_entity_type(record: &CsvRecord, index: usize) -> Result<i16> {
    let value = parse_i16_range(record, index, "entityType", 0, i16::MAX)?;
    if !matches!(value, 20 | 40 | 60 | 70 | 80) {
        bail!(
            "record {} field entityType is unsupported",
            record.record_number
        );
    }
    Ok(value)
}

fn parse_optional_i16(record: &CsvRecord, index: usize, field_name: &str) -> Result<Option<i16>> {
    let value = record.fields[index].trim();
    if value.is_empty() {
        return Ok(None);
    }
    value.parse::<i16>().map(Some).map_err(|_| {
        anyhow!(
            "record {} field {field_name} must be an integer or empty",
            record.record_number
        )
    })
}

fn parse_nonnegative_decimal(record: &CsvRecord, index: usize, field_name: &str) -> Result<String> {
    let raw = record.fields[index].trim();
    let value = raw.parse::<f64>().map_err(|_| {
        anyhow!(
            "record {} field {field_name} must be numeric",
            record.record_number
        )
    })?;
    if !value.is_finite() || value < 0.0 {
        bail!(
            "record {} field {field_name} must be finite and non-negative",
            record.record_number
        );
    }
    Ok(raw.to_string())
}

fn parse_optional_nonnegative_decimal(
    record: &CsvRecord,
    index: usize,
    field_name: &str,
) -> Result<Option<String>> {
    if record.fields[index].trim().is_empty() {
        Ok(None)
    } else {
        parse_nonnegative_decimal(record, index, field_name).map(Some)
    }
}

fn parse_share(record: &CsvRecord, index: usize) -> Result<String> {
    let raw = record.fields[index].trim();
    let value = raw.parse::<f64>().map_err(|_| {
        anyhow!(
            "record {} field share must be numeric",
            record.record_number
        )
    })?;
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        bail!(
            "record {} field share must be between zero and one",
            record.record_number
        );
    }
    Ok(raw.to_string())
}

fn is_hour_interval(value: &str) -> bool {
    value.trim() == "H"
}

fn allowed_profile_bucket(dimension_key: &str, bucket_key: &str) -> bool {
    match dimension_key {
        "age" => matches!(
            bucket_key,
            "0-17" | "18-24" | "25-34" | "35-44" | "45-54" | "55-64" | "65+" | "unknown"
        ),
        "gender" => matches!(bucket_key, "female" | "male"),
        "group_type" => matches!(bucket_key, "couple" | "family" | "friend" | "single"),
        _ => false,
    }
}

fn expected_profile_buckets(dimension_key: &str) -> HashSet<String> {
    let values: &[&str] = match dimension_key {
        "age" => &[
            "0-17", "18-24", "25-34", "35-44", "45-54", "55-64", "65+", "unknown",
        ],
        "gender" => &["female", "male"],
        "group_type" => &["couple", "family", "friend", "single"],
        _ => &[],
    };
    values.iter().map(|value| (*value).to_string()).collect()
}

fn prepare_traffic(documents: &[SourceDocument]) -> Result<PreparedTraffic> {
    let traffic_document = document(documents, "traffic_hourly.csv");
    let inventory_document = document(documents, "point_inventory.csv");
    expect_header(&traffic_document.csv, &TRAFFIC_HEADER)?;
    expect_header(&inventory_document.csv, &INVENTORY_HEADER)?;

    let mut facts = Vec::with_capacity(traffic_document.csv.records.len());
    let mut fact_keys = HashSet::with_capacity(traffic_document.csv.records.len());
    let mut traffic_dates = HashSet::new();
    let mut traffic_hours_by_date = HashMap::<NaiveDate, HashSet<i16>>::new();
    let mut source_mall_ids = HashSet::new();
    let mut source_internal_mall_ids = HashSet::new();
    for record in &traffic_document.csv.records {
        let source_mall_id = required_text(record, 0, "source_mall_id")?;
        if source_mall_id.trim().to_ascii_uppercase() != MALL_CODE {
            bail!(
                "record {} field source_mall_id is outside the SZ02 snapshot",
                record.record_number
            );
        }
        source_mall_ids.insert(source_mall_id.clone());
        let source_internal_mall_id = required_text(record, 1, "mallId")?;
        source_internal_mall_ids.insert(source_internal_mall_id.clone());
        let entity_type = parse_entity_type(record, 2)?;
        let entity_type_name = optional_text(record, 3);
        let entity_name = required_text(record, 4, "entityName")?;
        let source_entity_id = required_text(record, 5, "entityId")?;
        let aibee_entity_id = required_text(record, 6, "aibeeEntityId")?;
        let business_date = parse_compact_date(record, 7, "day")?;
        traffic_dates.insert(business_date);
        let hour = parse_i16_range(record, 8, "hour", 0, 23)?;
        traffic_hours_by_date
            .entry(business_date)
            .or_default()
            .insert(hour);
        let minute = parse_i16_range(record, 9, "minute", 0, 59)?;
        if minute != 0 {
            bail!(
                "record {} field minute must be zero for hourly data",
                record.record_number
            );
        }
        if !is_hour_interval(&record.fields[10]) {
            bail!(
                "record {} field interval is not hourly",
                record.record_number
            );
        }
        let traffic_in = parse_nonnegative_i64(record, 11, "trafficIn")?;
        let traffic_out = parse_nonnegative_i64(record, 12, "trafficOut")?;
        let visitors = parse_nonnegative_i64(record, 13, "visitors")?;
        let average_stay = parse_nonnegative_decimal(record, 14, "averageStay")?;
        if average_stay.parse::<f64>().expect("validated averageStay") != 0.0 {
            bail!(
                "record {} field averageStay must be zero while its unit is unavailable",
                record.record_number
            );
        }
        let key = (
            entity_type,
            aibee_entity_id.clone(),
            business_date,
            hour,
            minute,
        );
        if !fact_keys.insert(key) {
            bail!(
                "duplicate traffic natural key at record {}",
                record.record_number
            );
        }
        facts.push(TrafficRow {
            source_document_id: traffic_document.id,
            source_row_number: record.record_number,
            source_mall_id,
            source_internal_mall_id,
            entity_type,
            entity_type_name,
            entity_name,
            source_entity_id,
            aibee_entity_id,
            business_date,
            hour,
            minute,
            traffic_in,
            traffic_out,
            visitors,
            average_stay,
            visitors_metric_available: entity_type == 40,
            average_stay_metric_available: false,
        });
    }
    if facts.is_empty() {
        bail!("traffic_hourly.csv has no data rows");
    }
    if source_mall_ids.len() != 1 || source_internal_mall_ids.len() != 1 {
        bail!("traffic_hourly.csv mixes multiple source mall identities");
    }
    let expected_internal_mall_id = source_internal_mall_ids
        .iter()
        .next()
        .expect("traffic rows are nonempty");

    let mut points = Vec::with_capacity(inventory_document.csv.records.len());
    let mut point_indexes = HashMap::<(i16, String), usize>::new();
    for record in &inventory_document.csv.records {
        let source_inventory_mall_id = required_text(record, 0, "mallId")?;
        if &source_inventory_mall_id != expected_internal_mall_id {
            bail!(
                "record {} inventory mallId differs from traffic mallId",
                record.record_number
            );
        }
        let entity_type = parse_entity_type(record, 1)?;
        let entity_name = required_text(record, 2, "entityName")?;
        let source_entity_id = required_text(record, 3, "entityId")?;
        let aibee_entity_id = required_text(record, 4, "aibeeEntityId")?;
        let identity = (entity_type, aibee_entity_id.clone());
        if point_indexes.contains_key(&identity) {
            bail!(
                "duplicate point inventory natural key at record {}",
                record.record_number
            );
        }
        let point_index = points.len();
        point_indexes.insert(identity, point_index);
        points.push(PointRow {
            source_document_id: inventory_document.id,
            source_row_number: record.record_number,
            source_inventory_mall_id: Some(source_inventory_mall_id),
            entity_type,
            entity_type_name: None,
            entity_name,
            source_entity_id,
            aibee_entity_id,
            floor_code: optional_text(record, 5),
            floor_name: optional_text(record, 6),
            entity_status: parse_optional_i16(record, 7, "entityStatus")?,
            area: parse_optional_nonnegative_decimal(record, 8, "area")?,
            l1_retail_format: optional_text(record, 9),
            l2_retail_format: optional_text(record, 10),
            inventory_present: true,
        });
    }
    if points.is_empty() {
        bail!("point_inventory.csv has no data rows");
    }

    let mut synthesized_point_count = 0usize;
    for fact in &facts {
        let identity = (fact.entity_type, fact.aibee_entity_id.clone());
        if let Some(point_index) = point_indexes.get(&identity).copied() {
            let point = &mut points[point_index];
            if point.entity_name != fact.entity_name
                || point.source_entity_id != fact.source_entity_id
            {
                bail!("traffic and inventory rows disagree on a point identity");
            }
            match (&point.entity_type_name, &fact.entity_type_name) {
                (None, Some(name)) => point.entity_type_name = Some(name.clone()),
                (Some(left), Some(right)) if left != right => {
                    bail!("traffic rows disagree on the entity type name of a point")
                }
                _ => {}
            }
            continue;
        }
        let point_index = points.len();
        point_indexes.insert(identity, point_index);
        points.push(PointRow {
            source_document_id: fact.source_document_id,
            source_row_number: fact.source_row_number,
            source_inventory_mall_id: None,
            entity_type: fact.entity_type,
            entity_type_name: fact.entity_type_name.clone(),
            entity_name: fact.entity_name.clone(),
            source_entity_id: fact.source_entity_id.clone(),
            aibee_entity_id: fact.aibee_entity_id.clone(),
            floor_code: None,
            floor_name: None,
            entity_status: None,
            area: None,
            l1_retail_format: None,
            l2_retail_format: None,
            inventory_present: false,
        });
        synthesized_point_count += 1;
    }

    let business_date_from = facts
        .iter()
        .map(|row| row.business_date)
        .min()
        .expect("facts are nonempty");
    let business_date_to = facts
        .iter()
        .map(|row| row.business_date)
        .max()
        .expect("facts are nonempty");
    let expected_day_count = (business_date_to - business_date_from).num_days() + 1;
    if traffic_dates.len() as i64 != expected_day_count {
        bail!("traffic_hourly.csv does not cover a contiguous date range");
    }
    let expected_hours = traffic_hours_by_date
        .get(&business_date_from)
        .filter(|hours| !hours.is_empty())
        .ok_or_else(|| anyhow!("traffic_hourly.csv has no hourly coverage"))?;
    if traffic_dates.iter().any(|date| {
        traffic_hours_by_date
            .get(date)
            .is_none_or(|hours| hours != expected_hours)
    }) {
        bail!("traffic_hourly.csv has inconsistent global hour coverage across dates");
    }
    Ok(PreparedTraffic {
        batch: batch_metadata(documents),
        facts,
        points,
        business_date_from,
        business_date_to,
        synthesized_point_count,
    })
}

fn prepare_profile(documents: &[SourceDocument]) -> Result<PreparedProfile> {
    let summary_document = document(documents, "daily_summary.csv");
    expect_header(&summary_document.csv, &SUMMARY_HEADER)?;
    let mut summaries = Vec::with_capacity(summary_document.csv.records.len());
    let mut summary_dates = HashSet::new();
    for record in &summary_document.csv.records {
        let business_date = parse_date(record, 0, "date")?;
        if !summary_dates.insert(business_date) {
            bail!(
                "duplicate profile daily summary natural key at record {}",
                record.record_number
            );
        }
        let record_count = parse_nonnegative_i64(record, 1, "record_count")?;
        let unique_pid_count = parse_nonnegative_i64(record, 2, "unique_pid_count")?;
        let duplicate_record_count = parse_nonnegative_i64(record, 3, "duplicate_record_count")?;
        let missing_pid_count = parse_nonnegative_i64(record, 4, "missing_pid_count")?;
        let date_mismatch_count = parse_nonnegative_i64(record, 5, "date_mismatch_count")?;
        let invalid_age_count = parse_nonnegative_i64(record, 6, "invalid_age_count")?;
        for (field_name, count) in [
            ("unique_pid_count", unique_pid_count),
            ("duplicate_record_count", duplicate_record_count),
            ("missing_pid_count", missing_pid_count),
            ("date_mismatch_count", date_mismatch_count),
            ("invalid_age_count", invalid_age_count),
        ] {
            if count > record_count {
                bail!(
                    "record {} field {field_name} exceeds record_count",
                    record.record_number
                );
            }
        }
        summaries.push(DailySummaryRow {
            source_document_id: summary_document.id,
            source_row_number: record.record_number,
            business_date,
            record_count,
            unique_pid_count,
            duplicate_record_count,
            missing_pid_count,
            date_mismatch_count,
            invalid_age_count,
        });
    }
    if summaries.is_empty() {
        bail!("daily_summary.csv has no data rows");
    }
    let business_date_from = summaries
        .iter()
        .map(|row| row.business_date)
        .min()
        .expect("summaries are nonempty");
    let business_date_to = summaries
        .iter()
        .map(|row| row.business_date)
        .max()
        .expect("summaries are nonempty");
    let expected_day_count = (business_date_to - business_date_from).num_days() + 1;
    if summary_dates.len() as i64 != expected_day_count {
        bail!("profile daily_summary.csv does not cover a contiguous date range");
    }

    let dimension_files = [
        (
            "age",
            "age_bucket",
            "age_distribution_daily.csv",
            "age_distribution_total.csv",
        ),
        (
            "gender",
            "gender",
            "gender_distribution_daily.csv",
            "gender_distribution_total.csv",
        ),
        (
            "group_type",
            "group_type",
            "group_type_distribution_daily.csv",
            "group_type_distribution_total.csv",
        ),
    ];
    let mut daily_distributions = Vec::new();
    let mut period_distributions = Vec::new();
    let mut daily_keys = HashSet::<(NaiveDate, &'static str, String)>::new();
    let mut period_keys = HashSet::<(&'static str, String)>::new();
    for (dimension_key, bucket_header, daily_title, total_title) in dimension_files {
        let daily_document = document(documents, daily_title);
        let total_document = document(documents, total_title);
        let daily_expected = ["date", bucket_header, "count", "share"];
        let total_expected = [bucket_header, "count", "share"];
        expect_header(&daily_document.csv, &daily_expected)?;
        expect_header(&total_document.csv, &total_expected)?;

        let mut total_buckets = HashSet::new();
        let mut period_share_sum = 0.0f64;
        for record in &total_document.csv.records {
            let bucket_key = required_text(record, 0, bucket_header)?;
            if !allowed_profile_bucket(dimension_key, &bucket_key) {
                bail!(
                    "record {} contains a bucket outside the {dimension_key} whitelist",
                    record.record_number
                );
            }
            if !period_keys.insert((dimension_key, bucket_key.clone())) {
                bail!(
                    "duplicate profile period distribution natural key at record {}",
                    record.record_number
                );
            }
            total_buckets.insert(bucket_key.clone());
            let share = parse_share(record, 2)?;
            period_share_sum += share.parse::<f64>().expect("validated share");
            period_distributions.push(PeriodDistributionRow {
                source_document_id: total_document.id,
                source_row_number: record.record_number,
                dimension_key,
                bucket_key,
                visitor_count: parse_nonnegative_i64(record, 1, "count")?,
                share,
            });
        }
        if total_buckets.is_empty() {
            bail!("{total_title} has no data rows");
        }
        if total_buckets != expected_profile_buckets(dimension_key) {
            bail!("{total_title} does not contain the complete approved bucket set");
        }
        if (period_share_sum - 1.0).abs() > 0.001 {
            bail!("{total_title} share values do not sum to one");
        }

        let mut buckets_by_date = HashMap::<NaiveDate, HashSet<String>>::new();
        let mut share_sum_by_date = HashMap::<NaiveDate, f64>::new();
        for record in &daily_document.csv.records {
            let business_date = parse_date(record, 0, "date")?;
            if !summary_dates.contains(&business_date) {
                bail!("{daily_title} contains a date outside daily_summary.csv");
            }
            let bucket_key = required_text(record, 1, bucket_header)?;
            if !allowed_profile_bucket(dimension_key, &bucket_key) {
                bail!(
                    "record {} contains a bucket outside the {dimension_key} whitelist",
                    record.record_number
                );
            }
            if !daily_keys.insert((business_date, dimension_key, bucket_key.clone())) {
                bail!(
                    "duplicate profile daily distribution natural key at record {}",
                    record.record_number
                );
            }
            buckets_by_date
                .entry(business_date)
                .or_default()
                .insert(bucket_key.clone());
            let share = parse_share(record, 3)?;
            *share_sum_by_date.entry(business_date).or_default() +=
                share.parse::<f64>().expect("validated share");
            daily_distributions.push(DailyDistributionRow {
                source_document_id: daily_document.id,
                source_row_number: record.record_number,
                business_date,
                dimension_key,
                bucket_key,
                visitor_count: parse_nonnegative_i64(record, 2, "count")?,
                share,
            });
        }
        if buckets_by_date.len() != summary_dates.len() {
            bail!("{daily_title} does not cover every daily_summary.csv date");
        }
        for date in &summary_dates {
            if buckets_by_date.get(date) != Some(&total_buckets) {
                bail!("{daily_title} bucket coverage differs from {total_title}");
            }
            if (share_sum_by_date.get(date).copied().unwrap_or_default() - 1.0).abs() > 0.001 {
                bail!("{daily_title} share values do not sum to one for every date");
            }
        }
    }
    validate_distribution_share_ratios(&daily_distributions, &period_distributions)?;

    Ok(PreparedProfile {
        batch: batch_metadata(documents),
        summaries,
        daily_distributions,
        period_distributions,
        business_date_from,
        business_date_to,
    })
}

fn validate_distribution_share_ratios(
    daily: &[DailyDistributionRow],
    period: &[PeriodDistributionRow],
) -> Result<()> {
    let mut daily_totals = HashMap::<(NaiveDate, &'static str), i64>::new();
    for row in daily {
        *daily_totals
            .entry((row.business_date, row.dimension_key))
            .or_default() += row.visitor_count;
    }
    for row in daily {
        let total = daily_totals[&(row.business_date, row.dimension_key)];
        if total <= 0 {
            bail!("profile daily distribution has no count denominator");
        }
        let expected = row.visitor_count as f64 / total as f64;
        let actual = row.share.parse::<f64>().expect("validated share");
        if (actual - expected).abs() > 0.001 {
            bail!("profile daily distribution share does not match its count ratio");
        }
    }

    let mut period_totals = HashMap::<&'static str, i64>::new();
    for row in period {
        *period_totals.entry(row.dimension_key).or_default() += row.visitor_count;
    }
    for row in period {
        let total = period_totals[&row.dimension_key];
        if total <= 0 {
            bail!("profile period distribution has no count denominator");
        }
        let expected = row.visitor_count as f64 / total as f64;
        let actual = row.share.parse::<f64>().expect("validated share");
        if (actual - expected).abs() > 0.001 {
            bail!("profile period distribution share does not match its count ratio");
        }
    }
    Ok(())
}

fn hash_prefix(value: &str) -> &str {
    value.get(..12).unwrap_or(value)
}

fn validate_matching_snapshot_ranges(
    traffic_from: NaiveDate,
    traffic_to: NaiveDate,
    profile_from: NaiveDate,
    profile_to: NaiveDate,
) -> Result<()> {
    if traffic_from != profile_from || traffic_to != profile_to {
        bail!("traffic and profile complete snapshots cover different date ranges");
    }
    Ok(())
}

fn validate_expected_snapshot_range(
    actual_from: NaiveDate,
    actual_to: NaiveDate,
    expected_from: NaiveDate,
    expected_to: NaiveDate,
) -> Result<()> {
    if actual_from != expected_from || actual_to != expected_to {
        bail!("validated snapshot does not match the explicit expected date range");
    }
    Ok(())
}

fn dataset_lock_keys(tenant_id: Uuid, dataset_ids: [Uuid; 2]) -> [String; 2] {
    let mut dataset_ids = dataset_ids;
    dataset_ids.sort();
    dataset_ids.map(|dataset_id| format!("mall_sz02:{tenant_id}:{MALL_CODE}:{dataset_id}"))
}

fn build_output(
    args: &Args,
    traffic: &PreparedTraffic,
    profile: &PreparedProfile,
    batch_ids: Option<(Uuid, Uuid)>,
) -> Value {
    let (traffic_batch_id, profile_batch_id) = batch_ids
        .map(|(traffic, profile)| (Some(traffic), Some(profile)))
        .unwrap_or((None, None));
    json!({
        "status": if args.dry_run { "validated" } else { "ready" },
        "dry_run": args.dry_run,
        "mall_code": MALL_CODE,
        "expected_date_from": args.expected_date_from,
        "expected_date_to": args.expected_date_to,
        "traffic": {
            "dataset_id": args.traffic_dataset_id,
            "source_kind": TRAFFIC_SOURCE_KIND,
            "batch_id": traffic_batch_id,
            "combined_hash_prefix": hash_prefix(&traffic.batch.combined_sha256),
            "source_file_count": traffic.batch.source_file_count,
            "source_row_count": traffic.batch.source_row_count,
            "fact_row_count": traffic.facts.len(),
            "point_row_count": traffic.points.len(),
            "synthesized_point_count": traffic.synthesized_point_count,
            "business_date_from": traffic.business_date_from,
            "business_date_to": traffic.business_date_to,
        },
        "profile": {
            "dataset_id": args.profile_dataset_id,
            "source_kind": PROFILE_SOURCE_KIND,
            "batch_id": profile_batch_id,
            "combined_hash_prefix": hash_prefix(&profile.batch.combined_sha256),
            "source_file_count": profile.batch.source_file_count,
            "source_row_count": profile.batch.source_row_count,
            "daily_summary_row_count": profile.summaries.len(),
            "daily_distribution_row_count": profile.daily_distributions.len(),
            "period_distribution_row_count": profile.period_distributions.len(),
            "business_date_from": profile.business_date_from,
            "business_date_to": profile.business_date_to,
        }
    })
}

async fn import_all(
    pool: &PgPool,
    tenant_id: Uuid,
    args: &Args,
    traffic: &PreparedTraffic,
    profile: &PreparedProfile,
) -> Result<(Uuid, Uuid)> {
    let mut tx = pool.begin().await.context("begin analytics import")?;
    for lock_key in dataset_lock_keys(
        tenant_id,
        [args.traffic_dataset_id, args.profile_dataset_id],
    ) {
        sqlx::query("select pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(lock_key)
            .execute(&mut *tx)
            .await
            .context("acquire dataset snapshot import lock")?;
    }
    ensure_snapshot_does_not_shrink(
        &mut tx,
        tenant_id,
        args.traffic_dataset_id,
        TRAFFIC_SOURCE_KIND,
        traffic.business_date_from,
        traffic.business_date_to,
    )
    .await?;
    ensure_snapshot_does_not_shrink(
        &mut tx,
        tenant_id,
        args.profile_dataset_id,
        PROFILE_SOURCE_KIND,
        profile.business_date_from,
        profile.business_date_to,
    )
    .await?;

    let traffic_loaded_count = (traffic.facts.len() + traffic.points.len()) as i64;
    let profile_loaded_count = (profile.summaries.len()
        + profile.daily_distributions.len()
        + profile.period_distributions.len()) as i64;
    let traffic_batch_id = prepare_batch(
        &mut tx,
        tenant_id,
        args.traffic_dataset_id,
        TRAFFIC_SOURCE_KIND,
        &traffic.batch,
        traffic.business_date_from,
        traffic.business_date_to,
        traffic_loaded_count,
        json!([
            "canonical_paths",
            "exact_source_set",
            "exact_headers",
            "typed_values",
            "non_negative_metrics",
            "natural_keys_unique",
            "contiguous_date_range",
            "hourly_interval_H",
            "minute_zero",
            "point_union_complete",
            "manifest_contains_no_row_values"
        ]),
    )
    .await?;
    let profile_batch_id = prepare_batch(
        &mut tx,
        tenant_id,
        args.profile_dataset_id,
        PROFILE_SOURCE_KIND,
        &profile.batch,
        profile.business_date_from,
        profile.business_date_to,
        profile_loaded_count,
        json!([
            "canonical_paths",
            "exact_source_set",
            "exact_headers",
            "typed_values",
            "non_negative_metrics",
            "natural_keys_unique",
            "contiguous_date_range",
            "daily_bucket_coverage_complete",
            "bucket_whitelist_validated",
            "share_totals_validated",
            "manifest_contains_no_row_values"
        ]),
    )
    .await?;

    upsert_points(
        &mut tx,
        tenant_id,
        args.traffic_dataset_id,
        traffic_batch_id,
        &traffic.points,
    )
    .await?;
    upsert_traffic_facts(
        &mut tx,
        tenant_id,
        args.traffic_dataset_id,
        traffic_batch_id,
        &traffic.facts,
    )
    .await?;
    upsert_profile_summaries(
        &mut tx,
        tenant_id,
        args.profile_dataset_id,
        profile_batch_id,
        &profile.summaries,
    )
    .await?;
    upsert_profile_daily_distributions(
        &mut tx,
        tenant_id,
        args.profile_dataset_id,
        profile_batch_id,
        &profile.daily_distributions,
    )
    .await?;
    upsert_profile_period_distributions(
        &mut tx,
        tenant_id,
        args.profile_dataset_id,
        profile_batch_id,
        profile.business_date_from,
        profile.business_date_to,
        &profile.period_distributions,
    )
    .await?;

    delete_stale_snapshot_rows(
        &mut tx,
        tenant_id,
        args.traffic_dataset_id,
        traffic_batch_id,
        args.profile_dataset_id,
        profile_batch_id,
    )
    .await?;
    verify_persisted_snapshot(
        &mut tx,
        tenant_id,
        args,
        traffic,
        traffic_batch_id,
        profile,
        profile_batch_id,
    )
    .await?;

    mark_batch_ready(
        &mut tx,
        tenant_id,
        args.traffic_dataset_id,
        traffic_batch_id,
        TRAFFIC_SOURCE_KIND,
        traffic_loaded_count,
    )
    .await?;
    mark_batch_ready(
        &mut tx,
        tenant_id,
        args.profile_dataset_id,
        profile_batch_id,
        PROFILE_SOURCE_KIND,
        profile_loaded_count,
    )
    .await?;
    tx.commit().await.context("commit analytics import")?;
    Ok((traffic_batch_id, profile_batch_id))
}

async fn ensure_snapshot_does_not_shrink(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    dataset_id: Uuid,
    source_kind: &str,
    candidate_from: NaiveDate,
    candidate_to: NaiveDate,
) -> Result<()> {
    let existing = sqlx::query(
        r#"
        select business_date_from, business_date_to
        from mall_sz02.ingest_batch
        where tenant_id = $1 and dataset_id = $2 and mall_code = $3
          and source_kind = $4 and status = 'ready'
        order by completed_at desc, updated_at desc, created_at desc
        limit 1
        for update
        "#,
    )
    .bind(tenant_id)
    .bind(dataset_id)
    .bind(MALL_CODE)
    .bind(source_kind)
    .fetch_optional(&mut **tx)
    .await
    .context("load current ready snapshot range")?;
    if let Some(row) = existing {
        let existing_from = row.get::<Option<NaiveDate>, _>("business_date_from");
        let existing_to = row.get::<Option<NaiveDate>, _>("business_date_to");
        match (existing_from, existing_to) {
            (Some(existing_from), Some(existing_to)) => validate_nonshrinking_range(
                existing_from,
                existing_to,
                candidate_from,
                candidate_to,
            )?,
            _ => bail!("current ready snapshot is missing its date range"),
        }
    }
    Ok(())
}

fn validate_nonshrinking_range(
    existing_from: NaiveDate,
    existing_to: NaiveDate,
    candidate_from: NaiveDate,
    candidate_to: NaiveDate,
) -> Result<()> {
    if candidate_from > existing_from || candidate_to < existing_to {
        bail!("candidate complete snapshot would shrink the current ready date range");
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn prepare_batch(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    dataset_id: Uuid,
    source_kind: &str,
    metadata: &BatchMetadata,
    date_from: NaiveDate,
    date_to: NaiveDate,
    loaded_row_count: i64,
    quality_flags: Value,
) -> Result<Uuid> {
    let batch_key = format!("v1:{source_kind}:{}", metadata.combined_sha256);
    let existing = sqlx::query(
        r#"
        select id, batch_key, combined_content_sha256
        from mall_sz02.ingest_batch
        where tenant_id = $1
          and dataset_id = $2
          and mall_code = $3
          and (batch_key = $4 or combined_content_sha256 = $5)
        for update
        "#,
    )
    .bind(tenant_id)
    .bind(dataset_id)
    .bind(MALL_CODE)
    .bind(&batch_key)
    .bind(&metadata.combined_sha256)
    .fetch_all(&mut **tx)
    .await
    .context("lock matching ingest batch")?;
    if existing.len() > 1 {
        bail!("batch key and content hash resolve to different ingest batches");
    }
    let batch_id = if let Some(row) = existing.first() {
        let existing_key = row.get::<String, _>("batch_key");
        let existing_hash = row.get::<String, _>("combined_content_sha256");
        if existing_key != batch_key || existing_hash != metadata.combined_sha256 {
            bail!("existing ingest batch has conflicting idempotency keys");
        }
        let id = row.get::<Uuid, _>("id");
        sqlx::query(
            r#"
            update mall_sz02.ingest_batch
            set source_kind = $5,
                status = 'loading',
                file_manifest = $6,
                source_file_count = $7,
                source_row_count = $8,
                loaded_row_count = $9,
                rejected_row_count = 0,
                business_date_from = $10,
                business_date_to = $11,
                quality_flags = $12,
                failure_message = null,
                started_at = now(),
                completed_at = null,
                updated_at = now()
            where tenant_id = $1 and dataset_id = $2 and mall_code = $3 and id = $4
            "#,
        )
        .bind(tenant_id)
        .bind(dataset_id)
        .bind(MALL_CODE)
        .bind(id)
        .bind(source_kind)
        .bind(&metadata.manifest)
        .bind(metadata.source_file_count)
        .bind(metadata.source_row_count)
        .bind(loaded_row_count)
        .bind(date_from)
        .bind(date_to)
        .bind(quality_flags)
        .execute(&mut **tx)
        .await
        .context("refresh matching ingest batch")?;
        id
    } else {
        sqlx::query_scalar::<_, Uuid>(
            r#"
            insert into mall_sz02.ingest_batch (
                tenant_id, dataset_id, mall_code, batch_key, source_kind, status,
                file_manifest, combined_content_sha256, source_file_count,
                source_row_count, loaded_row_count, rejected_row_count,
                business_date_from, business_date_to, quality_flags,
                failure_message, started_at, completed_at
            ) values (
                $1, $2, $3, $4, $5, 'loading', $6, $7, $8,
                $9, $10, 0, $11, $12, $13, null, now(), null
            )
            returning id
            "#,
        )
        .bind(tenant_id)
        .bind(dataset_id)
        .bind(MALL_CODE)
        .bind(&batch_key)
        .bind(source_kind)
        .bind(&metadata.manifest)
        .bind(&metadata.combined_sha256)
        .bind(metadata.source_file_count)
        .bind(metadata.source_row_count)
        .bind(loaded_row_count)
        .bind(date_from)
        .bind(date_to)
        .bind(quality_flags)
        .fetch_one(&mut **tx)
        .await
        .context("create ingest batch")?
    };
    Ok(batch_id)
}

async fn upsert_points(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    dataset_id: Uuid,
    batch_id: Uuid,
    rows: &[PointRow],
) -> Result<()> {
    for rows in rows.chunks(INSERT_CHUNK_SIZE) {
        let mut query = QueryBuilder::<Postgres>::new(
            r#"insert into mall_sz02.traffic_point_dim (
                tenant_id, dataset_id, batch_id, source_document_id, source_row_number,
                mall_code, source_inventory_mall_id, entity_type, entity_type_name,
                entity_name, source_entity_id, aibee_entity_id, floor_code, floor_name,
                entity_status, area, l1_retail_format, l2_retail_format, inventory_present
            ) "#,
        );
        query.push_values(rows, |mut values, row| {
            values
                .push_bind(tenant_id)
                .push_bind(dataset_id)
                .push_bind(batch_id)
                .push_bind(row.source_document_id)
                .push_bind(row.source_row_number)
                .push_bind(MALL_CODE)
                .push_bind(row.source_inventory_mall_id.as_deref())
                .push_bind(row.entity_type)
                .push_bind(row.entity_type_name.as_deref())
                .push_bind(&row.entity_name)
                .push_bind(&row.source_entity_id)
                .push_bind(&row.aibee_entity_id)
                .push_bind(row.floor_code.as_deref())
                .push_bind(row.floor_name.as_deref())
                .push_bind(row.entity_status)
                .push_bind(row.area.as_deref())
                .push_unseparated("::numeric")
                .push_bind(row.l1_retail_format.as_deref())
                .push_bind(row.l2_retail_format.as_deref())
                .push_bind(row.inventory_present);
        });
        query.push(
            r#" on conflict (tenant_id, dataset_id, mall_code, entity_type, aibee_entity_id)
            do update set
                batch_id = excluded.batch_id,
                source_document_id = excluded.source_document_id,
                source_row_number = excluded.source_row_number,
                source_inventory_mall_id = excluded.source_inventory_mall_id,
                entity_type_name = excluded.entity_type_name,
                entity_name = excluded.entity_name,
                source_entity_id = excluded.source_entity_id,
                floor_code = excluded.floor_code,
                floor_name = excluded.floor_name,
                entity_status = excluded.entity_status,
                area = excluded.area,
                l1_retail_format = excluded.l1_retail_format,
                l2_retail_format = excluded.l2_retail_format,
                inventory_present = excluded.inventory_present,
                updated_at = now()"#,
        );
        query
            .build()
            .execute(&mut **tx)
            .await
            .context("upsert traffic points")?;
    }
    Ok(())
}

async fn upsert_traffic_facts(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    dataset_id: Uuid,
    batch_id: Uuid,
    rows: &[TrafficRow],
) -> Result<()> {
    for rows in rows.chunks(INSERT_CHUNK_SIZE) {
        let mut query = QueryBuilder::<Postgres>::new(
            r#"insert into mall_sz02.traffic_hourly_fact (
                tenant_id, dataset_id, batch_id, source_document_id, source_row_number,
                mall_code, source_mall_id, source_internal_mall_id, entity_type,
                entity_type_name, entity_name, source_entity_id, aibee_entity_id,
                business_date, hour, minute, interval_code, traffic_in, traffic_out,
                visitors, average_stay, visitors_metric_available,
                average_stay_metric_available
            ) "#,
        );
        query.push_values(rows, |mut values, row| {
            values
                .push_bind(tenant_id)
                .push_bind(dataset_id)
                .push_bind(batch_id)
                .push_bind(row.source_document_id)
                .push_bind(row.source_row_number)
                .push_bind(MALL_CODE)
                .push_bind(&row.source_mall_id)
                .push_bind(&row.source_internal_mall_id)
                .push_bind(row.entity_type)
                .push_bind(row.entity_type_name.as_deref())
                .push_bind(&row.entity_name)
                .push_bind(&row.source_entity_id)
                .push_bind(&row.aibee_entity_id)
                .push_bind(row.business_date)
                .push_bind(row.hour)
                .push_bind(row.minute)
                .push_bind("H")
                .push_bind(row.traffic_in)
                .push_bind(row.traffic_out)
                .push_bind(row.visitors)
                .push_bind(&row.average_stay)
                .push_unseparated("::numeric")
                .push_bind(row.visitors_metric_available)
                .push_bind(row.average_stay_metric_available);
        });
        query.push(
            r#" on conflict (
                tenant_id, dataset_id, mall_code, entity_type, aibee_entity_id,
                business_date, hour, minute, interval_code
            ) do update set
                batch_id = excluded.batch_id,
                source_document_id = excluded.source_document_id,
                source_row_number = excluded.source_row_number,
                source_mall_id = excluded.source_mall_id,
                source_internal_mall_id = excluded.source_internal_mall_id,
                entity_type_name = excluded.entity_type_name,
                entity_name = excluded.entity_name,
                source_entity_id = excluded.source_entity_id,
                traffic_in = excluded.traffic_in,
                traffic_out = excluded.traffic_out,
                visitors = excluded.visitors,
                average_stay = excluded.average_stay,
                visitors_metric_available = excluded.visitors_metric_available,
                average_stay_metric_available = excluded.average_stay_metric_available,
                updated_at = now()"#,
        );
        query
            .build()
            .execute(&mut **tx)
            .await
            .context("upsert hourly traffic facts")?;
    }
    Ok(())
}

async fn upsert_profile_summaries(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    dataset_id: Uuid,
    batch_id: Uuid,
    rows: &[DailySummaryRow],
) -> Result<()> {
    for rows in rows.chunks(INSERT_CHUNK_SIZE) {
        let mut query = QueryBuilder::<Postgres>::new(
            r#"insert into mall_sz02.profile_daily_summary (
                tenant_id, dataset_id, batch_id, source_document_id, source_row_number,
                mall_code, business_date, record_count, unique_pid_count,
                duplicate_record_count, missing_pid_count, date_mismatch_count,
                invalid_age_count
            ) "#,
        );
        query.push_values(rows, |mut values, row| {
            values
                .push_bind(tenant_id)
                .push_bind(dataset_id)
                .push_bind(batch_id)
                .push_bind(row.source_document_id)
                .push_bind(row.source_row_number)
                .push_bind(MALL_CODE)
                .push_bind(row.business_date)
                .push_bind(row.record_count)
                .push_bind(row.unique_pid_count)
                .push_bind(row.duplicate_record_count)
                .push_bind(row.missing_pid_count)
                .push_bind(row.date_mismatch_count)
                .push_bind(row.invalid_age_count);
        });
        query.push(
            r#" on conflict (tenant_id, dataset_id, mall_code, business_date)
            do update set
                batch_id = excluded.batch_id,
                source_document_id = excluded.source_document_id,
                source_row_number = excluded.source_row_number,
                record_count = excluded.record_count,
                unique_pid_count = excluded.unique_pid_count,
                duplicate_record_count = excluded.duplicate_record_count,
                missing_pid_count = excluded.missing_pid_count,
                date_mismatch_count = excluded.date_mismatch_count,
                invalid_age_count = excluded.invalid_age_count,
                updated_at = now()"#,
        );
        query
            .build()
            .execute(&mut **tx)
            .await
            .context("upsert profile daily summaries")?;
    }
    Ok(())
}

async fn upsert_profile_daily_distributions(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    dataset_id: Uuid,
    batch_id: Uuid,
    rows: &[DailyDistributionRow],
) -> Result<()> {
    for rows in rows.chunks(INSERT_CHUNK_SIZE) {
        let mut query = QueryBuilder::<Postgres>::new(
            r#"insert into mall_sz02.profile_distribution_daily (
                tenant_id, dataset_id, batch_id, source_document_id, source_row_number,
                mall_code, business_date, dimension_key, bucket_key, visitor_count, share
            ) "#,
        );
        query.push_values(rows, |mut values, row| {
            values
                .push_bind(tenant_id)
                .push_bind(dataset_id)
                .push_bind(batch_id)
                .push_bind(row.source_document_id)
                .push_bind(row.source_row_number)
                .push_bind(MALL_CODE)
                .push_bind(row.business_date)
                .push_bind(row.dimension_key)
                .push_bind(&row.bucket_key)
                .push_bind(row.visitor_count)
                .push_bind(&row.share)
                .push_unseparated("::numeric");
        });
        query.push(
            r#" on conflict (
                tenant_id, dataset_id, mall_code, business_date, dimension_key, bucket_key
            ) do update set
                batch_id = excluded.batch_id,
                source_document_id = excluded.source_document_id,
                source_row_number = excluded.source_row_number,
                visitor_count = excluded.visitor_count,
                share = excluded.share,
                updated_at = now()"#,
        );
        query
            .build()
            .execute(&mut **tx)
            .await
            .context("upsert profile daily distributions")?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn upsert_profile_period_distributions(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    dataset_id: Uuid,
    batch_id: Uuid,
    period_start_date: NaiveDate,
    period_end_date: NaiveDate,
    rows: &[PeriodDistributionRow],
) -> Result<()> {
    for rows in rows.chunks(INSERT_CHUNK_SIZE) {
        let mut query = QueryBuilder::<Postgres>::new(
            r#"insert into mall_sz02.profile_distribution_period (
                tenant_id, dataset_id, batch_id, source_document_id, source_row_number,
                mall_code, period_start_date, period_end_date, dimension_key, bucket_key,
                visitor_count, share
            ) "#,
        );
        query.push_values(rows, |mut values, row| {
            values
                .push_bind(tenant_id)
                .push_bind(dataset_id)
                .push_bind(batch_id)
                .push_bind(row.source_document_id)
                .push_bind(row.source_row_number)
                .push_bind(MALL_CODE)
                .push_bind(period_start_date)
                .push_bind(period_end_date)
                .push_bind(row.dimension_key)
                .push_bind(&row.bucket_key)
                .push_bind(row.visitor_count)
                .push_bind(&row.share)
                .push_unseparated("::numeric");
        });
        query.push(
            r#" on conflict (
                tenant_id, dataset_id, mall_code, period_start_date, period_end_date,
                dimension_key, bucket_key
            ) do update set
                batch_id = excluded.batch_id,
                source_document_id = excluded.source_document_id,
                source_row_number = excluded.source_row_number,
                visitor_count = excluded.visitor_count,
                share = excluded.share,
                updated_at = now()"#,
        );
        query
            .build()
            .execute(&mut **tx)
            .await
            .context("upsert profile period distributions")?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn delete_stale_snapshot_rows(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    traffic_dataset_id: Uuid,
    traffic_batch_id: Uuid,
    profile_dataset_id: Uuid,
    profile_batch_id: Uuid,
) -> Result<()> {
    for (table, sql) in [
        (
            "traffic_hourly_fact",
            "delete from mall_sz02.traffic_hourly_fact where tenant_id = $1 and dataset_id = $2 and mall_code = $3 and batch_id <> $4",
        ),
        (
            "traffic_point_dim",
            "delete from mall_sz02.traffic_point_dim where tenant_id = $1 and dataset_id = $2 and mall_code = $3 and batch_id <> $4",
        ),
    ] {
        sqlx::query(sql)
            .bind(tenant_id)
            .bind(traffic_dataset_id)
            .bind(MALL_CODE)
            .bind(traffic_batch_id)
            .execute(&mut **tx)
            .await
            .with_context(|| format!("remove stale rows from {table}"))?;
    }
    for (table, sql) in [
        (
            "profile_daily_summary",
            "delete from mall_sz02.profile_daily_summary where tenant_id = $1 and dataset_id = $2 and mall_code = $3 and batch_id <> $4",
        ),
        (
            "profile_distribution_daily",
            "delete from mall_sz02.profile_distribution_daily where tenant_id = $1 and dataset_id = $2 and mall_code = $3 and batch_id <> $4",
        ),
        (
            "profile_distribution_period",
            "delete from mall_sz02.profile_distribution_period where tenant_id = $1 and dataset_id = $2 and mall_code = $3 and batch_id <> $4",
        ),
    ] {
        sqlx::query(sql)
            .bind(tenant_id)
            .bind(profile_dataset_id)
            .bind(MALL_CODE)
            .bind(profile_batch_id)
            .execute(&mut **tx)
            .await
            .with_context(|| format!("remove stale rows from {table}"))?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn verify_persisted_snapshot(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    args: &Args,
    traffic: &PreparedTraffic,
    traffic_batch_id: Uuid,
    profile: &PreparedProfile,
    profile_batch_id: Uuid,
) -> Result<()> {
    let traffic_state = sqlx::query(
        r#"
        select count(*)::bigint as row_count,
               min(business_date) as date_from,
               max(business_date) as date_to
        from mall_sz02.traffic_hourly_fact
        where tenant_id = $1 and dataset_id = $2 and mall_code = $3 and batch_id = $4
        "#,
    )
    .bind(tenant_id)
    .bind(args.traffic_dataset_id)
    .bind(MALL_CODE)
    .bind(traffic_batch_id)
    .fetch_one(&mut **tx)
    .await
    .context("verify persisted traffic facts")?;
    verify_count_and_range(
        &traffic_state,
        traffic.facts.len(),
        traffic.business_date_from,
        traffic.business_date_to,
        "traffic facts",
    )?;
    let point_count = sqlx::query_scalar::<_, i64>(
        r#"
        select count(*)::bigint
        from mall_sz02.traffic_point_dim
        where tenant_id = $1 and dataset_id = $2 and mall_code = $3 and batch_id = $4
        "#,
    )
    .bind(tenant_id)
    .bind(args.traffic_dataset_id)
    .bind(MALL_CODE)
    .bind(traffic_batch_id)
    .fetch_one(&mut **tx)
    .await
    .context("verify persisted traffic points")?;
    if point_count != traffic.points.len() as i64 {
        bail!("persisted traffic point count differs from the validated snapshot");
    }
    let missing_point_count = sqlx::query_scalar::<_, i64>(
        r#"
        select count(*)::bigint
        from mall_sz02.traffic_hourly_fact f
        where f.tenant_id = $1 and f.dataset_id = $2 and f.mall_code = $3 and f.batch_id = $4
          and not exists (
              select 1 from mall_sz02.traffic_point_dim p
              where p.tenant_id = f.tenant_id and p.dataset_id = f.dataset_id
                and p.mall_code = f.mall_code and p.entity_type = f.entity_type
                and p.aibee_entity_id = f.aibee_entity_id and p.batch_id = f.batch_id
          )
        "#,
    )
    .bind(tenant_id)
    .bind(args.traffic_dataset_id)
    .bind(MALL_CODE)
    .bind(traffic_batch_id)
    .fetch_one(&mut **tx)
    .await
    .context("verify traffic point coverage")?;
    if missing_point_count != 0 {
        bail!("persisted traffic snapshot contains facts without point dimensions");
    }

    let summary_state = sqlx::query(
        r#"
        select count(*)::bigint as row_count,
               min(business_date) as date_from,
               max(business_date) as date_to
        from mall_sz02.profile_daily_summary
        where tenant_id = $1 and dataset_id = $2 and mall_code = $3 and batch_id = $4
        "#,
    )
    .bind(tenant_id)
    .bind(args.profile_dataset_id)
    .bind(MALL_CODE)
    .bind(profile_batch_id)
    .fetch_one(&mut **tx)
    .await
    .context("verify persisted profile summaries")?;
    verify_count_and_range(
        &summary_state,
        profile.summaries.len(),
        profile.business_date_from,
        profile.business_date_to,
        "profile summaries",
    )?;
    let daily_state = sqlx::query(
        r#"
        select count(*)::bigint as row_count,
               min(business_date) as date_from,
               max(business_date) as date_to
        from mall_sz02.profile_distribution_daily
        where tenant_id = $1 and dataset_id = $2 and mall_code = $3 and batch_id = $4
        "#,
    )
    .bind(tenant_id)
    .bind(args.profile_dataset_id)
    .bind(MALL_CODE)
    .bind(profile_batch_id)
    .fetch_one(&mut **tx)
    .await
    .context("verify persisted daily distributions")?;
    verify_count_and_range(
        &daily_state,
        profile.daily_distributions.len(),
        profile.business_date_from,
        profile.business_date_to,
        "profile daily distributions",
    )?;
    let period_state = sqlx::query(
        r#"
        select count(*)::bigint as row_count,
               min(period_start_date) as date_from,
               max(period_end_date) as date_to
        from mall_sz02.profile_distribution_period
        where tenant_id = $1 and dataset_id = $2 and mall_code = $3 and batch_id = $4
        "#,
    )
    .bind(tenant_id)
    .bind(args.profile_dataset_id)
    .bind(MALL_CODE)
    .bind(profile_batch_id)
    .fetch_one(&mut **tx)
    .await
    .context("verify persisted period distributions")?;
    verify_count_and_range(
        &period_state,
        profile.period_distributions.len(),
        profile.business_date_from,
        profile.business_date_to,
        "profile period distributions",
    )?;
    Ok(())
}

fn verify_count_and_range(
    row: &sqlx::postgres::PgRow,
    expected_count: usize,
    expected_from: NaiveDate,
    expected_to: NaiveDate,
    label: &str,
) -> Result<()> {
    let count = row.get::<i64, _>("row_count");
    let date_from = row.get::<Option<NaiveDate>, _>("date_from");
    let date_to = row.get::<Option<NaiveDate>, _>("date_to");
    if count != expected_count as i64
        || date_from != Some(expected_from)
        || date_to != Some(expected_to)
    {
        bail!("persisted {label} count or date range differs from the validated snapshot");
    }
    Ok(())
}

async fn mark_batch_ready(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
    dataset_id: Uuid,
    batch_id: Uuid,
    source_kind: &str,
    loaded_row_count: i64,
) -> Result<()> {
    sqlx::query(
        r#"
        update mall_sz02.ingest_batch
        set status = 'superseded', updated_at = now()
        where tenant_id = $1 and dataset_id = $2 and mall_code = $3
          and source_kind = $4 and id <> $5 and status = 'ready'
        "#,
    )
    .bind(tenant_id)
    .bind(dataset_id)
    .bind(MALL_CODE)
    .bind(source_kind)
    .bind(batch_id)
    .execute(&mut **tx)
    .await
    .context("supersede prior ready ingest batches")?;
    let updated = sqlx::query(
        r#"
        update mall_sz02.ingest_batch
        set status = 'ready', loaded_row_count = $5, rejected_row_count = 0,
            failure_message = null, completed_at = now(), updated_at = now()
        where tenant_id = $1 and dataset_id = $2 and mall_code = $3 and id = $4
        "#,
    )
    .bind(tenant_id)
    .bind(dataset_id)
    .bind(MALL_CODE)
    .bind(batch_id)
    .bind(loaded_row_count)
    .execute(&mut **tx)
    .await
    .context("mark ingest batch ready")?;
    if updated.rows_affected() != 1 {
        bail!("ingest batch disappeared before it could be marked ready");
    }
    Ok(())
}

// Pure validation tests follow below.

#[cfg(test)]
mod tests {
    use super::*;

    fn record(fields: &[&str]) -> CsvRecord {
        CsvRecord {
            record_number: 2,
            fields: fields.iter().map(|value| (*value).to_string()).collect(),
        }
    }

    fn source_document(title: &str, body: &str) -> SourceDocument {
        SourceDocument {
            id: Uuid::new_v4(),
            title: title.to_string(),
            sha256: sha256_hex(body.as_bytes()),
            csv: parse_csv(body.as_bytes()).expect("fixture CSV should parse"),
        }
    }

    fn traffic_documents(traffic_rows: &[&str], inventory_rows: &[&str]) -> Vec<SourceDocument> {
        let traffic = std::iter::once(TRAFFIC_HEADER.join(","))
            .chain(traffic_rows.iter().map(|row| (*row).to_string()))
            .collect::<Vec<_>>()
            .join("\r\n");
        let inventory = std::iter::once(INVENTORY_HEADER.join(","))
            .chain(inventory_rows.iter().map(|row| (*row).to_string()))
            .collect::<Vec<_>>()
            .join("\r\n");
        vec![
            source_document("traffic_hourly.csv", &traffic),
            source_document("point_inventory.csv", &inventory),
        ]
    }

    fn valid_profile_documents() -> Vec<SourceDocument> {
        let age_rows = [
            ("0-17", 10, "0.1"),
            ("18-24", 10, "0.1"),
            ("25-34", 10, "0.1"),
            ("35-44", 10, "0.1"),
            ("45-54", 10, "0.1"),
            ("55-64", 10, "0.1"),
            ("65+", 20, "0.2"),
            ("unknown", 20, "0.2"),
        ];
        let gender_rows = [("female", 50, "0.5"), ("male", 50, "0.5")];
        let group_rows = [
            ("couple", 25, "0.25"),
            ("family", 25, "0.25"),
            ("friend", 25, "0.25"),
            ("single", 25, "0.25"),
        ];
        let mut documents = vec![source_document(
            "daily_summary.csv",
            "date,record_count,unique_pid_count,duplicate_record_count,missing_pid_count,date_mismatch_count,invalid_age_count\n2026-07-01,100,100,0,0,0,0\n",
        )];
        for (dimension, bucket_header, rows) in [
            ("age", "age_bucket", age_rows.as_slice()),
            ("gender", "gender", gender_rows.as_slice()),
            ("group_type", "group_type", group_rows.as_slice()),
        ] {
            let daily_body =
                std::iter::once(format!("date,{bucket_header},count,share"))
                    .chain(rows.iter().map(|(bucket, count, share)| {
                        format!("2026-07-01,{bucket},{count},{share}")
                    }))
                    .collect::<Vec<_>>()
                    .join("\n");
            let total_body = std::iter::once(format!("{bucket_header},count,share"))
                .chain(
                    rows.iter()
                        .map(|(bucket, count, share)| format!("{bucket},{count},{share}")),
                )
                .collect::<Vec<_>>()
                .join("\n");
            documents.push(source_document(
                &format!("{dimension}_distribution_daily.csv"),
                &daily_body,
            ));
            documents.push(source_document(
                &format!("{dimension}_distribution_total.csv"),
                &total_body,
            ));
        }
        documents
    }

    #[test]
    fn parses_bom_quoted_delimiter_and_crlf() {
        let csv =
            parse_csv(b"\xef\xbb\xbfid,name,remark\r\n1,\"East, Gate\",\"say \"\"hi\"\"\"\r\n")
                .expect("valid CSV should parse");
        assert_eq!(csv.header, ["id", "name", "remark"]);
        assert_eq!(csv.records.len(), 1);
        assert_eq!(csv.records[0].fields[1], "East, Gate");
        assert_eq!(csv.records[0].fields[2], "say \"hi\"");
    }

    #[test]
    fn rejects_bad_quotes_and_header_mismatch() {
        let error = parse_csv(b"a,b\r\n1,\"unterminated\r\n").expect_err("bad quote must fail");
        assert!(error.to_string().contains("unterminated"));

        let csv = parse_csv(b"day,trafficIn\n20260701,1\n").expect("CSV should parse");
        let error = expect_header(&csv, &TRAFFIC_HEADER).expect_err("wrong header must fail");
        assert!(error.to_string().contains("required schema"));
    }

    #[test]
    fn enforces_distinct_traffic_and_profile_date_formats() {
        assert_eq!(
            parse_compact_date(&record(&["20260701"]), 0, "day").expect("compact date"),
            NaiveDate::from_ymd_opt(2026, 7, 1).unwrap()
        );
        assert!(parse_compact_date(&record(&["2026-07-01"]), 0, "day").is_err());
        assert_eq!(
            parse_date(&record(&["2026-07-01"]), 0, "date").expect("ISO date"),
            NaiveDate::from_ymd_opt(2026, 7, 1).unwrap()
        );
        assert!(parse_date(&record(&["20260701"]), 0, "date").is_err());
    }

    #[test]
    fn rejects_negative_metrics_and_invalid_share() {
        assert!(parse_nonnegative_i64(&record(&["-1"]), 0, "count").is_err());
        assert!(parse_nonnegative_decimal(&record(&["-0.1"]), 0, "averageStay").is_err());
        assert!(parse_share(&record(&["1.0001"]), 0).is_err());
        assert!(parse_share(&record(&["not-a-number"]), 0).is_err());
    }

    #[test]
    fn rejects_duplicate_traffic_natural_key() {
        let traffic_row = "SZ02,internal,40,shop,Gate,source-1,aibee-1,20260701,10,0,H,1,2,1,0";
        let documents = traffic_documents(
            &[traffic_row, traffic_row],
            &["internal,40,Gate,source-1,aibee-1,F1,First,1,10,retail,shop"],
        );
        let error = prepare_traffic(&documents).expect_err("duplicate key must fail");
        assert!(error.to_string().contains("duplicate traffic natural key"));
    }

    #[test]
    fn rejects_nonzero_average_stay_and_non_hourly_rows() {
        let documents = traffic_documents(
            &["SZ02,internal,40,shop,Gate,source-1,aibee-1,20260701,10,0,H,1,2,1,0.5"],
            &["internal,40,Gate,source-1,aibee-1,F1,First,1,10,retail,shop"],
        );
        assert!(prepare_traffic(&documents)
            .expect_err("nonzero averageStay must fail")
            .to_string()
            .contains("averageStay must be zero"));

        let documents = traffic_documents(
            &["SZ02,internal,40,shop,Gate,source-1,aibee-1,20260701,10,0,60,1,2,1,0"],
            &["internal,40,Gate,source-1,aibee-1,F1,First,1,10,retail,shop"],
        );
        assert!(prepare_traffic(&documents)
            .expect_err("interval must be exact H")
            .to_string()
            .contains("not hourly"));

        let documents = traffic_documents(
            &["SZ02,internal,40,shop,Gate,source-1,aibee-1,20260701,10,15,H,1,2,1,0"],
            &["internal,40,Gate,source-1,aibee-1,F1,First,1,10,retail,shop"],
        );
        assert!(prepare_traffic(&documents)
            .expect_err("minute must be zero")
            .to_string()
            .contains("minute must be zero"));
    }

    #[test]
    fn point_union_uses_composite_identity_and_never_fakes_inventory_mall() {
        let documents = traffic_documents(
            &[
                "SZ02,internal,40,shop,Gate,source-1,shared-id,20260701,10,0,H,1,2,1,0",
                "SZ02,internal,60,floor,Floor,source-2,shared-id,20260701,10,0,H,3,4,2,0",
            ],
            &["internal,40,Gate,source-1,shared-id,F1,First,1,10,retail,shop"],
        );
        let prepared = prepare_traffic(&documents).expect("composite point identities should load");
        assert_eq!(prepared.points.len(), 2);
        let synthesized = prepared
            .points
            .iter()
            .find(|point| !point.inventory_present)
            .expect("type 60 should be synthesized");
        assert_eq!(synthesized.entity_type, 60);
        assert_eq!(synthesized.source_inventory_mall_id, None);
    }

    #[test]
    fn approved_profile_buckets_are_closed_whitelists() {
        assert!(allowed_profile_bucket("age", "65+"));
        assert!(allowed_profile_bucket("gender", "female"));
        assert!(allowed_profile_bucket("group_type", "friend"));
        assert!(!allowed_profile_bucket("age", "person-identity-sentinel"));
        assert_eq!(expected_profile_buckets("age").len(), 8);
        assert_eq!(expected_profile_buckets("gender").len(), 2);
        assert_eq!(expected_profile_buckets("group_type").len(), 4);
    }

    #[test]
    fn profile_requires_every_approved_bucket_and_count_consistent_share() {
        let valid = valid_profile_documents();
        let prepared = prepare_profile(&valid).expect("complete profile snapshot should validate");
        assert_eq!(prepared.summaries.len(), 1);
        assert_eq!(prepared.daily_distributions.len(), 14);
        assert_eq!(prepared.period_distributions.len(), 14);

        let mut missing_bucket = valid_profile_documents();
        let age_total = missing_bucket
            .iter_mut()
            .find(|document| document.title == "age_distribution_total.csv")
            .unwrap();
        age_total.csv.records.pop();
        let age_daily = missing_bucket
            .iter_mut()
            .find(|document| document.title == "age_distribution_daily.csv")
            .unwrap();
        age_daily.csv.records.pop();
        assert!(prepare_profile(&missing_bucket)
            .expect_err("closed bucket set must reject omissions")
            .to_string()
            .contains("complete approved bucket set"));

        let mut wrong_share = valid_profile_documents();
        let gender_total = wrong_share
            .iter_mut()
            .find(|document| document.title == "gender_distribution_total.csv")
            .unwrap();
        gender_total.csv.records[0].fields[2] = "0.6".to_string();
        gender_total.csv.records[1].fields[2] = "0.4".to_string();
        assert!(prepare_profile(&wrong_share)
            .expect_err("share must match count ratio")
            .to_string()
            .contains("share does not match"));
    }

    #[test]
    fn snapshot_ranges_and_dataset_locks_are_explicit_contracts() {
        let from = NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
        let to = NaiveDate::from_ymd_opt(2026, 7, 15).unwrap();
        assert!(validate_matching_snapshot_ranges(from, to, from, to).is_ok());
        assert!(validate_matching_snapshot_ranges(from, to, from, to.succ_opt().unwrap()).is_err());
        assert!(validate_expected_snapshot_range(from, to, from, to).is_ok());
        assert!(validate_expected_snapshot_range(from, to, from.succ_opt().unwrap(), to).is_err());
        assert!(validate_expected_snapshot_range(from, to, from, to.pred_opt().unwrap()).is_err());
        assert!(validate_nonshrinking_range(from, to, from, to.succ_opt().unwrap()).is_ok());
        assert!(validate_nonshrinking_range(from, to, from.succ_opt().unwrap(), to).is_err());

        let tenant = Uuid::new_v4();
        let left = Uuid::from_u128(1);
        let right = Uuid::from_u128(2);
        let keys = dataset_lock_keys(tenant, [right, left]);
        assert!(keys[0].ends_with(&left.to_string()));
        assert!(keys[1].ends_with(&right.to_string()));
        assert_ne!(keys[0], keys[1]);
        assert_eq!(SOURCE_DOCUMENT_LIFECYCLE, "indexed");
        assert_eq!(TRAFFIC_SOURCE_KIND, "aibee_traffic_hourly");
        assert_eq!(PROFILE_SOURCE_KIND, "aibee_profile_aggregate");
        assert_eq!(MALL_CODE, "SZ02");
    }

    #[test]
    fn manifest_and_errors_do_not_echo_source_row_values() {
        let sentinel = "PID-SENTINEL-token-query-value";
        let source = source_document("daily_summary.csv", &format!("field\n{sentinel}\n"));
        let metadata = batch_metadata(&[source]);
        let encoded = serde_json::to_string(&metadata.manifest).expect("manifest serializes");
        assert!(!encoded.contains(sentinel));
        assert!(!encoded.to_ascii_lowercase().contains("object_key"));

        let error = parse_nonnegative_i64(&record(&[sentinel]), 0, "count")
            .expect_err("invalid integer must fail")
            .to_string();
        assert!(!error.contains(sentinel));
    }

    #[test]
    fn relative_object_key_must_resolve_to_exactly_one_root() {
        let base = std::env::temp_dir().join(format!("mall-sz02-import-{}", Uuid::new_v4()));
        let left = base.join("left");
        let right = base.join("right");
        fs::create_dir_all(&left).expect("left root");
        fs::create_dir_all(&right).expect("right root");
        fs::write(left.join("same.csv"), b"a\n1\n").expect("left file");
        fs::write(right.join("same.csv"), b"a\n1\n").expect("right file");
        let roots = [
            fs::canonicalize(&left).expect("canonical left"),
            fs::canonicalize(&right).expect("canonical right"),
        ];
        let error = resolve_object_path("same.csv", &roots).expect_err("ambiguous roots fail");
        assert!(error.to_string().contains("ambiguously"));
        fs::remove_dir_all(base).expect("clean fixture");
    }

    #[test]
    fn parses_required_args_and_dry_run() {
        let traffic = Uuid::new_v4();
        let profile = Uuid::new_v4();
        let args = parse_args(
            "mall-sz02-import",
            vec![
                "--traffic-dataset-id".to_string(),
                traffic.to_string(),
                "--profile-dataset-id".to_string(),
                profile.to_string(),
                "--expected-date-from".to_string(),
                "2026-07-01".to_string(),
                "--expected-date-to".to_string(),
                "2026-07-15".to_string(),
                "--dry-run".to_string(),
            ],
        )
        .expect("args should parse");
        assert_eq!(args.traffic_dataset_id, traffic);
        assert_eq!(args.profile_dataset_id, profile);
        assert_eq!(
            args.expected_date_from,
            NaiveDate::from_ymd_opt(2026, 7, 1).unwrap()
        );
        assert_eq!(
            args.expected_date_to,
            NaiveDate::from_ymd_opt(2026, 7, 15).unwrap()
        );
        assert!(args.dry_run);

        let missing_anchor = parse_args(
            "mall-sz02-import",
            vec![
                "--traffic-dataset-id".to_string(),
                traffic.to_string(),
                "--profile-dataset-id".to_string(),
                profile.to_string(),
                "--dry-run".to_string(),
            ],
        );
        assert!(missing_anchor.is_err());
    }
}
