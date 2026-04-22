use anyhow::Result;
use chrono::Utc;
use domain_model::{
    Dataset, DatasetId, DatasetLifecycle, ReportModule, ReportModuleId, ReportModuleKind,
    ReportPlan, ReportPlanId, ReportPlanStatus, SecretBindingId, TenantId,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::OnceLock;
use std::time::Duration;
use storage::{PgStorage, DEFAULT_LOCAL_DATABASE_URL, TABLES};
use tokio::sync::Mutex;

pub fn sample_dataset() -> Dataset {
    Dataset {
        id: DatasetId::new(),
        tenant_id: TenantId::new(),
        key: "sample-dataset".to_string(),
        title: "Sample Dataset".to_string(),
        description: Some("Fixture dataset used for early workflow testing.".to_string()),
        lifecycle: DatasetLifecycle::Active,
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
    Available(PgStorage),
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
    let mut guard = state.lock().await;

    if let Some(current) = guard.as_ref() {
        return match current {
            SharedLocalPostgresState::Available(storage) => Ok(storage.clone()),
            SharedLocalPostgresState::Unavailable(reason) => Err(reason.clone()),
        };
    }

    let database_url = std::env::var("PLATFORM_DATABASE_URL")
        .unwrap_or_else(|_| DEFAULT_LOCAL_DATABASE_URL.into());
    let resolved =
        match PgStorage::connect_with_settings(&database_url, 4, Duration::from_secs(2)).await {
            Ok(storage) => match storage.migrate().await {
                Ok(()) => SharedLocalPostgresState::Available(storage),
                Err(error) => SharedLocalPostgresState::Unavailable(format!(
                    "failed to migrate shared local postgres fixture at {database_url}: {error}"
                )),
            },
            Err(error) => SharedLocalPostgresState::Unavailable(format!(
                "failed to connect shared local postgres fixture at {database_url}: {error}"
            )),
        };

    *guard = Some(resolved.clone());
    match resolved {
        SharedLocalPostgresState::Available(storage) => Ok(storage),
        SharedLocalPostgresState::Unavailable(reason) => Err(reason),
    }
}

pub async fn reset_local_postgres_storage(storage: &PgStorage) -> Result<()> {
    let truncate = format!("truncate table {} cascade", TABLES.join(", "));
    sqlx::query(&truncate).execute(storage.pool()).await?;
    storage.migrate().await?;
    Ok(())
}
