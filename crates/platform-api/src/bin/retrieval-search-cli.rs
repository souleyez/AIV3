use domain_model::DatasetId;
use std::str::FromStr;
use uuid::Uuid;

fn usage(program: &str) -> String {
    format!("Usage: {program} search <dataset_id> <query> [--limit <n>] [--pretty]")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    observability::install("retrieval_search_cli")?;

    let program = std::env::args()
        .next()
        .unwrap_or_else(|| "retrieval-search-cli".to_string());
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    let pretty = if let Some(index) = args.iter().position(|arg| arg == "--pretty") {
        args.remove(index);
        true
    } else {
        false
    };

    let limit = if let Some(index) = args.iter().position(|arg| arg == "--limit") {
        if index + 1 >= args.len() {
            anyhow::bail!(usage(&program));
        }
        let raw_limit = args.remove(index + 1);
        args.remove(index);
        Some(raw_limit.parse::<usize>()?)
    } else {
        None
    };

    let (subcommand, dataset_id_raw, query) = match args.as_slice() {
        [subcommand, dataset_id, query] => (subcommand.as_str(), dataset_id.clone(), query.clone()),
        _ => anyhow::bail!(usage(&program)),
    };
    if subcommand != "search" {
        anyhow::bail!(usage(&program));
    }

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
    let response =
        platform_api::search_dataset_retrieval(storage, tenant.id, dataset_id, query, limit)
            .await
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;

    if pretty {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else {
        println!("{}", serde_json::to_string(&response)?);
    }

    Ok(())
}
