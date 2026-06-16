use serde_json::{json, Value};

pub(crate) fn assistant_run_codex_promotion_gate_summary_with_model_gateway(
    shadow_gate: &Value,
    host_validation: &Value,
    model_gateway_gate: &Value,
) -> Value {
    let shadow_ready = shadow_gate
        .get("host_validation_allowed")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let host_validated = host_validation.get("status").and_then(Value::as_str) == Some("validated");
    let model_gateway_ready = model_gateway_gate
        .get("ready_for_promotion_review")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let status = if shadow_ready && host_validated && model_gateway_ready {
        "eligible_for_feature_gate_review"
    } else if !shadow_ready {
        "blocked_by_shadow_gate"
    } else if !host_validated {
        "blocked_by_host_validation"
    } else {
        "blocked_by_model_gateway"
    };
    let blocked_by = match status {
        "eligible_for_feature_gate_review" => Value::Null,
        "blocked_by_shadow_gate" => shadow_gate
            .get("status")
            .cloned()
            .unwrap_or_else(|| json!("shadow_gate_not_ready")),
        "blocked_by_host_validation" => host_validation
            .get("status")
            .cloned()
            .unwrap_or_else(|| json!("host_validation_not_run")),
        "blocked_by_model_gateway" => model_gateway_gate
            .get("status")
            .cloned()
            .unwrap_or_else(|| json!("model_gateway_not_ready")),
        _ => json!("unknown"),
    };
    let next_step = match status {
        "eligible_for_feature_gate_review" => {
            "review_shadow_and_host_reports_before_enabling_codex_executor_gate"
        }
        "blocked_by_shadow_gate" => "continue_shadow_comparison_until_stable",
        "blocked_by_host_validation" => "run_or_fix_jump_host_validation",
        "blocked_by_model_gateway" => "fix_codex_model_gateway_profile_before_promotion",
        _ => "inspect_codex_executor_diagnostics",
    };
    let shadow_gate_status = shadow_gate.get("status").cloned().unwrap_or(Value::Null);
    let host_validation_status = host_validation
        .get("status")
        .cloned()
        .unwrap_or(Value::Null);
    let model_gateway_status = model_gateway_gate
        .get("status")
        .cloned()
        .unwrap_or(Value::Null);

    json!({
        "status": status,
        "shadow_gate_status": shadow_gate_status,
        "host_validation_status": host_validation_status,
        "model_gateway_status": model_gateway_status,
        "readiness_checks": {
            "shadow_gate": {
                "ready": shadow_ready,
                "status": shadow_gate_status,
            },
            "host_validation": {
                "ready": host_validated,
                "status": host_validation_status,
            },
            "model_gateway": {
                "ready": model_gateway_ready,
                "status": model_gateway_status,
            },
            "all_ready": shadow_ready && host_validated && model_gateway_ready,
        },
        "eligible_for_feature_gate_review": status == "eligible_for_feature_gate_review",
        "blocked_by": blocked_by,
        "direct_execution_authoritative": true,
        "local_execution_allowed": false,
        "codex_mutation_allowed": false,
        "queue_allowed": false,
        "requires_manual_feature_gate_change": true,
        "next_step": next_step,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn promotion_gate_requires_shadow_host_and_model_gateway_readiness() {
        let shadow_gate = json!({
            "status": "eligible_for_host_validation",
            "host_validation_allowed": true
        });
        let host_validation = json!({"status": "validated"});
        let model_gateway_ready = json!({
            "status": "ready",
            "ready_for_promotion_review": true
        });

        let ready = assistant_run_codex_promotion_gate_summary_with_model_gateway(
            &shadow_gate,
            &host_validation,
            &model_gateway_ready,
        );

        assert_eq!(ready["status"], json!("eligible_for_feature_gate_review"));
        assert_eq!(ready["eligible_for_feature_gate_review"], json!(true));
        assert_eq!(ready["readiness_checks"]["all_ready"], json!(true));
        assert_eq!(ready["codex_mutation_allowed"], json!(false));
        assert_eq!(ready["queue_allowed"], json!(false));
        assert_eq!(
            ready["next_step"],
            json!("review_shadow_and_host_reports_before_enabling_codex_executor_gate")
        );

        let blocked_model_gateway = assistant_run_codex_promotion_gate_summary_with_model_gateway(
            &shadow_gate,
            &host_validation,
            &json!({
                "status": "auth_not_configured",
                "ready_for_promotion_review": false
            }),
        );

        assert_eq!(
            blocked_model_gateway["status"],
            json!("blocked_by_model_gateway")
        );
        assert_eq!(
            blocked_model_gateway["blocked_by"],
            json!("auth_not_configured")
        );
        assert_eq!(
            blocked_model_gateway["readiness_checks"]["model_gateway"]["ready"],
            json!(false)
        );
        assert_eq!(
            blocked_model_gateway["readiness_checks"]["all_ready"],
            json!(false)
        );
        assert_eq!(
            blocked_model_gateway["next_step"],
            json!("fix_codex_model_gateway_profile_before_promotion")
        );
    }
}
