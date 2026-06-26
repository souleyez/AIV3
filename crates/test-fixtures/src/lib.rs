use anyhow::{bail, Result};
use chrono::Utc;
use domain_model::{
    Dataset, DatasetId, DatasetLifecycle, DatasetVisibility, ReportModule, ReportModuleId,
    ReportModuleKind, ReportPlan, ReportPlanId, ReportPlanStatus, SecretBindingId, TenantId,
};
use serde_json::json;
use sqlx::AssertSqlSafe;
use std::collections::BTreeMap;
use std::sync::OnceLock;
use std::time::Duration;
use storage::{PgStorage, DEFAULT_LOCAL_DATABASE_URL, TABLES};
use tokio::sync::Mutex;

const ALLOW_SHARED_DATABASE_ENV: &str = "AIV3_ALLOW_TEST_FIXTURE_SHARED_DATABASE";

pub fn sample_dataset() -> Dataset {
    Dataset {
        id: DatasetId::new(),
        tenant_id: TenantId::new(),
        owner_user_id: None,
        key: "sample-dataset".to_string(),
        title: "Sample Dataset".to_string(),
        description: Some("Fixture dataset used for early workflow testing.".to_string()),
        lifecycle: DatasetLifecycle::Active,
        visibility: DatasetVisibility::Private,
        default_secret_binding_ids: vec![SecretBindingId::new()],
        metadata: BTreeMap::from([("source".to_string(), json!("fixture"))]),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

pub fn sample_report_plan() -> ReportPlan {
    let plan_id = ReportPlanId::new();

    ReportPlan {
        id: plan_id,
        tenant_id: TenantId::new(),
        dataset_id: DatasetId::new(),
        owner_user_id: None,
        title: "Executive Snapshot".to_string(),
        objective: "Validate report planning, binding, and publishing skeletons.".to_string(),
        status: ReportPlanStatus::Draft,
        theme_key: "default-light".to_string(),
        current_ast_version_id: None,
        modules: vec![ReportModule {
            id: ReportModuleId::new(),
            plan_id,
            sort_order: 1,
            kind: ReportModuleKind::Hero,
            title: "Hero Summary".to_string(),
            expected_copy: Some("Topline outcomes and dataset context.".to_string()),
            chart_intent: Some("trend_line".to_string()),
            data_binding_slot: Some("summary.hero".to_string()),
        }],
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

#[derive(Clone)]
enum SharedLocalPostgresState {
    Unavailable(String),
}

fn shared_local_postgres_state() -> &'static Mutex<Option<SharedLocalPostgresState>> {
    static STATE: OnceLock<Mutex<Option<SharedLocalPostgresState>>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(None))
}

pub fn shared_local_postgres_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

pub async fn shared_local_postgres_storage() -> std::result::Result<PgStorage, String> {
    let state = shared_local_postgres_state();
    {
        let guard = state.lock().await;
        if let Some(SharedLocalPostgresState::Unavailable(reason)) = guard.as_ref() {
            return Err(reason.clone());
        }
    }

    match local_postgres_storage().await {
        Ok(storage) => Ok(storage),
        Err(reason) => {
            let mut guard = state.lock().await;
            *guard = Some(SharedLocalPostgresState::Unavailable(reason.clone()));
            Err(reason)
        }
    }
}

pub async fn local_postgres_storage() -> std::result::Result<PgStorage, String> {
    let database_url = std::env::var("PLATFORM_DATABASE_URL")
        .unwrap_or_else(|_| DEFAULT_LOCAL_DATABASE_URL.into());
    ensure_safe_local_postgres_fixture_url(&database_url)?;
    let storage = PgStorage::connect_with_settings(&database_url, 4, Duration::from_secs(10))
        .await
        .map_err(|error| {
            format!("failed to connect local postgres fixture at {database_url}: {error}")
        })?;
    storage.migrate().await.map_err(|error| {
        format!("failed to migrate local postgres fixture at {database_url}: {error}")
    })?;
    Ok(storage)
}

pub async fn reset_local_postgres_storage(storage: &PgStorage) -> Result<()> {
    ensure_safe_local_postgres_fixture_database(storage).await?;
    let truncate = format!("truncate table {} cascade", TABLES.join(", "));
    sqlx::query(AssertSqlSafe(truncate.as_str()))
        .execute(storage.pool())
        .await?;
    storage.migrate().await?;
    Ok(())
}

fn ensure_safe_local_postgres_fixture_url(database_url: &str) -> std::result::Result<(), String> {
    if shared_database_fixture_override_enabled() {
        return Ok(());
    }
    let database_name =
        database_name_from_url(database_url).unwrap_or_else(|| "<unknown>".to_string());
    if database_name_looks_disposable(&database_name) {
        return Ok(());
    }
    Err(format!(
        "refusing to use non-test postgres fixture database '{database_name}'. \
         Point PLATFORM_DATABASE_URL at a disposable test database, or set \
         {ALLOW_SHARED_DATABASE_ENV}=1 only after confirming this is intentional."
    ))
}

async fn ensure_safe_local_postgres_fixture_database(storage: &PgStorage) -> Result<()> {
    if shared_database_fixture_override_enabled() {
        return Ok(());
    }
    let database_name: String = sqlx::query_scalar("select current_database()")
        .fetch_one(storage.pool())
        .await?;
    if database_name_looks_disposable(&database_name) {
        return Ok(());
    }
    bail!(
        "refusing to reset non-test postgres fixture database '{database_name}'. \
         Point PLATFORM_DATABASE_URL at a disposable test database, or set \
         {ALLOW_SHARED_DATABASE_ENV}=1 only after confirming this is intentional."
    )
}

fn shared_database_fixture_override_enabled() -> bool {
    std::env::var(ALLOW_SHARED_DATABASE_ENV)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn database_name_from_url(database_url: &str) -> Option<String> {
    let without_fragment = database_url.split('#').next().unwrap_or(database_url);
    let without_query = without_fragment
        .split('?')
        .next()
        .unwrap_or(without_fragment);
    let raw_name = without_query.rsplit('/').next()?.trim();
    (!raw_name.is_empty()).then(|| raw_name.to_ascii_lowercase())
}

fn database_name_looks_disposable(database_name: &str) -> bool {
    let normalized = database_name.trim().to_ascii_lowercase().replace('-', "_");
    normalized.contains("test")
        || normalized.starts_with("tmp_")
        || normalized.ends_with("_tmp")
        || normalized.starts_with("scratch_")
        || normalized.ends_with("_scratch")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_name_from_url_extracts_name_without_query() {
        assert_eq!(
            database_name_from_url("postgres://user:pass@127.0.0.1:5432/aiv3_test?sslmode=disable"),
            Some("aiv3_test".to_string())
        );
    }

    #[test]
    fn disposable_database_guard_rejects_shared_production_name() {
        assert!(!database_name_looks_disposable("ai_data_platform_v3"));
        assert!(database_name_looks_disposable("ai_data_platform_v3_test"));
        assert!(database_name_looks_disposable("scratch_aiv3"));
    }
}
