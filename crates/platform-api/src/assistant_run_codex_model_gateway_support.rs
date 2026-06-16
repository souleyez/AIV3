use llm_gateway::{LlmRuntimeSelection, ModelProviderProfile};
use serde_json::{json, Value};

pub(crate) fn assistant_run_codex_model_gateway_snapshot(runtime: &LlmRuntimeSelection) -> Value {
    let profile_env_prefix = std::env::var("ASSISTANT_RUN_CODEX_MODEL_PROFILE_ENV")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let profile = profile_env_prefix
        .as_deref()
        .and_then(|prefix| ModelProviderProfile::from_env(prefix).ok())
        .map(|profile| profile.public_manifest());
    let profile_status = match (&profile_env_prefix, &profile) {
        (Some(_), Some(_)) => "profile_loaded",
        (Some(_), None) => "profile_unavailable",
        (None, _) => "runtime_selection_only",
    };
    let profile_source = if profile.is_some() {
        "model_provider_profile_env"
    } else {
        "llm_gateway_runtime_selection"
    };

    json!({
        "lane": runtime.lane.as_str(),
        "selected_model": {
            "mode": runtime.mode.as_str(),
            "provider": runtime.provider.as_str(),
            "model": runtime.model.as_str(),
        },
        "profile_source": profile_source,
        "profile_status": profile_status,
        "profile_env_prefix": profile_env_prefix,
        "profile": profile.unwrap_or_else(|| assistant_run_codex_runtime_selection_profile(runtime)),
        "safety": {
            "secrets_redacted": true,
            "raw_provider_payloads_allowed": false,
            "codex_real_execution_allowed_on_this_host": false,
            "v3_validates_all_actions": true,
        }
    })
}

pub(crate) fn assistant_run_codex_runtime_selection_profile(
    runtime: &LlmRuntimeSelection,
) -> Value {
    json!({
        "profile_id": format!("{}:{}", runtime.provider, runtime.model),
        "provider_id": runtime.provider.as_str(),
        "model_id": runtime.model.as_str(),
        "wire_api": if runtime.mode == "placeholder" {
            "placeholder"
        } else {
            "runtime_selection"
        },
        "auth": {
            "env_key_name": Value::Null,
            "configured": runtime.mode == "placeholder",
        },
        "capabilities": [],
        "redaction": {
            "redact_provider_errors": true,
            "redact_request_payloads": true,
            "redact_response_payloads": true,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime(mode: &str, provider: &str, model: &str) -> LlmRuntimeSelection {
        LlmRuntimeSelection {
            mode: mode.to_string(),
            provider: provider.to_string(),
            model: model.to_string(),
            lane: "codex_conversation".to_string(),
        }
    }

    #[test]
    fn runtime_selection_profile_marks_provider_runtime_without_auth() {
        let profile =
            assistant_run_codex_runtime_selection_profile(&runtime("provider", "minimax", "m3"));

        assert_eq!(profile["profile_id"], "minimax:m3");
        assert_eq!(profile["provider_id"], "minimax");
        assert_eq!(profile["model_id"], "m3");
        assert_eq!(profile["wire_api"], "runtime_selection");
        assert_eq!(profile["auth"]["configured"], false);
        assert_eq!(profile["auth"]["env_key_name"], Value::Null);
        assert_eq!(profile["redaction"]["redact_provider_errors"], true);
        assert_eq!(profile["redaction"]["redact_request_payloads"], true);
        assert_eq!(profile["redaction"]["redact_response_payloads"], true);
    }

    #[test]
    fn runtime_selection_profile_marks_placeholder_runtime_as_configured_stub() {
        let profile = assistant_run_codex_runtime_selection_profile(&runtime(
            "placeholder",
            "placeholder",
            "codex-shadow-placeholder",
        ));

        assert_eq!(
            profile["profile_id"],
            "placeholder:codex-shadow-placeholder"
        );
        assert_eq!(profile["wire_api"], "placeholder");
        assert_eq!(profile["auth"]["configured"], true);
    }
}
