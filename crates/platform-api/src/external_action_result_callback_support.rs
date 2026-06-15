use chrono::{DateTime, Utc};
use contracts::ExternalActionResultCallbackRequestView;
use serde_json::{json, Map, Value};

use crate::{external_integration_summary::action_result_payload_summary, ApiError};

pub(crate) fn normalize_external_action_result_status(
    status: &str,
) -> std::result::Result<String, ApiError> {
    let normalized = status.trim().to_ascii_lowercase().replace('-', "_");
    let status = match normalized.as_str() {
        "success" | "succeeded" | "complete" | "completed" => "succeeded",
        "fail" | "failed" | "error" => "failed",
        "cancelled" | "canceled" => "cancelled",
        "rejected" => "rejected",
        "running" | "processing" => "running",
        "accepted" => "accepted",
        _ => {
            return Err(ApiError::bad_request_with_details(
                "external_action_result_status_invalid",
                "result callback status must be one of succeeded, failed, cancelled, rejected, running, or accepted"
                    .to_string(),
                json!({
                    "status": status,
                }),
            ))
        }
    };
    Ok(status.to_string())
}

pub(crate) fn external_action_result_failure_kind(status: &str) -> Option<String> {
    match status {
        "failed" | "cancelled" | "rejected" => Some(format!("external_action_{status}")),
        _ => None,
    }
}

pub(crate) fn external_action_result_safe_code(code: Option<&str>) -> Option<String> {
    let code = code?.trim();
    if code.is_empty() || code.len() > 80 {
        return None;
    }
    code.chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
        .then(|| code.to_string())
}

pub(crate) fn external_action_result_callback_summary(
    request: &ExternalActionResultCallbackRequestView,
    status: &str,
    external_request_id: Option<&str>,
    received_at: DateTime<Utc>,
) -> Value {
    let mut callback = Map::new();
    callback.insert("status".to_string(), json!(status));
    callback.insert(
        "idempotency_key".to_string(),
        json!(request.idempotency_key.trim()),
    );
    callback.insert("received_at".to_string(), json!(received_at));
    callback.insert(
        "message_present".to_string(),
        json!(request
            .message
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty())),
    );
    callback.insert(
        "result_present".to_string(),
        json!(request.result.is_some()),
    );
    if let Some(external_request_id) = external_request_id {
        callback.insert(
            "external_request_id".to_string(),
            json!(external_request_id),
        );
    }
    if let Some(completed_at) = request.completed_at {
        callback.insert("completed_at".to_string(), json!(completed_at));
    }
    if let Some(code) = external_action_result_safe_code(request.code.as_deref()) {
        callback.insert("code".to_string(), json!(code));
    }
    if let Some(result) = request.result.clone() {
        callback.insert(
            "result_summary".to_string(),
            action_result_payload_summary(&result),
        );
    }

    json!({
        "status": format!("external_action_{status}"),
        "external_callback": Value::Object(callback),
        "updated_at": received_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn action_result_status_normalizes_aliases_and_rejects_unknown_values() {
        assert_eq!(
            normalize_external_action_result_status(" Completed ").unwrap(),
            "succeeded"
        );
        assert_eq!(
            normalize_external_action_result_status("fail").unwrap(),
            "failed"
        );
        assert_eq!(
            normalize_external_action_result_status("canceled").unwrap(),
            "cancelled"
        );
        assert_eq!(
            normalize_external_action_result_status("processing").unwrap(),
            "running"
        );

        let error = normalize_external_action_result_status("paused")
            .expect_err("unknown status should be rejected");
        assert_eq!(error.payload.code, "external_action_result_status_invalid");
        let details = error.payload.details.expect("error details should exist");
        assert_eq!(details["status"], json!("paused"));
    }

    #[test]
    fn action_result_failure_kind_tracks_only_terminal_failures() {
        assert_eq!(
            external_action_result_failure_kind("failed"),
            Some("external_action_failed".to_string())
        );
        assert_eq!(
            external_action_result_failure_kind("cancelled"),
            Some("external_action_cancelled".to_string())
        );
        assert_eq!(
            external_action_result_failure_kind("rejected"),
            Some("external_action_rejected".to_string())
        );
        assert_eq!(external_action_result_failure_kind("succeeded"), None);
        assert_eq!(external_action_result_failure_kind("running"), None);
    }

    #[test]
    fn action_result_safe_code_trims_and_filters_untrusted_codes() {
        assert_eq!(
            external_action_result_safe_code(Some(" code-1.ok ")),
            Some("code-1.ok".to_string())
        );
        assert_eq!(external_action_result_safe_code(Some("")), None);
        assert_eq!(external_action_result_safe_code(Some("bad code")), None);
        assert_eq!(external_action_result_safe_code(Some("bad/code")), None);
        assert_eq!(
            external_action_result_safe_code(Some(&"a".repeat(81))),
            None
        );
    }

    #[test]
    fn action_result_callback_summary_redacts_payload_values() {
        let received_at = DateTime::parse_from_rfc3339("2026-06-16T08:00:00Z")
            .expect("valid timestamp")
            .with_timezone(&Utc);
        let completed_at = DateTime::parse_from_rfc3339("2026-06-16T08:01:00Z")
            .expect("valid timestamp")
            .with_timezone(&Utc);
        let request = ExternalActionResultCallbackRequestView {
            idempotency_key: " idem-1 ".to_string(),
            status: "completed".to_string(),
            external_request_id: Some("ext-req-1".to_string()),
            message: Some(" contains private customer details ".to_string()),
            code: Some(" ok.done ".to_string()),
            result: Some(json!({
                "customer_name": "Sensitive Name",
                "nested": {
                    "secret": "hidden"
                },
                "rows": [1, 2, 3]
            })),
            completed_at: Some(completed_at),
        };

        let summary = external_action_result_callback_summary(
            &request,
            "succeeded",
            Some("ext-req-1"),
            received_at,
        );
        let summary_text = summary.to_string();

        assert_eq!(summary["status"], json!("external_action_succeeded"));
        assert_eq!(summary["external_callback"]["status"], json!("succeeded"));
        assert_eq!(
            summary["external_callback"]["idempotency_key"],
            json!("idem-1")
        );
        assert_eq!(
            summary["external_callback"]["external_request_id"],
            json!("ext-req-1")
        );
        assert_eq!(summary["external_callback"]["code"], json!("ok.done"));
        assert_eq!(summary["external_callback"]["message_present"], json!(true));
        assert_eq!(summary["external_callback"]["result_present"], json!(true));
        assert_eq!(
            summary["external_callback"]["result_summary"]["kind"],
            json!("object")
        );
        assert_eq!(
            summary["external_callback"]["result_summary"]["field_count"],
            json!(3)
        );
        assert_eq!(
            summary["external_callback"]["result_summary"]["sensitive_field_count"],
            json!(0)
        );
        assert!(!summary_text.contains("Sensitive Name"));
        assert!(!summary_text.contains("hidden"));
        assert!(!summary_text.contains("private customer details"));
    }
}
