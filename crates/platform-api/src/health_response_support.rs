use chrono::{DateTime, Utc};
use contracts::HealthResponse;

pub(crate) fn platform_api_health_response(
    status: &str,
    checked_at: DateTime<Utc>,
) -> HealthResponse {
    HealthResponse {
        service: "platform-api".to_string(),
        status: status.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        checked_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_api_health_response_preserves_wire_fields() {
        let checked_at = Utc::now();
        let response = platform_api_health_response("ready", checked_at);

        assert_eq!(response.service, "platform-api");
        assert_eq!(response.status, "ready");
        assert_eq!(response.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(response.checked_at, checked_at);
    }
}
