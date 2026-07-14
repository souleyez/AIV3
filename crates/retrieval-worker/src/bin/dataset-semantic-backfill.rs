use anyhow::{anyhow, Result};
use chrono::Utc;
use domain_model::{DatasetId, TenantId};
use platform_api::semantic_understanding::SemanticEvidenceClass;
use retrieval_worker::dataset_semantic_understanding_access;
use serde_json::json;
use std::str::FromStr;
use storage::{PgStorage, DEFAULT_LOCAL_DATABASE_URL};
use uuid::Uuid;

const SOURCE_DOCUMENT_LIMIT: usize = 10_000;

#[derive(Clone, Debug, Eq, PartialEq)]
struct BackfillArgs {
    dataset_id: DatasetId,
    limit: usize,
    source_limit: usize,
    explicit_limit: bool,
    dry_run: bool,
    summary_only: bool,
    confirm_real_run: bool,
    pretty: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    observability::install("dataset_semantic_backfill")?;
    let program = std::env::args()
        .next()
        .unwrap_or_else(|| "dataset-semantic-backfill".to_string());
    let args = parse_args(&program, std::env::args().skip(1).collect())?;
    let database_url = std::env::var("PLATFORM_DATABASE_URL")
        .unwrap_or_else(|_| DEFAULT_LOCAL_DATABASE_URL.to_string());
    let storage = PgStorage::connect(&database_url).await?;
    if !args.dry_run {
        storage.migrate().await?;
    }
    let tenant_key = std::env::var("PLATFORM_TENANT_KEY")
        .unwrap_or_else(|_| storage::DEFAULT_LOCAL_TENANT_KEY.to_string());
    let tenant_name = std::env::var("PLATFORM_TENANT_NAME")
        .unwrap_or_else(|_| storage::DEFAULT_LOCAL_TENANT_NAME.to_string());
    let tenant_id = if args.dry_run {
        TenantId(
            sqlx::query_scalar::<_, Uuid>("select id from tenants where key = $1")
                .bind(&tenant_key)
                .fetch_optional(storage.pool())
                .await?
                .ok_or_else(|| anyhow!("tenant key {tenant_key} not found"))?,
        )
    } else {
        storage.ensure_tenant(&tenant_key, &tenant_name).await?.id
    };
    if !args.dry_run {
        let access = dataset_semantic_understanding_access(tenant_id, args.dataset_id);
        if !access.is_allowed() {
            anyhow::bail!(
                "real semantic backfill denied by semantic rollout gate: {}",
                access.safe_reason()
            );
        }
    }

    let generated_at = Utc::now();
    let output = if args.dry_run {
        let preview = platform_api::dataset_semantic_snapshot::preview_dataset_semantic_snapshot_from_storage(
            &storage,
            tenant_id,
            args.dataset_id,
            generated_at,
            args.source_limit,
        )
        .await?;
        let direct_document_count = sqlx::query_scalar::<_, i64>(
            "select count(*) from documents where tenant_id = $1 and dataset_id = $2 and created_at <= $3",
        )
        .bind(tenant_id.0)
        .bind(args.dataset_id.0)
        .bind(generated_at)
        .fetch_one(storage.pool())
        .await?;
        let membership_document_count = sqlx::query_scalar::<_, i64>(
            r#"
            select count(distinct membership.document_id)
            from dataset_document_memberships membership
            join documents document
              on document.id = membership.document_id
             and document.tenant_id = membership.tenant_id
            where membership.tenant_id = $1
              and membership.dataset_id = $2
              and membership.created_at <= $3
              and document.created_at <= $3
              and (membership.expires_at is null or membership.expires_at > $3)
            "#,
        )
        .bind(tenant_id.0)
        .bind(args.dataset_id.0)
        .bind(generated_at)
        .fetch_one(storage.pool())
        .await?;
        let relation_counts = preview.snapshot.relations.iter().fold(
            (0usize, 0usize, 0usize),
            |(confirmed, observed, inferred), relation| match relation.evidence_class {
                SemanticEvidenceClass::Confirmed => (confirmed + 1, observed, inferred),
                SemanticEvidenceClass::Observed => (confirmed, observed + 1, inferred),
                SemanticEvidenceClass::Inferred => (confirmed, observed, inferred + 1),
            },
        );
        let database_source_object_count = preview
            .snapshot
            .objects
            .iter()
            .filter(|object| object.kind == "database_table")
            .count();
        let manifest_bytes = serde_json::to_vec(&preview.snapshot)?.len();
        let quality = platform_api::dataset_semantic_snapshot::audit_semantic_snapshot_quality(
            &preview.snapshot,
        );
        json!({
            "dry_run": true,
            "summary_only": true,
            "dataset_id": args.dataset_id,
            "limit": args.limit,
            "source_limit": args.source_limit,
            "direct_document_count": direct_document_count,
            "membership_document_count": membership_document_count,
            "deduplicated_document_count": preview.source_document_count,
            "asset_count": preview.source_asset_count,
            "source_object_count": preview.snapshot.objects.len(),
            "database_source_object_count": database_source_object_count,
            "auxiliary_source_object_count": preview.snapshot.objects.len().saturating_sub(database_source_object_count),
            "field_count": preview.snapshot.fields.len(),
            "relation_count": preview.snapshot.relations.len(),
            "confirmed_relation_count": relation_counts.0,
            "observed_relation_count": relation_counts.1,
            "inferred_relation_count": relation_counts.2,
            "confirmed_fact_count": preview.snapshot.coverage.confirmed_fact_count,
            "unresolved_field_count": preview.snapshot.coverage.unresolved_field_count,
            "headline": preview.snapshot.summary.headline,
            "schema_version": preview.snapshot.schema_version,
            "generation_version": preview.snapshot.generation_version,
            "manifest_bytes": manifest_bytes,
            "business_label_count": quality.business_label_count,
            "chinese_business_label_count": quality.chinese_business_label_count,
            "chinese_label_ratio": quality.chinese_label_ratio,
            "raw_row_hit_count": quality.raw_row_hit_count,
            "sql_or_mime_hit_count": quality.sql_or_mime_hit_count,
            "strategy_hit_count": quality.strategy_hit_count,
            "path_or_connection_hit_count": quality.path_or_connection_hit_count,
            "technical_filename_hit_count": quality.technical_filename_hit_count,
            "numeric_identifier_hit_count": quality.numeric_identifier_hit_count,
            "quality_gate_passed": quality.quality_gate_passed,
            "source_fingerprint": preview.source_fingerprint,
            "status": "planned",
            "write_count": 0,
        })
    } else {
        let outcome = platform_api::dataset_semantic_snapshot::rebuild_dataset_semantic_snapshot_from_storage(
            &storage,
            tenant_id,
            args.dataset_id,
            generated_at,
        )
        .await?;
        let quality = outcome
            .snapshot
            .as_ref()
            .map(platform_api::dataset_semantic_snapshot::audit_semantic_snapshot_quality)
            .unwrap_or_else(empty_quality_report);
        let current_attempt_quality_passed = matches!(outcome.status.as_str(), "ready" | "skipped")
            && outcome.failure_code.is_none()
            && quality.quality_gate_passed;
        json!({
            "dry_run": false,
            "summary_only": args.summary_only,
            "dataset_id": args.dataset_id,
            "limit": args.limit,
            "source_limit": args.source_limit,
            "source_fingerprint": outcome.source_fingerprint,
            "status": outcome.status,
            "failure_code": outcome.failure_code,
            "object_count": outcome.snapshot.as_ref().map(|item| item.objects.len()).unwrap_or(0),
            "field_count": outcome.snapshot.as_ref().map(|item| item.fields.len()).unwrap_or(0),
            "relation_count": outcome.snapshot.as_ref().map(|item| item.relations.len()).unwrap_or(0),
            "manifest_bytes": quality.manifest_bytes,
            "business_label_count": quality.business_label_count,
            "chinese_business_label_count": quality.chinese_business_label_count,
            "chinese_label_ratio": quality.chinese_label_ratio,
            "raw_row_hit_count": quality.raw_row_hit_count,
            "sql_or_mime_hit_count": quality.sql_or_mime_hit_count,
            "strategy_hit_count": quality.strategy_hit_count,
            "path_or_connection_hit_count": quality.path_or_connection_hit_count,
            "technical_filename_hit_count": quality.technical_filename_hit_count,
            "numeric_identifier_hit_count": quality.numeric_identifier_hit_count,
            "quality_gate_passed": current_attempt_quality_passed,
        })
    };
    if args.pretty {
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("{}", serde_json::to_string(&output)?);
    }
    Ok(())
}

fn empty_quality_report() -> platform_api::dataset_semantic_snapshot::SemanticSnapshotQualityReport
{
    platform_api::dataset_semantic_snapshot::SemanticSnapshotQualityReport {
        business_label_count: 0,
        chinese_business_label_count: 0,
        chinese_label_ratio: 0.0,
        raw_row_hit_count: 0,
        sql_or_mime_hit_count: 0,
        strategy_hit_count: 0,
        path_or_connection_hit_count: 0,
        technical_filename_hit_count: 0,
        numeric_identifier_hit_count: 0,
        manifest_bytes: 0,
        quality_gate_passed: false,
    }
}

fn parse_args(program: &str, mut args: Vec<String>) -> Result<BackfillArgs> {
    let pretty = remove_flag(&mut args, "--pretty");
    let requested_dry_run = remove_flag(&mut args, "--dry-run");
    let requested_summary_only = remove_flag(&mut args, "--summary-only");
    let confirm_real_run = remove_flag(&mut args, "--confirm-real-run");
    let dataset_id = take_option(&mut args, "--dataset-id")
        .ok_or_else(|| anyhow!(usage(program)))
        .and_then(|value| Uuid::from_str(&value).map(DatasetId).map_err(Into::into))?;
    let raw_limit = take_option(&mut args, "--limit");
    let explicit_limit = raw_limit.is_some();
    let limit = raw_limit
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(1);
    if limit != 1 {
        anyhow::bail!(
            "single-dataset semantic backfill requires --limit 1; source documents use the fixed bounded limit {SOURCE_DOCUMENT_LIMIT}"
        );
    }
    if !args.is_empty() || (requested_dry_run && confirm_real_run) {
        anyhow::bail!(usage(program));
    }
    let dry_run = requested_dry_run || !confirm_real_run;
    if !dry_run && !explicit_limit {
        anyhow::bail!("real semantic backfill requires an explicit --limit");
    }
    Ok(BackfillArgs {
        dataset_id,
        limit,
        source_limit: SOURCE_DOCUMENT_LIMIT,
        explicit_limit,
        dry_run,
        summary_only: requested_summary_only || dry_run,
        confirm_real_run,
        pretty,
    })
}

fn usage(program: &str) -> String {
    format!(
        "Usage: {program} --dataset-id <uuid> [--limit 1] [--dry-run] [--summary-only] [--confirm-real-run] [--pretty]"
    )
}

fn remove_flag(args: &mut Vec<String>, flag: &str) -> bool {
    if let Some(index) = args.iter().position(|value| value == flag) {
        args.remove(index);
        true
    } else {
        false
    }
}

fn take_option(args: &mut Vec<String>, name: &str) -> Option<String> {
    let index = args.iter().position(|value| value == name)?;
    if index + 1 >= args.len() {
        return Some(String::new());
    }
    let value = args.remove(index + 1);
    args.remove(index);
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_profile_backfill_defaults_to_dry_run_and_summary_only() {
        let dataset_id = Uuid::from_u128(1);
        let args = parse_args(
            "dataset-semantic-backfill",
            vec!["--dataset-id".to_string(), dataset_id.to_string()],
        )
        .expect("safe defaults");
        assert!(args.dry_run);
        assert!(args.summary_only);
        assert!(!args.confirm_real_run);
        assert_eq!(args.limit, 1);
        assert_eq!(args.source_limit, 10_000);
    }

    #[test]
    fn semantic_profile_real_backfill_requires_confirmation_and_explicit_limit() {
        let dataset_id = Uuid::from_u128(1);
        let error = parse_args(
            "dataset-semantic-backfill",
            vec![
                "--dataset-id".to_string(),
                dataset_id.to_string(),
                "--confirm-real-run".to_string(),
            ],
        )
        .expect_err("explicit limit required");
        assert!(error.to_string().contains("explicit --limit"));

        let args = parse_args(
            "dataset-semantic-backfill",
            vec![
                "--dataset-id".to_string(),
                dataset_id.to_string(),
                "--limit".to_string(),
                "1".to_string(),
                "--confirm-real-run".to_string(),
                "--summary-only".to_string(),
            ],
        )
        .expect("confirmed bounded run");
        assert!(!args.dry_run);
        assert_eq!(args.limit, 1);
        assert_eq!(args.source_limit, 10_000);
    }

    #[test]
    fn semantic_backfill_dry_and_real_use_the_same_source_bound() {
        let dataset_id = Uuid::from_u128(1);
        let dry = parse_args(
            "dataset-semantic-backfill",
            vec![
                "--dataset-id".to_string(),
                dataset_id.to_string(),
                "--limit".to_string(),
                "1".to_string(),
                "--dry-run".to_string(),
            ],
        )
        .expect("dry run");
        let real = parse_args(
            "dataset-semantic-backfill",
            vec![
                "--dataset-id".to_string(),
                dataset_id.to_string(),
                "--limit".to_string(),
                "1".to_string(),
                "--confirm-real-run".to_string(),
            ],
        )
        .expect("real run");

        assert_eq!(dry.source_limit, real.source_limit);
        assert_eq!(dry.source_limit, 10_000);
    }

    #[test]
    fn semantic_backfill_rejects_multi_dataset_limit_for_single_dataset_command() {
        let dataset_id = Uuid::from_u128(1);
        let error = parse_args(
            "dataset-semantic-backfill",
            vec![
                "--dataset-id".to_string(),
                dataset_id.to_string(),
                "--limit".to_string(),
                "2".to_string(),
                "--dry-run".to_string(),
            ],
        )
        .expect_err("single dataset command must require limit one");

        assert!(error.to_string().contains("--limit 1"));
    }
}
