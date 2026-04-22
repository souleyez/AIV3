use contracts::RetryWorkflowExecutionRequest;
use domain_model::WorkflowExecutionId;
use std::str::FromStr;
use uuid::Uuid;

fn usage(program: &str) -> String {
    format!("Usage: {program} <execution_id> <reason> [--pretty]")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    observability::install("workflow_retry_cli")?;

    let program = std::env::args()
        .next()
        .unwrap_or_else(|| "workflow-retry-cli".to_string());
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    let pretty = if let Some(index) = args.iter().position(|arg| arg == "--pretty") {
        args.remove(index);
        true
    } else {
        false
    };

    let (execution_id_raw, reason) = match args.as_slice() {
        [execution_id, reason] => (execution_id.clone(), reason.clone()),
        _ => anyhow::bail!(usage(&program)),
    };

    let execution_id = WorkflowExecutionId(Uuid::from_str(&execution_id_raw)?);
    let database_url = std::env::var("PLATFORM_DATABASE_URL")
        .unwrap_or_else(|_| storage::DEFAULT_LOCAL_DATABASE_URL.to_string());
    let storage = storage::PgStorage::connect(&database_url).await?;
    storage.migrate().await?;

    let tenant_key = std::env::var("PLATFORM_TENANT_KEY")
        .unwrap_or_else(|_| storage::DEFAULT_LOCAL_TENANT_KEY.to_string());
    let tenant_name = std::env::var("PLATFORM_TENANT_NAME")
        .unwrap_or_else(|_| storage::DEFAULT_LOCAL_TENANT_NAME.to_string());
    let tenant = storage.ensure_tenant(&tenant_key, &tenant_name).await?;
    let response = platform_api::request_workflow_retry(
        storage,
        tenant.id,
        execution_id,
        RetryWorkflowExecutionRequest { reason },
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
