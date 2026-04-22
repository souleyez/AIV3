use contracts::PublishReportRequest;
use domain_model::{PublishedSurface, ReportPlanId};
use std::str::FromStr;
use uuid::Uuid;

fn usage(program: &str) -> String {
    format!("Usage: {program} <plan_id> <pc|mobile> [publish_note] [--pretty]")
}

fn parse_surface(raw: &str) -> anyhow::Result<PublishedSurface> {
    PublishedSurface::from_str(raw)
        .ok_or_else(|| anyhow::anyhow!("unsupported published surface: {raw}"))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    observability::install("report_publish_cli")?;

    let program = std::env::args()
        .next()
        .unwrap_or_else(|| "report-publish-cli".to_string());
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    let pretty = if let Some(index) = args.iter().position(|arg| arg == "--pretty") {
        args.remove(index);
        true
    } else {
        false
    };

    let (plan_id_raw, surface_raw, publish_note) = match args.as_slice() {
        [plan_id, surface] => (plan_id.clone(), surface.clone(), None),
        [plan_id, surface, publish_note] => {
            (plan_id.clone(), surface.clone(), Some(publish_note.clone()))
        }
        _ => anyhow::bail!(usage(&program)),
    };

    let plan_id = ReportPlanId(Uuid::from_str(&plan_id_raw)?);
    let surface = parse_surface(&surface_raw)?;
    let database_url = std::env::var("PLATFORM_DATABASE_URL")
        .unwrap_or_else(|_| storage::DEFAULT_LOCAL_DATABASE_URL.to_string());
    let storage = storage::PgStorage::connect(&database_url).await?;
    storage.migrate().await?;

    let tenant_key = std::env::var("PLATFORM_TENANT_KEY")
        .unwrap_or_else(|_| storage::DEFAULT_LOCAL_TENANT_KEY.to_string());
    let tenant_name = std::env::var("PLATFORM_TENANT_NAME")
        .unwrap_or_else(|_| storage::DEFAULT_LOCAL_TENANT_NAME.to_string());
    let tenant = storage.ensure_tenant(&tenant_key, &tenant_name).await?;
    let response = platform_api::request_report_publish(
        storage,
        tenant.id,
        plan_id,
        PublishReportRequest {
            surface,
            publish_note,
        },
    )
    .await
    .map_err(|error| anyhow::anyhow!(error.to_string()))?;

    if pretty {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else {
        println!("{}", serde_json::to_string(&response)?);
    }

    Ok(())
}
