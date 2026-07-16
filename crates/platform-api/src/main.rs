#[tokio::main]
async fn main() -> anyhow::Result<()> {
    observability::install("platform_api")?;

    let database_url = std::env::var("PLATFORM_DATABASE_URL")
        .unwrap_or_else(|_| storage::DEFAULT_LOCAL_DATABASE_URL.to_string());
    let workflow_catalog = workflow_definitions::catalog();
    let storage = storage::PgStorage::connect_with_configured_max_connections(
        &database_url,
        "PLATFORM_API_DATABASE_MAX_CONNECTIONS",
    )
    .await?;
    let event_bus = event_bus::EventBus::connect_from_env_or_disabled("PLATFORM_NATS_URL").await;
    storage.migrate().await?;
    storage
        .sync_workflow_definitions(&workflow_catalog.descriptors())
        .await?;
    let tenant = storage
        .ensure_tenant(
            storage::DEFAULT_LOCAL_TENANT_KEY,
            storage::DEFAULT_LOCAL_TENANT_NAME,
        )
        .await?;

    let addr = std::env::var("PLATFORM_API_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_string());
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    tracing::info!(
        %addr,
        database_endpoint = %observability::redact_connection_endpoint(&database_url),
        tenant_key = %tenant.key,
        "platform-api listening"
    );
    axum::serve(
        listener,
        platform_api::router(storage, workflow_catalog, tenant.id, event_bus),
    )
    .await?;

    Ok(())
}
