use crate::{env_flag, external_channel_support::external_channel_platform_wire_value};
use contracts::ExternalBotMessageView;
use llm_gateway::{model_gateway_lane_env_prefix, MODEL_LANE_ASSISTANT_CHAT};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use crate::model_gateway_runtime::{
    model_gateway_lane_canary_percent, model_gateway_lane_routing_mode,
};

pub(crate) fn external_channel_model_pool_is_active(
    connection_id: &str,
    message: &ExternalBotMessageView,
) -> bool {
    if !external_channel_model_pool_scope_is_active(connection_id, message) {
        return false;
    }
    match model_gateway_lane_routing_mode(MODEL_LANE_ASSISTANT_CHAT).as_str() {
        "active" => true,
        "canary" => external_channel_model_pool_canary_hit(connection_id, message),
        _ => false,
    }
}

pub(crate) fn external_channel_model_pool_is_shadow_eval(
    connection_id: &str,
    message: &ExternalBotMessageView,
) -> bool {
    model_gateway_lane_routing_mode(MODEL_LANE_ASSISTANT_CHAT) == "shadow_eval"
        && external_channel_model_pool_scope_is_active(connection_id, message)
}

pub(crate) fn external_channel_model_pool_is_observe_only(
    connection_id: &str,
    message: &ExternalBotMessageView,
) -> bool {
    model_gateway_lane_routing_mode(MODEL_LANE_ASSISTANT_CHAT) == "observe_only"
        && external_channel_model_pool_scope_is_active(connection_id, message)
}

fn external_channel_model_pool_canary_hit(
    connection_id: &str,
    message: &ExternalBotMessageView,
) -> bool {
    let Some(percent) = model_gateway_lane_canary_percent(MODEL_LANE_ASSISTANT_CHAT) else {
        return false;
    };
    if percent == 0 {
        return false;
    }
    if percent >= 100 {
        return true;
    }

    let prefix = model_gateway_lane_env_prefix(MODEL_LANE_ASSISTANT_CHAT);
    let salt = std::env::var(format!("{prefix}_CANARY_SALT")).unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    MODEL_LANE_ASSISTANT_CHAT.hash(&mut hasher);
    salt.hash(&mut hasher);
    connection_id.hash(&mut hasher);
    message.tenant_external_id.hash(&mut hasher);
    message.bot_external_id.hash(&mut hasher);
    message.conversation_external_id.hash(&mut hasher);
    message.sender_external_id.hash(&mut hasher);
    let bucket = (hasher.finish() % 100) as u32;
    bucket < percent
}

fn external_channel_model_pool_scope_is_active(
    connection_id: &str,
    message: &ExternalBotMessageView,
) -> bool {
    env_flag("LLM_GATEWAY_EXTERNAL_CHANNEL_ACTIVE", false)
        || env_csv_contains(
            "LLM_GATEWAY_EXTERNAL_CHANNEL_ACTIVE_CONNECTIONS",
            connection_id,
        )
        || env_csv_contains(
            "LLM_GATEWAY_EXTERNAL_CHANNEL_ACTIVE_TENANTS",
            &message.tenant_external_id,
        )
        || env_csv_contains(
            "LLM_GATEWAY_EXTERNAL_CHANNEL_ACTIVE_PLATFORMS",
            external_channel_platform_wire_value(&message.platform),
        )
}

pub(crate) fn env_csv_contains(key: &str, expected: &str) -> bool {
    let expected = expected.trim();
    !expected.is_empty()
        && std::env::var(key)
            .ok()
            .map(|value| {
                value
                    .split(',')
                    .map(str::trim)
                    .any(|item| item.eq_ignore_ascii_case(expected))
            })
            .unwrap_or(false)
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
    fn env_csv_contains_trims_matches_case_insensitively_and_ignores_empty_expected() {
        let _guard = env_lock();
        let key = "DATAMAX_TEST_EXTERNAL_CHANNEL_MODEL_POOL_CSV";
        std::env::set_var(key, "alpha, Beta ,gamma");

        assert!(env_csv_contains(key, "beta"));
        assert!(env_csv_contains(key, " BETA "));
        assert!(!env_csv_contains(key, "delta"));
        assert!(!env_csv_contains(key, "   "));

        std::env::remove_var(key);
    }
}
