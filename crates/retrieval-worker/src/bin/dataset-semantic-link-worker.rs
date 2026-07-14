use anyhow::{anyhow, Result};
use chrono::{Duration as ChronoDuration, Utc};
use domain_model::TenantId;
use platform_api::cross_dataset_semantic_graph::{
    DATASET_SEMANTIC_GRAPH_GENERATION_VERSION, DATASET_SEMANTIC_GRAPH_SCHEMA_VERSION,
};
use retrieval_worker::{
    build_dataset_semantic_link_graph_from_ready_snapshots, dataset_cross_semantic_graph_access,
    dataset_cross_semantic_graph_allowed_dataset_ids, dataset_cross_semantic_graph_worker_enabled,
    dataset_semantic_link_manifest_is_safe, dataset_semantic_understanding_access,
    semantic_link_run_inputs_are_current,
};
use serde_json::json;
use std::time::{Duration, Instant};
use storage::{
    DatasetSemanticLinkRun, NewDatasetSemanticLinkSnapshot, PgStorage, DEFAULT_LOCAL_DATABASE_URL,
};
use tokio::time::sleep;
use uuid::Uuid;

const DEFAULT_POLL_INTERVAL_MS: u64 = 3_000;
const DEFAULT_ERROR_BACKOFF_SECONDS: i64 = 30;
const DEFAULT_DEFER_SECONDS: i64 = 5 * 60;
const DEFAULT_LEASE_TIMEOUT_SECONDS: i64 = 30 * 60;
const DEFAULT_RECONCILE_INTERVAL_SECONDS: u64 = 5 * 60;
const MAX_RECONCILE_DATASETS: usize = 64;
const MAX_ERROR_BACKOFF_SECONDS: i64 = 15 * 60;
const SAFE_BUILD_FAILURE_CODE: &str = "semantic_link_build_failed";
const SAFE_INPUT_STALE_CODE: &str = "semantic_link_input_stale";

#[derive(Clone, Debug)]
struct WorkerConfig {
    poll_interval: Duration,
    error_backoff: ChronoDuration,
    defer_delay: ChronoDuration,
    lease_timeout: ChronoDuration,
    reconcile_interval: Duration,
    once: bool,
    max_runs: Option<usize>,
}

impl WorkerConfig {
    fn from_env() -> Self {
        Self {
            poll_interval: Duration::from_millis(env_u64(
                "DATASET_SEMANTIC_LINK_WORKER_POLL_INTERVAL_MS",
                DEFAULT_POLL_INTERVAL_MS,
            )),
            error_backoff: ChronoDuration::seconds(env_i64(
                "DATASET_SEMANTIC_LINK_WORKER_ERROR_BACKOFF_SECONDS",
                DEFAULT_ERROR_BACKOFF_SECONDS,
            )),
            defer_delay: ChronoDuration::seconds(env_i64(
                "DATASET_SEMANTIC_LINK_WORKER_DEFER_SECONDS",
                DEFAULT_DEFER_SECONDS,
            )),
            lease_timeout: ChronoDuration::seconds(env_i64(
                "DATASET_SEMANTIC_LINK_WORKER_LEASE_TIMEOUT_SECONDS",
                DEFAULT_LEASE_TIMEOUT_SECONDS,
            )),
            reconcile_interval: Duration::from_secs(env_u64(
                "DATASET_SEMANTIC_LINK_WORKER_RECONCILE_INTERVAL_SECONDS",
                DEFAULT_RECONCILE_INTERVAL_SECONDS,
            )),
            once: env_bool("DATASET_SEMANTIC_LINK_WORKER_ONCE", false),
            max_runs: std::env::var("DATASET_SEMANTIC_LINK_WORKER_MAX_RUNS")
                .ok()
                .and_then(|value| value.trim().parse::<usize>().ok())
                .filter(|value| *value > 0),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    observability::install("dataset_semantic_link_worker")?;
    let database_url = std::env::var("PLATFORM_DATABASE_URL")
        .unwrap_or_else(|_| DEFAULT_LOCAL_DATABASE_URL.to_string());
    let storage = PgStorage::connect_with_configured_max_connections(
        &database_url,
        "DATASET_SEMANTIC_LINK_WORKER_DATABASE_MAX_CONNECTIONS",
    )
    .await?;
    storage.migrate().await?;
    let tenant_key = std::env::var("PLATFORM_TENANT_KEY")
        .unwrap_or_else(|_| storage::DEFAULT_LOCAL_TENANT_KEY.to_string());
    let tenant_id = load_existing_tenant_id(&storage, &tenant_key).await?;
    let config = WorkerConfig::from_env();

    tracing::info!(
        tenant_id = %tenant_id,
        tenant_key = %tenant_key,
        poll_interval_ms = config.poll_interval.as_millis(),
        reconcile_interval_seconds = config.reconcile_interval.as_secs(),
        lease_timeout_seconds = config.lease_timeout.num_seconds(),
        once = config.once,
        max_runs = config.max_runs.unwrap_or(0),
        concurrency = 1,
        "dataset semantic link worker polling started"
    );
    run_worker_loop(&storage, tenant_id, &config).await
}

async fn load_existing_tenant_id(storage: &PgStorage, tenant_key: &str) -> Result<TenantId> {
    let id = sqlx::query_scalar::<_, Uuid>("select id from tenants where key = $1")
        .bind(tenant_key)
        .fetch_optional(storage.pool())
        .await?
        .ok_or_else(|| anyhow!("configured tenant was not found"))?;
    Ok(TenantId(id))
}

async fn run_worker_loop(
    storage: &PgStorage,
    tenant_id: TenantId,
    config: &WorkerConfig,
) -> Result<()> {
    let mut processed_count = 0usize;
    let mut last_reconcile = None::<Instant>;
    loop {
        if !dataset_cross_semantic_graph_worker_enabled(tenant_id) {
            if config.once {
                return Ok(());
            }
            sleep(config.poll_interval).await;
            continue;
        }

        let allowed_dataset_ids = dataset_cross_semantic_graph_allowed_dataset_ids();
        if allowed_dataset_ids.is_empty() {
            if config.once {
                return Ok(());
            }
            sleep(config.poll_interval).await;
            continue;
        }

        if last_reconcile.is_none_or(|last| last.elapsed() >= config.reconcile_interval) {
            let now = Utc::now();
            match storage
                .dataset_semantic_links()
                .recover_stale_runs(
                    tenant_id,
                    now - config.lease_timeout,
                    now,
                    &allowed_dataset_ids,
                )
                .await
            {
                Ok(recovered) if !recovered.is_empty() => tracing::warn!(
                    recovered_run_count = recovered.len(),
                    "recovered expired semantic link worker leases"
                ),
                Ok(_) => {}
                Err(_) => {
                    tracing::error!(
                        failure_code = "lease_recovery_failed",
                        "semantic link lease recovery failed"
                    )
                }
            }
            if let Err(_) =
                reconcile_semantic_link_inputs(storage, tenant_id, &allowed_dataset_ids, now).await
            {
                tracing::error!(
                    failure_code = "input_reconciliation_failed",
                    "semantic link input reconciliation failed"
                );
            }
            last_reconcile = Some(Instant::now());
        }

        let claimed = storage
            .dataset_semantic_links()
            .claim_ready_run_for_datasets(tenant_id, Utc::now(), &allowed_dataset_ids)
            .await?;
        if let Some(run) = claimed {
            processed_count += 1;
            let access = dataset_cross_semantic_graph_access(
                tenant_id,
                run.left_dataset_id,
                run.right_dataset_id,
            );
            if !access.is_allowed() {
                let now = Utc::now();
                storage
                    .dataset_semantic_links()
                    .defer_run_without_attempt(tenant_id, run.id, now + config.defer_delay, now)
                    .await?;
                tracing::warn!(run_id = %run.id, reason = access.safe_reason(), "semantic link run deferred by rollout gate");
            } else if let Err(_) = process_claimed_run(storage, tenant_id, &run).await {
                tracing::error!(run_id = %run.id, failure_code = SAFE_BUILD_FAILURE_CODE, "dataset semantic link run failed");
                let now = Utc::now();
                storage
                    .dataset_semantic_links()
                    .retry_or_dead_letter(
                        tenant_id,
                        &run,
                        SAFE_BUILD_FAILURE_CODE,
                        now + retry_backoff(config.error_backoff, run.attempt_count),
                        now,
                    )
                    .await?;
            }
            if config.once
                || config
                    .max_runs
                    .is_some_and(|limit| processed_count >= limit)
            {
                return Ok(());
            }
            continue;
        }

        if config.once
            || config
                .max_runs
                .is_some_and(|limit| processed_count >= limit)
        {
            return Ok(());
        }
        sleep(config.poll_interval).await;
    }
}

async fn reconcile_semantic_link_inputs(
    storage: &PgStorage,
    tenant_id: TenantId,
    allowed_dataset_ids: &[domain_model::DatasetId],
    as_of: chrono::DateTime<Utc>,
) -> Result<()> {
    let allowed = allowed_dataset_ids
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let snapshots = storage
        .dataset_semantic_snapshots()
        .list_latest_ready_by_tenant(tenant_id, 10_000)
        .await?;
    for snapshot in snapshots
        .into_iter()
        .filter(|snapshot| allowed.contains(&snapshot.dataset_id))
        .filter(|snapshot| {
            dataset_semantic_understanding_access(tenant_id, snapshot.dataset_id).is_allowed()
        })
        .take(MAX_RECONCILE_DATASETS)
    {
        let preview = match platform_api::dataset_semantic_snapshot::preview_dataset_semantic_snapshot_from_storage(
            storage,
            tenant_id,
            snapshot.dataset_id,
            as_of,
            10_000,
        )
        .await
        {
            Ok(preview) => preview,
            Err(_) => {
                tracing::warn!(dataset_id = %snapshot.dataset_id, failure_code = "snapshot_preview_failed", "semantic snapshot reconciliation preview failed");
                continue;
            }
        };
        if preview.source_fingerprint != snapshot.source_fingerprint {
            storage
                .dataset_semantic_links()
                .mark_ready_links_stale_for_dataset(tenant_id, snapshot.dataset_id, as_of)
                .await?;
            if let Err(_) = platform_api::dataset_semantic_snapshot::rebuild_dataset_semantic_snapshot_from_storage(
                storage,
                tenant_id,
                snapshot.dataset_id,
                as_of,
            )
            .await
            {
                tracing::warn!(dataset_id = %snapshot.dataset_id, failure_code = "snapshot_rebuild_failed", "semantic snapshot reconciliation rebuild failed");
            }
        } else if let Err(_) = platform_api::dataset_semantic_snapshot::enqueue_dataset_semantic_link_runs_for_ready_snapshot(
            storage,
            &snapshot,
            as_of,
        )
        .await
        {
            tracing::warn!(dataset_id = %snapshot.dataset_id, failure_code = "link_enqueue_failed", "semantic link enqueue reconciliation failed");
        }
    }
    Ok(())
}

async fn process_claimed_run(
    storage: &PgStorage,
    tenant_id: TenantId,
    run: &DatasetSemanticLinkRun,
) -> Result<()> {
    if run.tenant_id != tenant_id {
        return Err(anyhow!("semantic link run tenant mismatch"));
    }
    let access =
        dataset_cross_semantic_graph_access(tenant_id, run.left_dataset_id, run.right_dataset_id);
    if !access.is_allowed() {
        return Err(anyhow!(
            "semantic link rollout gate denied: {}",
            access.safe_reason()
        ));
    }

    let snapshots = storage.dataset_semantic_snapshots();
    let left_snapshot = snapshots
        .load_ready_by_id(tenant_id, run.left_dataset_id, run.left_snapshot_id)
        .await?
        .ok_or_else(|| anyhow!("left ready semantic snapshot was not found"))?;
    let right_snapshot = snapshots
        .load_ready_by_id(tenant_id, run.right_dataset_id, run.right_snapshot_id)
        .await?
        .ok_or_else(|| anyhow!("right ready semantic snapshot was not found"))?;
    let left_latest = snapshots
        .load_latest_ready(tenant_id, run.left_dataset_id)
        .await?
        .ok_or_else(|| anyhow!("left latest ready semantic snapshot was not found"))?;
    let right_latest = snapshots
        .load_latest_ready(tenant_id, run.right_dataset_id)
        .await?
        .ok_or_else(|| anyhow!("right latest ready semantic snapshot was not found"))?;
    if !semantic_link_run_inputs_are_current(run, &left_latest, &right_latest) {
        storage
            .dataset_semantic_links()
            .mark_obsolete_run_succeeded(tenant_id, run, SAFE_INPUT_STALE_CODE, Utc::now())
            .await?
            .ok_or_else(|| anyhow!("obsolete semantic link run was not in running state"))?;
        tracing::info!(run_id = %run.id, "obsolete semantic link run skipped");
        return Ok(());
    }

    let expected_fingerprint =
        platform_api::dataset_semantic_snapshot::dataset_semantic_link_source_fingerprint(
            run.left_dataset_id,
            run.right_dataset_id,
            run.left_snapshot_id,
            run.right_snapshot_id,
            &left_snapshot.source_fingerprint,
            &right_snapshot.source_fingerprint,
        );
    if run.generation_version != DATASET_SEMANTIC_GRAPH_GENERATION_VERSION
        || run.source_fingerprint != expected_fingerprint
    {
        return Err(anyhow!("semantic link run identity validation failed"));
    }

    let links = storage.dataset_semantic_links();
    let build = links
        .try_begin_build(
            tenant_id,
            &NewDatasetSemanticLinkSnapshot {
                left_dataset_id: run.left_dataset_id,
                right_dataset_id: run.right_dataset_id,
                left_snapshot_id: run.left_snapshot_id,
                right_snapshot_id: run.right_snapshot_id,
                schema_version: DATASET_SEMANTIC_GRAPH_SCHEMA_VERSION.to_string(),
                generation_version: DATASET_SEMANTIC_GRAPH_GENERATION_VERSION.to_string(),
                source_fingerprint: expected_fingerprint,
                manifest: json!({}),
            },
            Utc::now(),
        )
        .await?;
    let build = match build {
        Some(build) => build,
        None => {
            let existing = links
                .load_by_inputs(
                    tenant_id,
                    run.left_dataset_id,
                    run.right_dataset_id,
                    run.left_snapshot_id,
                    run.right_snapshot_id,
                    DATASET_SEMANTIC_GRAPH_GENERATION_VERSION,
                )
                .await?
                .ok_or_else(|| anyhow!("semantic link build record was not found"))?;
            let same_inputs = existing.left_snapshot_id == run.left_snapshot_id
                && existing.right_snapshot_id == run.right_snapshot_id
                && existing.generation_version == DATASET_SEMANTIC_GRAPH_GENERATION_VERSION
                && existing.source_fingerprint == run.source_fingerprint;
            if existing.status == "ready" && same_inputs {
                links
                    .mark_run_succeeded(tenant_id, run.id, Utc::now())
                    .await?;
                return Ok(());
            }
            if existing.status == "building" && same_inputs {
                existing
            } else {
                return Err(anyhow!("semantic link build is already in progress"));
            }
        }
    };

    let build_result: Result<()> = async {
        let graph = build_dataset_semantic_link_graph_from_ready_snapshots(
            tenant_id,
            &left_snapshot,
            &right_snapshot,
        )?;
        let manifest = serde_json::to_value(&graph)?;
        if !dataset_semantic_link_manifest_is_safe(&manifest) {
            return Err(anyhow!("semantic link manifest failed safe-output audit"));
        }
        let node_count = i32::try_from(graph.nodes.len())?;
        let edge_count = i32::try_from(graph.edges.len())?;
        let left_latest = snapshots
            .load_latest_ready(tenant_id, run.left_dataset_id)
            .await?
            .ok_or_else(|| anyhow!("left latest ready semantic snapshot was not found"))?;
        let right_latest = snapshots
            .load_latest_ready(tenant_id, run.right_dataset_id)
            .await?
            .ok_or_else(|| anyhow!("right latest ready semantic snapshot was not found"))?;
        if !semantic_link_run_inputs_are_current(run, &left_latest, &right_latest)
            || left_latest.source_fingerprint != left_snapshot.source_fingerprint
            || right_latest.source_fingerprint != right_snapshot.source_fingerprint
        {
            links
                .mark_failed(
                    tenant_id,
                    run.left_dataset_id,
                    run.right_dataset_id,
                    build.id,
                    SAFE_INPUT_STALE_CODE,
                    Utc::now(),
                )
                .await?
                .ok_or_else(|| anyhow!("stale semantic link build was not claimable"))?;
            return Ok(());
        }
        links
            .mark_ready(
                tenant_id,
                run.left_dataset_id,
                run.right_dataset_id,
                build.id,
                &manifest,
                node_count,
                edge_count,
                Utc::now(),
            )
            .await?
            .ok_or_else(|| anyhow!("semantic link build was not in building state"))?;
        Ok(())
    }
    .await;

    if let Err(error) = build_result {
        if let Err(_) = links
            .mark_failed(
                tenant_id,
                run.left_dataset_id,
                run.right_dataset_id,
                build.id,
                SAFE_BUILD_FAILURE_CODE,
                Utc::now(),
            )
            .await
        {
            tracing::error!(run_id = %run.id, failure_code = "mark_snapshot_failed_failed", "failed to mark semantic link snapshot failed");
        }
        return Err(error);
    }

    links
        .mark_run_succeeded(tenant_id, run.id, Utc::now())
        .await?
        .ok_or_else(|| anyhow!("semantic link run was not in running state"))?;
    tracing::info!(run_id = %run.id, "dataset semantic link run completed");
    Ok(())
}

fn retry_backoff(base: ChronoDuration, attempt_count: i32) -> ChronoDuration {
    let base_seconds = base.num_seconds().clamp(1, MAX_ERROR_BACKOFF_SECONDS);
    let exponent = attempt_count.saturating_sub(1).clamp(0, 8) as u32;
    ChronoDuration::seconds(
        base_seconds
            .saturating_mul(2_i64.saturating_pow(exponent))
            .min(MAX_ERROR_BACKOFF_SECONDS),
    )
}

fn env_bool(key: &str, default: bool) -> bool {
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

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn env_i64(key: &str, default: i64) -> i64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_link_worker_retry_backoff_is_bounded() {
        assert_eq!(
            retry_backoff(ChronoDuration::seconds(30), 1).num_seconds(),
            30
        );
        assert_eq!(
            retry_backoff(ChronoDuration::seconds(30), 2).num_seconds(),
            60
        );
        assert_eq!(
            retry_backoff(ChronoDuration::seconds(30), 100).num_seconds(),
            MAX_ERROR_BACKOFF_SECONDS
        );
    }
}
