use domain_model::{
    AssistantRunEvent, AssistantRunId, WorkflowEventId, WorkflowEventRecord, WorkflowExecution,
    WorkflowKind,
};
use serde_json::{json, Value};

use crate::{
    assistant_run_answer_quality_autofix_support::assistant_run_answer_quality_autofix_output_validation,
    assistant_run_data_ingestion_output_validation_support::assistant_run_data_ingestion_analysis_output_validation,
    assistant_run_text_support::truncate_assistant_supply_text,
    external_channel_assistant_run_reply_status_url_str, json_value_support::value_array,
};

pub(crate) fn codex_host_fixed_task_bundle_manifest(template_id: &str) -> Value {
    json!({
        "version": 1,
        "template_id": template_id,
        "files": [
            {
                "path": "task.json",
                "kind": "fixed_task_context",
                "required": true,
            },
            {
                "path": "README.md",
                "kind": "instructions",
                "required": true,
            },
            {
                "path": "schemas/output.schema.json",
                "kind": "output_schema",
                "required": true,
            },
            {
                "path": "evidence/summary.json",
                "kind": "evidence_summary",
                "required": false,
            },
            {
                "path": "runtime.json",
                "kind": "runtime_summary",
                "required": true,
            }
        ],
    })
}

pub(crate) fn codex_host_fixed_task_created_event(
    execution: &WorkflowExecution,
    assistant_run_id: AssistantRunId,
) -> WorkflowEventRecord {
    WorkflowEventRecord {
        id: WorkflowEventId::new(),
        execution_id: execution.id,
        sequence_no: 1,
        event_name: "codex_host_task.created".to_string(),
        payload: json!({
            "kind": execution.kind.as_str(),
            "version": execution.version,
            "status": execution.status.as_str(),
            "stage": execution.stage,
            "assistant_run_id": assistant_run_id.to_string(),
            "capability": execution.context.get("capability").cloned().unwrap_or(Value::Null),
            "template_id": execution.context.get("template_id").cloned().unwrap_or(Value::Null),
            "task_memory_policy": execution.context.get("task_memory_policy").cloned().unwrap_or(Value::Null),
            "task_memory_space_id": execution.context.get("task_memory_space_id").cloned().unwrap_or(Value::Null),
        }),
        created_at: execution.created_at,
    }
}

pub(crate) trait JsonObjectExtra {
    fn with_extra(self, extra: Value) -> Value;
}

impl JsonObjectExtra for Value {
    fn with_extra(mut self, extra: Value) -> Value {
        if let (Some(target), Some(extra)) = (self.as_object_mut(), extra.as_object()) {
            for (key, value) in extra {
                target.insert(key.clone(), value.clone());
            }
        }
        self
    }
}

pub(crate) fn codex_host_fixed_task_base_payload(
    execution: &WorkflowExecution,
    assistant_run_id: Option<&str>,
    capability: &str,
    fixed_task: &Value,
    template_id: &str,
    status: &str,
) -> Value {
    let allowed_write_file_count = fixed_task
        .pointer("/allowed_write_scope/files")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let external_status_url = assistant_run_id
        .zip(
            fixed_task
                .pointer("/requirements/channel_connection_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        )
        .map(|(run_id, connection_id)| {
            external_channel_assistant_run_reply_status_url_str(connection_id, run_id)
        });
    let recipient_delivery = fixed_task
        .pointer("/requirements/recipient_delivery")
        .cloned()
        .unwrap_or(Value::Null);
    json!({
        "template_id": template_id,
        "assistant_run_id": assistant_run_id,
        "workflow_execution_id": execution.id.to_string(),
        "capability": capability,
        "status": status,
        "status_url": external_status_url.clone(),
        "status_method": if external_status_url.is_some() { Value::String("GET".to_string()) } else { Value::Null },
        "recipient_delivery": recipient_delivery.clone(),
        "permission_review_status": recipient_delivery
            .get("permission_review_status")
            .cloned()
            .unwrap_or(Value::Null),
        "editable_after_publish": recipient_delivery
            .get("editable_after_publish")
            .cloned()
            .unwrap_or(Value::Null),
        "human_review_policy": fixed_task
            .get("human_review_policy")
            .cloned()
            .unwrap_or(Value::Null),
        "publish_mode": fixed_task
            .pointer("/policies/publish_mode")
            .cloned()
            .unwrap_or(Value::Null),
        "allowed_write_file_count": allowed_write_file_count,
        "raw_prompt_exposed": false,
        "raw_diff_exposed": false,
        "provider_logs_exposed": false,
        "secrets_exposed": false,
    })
}

#[derive(Debug, Clone)]
pub(crate) struct CodexHostFixedTaskAuditEvent {
    pub(crate) event_name: String,
    pub(crate) payload: Value,
    pub(crate) notify_human: bool,
}

pub(crate) fn codex_host_fixed_task_transition_audit_event(
    execution: &WorkflowExecution,
    workflow_event: &WorkflowEventRecord,
    enqueued_task_count: usize,
) -> Option<CodexHostFixedTaskAuditEvent> {
    if execution.kind != WorkflowKind::CodexHostTask {
        return None;
    }
    let fixed_task = execution
        .context
        .get("fixed_task")
        .filter(|value| value.is_object())?;
    let template_id = codex_host_fixed_task_template_id_from_context(&execution.context)?;
    let assistant_run_id = execution
        .context
        .get("assistant_run_id")
        .and_then(Value::as_str)
        .map(str::to_string);
    let capability = execution
        .context
        .get("capability")
        .and_then(Value::as_str)
        .unwrap_or(template_id.as_str());

    if workflow_event.event_name == "workflow.started" {
        let payload = codex_host_fixed_task_base_payload(
            execution,
            assistant_run_id.as_deref(),
            capability,
            fixed_task,
            &template_id,
            "queued",
        )
        .with_extra(json!({
            "workflow_event_name": workflow_event.event_name,
            "enqueued_task_count": enqueued_task_count,
        }));
        return Some(CodexHostFixedTaskAuditEvent {
            event_name: "codex_host.fixed_task.queued".to_string(),
            payload,
            notify_human: false,
        });
    }

    if workflow_event.event_name != "workflow.step_completed" {
        if workflow_event.event_name == "workflow.step_failed" {
            let error = workflow_event
                .payload
                .get("error")
                .and_then(Value::as_str)
                .map(codex_host_fixed_task_safe_text)
                .unwrap_or_else(|| "workflow_step_failed".to_string());
            let payload = codex_host_fixed_task_base_payload(
                execution,
                assistant_run_id.as_deref(),
                capability,
                fixed_task,
                &template_id,
                "failed",
            )
            .with_extra(json!({
                "workflow_event_name": workflow_event.event_name,
                "output": codex_host_fixed_task_output_summary(None),
                "validation": {
                    "accepted": false,
                    "status": "failed",
                    "auto_apply_allowed": false,
                    "reason": "workflow_step_failed",
                    "error": error,
                },
            }));
            return Some(CodexHostFixedTaskAuditEvent {
                event_name: "codex_host.fixed_task.rejected".to_string(),
                payload,
                notify_human: true,
            });
        }
        return None;
    }

    let output = execution.context.get("last_output");
    let extracted =
        output.and_then(|value| codex_host_fixed_task_extract_output(&template_id, value));
    let validation = codex_host_fixed_task_sanitize_validation(
        codex_host_fixed_task_output_validation_summary(&template_id, extracted),
    );
    let output_summary = codex_host_fixed_task_output_summary(extracted);
    let accepted = validation
        .get("accepted")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let output_status = output_summary
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("failed");
    let event_name = if !accepted || output_status == "failed" {
        "codex_host.fixed_task.rejected"
    } else if output_status == "needs_human" {
        "codex_host.fixed_task.needs_human"
    } else {
        "codex_host.fixed_task.completed"
    };
    let status = match event_name {
        "codex_host.fixed_task.completed" => output_status,
        "codex_host.fixed_task.needs_human" => "needs_human",
        _ => "rejected",
    };
    let payload = codex_host_fixed_task_base_payload(
        execution,
        assistant_run_id.as_deref(),
        capability,
        fixed_task,
        &template_id,
        status,
    )
    .with_extra(json!({
        "workflow_event_name": workflow_event.event_name,
        "output": output_summary,
        "validation": validation,
    }));

    Some(CodexHostFixedTaskAuditEvent {
        event_name: event_name.to_string(),
        payload,
        notify_human: event_name != "codex_host.fixed_task.completed",
    })
}

pub(crate) fn assistant_run_codex_fixed_task_event_summary(events: &[AssistantRunEvent]) -> Value {
    let fixed_events = events
        .iter()
        .filter(|event| event.event_name.starts_with("codex_host.fixed_task."))
        .collect::<Vec<_>>();
    let runtime_events = events
        .iter()
        .filter(|event| {
            matches!(
                event.event_name.as_str(),
                "codex_host_task.poll_retry"
                    | "codex_host_task.exec_heartbeat"
                    | "codex_host_task.cloudflare_heartbeat"
                    | "codex_host_task.cancelled"
                    | "codex_host_task.exec_failed"
                    | "codex_host_task.exec_completed"
                    | "codex_host_task.completed"
            )
        })
        .collect::<Vec<_>>();
    let latest = fixed_events.last().copied();
    let latest_runtime = runtime_events.last().copied();
    let recent = fixed_events
        .iter()
        .rev()
        .take(8)
        .map(|event| {
            json!({
                "event_id": event.id.to_string(),
                "sequence_no": event.sequence_no,
                "event_name": event.event_name.clone(),
                "template_id": event.payload.get("template_id").cloned().unwrap_or(Value::Null),
                "status": event.payload.get("status").cloned().unwrap_or(Value::Null),
                "artifact_public_url": event
                    .payload
                    .pointer("/output/artifact_public_url")
                    .cloned()
                    .unwrap_or(Value::Null),
                "changed_file_count": event
                    .payload
                    .pointer("/output/changed_file_count")
                    .cloned()
                    .unwrap_or(Value::Null),
                "test_commands": event
                    .payload
                    .pointer("/output/test_commands")
                    .cloned()
                    .unwrap_or_else(|| json!([])),
                "human_review_reason": event
                    .payload
                    .pointer("/output/human_review_reason")
                    .cloned()
                    .unwrap_or(Value::Null),
                "validation_reason": event
                    .payload
                    .pointer("/validation/reason")
                    .cloned()
                    .unwrap_or(Value::Null),
            })
        })
        .collect::<Vec<_>>();
    let recent_runtime = runtime_events
        .iter()
        .rev()
        .take(8)
        .map(|event| {
            json!({
                "event_id": event.id.to_string(),
                "sequence_no": event.sequence_no,
                "event_name": event.event_name.clone(),
                "status": event.payload.get("status").cloned().unwrap_or(Value::Null),
                "reason": event
                    .payload
                    .get("reason")
                    .and_then(Value::as_str)
                    .map(codex_host_fixed_task_safe_text)
                    .unwrap_or_default(),
                "retryable": event.payload.get("retryable").cloned().unwrap_or(Value::Null),
                "attempt": event.payload.get("attempt").cloned().unwrap_or(Value::Null),
                "max_attempts": event.payload.get("max_attempts").cloned().unwrap_or(Value::Null),
                "available_at": event.payload.get("available_at").cloned().unwrap_or(Value::Null),
                "elapsed_ms": event.payload.get("elapsed_ms").cloned().unwrap_or(Value::Null),
                "heartbeat_count": event.payload.get("heartbeat_count").cloned().unwrap_or(Value::Null),
                "secrets_exposed": event
                    .payload
                    .get("secrets_exposed")
                    .cloned()
                    .unwrap_or(Value::Bool(false)),
            })
        })
        .collect::<Vec<_>>();
    json!({
        "event_count": fixed_events.len(),
        "queued_count": fixed_events.iter().filter(|event| event.event_name == "codex_host.fixed_task.queued").count(),
        "completed_count": fixed_events.iter().filter(|event| event.event_name == "codex_host.fixed_task.completed").count(),
        "needs_human_count": fixed_events.iter().filter(|event| event.event_name == "codex_host.fixed_task.needs_human").count(),
        "rejected_count": fixed_events.iter().filter(|event| event.event_name == "codex_host.fixed_task.rejected").count(),
        "poll_retry_count": runtime_events.iter().filter(|event| event.event_name == "codex_host_task.poll_retry").count(),
        "cancelled_count": runtime_events.iter().filter(|event| event.event_name == "codex_host_task.cancelled").count(),
        "exec_failed_count": runtime_events.iter().filter(|event| event.event_name == "codex_host_task.exec_failed").count(),
        "heartbeat_count": runtime_events
            .iter()
            .filter(|event| matches!(
                event.event_name.as_str(),
                "codex_host_task.exec_heartbeat" | "codex_host_task.cloudflare_heartbeat"
            ))
            .count(),
        "exec_completed_count": runtime_events.iter().filter(|event| event.event_name == "codex_host_task.exec_completed").count(),
        "latest": latest.map(|event| {
            json!({
                "event_name": event.event_name.clone(),
                "template_id": event.payload.get("template_id").cloned().unwrap_or(Value::Null),
                "status": event.payload.get("status").cloned().unwrap_or(Value::Null),
                "validation_reason": event.payload.pointer("/validation/reason").cloned().unwrap_or(Value::Null),
            })
        }).unwrap_or(Value::Null),
        "latest_runtime": latest_runtime.map(|event| {
            json!({
                "event_name": event.event_name.clone(),
                "status": event.payload.get("status").cloned().unwrap_or(Value::Null),
                "reason": event
                    .payload
                    .get("reason")
                    .and_then(Value::as_str)
                    .map(codex_host_fixed_task_safe_text)
                    .unwrap_or_default(),
                "retryable": event.payload.get("retryable").cloned().unwrap_or(Value::Null),
                "attempt": event.payload.get("attempt").cloned().unwrap_or(Value::Null),
                "max_attempts": event.payload.get("max_attempts").cloned().unwrap_or(Value::Null),
                "available_at": event.payload.get("available_at").cloned().unwrap_or(Value::Null),
            })
        }).unwrap_or(Value::Null),
        "recent": recent,
        "recent_runtime": recent_runtime,
    })
}

pub(crate) fn codex_host_fixed_task_public_artifact_url_allowed(public_url: &str) -> bool {
    let normalized = public_url.trim();
    if normalized.contains("/generated-artifacts/pending-")
        || normalized.contains("/generated-artifacts/pending/")
        || normalized.ends_with("/generated-artifacts/pending")
    {
        return false;
    }
    normalized.starts_with("https://v3.elepcloud.com/generated-artifacts/")
        || normalized.starts_with("/generated-artifacts/")
}

pub(crate) fn codex_host_fixed_task_sanitize_validation(mut validation: Value) -> Value {
    if let Some(reason) = validation.get("reason").and_then(Value::as_str) {
        validation["reason"] = Value::String(codex_host_fixed_task_safe_text(reason));
    }
    validation
}

pub(crate) fn codex_host_fixed_task_safe_text(value: &str) -> String {
    let compact = truncate_assistant_supply_text(value, 500);
    let lowered = compact.to_ascii_lowercase();
    if lowered.contains("database_url")
        || lowered.contains("postgres://")
        || lowered.contains("mysql://")
        || lowered.contains("sk-")
        || lowered.contains("api_token")
        || lowered.contains("authorization:")
        || lowered.contains("bearer ")
    {
        "[redacted]".to_string()
    } else {
        compact
    }
}

pub(crate) fn codex_host_fixed_task_value_contains_sensitive_text(value: &Value) -> bool {
    match value {
        Value::String(text) => codex_host_fixed_task_text_is_sensitive(text),
        Value::Array(values) => values
            .iter()
            .any(codex_host_fixed_task_value_contains_sensitive_text),
        Value::Object(map) => map.iter().any(|(key, value)| {
            codex_host_fixed_task_text_is_sensitive(key)
                || codex_host_fixed_task_value_contains_sensitive_text(value)
        }),
        _ => false,
    }
}

fn codex_host_fixed_task_text_is_sensitive(value: &str) -> bool {
    let lowered = value.to_ascii_lowercase();
    lowered.contains("database_url")
        || lowered.contains("postgres://")
        || lowered.contains("mysql://")
        || lowered.contains("api_key")
        || lowered.contains("api_token")
        || lowered.contains("authorization:")
        || lowered.contains("bearer ")
        || lowered.contains("password=")
        || lowered.contains("sk-")
}

pub(crate) fn codex_host_fixed_task_output_summary(output: Option<&Value>) -> Value {
    let Some(output) = output else {
        return json!({
            "status": "missing",
            "artifact_public_url": Value::Null,
            "changed_file_count": 0,
            "test_commands": [],
            "human_review_reason": "fixed_task_output_missing",
        });
    };
    let changed_files = value_array(output.get("changed_files").cloned().unwrap_or(Value::Null));
    let source_summary = value_array(output.get("source_summary").cloned().unwrap_or(Value::Null));
    let validation_checks = value_array(
        output
            .get("validation_checks")
            .cloned()
            .unwrap_or(Value::Null),
    );
    let recommended_next_actions = value_array(
        output
            .get("recommended_next_actions")
            .cloned()
            .unwrap_or(Value::Null),
    );
    let test_commands = value_array(output.get("test_commands").cloned().unwrap_or(Value::Null))
        .into_iter()
        .filter_map(|value| value.as_str().map(codex_host_fixed_task_safe_text))
        .collect::<Vec<_>>();
    let warnings = value_array(
        output
            .pointer("/validation_report/warnings")
            .cloned()
            .unwrap_or(Value::Null),
    )
    .into_iter()
    .filter_map(|value| value.as_str().map(codex_host_fixed_task_safe_text))
    .collect::<Vec<_>>();
    json!({
        "status": output.get("status").and_then(Value::as_str).unwrap_or("unknown"),
        "artifact_public_url": output
            .pointer("/artifact/public_url")
            .and_then(Value::as_str)
            .filter(|url| codex_host_fixed_task_public_artifact_url_allowed(url))
            .map(Value::from)
            .unwrap_or(Value::Null),
        "latest_snapshot": output
            .pointer("/validation_report/latest_snapshot")
            .cloned()
            .unwrap_or(Value::Null),
        "source_row_count": output
            .pointer("/validation_report/source_row_count")
            .cloned()
            .unwrap_or(Value::Null),
        "current_state_row_count": output
            .pointer("/validation_report/current_state_row_count")
            .cloned()
            .unwrap_or(Value::Null),
        "detail_row_count": output
            .pointer("/validation_report/detail_row_count")
            .cloned()
            .unwrap_or(Value::Null),
        "source_summary_count": source_summary.len(),
        "data_quality_report_present": output.get("data_quality_report").is_some_and(Value::is_object),
        "mapping_plan_present": output.get("mapping_plan").is_some_and(Value::is_object),
        "staging_spec_present": output.get("staging_spec").is_some(),
        "validation_checks_count": validation_checks.len(),
        "recommended_next_actions_count": recommended_next_actions.len(),
        "unit_policy": output
            .pointer("/validation_report/unit_policy")
            .cloned()
            .unwrap_or(Value::Null),
        "changed_file_count": changed_files.len(),
        "tests_added_count": value_array(output.get("tests_added").cloned().unwrap_or(Value::Null)).len(),
        "test_commands": test_commands,
        "risk_level": output.get("risk_level").cloned().unwrap_or(Value::Null),
        "failure_type": output.get("failure_type").cloned().unwrap_or(Value::Null),
        "rollback_notes_present": output
            .get("rollback_notes")
            .and_then(Value::as_str)
            .map(str::trim)
            .is_some_and(|notes| !notes.is_empty()),
        "human_review_reason": output
            .get("human_review_reason")
            .and_then(Value::as_str)
            .map(codex_host_fixed_task_safe_text)
            .unwrap_or_else(|| "".to_string()),
        "warnings": warnings,
    })
}

pub(crate) fn codex_host_fixed_task_extract_output<'a>(
    template_id: &str,
    output: &'a Value,
) -> Option<&'a Value> {
    if output.get("template_id").and_then(Value::as_str) == Some(template_id) {
        return Some(output);
    }
    for field in [
        "fixed_task_output",
        "fixedTaskOutput",
        "template_output",
        "templateOutput",
        "result",
        "output",
    ] {
        if let Some(value) = output.get(field) {
            if let Some(extracted) = codex_host_fixed_task_extract_output(template_id, value) {
                return Some(extracted);
            }
        }
    }
    None
}

pub(crate) fn codex_host_fixed_task_template_id_from_context(context: &Value) -> Option<String> {
    context
        .get("template_id")
        .and_then(Value::as_str)
        .or_else(|| {
            context
                .pointer("/fixed_task/template_id")
                .and_then(Value::as_str)
        })
        .map(str::to_string)
}

pub(crate) fn codex_host_fixed_task_output_validation_summary(
    template_id: &str,
    output: Option<&Value>,
) -> Value {
    let Some(output) = output else {
        return json!({
            "accepted": false,
            "status": "needs_human",
            "auto_apply_allowed": false,
            "reason": "fixed_task_output_missing"
        });
    };
    if output.get("template_id").and_then(Value::as_str) != Some(template_id) {
        return json!({
            "accepted": false,
            "status": "needs_human",
            "auto_apply_allowed": false,
            "reason": "template_id_mismatch"
        });
    }
    match template_id {
        "answer_quality_autofix" => assistant_run_answer_quality_autofix_output_validation(output),
        "data_ingestion_analysis" => {
            assistant_run_data_ingestion_analysis_output_validation(output)
        }
        "static_page_image2_data_publish" => {
            let status = output
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("failed");
            if !matches!(status, "success" | "needs_human" | "failed") {
                return json!({
                    "accepted": false,
                    "status": "needs_human",
                    "auto_apply_allowed": false,
                    "reason": "unknown_status"
                });
            }
            if status == "needs_human" {
                return json!({
                    "accepted": true,
                    "status": "needs_human",
                    "auto_apply_allowed": false,
                    "reason": output
                        .get("human_review_reason")
                        .and_then(Value::as_str)
                        .unwrap_or("human_review_requested")
                });
            }
            if status == "failed" {
                return json!({
                    "accepted": true,
                    "status": "failed",
                    "auto_apply_allowed": false,
                    "reason": output
                        .get("human_review_reason")
                        .and_then(Value::as_str)
                        .unwrap_or("host_failed")
                });
            }
            let public_url = output
                .pointer("/artifact/public_url")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if !codex_host_fixed_task_public_artifact_url_allowed(public_url) {
                return json!({
                    "accepted": false,
                    "status": "needs_human",
                    "auto_apply_allowed": false,
                    "reason": "artifact_public_url_not_generated_artifact"
                });
            }
            if output.get("validation_report").is_none() {
                return json!({
                    "accepted": false,
                    "status": "needs_human",
                    "auto_apply_allowed": false,
                    "reason": "validation_report_required"
                });
            }
            json!({
                "accepted": true,
                "status": "success",
                "auto_apply_allowed": true,
                "reason": "new_generated_artifact_validated"
            })
        }
        _ => json!({
            "accepted": false,
            "status": "needs_human",
            "auto_apply_allowed": false,
            "reason": "unknown_template_id"
        }),
    }
}

pub(crate) fn external_channel_static_page_html_from_fixed_task_output(
    output: &Value,
) -> Option<String> {
    for pointer in [
        "/artifact/html",
        "/artifact/html_text",
        "/artifact/index_html",
        "/html",
        "/html_text",
    ] {
        if let Some(html) = output
            .pointer(pointer)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(external_channel_standalone_html_document(html));
        }
    }
    None
}

fn external_channel_standalone_html_document(html: &str) -> String {
    let trimmed = html.trim();
    let lower = trimmed
        .chars()
        .take(256)
        .collect::<String>()
        .to_ascii_lowercase();
    if lower.contains("<!doctype html") || lower.contains("<html") {
        trimmed.to_string()
    } else {
        format!(
            "<!doctype html><html lang=\"zh-CN\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>DataMax Generated Artifact</title></head><body>{}</body></html>",
            trimmed
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{
        AssistantRunEventId, AssistantRunId, TenantId, WorkflowExecutionId, WorkflowKind,
        WorkflowStatus,
    };

    fn event(sequence_no: i32, event_name: &str, payload: Value) -> AssistantRunEvent {
        AssistantRunEvent {
            id: AssistantRunEventId::new(),
            tenant_id: TenantId::new(),
            run_id: AssistantRunId::new(),
            sequence_no,
            event_name: event_name.to_string(),
            payload,
            created_at: Utc::now(),
        }
    }

    fn workflow_event(
        execution_id: WorkflowExecutionId,
        event_name: &str,
        payload: Value,
    ) -> WorkflowEventRecord {
        WorkflowEventRecord {
            id: WorkflowEventId::new(),
            execution_id,
            sequence_no: 2,
            event_name: event_name.to_string(),
            payload,
            created_at: Utc::now(),
        }
    }

    fn codex_task_execution(context: Value) -> WorkflowExecution {
        let now = Utc::now();
        WorkflowExecution {
            id: WorkflowExecutionId::new(),
            tenant_id: TenantId::new(),
            dataset_id: None,
            report_plan_id: None,
            kind: WorkflowKind::CodexHostTask,
            version: "v1".to_string(),
            stage: "queued".to_string(),
            status: WorkflowStatus::Pending,
            attempt: 0,
            context,
            created_at: now,
            updated_at: now,
        }
    }

    fn static_page_fixed_task_context(
        assistant_run_id: AssistantRunId,
        last_output: Value,
    ) -> Value {
        json!({
            "assistant_run_id": assistant_run_id.to_string(),
            "capability": "static_page",
            "template_id": "static_page_image2_data_publish",
            "fixed_task": {
                "requirements": {
                    "channel_connection_id": "generic-chat-main",
                    "recipient_delivery": {
                        "permission_review_status": "approved",
                        "editable_after_publish": true
                    }
                },
                "allowed_write_scope": {"files": ["artifact/index.html"]},
                "human_review_policy": "notify_only",
                "policies": {"publish_mode": "new_generated_artifact_only"}
            },
            "last_output": last_output
        })
    }

    #[test]
    fn codex_host_fixed_task_bundle_manifest_preserves_required_files() {
        assert_eq!(
            codex_host_fixed_task_bundle_manifest("answer_quality_autofix"),
            json!({
                "version": 1,
                "template_id": "answer_quality_autofix",
                "files": [
                    {
                        "path": "task.json",
                        "kind": "fixed_task_context",
                        "required": true
                    },
                    {
                        "path": "README.md",
                        "kind": "instructions",
                        "required": true
                    },
                    {
                        "path": "schemas/output.schema.json",
                        "kind": "output_schema",
                        "required": true
                    },
                    {
                        "path": "evidence/summary.json",
                        "kind": "evidence_summary",
                        "required": false
                    },
                    {
                        "path": "runtime.json",
                        "kind": "runtime_summary",
                        "required": true
                    }
                ]
            })
        );
    }

    #[test]
    fn codex_host_fixed_task_created_event_copies_safe_context_fields() {
        let now = Utc::now();
        let assistant_run_id = AssistantRunId::new();
        let execution = WorkflowExecution {
            id: WorkflowExecutionId::new(),
            tenant_id: TenantId::new(),
            dataset_id: None,
            report_plan_id: None,
            kind: WorkflowKind::CodexHostTask,
            version: "v1".to_string(),
            stage: "queued".to_string(),
            status: WorkflowStatus::Pending,
            attempt: 0,
            context: json!({
                "capability": "customer_artifact_request",
                "template_id": "customer_artifacts",
                "task_memory_policy": {"mode": "task_scoped"},
                "task_memory_space_id": "space-1",
                "raw_prompt": "not copied"
            }),
            created_at: now,
            updated_at: now,
        };

        let event = codex_host_fixed_task_created_event(&execution, assistant_run_id);

        assert_eq!(event.execution_id, execution.id);
        assert_eq!(event.sequence_no, 1);
        assert_eq!(event.event_name, "codex_host_task.created");
        assert_eq!(event.created_at, now);
        assert_eq!(event.payload["kind"], json!("codex_host_task_workflow"));
        assert_eq!(event.payload["version"], json!("v1"));
        assert_eq!(event.payload["status"], json!("pending"));
        assert_eq!(event.payload["stage"], json!("queued"));
        assert_eq!(
            event.payload["assistant_run_id"],
            json!(assistant_run_id.to_string())
        );
        assert_eq!(
            event.payload["capability"],
            json!("customer_artifact_request")
        );
        assert_eq!(event.payload["template_id"], json!("customer_artifacts"));
        assert_eq!(
            event.payload["task_memory_policy"],
            json!({"mode": "task_scoped"})
        );
        assert_eq!(event.payload["task_memory_space_id"], json!("space-1"));
        assert!(event.payload.get("raw_prompt").is_none());
    }

    #[test]
    fn json_object_extra_merges_object_fields_and_overwrites_existing_keys() {
        let merged = json!({
            "status": "queued",
            "workflow_event_name": "workflow.started"
        })
        .with_extra(json!({
            "status": "success",
            "output": {"changed_file_count": 2}
        }));

        assert_eq!(merged["status"], json!("success"));
        assert_eq!(merged["workflow_event_name"], json!("workflow.started"));
        assert_eq!(merged["output"], json!({"changed_file_count": 2}));
    }

    #[test]
    fn json_object_extra_ignores_non_object_inputs() {
        assert_eq!(
            json!({"status": "queued"}).with_extra(Value::Null),
            json!({"status": "queued"})
        );
        assert_eq!(
            Value::Null.with_extra(json!({"status": "success"})),
            Value::Null
        );
    }

    #[test]
    fn codex_host_fixed_task_base_payload_copies_safe_control_fields() {
        let now = Utc::now();
        let execution = WorkflowExecution {
            id: WorkflowExecutionId::new(),
            tenant_id: TenantId::new(),
            dataset_id: None,
            report_plan_id: None,
            kind: WorkflowKind::CodexHostTask,
            version: "v1".to_string(),
            stage: "queued".to_string(),
            status: WorkflowStatus::Pending,
            attempt: 0,
            context: json!({}),
            created_at: now,
            updated_at: now,
        };
        let assistant_run_id = AssistantRunId::new().to_string();
        let fixed_task = json!({
            "requirements": {
                "channel_connection_id": "generic-chat-main",
                "recipient_delivery": {
                    "permission_review_status": "approved",
                    "editable_after_publish": true
                }
            },
            "allowed_write_scope": {
                "files": ["artifact/index.html", "artifact/data.json"]
            },
            "human_review_policy": "notify_only",
            "policies": {
                "publish_mode": "new_generated_artifact_only"
            },
            "raw_prompt": "sk-should-not-leak"
        });

        let payload = codex_host_fixed_task_base_payload(
            &execution,
            Some(&assistant_run_id),
            "static_page",
            &fixed_task,
            "static_page_image2_data_publish",
            "queued",
        );
        let serialized = payload.to_string();

        assert_eq!(
            payload["template_id"],
            json!("static_page_image2_data_publish")
        );
        assert_eq!(payload["assistant_run_id"], json!(assistant_run_id));
        assert_eq!(
            payload["workflow_execution_id"],
            json!(execution.id.to_string())
        );
        assert_eq!(payload["capability"], json!("static_page"));
        assert_eq!(payload["status"], json!("queued"));
        assert!(payload["status_url"]
            .as_str()
            .unwrap_or_default()
            .ends_with(&format!(
                "/v1/external/channels/generic-chat-main/assistant-runs/{}/reply",
                assistant_run_id
            )));
        assert_eq!(payload["status_method"], json!("GET"));
        assert_eq!(payload["permission_review_status"], json!("approved"));
        assert_eq!(payload["editable_after_publish"], json!(true));
        assert_eq!(payload["human_review_policy"], json!("notify_only"));
        assert_eq!(
            payload["publish_mode"],
            json!("new_generated_artifact_only")
        );
        assert_eq!(payload["allowed_write_file_count"], json!(2));
        assert_eq!(payload["raw_prompt_exposed"], json!(false));
        assert_eq!(payload["raw_diff_exposed"], json!(false));
        assert_eq!(payload["provider_logs_exposed"], json!(false));
        assert_eq!(payload["secrets_exposed"], json!(false));
        assert!(!serialized.contains("sk-should-not-leak"));
    }

    #[test]
    fn codex_host_fixed_task_base_payload_uses_nulls_without_delivery_context() {
        let now = Utc::now();
        let execution = WorkflowExecution {
            id: WorkflowExecutionId::new(),
            tenant_id: TenantId::new(),
            dataset_id: None,
            report_plan_id: None,
            kind: WorkflowKind::CodexHostTask,
            version: "v1".to_string(),
            stage: "queued".to_string(),
            status: WorkflowStatus::Pending,
            attempt: 0,
            context: json!({}),
            created_at: now,
            updated_at: now,
        };

        let payload = codex_host_fixed_task_base_payload(
            &execution,
            None,
            "answer_quality",
            &json!({}),
            "answer_quality_autofix",
            "rejected",
        );

        assert_eq!(payload["assistant_run_id"], Value::Null);
        assert_eq!(payload["status_url"], Value::Null);
        assert_eq!(payload["status_method"], Value::Null);
        assert_eq!(payload["recipient_delivery"], Value::Null);
        assert_eq!(payload["permission_review_status"], Value::Null);
        assert_eq!(payload["editable_after_publish"], Value::Null);
        assert_eq!(payload["human_review_policy"], Value::Null);
        assert_eq!(payload["publish_mode"], Value::Null);
        assert_eq!(payload["allowed_write_file_count"], json!(0));
    }

    #[test]
    fn codex_host_fixed_task_transition_audit_event_emits_queued_payload() {
        let assistant_run_id = AssistantRunId::new();
        let execution = codex_task_execution(static_page_fixed_task_context(
            assistant_run_id,
            Value::Null,
        ));
        let event = workflow_event(execution.id, "workflow.started", json!({}));

        let audit = codex_host_fixed_task_transition_audit_event(&execution, &event, 3)
            .expect("queued audit event");

        assert_eq!(audit.event_name, "codex_host.fixed_task.queued");
        assert_eq!(audit.payload["status"], json!("queued"));
        assert_eq!(
            audit.payload["workflow_event_name"],
            json!("workflow.started")
        );
        assert_eq!(audit.payload["enqueued_task_count"], json!(3));
        assert_eq!(
            audit.payload["template_id"],
            json!("static_page_image2_data_publish")
        );
        assert_eq!(audit.payload["allowed_write_file_count"], json!(1));
        assert!(!audit.notify_human);
    }

    #[test]
    fn codex_host_fixed_task_transition_audit_event_accepts_valid_static_page_output() {
        let assistant_run_id = AssistantRunId::new();
        let execution = codex_task_execution(static_page_fixed_task_context(
            assistant_run_id,
            json!({
                "template_id": "static_page_image2_data_publish",
                "status": "success",
                "artifact": {
                    "public_url": "https://v3.elepcloud.com/generated-artifacts/static-page/index.html"
                },
                "validation_report": {
                    "source_row_count": 12,
                    "warnings": []
                }
            }),
        ));
        let event = workflow_event(execution.id, "workflow.step_completed", json!({}));

        let audit = codex_host_fixed_task_transition_audit_event(&execution, &event, 0)
            .expect("completed audit event");

        assert_eq!(audit.event_name, "codex_host.fixed_task.completed");
        assert_eq!(audit.payload["status"], json!("success"));
        assert_eq!(audit.payload["validation"]["accepted"], json!(true));
        assert_eq!(
            audit.payload["validation"]["auto_apply_allowed"],
            json!(true)
        );
        assert_eq!(
            audit.payload["output"]["artifact_public_url"],
            json!("https://v3.elepcloud.com/generated-artifacts/static-page/index.html")
        );
        assert!(!audit.notify_human);
    }

    #[test]
    fn codex_host_fixed_task_transition_audit_event_rejects_failed_step_safely() {
        let assistant_run_id = AssistantRunId::new();
        let execution = codex_task_execution(static_page_fixed_task_context(
            assistant_run_id,
            Value::Null,
        ));
        let event = workflow_event(
            execution.id,
            "workflow.step_failed",
            json!({"error": "provider returned sk-should-not-leak"}),
        );

        let audit = codex_host_fixed_task_transition_audit_event(&execution, &event, 0)
            .expect("rejected audit event");

        assert_eq!(audit.event_name, "codex_host.fixed_task.rejected");
        assert_eq!(audit.payload["status"], json!("failed"));
        assert_eq!(
            audit.payload["validation"]["reason"],
            json!("workflow_step_failed")
        );
        assert_eq!(audit.payload["validation"]["error"], json!("[redacted]"));
        assert!(audit.notify_human);
        assert!(!audit.payload.to_string().contains("sk-should-not-leak"));
    }

    #[test]
    fn assistant_run_codex_fixed_task_event_summary_counts_and_redacts_runtime() {
        let events = vec![
            event(
                1,
                "codex_host.fixed_task.queued",
                json!({
                    "template_id": "static_page_generation",
                    "status": "queued",
                    "raw_prompt": "sk-should-not-leak"
                }),
            ),
            event(
                2,
                "codex_host_task.poll_retry",
                json!({
                    "status": "processing",
                    "reason": "retry after provider key sk-secret-value",
                    "attempt": 1,
                    "max_attempts": 3,
                    "secrets_exposed": true,
                    "raw_error": "raw-provider-payload-should-not-leak"
                }),
            ),
        ];

        let summary = assistant_run_codex_fixed_task_event_summary(&events);
        let serialized = summary.to_string();

        assert_eq!(summary["event_count"], json!(1));
        assert_eq!(summary["queued_count"], json!(1));
        assert_eq!(summary["poll_retry_count"], json!(1));
        assert_eq!(
            summary["latest"]["template_id"],
            json!("static_page_generation")
        );
        assert_eq!(summary["latest_runtime"]["attempt"], json!(1));
        assert_eq!(summary["recent_runtime"][0]["secrets_exposed"], json!(true));
        assert!(summary["latest_runtime"]["reason"]
            .as_str()
            .unwrap_or_default()
            .contains("[redacted]"));
        assert!(!serialized.contains("sk-should-not-leak"));
        assert!(!serialized.contains("raw-provider-payload-should-not-leak"));
    }

    #[test]
    fn codex_host_fixed_task_output_summary_reports_missing_output() {
        assert_eq!(
            codex_host_fixed_task_output_summary(None),
            json!({
                "status": "missing",
                "artifact_public_url": Value::Null,
                "changed_file_count": 0,
                "test_commands": [],
                "human_review_reason": "fixed_task_output_missing"
            })
        );
    }

    #[test]
    fn codex_host_fixed_task_output_summary_counts_and_redacts_safe_fields() {
        let summary = codex_host_fixed_task_output_summary(Some(&json!({
            "status": "success",
            "artifact": {
                "public_url": "https://v3.elepcloud.com/generated-artifacts/database-static-pages/final/index.html"
            },
            "validation_report": {
                "latest_snapshot": "2026-06-30",
                "source_row_count": 12,
                "current_state_row_count": 10,
                "detail_row_count": 2,
                "unit_policy": "raw_value_first",
                "warnings": ["ok", "bearer token should be redacted"]
            },
            "source_summary": ["rows sampled"],
            "data_quality_report": {"row_count": 12},
            "mapping_plan": {"fields": []},
            "staging_spec": [{"table": "target"}],
            "validation_checks": ["row_count_check"],
            "recommended_next_actions": ["confirm staging"],
            "changed_files": ["README.md", "report.md"],
            "tests_added": ["fixed_task_output_summary"],
            "test_commands": ["cargo test ok", "curl -H 'Authorization: bearer secret'"],
            "risk_level": "low",
            "failure_type": "none",
            "rollback_notes": "revert generated artifact",
            "human_review_reason": "postgres://secret should redact"
        })));

        assert_eq!(summary["status"], json!("success"));
        assert_eq!(
            summary["artifact_public_url"],
            json!("https://v3.elepcloud.com/generated-artifacts/database-static-pages/final/index.html")
        );
        assert_eq!(summary["latest_snapshot"], json!("2026-06-30"));
        assert_eq!(summary["source_row_count"], json!(12));
        assert_eq!(summary["source_summary_count"], json!(1));
        assert_eq!(summary["data_quality_report_present"], json!(true));
        assert_eq!(summary["mapping_plan_present"], json!(true));
        assert_eq!(summary["staging_spec_present"], json!(true));
        assert_eq!(summary["validation_checks_count"], json!(1));
        assert_eq!(summary["recommended_next_actions_count"], json!(1));
        assert_eq!(summary["changed_file_count"], json!(2));
        assert_eq!(summary["tests_added_count"], json!(1));
        assert_eq!(summary["test_commands"][0], json!("cargo test ok"));
        assert_eq!(summary["test_commands"][1], json!("[redacted]"));
        assert_eq!(summary["warnings"][0], json!("ok"));
        assert_eq!(summary["warnings"][1], json!("[redacted]"));
        assert_eq!(summary["human_review_reason"], json!("[redacted]"));
        assert_eq!(summary["rollback_notes_present"], json!(true));
    }

    #[test]
    fn codex_host_fixed_task_output_summary_filters_pending_or_external_artifacts() {
        for public_url in [
            "https://v3.elepcloud.com/generated-artifacts/pending-draft-1/index.html",
            "https://example.com/generated-artifacts/final/index.html",
        ] {
            let summary = codex_host_fixed_task_output_summary(Some(&json!({
                "status": "success",
                "artifact": {"public_url": public_url}
            })));

            assert_eq!(summary["artifact_public_url"], Value::Null);
        }
    }

    #[test]
    fn codex_host_fixed_task_extract_output_reads_direct_or_nested_template_output() {
        let direct = json!({
            "template_id": "static_page_image2_data_publish",
            "status": "success"
        });
        assert_eq!(
            codex_host_fixed_task_extract_output("static_page_image2_data_publish", &direct),
            Some(&direct)
        );

        let nested = json!({
            "result": {
                "fixedTaskOutput": {
                    "template_id": "data_ingestion_analysis",
                    "status": "analysis_ready"
                }
            }
        });
        let extracted = codex_host_fixed_task_extract_output("data_ingestion_analysis", &nested)
            .expect("nested output should be found");
        assert_eq!(extracted["status"], json!("analysis_ready"));
    }

    #[test]
    fn codex_host_fixed_task_extract_output_rejects_template_mismatch_or_missing_output() {
        let mismatched = json!({
            "output": {
                "template_id": "other_template",
                "status": "success"
            }
        });
        assert!(codex_host_fixed_task_extract_output("expected_template", &mismatched).is_none());

        let missing = json!({
            "status": "success",
            "output": {"status": "success"}
        });
        assert!(codex_host_fixed_task_extract_output("expected_template", &missing).is_none());
    }

    #[test]
    fn codex_host_fixed_task_template_id_from_context_reads_top_level_or_fixed_task() {
        assert_eq!(
            codex_host_fixed_task_template_id_from_context(&json!({
                "template_id": "static_page_image2_data_publish",
                "fixed_task": {"template_id": "ignored_when_top_level_present"}
            })),
            Some("static_page_image2_data_publish".to_string())
        );
        assert_eq!(
            codex_host_fixed_task_template_id_from_context(&json!({
                "fixed_task": {"template_id": "data_ingestion_analysis"}
            })),
            Some("data_ingestion_analysis".to_string())
        );
    }

    #[test]
    fn codex_host_fixed_task_template_id_from_context_rejects_missing_or_non_string_values() {
        assert_eq!(
            codex_host_fixed_task_template_id_from_context(&json!({})),
            None
        );
        assert_eq!(
            codex_host_fixed_task_template_id_from_context(&json!({
                "template_id": 123,
                "fixed_task": {"template_id": false}
            })),
            None
        );
    }

    #[test]
    fn codex_host_fixed_task_output_validation_summary_rejects_missing_mismatch_or_unknown() {
        assert_eq!(
            codex_host_fixed_task_output_validation_summary(
                "static_page_image2_data_publish",
                None
            )["reason"],
            json!("fixed_task_output_missing")
        );
        assert_eq!(
            codex_host_fixed_task_output_validation_summary(
                "static_page_image2_data_publish",
                Some(&json!({"template_id": "other"}))
            )["reason"],
            json!("template_id_mismatch")
        );
        assert_eq!(
            codex_host_fixed_task_output_validation_summary(
                "unknown_template",
                Some(&json!({"template_id": "unknown_template"}))
            )["reason"],
            json!("unknown_template_id")
        );
    }

    #[test]
    fn codex_host_fixed_task_output_validation_summary_accepts_static_page_success() {
        let decision = codex_host_fixed_task_output_validation_summary(
            "static_page_image2_data_publish",
            Some(&json!({
                "template_id": "static_page_image2_data_publish",
                "status": "success",
                "artifact": {
                    "public_url": "https://v3.elepcloud.com/generated-artifacts/database-static-pages/final/index.html"
                },
                "validation_report": {"latest_snapshot": "2026-06-30"}
            })),
        );

        assert_eq!(decision["accepted"], json!(true));
        assert_eq!(decision["status"], json!("success"));
        assert_eq!(decision["auto_apply_allowed"], json!(true));
        assert_eq!(
            decision["reason"],
            json!("new_generated_artifact_validated")
        );
    }

    #[test]
    fn codex_host_fixed_task_output_validation_summary_rejects_static_page_invalid_success() {
        let bad_url = codex_host_fixed_task_output_validation_summary(
            "static_page_image2_data_publish",
            Some(&json!({
                "template_id": "static_page_image2_data_publish",
                "status": "success",
                "artifact": {"public_url": "https://example.com/report/index.html"},
                "validation_report": {"latest_snapshot": "2026-06-30"}
            })),
        );
        assert_eq!(
            bad_url["reason"],
            json!("artifact_public_url_not_generated_artifact")
        );

        let missing_validation = codex_host_fixed_task_output_validation_summary(
            "static_page_image2_data_publish",
            Some(&json!({
                "template_id": "static_page_image2_data_publish",
                "status": "success",
                "artifact": {
                    "public_url": "https://v3.elepcloud.com/generated-artifacts/database-static-pages/final/index.html"
                }
            })),
        );
        assert_eq!(
            missing_validation["reason"],
            json!("validation_report_required")
        );
    }

    #[test]
    fn codex_host_fixed_task_output_validation_summary_passes_static_page_terminal_status() {
        let needs_human = codex_host_fixed_task_output_validation_summary(
            "static_page_image2_data_publish",
            Some(&json!({
                "template_id": "static_page_image2_data_publish",
                "status": "needs_human",
                "human_review_reason": "operator_review"
            })),
        );
        assert_eq!(needs_human["accepted"], json!(true));
        assert_eq!(needs_human["status"], json!("needs_human"));
        assert_eq!(needs_human["auto_apply_allowed"], json!(false));
        assert_eq!(needs_human["reason"], json!("operator_review"));

        let failed = codex_host_fixed_task_output_validation_summary(
            "static_page_image2_data_publish",
            Some(&json!({
                "template_id": "static_page_image2_data_publish",
                "status": "failed"
            })),
        );
        assert_eq!(failed["accepted"], json!(true));
        assert_eq!(failed["status"], json!("failed"));
        assert_eq!(failed["auto_apply_allowed"], json!(false));
        assert_eq!(failed["reason"], json!("host_failed"));
    }

    #[test]
    fn codex_host_fixed_task_output_validation_summary_rejects_static_page_unknown_status() {
        let decision = codex_host_fixed_task_output_validation_summary(
            "static_page_image2_data_publish",
            Some(&json!({
                "template_id": "static_page_image2_data_publish",
                "status": "ready"
            })),
        );

        assert_eq!(decision["accepted"], json!(false));
        assert_eq!(decision["status"], json!("needs_human"));
        assert_eq!(decision["reason"], json!("unknown_status"));
    }

    #[test]
    fn external_channel_static_page_html_from_fixed_task_output_wraps_fragment() {
        let html = external_channel_static_page_html_from_fixed_task_output(&json!({
            "artifact": {
                "html": "<main>报表内容</main>"
            }
        }))
        .expect("html");

        assert!(html.starts_with("<!doctype html><html lang=\"zh-CN\">"));
        assert!(html.contains("<body><main>报表内容</main></body>"));
    }

    #[test]
    fn external_channel_static_page_html_from_fixed_task_output_preserves_full_document() {
        let document = "<!doctype html><html><body>完整页面</body></html>";
        let html = external_channel_static_page_html_from_fixed_task_output(&json!({
            "artifact": {
                "index_html": format!("  {document}  ")
            }
        }))
        .expect("html");

        assert_eq!(html, document);
    }

    #[test]
    fn external_channel_static_page_html_from_fixed_task_output_reads_top_level_aliases() {
        let html = external_channel_static_page_html_from_fixed_task_output(&json!({
            "html_text": "<section>备用字段</section>"
        }))
        .expect("html");

        assert!(html.contains("<section>备用字段</section>"));
    }

    #[test]
    fn external_channel_static_page_html_from_fixed_task_output_rejects_missing_or_empty_html() {
        assert!(external_channel_static_page_html_from_fixed_task_output(&json!({})).is_none());
        assert!(
            external_channel_static_page_html_from_fixed_task_output(&json!({
                "artifact": {"html": "   "}
            }))
            .is_none()
        );
    }

    #[test]
    fn codex_host_fixed_task_public_artifact_url_allows_only_generated_artifacts() {
        assert!(codex_host_fixed_task_public_artifact_url_allowed(
            "https://v3.elepcloud.com/generated-artifacts/database-static-pages/final/index.html"
        ));
        assert!(codex_host_fixed_task_public_artifact_url_allowed(
            "/generated-artifacts/database-static-pages/final/index.html"
        ));
        assert!(codex_host_fixed_task_public_artifact_url_allowed(
            "  /generated-artifacts/database-static-pages/final/index.html  "
        ));
        assert!(!codex_host_fixed_task_public_artifact_url_allowed(
            "https://v3.elepcloud.com/generated-artifacts/pending/draft-1/"
        ));
        assert!(!codex_host_fixed_task_public_artifact_url_allowed(
            "https://v3.elepcloud.com/generated-artifacts/pending-draft-1/index.html"
        ));
        assert!(!codex_host_fixed_task_public_artifact_url_allowed(
            "https://example.com/generated-artifacts/database-static-pages/final/index.html"
        ));
    }

    #[test]
    fn codex_host_fixed_task_sanitize_validation_redacts_reason_only() {
        let sanitized = codex_host_fixed_task_sanitize_validation(json!({
            "accepted": false,
            "status": "needs_human",
            "reason": "DATABASE_URL=postgres://secret",
            "raw_reason": "DATABASE_URL=postgres://not-inspected"
        }));

        assert_eq!(sanitized["reason"], json!("[redacted]"));
        assert_eq!(
            sanitized["raw_reason"],
            json!("DATABASE_URL=postgres://not-inspected")
        );
    }

    #[test]
    fn codex_host_fixed_task_sanitize_validation_keeps_safe_or_non_string_reason() {
        let safe = codex_host_fixed_task_sanitize_validation(json!({
            "reason": "validation_report_required",
        }));
        assert_eq!(safe["reason"], json!("validation_report_required"));

        let non_string = codex_host_fixed_task_sanitize_validation(json!({
            "reason": 123,
        }));
        assert_eq!(non_string["reason"], json!(123));
    }

    #[test]
    fn codex_host_fixed_task_safe_text_redacts_sensitive_markers() {
        for value in [
            "DATABASE_URL=postgres://user:pass@example.invalid/db",
            "postgres://user:pass@example.invalid/db",
            "mysql://user:pass@example.invalid/db",
            "sk-test-secret",
            "api_token=test-token",
            "Authorization: Bearer token",
            "bearer token",
        ] {
            assert_eq!(codex_host_fixed_task_safe_text(value), "[redacted]");
        }
    }

    #[test]
    fn codex_host_fixed_task_safe_text_preserves_safe_compacted_excerpt() {
        let safe = codex_host_fixed_task_safe_text("  validation\tpassed\nwith warning  ");
        assert_eq!(safe, "validation passed with warning");

        let long = codex_host_fixed_task_safe_text(&"a".repeat(700));
        assert_eq!(long.chars().count(), 500);
    }

    #[test]
    fn codex_host_fixed_task_value_contains_sensitive_text_detects_sensitive_strings() {
        for value in [
            "DATABASE_URL=postgres://user:pass@example.invalid/db",
            "postgres://user:pass@example.invalid/db",
            "mysql://user:pass@example.invalid/db",
            "api_key=test-key",
            "api_token=test-token",
            "Authorization: Bearer token",
            "bearer token",
            "password=secret",
            "sk-test-secret",
        ] {
            assert!(
                codex_host_fixed_task_value_contains_sensitive_text(&json!(value)),
                "expected sensitive text to be detected: {value}"
            );
        }
    }

    #[test]
    fn codex_host_fixed_task_value_contains_sensitive_text_detects_nested_values_and_keys() {
        assert!(codex_host_fixed_task_value_contains_sensitive_text(
            &json!({
                "source_summary": [
                    {"label": "safe"},
                    {"example": "mysql://user:pass@example.invalid/db"}
                ]
            })
        ));

        assert!(codex_host_fixed_task_value_contains_sensitive_text(
            &json!({
                "DATABASE_URL": "redacted elsewhere"
            })
        ));
    }

    #[test]
    fn codex_host_fixed_task_value_contains_sensitive_text_allows_safe_values() {
        for value in [
            Value::Null,
            json!(123),
            json!("source summary only"),
            json!({"source_summary": ["row samples available"], "warnings": []}),
        ] {
            assert!(!codex_host_fixed_task_value_contains_sensitive_text(&value));
        }
    }
}
