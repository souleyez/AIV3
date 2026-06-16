use contracts::AssistantRunExecutorTransportView;
use serde_json::{json, Value};

#[derive(Clone, Debug)]
pub(crate) struct AssistantRunExecutorTransportSelection {
    pub(crate) effective_transport: AssistantRunExecutorTransportView,
    pub(crate) policy: Value,
}

pub(crate) fn assistant_run_executor_transport_selection_from_env(
) -> Option<AssistantRunExecutorTransportSelection> {
    std::env::var("ASSISTANT_RUN_EXECUTOR")
        .ok()
        .and_then(|value| {
            assistant_run_executor_transport_selection_from_value(
                &value,
                assistant_run_codex_real_transport_feature_gate_enabled(),
                assistant_run_codex_real_transport_promotion_review_approved(),
            )
        })
}

pub(crate) fn assistant_run_executor_transport_selection_from_value(
    value: &str,
    real_transport_feature_gate_enabled: bool,
    real_transport_promotion_review_approved: bool,
) -> Option<AssistantRunExecutorTransportSelection> {
    let requested_transport = assistant_run_executor_transport_from_value(value)?;
    let real_transport_requested = assistant_run_executor_transport_is_real(&requested_transport);
    let real_transport_allowed = !real_transport_requested
        || (real_transport_feature_gate_enabled && real_transport_promotion_review_approved);
    let downgraded = real_transport_requested && !real_transport_allowed;
    let effective_transport = if downgraded {
        AssistantRunExecutorTransportView::CodexPlanOnly
    } else {
        requested_transport.clone()
    };
    let downgrade_reason = if real_transport_requested && !real_transport_feature_gate_enabled {
        json!("real_transport_feature_gate_disabled")
    } else if real_transport_requested && !real_transport_promotion_review_approved {
        json!("real_transport_promotion_review_not_approved")
    } else if downgraded {
        json!("real_transport_not_allowed")
    } else {
        Value::Null
    };
    let next_step = if real_transport_requested && !real_transport_feature_gate_enabled {
        "keep_codex_in_shadow_plan_only_until_promotion_gate_review"
    } else if real_transport_requested && !real_transport_promotion_review_approved {
        "review_shadow_and_host_reports_before_enabling_real_transport"
    } else if real_transport_requested {
        "assistant_runtime_still_validates_real_transport_before_execution"
    } else {
        "run_codex_shadow_transport"
    };

    Some(AssistantRunExecutorTransportSelection {
        effective_transport: effective_transport.clone(),
        policy: json!({
            "requested_transport": requested_transport.as_str(),
            "effective_transport": effective_transport.as_str(),
            "real_transport_requested": real_transport_requested,
            "real_transport_feature_gate_enabled": real_transport_feature_gate_enabled,
            "real_transport_promotion_review_approved": real_transport_promotion_review_approved,
            "downgraded": downgraded,
            "downgrade_reason": downgrade_reason,
            "direct_execution_authoritative": true,
            "codex_mutation_allowed": false,
            "queue_allowed": false,
            "manual_feature_gate_required_for_real_transport": real_transport_requested,
            "promotion_review_required_for_real_transport": real_transport_requested,
            "host_validation_required_for_real_transport": real_transport_requested,
            "next_step": next_step,
        }),
    })
}

pub(crate) fn assistant_run_executor_transport_is_real(
    transport: &AssistantRunExecutorTransportView,
) -> bool {
    matches!(
        transport,
        AssistantRunExecutorTransportView::CodexExecSchema
            | AssistantRunExecutorTransportView::CodexSdkThread
            | AssistantRunExecutorTransportView::CodexAppServer
            | AssistantRunExecutorTransportView::CodexMcpServer
    )
}

pub(crate) fn assistant_run_codex_real_transport_feature_gate_enabled() -> bool {
    std::env::var("ASSISTANT_RUN_CODEX_REAL_TRANSPORT_FEATURE_GATE")
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "enabled" | "on" | "yes"
            )
        })
        .unwrap_or(false)
}

pub(crate) fn assistant_run_codex_real_transport_promotion_review_approved() -> bool {
    std::env::var("ASSISTANT_RUN_CODEX_REAL_TRANSPORT_PROMOTION_REVIEW_APPROVED")
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "approved" | "enabled" | "on" | "yes"
            )
        })
        .unwrap_or(false)
}

pub(crate) fn assistant_run_executor_transport_from_value(
    value: &str,
) -> Option<AssistantRunExecutorTransportView> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "direct" => None,
        "codex" | "codex_dry_run" | "dry_run" | "shadow" | "shadow_dry_run" => {
            Some(AssistantRunExecutorTransportView::CodexDryRun)
        }
        "codex_plan_only" | "plan_only" | "plan" => {
            Some(AssistantRunExecutorTransportView::CodexPlanOnly)
        }
        "codex_exec_schema" | "exec_schema" => {
            Some(AssistantRunExecutorTransportView::CodexExecSchema)
        }
        "codex_sdk_thread" | "sdk_thread" => {
            Some(AssistantRunExecutorTransportView::CodexSdkThread)
        }
        "codex_app_server" | "app_server" => {
            Some(AssistantRunExecutorTransportView::CodexAppServer)
        }
        "codex_mcp_server" | "mcp_server" => {
            Some(AssistantRunExecutorTransportView::CodexMcpServer)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_from_value_accepts_only_explicit_codex_modes() {
        assert_eq!(assistant_run_executor_transport_from_value(""), None);
        assert_eq!(assistant_run_executor_transport_from_value("direct"), None);
        assert_eq!(
            assistant_run_executor_transport_from_value("codex"),
            Some(AssistantRunExecutorTransportView::CodexDryRun)
        );
        assert_eq!(
            assistant_run_executor_transport_from_value("shadow_dry_run"),
            Some(AssistantRunExecutorTransportView::CodexDryRun)
        );
        assert_eq!(
            assistant_run_executor_transport_from_value("codex_plan_only"),
            Some(AssistantRunExecutorTransportView::CodexPlanOnly)
        );
        assert_eq!(
            assistant_run_executor_transport_from_value("app_server"),
            Some(AssistantRunExecutorTransportView::CodexAppServer)
        );
        assert_eq!(assistant_run_executor_transport_from_value("unknown"), None);
    }

    #[test]
    fn real_transport_downgrades_without_feature_gate_and_review() {
        let selection = assistant_run_executor_transport_selection_from_value(
            "codex_exec_schema",
            false,
            false,
        )
        .expect("real transport selection");

        assert_eq!(
            selection.effective_transport,
            AssistantRunExecutorTransportView::CodexPlanOnly
        );
        assert_eq!(selection.policy["requested_transport"], "codex_exec_schema");
        assert_eq!(selection.policy["effective_transport"], "codex_plan_only");
        assert_eq!(selection.policy["downgraded"], true);
        assert_eq!(
            selection.policy["downgrade_reason"],
            "real_transport_feature_gate_disabled"
        );
        assert_eq!(selection.policy["codex_mutation_allowed"], false);
        assert_eq!(selection.policy["queue_allowed"], false);
    }

    #[test]
    fn real_transport_requires_promotion_review_after_feature_gate() {
        let selection =
            assistant_run_executor_transport_selection_from_value("app_server", true, false)
                .expect("review-blocked real transport selection");

        assert_eq!(
            selection.effective_transport,
            AssistantRunExecutorTransportView::CodexPlanOnly
        );
        assert_eq!(selection.policy["downgraded"], true);
        assert_eq!(
            selection.policy["downgrade_reason"],
            "real_transport_promotion_review_not_approved"
        );
        assert_eq!(
            selection.policy["real_transport_feature_gate_enabled"],
            true
        );
        assert_eq!(
            selection.policy["real_transport_promotion_review_approved"],
            false
        );
        assert_eq!(
            selection.policy["promotion_review_required_for_real_transport"],
            true
        );
    }

    #[test]
    fn real_transport_passes_when_feature_gate_and_review_are_enabled() {
        let selection =
            assistant_run_executor_transport_selection_from_value("app_server", true, true)
                .expect("approved real transport selection");

        assert_eq!(
            selection.effective_transport,
            AssistantRunExecutorTransportView::CodexAppServer
        );
        assert_eq!(selection.policy["downgraded"], false);
        assert_eq!(
            selection.policy["manual_feature_gate_required_for_real_transport"],
            true
        );
        assert_eq!(
            selection.policy["real_transport_promotion_review_approved"],
            true
        );
    }
}
