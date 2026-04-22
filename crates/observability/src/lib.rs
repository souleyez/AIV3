use anyhow::Result;
use std::sync::OnceLock;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

static INSTALLED: OnceLock<()> = OnceLock::new();

pub fn install(service_name: &str) -> Result<()> {
    if INSTALLED.get().is_some() {
        return Ok(());
    }

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("{service_name}=info,axum=info")));

    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_current_span(true)
                .with_span_list(true),
        )
        .try_init()?;

    let _ = INSTALLED.set(());
    Ok(())
}
