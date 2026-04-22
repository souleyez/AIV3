use domain_model::WorkflowExecutionId;
use std::str::FromStr;
use uuid::Uuid;

fn usage(program: &str) -> String {
    format!("Usage: {program} <execution_id> [--pretty]")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    observability::install("runtime_inspect_cli")?;

    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    let pretty = if let Some(index) = args.iter().position(|arg| arg == "--pretty") {
        args.remove(index);
        true
    } else {
        false
    };

    let execution_id_raw = match args.as_slice() {
        [execution_id] => execution_id.clone(),
        _ => {
            let program = std::env::args()
                .next()
                .unwrap_or_else(|| "runtime-inspect-cli".to_string());
            anyhow::bail!(usage(&program));
        }
    };

    let execution_id = WorkflowExecutionId(Uuid::from_str(&execution_id_raw)?);
    let database_url = std::env::var("PLATFORM_DATABASE_URL")
        .unwrap_or_else(|_| storage::DEFAULT_LOCAL_DATABASE_URL.to_string());
    let storage = storage::PgStorage::connect(&database_url).await?;
    storage.migrate().await?;
    let tenant = storage
        .ensure_tenant(
            storage::DEFAULT_LOCAL_TENANT_KEY,
            storage::DEFAULT_LOCAL_TENANT_NAME,
        )
        .await?;
    let inspect =
        platform_api::load_workflow_runtime_inspect(storage, tenant.id, execution_id).await?;

    if pretty {
        for summary in &inspect.pretty_summaries {
            println!("{summary}\n");
        }
        println!("{}", serde_json::to_string_pretty(&inspect)?);
    } else {
        println!("{}", serde_json::to_string(&inspect)?);
    }

    Ok(())
}
