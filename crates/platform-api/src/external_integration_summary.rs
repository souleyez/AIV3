use chrono::{DateTime, Utc};
use serde_json::{json, Map, Value};

pub(crate) fn redacted_summary(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .filter(|(key, _)| !sensitive_key(key))
                .map(|(key, value)| (key, redacted_summary(value)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.into_iter().map(redacted_summary).collect()),
        Value::String(text) if text.starts_with("[redacted") => {
            Value::String("[redacted]".to_string())
        }
        other => other,
    }
}

pub(crate) fn sensitive_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    lower.contains("secret")
        || lower.contains("token")
        || lower.contains("authorization")
        || lower.contains("cookie")
        || lower.contains("password")
}

pub(crate) fn has_redacted_value(value: &Value) -> bool {
    match value {
        Value::String(text) => text.starts_with("[redacted"),
        Value::Array(items) => items.iter().any(has_redacted_value),
        Value::Object(map) => map.values().any(has_redacted_value),
        _ => false,
    }
}

pub(crate) fn action_result_payload_summary(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let sensitive_field_count = map.keys().filter(|key| sensitive_key(key)).count();
            json!({
                "kind": "object",
                "field_count": map.len(),
                "sensitive_field_count": sensitive_field_count,
            })
        }
        Value::Array(items) => json!({
            "kind": "array",
            "item_count": items.len(),
        }),
        Value::String(text) => json!({
            "kind": "string",
            "length_chars": text.chars().count(),
        }),
        Value::Number(_) => json!({
            "kind": "number",
        }),
        Value::Bool(_) => json!({
            "kind": "boolean",
        }),
        Value::Null => json!({
            "kind": "null",
        }),
    }
}

pub(crate) fn action_response_summary(response: Option<&Value>, response_text: &str) -> Value {
    if let Some(response) = response {
        return json!({
            "json": action_redacted_response_summary(response),
        });
    }
    json!({
        "text_chars": response_text.chars().count(),
        "body_redacted": true,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn artifact_summary(
    status_action_count: i64,
    publish_action_count: i64,
    revoke_action_count: i64,
    pending_confirmation_count: i64,
    blocked_count: i64,
    failed_count: i64,
    published_count: i64,
    revoked_count: i64,
    latest_artifact_action_at: Option<DateTime<Utc>>,
) -> Value {
    let signal = if pending_confirmation_count > 0 {
        "artifact_confirmation_pending"
    } else if failed_count > 0 {
        "artifact_failed"
    } else if blocked_count > 0 {
        "artifact_blocked"
    } else if revoked_count > 0 {
        "artifact_revoked"
    } else if published_count > 0 {
        "artifact_published"
    } else if status_action_count + publish_action_count + revoke_action_count > 0 {
        "artifact_observed"
    } else {
        "none"
    };
    json!({
        "signal": signal,
        "status_action_count": status_action_count.max(0),
        "publish_action_count": publish_action_count.max(0),
        "revoke_action_count": revoke_action_count.max(0),
        "pending_confirmation_count": pending_confirmation_count.max(0),
        "blocked_count": blocked_count.max(0),
        "failed_count": failed_count.max(0),
        "published_count": published_count.max(0),
        "revoked_count": revoked_count.max(0),
        "latest_artifact_action_at": latest_artifact_action_at,
    })
}

pub(crate) fn search_evidence_summary(
    required_count: i64,
    latest_required_at: Option<DateTime<Utc>>,
) -> Value {
    let signal = if required_count > 0 {
        "search_evidence_required"
    } else {
        "none"
    };
    json!({
        "signal": signal,
        "required_count": required_count.max(0),
        "latest_required_at": latest_required_at,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn action_lifecycle_summary(
    total_action_count: i64,
    pending_confirmation_count: i64,
    blocked_count: i64,
    failed_count: i64,
    waiting_result_count: i64,
    result_callback_count: i64,
    result_succeeded_count: i64,
    result_failed_count: i64,
    result_running_count: i64,
    latest_action_at: Option<DateTime<Utc>>,
    latest_result_callback_at: Option<DateTime<Utc>>,
) -> Value {
    let signal = if result_failed_count > 0 {
        "result_failed"
    } else if blocked_count > 0 {
        "dispatch_blocked"
    } else if failed_count > 0 {
        "dispatch_failed"
    } else if pending_confirmation_count > 0 {
        "confirmation_pending"
    } else if waiting_result_count > 0 {
        "waiting_result"
    } else if result_running_count > 0 {
        "result_running"
    } else if result_succeeded_count > 0 {
        "result_succeeded"
    } else if total_action_count > 0 {
        "action_observed"
    } else {
        "none"
    };
    json!({
        "signal": signal,
        "total_action_count": total_action_count.max(0),
        "pending_confirmation_count": pending_confirmation_count.max(0),
        "blocked_count": blocked_count.max(0),
        "failed_count": failed_count.max(0),
        "waiting_result_count": waiting_result_count.max(0),
        "result_callback_count": result_callback_count.max(0),
        "result_succeeded_count": result_succeeded_count.max(0),
        "result_failed_count": result_failed_count.max(0),
        "result_running_count": result_running_count.max(0),
        "latest_action_at": latest_action_at,
        "latest_result_callback_at": latest_result_callback_at,
    })
}

pub(crate) fn action_run_audit_summary(
    action_type: &str,
    target_system: &str,
    arguments_redacted: &Value,
    confirmation_state: &str,
    external_request_id: Option<&str>,
    result_summary: &Value,
) -> Value {
    let dispatch = result_summary.get("dispatch").unwrap_or(&Value::Null);
    let callback = result_summary
        .get("external_callback")
        .unwrap_or(&Value::Null);
    json!({
        "action_type": action_type,
        "target_system": target_system,
        "is_external_artifact_action": action_type.starts_with("external_artifact."),
        "artifact_ref": arguments_redacted.get("artifact_ref").cloned().unwrap_or(Value::Null),
        "confirmation_state": confirmation_state,
        "action_lifecycle_status": result_summary.get("status").cloned().unwrap_or(Value::Null),
        "external_request_recorded": external_request_id.is_some(),
        "dispatch_status": result_summary.get("status").cloned().unwrap_or(Value::Null),
        "dispatch_reason": dispatch.get("reason").cloned().unwrap_or(Value::Null),
        "dispatch_auth_mode": dispatch.get("auth_mode").cloned().unwrap_or(Value::Null),
        "http_status": dispatch.get("http_status").cloned().unwrap_or(Value::Null),
        "response_summary": dispatch.get("response_summary").cloned().unwrap_or(Value::Null),
        "result_callback_received": callback.is_object(),
        "result_status": callback.get("status").cloned().unwrap_or(Value::Null),
        "callback_idempotency_key": callback.get("idempotency_key").cloned().unwrap_or(Value::Null),
        "callback_completed_at": callback.get("completed_at").cloned().unwrap_or(Value::Null),
        "callback_result_summary": callback.get("result_summary").cloned().unwrap_or(Value::Null),
    })
}

fn action_redacted_response_summary(response: &Value) -> Value {
    match response {
        Value::Object(map) => {
            let mut summary = Map::new();
            for key in [
                "external_request_id",
                "externalRequestId",
                "request_id",
                "requestId",
                "status",
                "code",
            ] {
                if let Some(value) = map.get(key) {
                    summary.insert(key.to_string(), value.clone());
                }
            }
            if map.contains_key("message") {
                summary.insert("message_present".to_string(), json!(true));
            }
            Value::Object(summary)
        }
        _ => Value::Null,
    }
}
