use serde_json::{json, Value};

use crate::assistant_run_codex_fixed_task_support::codex_host_fixed_task_safe_text;

pub(crate) fn external_channel_static_page_publish_validation_summary_from_fixed_task_output(
    fixed_task_output: &Value,
) -> Value {
    let report = fixed_task_output
        .get("validation_report")
        .unwrap_or(&Value::Null);
    json!({
        "status": fixed_task_output.get("status").cloned().unwrap_or(Value::Null),
        "reason": "new_generated_artifact_validated",
        "latest_snapshot": report.get("latest_snapshot").cloned().unwrap_or(Value::Null),
        "source_row_count": report.get("source_row_count").cloned().unwrap_or(Value::Null),
        "current_state_row_count": report
            .get("current_state_row_count")
            .cloned()
            .unwrap_or(Value::Null),
        "detail_row_count": report.get("detail_row_count").cloned().unwrap_or(Value::Null),
        "unit_policy": report.get("unit_policy").cloned().unwrap_or(Value::Null),
        "warnings": report
            .get("warnings")
            .cloned()
            .unwrap_or_else(|| json!([])),
    })
}

pub(crate) fn external_channel_static_page_publish_validation_summary(payload: &Value) -> Value {
    let output = payload.get("output").unwrap_or(&Value::Null);
    let validation = payload.get("validation").unwrap_or(&Value::Null);
    json!({
        "status": payload.get("status").cloned().unwrap_or(Value::Null),
        "reason": validation
            .get("reason")
            .and_then(Value::as_str)
            .map(codex_host_fixed_task_safe_text)
            .unwrap_or_default(),
        "latest_snapshot": output
            .get("latest_snapshot")
            .cloned()
            .unwrap_or(Value::Null),
        "source_row_count": output
            .get("source_row_count")
            .cloned()
            .unwrap_or(Value::Null),
        "current_state_row_count": output
            .get("current_state_row_count")
            .cloned()
            .unwrap_or(Value::Null),
        "detail_row_count": output
            .get("detail_row_count")
            .cloned()
            .unwrap_or(Value::Null),
        "unit_policy": output
            .get("unit_policy")
            .cloned()
            .unwrap_or(Value::Null),
        "warnings": output
            .get("warnings")
            .cloned()
            .unwrap_or_else(|| json!([])),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn fixed_task_output_summary_reads_validation_report() {
        let summary =
            external_channel_static_page_publish_validation_summary_from_fixed_task_output(
                &json!({
                    "status": "completed",
                    "validation_report": {
                        "latest_snapshot": "2026-06-01",
                        "source_row_count": 120,
                        "current_state_row_count": 80,
                        "detail_row_count": 40,
                        "unit_policy": "validate_raw_value_then_choose_wan_or_yi",
                        "warnings": ["missing_area"]
                    }
                }),
            );

        assert_eq!(summary["status"], json!("completed"));
        assert_eq!(summary["reason"], json!("new_generated_artifact_validated"));
        assert_eq!(summary["latest_snapshot"], json!("2026-06-01"));
        assert_eq!(summary["source_row_count"], json!(120));
        assert_eq!(summary["current_state_row_count"], json!(80));
        assert_eq!(summary["detail_row_count"], json!(40));
        assert_eq!(
            summary["unit_policy"],
            json!("validate_raw_value_then_choose_wan_or_yi")
        );
        assert_eq!(summary["warnings"], json!(["missing_area"]));
    }

    #[test]
    fn fixed_task_output_summary_defaults_missing_report_fields() {
        let summary =
            external_channel_static_page_publish_validation_summary_from_fixed_task_output(
                &json!({"status": "completed"}),
            );

        assert_eq!(summary["status"], json!("completed"));
        assert_eq!(summary["latest_snapshot"], Value::Null);
        assert_eq!(summary["warnings"], json!([]));
    }

    #[test]
    fn event_payload_summary_sanitizes_reason_and_reads_output() {
        let summary = external_channel_static_page_publish_validation_summary(&json!({
            "status": "static_page_publish_failed",
            "validation": {
                "reason": "provider returned authorization: bearer sk-secret"
            },
            "output": {
                "latest_snapshot": "2026-06-02",
                "source_row_count": 220,
                "current_state_row_count": 180,
                "detail_row_count": 60,
                "unit_policy": "raw",
                "warnings": ["retryable"]
            }
        }));

        assert_eq!(summary["status"], json!("static_page_publish_failed"));
        assert_eq!(summary["reason"], json!("[redacted]"));
        assert_eq!(summary["latest_snapshot"], json!("2026-06-02"));
        assert_eq!(summary["source_row_count"], json!(220));
        assert_eq!(summary["current_state_row_count"], json!(180));
        assert_eq!(summary["detail_row_count"], json!(60));
        assert_eq!(summary["unit_policy"], json!("raw"));
        assert_eq!(summary["warnings"], json!(["retryable"]));
    }
}
