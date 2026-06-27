use std::collections::BTreeSet;

use serde_json::{json, Value};

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
}
