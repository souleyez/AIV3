use contracts::CompareDocumentsRequest;
use domain_model::DocumentId;
use std::str::FromStr;
use uuid::Uuid;

fn usage(program: &str) -> String {
    format!("Usage: {program} <document_id> <document_id> [<document_id> ...] [--pretty]")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    observability::install("document_compare_cli")?;

    let program = std::env::args()
        .next()
        .unwrap_or_else(|| "document-compare-cli".to_string());
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    let pretty = if let Some(index) = args.iter().position(|arg| arg == "--pretty") {
        args.remove(index);
        true
    } else {
        false
    };

    if args.len() < 2 {
        anyhow::bail!(usage(&program));
    }

    let document_ids = args
        .into_iter()
        .map(|raw| Uuid::from_str(&raw).map(DocumentId))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let database_url = std::env::var("PLATFORM_DATABASE_URL")
        .unwrap_or_else(|_| storage::DEFAULT_LOCAL_DATABASE_URL.to_string());
    let storage = storage::PgStorage::connect(&database_url).await?;
    storage.migrate().await?;

    let tenant_key = std::env::var("PLATFORM_TENANT_KEY")
        .unwrap_or_else(|_| storage::DEFAULT_LOCAL_TENANT_KEY.to_string());
    let tenant_name = std::env::var("PLATFORM_TENANT_NAME")
        .unwrap_or_else(|_| storage::DEFAULT_LOCAL_TENANT_NAME.to_string());
    let tenant = storage.ensure_tenant(&tenant_key, &tenant_name).await?;
    let response = platform_api::compare_documents(
        storage,
        tenant.id,
        CompareDocumentsRequest { document_ids },
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
