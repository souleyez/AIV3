use std::collections::BTreeSet;

use crate::react_agent_tools::react_final_answer_content_is_raw_observation;
use serde_json::{json, Value};

const ASSISTANT_RUN_REACT_MESSAGE_TRACE_LIMIT: usize = 240;

pub(crate) fn assistant_run_react_completed_event_payload(
    step_index: usize,
    action_type: &str,
    observation_summary: &Value,
    entrypoint: Option<&str>,
    observation: &Value,
) -> Value {
    let mut payload = json!({
        "step": step_index,
        "action_type": action_type,
        "observation_summary": observation_summary.clone(),
    });
    if let Some(entrypoint) = entrypoint {
        payload["entrypoint"] = json!(entrypoint);
    }
    if let Some(html_artifacts) = observation
        .get("html_artifacts")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
    {
        payload["html_artifacts"] = Value::Array(html_artifacts.clone());
    }
    payload
}

pub(crate) fn assistant_run_react_output_artifacts_from_observations(
    observations: &[Value],
) -> Vec<Value> {
    let mut seen = BTreeSet::<String>::new();
    let mut summaries = Vec::<Value>::new();
    for artifact in observations
        .iter()
        .filter_map(|observation| observation.get("html_artifacts").and_then(Value::as_array))
        .flat_map(|items| items.iter())
    {
        let Some(id) = artifact.get("id").and_then(Value::as_str) else {
            continue;
        };
        if !seen.insert(id.to_string()) {
            continue;
        }
        summaries.push(json!({
            "type": "html_artifact",
            "id": id,
            "title": artifact.get("title").and_then(Value::as_str).unwrap_or("HTML 产物"),
            "template_id": artifact.get("template_id").and_then(Value::as_str).unwrap_or(""),
            "source_type": artifact.get("source_type").and_then(Value::as_str).unwrap_or(""),
        }));
    }
    summaries
}

pub(crate) fn bounded_duration_ms(duration_ms: u128) -> u64 {
    duration_ms.min(u64::MAX as u128) as u64
}

pub(crate) fn redact_react_trace_text(raw: &str, max_chars: usize) -> String {
    let value = raw.trim().chars().take(max_chars).collect::<String>();
    let lower = value.to_ascii_lowercase();
    if lower.contains("secret")
        || lower.contains("token")
        || lower.contains("password")
        || lower.contains("api_key")
        || lower.contains("apikey")
        || lower.contains("authorization")
        || lower.contains("bearer ")
        || lower.contains("sk-")
    {
        "[redacted]".to_string()
    } else {
        value
    }
}

pub(crate) fn assistant_run_react_observation_summary(observation: &Value) -> Value {
    let status = observation
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let action_type = observation
        .get("action_type")
        .or_else(|| observation.get("actionType"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let denied_count = observation
        .get("denied")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default();
    let returned_count = assistant_run_react_returned_count(observation);
    let detail_target_count = observation
        .get("detail_target_count")
        .or_else(|| observation.get("detailTargetCount"))
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let safe_error_code = observation
        .get("repair_code")
        .or_else(|| observation.get("error_code"))
        .and_then(Value::as_str)
        .map(|value| redact_react_trace_text(value, ASSISTANT_RUN_REACT_MESSAGE_TRACE_LIMIT))
        .or_else(|| {
            observation
                .get("error")
                .and_then(Value::as_str)
                .map(|_| "tool_failed".to_string())
        });
    let safe_message = observation
        .get("message")
        .and_then(Value::as_str)
        .map(|value| redact_react_trace_text(value, ASSISTANT_RUN_REACT_MESSAGE_TRACE_LIMIT))
        .or_else(|| {
            observation
                .get("error")
                .and_then(Value::as_str)
                .map(|_| "工具执行失败".to_string())
        })
        .unwrap_or_default();

    json!({
        "status": status,
        "action_type": action_type,
        "denied_count": denied_count,
        "returned_count": returned_count,
        "detail_target_count": detail_target_count,
        "safe_error_code": safe_error_code,
        "safe_message": safe_message,
    })
}

pub(crate) fn assistant_run_react_returned_count(observation: &Value) -> usize {
    if let Some(count) = observation
        .get("items")
        .and_then(Value::as_array)
        .map(Vec::len)
        .filter(|count| *count > 0)
    {
        return count;
    }
    if let Some(count) = observation
        .get("supplied_items")
        .and_then(Value::as_array)
        .map(Vec::len)
        .filter(|count| *count > 0)
    {
        return count;
    }
    observation
        .get("supplied_count")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or_default()
}

pub(crate) fn assistant_run_react_direct_natural_answer_from_invalid_output(
    output_text: &str,
) -> Option<String> {
    let trimmed = output_text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if react_final_answer_content_is_raw_observation(trimmed)
        || assistant_run_react_output_is_json_payload(trimmed)
        || assistant_run_react_output_contains_internal_marker(trimmed)
    {
        return None;
    }
    Some(trimmed.to_string())
}

pub(crate) fn assistant_run_react_output_is_json_payload(output_text: &str) -> bool {
    let Some(candidate) = assistant_run_react_json_payload_candidate(output_text) else {
        return false;
    };
    serde_json::from_str::<Value>(&candidate).is_ok()
}

pub(crate) fn assistant_run_react_json_payload_candidate(output_text: &str) -> Option<String> {
    let trimmed = output_text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let fenced = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```JSON"))
        .or_else(|| trimmed.strip_prefix("```"));
    if let Some(fenced) = fenced {
        let inner = fenced.trim();
        let inner = inner.strip_suffix("```").unwrap_or(inner).trim();
        return Some(inner.to_string());
    }
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return Some(trimmed.to_string());
    }
    None
}

pub(crate) fn assistant_run_react_output_contains_internal_marker(output_text: &str) -> bool {
    let normalized = output_text.to_ascii_lowercase();
    [
        "[tool_call]",
        "[/tool_call]",
        "<tool_call",
        "</tool_call",
        "execution_trail",
        "react_trace",
        "tool_trace",
        "runtime_manifest",
        "provider_raw",
        "safe_error_code",
        "requires_confirmation",
        "parse_degraded",
        "low_text_coverage",
        "model_status",
        "retrieve_evidence:",
        "read_document_detail:",
        "upgrade_parse_vlm:",
        "recall_conversation_memory:",
        "codex_host_task:",
        "create_static_page_draft:",
        "update_static_page_module:",
        "submit_static_page_image_preview:",
        "render_static_page:",
        "publish_static_page_revision:",
        "create_report_draft:",
        "\"action_type\"",
        "\"actiontype\"",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
        || [
            "[observation]",
            "[/observation]",
            "<observation",
            "</observation",
            "\"observation\"",
            "\"observations\"",
        ]
        .iter()
        .any(|marker| normalized.contains(marker))
        || normalized.lines().any(|line| {
            let line = line.trim_start();
            line.starts_with("observation:") || line.starts_with("observations:")
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn completed_event_payload_preserves_optional_entrypoint_and_html_artifacts() {
        let observation_summary = json!({"status": "completed", "count": 2});
        let observation = json!({
            "html_artifacts": [
                {"id": "artifact-1", "title": "月报"},
                {"id": "artifact-2", "title": "明细"}
            ],
            "private_note": "ignored"
        });

        let payload = assistant_run_react_completed_event_payload(
            3,
            "GenerateReport",
            &observation_summary,
            Some("report_tool"),
            &observation,
        );

        assert_eq!(
            payload,
            json!({
                "step": 3,
                "action_type": "GenerateReport",
                "observation_summary": {"status": "completed", "count": 2},
                "entrypoint": "report_tool",
                "html_artifacts": [
                    {"id": "artifact-1", "title": "月报"},
                    {"id": "artifact-2", "title": "明细"}
                ]
            })
        );
    }

    #[test]
    fn completed_event_payload_omits_empty_optional_fields() {
        let payload = assistant_run_react_completed_event_payload(
            1,
            "RetrieveEvidence",
            &json!({"status": "completed"}),
            None,
            &json!({"html_artifacts": []}),
        );

        assert_eq!(
            payload,
            json!({
                "step": 1,
                "action_type": "RetrieveEvidence",
                "observation_summary": {"status": "completed"}
            })
        );
    }

    #[test]
    fn output_artifacts_collects_first_html_artifact_per_id_with_defaults() {
        let observations = vec![
            json!({
                "html_artifacts": [
                    {
                        "id": "artifact-1",
                        "title": "经营月报",
                        "template_id": "xinbai",
                        "source_type": "report"
                    },
                    {"title": "missing id skipped"}
                ]
            }),
            json!({
                "html_artifacts": [
                    {"id": "artifact-1", "title": "duplicate ignored"},
                    {"id": "artifact-2"}
                ]
            }),
            json!({"html_artifacts": "not-an-array"}),
        ];

        assert_eq!(
            assistant_run_react_output_artifacts_from_observations(&observations),
            vec![
                json!({
                    "type": "html_artifact",
                    "id": "artifact-1",
                    "title": "经营月报",
                    "template_id": "xinbai",
                    "source_type": "report"
                }),
                json!({
                    "type": "html_artifact",
                    "id": "artifact-2",
                    "title": "HTML 产物",
                    "template_id": "",
                    "source_type": ""
                })
            ]
        );
    }

    #[test]
    fn react_trace_text_trims_and_truncates_non_sensitive_text() {
        assert_eq!(redact_react_trace_text("  abcdef  ", 3), "abc");
    }

    #[test]
    fn react_trace_text_redacts_sensitive_tokens_after_truncation() {
        assert_eq!(
            redact_react_trace_text("Authorization: Bearer abc", 240),
            "[redacted]"
        );
        assert_eq!(redact_react_trace_text("sk-test-key", 240), "[redacted]");
    }

    #[test]
    fn bounded_duration_ms_saturates_to_u64_max() {
        assert_eq!(bounded_duration_ms(42), 42);
        assert_eq!(bounded_duration_ms(u128::MAX), u64::MAX);
    }

    #[test]
    fn react_observation_summary_redacts_sensitive_message_and_counts_items() {
        let summary = assistant_run_react_observation_summary(&json!({
            "status": "completed",
            "actionType": "read_document_detail",
            "message": "token provider marker",
            "detailTargetCount": 2,
            "items": [{"id": "item-1"}],
            "denied": ["document:denied"]
        }));

        assert_eq!(summary["action_type"], "read_document_detail");
        assert_eq!(summary["returned_count"], 1);
        assert_eq!(summary["denied_count"], 1);
        assert_eq!(summary["detail_target_count"], 2);
        assert_eq!(summary["safe_message"], "[redacted]");
    }

    #[test]
    fn react_returned_count_prefers_real_items_then_supplied_items_then_count() {
        assert_eq!(
            assistant_run_react_returned_count(&json!({"items": [1, 2], "supplied_count": 9})),
            2
        );
        assert_eq!(
            assistant_run_react_returned_count(
                &json!({"items": [], "supplied_items": [1], "supplied_count": 9})
            ),
            1
        );
        assert_eq!(
            assistant_run_react_returned_count(
                &json!({"items": [], "supplied_items": [], "supplied_count": 9})
            ),
            9
        );
    }

    #[test]
    fn react_direct_natural_answer_accepts_only_plain_customer_text() {
        assert_eq!(
            assistant_run_react_direct_natural_answer_from_invalid_output(
                "  可以，先按普通问答回答。  "
            ),
            Some("可以，先按普通问答回答。".to_string())
        );
        assert!(assistant_run_react_direct_natural_answer_from_invalid_output("").is_none());
        assert!(
            assistant_run_react_direct_natural_answer_from_invalid_output(
                r#"{"status":"ok","action_type":"retrieve_evidence","items":[]}"#,
            )
            .is_none()
        );
        assert!(
            assistant_run_react_direct_natural_answer_from_invalid_output(
                "runtime_manifest: {\"provider_raw\": true}",
            )
            .is_none()
        );
    }

    #[test]
    fn react_json_payload_candidate_handles_fenced_and_raw_json() {
        assert_eq!(
            assistant_run_react_json_payload_candidate("```json\n{\"status\":\"ok\"}\n```"),
            Some("{\"status\":\"ok\"}".to_string())
        );
        assert_eq!(
            assistant_run_react_json_payload_candidate(" [1,2] "),
            Some("[1,2]".to_string())
        );
        assert!(assistant_run_react_json_payload_candidate("not json").is_none());
    }

    #[test]
    fn react_internal_marker_blocks_structural_observation_not_plain_word() {
        assert!(!assistant_run_react_output_contains_internal_marker(
            "My observation is that the answer can be plain text."
        ));
        assert!(assistant_run_react_output_contains_internal_marker(
            "Observation: {\"items\":[{\"summary\":\"内部供料\"}]}",
        ));
        assert!(assistant_run_react_output_contains_internal_marker(
            "[tool_call]{\"action_type\":\"retrieve_evidence\"}[/tool_call]",
        ));
    }
}
