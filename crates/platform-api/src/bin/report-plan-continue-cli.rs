use domain_model::ReportPlanId;
use std::str::FromStr;
use uuid::Uuid;

fn usage(program: &str) -> String {
    format!("Usage: {program} <plan_id> [--pretty]")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    observability::install("report_plan_continue_cli")?;

    let program = std::env::args()
        .next()
        .unwrap_or_else(|| "report-plan-continue-cli".to_string());
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    let pretty = if let Some(index) = args.iter().position(|arg| arg == "--pretty") {
        args.remove(index);
        true
    } else {
        false
    };

    let plan_id_raw = match args.as_slice() {
        [plan_id] => plan_id.clone(),
        _ => anyhow::bail!(usage(&program)),
    };

    let plan_id = ReportPlanId(Uuid::from_str(&plan_id_raw)?);
    let database_url = std::env::var("PLATFORM_DATABASE_URL")
        .unwrap_or_else(|_| storage::DEFAULT_LOCAL_DATABASE_URL.to_string());
    let storage = storage::PgStorage::connect(&database_url).await?;
    storage.migrate().await?;

    let tenant_key = std::env::var("PLATFORM_TENANT_KEY")
        .unwrap_or_else(|_| storage::DEFAULT_LOCAL_TENANT_KEY.to_string());
    let tenant_name = std::env::var("PLATFORM_TENANT_NAME")
        .unwrap_or_else(|_| storage::DEFAULT_LOCAL_TENANT_NAME.to_string());
    let tenant = storage.ensure_tenant(&tenant_key, &tenant_name).await?;
    let response = platform_api::request_report_plan_continue(storage, tenant.id, plan_id)
        .await
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;

    if pretty {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else {
        println!("{}", serde_json::to_string(&response)?);
    }

    Ok(())
}
