use domain_model::AssistantRunEvent;
use serde_json::{json, Value};

pub(crate) fn assistant_run_codex_host_validation_readiness(
    gate_status: &str,
    window_size: usize,
    matched_streak_count: usize,
    last_blocking_event: &Value,
) -> Value {
    let ready = gate_status == "eligible_for_host_validation";
    let blocked_by = if ready {
        Value::Null
    } else if gate_status == "insufficient_sample" {
        json!("insufficient_shadow_sample")
    } else if !last_blocking_event.is_null() {
        last_blocking_event
            .get("comparison_status")
            .cloned()
            .unwrap_or_else(|| json!("shadow_gate_not_stable"))
    } else {
        json!("shadow_gate_not_stable")
    };

    json!({
        "ready_for_jump_host_validation": ready,
        "ready_for_mac_host_validation": ready,
        "required_host_kinds": ["windows_jump", "mac_host"],
        "candidate_transports": [
            "codex_exec_schema",
            "codex_sdk_thread",
            "codex_app_server"
        ],
        "local_execution_allowed": false,
        "direct_execution_authoritative": true,
        "codex_mutation_allowed_before_host_validation": false,
        "queue_allowed_before_host_validation": false,
        "requires_task_workspace_root": true,
        "requires_real_exec_allow_flag": true,
        "requires_v3_action_validation": true,
        "shadow_window_size": window_size,
        "matched_streak_count": matched_streak_count,
        "blocked_by": blocked_by,
        "next_step": if ready {
            "run_jump_host_smoke_with_codex_host_agent"
        } else {
            "continue_shadow_comparison_until_stable"
        },
    })
}

pub(crate) fn assistant_run_codex_host_validation_results(
    events: &[AssistantRunEvent],
    limit: usize,
) -> Vec<Value> {
    events
        .iter()
        .rev()
        .filter_map(assistant_run_codex_host_validation_event_summary)
        .take(limit)
        .collect()
}

pub(crate) fn assistant_run_codex_host_validation_summary(results: &[Value]) -> Value {
    let completed_count = results
        .iter()
        .filter(|result| assistant_run_codex_host_validation_result_completed(result))
        .count();
    let invalid_host_count = results
        .iter()
        .filter(|result| {
            result.get("mode").and_then(Value::as_str) == Some("codex_exec")
                && result.get("status").and_then(Value::as_str) == Some("completed")
                && !result
                    .get("host_kind_allowed")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
        })
        .count();
    let guard_failed_count = results
        .iter()
        .filter(|result| {
            result.get("mode").and_then(Value::as_str) == Some("codex_exec")
                && result.get("status").and_then(Value::as_str) == Some("completed")
                && result
                    .get("host_kind_allowed")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                && !assistant_run_codex_host_validation_result_completed(result)
        })
        .count();
    let failed_count = results
        .iter()
        .filter(|result| {
            result.get("mode").and_then(Value::as_str) == Some("codex_exec")
                && !assistant_run_codex_host_validation_result_completed(result)
        })
        .count();
    let latest = results.first();
    let latest_status = latest
        .map(|result| {
            if assistant_run_codex_host_validation_result_completed(result) {
                "validated"
            } else if result.get("mode").and_then(Value::as_str) == Some("codex_exec")
                && result.get("status").and_then(Value::as_str) == Some("completed")
                && !result
                    .get("host_kind_allowed")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            {
                "invalid_host"
            } else if result.get("mode").and_then(Value::as_str) == Some("codex_exec") {
                "failed"
            } else {
                "observed_non_exec"
            }
        })
        .unwrap_or("not_run");
    let next_step = match latest_status {
        "validated" => "review_host_report_then_consider_feature_gate_promotion",
        "invalid_host" => "rerun_codex_host_smoke_on_windows_jump_or_mac_host",
        "failed" => "inspect_redacted_codex_host_report_and_retry_on_jump_host",
        _ => "run_jump_host_smoke_with_codex_host_agent_when_shadow_gate_ready",
    };
    let latest_summary = latest
        .map(|result| {
            json!({
                "mode": result.get("mode").cloned().unwrap_or(Value::Null),
                "status": result.get("status").cloned().unwrap_or(Value::Null),
                "host_kind": result.get("host_kind").cloned().unwrap_or(Value::Null),
                "host_kind_allowed": result
                    .get("host_kind_allowed")
                    .cloned()
                    .unwrap_or(Value::Bool(false)),
                "capability": result.get("capability").cloned().unwrap_or(Value::Null),
                "codex_invoked": result
                    .get("codex_invoked")
                    .cloned()
                    .unwrap_or(Value::Bool(false)),
                "validation_requirements_met": assistant_run_codex_host_validation_result_completed(result),
                "profile_kind": result.pointer("/profile/kind").cloned().unwrap_or(Value::Null),
                "model": result.pointer("/profile/model").cloned().unwrap_or(Value::Null),
                "workspace_configured": result
                    .pointer("/command_plan/workspace_configured")
                    .cloned()
                    .unwrap_or(Value::Bool(false)),
                "prompt_redacted": result
                    .pointer("/command_plan/prompt_redacted")
                    .cloned()
                    .unwrap_or(Value::Bool(false)),
                "exit_code": result.pointer("/process/exit_code").cloned().unwrap_or(Value::Null),
                "task_memory_isolated": result
                    .get("task_memory_isolated")
                    .cloned()
                    .unwrap_or(Value::Bool(false)),
                "task_memory_space_configured": result
                    .get("task_memory_space_configured")
                    .cloned()
                    .unwrap_or(Value::Bool(false)),
                "html_artifact_count": result
                    .get("html_artifact_count")
                    .cloned()
                    .unwrap_or(Value::Null),
            })
        })
        .unwrap_or(Value::Null);

    json!({
        "status": latest_status,
        "result_count": results.len(),
        "completed_count": completed_count,
        "failed_count": failed_count,
        "invalid_host_count": invalid_host_count,
        "guard_failed_count": guard_failed_count,
        "latest": latest_summary,
        "direct_execution_authoritative": true,
        "local_execution_allowed": false,
        "codex_mutation_allowed": false,
        "queue_allowed": false,
        "next_step": next_step,
    })
}

fn assistant_run_codex_host_validation_event_summary(event: &AssistantRunEvent) -> Option<Value> {
    let output = assistant_run_codex_host_output_payload(&event.payload)?;
    let mode = output
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    if !event.event_name.starts_with("codex_host")
        && !event.event_name.contains("codex_host")
        && mode != "codex_exec"
    {
        return None;
    }
    let status = output
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let host_kind = output.get("host_kind").and_then(Value::as_str);
    let host_kind_allowed = assistant_run_codex_host_kind_is_allowed(host_kind);
    let process = output.get("process").unwrap_or(&Value::Null);
    let command_plan = output.get("command_plan").unwrap_or(&Value::Null);
    let profile = output.get("profile").unwrap_or(&Value::Null);
    let codex_invoked = output
        .get("codex_invoked")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let workspace_configured = command_plan
        .get("workspace_configured")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let prompt_redacted = command_plan
        .get("prompt_redacted")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let exit_code_is_zero = process
        .get("exit_code")
        .and_then(Value::as_i64)
        .is_some_and(|exit_code| exit_code == 0);
    let task_memory_isolated = output
        .get("task_memory_isolated")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let task_memory_space_configured = output.get("task_memory_space_id").is_some();
    let host_validation_completed = mode == "codex_exec"
        && status == "completed"
        && host_kind_allowed
        && codex_invoked
        && workspace_configured
        && prompt_redacted
        && exit_code_is_zero
        && task_memory_isolated
        && task_memory_space_configured;
    let stdout_chars = process
        .get("stdout_chars")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .or_else(|| {
            process
                .get("stdout_excerpt")
                .and_then(Value::as_str)
                .map(|value| value.chars().count())
        })
        .unwrap_or(0);
    let stderr_chars = process
        .get("stderr_chars")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .or_else(|| {
            process
                .get("stderr_excerpt")
                .and_then(Value::as_str)
                .map(|value| value.chars().count())
        })
        .unwrap_or(0);
    let html_artifact_count = output
        .get("html_artifacts")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);

    Some(json!({
        "event_id": event.id.to_string(),
        "sequence_no": event.sequence_no,
        "event_name": event.event_name.clone(),
        "mode": mode,
        "status": status,
        "host_kind": host_kind,
        "host_kind_allowed": host_kind_allowed,
        "host_validation_completed": host_validation_completed,
        "codex_invoked": codex_invoked,
        "capability": output.get("capability").cloned().unwrap_or(Value::Null),
        "profile": {
            "id": profile.get("id").cloned().unwrap_or(Value::Null),
            "kind": profile.get("kind").cloned().unwrap_or(Value::Null),
            "model": profile.get("model").cloned().unwrap_or(Value::Null),
            "provider_id": profile.get("provider_id").cloned().unwrap_or(Value::Null),
            "wire_api": profile.get("wire_api").cloned().unwrap_or(Value::Null),
            "base_url_configured": profile
                .get("base_url_configured")
                .cloned()
                .unwrap_or(Value::Bool(false)),
            "allowed_capability_count": profile
                .get("allowed_capabilities")
                .and_then(Value::as_array)
                .map(Vec::len)
                .unwrap_or(0),
        },
        "command_plan": {
            "program": command_plan.get("program").cloned().unwrap_or(Value::Null),
            "sandbox": command_plan.get("sandbox").cloned().unwrap_or(Value::Null),
            "prompt_chars": command_plan.get("prompt_chars").cloned().unwrap_or(Value::Null),
            "workspace_configured": command_plan
                .get("workspace_configured")
                .cloned()
                .unwrap_or(Value::Bool(false)),
            "workspace_label": command_plan
                .get("workspace_label")
                .cloned()
                .unwrap_or(Value::Null),
            "prompt_redacted": command_plan
                .get("prompt_redacted")
                .cloned()
                .unwrap_or(Value::Bool(true)),
        },
        "process": {
            "exit_code": process.get("exit_code").cloned().unwrap_or(Value::Null),
            "stdout_chars": stdout_chars,
            "stderr_chars": stderr_chars,
        },
        "task_chars": output.get("task_chars").cloned().unwrap_or(Value::Null),
        "local_thread_id": output
            .get("local_thread_id")
            .cloned()
            .unwrap_or(Value::Null),
        "task_memory_isolated": output
            .get("task_memory_isolated")
            .cloned()
            .unwrap_or(Value::Bool(false)),
        "task_memory_space_configured": task_memory_space_configured,
        "html_artifact_count": html_artifact_count,
        "raw_logs_exposed": false,
    }))
}

fn assistant_run_codex_host_validation_result_completed(result: &Value) -> bool {
    result.get("mode").and_then(Value::as_str) == Some("codex_exec")
        && result.get("status").and_then(Value::as_str) == Some("completed")
        && result
            .get("host_kind_allowed")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        && result
            .get("codex_invoked")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        && result
            .pointer("/command_plan/workspace_configured")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        && result
            .pointer("/command_plan/prompt_redacted")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        && result
            .pointer("/process/exit_code")
            .and_then(Value::as_i64)
            .is_some_and(|exit_code| exit_code == 0)
        && result
            .get("task_memory_isolated")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        && result
            .get("task_memory_space_configured")
            .and_then(Value::as_bool)
            .unwrap_or(false)
}

fn assistant_run_codex_host_kind_is_allowed(host_kind: Option<&str>) -> bool {
    matches!(
        host_kind,
        Some("windows_jump" | "mac_host" | "cloudflare_codex")
    )
}

fn assistant_run_codex_host_output_payload(payload: &Value) -> Option<&Value> {
    if payload.get("mode").and_then(Value::as_str).is_some() {
        return Some(payload);
    }
    let output = payload.get("output")?;
    if output.get("mode").and_then(Value::as_str).is_some() {
        Some(output)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{AssistantRunEventId, AssistantRunId, TenantId};

    #[test]
    fn host_validation_results_collect_nested_output_without_raw_logs() {
        let event = AssistantRunEvent {
            id: AssistantRunEventId::new(),
            tenant_id: TenantId::new(),
            run_id: AssistantRunId::new(),
            sequence_no: 7,
            event_name: "codex_host_task.completed".to_string(),
            payload: json!({
                "output": {
                    "mode": "codex_exec",
                    "status": "completed",
                    "host_kind": "windows_jump",
                    "codex_invoked": true,
                    "capability": "inspect_project",
                    "profile": {
                        "kind": "codex-compatible-shim",
                        "model": "MiniMax-M2.7",
                        "env_key": "MINIMAX_API_KEY"
                    },
                    "command_plan": {
                        "workspace_configured": true,
                        "prompt_redacted": true,
                        "program": "codex"
                    },
                    "process": {
                        "exit_code": 0,
                        "stdout_excerpt": "sk-should-not-leak"
                    },
                    "task_memory_isolated": true,
                    "task_memory_space_id": "run-memory",
                    "html_artifacts": [{"path": "index.html"}]
                }
            }),
            created_at: Utc::now(),
        };

        let results = assistant_run_codex_host_validation_results(&[event], 8);
        let summary = assistant_run_codex_host_validation_summary(&results);
        let serialized = summary.to_string();

        assert_eq!(results.len(), 1);
        assert_eq!(summary["status"], json!("validated"));
        assert_eq!(summary["completed_count"], json!(1));
        assert_eq!(summary["latest"]["host_kind"], json!("windows_jump"));
        assert_eq!(
            summary["latest"]["validation_requirements_met"],
            json!(true)
        );
        assert_eq!(summary["latest"]["html_artifact_count"], json!(1));
        assert!(!serialized.contains("MINIMAX_API_KEY"));
        assert!(!serialized.contains("sk-should-not-leak"));
    }
}
