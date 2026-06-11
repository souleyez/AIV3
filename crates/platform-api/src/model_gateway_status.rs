use contracts::{
    ModelGatewayRuntimeDatabasePoolStatusView, ModelGatewayRuntimeWorkerPoolStatusView,
};
use llm_gateway::ModelProviderProfile;
use storage::{configured_database_max_connections, ModelGatewayProfile};
#[derive(Clone, Debug)]
pub(crate) struct ModelGatewayStatusProviderSource {
    pub(crate) profile_id: String,
    pub(crate) display_name: String,
    pub(crate) lane: String,
    pub(crate) provider_id: String,
    pub(crate) model_id: String,
    pub(crate) wire_api: String,
    pub(crate) source: String,
    pub(crate) priority: i32,
    pub(crate) enabled: bool,
    pub(crate) max_concurrency: Option<u32>,
    pub(crate) rpm_limit: Option<i32>,
    pub(crate) tpm_limit: Option<i32>,
}

pub(crate) fn model_gateway_status_source_from_record(
    profile: ModelGatewayProfile,
) -> ModelGatewayStatusProviderSource {
    ModelGatewayStatusProviderSource {
        profile_id: profile.profile_id,
        display_name: profile.display_name,
        lane: profile.lane,
        provider_id: profile.provider_id,
        model_id: profile.model_id,
        wire_api: profile.wire_api,
        source: "database".to_string(),
        priority: profile.priority,
        enabled: profile.enabled,
        max_concurrency: model_gateway_i32_to_u32(profile.max_concurrency),
        rpm_limit: profile.rpm_limit,
        tpm_limit: profile.tpm_limit,
    }
}

pub(crate) fn model_gateway_status_source_from_env_profile(
    lane: &str,
    profile: ModelProviderProfile,
) -> ModelGatewayStatusProviderSource {
    ModelGatewayStatusProviderSource {
        profile_id: profile.profile_id.clone(),
        display_name: profile.profile_id,
        lane: lane.to_string(),
        provider_id: profile.provider_id,
        model_id: profile.model_id,
        wire_api: profile.wire_api.as_str().to_string(),
        source: "env".to_string(),
        priority: profile.priority,
        enabled: true,
        max_concurrency: profile.rate_limit.concurrent_requests,
        rpm_limit: model_gateway_u32_to_i32(profile.rate_limit.requests_per_minute),
        tpm_limit: model_gateway_u32_to_i32(profile.rate_limit.tokens_per_minute),
    }
}

pub(crate) fn model_gateway_i32_to_u32(value: Option<i32>) -> Option<u32> {
    value
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)
}

pub(crate) fn model_gateway_u32_to_i32(value: Option<u32>) -> Option<i32> {
    value.and_then(|value| i32::try_from(value).ok())
}

pub(crate) fn model_gateway_percent(numerator: i64, denominator: i64) -> Option<u32> {
    if denominator <= 0 || numerator < 0 {
        return None;
    }
    Some(((numerator * 100) / denominator).clamp(0, 100) as u32)
}

pub(crate) fn model_gateway_lane_active_source(
    enabled_database_profile_count: usize,
    env_profile_count: usize,
) -> &'static str {
    if enabled_database_profile_count > 0 {
        return "database";
    }
    if env_profile_count > 0 {
        return "env";
    }
    "none"
}

pub(crate) fn model_gateway_lane_active_profile_count(
    active_source: &str,
    enabled_database_profile_count: usize,
    env_profile_count: usize,
) -> usize {
    match active_source {
        "database" => enabled_database_profile_count,
        "env" => env_profile_count,
        _ => 0,
    }
}

pub(crate) fn model_gateway_provider_source_eligible(
    source: &ModelGatewayStatusProviderSource,
    active_source: &str,
) -> bool {
    match source.source.as_str() {
        "database" => source.enabled && active_source == "database",
        "env" => active_source == "env",
        _ => false,
    }
}

pub(crate) fn model_gateway_provider_dormant_reason(
    source: &ModelGatewayStatusProviderSource,
    active_source: &str,
) -> Option<&'static str> {
    if model_gateway_provider_source_eligible(source, active_source) {
        return None;
    }
    match source.source.as_str() {
        "database" if !source.enabled => Some("profile_disabled"),
        "database" => Some("lane_uses_env_profiles"),
        "env" if active_source == "database" => Some("database_profiles_active"),
        "env" => Some("lane_has_no_active_source"),
        _ => Some("unknown_source"),
    }
}

pub(crate) fn model_gateway_runtime_worker_pool_status(
    service: &str,
    label: &str,
    env_keys: &[&str],
    default_concurrency: usize,
    max_concurrency: usize,
) -> ModelGatewayRuntimeWorkerPoolStatusView {
    ModelGatewayRuntimeWorkerPoolStatusView {
        service: service.to_string(),
        label: label.to_string(),
        concurrency: configured_worker_pool_concurrency(
            env_keys,
            default_concurrency,
            max_concurrency,
        ),
        default_concurrency,
        max_concurrency,
        env_keys: env_keys.iter().map(|key| (*key).to_string()).collect(),
    }
}

pub(crate) fn model_gateway_runtime_database_pool_status(
    service: &str,
    label: &str,
    env_key: &str,
) -> ModelGatewayRuntimeDatabasePoolStatusView {
    ModelGatewayRuntimeDatabasePoolStatusView {
        service: service.to_string(),
        label: label.to_string(),
        max_connections: configured_database_max_connections(env_key),
        env_key: env_key.to_string(),
        global_env_key: "PLATFORM_DATABASE_MAX_CONNECTIONS".to_string(),
    }
}

fn configured_worker_pool_concurrency(
    env_keys: &[&str],
    default_concurrency: usize,
    max_concurrency: usize,
) -> usize {
    let values = env_keys
        .iter()
        .map(|key| std::env::var(key).ok())
        .collect::<Vec<_>>();
    parse_worker_pool_concurrency(
        values.iter().map(|value| value.as_deref()),
        default_concurrency,
        max_concurrency,
    )
}

pub(crate) fn parse_worker_pool_concurrency<'a>(
    values: impl IntoIterator<Item = Option<&'a str>>,
    default_concurrency: usize,
    max_concurrency: usize,
) -> usize {
    values
        .into_iter()
        .flatten()
        .find_map(|value| {
            value
                .trim()
                .parse::<usize>()
                .ok()
                .filter(|value| *value > 0)
        })
        .unwrap_or(default_concurrency)
        .min(max_concurrency)
}
