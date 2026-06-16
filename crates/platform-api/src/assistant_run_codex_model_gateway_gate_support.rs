use domain_model::AssistantRunEvent;
use serde_json::{json, Value};

use crate::assistant_run_codex_model_gateway_diagnostics_summary;

pub(crate) fn assistant_run_codex_model_gateway_gate_summary(
    latest: Option<&AssistantRunEvent>,
) -> Value {
    let Some(event) = latest else {
        return json!({
            "status": "not_run",
            "ready_for_promotion_review": false,
            "profile_available": false,
            "auth_configured": false,
            "codex_surface_supported": false,
            "next_step": "run_codex_shadow_diagnostics_with_model_gateway_snapshot",
        });
    };
    let model_gateway = event.payload.get("model_gateway").unwrap_or(&Value::Null);
    let summary = assistant_run_codex_model_gateway_diagnostics_summary(Some(model_gateway));
    let profile_available = summary
        .get("profile_available")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let auth_configured = summary
        .get("auth_configured")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let codex_surface = summary.get("codex_surface").unwrap_or(&Value::Null);
    let wire_api = summary
        .get("wire_api")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let codex_compatible = codex_surface
        .get("codex_compatible")
        .and_then(Value::as_bool)
        .unwrap_or_else(|| {
            matches!(
                wire_api,
                "codex_compatible_shim" | "codex-compatible-shim" | "codex_shim" | "provider_shim"
            )
        });
    let json_actions_supported = codex_surface
        .get("json_actions_supported")
        .and_then(Value::as_bool)
        .unwrap_or_else(|| matches!(wire_api, "responses" | "codex_compatible_shim"));
    let tool_calls_supported = codex_surface
        .get("tool_calls_supported")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let codex_surface_supported = codex_compatible || json_actions_supported;
    let status = if !profile_available {
        "profile_missing"
    } else if !auth_configured {
        "auth_not_configured"
    } else if !codex_surface_supported {
        "unsupported_codex_surface"
    } else {
        "ready"
    };
    let next_step = match status {
        "ready" => "continue_shadow_and_host_gate_review",
        "profile_missing" => "configure_codex_conversation_model_profile",
        "auth_not_configured" => "configure_codex_model_profile_auth",
        "unsupported_codex_surface" => "select_codex_compatible_or_json_action_profile",
        _ => "inspect_model_gateway_diagnostics",
    };

    json!({
        "status": status,
        "ready_for_promotion_review": status == "ready",
        "profile_available": profile_available,
        "profile_source": summary.get("profile_source").cloned().unwrap_or(Value::Null),
        "profile_status": summary.get("profile_status").cloned().unwrap_or(Value::Null),
        "profile_id": summary.get("profile_id").cloned().unwrap_or(Value::Null),
        "provider_id": summary.get("provider_id").cloned().unwrap_or(Value::Null),
        "model_id": summary.get("model_id").cloned().unwrap_or(Value::Null),
        "wire_api": summary.get("wire_api").cloned().unwrap_or(Value::Null),
        "auth_configured": auth_configured,
        "capabilities": summary
            .get("capabilities")
            .cloned()
            .unwrap_or_else(|| json!([])),
        "capability_manifest": summary
            .get("capability_manifest")
            .cloned()
            .unwrap_or(Value::Null),
        "codex_surface": {
            "codex_compatible": codex_compatible,
            "json_actions_supported": json_actions_supported,
            "tool_calls_supported": tool_calls_supported,
            "real_execution_block_reason": codex_surface
                .get("real_execution_block_reason")
                .cloned()
                .unwrap_or(Value::Null),
        },
        "direct_execution_authoritative": true,
        "local_execution_allowed": false,
        "codex_mutation_allowed": false,
        "queue_allowed": false,
        "next_step": next_step,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{AssistantRunEventId, AssistantRunId, TenantId};

    fn event(model_gateway: Value) -> AssistantRunEvent {
        AssistantRunEvent {
            id: AssistantRunEventId::new(),
            tenant_id: TenantId::new(),
            run_id: AssistantRunId::new(),
            sequence_no: 1,
            event_name: "assistant_run.codex_executor_diagnostic".to_string(),
            payload: json!({ "model_gateway": model_gateway }),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn model_gateway_gate_marks_ready_auth_missing_and_not_run() {
        let ready_event = event(json!({
            "profile_source": "model_provider_profile_env",
            "profile_status": "profile_loaded",
            "profile": {
                "profile_id": "codex-profile",
                "provider_id": "minimax",
                "model_id": "m3",
                "wire_api": "codex_compatible_shim",
                "auth": {"configured": true},
                "capabilities": ["json_actions"]
            },
            "codex_surface": {
                "codex_compatible": true,
                "json_actions_supported": true,
                "tool_calls_supported": false
            }
        }));
        let ready = assistant_run_codex_model_gateway_gate_summary(Some(&ready_event));

        assert_eq!(ready["status"], json!("ready"));
        assert_eq!(ready["ready_for_promotion_review"], json!(true));
        assert_eq!(ready["profile_available"], json!(true));
        assert_eq!(ready["auth_configured"], json!(true));
        assert_eq!(
            ready["next_step"],
            json!("continue_shadow_and_host_gate_review")
        );
        assert_eq!(ready["codex_mutation_allowed"], json!(false));
        assert_eq!(ready["queue_allowed"], json!(false));

        let auth_missing_event = event(json!({
            "profile_source": "model_provider_profile_env",
            "profile_status": "profile_loaded",
            "profile": {
                "profile_id": "codex-profile",
                "provider_id": "minimax",
                "model_id": "m3",
                "wire_api": "codex_compatible_shim",
                "auth": {"configured": false}
            }
        }));
        let auth_missing =
            assistant_run_codex_model_gateway_gate_summary(Some(&auth_missing_event));

        assert_eq!(auth_missing["status"], json!("auth_not_configured"));
        assert_eq!(auth_missing["ready_for_promotion_review"], json!(false));
        assert_eq!(
            auth_missing["next_step"],
            json!("configure_codex_model_profile_auth")
        );

        let not_run = assistant_run_codex_model_gateway_gate_summary(None);
        assert_eq!(not_run["status"], json!("not_run"));
        assert_eq!(not_run["profile_available"], json!(false));
        assert_eq!(
            not_run["next_step"],
            json!("run_codex_shadow_diagnostics_with_model_gateway_snapshot")
        );
    }
}
