use std::time::Duration;

const EXTERNAL_CHANNEL_DIRECT_REPLY_DEFAULT_TOTAL_BUDGET_MS: u64 = 60_000;
const EXTERNAL_CHANNEL_DIRECT_REPLY_DEFAULT_ATTEMPT_TIMEOUT_MS: u64 = 20_000;

pub(crate) fn external_channel_direct_reply_env_ms(key: &str, default_ms: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|value| {
            let value = value.trim();
            (!value.is_empty())
                .then(|| value.parse::<u64>().ok())
                .flatten()
        })
        .filter(|value| *value > 0)
        .unwrap_or(default_ms)
}

pub(crate) fn external_channel_direct_reply_total_budget() -> Duration {
    Duration::from_millis(external_channel_direct_reply_env_ms(
        "EXTERNAL_CHANNEL_DIRECT_REPLY_TOTAL_BUDGET_MS",
        EXTERNAL_CHANNEL_DIRECT_REPLY_DEFAULT_TOTAL_BUDGET_MS,
    ))
}

pub(crate) fn external_channel_direct_reply_attempt_timeout(
    remaining_budget: Duration,
) -> Duration {
    let configured = Duration::from_millis(external_channel_direct_reply_env_ms(
        "EXTERNAL_CHANNEL_DIRECT_REPLY_ATTEMPT_TIMEOUT_MS",
        EXTERNAL_CHANNEL_DIRECT_REPLY_DEFAULT_ATTEMPT_TIMEOUT_MS,
    ));
    configured
        .min(remaining_budget)
        .max(Duration::from_millis(1))
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

    fn clear_direct_reply_env() {
        std::env::remove_var("EXTERNAL_CHANNEL_DIRECT_REPLY_TOTAL_BUDGET_MS");
        std::env::remove_var("EXTERNAL_CHANNEL_DIRECT_REPLY_ATTEMPT_TIMEOUT_MS");
    }

    #[test]
    fn direct_reply_timeout_defaults_are_20_way_safe() {
        let _guard = env_lock();
        clear_direct_reply_env();

        assert_eq!(
            external_channel_direct_reply_total_budget(),
            Duration::from_millis(60_000)
        );
        assert_eq!(
            external_channel_direct_reply_attempt_timeout(Duration::from_millis(60_000)),
            Duration::from_millis(20_000)
        );

        std::env::set_var("EXTERNAL_CHANNEL_DIRECT_REPLY_TOTAL_BUDGET_MS", "45000");
        std::env::set_var("EXTERNAL_CHANNEL_DIRECT_REPLY_ATTEMPT_TIMEOUT_MS", "15000");
        assert_eq!(
            external_channel_direct_reply_total_budget(),
            Duration::from_millis(45_000)
        );
        assert_eq!(
            external_channel_direct_reply_attempt_timeout(Duration::from_millis(5_000)),
            Duration::from_millis(5_000)
        );

        std::env::set_var("EXTERNAL_CHANNEL_DIRECT_REPLY_TOTAL_BUDGET_MS", "0");
        std::env::set_var("EXTERNAL_CHANNEL_DIRECT_REPLY_ATTEMPT_TIMEOUT_MS", "bad");
        assert_eq!(
            external_channel_direct_reply_total_budget(),
            Duration::from_millis(60_000)
        );
        assert_eq!(
            external_channel_direct_reply_attempt_timeout(Duration::from_millis(60_000)),
            Duration::from_millis(20_000)
        );

        clear_direct_reply_env();
    }

    #[test]
    fn env_ms_trims_and_rejects_empty_invalid_or_zero_values() {
        let _guard = env_lock();
        let key = "EXTERNAL_CHANNEL_DIRECT_REPLY_ENV_MS_TEST";
        std::env::remove_var(key);

        assert_eq!(external_channel_direct_reply_env_ms(key, 123), 123);

        std::env::set_var(key, " 250 ");
        assert_eq!(external_channel_direct_reply_env_ms(key, 123), 250);

        for value in ["", "   ", "0", "-1", "bad"] {
            std::env::set_var(key, value);
            assert_eq!(external_channel_direct_reply_env_ms(key, 123), 123);
        }

        std::env::remove_var(key);
    }
}
