use crate::{
    env_flag,
    external_channel_runtime_selection_support::{
        external_channel_fallback_runtime_selection_from_env,
        external_channel_same_runtime_selection,
    },
    external_channel_support::external_channel_platform_wire_value,
    model_gateway_runtime::model_gateway_provider_profile_from_record,
};
use contracts::ExternalBotMessageView;
use llm_gateway::{
    model_gateway_lane_env_prefix, model_gateway_profile_env_prefix, LlmRuntimeSelection,
    ModelGatewayLaneLimits, ModelProviderProfile, MODEL_LANE_ASSISTANT_CHAT,
};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};
use storage::ModelGatewayProfile;

use crate::model_gateway_runtime::{
    model_gateway_lane_canary_percent, model_gateway_lane_routing_mode,
};

#[derive(Clone, Debug)]
pub(crate) struct ExternalChannelChatRuntimeAttempt {
    pub(crate) env_prefix: String,
    pub(crate) label: String,
    pub(crate) runtime: LlmRuntimeSelection,
    pub(crate) profile: Option<ModelProviderProfile>,
    pub(crate) lane_limits: Option<ModelGatewayLaneLimits>,
}

pub(crate) fn external_channel_chat_runtime_attempts(
    primary: LlmRuntimeSelection,
) -> Vec<ExternalChannelChatRuntimeAttempt> {
    let mut attempts = vec![ExternalChannelChatRuntimeAttempt {
        env_prefix: "ASSISTANT_RUN".to_string(),
        label: "primary".to_string(),
        runtime: primary.clone(),
        profile: None,
        lane_limits: None,
    }];

    let mut has_distinct_fallback = false;
    if let Some(fallback) = external_channel_fallback_runtime_selection_from_env() {
        if !external_channel_same_runtime_selection(&primary, &fallback) {
            has_distinct_fallback = true;
            attempts.push(ExternalChannelChatRuntimeAttempt {
                env_prefix: "ASSISTANT_RUN_FALLBACK".to_string(),
                label: "fallback".to_string(),
                runtime: fallback,
                profile: None,
                lane_limits: None,
            });
        }
    }

    if !has_distinct_fallback && external_channel_same_runtime_retry_enabled() {
        attempts.push(ExternalChannelChatRuntimeAttempt {
            env_prefix: "ASSISTANT_RUN".to_string(),
            label: "primary_retry".to_string(),
            runtime: primary,
            profile: None,
            lane_limits: None,
        });
    }

    attempts
}

pub(crate) fn external_channel_chat_attempt_from_db_profile(
    profile: ModelGatewayProfile,
) -> ExternalChannelChatRuntimeAttempt {
    let lane_limits = ModelGatewayLaneLimits::default();
    let provider_profile = model_gateway_provider_profile_from_record(profile);
    external_channel_chat_attempt_from_profile(provider_profile, Some(lane_limits))
}

pub(crate) fn external_channel_chat_attempt_from_profile(
    profile: ModelProviderProfile,
    lane_limits: Option<ModelGatewayLaneLimits>,
) -> ExternalChannelChatRuntimeAttempt {
    let env_prefix = model_gateway_profile_env_prefix(&profile.profile_id);
    let runtime = profile.runtime_selection_for_lane(MODEL_LANE_ASSISTANT_CHAT);
    ExternalChannelChatRuntimeAttempt {
        label: format!("profile:{}", profile.profile_id),
        env_prefix,
        runtime,
        profile: Some(profile),
        lane_limits,
    }
}

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

fn external_channel_same_runtime_retry_enabled() -> bool {
    env_flag("EXTERNAL_CHANNEL_DIRECT_REPLY_SAME_RUNTIME_RETRY", true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::TenantId;
    use std::sync::{Mutex, OnceLock};
    use uuid::Uuid;

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env lock")
    }

    fn clear_attempt_env() {
        std::env::remove_var("ASSISTANT_RUN_FALLBACK_RUNTIME_MODE");
        std::env::remove_var("ASSISTANT_RUN_FALLBACK_RUNTIME_PROVIDER");
        std::env::remove_var("ASSISTANT_RUN_FALLBACK_RUNTIME_MODEL");
        std::env::remove_var("EXTERNAL_CHANNEL_DIRECT_REPLY_SAME_RUNTIME_RETRY");
    }

    fn primary_runtime() -> LlmRuntimeSelection {
        LlmRuntimeSelection {
            mode: "provider".to_string(),
            provider: "openai".to_string(),
            model: "primary-v1".to_string(),
            lane: MODEL_LANE_ASSISTANT_CHAT.to_string(),
        }
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

    #[test]
    fn chat_runtime_attempts_add_primary_retry_without_distinct_fallback() {
        let _guard = env_lock();
        clear_attempt_env();

        let attempts = external_channel_chat_runtime_attempts(primary_runtime());

        assert_eq!(
            attempts
                .iter()
                .map(|attempt| attempt.label.as_str())
                .collect::<Vec<_>>(),
            vec!["primary", "primary_retry"]
        );
        assert_eq!(attempts[0].env_prefix, "ASSISTANT_RUN");
        assert_eq!(attempts[1].env_prefix, "ASSISTANT_RUN");
        assert!(attempts.iter().all(|attempt| attempt.profile.is_none()));

        clear_attempt_env();
    }

    #[test]
    fn chat_runtime_attempts_use_distinct_fallback_without_primary_retry() {
        let _guard = env_lock();
        clear_attempt_env();
        std::env::set_var("ASSISTANT_RUN_FALLBACK_RUNTIME_MODE", "provider");
        std::env::set_var("ASSISTANT_RUN_FALLBACK_RUNTIME_PROVIDER", "minimax");
        std::env::set_var("ASSISTANT_RUN_FALLBACK_RUNTIME_MODEL", "fallback-v1");

        let attempts = external_channel_chat_runtime_attempts(primary_runtime());

        assert_eq!(
            attempts
                .iter()
                .map(|attempt| attempt.label.as_str())
                .collect::<Vec<_>>(),
            vec!["primary", "fallback"]
        );
        assert_eq!(attempts[1].env_prefix, "ASSISTANT_RUN_FALLBACK");
        assert_eq!(attempts[1].runtime.provider, "minimax");
        assert_eq!(attempts[1].runtime.model, "fallback-v1");

        clear_attempt_env();
    }

    #[test]
    fn chat_runtime_attempt_from_db_profile_preserves_profile_runtime_metadata() {
        let now = Utc::now();
        let attempt = external_channel_chat_attempt_from_db_profile(ModelGatewayProfile {
            id: Uuid::new_v4(),
            tenant_id: TenantId::new(),
            profile_id: "pool-main".to_string(),
            display_name: "Pool Main".to_string(),
            lane: MODEL_LANE_ASSISTANT_CHAT.to_string(),
            provider_id: "scripted".to_string(),
            model_id: "pool-model-v1".to_string(),
            base_url: None,
            api_path: None,
            wire_api: "chat_completions".to_string(),
            auth_mode: "env".to_string(),
            auth_env_key_name: Some("POOL_MAIN_KEY".to_string()),
            recommended_preset: None,
            max_concurrency: Some(20),
            rpm_limit: Some(60),
            tpm_limit: Some(30_000),
            timeout_ms: Some(15_000),
            priority: 10,
            enabled: true,
            capabilities: serde_json::json!(["chat"]),
            created_at: now,
            updated_at: now,
        });

        assert_eq!(attempt.label, "profile:pool-main");
        assert_eq!(attempt.env_prefix, "LLM_GATEWAY_PROFILE_POOL_MAIN");
        assert_eq!(attempt.runtime.lane, MODEL_LANE_ASSISTANT_CHAT);
        assert_eq!(attempt.runtime.provider, "scripted");
        assert_eq!(attempt.runtime.model, "pool-model-v1");
        assert!(attempt.lane_limits.is_some());
        let profile = attempt.profile.expect("profile should be attached");
        assert_eq!(profile.profile_id, "pool-main");
        assert_eq!(profile.rate_limit.concurrent_requests, Some(20));
        assert_eq!(profile.rate_limit.requests_per_minute, Some(60));
        assert_eq!(profile.rate_limit.tokens_per_minute, Some(30_000));
    }
}
