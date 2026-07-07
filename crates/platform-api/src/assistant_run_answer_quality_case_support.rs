use contracts::CreateAssistantRunRequest;
use domain_model::AssistantRunId;
use serde_json::{json, Value};

use crate::{
    assistant_run_answer_quality_budget_support::{
        assistant_run_request_expresses_dissatisfaction,
        assistant_run_request_expresses_strong_complaint,
    },
    assistant_run_answer_quality_judge_support::assistant_run_answer_contains_insufficient_evidence_marker,
    assistant_run_answer_quality_retry_support::assistant_run_answer_quality_retry_reason,
    assistant_run_react_support::assistant_run_assistant_message_content_from_artifacts,
    assistant_run_supply_quality_support::{
        assistant_run_answer_quality_case_supply_sources,
        assistant_run_answer_quality_case_supply_status,
        assistant_run_answer_quality_has_deterministic_supply,
        assistant_run_answer_quality_repeated_fallback_or_timeout,
        assistant_run_output_artifacts_include_report_link,
        assistant_run_supply_quality_suggests_parse_recovery,
    },
    assistant_run_text_support::truncate_assistant_supply_text,
    assistant_run_xinbai_report_link_support::assistant_run_xinbai_published_report_link_answer,
    external_channel_prompt_requests_static_page_report_workflow,
    json_value_support::value_array,
};

pub(crate) fn assistant_run_answer_quality_case_selected_scope_summary(
    selected_scope: &Value,
) -> Value {
    json!({
        "mode": selected_scope.get("mode").cloned().unwrap_or(Value::Null),
        "dataset_count": value_array(selected_scope.get("datasets").cloned().unwrap_or(Value::Null)).len(),
        "document_count": value_array(selected_scope.get("documents").cloned().unwrap_or(Value::Null)).len(),
        "database_source_count": value_array(selected_scope.get("database_sources").cloned().unwrap_or(Value::Null)).len(),
    })
}

pub(crate) fn assistant_run_answer_quality_report_link_expected(
    request: &CreateAssistantRunRequest,
) -> bool {
    assistant_run_xinbai_published_report_link_answer(&request.prompt).is_some()
        || external_channel_prompt_requests_static_page_report_workflow(&request.prompt)
}

pub(crate) fn assistant_run_answer_quality_low_quality_case_package(
    assistant_run_id: AssistantRunId,
    request: &CreateAssistantRunRequest,
    selected_scope: &Value,
    evidence_state: &Value,
    output_artifacts: &[Value],
    event_names: &[String],
) -> Option<Value> {
    let answer = assistant_run_assistant_message_content_from_artifacts(output_artifacts)
        .unwrap_or_default();
    let mut signals = Vec::<String>::new();
    if assistant_run_request_expresses_strong_complaint(request) {
        signals.push("strong_user_complaint".to_string());
    } else if assistant_run_request_expresses_dissatisfaction(request) {
        signals.push("user_complaint".to_string());
    }
    if let Some(reason) =
        assistant_run_answer_quality_retry_reason(&answer, evidence_state, request)
    {
        signals.push(match reason {
            "internal_marker_answer" => "unsafe_internal_marker_answer".to_string(),
            "insufficient_or_uncertain_answer" => "weak_insufficient_evidence_answer".to_string(),
            other => format!("answer_quality_{other}"),
        });
    }
    if assistant_run_answer_contains_insufficient_evidence_marker(&answer)
        && assistant_run_answer_quality_has_deterministic_supply(evidence_state)
    {
        signals.push("deterministic_supply_ignored".to_string());
    }
    if assistant_run_answer_quality_report_link_expected(request)
        && !assistant_run_output_artifacts_include_report_link(output_artifacts)
    {
        signals.push("missing_report_artifact_link".to_string());
    }
    if event_names
        .iter()
        .any(|event| event.contains("answer_quality_gate.retry_exhausted"))
    {
        signals.push("retry_exhausted".to_string());
    }
    if event_names
        .iter()
        .any(|event| event.contains("answer_quality_gate.exhausted_controlled_fallback"))
    {
        signals.push("controlled_fallback_used".to_string());
    }
    let upgrade_parse_attempted = event_names
        .iter()
        .any(|event| event.contains("upgrade_parse_vlm"));
    if assistant_run_supply_quality_suggests_parse_recovery(evidence_state)
        && !upgrade_parse_attempted
    {
        signals.push("parse_quality_degraded_without_upgrade".to_string());
    }
    if assistant_run_answer_quality_repeated_fallback_or_timeout(event_names) {
        signals.push("repeated_fallback_or_timeout".to_string());
    }
    signals.sort();
    signals.dedup();
    if signals.is_empty() {
        return None;
    }
    let supply_quality = evidence_state
        .get("supply_quality")
        .cloned()
        .unwrap_or(Value::Null);
    Some(json!({
        "template_id": "answer_quality_autofix",
        "status": "collected",
        "assistant_run_id": assistant_run_id.to_string(),
        "low_quality_signals": signals,
        "user_question": truncate_assistant_supply_text(&request.prompt, 800),
        "customer_answer_excerpt": truncate_assistant_supply_text(&answer, 1000),
        "selected_scope_summary": assistant_run_answer_quality_case_selected_scope_summary(selected_scope),
        "evidence_summary": {
            "supply_quality": supply_quality,
            "recovery_followup": evidence_state
                .get("recovery_followup")
                .filter(|value| !value.is_null())
                .cloned()
                .unwrap_or(Value::Null),
            "answer_supply_sources": assistant_run_answer_quality_case_supply_sources(evidence_state),
            "retrieval_or_fact_snapshot_status": assistant_run_answer_quality_case_supply_status(evidence_state),
        },
        "trace_summary": {
            "events": event_names.iter().take(24).cloned().collect::<Vec<_>>(),
            "quality_gate_events": event_names.iter().filter(|event| event.contains("answer_quality_gate")).take(12).cloned().collect::<Vec<_>>(),
        },
        "blocking_gate_enabled": false,
    }))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn request(prompt: &str) -> CreateAssistantRunRequest {
        CreateAssistantRunRequest {
            prompt: prompt.to_string(),
            local_thread_id: None,
            startup_briefing: None,
            selected_scope: None,
            scope_candidates: Vec::new(),
            context_policy_hint: None,
            current_artifact: None,
            messages: Vec::new(),
        }
    }

    #[test]
    fn selected_scope_summary_counts_arrays_and_preserves_mode() {
        let summary = assistant_run_answer_quality_case_selected_scope_summary(&json!({
            "mode": "user_selected",
            "datasets": ["dataset-1", "dataset-2"],
            "documents": ["doc-1"],
            "database_sources": ["db-1", "db-2", "db-3"],
        }));

        assert_eq!(summary["mode"], json!("user_selected"));
        assert_eq!(summary["dataset_count"], json!(2));
        assert_eq!(summary["document_count"], json!(1));
        assert_eq!(summary["database_source_count"], json!(3));
    }

    #[test]
    fn selected_scope_summary_defaults_missing_and_non_arrays_to_zero() {
        let summary = assistant_run_answer_quality_case_selected_scope_summary(&json!({
            "datasets": "dataset-1",
            "documents": null
        }));

        assert_eq!(summary["mode"], Value::Null);
        assert_eq!(summary["dataset_count"], json!(0));
        assert_eq!(summary["document_count"], json!(0));
        assert_eq!(summary["database_source_count"], json!(0));
    }

    #[test]
    fn report_link_expected_matches_published_xinbai_link_request() {
        assert!(assistant_run_answer_quality_report_link_expected(&request(
            "昨天/之前生成的新百报表链接"
        )));
    }

    #[test]
    fn report_link_expected_matches_static_page_report_workflow_request() {
        assert!(assistant_run_answer_quality_report_link_expected(&request(
            "请生成一份门店取高机会可视化报表"
        )));
    }

    #[test]
    fn report_link_expected_ignores_ordinary_questions() {
        assert!(!assistant_run_answer_quality_report_link_expected(
            &request("请用一句话介绍 DataMax")
        ));
    }

    #[test]
    fn low_quality_case_package_collects_complaint_case() {
        let package = assistant_run_answer_quality_low_quality_case_package(
            AssistantRunId::new(),
            &request("客户不满意，重新说这份考勤表缺勤。"),
            &json!({"mode": "user_selected", "datasets": ["dataset-1"]}),
            &json!({
                "status": "supplied",
                "supply_quality": {
                    "selectedDatasetCount": 1,
                    "suppliedItemCount": 2,
                    "spreadsheetRowAnalysisCount": 1
                }
            }),
            &[json!({
                "type": "assistant_message",
                "content": "根据当前资料整理如下。"
            })],
            &[],
        )
        .expect("complaint should collect a case");

        assert_eq!(package["template_id"], json!("answer_quality_autofix"));
        assert_eq!(package["blocking_gate_enabled"], json!(false));
        assert!(package["low_quality_signals"]
            .as_array()
            .expect("signals")
            .iter()
            .any(|signal| signal == "user_complaint"));
        assert_eq!(package["selected_scope_summary"]["dataset_count"], json!(1));
        assert!(package["evidence_summary"]["answer_supply_sources"]
            .as_array()
            .expect("sources")
            .iter()
            .any(|source| source == "spreadsheet_row_analysis"));
    }

    #[test]
    fn low_quality_case_package_skips_clean_answer_without_signals() {
        let package = assistant_run_answer_quality_low_quality_case_package(
            AssistantRunId::new(),
            &request("请用一句话介绍 DataMax"),
            &json!({"mode": "plain_chat"}),
            &json!({
                "status": "supplied",
                "supply_quality": {
                    "suppliedItemCount": 1,
                    "indexedEvidenceCount": 1
                }
            }),
            &[json!({
                "type": "assistant_message",
                "content": "DataMax 是企业级数据处理助手。"
            })],
            &[],
        );

        assert!(package.is_none());
    }
}
