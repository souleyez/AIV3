use assistant_runtime::CodexConversationExecutorOutput;
use chrono::{DateTime, Utc};
use serde_json::{json, Value};

pub(crate) fn assistant_run_codex_execution_trail_entries(
    output: &CodexConversationExecutorOutput,
    now: DateTime<Utc>,
    shadow_comparison: Option<&Value>,
    transport_policy: Option<&Value>,
) -> Vec<Value> {
    vec![json!({
        "status": "completed",
        "label": "Codex 执行器诊断",
        "transport": output.transport.as_str(),
        "transport_policy": assistant_run_codex_transport_policy_summary(transport_policy),
        "executor_status": output.status.as_str(),
        "codex_invoked": output.codex_invoked,
        "fallback_to_direct": output.fallback_to_direct,
        "planned_action_types": output.planned_action_types.clone(),
        "suggested_action": assistant_run_codex_suggested_action_summary(
            output.suggested_action.as_ref(),
            shadow_comparison,
        ),
        "model_gateway": output.model_gateway.clone(),
        "provider_shim_observability": assistant_run_codex_provider_shim_observability_from_output(output),
        "output_schema": output.output_schema.clone(),
        "host_invocation": assistant_run_codex_host_invocation_summary(output.host_invocation.as_ref()),
        "shadow_comparison": shadow_comparison.cloned(),
        "at": now,
    })]
}

pub(crate) fn assistant_run_codex_event_payload(
    output: &CodexConversationExecutorOutput,
    shadow_comparison: Option<&Value>,
    transport_policy: Option<&Value>,
) -> Value {
    json!({
        "transport": output.transport.as_str(),
        "transport_policy": assistant_run_codex_transport_policy_summary(transport_policy),
        "status": output.status.as_str(),
        "codex_invoked": output.codex_invoked,
        "fallback_to_direct": output.fallback_to_direct,
        "planned_action_types": output.planned_action_types.clone(),
        "suggested_action": assistant_run_codex_suggested_action_summary(
            output.suggested_action.as_ref(),
            shadow_comparison,
        ),
        "model_gateway": output.model_gateway.clone(),
        "provider_shim_observability": assistant_run_codex_provider_shim_observability_from_output(output),
        "output_schema": output.output_schema.clone(),
        "host_invocation": assistant_run_codex_host_invocation_summary(output.host_invocation.as_ref()),
        "context_budget": output.context_budget.clone(),
        "supply_quality": assistant_run_codex_output_supply_quality_summary(output),
        "execution_trail": assistant_run_codex_runtime_execution_trail_summary(&output.execution_trail),
        "shadow_comparison": shadow_comparison.cloned(),
    })
}

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

pub(crate) fn assistant_run_codex_transport_policy_summary(
    transport_policy: Option<&Value>,
) -> Value {
    let Some(policy) = transport_policy.filter(|value| value.is_object()) else {
        return Value::Null;
    };

    json!({
        "requested_transport": policy
            .get("requested_transport")
            .cloned()
            .unwrap_or(Value::Null),
        "effective_transport": policy
            .get("effective_transport")
            .cloned()
            .unwrap_or(Value::Null),
        "real_transport_requested": policy
            .get("real_transport_requested")
            .cloned()
            .unwrap_or(Value::Bool(false)),
        "real_transport_feature_gate_enabled": policy
            .get("real_transport_feature_gate_enabled")
            .cloned()
            .unwrap_or(Value::Bool(false)),
        "real_transport_promotion_review_approved": policy
            .get("real_transport_promotion_review_approved")
            .cloned()
            .unwrap_or(Value::Bool(false)),
        "downgraded": policy
            .get("downgraded")
            .cloned()
            .unwrap_or(Value::Bool(false)),
        "downgrade_reason": policy
            .get("downgrade_reason")
            .cloned()
            .unwrap_or(Value::Null),
        "direct_execution_authoritative": policy
            .get("direct_execution_authoritative")
            .cloned()
            .unwrap_or(Value::Bool(true)),
        "codex_mutation_allowed": policy
            .get("codex_mutation_allowed")
            .cloned()
            .unwrap_or(Value::Bool(false)),
        "queue_allowed": policy
            .get("queue_allowed")
            .cloned()
            .unwrap_or(Value::Bool(false)),
        "manual_feature_gate_required_for_real_transport": policy
            .get("manual_feature_gate_required_for_real_transport")
            .cloned()
            .unwrap_or(Value::Bool(false)),
        "promotion_review_required_for_real_transport": policy
            .get("promotion_review_required_for_real_transport")
            .cloned()
            .unwrap_or(Value::Bool(false)),
        "host_validation_required_for_real_transport": policy
            .get("host_validation_required_for_real_transport")
            .cloned()
            .unwrap_or(Value::Bool(false)),
        "next_step": policy.get("next_step").cloned().unwrap_or(Value::Null),
    })
}

pub(crate) fn assistant_run_codex_host_invocation_summary(
    host_invocation: Option<&Value>,
) -> Value {
    let Some(host_invocation) = host_invocation.filter(|value| value.is_object()) else {
        return Value::Null;
    };
    let command_blueprint = host_invocation
        .get("command_blueprint")
        .filter(|value| value.is_object())
        .map(|command| {
            json!({
                "program": command.get("program").cloned().unwrap_or(Value::Null),
                "args": command.get("args").cloned().unwrap_or_else(|| json!([])),
                "stdin": command.get("stdin").cloned().unwrap_or(Value::Null),
                "workspace": command.get("workspace").cloned().unwrap_or(Value::Null),
            })
        })
        .unwrap_or(Value::Null);
    let safety = host_invocation
        .get("safety")
        .filter(|value| value.is_object())
        .map(|safety| {
            json!({
                "v3_validates_all_actions": safety
                    .get("v3_validates_all_actions")
                    .cloned()
                    .unwrap_or(Value::Bool(true)),
                "direct_database_access_allowed": safety
                    .get("direct_database_access_allowed")
                    .cloned()
                    .unwrap_or(Value::Bool(false)),
                "direct_queue_access_allowed": safety
                    .get("direct_queue_access_allowed")
                    .cloned()
                    .unwrap_or(Value::Bool(false)),
                "real_host_validation_required": safety
                    .get("real_host_validation_required")
                    .cloned()
                    .unwrap_or(Value::Bool(true)),
            })
        })
        .unwrap_or(Value::Null);

    json!({
        "kind": host_invocation.get("kind").cloned().unwrap_or(Value::Null),
        "transport": host_invocation
            .get("transport")
            .cloned()
            .unwrap_or(Value::Null),
        "host_required": host_invocation
            .get("host_required")
            .cloned()
            .unwrap_or(Value::Bool(true)),
        "local_execution_allowed": host_invocation
            .get("local_execution_allowed")
            .cloned()
            .unwrap_or(Value::Bool(false)),
        "mutation_allowed": host_invocation
            .get("mutation_allowed")
            .cloned()
            .unwrap_or(Value::Bool(false)),
        "queue_allowed": host_invocation
            .get("queue_allowed")
            .cloned()
            .unwrap_or(Value::Bool(false)),
        "input_contract": host_invocation
            .get("input_contract")
            .cloned()
            .unwrap_or(Value::Null),
        "output_schema_title": host_invocation
            .get("output_schema_title")
            .cloned()
            .unwrap_or(Value::Null),
        "output_schema_required": host_invocation
            .get("output_schema_required")
            .cloned()
            .unwrap_or_else(|| json!([])),
        "command_blueprint": command_blueprint,
        "model_gateway": assistant_run_codex_model_gateway_diagnostics_summary(
            host_invocation.get("model_gateway"),
        ),
        "safety": safety,
    })
}

pub(crate) fn assistant_run_codex_runtime_execution_trail_summary(items: &[Value]) -> Vec<Value> {
    items
        .iter()
        .map(|item| {
            let mut item = item.clone();
            if let Some(object) = item.as_object_mut() {
                if let Some(host_invocation) = object.get("host_invocation").cloned() {
                    object.insert(
                        "host_invocation".to_string(),
                        assistant_run_codex_host_invocation_summary(Some(&host_invocation)),
                    );
                }
                if let Some(suggested_action) = object.get("suggested_action").cloned() {
                    object.insert(
                        "suggested_action".to_string(),
                        assistant_run_codex_suggested_action_summary(Some(&suggested_action), None),
                    );
                }
            }
            item
        })
        .collect()
}

pub(crate) fn assistant_run_codex_output_supply_quality_summary(
    output: &CodexConversationExecutorOutput,
) -> Value {
    output
        .execution_trail
        .iter()
        .find_map(|item| item.get("supply_quality"))
        .map(|supply_quality| {
            assistant_run_codex_supply_quality_diagnostics_summary(Some(supply_quality))
        })
        .unwrap_or(Value::Null)
}

pub(crate) fn assistant_run_codex_supply_quality_diagnostics_summary(
    supply_quality: Option<&Value>,
) -> Value {
    let Some(source) = supply_quality.filter(|value| value.is_object()) else {
        return Value::Null;
    };
    let field = |names: &[&str]| -> Value {
        names
            .iter()
            .find_map(|name| source.get(*name).cloned())
            .unwrap_or(Value::Null)
    };

    json!({
        "status": field(&["status"]),
        "intent": field(&["intent"]),
        "supplyRequested": field(&["supplyRequested", "supply_requested"]),
        "qualityFirst": field(&["qualityFirst", "quality_first"]),
        "selectedDatasetCount": field(&["selectedDatasetCount", "selected_dataset_count"]),
        "suppliedItemCount": field(&["suppliedItemCount", "supplied_item_count"]),
        "indexedEvidenceCount": field(&["indexedEvidenceCount", "indexed_evidence_count"]),
        "fallbackChunkCount": field(&["fallbackChunkCount", "fallback_chunk_count"]),
        "conversationMemoryItemCount": field(&["conversationMemoryItemCount", "conversation_memory_item_count"]),
        "hiddenMemoryItemCount": field(&["hiddenMemoryItemCount", "hidden_memory_item_count"]),
        "mediaContextCount": field(&["mediaContextCount", "media_context_count"]),
        "detailTargetCount": field(&["detailTargetCount", "detail_target_count"]),
        "limit": field(&["limit"]),
        "citationLocatorCount": field(&["citationLocatorCount", "citation_locator_count"]),
    })
}

pub(crate) fn assistant_run_codex_suggested_action_summary(
    suggested_action: Option<&Value>,
    shadow: Option<&Value>,
) -> Value {
    let Some(action) = suggested_action.filter(|value| value.is_object()) else {
        return Value::Null;
    };
    let shadow = shadow.unwrap_or(&Value::Null);

    json!({
        "action_type": action
            .get("action_type")
            .cloned()
            .or_else(|| shadow.pointer("/codex/suggested_action_type").cloned())
            .unwrap_or(Value::Null),
        "title": action.get("title").cloned().unwrap_or(Value::Null),
        "source": action.get("source").cloned().unwrap_or(Value::Null),
        "confidence": action.get("confidence").cloned().unwrap_or(Value::Null),
        "requires_v3_validation": action
            .get("requires_v3_validation")
            .cloned()
            .unwrap_or(Value::Null),
        "mutates_state": action.get("mutates_state").cloned().unwrap_or(Value::Null),
        "mutation_allowed": action
            .get("mutation_allowed")
            .cloned()
            .unwrap_or(Value::Bool(false)),
        "queue_allowed": action
            .get("queue_allowed")
            .cloned()
            .unwrap_or(Value::Bool(false)),
        "requires_confirmation": action
            .get("requires_confirmation")
            .cloned()
            .unwrap_or(Value::Null),
        "has_arguments": action.get("arguments").is_some(),
        "has_input_schema": action.get("input_schema").is_some(),
    })
}

pub(crate) fn assistant_run_codex_model_gateway_diagnostics_summary(
    model_gateway: Option<&Value>,
) -> Value {
    let Some(model_gateway) = model_gateway.filter(|value| value.is_object()) else {
        return Value::Null;
    };
    let selected_model = model_gateway
        .get("selected_model")
        .filter(|value| value.is_object())
        .map(|selected| {
            json!({
                "mode": selected.get("mode").cloned().unwrap_or(Value::Null),
                "provider": selected.get("provider").cloned().unwrap_or(Value::Null),
                "model": selected.get("model").cloned().unwrap_or(Value::Null),
            })
        })
        .unwrap_or(Value::Null);

    json!({
        "lane": model_gateway.get("lane").cloned().unwrap_or(Value::Null),
        "selected_model": selected_model,
        "profile_source": model_gateway
            .get("profile_source")
            .cloned()
            .unwrap_or(Value::Null),
        "profile_status": model_gateway
            .get("profile_status")
            .cloned()
            .unwrap_or(Value::Null),
        "profile_available": model_gateway.get("profile").is_some(),
        "profile_id": model_gateway
            .get("profile_id")
            .or_else(|| model_gateway.pointer("/profile/profile_id"))
            .cloned()
            .unwrap_or(Value::Null),
        "provider_id": model_gateway
            .get("provider_id")
            .or_else(|| model_gateway.pointer("/profile/provider_id"))
            .cloned()
            .unwrap_or(Value::Null),
        "model_id": model_gateway
            .get("model_id")
            .or_else(|| model_gateway.pointer("/profile/model_id"))
            .cloned()
            .unwrap_or(Value::Null),
        "wire_api": model_gateway
            .get("wire_api")
            .or_else(|| model_gateway.pointer("/profile/wire_api"))
            .cloned()
            .unwrap_or(Value::Null),
        "auth_configured": model_gateway
            .get("auth_configured")
            .or_else(|| model_gateway.pointer("/profile/auth/configured"))
            .cloned()
            .unwrap_or(Value::Bool(false)),
        "capabilities": model_gateway
            .get("capabilities")
            .or_else(|| model_gateway.pointer("/profile/capabilities"))
            .cloned()
            .unwrap_or_else(|| json!([])),
        "capability_manifest": model_gateway
            .get("capability_manifest")
            .or_else(|| model_gateway.pointer("/profile/capability_flags"))
            .cloned()
            .unwrap_or(Value::Null),
        "codex_surface": model_gateway
            .get("codex_surface")
            .cloned()
            .unwrap_or(Value::Null),
        "codex_real_execution_allowed": model_gateway
            .get("codex_real_execution_allowed")
            .cloned()
            .unwrap_or(Value::Bool(false)),
        "secrets_redacted": model_gateway
            .pointer("/safety/secrets_redacted")
            .cloned()
            .unwrap_or(Value::Bool(true)),
        "raw_provider_payloads_allowed": model_gateway
            .pointer("/safety/raw_provider_payloads_allowed")
            .cloned()
            .unwrap_or(Value::Bool(false)),
        "codex_real_execution_allowed_on_this_host": model_gateway
            .pointer("/safety/codex_real_execution_allowed_on_this_host")
            .cloned()
            .unwrap_or(Value::Bool(false)),
        "v3_validates_all_actions": model_gateway
            .pointer("/safety/v3_validates_all_actions")
            .cloned()
            .unwrap_or(Value::Bool(true)),
    })
}

pub(crate) fn assistant_run_codex_provider_shim_observability_summary(
    snapshot: Option<&Value>,
) -> Value {
    let Some(snapshot) = snapshot.filter(|value| value.is_object()) else {
        return Value::Null;
    };
    let health = snapshot
        .get("health")
        .filter(|value| value.is_object())
        .map(|health| {
            json!({
                "status": health.get("status").cloned().unwrap_or(Value::Null),
                "process_reachable": health
                    .get("process_reachable")
                    .cloned()
                    .unwrap_or(Value::Bool(false)),
                "upstream_reachable": health
                    .get("upstream_reachable")
                    .cloned()
                    .unwrap_or(Value::Bool(false)),
                "checked_at": health.get("checked_at").cloned().unwrap_or(Value::Null),
                "has_message": health.get("message").is_some(),
            })
        })
        .unwrap_or(Value::Null);
    let profile = snapshot
        .get("profile")
        .filter(|value| value.is_object())
        .map(|profile| {
            json!({
                "profile_id": profile.get("profile_id").cloned().unwrap_or(Value::Null),
                "provider_id": profile.get("provider_id").cloned().unwrap_or(Value::Null),
                "model_id": profile.get("model_id").cloned().unwrap_or(Value::Null),
                "wire_api": profile.get("wire_api").cloned().unwrap_or(Value::Null),
                "endpoint_scope": profile.get("endpoint_scope").cloned().unwrap_or(Value::Null),
                "base_url_configured": profile
                    .get("base_url_configured")
                    .cloned()
                    .unwrap_or(Value::Bool(false)),
                "api_path": profile.get("api_path").cloned().unwrap_or(Value::Null),
                "auth_configured": profile
                    .get("auth_configured")
                    .cloned()
                    .unwrap_or(Value::Bool(false)),
                "timeout_ms": profile.get("timeout_ms").cloned().unwrap_or(Value::Null),
                "capabilities": profile
                    .get("capabilities")
                    .cloned()
                    .unwrap_or_else(|| json!([])),
                "rate_limit": {
                    "requests_per_minute": profile
                        .pointer("/rate_limit/requests_per_minute")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "tokens_per_minute": profile
                        .pointer("/rate_limit/tokens_per_minute")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "concurrent_requests": profile
                        .pointer("/rate_limit/concurrent_requests")
                        .cloned()
                        .unwrap_or(Value::Null),
                },
                "cost": {
                    "currency": profile.pointer("/cost/currency").cloned().unwrap_or(Value::Null),
                    "input_microusd_per_million_tokens": profile
                        .pointer("/cost/input_microusd_per_million_tokens")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "output_microusd_per_million_tokens": profile
                        .pointer("/cost/output_microusd_per_million_tokens")
                        .cloned()
                        .unwrap_or(Value::Null),
                },
                "redaction": {
                    "redact_provider_errors": profile
                        .pointer("/redaction/redact_provider_errors")
                        .cloned()
                        .unwrap_or(Value::Bool(true)),
                    "redact_request_payloads": profile
                        .pointer("/redaction/redact_request_payloads")
                        .cloned()
                        .unwrap_or(Value::Bool(true)),
                    "redact_response_payloads": profile
                        .pointer("/redaction/redact_response_payloads")
                        .cloned()
                        .unwrap_or(Value::Bool(true)),
                    "max_error_chars": profile
                        .pointer("/redaction/max_error_chars")
                        .cloned()
                        .unwrap_or(Value::Null),
                },
            })
        })
        .unwrap_or(Value::Null);
    let usage_summary = snapshot
        .get("usage_summary")
        .filter(|value| value.is_object())
        .map(|usage| {
            json!({
                "request_count": usage.get("request_count").cloned().unwrap_or(Value::Null),
                "failed_request_count": usage
                    .get("failed_request_count")
                    .cloned()
                    .unwrap_or(Value::Null),
                "input_tokens": usage.get("input_tokens").cloned().unwrap_or(Value::Null),
                "output_tokens": usage.get("output_tokens").cloned().unwrap_or(Value::Null),
                "total_tokens": usage.get("total_tokens").cloned().unwrap_or(Value::Null),
                "has_last_request_id": usage.get("last_request_id").is_some(),
            })
        })
        .unwrap_or(Value::Null);
    let recent_usage_events = snapshot
        .get("recent_usage_events")
        .and_then(Value::as_array)
        .map(|events| {
            let failed_count = events
                .iter()
                .filter(|event| {
                    event
                        .get("status")
                        .and_then(Value::as_str)
                        .map(|status| status != "ok" && status != "completed")
                        .unwrap_or(false)
                })
                .count();
            json!({
                "event_count": events.len(),
                "failed_event_count": failed_count,
                "latest_status": events
                    .last()
                    .and_then(|event| event.get("status"))
                    .cloned()
                    .unwrap_or(Value::Null),
                "latest_failure_kind": events
                    .last()
                    .and_then(|event| event.get("provider_failure_kind"))
                    .cloned()
                    .unwrap_or(Value::Null),
            })
        })
        .unwrap_or_else(|| {
            json!({
                "event_count": 0,
                "failed_event_count": 0,
                "latest_status": Value::Null,
                "latest_failure_kind": Value::Null,
            })
        });
    let balance = snapshot
        .get("balance")
        .filter(|value| value.is_object())
        .map(|balance| {
            json!({
                "supported": balance.get("supported").cloned().unwrap_or(Value::Bool(false)),
                "currency": balance.get("currency").cloned().unwrap_or(Value::Null),
                "has_amount": balance.get("amount_microunits").is_some(),
                "checked_at": balance.get("checked_at").cloned().unwrap_or(Value::Null),
                "has_note": balance.get("note").is_some(),
            })
        })
        .unwrap_or(Value::Null);
    let debug_trace_status = snapshot
        .get("debug_trace_status")
        .filter(|value| value.is_object())
        .map(|trace| {
            json!({
                "enabled": trace.get("enabled").cloned().unwrap_or(Value::Bool(false)),
                "redacted": trace.get("redacted").cloned().unwrap_or(Value::Bool(true)),
                "storage": trace.get("storage").cloned().unwrap_or(Value::Null),
                "retained_trace_count": trace
                    .get("retained_trace_count")
                    .cloned()
                    .unwrap_or(Value::Null),
                "has_latest_trace": trace.get("latest_trace_id").is_some(),
                "has_note": trace.get("note").is_some(),
            })
        })
        .unwrap_or(Value::Null);
    let context_budget_report = snapshot
        .get("context_budget_report")
        .filter(|value| value.is_object())
        .map(|budget| {
            json!({
                "quality_first": budget.get("quality_first").cloned().unwrap_or(Value::Bool(true)),
                "max_prompt_chars": budget.get("max_prompt_chars").cloned().unwrap_or(Value::Null),
                "estimated_prompt_chars": budget
                    .get("estimated_prompt_chars")
                    .cloned()
                    .unwrap_or(Value::Null),
                "budget_pressure": budget.get("budget_pressure").cloned().unwrap_or(Value::Null),
                "trimmed_item_count": budget
                    .get("trimmed_item_count")
                    .cloned()
                    .unwrap_or(Value::Null),
                "item_count": budget
                    .get("items")
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    .unwrap_or(0),
            })
        })
        .unwrap_or(Value::Null);
    let tool_output_budget = snapshot
        .get("tool_output_budget")
        .filter(|value| value.is_object())
        .map(|budget| {
            json!({
                "largest_output_chars": budget
                    .get("largest_output_chars")
                    .cloned()
                    .unwrap_or(Value::Null),
                "trimmed_output_count": budget
                    .get("trimmed_output_count")
                    .cloned()
                    .unwrap_or(Value::Null),
                "preserved_recent_output_count": budget
                    .get("preserved_recent_output_count")
                    .cloned()
                    .unwrap_or(Value::Null),
                "preserved_error_count": budget
                    .get("preserved_error_count")
                    .cloned()
                    .unwrap_or(Value::Null),
                "preserved_evidence_ref_count": budget
                    .get("preserved_evidence_ref_count")
                    .cloned()
                    .unwrap_or(Value::Null),
                "has_note": budget.get("note").is_some(),
            })
        })
        .unwrap_or(Value::Null);
    let liveness_events = snapshot
        .get("liveness_events")
        .and_then(Value::as_array)
        .map(|events| {
            json!({
                "event_count": events.len(),
                "latest": events.last().map(|event| {
                    json!({
                        "event_type": event.get("event_type").cloned().unwrap_or(Value::Null),
                        "status": event.get("status").cloned().unwrap_or(Value::Null),
                        "retry_count": event.get("retry_count").cloned().unwrap_or(Value::Null),
                        "action": event.get("action").cloned().unwrap_or(Value::Null),
                        "occurred_at": event.get("occurred_at").cloned().unwrap_or(Value::Null),
                        "has_note": event.get("note").is_some(),
                    })
                }).unwrap_or(Value::Null),
            })
        })
        .unwrap_or_else(|| json!({"event_count": 0, "latest": Value::Null}));

    json!({
        "schema_version": snapshot.get("schema_version").cloned().unwrap_or(Value::Null),
        "health": health,
        "profile": profile,
        "usage_summary": usage_summary,
        "recent_usage_events": recent_usage_events,
        "balance": balance,
        "debug_trace_status": debug_trace_status,
        "context_budget_report": context_budget_report,
        "tool_output_budget": tool_output_budget,
        "liveness_events": liveness_events,
        "raw_payloads_exposed": false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use assistant_runtime::CodexConversationExecutorStatus;
    use chrono::TimeZone;
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

    #[test]
    fn assistant_run_codex_observability_support_builds_event_payload_without_raw_action_inputs() {
        let mut output = codex_executor_output(json!({
            "wire_api": "codex_compatible_shim",
            "selected_model": {
                "provider": "minimax",
                "model": "MiniMax-M2.7"
            },
            "auth_configured": true,
            "profile_id": "minimax-codex-shadow",
            "capabilities": ["chat", "json_mode", "tool_calling", "codex_compatible"],
            "capability_manifest": {
                "codex_compatible": true
            }
        }));
        output.codex_invoked = true;
        output.fallback_to_direct = false;
        output.planned_action_types = vec!["submit_html_artifact_event".to_string()];
        output.suggested_action = Some(json!({
            "action_type": "submit_html_artifact_event",
            "title": "生成经营报表",
            "source": "codex",
            "confidence": 0.86,
            "requires_v3_validation": true,
            "mutation_allowed": false,
            "queue_allowed": true,
            "arguments": {
                "raw_customer_prompt": "should-not-leak"
            },
            "input_schema": {
                "type": "object"
            }
        }));
        output.host_invocation = Some(json!({
            "kind": "codex_host_invocation",
            "transport": "codex_exec_schema",
            "host_required": true,
            "local_execution_allowed": false,
            "mutation_allowed": false,
            "queue_allowed": true,
            "command_blueprint": {
                "program": "codex",
                "args": ["exec", "--json"],
                "stdin": {"redacted": true},
                "workspace": "/srv/aiv3/repo"
            },
            "model_gateway": {
                "lane": "codex_conversation",
                "selected_model": {
                    "mode": "model_gateway",
                    "provider": "minimax",
                    "model": "MiniMax-M2.7"
                },
                "safety": {
                    "secrets_redacted": true,
                    "raw_provider_payloads_allowed": false
                }
            },
            "safety": {
                "v3_validates_all_actions": true,
                "direct_database_access_allowed": false,
                "direct_queue_access_allowed": false,
                "real_host_validation_required": true
            }
        }));
        output.execution_trail = vec![json!({
            "kind": "codex_executor.plan_only",
            "suggested_action": {
                "action_type": "submit_html_artifact_event",
                "arguments": {
                    "raw_customer_prompt": "should-not-leak"
                },
                "input_schema": {
                    "type": "object"
                }
            },
            "supply_quality": {
                "status": "sufficient",
                "intent": "static_report",
                "supplyRequested": true,
                "selectedDatasetCount": 1,
                "indexedEvidenceCount": 9,
                "citationLocatorCount": 3
            }
        })];

        let shadow_comparison = json!({
            "codex": {
                "suggested_action_type": "submit_html_artifact_event"
            }
        });
        let transport_policy = json!({
            "requested_transport": "codex_real",
            "effective_transport": "codex_plan_only",
            "real_transport_requested": true,
            "real_transport_feature_gate_enabled": false,
            "downgraded": true,
            "downgrade_reason": "feature_gate_disabled",
            "debug_token": "policy-secret-token"
        });
        let now = Utc
            .with_ymd_and_hms(2026, 6, 16, 8, 30, 0)
            .single()
            .expect("fixed timestamp is valid");

        let payload = assistant_run_codex_event_payload(
            &output,
            Some(&shadow_comparison),
            Some(&transport_policy),
        );
        let trail = assistant_run_codex_execution_trail_entries(
            &output,
            now,
            Some(&shadow_comparison),
            Some(&transport_policy),
        );
        let serialized =
            serde_json::to_string(&(payload.clone(), trail.clone())).expect("payload serializes");

        assert_eq!(
            payload["transport_policy"]["requested_transport"],
            json!("codex_real")
        );
        assert_eq!(
            payload["transport_policy"]["downgrade_reason"],
            json!("feature_gate_disabled")
        );
        assert_eq!(
            payload["suggested_action"]["action_type"],
            json!("submit_html_artifact_event")
        );
        assert_eq!(payload["suggested_action"]["has_arguments"], json!(true));
        assert_eq!(
            payload["provider_shim_observability"]["profile"]["provider_id"],
            json!("minimax")
        );
        assert_eq!(
            payload["host_invocation"]["command_blueprint"]["program"],
            json!("codex")
        );
        assert_eq!(payload["supply_quality"]["status"], json!("sufficient"));
        assert_eq!(
            payload["execution_trail"][0]["suggested_action"]["has_arguments"],
            json!(true)
        );
        assert_eq!(trail[0]["label"], json!("Codex 执行器诊断"));
        assert_eq!(trail[0]["executor_status"], json!("shadow_dry_run"));
        assert_eq!(trail[0]["at"], json!(now));
        assert!(!serialized.contains("should-not-leak"));
        assert!(!serialized.contains("policy-secret-token"));
    }

    #[test]
    fn assistant_run_codex_observability_support_sanitizes_runtime_trail_entries() {
        let trail = vec![json!({
            "kind": "codex_executor.plan_only",
            "suggested_action": {
                "action_type": "submit_html_artifact_event",
                "title": "生成报表",
                "arguments": {
                    "raw_customer_prompt": "should-not-leak"
                },
                "input_schema": {
                    "type": "object"
                },
                "mutation_allowed": false,
                "queue_allowed": false
            },
            "host_invocation": {
                "kind": "codex_host_invocation",
                "transport": "codex_exec_schema",
                "host_required": true,
                "mutation_allowed": false,
                "queue_allowed": false,
                "command_blueprint": {
                    "program": "codex",
                    "args": ["exec", "--json"],
                    "stdin": {"redacted": true},
                    "workspace": "/srv/aiv3/repo"
                },
                "model_gateway": {
                    "lane": "codex_conversation",
                    "selected_model": {
                        "mode": "model_gateway",
                        "provider": "minimax",
                        "model": "MiniMax-M2.7"
                    },
                    "auth_env_key_name": "MINIMAX_API_KEY",
                    "safety": {
                        "secrets_redacted": true,
                        "raw_provider_payloads_allowed": false
                    }
                }
            }
        })];

        let sanitized = assistant_run_codex_runtime_execution_trail_summary(&trail);
        let serialized =
            serde_json::to_string(&sanitized).expect("sanitized codex trail serializes");

        assert_eq!(
            sanitized[0]["suggested_action"]["action_type"],
            json!("submit_html_artifact_event")
        );
        assert_eq!(
            sanitized[0]["suggested_action"]["has_arguments"],
            json!(true)
        );
        assert_eq!(
            sanitized[0]["host_invocation"]["model_gateway"]["lane"],
            json!("codex_conversation")
        );
        assert_eq!(
            sanitized[0]["host_invocation"]["command_blueprint"]["program"],
            json!("codex")
        );
        assert!(!serialized.contains("should-not-leak"));
        assert!(!serialized.contains("MINIMAX_API_KEY"));
    }
}
