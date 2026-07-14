use anyhow::{anyhow, Result};
use chrono::Utc;
use domain_model::{DatasetId, TenantId};
use platform_api::cross_dataset_semantic_graph::DATASET_SEMANTIC_GRAPH_GENERATION_VERSION;
use retrieval_worker::{
    build_dataset_semantic_link_graph_from_ready_snapshots, dataset_cross_semantic_graph_access,
    dataset_semantic_link_public_pair_id, dataset_semantic_link_safe_summary,
};
use std::str::FromStr;
use storage::{NewDatasetSemanticLinkRun, PgStorage, DEFAULT_LOCAL_DATABASE_URL};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
struct BackfillArgs {
    left_dataset_id: DatasetId,
    right_dataset_id: DatasetId,
    dry_run: bool,
    confirm_real_run: bool,
    summary_only: bool,
    pretty: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    observability::install("dataset_semantic_link_backfill")?;
    let program = std::env::args()
        .next()
        .unwrap_or_else(|| "dataset-semantic-link-backfill".to_string());
    let args = parse_args(&program, std::env::args().skip(1).collect())?;
    let database_url = std::env::var("PLATFORM_DATABASE_URL")
        .unwrap_or_else(|_| DEFAULT_LOCAL_DATABASE_URL.to_string());
    let storage = PgStorage::connect(&database_url).await?;
    if !args.dry_run {
        storage.migrate().await?;
    }
    let tenant_key = std::env::var("PLATFORM_TENANT_KEY")
        .unwrap_or_else(|_| storage::DEFAULT_LOCAL_TENANT_KEY.to_string());
    let tenant_id = load_existing_tenant_id(&storage, &tenant_key).await?;
    let access =
        dataset_cross_semantic_graph_access(tenant_id, args.left_dataset_id, args.right_dataset_id);
    if !access.is_allowed() {
        anyhow::bail!(
            "semantic link backfill denied by cross-graph rollout gate: {}",
            access.safe_reason()
        );
    }

    let snapshots = storage.dataset_semantic_snapshots();
    let left_snapshot = snapshots
        .load_latest_ready(tenant_id, args.left_dataset_id)
        .await?
        .ok_or_else(|| anyhow!("left dataset has no ready semantic snapshot"))?;
    let right_snapshot = snapshots
        .load_latest_ready(tenant_id, args.right_dataset_id)
        .await?
        .ok_or_else(|| anyhow!("right dataset has no ready semantic snapshot"))?;
    let generated_at = Utc::now();
    let graph = build_dataset_semantic_link_graph_from_ready_snapshots(
        tenant_id,
        &left_snapshot,
        &right_snapshot,
    )?;
    let pair_id = dataset_semantic_link_public_pair_id(
        left_snapshot.dataset_id,
        right_snapshot.dataset_id,
        left_snapshot.id,
        right_snapshot.id,
    );
    let summary = dataset_semantic_link_safe_summary(&pair_id, &graph);

    if !args.dry_run {
        let source_fingerprint =
            platform_api::dataset_semantic_snapshot::dataset_semantic_link_source_fingerprint(
                left_snapshot.dataset_id,
                right_snapshot.dataset_id,
                left_snapshot.id,
                right_snapshot.id,
                &left_snapshot.source_fingerprint,
                &right_snapshot.source_fingerprint,
            );
        let requested_run = NewDatasetSemanticLinkRun {
            left_dataset_id: left_snapshot.dataset_id,
            right_dataset_id: right_snapshot.dataset_id,
            left_snapshot_id: left_snapshot.id,
            right_snapshot_id: right_snapshot.id,
            generation_version: DATASET_SEMANTIC_GRAPH_GENERATION_VERSION.to_string(),
            source_fingerprint,
            priority: 100,
            max_attempts: 3,
            available_at: generated_at,
        };
        let persisted = platform_api::dataset_semantic_snapshot::ensure_dataset_semantic_link_run(
            &storage,
            tenant_id,
            &requested_run,
            generated_at,
        )
        .await?;
        if persisted.status == "dead_letter" {
            storage
                .dataset_semantic_links()
                .revive_dead_letter_run(tenant_id, persisted.id, generated_at)
                .await?
                .ok_or_else(|| anyhow!("dead-letter semantic link run could not be revived"))?;
            platform_api::dataset_semantic_snapshot::ensure_dataset_semantic_link_run(
                &storage,
                tenant_id,
                &requested_run,
                generated_at,
            )
            .await?;
        }
    }

    if args.pretty {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        println!("{}", serde_json::to_string(&summary)?);
    }
    Ok(())
}

async fn load_existing_tenant_id(storage: &PgStorage, tenant_key: &str) -> Result<TenantId> {
    let id = sqlx::query_scalar::<_, Uuid>("select id from tenants where key = $1")
        .bind(tenant_key)
        .fetch_optional(storage.pool())
        .await?
        .ok_or_else(|| anyhow!("configured tenant was not found"))?;
    Ok(TenantId(id))
}

fn parse_args(program: &str, mut args: Vec<String>) -> Result<BackfillArgs> {
    let requested_dry_run = remove_flag(&mut args, "--dry-run");
    let confirm_real_run = remove_flag(&mut args, "--confirm-real-run");
    let _requested_summary_only = remove_flag(&mut args, "--summary-only");
    let pretty = remove_flag(&mut args, "--pretty");
    let left_dataset_id = take_dataset_id(&mut args, "--left-dataset-id", program)?;
    let right_dataset_id = take_dataset_id(&mut args, "--right-dataset-id", program)?;
    if left_dataset_id == right_dataset_id {
        anyhow::bail!("semantic link backfill requires two distinct dataset ids");
    }
    if !args.is_empty() || (requested_dry_run && confirm_real_run) {
        anyhow::bail!(usage(program));
    }
    Ok(BackfillArgs {
        left_dataset_id,
        right_dataset_id,
        dry_run: requested_dry_run || !confirm_real_run,
        confirm_real_run,
        summary_only: true,
        pretty,
    })
}

fn take_dataset_id(args: &mut Vec<String>, name: &str, program: &str) -> Result<DatasetId> {
    take_option(args, name)
        .ok_or_else(|| anyhow!(usage(program)))
        .and_then(|value| Uuid::from_str(&value).map(DatasetId).map_err(Into::into))
}

fn usage(program: &str) -> String {
    format!(
        "Usage: {program} --left-dataset-id <uuid> --right-dataset-id <uuid> [--dry-run] [--confirm-real-run] [--summary-only] [--pretty]"
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
    fn semantic_link_backfill_defaults_to_dry_run_and_summary_only() {
        let left = Uuid::from_u128(1);
        let right = Uuid::from_u128(2);
        let args = parse_args(
            "dataset-semantic-link-backfill",
            vec![
                "--left-dataset-id".to_string(),
                left.to_string(),
                "--right-dataset-id".to_string(),
                right.to_string(),
            ],
        )
        .expect("safe defaults");
        assert!(args.dry_run);
        assert!(args.summary_only);
        assert!(!args.confirm_real_run);
    }

    #[test]
    fn semantic_link_backfill_real_run_requires_explicit_confirmation() {
        let left = Uuid::from_u128(1);
        let right = Uuid::from_u128(2);
        let args = parse_args(
            "dataset-semantic-link-backfill",
            vec![
                "--left-dataset-id".to_string(),
                left.to_string(),
                "--right-dataset-id".to_string(),
                right.to_string(),
                "--confirm-real-run".to_string(),
            ],
        )
        .expect("confirmed run");
        assert!(!args.dry_run);
        assert!(args.confirm_real_run);

        let error = parse_args(
            "dataset-semantic-link-backfill",
            vec![
                "--left-dataset-id".to_string(),
                left.to_string(),
                "--right-dataset-id".to_string(),
                right.to_string(),
                "--dry-run".to_string(),
                "--confirm-real-run".to_string(),
            ],
        )
        .expect_err("dry and real are mutually exclusive");
        assert!(error.to_string().contains("Usage:"));
    }

    #[test]
    fn semantic_link_backfill_rejects_same_dataset_pair() {
        let dataset_id = Uuid::from_u128(1);
        let error = parse_args(
            "dataset-semantic-link-backfill",
            vec![
                "--left-dataset-id".to_string(),
                dataset_id.to_string(),
                "--right-dataset-id".to_string(),
                dataset_id.to_string(),
            ],
        )
        .expect_err("pair endpoints must be distinct");
        assert!(error.to_string().contains("distinct"));
    }
}
