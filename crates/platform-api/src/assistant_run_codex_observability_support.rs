use assistant_runtime::CodexConversationExecutorOutput;
use serde_json::{json, Value};

pub(crate) fn assistant_run_codex_provider_shim_observability_from_output(
    output: &CodexConversationExecutorOutput,
) -> Value {
    let wire_api = output
        .model_gateway
        .get("wire_api")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let is_provider_shim = matches!(
        wire_api,
        "codex_compatible_shim" | "codex-compatible-shim" | "codex_shim" | "provider_shim"
    );
    if !is_provider_shim {
        return Value::Null;
    }

    let selected_model = output
        .model_gateway
        .get("selected_model")
        .filter(|value| value.is_object())
        .unwrap_or(&Value::Null);
    let provider_id = selected_model
        .get("provider")
        .cloned()
        .unwrap_or(Value::Null);
    let model_id = selected_model.get("model").cloned().unwrap_or(Value::Null);
    let auth_configured = output
        .model_gateway
        .get("auth_configured")
        .cloned()
        .unwrap_or(Value::Bool(false));
    let profile_id = output
        .model_gateway
        .get("profile_id")
        .cloned()
        .unwrap_or(Value::Null);
    let capabilities = output
        .model_gateway
        .get("capabilities")
        .cloned()
        .unwrap_or_else(|| json!([]));
    let capability_manifest = output
        .model_gateway
        .get("capability_manifest")
        .cloned()
        .unwrap_or(Value::Null);

    json!({
        "schema_version": 1,
        "source": "v3_codex_event_synthetic",
        "health": {
            "status": "unknown",
            "process_reachable": false,
            "upstream_reachable": false,
            "checked_at": Value::Null,
        },
        "profile": {
            "profile_id": profile_id,
            "provider_id": provider_id,
            "model_id": model_id,
            "wire_api": wire_api,
            "endpoint_scope": "v3_server_profile",
            "base_url_configured": false,
            "api_path": Value::Null,
            "auth_configured": auth_configured,
            "timeout_ms": Value::Null,
            "capabilities": capabilities,
            "capability_manifest": capability_manifest,
            "rate_limit": {
                "requests_per_minute": Value::Null,
                "tokens_per_minute": Value::Null,
                "concurrent_requests": Value::Null,
            },
            "cost": {
                "currency": Value::Null,
                "input_microusd_per_million_tokens": Value::Null,
                "output_microusd_per_million_tokens": Value::Null,
            },
            "redaction": {
                "redact_provider_errors": true,
                "redact_request_payloads": true,
                "redact_response_payloads": true,
                "max_error_chars": 0,
            },
        },
        "usage_summary": {
            "request_count": 0,
            "failed_request_count": 0,
            "input_tokens": 0,
            "output_tokens": 0,
            "total_tokens": 0,
            "last_request_id": Value::Null,
        },
        "recent_usage_events": [],
        "balance": {
            "supported": false,
            "currency": Value::Null,
            "amount_microunits": Value::Null,
            "checked_at": Value::Null,
        },
        "debug_trace_status": {
            "enabled": false,
            "redacted": true,
            "storage": "disabled",
            "retained_trace_count": 0,
            "latest_trace_id": Value::Null,
        },
        "context_budget_report": {
            "quality_first": output.context_budget.quality_first,
            "max_prompt_chars": output.context_budget.max_prompt_chars,
            "estimated_prompt_chars": output.context_budget.estimated_prompt_chars,
            "budget_pressure": output.context_budget.budget_pressure.clone(),
            "trimmed_item_count": output.context_budget.trimmed_item_count,
            "items": output.context_budget.items.clone(),
        },
        "tool_output_budget": {
            "largest_output_chars": 0,
            "trimmed_output_count": output.context_budget.trimmed_item_count,
            "preserved_recent_output_count": 0,
            "preserved_error_count": 0,
            "preserved_evidence_ref_count": output.context_budget.evidence_item_count,
        },
        "liveness_events": [],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use assistant_runtime::CodexConversationExecutorStatus;
    use contracts::{AssistantRunCodexContextBudgetView, AssistantRunExecutorTransportView};

    fn codex_executor_output(model_gateway: Value) -> CodexConversationExecutorOutput {
        CodexConversationExecutorOutput {
            transport: AssistantRunExecutorTransportView::CodexDryRun,
            status: CodexConversationExecutorStatus::ShadowDryRun,
            codex_invoked: false,
            fallback_to_direct: true,
            assistant_message: None,
            suggested_action: None,
            planned_action_types: Vec::new(),
            execution_trail: Vec::new(),
            context_budget: AssistantRunCodexContextBudgetView {
                max_prompt_chars: Some(4096),
                estimated_prompt_chars: 2048,
                budget_pressure: "medium".to_string(),
                evidence_item_count: 2,
                trimmed_item_count: 1,
                ..AssistantRunCodexContextBudgetView::default()
            },
            model_gateway,
            output_schema: None,
            host_invocation: None,
        }
    }

    #[test]
    fn assistant_run_codex_observability_support_returns_null_for_non_shim_wire_api() {
        let output = codex_executor_output(json!({
            "wire_api": "chat_completions",
            "selected_model": {
                "provider": "openai",
                "model": "gpt-5.5"
            }
        }));

        assert_eq!(
            assistant_run_codex_provider_shim_observability_from_output(&output),
            Value::Null
        );
    }

    #[test]
    fn assistant_run_codex_observability_support_summarizes_shim_without_secret_names() {
        let output = codex_executor_output(json!({
            "wire_api": "codex_compatible_shim",
            "selected_model": {
                "provider": "minimax",
                "model": "MiniMax-M2.7"
            },
            "auth_configured": true,
            "auth_env_key_name": "MINIMAX_API_KEY",
            "profile_id": "minimax-codex-shadow",
            "capabilities": ["chat", "json_mode", "tool_calling", "codex_compatible"],
            "capability_manifest": {
                "codex_compatible": true
            }
        }));

        let observability = assistant_run_codex_provider_shim_observability_from_output(&output);

        assert_eq!(observability["profile"]["provider_id"], json!("minimax"));
        assert_eq!(observability["profile"]["model_id"], json!("MiniMax-M2.7"));
        assert_eq!(
            observability["profile"]["profile_id"],
            json!("minimax-codex-shadow")
        );
        assert_eq!(
            observability["profile"]["wire_api"],
            json!("codex_compatible_shim")
        );
        assert_eq!(observability["profile"]["auth_configured"], json!(true));
        assert_eq!(
            observability["profile"]["capability_manifest"]["codex_compatible"],
            json!(true)
        );
        assert_eq!(
            observability["context_budget_report"]["budget_pressure"],
            json!("medium")
        );
        assert_eq!(
            observability["context_budget_report"]["trimmed_item_count"],
            json!(1)
        );
        assert_eq!(
            observability["tool_output_budget"]["preserved_evidence_ref_count"],
            json!(2)
        );
        assert_eq!(observability["profile"]["auth_env_key_name"], Value::Null);
        assert!(!observability.to_string().contains("MINIMAX_API_KEY"));
    }
}
