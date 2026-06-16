use contracts::AssistantRunCodexToolOutputPolicyView;
use serde_json::{json, Value};

pub(crate) fn assistant_run_codex_tool_output_policy() -> AssistantRunCodexToolOutputPolicyView {
    AssistantRunCodexToolOutputPolicyView {
        max_total_chars: assistant_run_codex_env_usize(
            "ASSISTANT_RUN_CODEX_TOOL_OUTPUT_MAX_TOTAL_CHARS",
            80_000,
        ),
        max_item_chars: assistant_run_codex_env_usize(
            "ASSISTANT_RUN_CODEX_TOOL_OUTPUT_MAX_ITEM_CHARS",
            16_000,
        ),
        preserve_recent_output_count: assistant_run_codex_env_usize(
            "ASSISTANT_RUN_CODEX_TOOL_OUTPUT_PRESERVE_RECENT",
            3,
        ),
        preserve_error_fields: assistant_run_codex_env_flag(
            "ASSISTANT_RUN_CODEX_TOOL_OUTPUT_PRESERVE_ERRORS",
            true,
        ),
        preserve_evidence_refs: assistant_run_codex_env_flag(
            "ASSISTANT_RUN_CODEX_TOOL_OUTPUT_PRESERVE_REFS",
            true,
        ),
        preserve_media_timestamps: assistant_run_codex_env_flag(
            "ASSISTANT_RUN_CODEX_TOOL_OUTPUT_PRESERVE_MEDIA_TIMESTAMPS",
            true,
        ),
    }
}

fn assistant_run_codex_env_usize(key: &str, default_value: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default_value)
}

fn assistant_run_codex_env_flag(key: &str, default_value: bool) -> bool {
    std::env::var(key)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default_value)
}

pub(crate) fn assistant_run_codex_bounded_evidence_state(
    evidence_state: &Value,
    policy: &AssistantRunCodexToolOutputPolicyView,
) -> Value {
    let mut bounded = evidence_state.clone();
    let Some(object) = bounded.as_object_mut() else {
        return bounded;
    };
    let Some(tool_outputs) = object.get_mut("tool_outputs").and_then(Value::as_array_mut) else {
        return bounded;
    };

    let total_output_count = tool_outputs.len();
    let mut trimmed_output_count = 0_usize;
    let mut trimmed_field_count = 0_usize;
    let mut largest_output_chars = 0_usize;
    let mut total_chars = 0_usize;
    for (index, output) in tool_outputs.iter_mut().enumerate() {
        let is_recent =
            total_output_count.saturating_sub(index) <= policy.preserve_recent_output_count;
        let before_chars = assistant_run_codex_value_chars(output);
        largest_output_chars = largest_output_chars.max(before_chars);
        total_chars = total_chars.saturating_add(before_chars);
        let trim_result = assistant_run_codex_trim_tool_output_value(output, policy, is_recent);
        if trim_result.trimmed {
            trimmed_output_count += 1;
            trimmed_field_count += trim_result.trimmed_field_count;
        }
    }

    let existing_trimmed_count = object
        .get("trimmed_item_count")
        .or_else(|| object.get("trimmedItemCount"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if trimmed_output_count > 0 {
        object.insert(
            "trimmed_item_count".to_string(),
            json!(existing_trimmed_count + trimmed_output_count as u64),
        );
    }
    object.insert(
        "codex_tool_output_budget".to_string(),
        json!({
            "policy": policy,
            "tool_output_count": total_output_count,
            "trimmed_output_count": trimmed_output_count,
            "trimmed_field_count": trimmed_field_count,
            "largest_output_chars": largest_output_chars,
            "estimated_total_chars_before_trim": total_chars,
            "preserves": {
                "recent_output_count": policy.preserve_recent_output_count,
                "error_fields": policy.preserve_error_fields,
                "evidence_refs": policy.preserve_evidence_refs,
                "media_timestamps": policy.preserve_media_timestamps,
            }
        }),
    );
    bounded
}

#[derive(Default)]
struct AssistantRunCodexTrimResult {
    trimmed: bool,
    trimmed_field_count: usize,
}

fn assistant_run_codex_trim_tool_output_value(
    value: &mut Value,
    policy: &AssistantRunCodexToolOutputPolicyView,
    is_recent: bool,
) -> AssistantRunCodexTrimResult {
    match value {
        Value::Object(object) => {
            let mut result = AssistantRunCodexTrimResult::default();
            let mut trimmed_fields = Vec::new();
            let keys: Vec<String> = object.keys().cloned().collect();
            for key in keys {
                let Some(child) = object.get_mut(&key) else {
                    continue;
                };
                if let Value::String(text) = child {
                    if assistant_run_codex_should_trim_tool_output_field(&key, policy) {
                        let item_limit =
                            assistant_run_codex_tool_output_item_limit(policy, is_recent);
                        if let Some(trimmed) = assistant_run_codex_trim_text_value(text, item_limit)
                        {
                            *text = trimmed;
                            result.trimmed = true;
                            result.trimmed_field_count += 1;
                            trimmed_fields.push(key);
                        }
                    }
                    continue;
                }
                let nested = assistant_run_codex_trim_tool_output_value(child, policy, is_recent);
                if nested.trimmed {
                    result.trimmed = true;
                    result.trimmed_field_count += nested.trimmed_field_count;
                }
            }
            if !trimmed_fields.is_empty() {
                object.insert("codex_trimmed_fields".to_string(), json!(trimmed_fields));
            }
            result
        }
        Value::Array(items) => items.iter_mut().fold(
            AssistantRunCodexTrimResult::default(),
            |mut result, item| {
                let nested = assistant_run_codex_trim_tool_output_value(item, policy, is_recent);
                if nested.trimmed {
                    result.trimmed = true;
                    result.trimmed_field_count += nested.trimmed_field_count;
                }
                result
            },
        ),
        _ => AssistantRunCodexTrimResult::default(),
    }
}

fn assistant_run_codex_tool_output_item_limit(
    policy: &AssistantRunCodexToolOutputPolicyView,
    is_recent: bool,
) -> usize {
    if is_recent {
        policy
            .max_item_chars
            .saturating_mul(2)
            .min(policy.max_total_chars)
            .max(policy.max_item_chars)
    } else {
        policy.max_item_chars
    }
}

fn assistant_run_codex_should_trim_tool_output_field(
    key: &str,
    policy: &AssistantRunCodexToolOutputPolicyView,
) -> bool {
    let lower = key.to_ascii_lowercase();
    if policy.preserve_evidence_refs
        && ["id", "ref", "refs", "citation", "source", "locator"]
            .iter()
            .any(|needle| lower.contains(needle))
    {
        return false;
    }
    if policy.preserve_media_timestamps
        && [
            "timestamp",
            "timestamps",
            "time_window",
            "timecode",
            "start_ms",
            "end_ms",
        ]
        .iter()
        .any(|needle| lower.contains(needle))
    {
        return false;
    }
    if policy.preserve_error_fields
        && ["error_code", "failure_kind", "exit_code", "status"]
            .iter()
            .any(|needle| lower.contains(needle))
    {
        return false;
    }
    [
        "content",
        "output",
        "stdout",
        "stderr",
        "text",
        "body",
        "raw",
        "transcript",
        "result",
        "message",
        "error",
        "failure",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn assistant_run_codex_trim_text_value(value: &str, max_chars: usize) -> Option<String> {
    let original_chars = value.chars().count();
    if original_chars <= max_chars {
        return None;
    }
    if max_chars < 80 {
        return Some(format!(
            "[codex-trimmed original_chars={original_chars} kept_chars=0]"
        ));
    }
    let marker = format!(
        "\n[codex-trimmed original_chars={} omitted_chars={}]\n",
        original_chars,
        original_chars.saturating_sub(max_chars)
    );
    let marker_chars = marker.chars().count();
    let kept_chars = max_chars.saturating_sub(marker_chars).max(2);
    let head_chars = kept_chars / 2;
    let tail_chars = kept_chars.saturating_sub(head_chars);
    let head: String = value.chars().take(head_chars).collect();
    let tail_reversed: String = value.chars().rev().take(tail_chars).collect();
    let tail: String = tail_reversed.chars().rev().collect();
    Some(format!("{head}{marker}{tail}"))
}

pub(crate) fn assistant_run_codex_value_chars(value: &Value) -> usize {
    if value.is_null() {
        return 0;
    }
    serde_json::to_string(value)
        .map(|value| value.chars().count())
        .unwrap_or(0)
}

pub(crate) fn assistant_run_codex_values_chars(values: &[Value]) -> usize {
    values.iter().map(assistant_run_codex_value_chars).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(
        max_total_chars: usize,
        max_item_chars: usize,
        preserve_recent_output_count: usize,
    ) -> AssistantRunCodexToolOutputPolicyView {
        AssistantRunCodexToolOutputPolicyView {
            max_total_chars,
            max_item_chars,
            preserve_recent_output_count,
            preserve_error_fields: true,
            preserve_evidence_refs: true,
            preserve_media_timestamps: true,
        }
    }

    #[test]
    fn bounded_evidence_state_trims_old_text_fields_and_preserves_refs() {
        let old_output = "A".repeat(120);
        let recent_output = "B".repeat(90);
        let bounded = assistant_run_codex_bounded_evidence_state(
            &json!({
                "trimmed_item_count": 2,
                "tool_outputs": [
                    {
                        "stdout": old_output,
                        "source_locator": "dataset://kept",
                        "timestamp": "00:01",
                        "error_code": "E_SAFE"
                    },
                    {
                        "stdout": recent_output
                    }
                ]
            }),
            &policy(180, 80, 1),
        );

        assert_eq!(bounded["trimmed_item_count"], json!(3));
        assert_eq!(
            bounded["tool_outputs"][0]["source_locator"],
            json!("dataset://kept")
        );
        assert_eq!(bounded["tool_outputs"][0]["timestamp"], json!("00:01"));
        assert_eq!(bounded["tool_outputs"][0]["error_code"], json!("E_SAFE"));
        assert!(bounded["tool_outputs"][0]["stdout"]
            .as_str()
            .expect("trimmed stdout")
            .contains("[codex-trimmed"));
        assert_eq!(
            bounded["tool_outputs"][0]["codex_trimmed_fields"],
            json!(["stdout"])
        );
        assert_eq!(bounded["tool_outputs"][1]["stdout"], json!("B".repeat(90)));
        assert_eq!(
            bounded["codex_tool_output_budget"]["trimmed_output_count"],
            json!(1)
        );
        assert_eq!(
            bounded["codex_tool_output_budget"]["trimmed_field_count"],
            json!(1)
        );
        assert_eq!(
            bounded["codex_tool_output_budget"]["preserves"]["recent_output_count"],
            json!(1)
        );
    }

    #[test]
    fn bounded_evidence_state_ignores_missing_tool_output_array() {
        let source = json!({"status": "supplied"});
        let bounded = assistant_run_codex_bounded_evidence_state(&source, &policy(180, 80, 1));

        assert_eq!(bounded, source);
    }

    #[test]
    fn value_chars_treats_null_as_zero_and_counts_json_text() {
        assert_eq!(assistant_run_codex_value_chars(&Value::Null), 0);
        assert!(assistant_run_codex_value_chars(&json!({"a": "b"})) > 0);
        assert_eq!(
            assistant_run_codex_values_chars(&[Value::Null, json!({"a": "b"})]),
            assistant_run_codex_value_chars(&json!({"a": "b"}))
        );
    }
}
