use std::time::Duration;

use crate::{LlmProviderError, LlmProviderFailureKind};

pub(crate) const ASSISTANT_RUN_RUNTIME_RETRY_DEFAULT_ATTEMPTS: usize = 3;
pub(crate) const ASSISTANT_RUN_RUNTIME_RETRY_MAX_ATTEMPTS: usize = 5;
pub(crate) const ASSISTANT_RUN_RUNTIME_RETRY_DEFAULT_BACKOFF_MS: u64 = 400;
pub(crate) const ASSISTANT_RUN_RUNTIME_RETRY_MAX_BACKOFF_MS: u64 = 10_000;

pub(crate) fn assistant_run_runtime_retry_attempts(env_prefix: &str) -> usize {
    assistant_run_runtime_retry_env_usize(
        &format!("{env_prefix}_RUNTIME_RETRY_ATTEMPTS"),
        ASSISTANT_RUN_RUNTIME_RETRY_DEFAULT_ATTEMPTS,
        ASSISTANT_RUN_RUNTIME_RETRY_MAX_ATTEMPTS,
    )
}

pub(crate) fn assistant_run_runtime_retry_backoff(env_prefix: &str) -> Duration {
    Duration::from_millis(assistant_run_runtime_retry_env_u64(
        &format!("{env_prefix}_RUNTIME_RETRY_BACKOFF_MS"),
        ASSISTANT_RUN_RUNTIME_RETRY_DEFAULT_BACKOFF_MS,
        ASSISTANT_RUN_RUNTIME_RETRY_MAX_BACKOFF_MS,
    ))
}

fn assistant_run_runtime_retry_env_usize(
    key: &str,
    default_value: usize,
    max_value: usize,
) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|value| {
            let value = value.trim();
            (!value.is_empty())
                .then(|| value.parse::<usize>().ok())
                .flatten()
        })
        .map(|value| value.clamp(1, max_value))
        .unwrap_or(default_value)
}

fn assistant_run_runtime_retry_env_u64(key: &str, default_value: u64, max_value: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|value| {
            let value = value.trim();
            (!value.is_empty())
                .then(|| value.parse::<u64>().ok())
                .flatten()
        })
        .map(|value| value.clamp(1, max_value))
        .unwrap_or(default_value)
}

pub(crate) fn assistant_run_runtime_retry_delay(
    base_delay: Duration,
    attempt_index: usize,
) -> Duration {
    let factor = 1_u32 << attempt_index.min(4);
    base_delay.saturating_mul(factor)
}

pub(crate) fn assistant_run_provider_error_is_retryable(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<LlmProviderError>()
        .and_then(|provider_error| provider_error.runtime().provider_failure.as_ref())
        .map(|failure| {
            matches!(
                failure.kind,
                LlmProviderFailureKind::RequestFailed
                    | LlmProviderFailureKind::RequestTimeout
                    | LlmProviderFailureKind::ResponseBodyReadFailed
            ) || (failure.kind == LlmProviderFailureKind::HttpStatus
                && assistant_run_provider_http_status_is_retryable(&failure.message))
                || (failure.kind == LlmProviderFailureKind::InvalidResponse
                    && assistant_run_provider_invalid_response_is_retryable(&failure.message))
        })
        .unwrap_or(false)
}

fn assistant_run_provider_http_status_is_retryable(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    [
        "http 429",
        "http 500",
        "http 502",
        "http 503",
        "http 504",
        "status 429",
        "status 500",
        "status 502",
        "status 503",
        "status 504",
        "too many requests",
        "rate limit",
        "bad gateway",
        "gateway timeout",
        "upstream",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn assistant_run_provider_invalid_response_is_retryable(message: &str) -> bool {
    message
        .to_ascii_lowercase()
        .contains("streaming response missing assistant content")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clear_env(key: &str) {
        std::env::remove_var(key);
    }

    #[test]
    fn retry_attempts_env_uses_default_and_clamps_bounds() {
        let key = "DATAMAX_TEST_RUNTIME_RETRY_ATTEMPTS";
        clear_env(key);
        assert_eq!(
            assistant_run_runtime_retry_env_usize(key, 3, 5),
            3,
            "missing env should use default"
        );

        std::env::set_var(key, "0");
        assert_eq!(assistant_run_runtime_retry_env_usize(key, 3, 5), 1);

        std::env::set_var(key, "8");
        assert_eq!(assistant_run_runtime_retry_env_usize(key, 3, 5), 5);

        std::env::set_var(key, "invalid");
        assert_eq!(assistant_run_runtime_retry_env_usize(key, 3, 5), 3);
        clear_env(key);
    }

    #[test]
    fn retry_backoff_env_uses_default_and_clamps_bounds() {
        let key = "DATAMAX_TEST_RUNTIME_RETRY_BACKOFF_MS";
        clear_env(key);
        assert_eq!(assistant_run_runtime_retry_env_u64(key, 400, 10_000), 400);

        std::env::set_var(key, "0");
        assert_eq!(assistant_run_runtime_retry_env_u64(key, 400, 10_000), 1);

        std::env::set_var(key, "20000");
        assert_eq!(
            assistant_run_runtime_retry_env_u64(key, 400, 10_000),
            10_000
        );
        clear_env(key);
    }

    #[test]
    fn retry_delay_exponentially_backs_off_with_cap() {
        let base = Duration::from_millis(10);
        assert_eq!(assistant_run_runtime_retry_delay(base, 0), base);
        assert_eq!(
            assistant_run_runtime_retry_delay(base, 3),
            Duration::from_millis(80)
        );
        assert_eq!(
            assistant_run_runtime_retry_delay(base, 8),
            Duration::from_millis(160)
        );
    }

    #[test]
    fn provider_retry_covers_transient_http_and_empty_streaming_response() {
        assert!(assistant_run_provider_http_status_is_retryable(
            "rightcode returned HTTP 502 with body error code: 502"
        ));
        assert!(assistant_run_provider_http_status_is_retryable(
            "rightcode returned HTTP 429 too many requests"
        ));
        assert!(!assistant_run_provider_http_status_is_retryable(
            "rightcode returned HTTP 401 authorized_error"
        ));
        assert!(assistant_run_provider_invalid_response_is_retryable(
            "rightcode streaming response missing assistant content"
        ));
        assert!(!assistant_run_provider_invalid_response_is_retryable(
            "rightcode returned malformed tool arguments"
        ));
    }
}
