use std::time::Duration;

use crate::external_channel_direct_reply_budget_support::external_channel_direct_reply_env_ms;

pub(crate) fn model_gateway_shadow_eval_timeout() -> Duration {
    Duration::from_millis(external_channel_direct_reply_env_ms(
        "LLM_GATEWAY_SHADOW_EVAL_TIMEOUT_MS",
        3_000,
    ))
}

pub(crate) fn model_gateway_shadow_eval_max_profiles() -> usize {
    std::env::var("LLM_GATEWAY_SHADOW_EVAL_MAX_PROFILES")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .map(|value| value.min(4))
        .unwrap_or(1)
}

pub(crate) fn model_gateway_u64_to_i32(value: u64) -> Option<i32> {
    i32::try_from(value).ok()
}

pub(crate) fn model_gateway_sanitized_error_kind(reason: &str) -> String {
    reason
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | ':' | '.'))
        .take(120)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env lock")
    }

    fn clear_shadow_eval_env() {
        std::env::remove_var("LLM_GATEWAY_SHADOW_EVAL_TIMEOUT_MS");
        std::env::remove_var("LLM_GATEWAY_SHADOW_EVAL_MAX_PROFILES");
    }

    #[test]
    fn model_gateway_shadow_eval_timeout_uses_default_and_positive_override() {
        let _guard = env_lock();
        clear_shadow_eval_env();

        assert_eq!(
            model_gateway_shadow_eval_timeout(),
            Duration::from_millis(3_000)
        );

        std::env::set_var("LLM_GATEWAY_SHADOW_EVAL_TIMEOUT_MS", " 1500 ");
        assert_eq!(
            model_gateway_shadow_eval_timeout(),
            Duration::from_millis(1_500)
        );

        for value in ["", "0", "bad"] {
            std::env::set_var("LLM_GATEWAY_SHADOW_EVAL_TIMEOUT_MS", value);
            assert_eq!(
                model_gateway_shadow_eval_timeout(),
                Duration::from_millis(3_000)
            );
        }

        clear_shadow_eval_env();
    }

    #[test]
    fn model_gateway_shadow_eval_max_profiles_defaults_and_caps() {
        let _guard = env_lock();
        clear_shadow_eval_env();

        assert_eq!(model_gateway_shadow_eval_max_profiles(), 1);

        std::env::set_var("LLM_GATEWAY_SHADOW_EVAL_MAX_PROFILES", " 3 ");
        assert_eq!(model_gateway_shadow_eval_max_profiles(), 3);

        std::env::set_var("LLM_GATEWAY_SHADOW_EVAL_MAX_PROFILES", "9");
        assert_eq!(model_gateway_shadow_eval_max_profiles(), 4);

        for value in ["", "0", "bad"] {
            std::env::set_var("LLM_GATEWAY_SHADOW_EVAL_MAX_PROFILES", value);
            assert_eq!(model_gateway_shadow_eval_max_profiles(), 1);
        }

        clear_shadow_eval_env();
    }

    #[test]
    fn model_gateway_u64_to_i32_preserves_in_range_values_and_rejects_overflow() {
        assert_eq!(model_gateway_u64_to_i32(0), Some(0));
        assert_eq!(model_gateway_u64_to_i32(i32::MAX as u64), Some(i32::MAX));
        assert_eq!(model_gateway_u64_to_i32(i32::MAX as u64 + 1), None);
    }

    #[test]
    fn model_gateway_sanitized_error_kind_keeps_safe_ascii_and_truncates() {
        let raw = format!(
            "provider_error:429.rate_limit-too_many_requests_{}中文 !@#",
            "x".repeat(160)
        );
        let sanitized = model_gateway_sanitized_error_kind(&raw);

        assert!(sanitized.starts_with("provider_error:429.rate_limit-too_many_requests_"));
        assert_eq!(sanitized.len(), 120);
        assert!(sanitized
            .chars()
            .all(|ch| { ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | ':' | '.') }));
    }
}
