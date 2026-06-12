use super::MODEL_GATEWAY_DEFAULT_LANE;
use crate::{env_csv_contains, env_flag, ApiError};
use contracts::{
    ModelGatewayPresetView, ModelGatewayProfileCreateRequest, ModelGatewayProfileUpdateRequest,
    ModelGatewayProfileView,
};
use domain_model::User;
use serde_json::json;
use storage::{ModelGatewayProfile, ModelGatewayProfileUpdate, NewModelGatewayProfile};
use uuid::Uuid;
pub(crate) fn model_gateway_presets() -> Vec<ModelGatewayPresetView> {
    vec![
        ModelGatewayPresetView {
            preset_id: "rightcode/gpt-5.5".to_string(),
            display_name: "Right Code GPT-5.5 默认通道".to_string(),
            provider_id: "rightcode".to_string(),
            model_id: "gpt-5.5".to_string(),
            lane: MODEL_GATEWAY_DEFAULT_LANE.to_string(),
            wire_api: "chat_completions".to_string(),
            base_url: Some("https://right.codes/codex/v1".to_string()),
            api_path: Some("/chat/completions".to_string()),
            max_concurrency: 20,
            rpm_limit: Some(120),
            tpm_limit: None,
            timeout_ms: 20_000,
            priority: 100,
            capabilities: json!(["chat", "reasoning", "json_mode"]),
        },
        ModelGatewayPresetView {
            preset_id: "openclaw/default".to_string(),
            display_name: "OpenClaw 默认模型".to_string(),
            provider_id: "openclaw".to_string(),
            model_id: "default".to_string(),
            lane: MODEL_GATEWAY_DEFAULT_LANE.to_string(),
            wire_api: "openai-compatible".to_string(),
            base_url: None,
            api_path: Some("/v1/chat/completions".to_string()),
            max_concurrency: 15,
            rpm_limit: None,
            tpm_limit: None,
            timeout_ms: 30_000,
            priority: 100,
            capabilities: json!({
                "chat": true,
                "streaming": true,
                "json_mode": true
            }),
        },
        ModelGatewayPresetView {
            preset_id: "minimax/MiniMax-M2.7".to_string(),
            display_name: "MiniMax M2.7 快速通道".to_string(),
            provider_id: "minimax".to_string(),
            model_id: "MiniMax-M2.7".to_string(),
            lane: MODEL_GATEWAY_DEFAULT_LANE.to_string(),
            wire_api: "openai-compatible".to_string(),
            base_url: None,
            api_path: Some("/v1/chat/completions".to_string()),
            max_concurrency: 5,
            rpm_limit: None,
            tpm_limit: None,
            timeout_ms: 30_000,
            priority: 80,
            capabilities: json!({
                "chat": true,
                "streaming": true,
                "json_mode": true,
                "long_context": true
            }),
        },
        ModelGatewayPresetView {
            preset_id: "openai-compatible/chat".to_string(),
            display_name: "OpenAI 兼容 Chat".to_string(),
            provider_id: "openai-compatible".to_string(),
            model_id: "chat".to_string(),
            lane: MODEL_GATEWAY_DEFAULT_LANE.to_string(),
            wire_api: "openai-compatible".to_string(),
            base_url: None,
            api_path: Some("/v1/chat/completions".to_string()),
            max_concurrency: 5,
            rpm_limit: None,
            tpm_limit: None,
            timeout_ms: 30_000,
            priority: 60,
            capabilities: json!({
                "chat": true,
                "streaming": true,
                "json_mode": true
            }),
        },
        ModelGatewayPresetView {
            preset_id: "custom".to_string(),
            display_name: "自定义 OpenAI 兼容端点".to_string(),
            provider_id: "custom".to_string(),
            model_id: "custom".to_string(),
            lane: MODEL_GATEWAY_DEFAULT_LANE.to_string(),
            wire_api: "openai-compatible".to_string(),
            base_url: None,
            api_path: Some("/v1/chat/completions".to_string()),
            max_concurrency: 2,
            rpm_limit: None,
            tpm_limit: None,
            timeout_ms: 30_000,
            priority: 10,
            capabilities: json!({
                "chat": true
            }),
        },
    ]
}

pub(crate) fn model_gateway_preset_by_id(preset_id: &str) -> Option<ModelGatewayPresetView> {
    model_gateway_presets()
        .into_iter()
        .find(|preset| preset.preset_id == preset_id)
}

pub(crate) fn model_gateway_profile_requires_configured_auth_env(
    profile: &ModelGatewayProfile,
) -> bool {
    let auth_mode = profile.auth_mode.trim().to_ascii_lowercase();
    if auth_mode.is_empty() || auth_mode == "none" {
        return false;
    }
    auth_mode == "env_key" || auth_mode.ends_with("_env") || auth_mode.contains("env_key")
}

pub(crate) fn ensure_model_gateway_operator(user: &User) -> std::result::Result<(), ApiError> {
    if model_gateway_operator_allowed(user) {
        return Ok(());
    }
    Err(ApiError::forbidden(
        "model_gateway_operator_required",
        "当前账号没有模型池运维权限".to_string(),
    ))
}

pub(crate) fn model_gateway_operator_allowed(user: &User) -> bool {
    env_flag("MODEL_GATEWAY_OPERATOR_ALLOW_ANY_SIGNED_IN", false)
        || model_gateway_operator_email_allowed(&user.email)
        || model_gateway_operator_role_allowed(user)
}

fn model_gateway_operator_email_allowed(email: &str) -> bool {
    env_csv_contains("MODEL_GATEWAY_OPERATOR_EMAILS", email)
        || env_csv_contains("MODEL_GATEWAY_OPERATOR_EMAIL_ALLOWLIST", email)
}

fn model_gateway_operator_role_allowed(user: &User) -> bool {
    user.roles.iter().any(|role| {
        model_gateway_builtin_operator_role(role)
            || env_csv_contains("MODEL_GATEWAY_OPERATOR_ROLES", role)
    })
}

fn model_gateway_builtin_operator_role(role: &str) -> bool {
    matches!(
        role.trim().to_ascii_lowercase().as_str(),
        "admin" | "operator" | "model_gateway_operator" | "model-gateway-operator"
    )
}

pub(crate) fn model_gateway_new_profile_from_request(
    request: ModelGatewayProfileCreateRequest,
) -> std::result::Result<NewModelGatewayProfile, ApiError> {
    let preset = request
        .recommended_preset
        .as_deref()
        .and_then(model_gateway_preset_by_id);
    let profile_id = validate_model_gateway_profile_id(&request.profile_id)?;
    let display_name = validate_model_gateway_required_text("display_name", &request.display_name)?;
    let lane = model_gateway_optional_text(request.lane)
        .or_else(|| preset.as_ref().map(|preset| preset.lane.clone()))
        .unwrap_or_else(|| MODEL_GATEWAY_DEFAULT_LANE.to_string());
    let provider_id = model_gateway_optional_text(request.provider_id)
        .or_else(|| preset.as_ref().map(|preset| preset.provider_id.clone()))
        .unwrap_or_else(|| "custom".to_string());
    let model_id = model_gateway_optional_text(request.model_id)
        .or_else(|| preset.as_ref().map(|preset| preset.model_id.clone()))
        .unwrap_or_else(|| "custom".to_string());
    let wire_api = model_gateway_optional_text(request.wire_api)
        .or_else(|| preset.as_ref().map(|preset| preset.wire_api.clone()))
        .unwrap_or_else(|| "openai-compatible".to_string());
    let capabilities = request
        .capabilities
        .or_else(|| preset.as_ref().map(|preset| preset.capabilities.clone()))
        .unwrap_or_else(|| json!({}));

    validate_model_gateway_numeric("max_concurrency", request.max_concurrency)?;
    validate_model_gateway_numeric("rpm_limit", request.rpm_limit)?;
    validate_model_gateway_numeric("tpm_limit", request.tpm_limit)?;
    validate_model_gateway_numeric("timeout_ms", request.timeout_ms)?;

    Ok(NewModelGatewayProfile {
        id: Uuid::new_v4(),
        profile_id,
        display_name,
        lane,
        provider_id,
        model_id,
        base_url: model_gateway_optional_text(request.base_url),
        api_path: model_gateway_optional_text(request.api_path)
            .or_else(|| preset.as_ref().and_then(|preset| preset.api_path.clone())),
        wire_api,
        auth_mode: model_gateway_optional_text(request.auth_mode)
            .unwrap_or_else(|| "env_key".to_string()),
        auth_env_key_name: model_gateway_optional_text(request.auth_env_key_name),
        recommended_preset: model_gateway_optional_text(request.recommended_preset)
            .or_else(|| preset.as_ref().map(|preset| preset.preset_id.clone())),
        max_concurrency: request
            .max_concurrency
            .or_else(|| preset.as_ref().map(|preset| preset.max_concurrency)),
        rpm_limit: request
            .rpm_limit
            .or_else(|| preset.as_ref().and_then(|preset| preset.rpm_limit)),
        tpm_limit: request
            .tpm_limit
            .or_else(|| preset.as_ref().and_then(|preset| preset.tpm_limit)),
        timeout_ms: request
            .timeout_ms
            .or_else(|| preset.as_ref().map(|preset| preset.timeout_ms)),
        priority: request
            .priority
            .or_else(|| preset.as_ref().map(|preset| preset.priority))
            .unwrap_or(100),
        enabled: request.enabled.unwrap_or(true),
        capabilities,
    })
}

pub(crate) fn model_gateway_update_from_request(
    request: ModelGatewayProfileUpdateRequest,
) -> std::result::Result<ModelGatewayProfileUpdate, ApiError> {
    validate_model_gateway_numeric("max_concurrency", request.max_concurrency)?;
    validate_model_gateway_numeric("rpm_limit", request.rpm_limit)?;
    validate_model_gateway_numeric("tpm_limit", request.tpm_limit)?;
    validate_model_gateway_numeric("timeout_ms", request.timeout_ms)?;

    Ok(ModelGatewayProfileUpdate {
        display_name: request
            .display_name
            .map(|value| validate_model_gateway_required_text("display_name", &value))
            .transpose()?,
        lane: request
            .lane
            .map(|value| validate_model_gateway_required_text("lane", &value))
            .transpose()?,
        provider_id: request
            .provider_id
            .map(|value| validate_model_gateway_required_text("provider_id", &value))
            .transpose()?,
        model_id: request
            .model_id
            .map(|value| validate_model_gateway_required_text("model_id", &value))
            .transpose()?,
        base_url: request.base_url.map(|value| value.trim().to_string()),
        api_path: request.api_path.map(|value| value.trim().to_string()),
        wire_api: request
            .wire_api
            .map(|value| validate_model_gateway_required_text("wire_api", &value))
            .transpose()?,
        auth_mode: request
            .auth_mode
            .map(|value| validate_model_gateway_required_text("auth_mode", &value))
            .transpose()?,
        auth_env_key_name: request
            .auth_env_key_name
            .map(|value| value.trim().to_string()),
        recommended_preset: request
            .recommended_preset
            .map(|value| value.trim().to_string()),
        max_concurrency: request.max_concurrency,
        rpm_limit: request.rpm_limit,
        tpm_limit: request.tpm_limit,
        timeout_ms: request.timeout_ms,
        priority: request.priority,
        enabled: request.enabled,
        capabilities: request.capabilities,
    })
}

pub(crate) fn model_gateway_profile_view(profile: ModelGatewayProfile) -> ModelGatewayProfileView {
    let has_secret = profile
        .auth_env_key_name
        .as_deref()
        .map(model_gateway_auth_env_is_configured)
        .unwrap_or(false);

    ModelGatewayProfileView {
        id: profile.id.to_string(),
        profile_id: profile.profile_id,
        display_name: profile.display_name,
        lane: profile.lane,
        provider_id: profile.provider_id,
        model_id: profile.model_id,
        base_url: profile.base_url.map(|url| redact_model_gateway_url(&url)),
        api_path: profile.api_path,
        wire_api: profile.wire_api,
        auth_mode: profile.auth_mode,
        auth_env_key_name: profile.auth_env_key_name,
        has_secret,
        recommended_preset: profile.recommended_preset,
        max_concurrency: profile.max_concurrency,
        rpm_limit: profile.rpm_limit,
        tpm_limit: profile.tpm_limit,
        timeout_ms: profile.timeout_ms,
        priority: profile.priority,
        enabled: profile.enabled,
        capabilities: profile.capabilities,
        created_at: profile.created_at,
        updated_at: profile.updated_at,
    }
}

pub(crate) fn model_gateway_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn validate_model_gateway_profile_id(
    value: &str,
) -> std::result::Result<String, ApiError> {
    let normalized = value.trim();
    if normalized.is_empty() || normalized.len() > 128 {
        return Err(ApiError::bad_request(
            "invalid_model_gateway_profile_id",
            "profile_id 不能为空，且长度不能超过 128".to_string(),
        ));
    }
    if !normalized
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        return Err(ApiError::bad_request(
            "invalid_model_gateway_profile_id",
            "profile_id 只能包含字母、数字、短横线、下划线或点号".to_string(),
        ));
    }

    Ok(normalized.to_string())
}

pub(crate) fn validate_model_gateway_required_text(
    field: &'static str,
    value: &str,
) -> std::result::Result<String, ApiError> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(ApiError::bad_request(
            "validation_error",
            format!("{field} is required"),
        ));
    }
    Ok(normalized.to_string())
}

pub(crate) fn validate_model_gateway_numeric(
    field: &'static str,
    value: Option<i32>,
) -> std::result::Result<(), ApiError> {
    if value.is_some_and(|value| value <= 0) {
        return Err(ApiError::bad_request(
            "validation_error",
            format!("{field} must be greater than 0"),
        ));
    }
    Ok(())
}

pub(crate) fn model_gateway_auth_env_is_configured(env_key_name: &str) -> bool {
    std::env::var(env_key_name)
        .ok()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

pub(crate) fn redact_model_gateway_url(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.to_string();
    };
    let Some(at_index) = rest.find('@') else {
        return url.to_string();
    };
    let slash_index = rest.find('/').unwrap_or(rest.len());
    if at_index > slash_index {
        return url.to_string();
    }
    format!("{scheme}://[redacted]@{}", &rest[at_index + 1..])
}

pub(crate) fn model_gateway_profile_not_found(profile_id: &str) -> ApiError {
    ApiError::not_found(
        "model_gateway_profile_not_found",
        format!("model gateway profile {profile_id} was not found"),
    )
}
