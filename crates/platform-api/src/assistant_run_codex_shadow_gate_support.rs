use domain_model::AssistantRunEvent;
use serde_json::{json, Value};

use crate::{
    assistant_run_codex_host_validation_readiness, assistant_run_codex_transport_policy_summary,
};

pub(crate) fn assistant_run_codex_recent_shadow_events(
    events: &[&AssistantRunEvent],
    limit: usize,
) -> Vec<Value> {
    events
        .iter()
        .rev()
        .take(limit)
        .copied()
        .map(assistant_run_codex_shadow_event_summary)
        .collect()
}

fn assistant_run_codex_shadow_event_summary(event: &AssistantRunEvent) -> Value {
    let payload = &event.payload;
    let shadow = payload.get("shadow_comparison").unwrap_or(&Value::Null);
    let suggested_action = payload.get("suggested_action").unwrap_or(&Value::Null);
    let model_gateway = payload.get("model_gateway").unwrap_or(&Value::Null);

    json!({
        "event_id": event.id.to_string(),
        "sequence_no": event.sequence_no,
        "transport": payload.get("transport").cloned().unwrap_or(Value::Null),
        "transport_policy": assistant_run_codex_transport_policy_summary(
            payload.get("transport_policy"),
        ),
        "status": payload.get("status").cloned().unwrap_or(Value::Null),
        "comparison_status": shadow
            .pointer("/comparison/status")
            .cloned()
            .unwrap_or(Value::Null),
        "actionable": shadow
            .pointer("/comparison/actionable")
            .cloned()
            .unwrap_or(Value::Bool(false)),
        "codex_has_suggestion": shadow
            .pointer("/comparison/codex_has_suggestion")
            .cloned()
            .unwrap_or(Value::Bool(false)),
        "direct_action_types": shadow
            .pointer("/direct/action_types")
            .cloned()
            .unwrap_or_else(|| json!([])),
        "codex_suggested_action_type": shadow
            .pointer("/codex/suggested_action_type")
            .cloned()
            .or_else(|| suggested_action.get("action_type").cloned())
            .unwrap_or(Value::Null),
        "codex_suggested_action_allowed": shadow
            .pointer("/codex/suggested_action_allowed")
            .cloned()
            .unwrap_or(Value::Null),
        "mutation_allowed": suggested_action
            .get("mutation_allowed")
            .cloned()
            .unwrap_or(Value::Bool(false)),
        "codex_mutation_allowed": shadow
            .get("codex_mutation_allowed")
            .cloned()
            .unwrap_or(Value::Bool(false)),
        "model_gateway_lane": model_gateway
            .get("lane")
            .cloned()
            .unwrap_or(Value::Null),
        "model_provider": model_gateway
            .pointer("/selected_model/provider")
            .cloned()
            .unwrap_or(Value::Null),
        "model": model_gateway
            .pointer("/selected_model/model")
            .cloned()
            .unwrap_or(Value::Null),
        "profile_status": model_gateway
            .get("profile_status")
            .cloned()
            .unwrap_or(Value::Null),
        "next_gate": shadow
            .pointer("/comparison/next_gate")
            .cloned()
            .unwrap_or(Value::Null),
    })
}

pub(crate) fn assistant_run_codex_shadow_gate_summary(events: &[&AssistantRunEvent]) -> Value {
    const MIN_STABLE_SHADOW_EVENTS: usize = 3;
    const SHADOW_GATE_WINDOW: usize = 20;

    let window = events
        .iter()
        .rev()
        .take(SHADOW_GATE_WINDOW)
        .copied()
        .collect::<Vec<_>>();
    let mut matched_count = 0_usize;
    let mut diverged_count = 0_usize;
    let mut invalid_count = 0_usize;
    let mut no_suggestion_count = 0_usize;
    let mut unsafe_mutation_signal_count = 0_usize;
    let mut matched_streak_count = 0_usize;
    let mut streak_open = true;
    let mut last_blocking_event = Value::Null;

    for event in &window {
        let comparison = event
            .payload
            .get("shadow_comparison")
            .unwrap_or(&Value::Null);
        let comparison_status = comparison
            .pointer("/comparison/status")
            .and_then(Value::as_str)
            .unwrap_or("missing");
        let unsafe_mutation_signal = comparison
            .get("codex_mutation_allowed")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            || event
                .payload
                .get("suggested_action")
                .and_then(|action| action.get("mutation_allowed"))
                .and_then(Value::as_bool)
                .unwrap_or(false);

        if streak_open {
            if comparison_status == "matched" && !unsafe_mutation_signal {
                matched_streak_count += 1;
            } else {
                streak_open = false;
            }
        }
        if last_blocking_event.is_null()
            && (comparison_status != "matched" || unsafe_mutation_signal)
        {
            last_blocking_event = assistant_run_codex_shadow_event_summary(event);
        }

        match comparison_status {
            "matched" => matched_count += 1,
            "diverged" => diverged_count += 1,
            "invalid_suggestion" => invalid_count += 1,
            "no_codex_suggestion" => no_suggestion_count += 1,
            _ => no_suggestion_count += 1,
        }
        if unsafe_mutation_signal {
            unsafe_mutation_signal_count += 1;
        }
    }

    let status = if window.len() < MIN_STABLE_SHADOW_EVENTS {
        "insufficient_sample"
    } else if invalid_count > 0 || unsafe_mutation_signal_count > 0 {
        "blocked"
    } else if diverged_count > 0 || no_suggestion_count > 0 {
        "warming"
    } else {
        "eligible_for_host_validation"
    };
    let reason = match status {
        "insufficient_sample" => "not enough Codex shadow diagnostic events",
        "blocked" => "invalid or unsafe Codex shadow suggestion was observed",
        "warming" => "Codex shadow suggestions are not consistently matched yet",
        "eligible_for_host_validation" => {
            "recent Codex shadow suggestions match direct actions; host validation can be considered"
        }
        _ => "unknown shadow gate status",
    };

    json!({
        "status": status,
        "reason": reason,
        "window_size": window.len(),
        "required_stable_events": MIN_STABLE_SHADOW_EVENTS,
        "matched_count": matched_count,
        "diverged_count": diverged_count,
        "invalid_count": invalid_count,
        "no_suggestion_count": no_suggestion_count,
        "unsafe_mutation_signal_count": unsafe_mutation_signal_count,
        "matched_streak_count": matched_streak_count,
        "last_blocking_event": last_blocking_event,
        "host_validation": assistant_run_codex_host_validation_readiness(
            status,
            window.len(),
            matched_streak_count,
            &last_blocking_event,
        ),
        "host_validation_allowed": status == "eligible_for_host_validation",
        "codex_mutation_allowed": false,
        "queue_allowed": false,
        "next_gate": if status == "eligible_for_host_validation" {
            "jump_host_or_mac_host_validation"
        } else {
            "continue_shadow_comparison"
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{AssistantRunEventId, AssistantRunId, TenantId};

    fn event(sequence_no: i32, status: &str, mutation_allowed: bool) -> AssistantRunEvent {
        AssistantRunEvent {
            id: AssistantRunEventId::new(),
            tenant_id: TenantId::new(),
            run_id: AssistantRunId::new(),
            sequence_no,
            event_name: "assistant_run.codex_executor_diagnostic".to_string(),
            payload: json!({
                "transport": "codex_plan_only",
                "provider_secret": "sk-should-not-leak",
                "suggested_action": {
                    "action_type": if status == "invalid_suggestion" {
                        "unsafe_os_command"
                    } else {
                        "create_static_page_draft"
                    },
                    "mutation_allowed": mutation_allowed,
                    "arguments": {
                        "prompt": "raw prompt should not leak"
                    }
                },
                "shadow_comparison": {
                    "codex_mutation_allowed": false,
                    "codex": {
                        "suggested_action_type": if status == "invalid_suggestion" {
                            "unsafe_os_command"
                        } else {
                            "create_static_page_draft"
                        },
                        "suggested_action_allowed": status != "invalid_suggestion"
                    },
                    "comparison": {
                        "status": status,
                        "codex_has_suggestion": true,
                        "actionable": false
                    }
                }
            }),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn shadow_gate_tracks_stability_blocks_unsafe_and_redacts_recent_events() {
        let matched = vec![
            event(1, "matched", false),
            event(2, "matched", false),
            event(3, "matched", false),
        ];
        let matched_refs = matched.iter().collect::<Vec<_>>();
        let eligible = assistant_run_codex_shadow_gate_summary(&matched_refs);

        assert_eq!(eligible["status"], json!("eligible_for_host_validation"));
        assert_eq!(eligible["matched_count"], json!(3));
        assert_eq!(eligible["host_validation_allowed"], json!(true));
        assert_eq!(
            eligible["host_validation"]["ready_for_jump_host_validation"],
            json!(true)
        );
        assert_eq!(eligible["codex_mutation_allowed"], json!(false));
        assert_eq!(eligible["queue_allowed"], json!(false));

        let mut blocked_events = matched;
        blocked_events.push(event(4, "invalid_suggestion", true));
        let blocked_refs = blocked_events.iter().collect::<Vec<_>>();
        let blocked = assistant_run_codex_shadow_gate_summary(&blocked_refs);
        let recent = assistant_run_codex_recent_shadow_events(&blocked_refs, 2);
        let recent_serialized =
            serde_json::to_string(&recent).expect("recent shadow events serialize");

        assert_eq!(blocked["status"], json!("blocked"));
        assert_eq!(blocked["invalid_count"], json!(1));
        assert_eq!(blocked["unsafe_mutation_signal_count"], json!(1));
        assert_eq!(
            blocked["last_blocking_event"]["codex_suggested_action_type"],
            json!("unsafe_os_command")
        );
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0]["sequence_no"], json!(4));
        assert_eq!(recent[0]["mutation_allowed"], json!(true));
        assert!(!recent_serialized.contains("sk-should-not-leak"));
        assert!(!recent_serialized.contains("raw prompt should not leak"));
    }
}
