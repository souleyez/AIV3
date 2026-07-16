use anyhow::Result;
use std::sync::OnceLock;
use tracing_subscriber::{
    filter::{filter_fn, FilterExt},
    layer::{Layer, SubscriberExt},
    util::SubscriberInitExt,
    EnvFilter,
};
use url::{Host, Url};

static INSTALLED: OnceLock<()> = OnceLock::new();
const REDACTED_ENDPOINT: &str = "<redacted-endpoint>";

/// Returns a connection URL safe for diagnostics.
///
/// Only the scheme, host, and parsed port (when present) are retained. Any
/// user information, path, query, or fragment is discarded. JDBC URLs are
/// accepted by parsing their nested URL while retaining the `jdbc:` prefix.
/// Invalid or hostless input is never echoed back to the caller.
pub fn redact_connection_endpoint(raw: &str) -> String {
    let raw = raw.trim();
    if raw.is_empty() || raw.chars().any(char::is_control) {
        return REDACTED_ENDPOINT.to_string();
    }
    let (prefix, candidate) = match raw.get(..5) {
        Some(prefix) if prefix.eq_ignore_ascii_case("jdbc:") => ("jdbc:", &raw[5..]),
        _ => ("", raw),
    };

    let Some(endpoint) = Url::parse(candidate)
        .ok()
        .and_then(|url| safe_endpoint(&url))
    else {
        return REDACTED_ENDPOINT.to_string();
    };

    format!("{prefix}{endpoint}")
}

fn safe_endpoint(url: &Url) -> Option<String> {
    let host = match url.host()? {
        Host::Domain(domain) => domain.to_string(),
        Host::Ipv4(address) => address.to_string(),
        Host::Ipv6(address) => format!("[{address}]"),
    };
    let port = url
        .port()
        .map(|port| format!(":{port}"))
        .unwrap_or_default();

    Some(format!("{}://{host}{port}", url.scheme()))
}

fn dependency_target_is_safe(target: &str) -> bool {
    target != "async_nats" && !target.starts_with("async_nats::")
}

pub fn install(service_name: &str) -> Result<()> {
    if INSTALLED.get().is_some() {
        return Ok(());
    }

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("{service_name}=info,axum=info")));
    let safe_filter = filter.and(filter_fn(|metadata| {
        dependency_target_is_safe(metadata.target())
    }));

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_current_span(true)
                .with_span_list(true)
                .with_filter(safe_filter),
        )
        .try_init()?;

    let _ = INSTALLED.set(());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_endpoint_redacts_userinfo_query_fragment_and_path() {
        assert_eq!(
            redact_connection_endpoint(
                "postgresql://report%40user:p%40ssword@db.example.com:5432/customer/private?sslmode=require&token=secret#credential",
            ),
            "postgresql://db.example.com:5432"
        );
        assert_eq!(
            redact_connection_endpoint(
                "nats://token%2Fname:secret@nats.example.com:4222/private/path?auth=hidden#secret",
            ),
            "nats://nats.example.com:4222"
        );
    }

    #[test]
    fn connection_endpoint_handles_ipv6_and_default_ports() {
        assert_eq!(
            redact_connection_endpoint(
                "postgres://user:password@[2001:db8::7]:5432/aiv3?sslmode=require",
            ),
            "postgres://[2001:db8::7]:5432"
        );
        assert_eq!(
            redact_connection_endpoint("nats://user:password@nats.internal/messages"),
            "nats://nats.internal"
        );
    }

    #[test]
    fn connection_endpoint_rejects_malformed_or_secret_only_values() {
        for raw in [
            "secret-only-value",
            "://user:password@",
            "postgres://user:password@",
            "postgres://user:password@db.example.com:99999/database",
            "postgres://user:password@db.example.com/\nsecret",
            "nats:opaque-secret",
            "",
        ] {
            assert_eq!(redact_connection_endpoint(raw), "<redacted-endpoint>");
        }
    }

    #[test]
    fn runtime_filter_blocks_async_nats_even_when_rust_log_is_verbose() {
        assert!(!dependency_target_is_safe("async_nats"));
        assert!(!dependency_target_is_safe("async_nats::connector"));
        assert!(dependency_target_is_safe("event_bus"));
        assert!(dependency_target_is_safe("platform_api"));

        let safe_filter = EnvFilter::new("trace").and(filter_fn(|metadata| {
            dependency_target_is_safe(metadata.target())
        }));
        let subscriber = tracing_subscriber::registry().with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::sink)
                .with_filter(safe_filter),
        );
        tracing::subscriber::with_default(subscriber, || {
            assert!(!tracing::enabled!(
                target: "async_nats::connector",
                tracing::Level::ERROR
            ));
            assert!(tracing::enabled!(
                target: "platform_api",
                tracing::Level::TRACE
            ));
        });
    }
}
