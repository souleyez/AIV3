pub(crate) fn platform_env_flag(key: &str, default_value: bool) -> bool {
    std::env::var(key)
        .ok()
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            matches!(normalized.as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(default_value)
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

    #[test]
    fn platform_env_flag_accepts_enabled_values_case_insensitively() {
        let _guard = env_lock();
        let key = "DATAMAX_TEST_PLATFORM_ENV_FLAG_ENABLED";
        std::env::set_var(key, " TrUe ");

        assert!(platform_env_flag(key, false));

        std::env::set_var(key, "on");
        assert!(platform_env_flag(key, false));

        std::env::set_var(key, "YES");
        assert!(platform_env_flag(key, false));

        std::env::set_var(key, "1");
        assert!(platform_env_flag(key, false));

        std::env::remove_var(key);
    }

    #[test]
    fn platform_env_flag_rejects_other_values_and_uses_default_when_missing() {
        let _guard = env_lock();
        let key = "DATAMAX_TEST_PLATFORM_ENV_FLAG_DEFAULT";
        std::env::remove_var(key);

        assert!(platform_env_flag(key, true));
        assert!(!platform_env_flag(key, false));

        std::env::set_var(key, "false");
        assert!(!platform_env_flag(key, true));

        std::env::set_var(key, "0");
        assert!(!platform_env_flag(key, true));

        std::env::remove_var(key);
    }
}
