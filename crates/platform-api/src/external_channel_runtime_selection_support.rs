use crate::DEFAULT_ASSISTANT_RUN_RUNTIME_MODEL;
use llm_gateway::{LlmRuntimeSelection, MODEL_LANE_ASSISTANT_CHAT};

pub(crate) fn external_channel_fallback_runtime_selection_from_env() -> Option<LlmRuntimeSelection>
{
    let mode = std::env::var("ASSISTANT_RUN_FALLBACK_RUNTIME_MODE").ok();
    let provider = std::env::var("ASSISTANT_RUN_FALLBACK_RUNTIME_PROVIDER").ok();
    let model = std::env::var("ASSISTANT_RUN_FALLBACK_RUNTIME_MODEL").ok();
    if mode.is_none() && provider.is_none() && model.is_none() {
        return None;
    }

    let mode = mode.unwrap_or_else(|| "provider".to_string());
    let provider = provider.unwrap_or_else(|| mode.clone());
    let model = model.unwrap_or_else(|| DEFAULT_ASSISTANT_RUN_RUNTIME_MODEL.to_string());
    Some(LlmRuntimeSelection {
        mode,
        provider,
        model,
        lane: MODEL_LANE_ASSISTANT_CHAT.to_string(),
    })
}

pub(crate) fn external_channel_same_runtime_selection(
    left: &LlmRuntimeSelection,
    right: &LlmRuntimeSelection,
) -> bool {
    left.mode == right.mode
        && left.provider == right.provider
        && left.model == right.model
        && left.lane == right.lane
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

    fn clear_fallback_runtime_env() {
        std::env::remove_var("ASSISTANT_RUN_FALLBACK_RUNTIME_MODE");
        std::env::remove_var("ASSISTANT_RUN_FALLBACK_RUNTIME_PROVIDER");
        std::env::remove_var("ASSISTANT_RUN_FALLBACK_RUNTIME_MODEL");
    }

    fn selection(mode: &str, provider: &str, model: &str, lane: &str) -> LlmRuntimeSelection {
        LlmRuntimeSelection {
            mode: mode.to_string(),
            provider: provider.to_string(),
            model: model.to_string(),
            lane: lane.to_string(),
        }
    }

    #[test]
    fn fallback_runtime_selection_returns_none_without_env() {
        let _guard = env_lock();
        clear_fallback_runtime_env();

        assert!(external_channel_fallback_runtime_selection_from_env().is_none());

        clear_fallback_runtime_env();
    }

    #[test]
    fn fallback_runtime_selection_defaults_provider_and_model() {
        let _guard = env_lock();
        clear_fallback_runtime_env();

        std::env::set_var("ASSISTANT_RUN_FALLBACK_RUNTIME_MODE", "fallback-mode");
        let runtime =
            external_channel_fallback_runtime_selection_from_env().expect("fallback runtime");

        assert_eq!(runtime.mode, "fallback-mode");
        assert_eq!(runtime.provider, "fallback-mode");
        assert_eq!(runtime.model, DEFAULT_ASSISTANT_RUN_RUNTIME_MODEL);
        assert_eq!(runtime.lane, MODEL_LANE_ASSISTANT_CHAT);

        clear_fallback_runtime_env();
    }

    #[test]
    fn fallback_runtime_selection_reads_explicit_env() {
        let _guard = env_lock();
        clear_fallback_runtime_env();

        std::env::set_var("ASSISTANT_RUN_FALLBACK_RUNTIME_MODE", "provider");
        std::env::set_var("ASSISTANT_RUN_FALLBACK_RUNTIME_PROVIDER", "minimax");
        std::env::set_var("ASSISTANT_RUN_FALLBACK_RUNTIME_MODEL", "m3-fallback");

        let runtime =
            external_channel_fallback_runtime_selection_from_env().expect("fallback runtime");
        assert_eq!(runtime.mode, "provider");
        assert_eq!(runtime.provider, "minimax");
        assert_eq!(runtime.model, "m3-fallback");
        assert_eq!(runtime.lane, MODEL_LANE_ASSISTANT_CHAT);

        clear_fallback_runtime_env();
    }

    #[test]
    fn same_runtime_selection_compares_all_wire_fields() {
        let primary = selection(
            "provider",
            "openai",
            "primary-v1",
            MODEL_LANE_ASSISTANT_CHAT,
        );
        let same = selection(
            "provider",
            "openai",
            "primary-v1",
            MODEL_LANE_ASSISTANT_CHAT,
        );

        assert!(external_channel_same_runtime_selection(&primary, &same));

        for different in [
            selection(
                "fallback",
                "openai",
                "primary-v1",
                MODEL_LANE_ASSISTANT_CHAT,
            ),
            selection(
                "provider",
                "minimax",
                "primary-v1",
                MODEL_LANE_ASSISTANT_CHAT,
            ),
            selection(
                "provider",
                "openai",
                "fallback-v1",
                MODEL_LANE_ASSISTANT_CHAT,
            ),
            selection("provider", "openai", "primary-v1", "assistant-react-json"),
        ] {
            assert!(!external_channel_same_runtime_selection(
                &primary, &different
            ));
        }
    }
}
