use serde_json::{json, Value};

use crate::{
    assistant_run_codex_fixed_task_support::codex_host_fixed_task_value_contains_sensitive_text,
    json_value_support::value_array,
};

pub(crate) fn assistant_run_data_ingestion_analysis_output_validation(output: &Value) -> Value {
    if output.get("template_id").and_then(Value::as_str) != Some("data_ingestion_analysis") {
        return json!({
            "accepted": false,
            "status": "needs_human",
            "auto_apply_allowed": false,
            "reason": "template_id_mismatch"
        });
    }
    let status = output
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("failed");
    if !matches!(
        status,
        "analysis_ready" | "staging_spec_ready" | "needs_human" | "failed"
    ) {
        return json!({
            "accepted": false,
            "status": "needs_human",
            "auto_apply_allowed": false,
            "reason": "unknown_status"
        });
    }
    if codex_host_fixed_task_value_contains_sensitive_text(output) {
        return json!({
            "accepted": false,
            "status": "needs_human",
            "auto_apply_allowed": false,
            "reason": "sensitive_connection_or_credential_text_detected"
        });
    }
    let unsafe_change_requested = [
        "production_write_requested",
        "credential_request_detected",
        "public_api_change_requested",
        "schema_change_requested",
    ]
    .iter()
    .any(|key| output.get(key).and_then(Value::as_bool) == Some(true));
    if unsafe_change_requested {
        return json!({
            "accepted": true,
            "status": "needs_human",
            "auto_apply_allowed": false,
            "reason": output
                .get("human_review_reason")
                .and_then(Value::as_str)
                .unwrap_or("unsafe_data_ingestion_change_requires_human_review")
        });
    }
    if status == "needs_human" || status == "failed" {
        return json!({
            "accepted": true,
            "status": status,
            "auto_apply_allowed": false,
            "reason": output
                .get("human_review_reason")
                .and_then(Value::as_str)
                .unwrap_or(status)
        });
    }
    let source_summary = value_array(output.get("source_summary").cloned().unwrap_or(Value::Null));
    let validation_checks = value_array(
        output
            .get("validation_checks")
            .cloned()
            .unwrap_or(Value::Null),
    );
    if source_summary.is_empty()
        || validation_checks.is_empty()
        || !output
            .get("data_quality_report")
            .is_some_and(Value::is_object)
    {
        return json!({
            "accepted": false,
            "status": "needs_human",
            "auto_apply_allowed": false,
            "reason": "data_quality_report_source_summary_and_validation_checks_required"
        });
    }
    if status == "staging_spec_ready"
        && !output
            .get("staging_spec")
            .is_some_and(|value| value.is_object() || value.is_array())
    {
        return json!({
            "accepted": false,
            "status": "needs_human",
            "auto_apply_allowed": false,
            "reason": "staging_spec_required"
        });
    }
    json!({
        "accepted": true,
        "status": status,
        "auto_apply_allowed": true,
        "reason": "read_only_data_ingestion_analysis_validated"
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_analysis_ready_output() {
        let decision = assistant_run_data_ingestion_analysis_output_validation(&json!({
            "template_id": "data_ingestion_analysis",
            "status": "analysis_ready",
            "source_summary": ["考勤表 sample rows available"],
            "data_quality_report": {
                "row_count": 128,
                "warnings": ["日期列需规范化"]
            },
            "mapping_plan": {
                "fields": [
                    {"source": "员工姓名", "target": "employee_name", "confidence": "high"}
                ]
            },
            "validation_checks": ["date_parse_check", "work_hour_range_check"],
            "recommended_next_actions": ["生成 staging import spec 后由 DataMax 审核执行"],
            "human_review_reason": null
        }));

        assert_eq!(decision["accepted"], json!(true));
        assert_eq!(decision["status"], json!("analysis_ready"));
        assert_eq!(decision["auto_apply_allowed"], json!(true));
    }

    #[test]
    fn requires_human_review_for_unsafe_changes() {
        let decision = assistant_run_data_ingestion_analysis_output_validation(&json!({
            "template_id": "data_ingestion_analysis",
            "status": "analysis_ready",
            "source_summary": ["用户要求直接覆盖生产表"],
            "data_quality_report": {"row_count": 128, "warnings": []},
            "validation_checks": ["production_write_guard"],
            "production_write_requested": true,
            "schema_change_requested": true,
            "human_review_reason": "production_write_or_schema_change_requires_confirmation"
        }));

        assert_eq!(decision["accepted"], json!(true));
        assert_eq!(decision["status"], json!("needs_human"));
        assert_eq!(decision["auto_apply_allowed"], json!(false));
        assert_eq!(
            decision["reason"],
            json!("production_write_or_schema_change_requires_confirmation")
        );
    }

    #[test]
    fn rejects_sensitive_connection_text() {
        let decision = assistant_run_data_ingestion_analysis_output_validation(&json!({
            "template_id": "data_ingestion_analysis",
            "status": "analysis_ready",
            "source_summary": ["postgres://user:pass@example.invalid/db"],
            "data_quality_report": {"row_count": 128, "warnings": []},
            "validation_checks": ["date_parse_check"]
        }));

        assert_eq!(decision["accepted"], json!(false));
        assert_eq!(decision["status"], json!("needs_human"));
        assert_eq!(
            decision["reason"],
            json!("sensitive_connection_or_credential_text_detected")
        );
    }

    #[test]
    fn rejects_template_id_mismatch() {
        let decision = assistant_run_data_ingestion_analysis_output_validation(&json!({
            "template_id": "other",
            "status": "analysis_ready"
        }));

        assert_eq!(decision["accepted"], json!(false));
        assert_eq!(decision["reason"], json!("template_id_mismatch"));
    }

    #[test]
    fn rejects_unknown_status() {
        let decision = assistant_run_data_ingestion_analysis_output_validation(&json!({
            "template_id": "data_ingestion_analysis",
            "status": "ready",
        }));

        assert_eq!(decision["accepted"], json!(false));
        assert_eq!(decision["reason"], json!("unknown_status"));
    }

    #[test]
    fn accepts_failed_and_needs_human_without_auto_apply() {
        for status in ["failed", "needs_human"] {
            let decision = assistant_run_data_ingestion_analysis_output_validation(&json!({
                "template_id": "data_ingestion_analysis",
                "status": status,
                "human_review_reason": "manual review"
            }));

            assert_eq!(decision["accepted"], json!(true));
            assert_eq!(decision["status"], json!(status));
            assert_eq!(decision["auto_apply_allowed"], json!(false));
            assert_eq!(decision["reason"], json!("manual review"));
        }
    }

    #[test]
    fn requires_quality_report_source_summary_and_validation_checks() {
        let decision = assistant_run_data_ingestion_analysis_output_validation(&json!({
            "template_id": "data_ingestion_analysis",
            "status": "analysis_ready",
            "source_summary": [],
            "data_quality_report": {"row_count": 128},
            "validation_checks": ["date_parse_check"]
        }));

        assert_eq!(decision["accepted"], json!(false));
        assert_eq!(
            decision["reason"],
            json!("data_quality_report_source_summary_and_validation_checks_required")
        );
    }

    #[test]
    fn staging_spec_ready_requires_staging_spec() {
        let decision = assistant_run_data_ingestion_analysis_output_validation(&json!({
            "template_id": "data_ingestion_analysis",
            "status": "staging_spec_ready",
            "source_summary": ["考勤表 sample rows available"],
            "data_quality_report": {"row_count": 128},
            "validation_checks": ["date_parse_check"]
        }));

        assert_eq!(decision["accepted"], json!(false));
        assert_eq!(decision["reason"], json!("staging_spec_required"));
    }
}
