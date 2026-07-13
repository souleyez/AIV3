use anyhow::{anyhow, Result};
use chrono::Utc;
use domain_model::{DatasetId, TenantId};
use platform_api::dataset_semantic_source_support::{
    source_fingerprint, SemanticAssetSourceVersion, SemanticDocumentSourceVersion,
    SemanticSourceFingerprintInput,
};
use serde_json::json;
use std::str::FromStr;
use storage::{PgStorage, DEFAULT_LOCAL_DATABASE_URL};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
struct BackfillArgs {
    dataset_id: DatasetId,
    limit: usize,
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
    if !args.dry_run && !env_flag("DATASET_SEMANTIC_UNDERSTANDING_ENABLED", false) {
        anyhow::bail!(
            "real semantic backfill requires DATASET_SEMANTIC_UNDERSTANDING_ENABLED=true"
        );
    }

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

    let output = if args.dry_run {
        let documents = storage
            .documents()
            .list_by_dataset_scope_bounded(tenant_id, args.dataset_id, args.limit)
            .await?;
        let assets = storage
            .asset_items()
            .list_by_dataset_scope(tenant_id, args.dataset_id, args.limit)
            .await?;
        let fingerprint = source_fingerprint(&SemanticSourceFingerprintInput {
            documents: documents
                .iter()
                .map(|document| SemanticDocumentSourceVersion {
                    document_id: document.id,
                    updated_at: document.updated_at,
                    parse_versions: Vec::new(),
                    fact_snapshot_versions: Vec::new(),
                })
                .collect(),
            assets: assets
                .iter()
                .map(|asset| SemanticAssetSourceVersion {
                    asset_id: asset.id,
                    updated_at: asset.updated_at,
                    profile_versions: Vec::new(),
                })
                .collect(),
            dataset_fact_snapshot_version: None,
        });
        json!({
            "dry_run": true,
            "summary_only": true,
            "dataset_id": args.dataset_id,
            "limit": args.limit,
            "document_count": documents.len(),
            "asset_count": assets.len(),
            "source_fingerprint": fingerprint,
            "status": "planned",
            "write_count": 0,
        })
    } else {
        let outcome = platform_api::dataset_semantic_snapshot::rebuild_dataset_semantic_snapshot_from_storage(
            &storage,
            tenant_id,
            args.dataset_id,
            Utc::now(),
        )
        .await?;
        json!({
            "dry_run": false,
            "summary_only": args.summary_only,
            "dataset_id": args.dataset_id,
            "limit": args.limit,
            "source_fingerprint": outcome.source_fingerprint,
            "status": outcome.status,
            "failure_code": outcome.failure_code,
            "object_count": outcome.snapshot.as_ref().map(|item| item.objects.len()).unwrap_or(0),
            "field_count": outcome.snapshot.as_ref().map(|item| item.fields.len()).unwrap_or(0),
            "relation_count": outcome.snapshot.as_ref().map(|item| item.relations.len()).unwrap_or(0),
        })
    };
    if args.pretty {
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("{}", serde_json::to_string(&output)?);
    }
    Ok(())
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
        .unwrap_or(10)
        .clamp(1, 10_000);
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
        explicit_limit,
        dry_run,
        summary_only: requested_summary_only || dry_run,
        confirm_real_run,
        pretty,
    })
}

fn usage(program: &str) -> String {
    format!(
        "Usage: {program} --dataset-id <uuid> [--limit <n>] [--dry-run] [--summary-only] [--confirm-real-run] [--pretty]"
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

fn env_flag(key: &str, default: bool) -> bool {
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
        assert_eq!(args.limit, 10);
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
                "5".to_string(),
                "--confirm-real-run".to_string(),
                "--summary-only".to_string(),
            ],
        )
        .expect("confirmed bounded run");
        assert!(!args.dry_run);
        assert_eq!(args.limit, 5);
    }

    #[test]
    fn semantic_profile_missing_feature_flag_is_false() {
        assert!(!env_flag("DATAMAX_TEST_MISSING_SEMANTIC_FLAG", false));
    }
}
