use domain_model::DatasetId;
use std::str::FromStr;
use uuid::Uuid;

fn usage(program: &str) -> String {
    format!("Usage: {program} <dataset_id> [--pretty]")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    observability::install("memory_directory_refresh_cli")?;

    let program = std::env::args()
        .next()
        .unwrap_or_else(|| "memory-directory-refresh-cli".to_string());
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    let pretty = if let Some(index) = args.iter().position(|arg| arg == "--pretty") {
        args.remove(index);
        true
    } else {
        false
    };

    let dataset_id_raw = match args.as_slice() {
        [dataset_id] => dataset_id.clone(),
        _ => anyhow::bail!(usage(&program)),
    };

    let dataset_id = DatasetId(Uuid::from_str(&dataset_id_raw)?);
    let database_url = std::env::var("PLATFORM_DATABASE_URL")
        .unwrap_or_else(|_| storage::DEFAULT_LOCAL_DATABASE_URL.to_string());
    let storage = storage::PgStorage::connect(&database_url).await?;
    storage.migrate().await?;

    let tenant_key = std::env::var("PLATFORM_TENANT_KEY")
        .unwrap_or_else(|_| storage::DEFAULT_LOCAL_TENANT_KEY.to_string());
    let tenant_name = std::env::var("PLATFORM_TENANT_NAME")
        .unwrap_or_else(|_| storage::DEFAULT_LOCAL_TENANT_NAME.to_string());
    let tenant = storage.ensure_tenant(&tenant_key, &tenant_name).await?;
    let response = platform_api::request_memory_directory_refresh(storage, tenant.id, dataset_id)
        .await
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;

    if pretty {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else {
        println!("{}", serde_json::to_string(&response)?);
    }

    Ok(())
}
