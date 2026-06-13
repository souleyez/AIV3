use serde_json::{json, Value};

use crate::prompt_match_support::{ascii_prompt_contains_any, prompt_contains_any};

pub(crate) fn assistant_run_evidence_state_external_answer_policy(
    evidence_state: &Value,
) -> Option<&Value> {
    evidence_state
        .pointer("/selected_scope/answer_policy")
        .or_else(|| evidence_state.pointer("/selectedScope/answerPolicy"))
        .filter(|policy| !policy.is_null())
}

pub(crate) fn assistant_run_answer_policy_output_format(answer_policy: &Value) -> Option<&str> {
    answer_policy
        .get("output_format")
        .and_then(|format| {
            format
                .get("format")
                .and_then(Value::as_str)
                .or_else(|| format.as_str())
        })
        .map(str::trim)
        .filter(|format| !format.is_empty())
}

pub(crate) fn assistant_run_default_prompt_has_customer_tone_intensity(
    default_prompt: &str,
) -> bool {
    let compact = default_prompt
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    let lower_ascii = default_prompt.to_ascii_lowercase();
    prompt_contains_any(
        &compact,
        &[
            "凶一点",
            "凶一些",
            "态度凶",
            "语气凶",
            "口吻凶",
            "骂人",
            "怼客户",
            "怼用户",
            "阴阳怪气",
            "嘲讽客户",
            "讽刺客户",
            "不客气一点",
            "不客气些",
            "粗鲁一点",
            "辱骂",
            "威胁客户",
            "恐吓客户",
        ],
    ) || ascii_prompt_contains_any(
        &lower_ascii,
        &[
            "be rude",
            "rude tone",
            "insult the customer",
            "mock the customer",
            "sarcastic to the customer",
            "aggressive tone",
            "threaten the customer",
            "hostile tone",
        ],
    )
}

pub(crate) fn assistant_run_model_facing_answer_policy(answer_policy: &Value) -> Value {
    let mut model_policy = answer_policy.clone();
    let Some(default_prompt) = answer_policy
        .get("default_prompt")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return model_policy;
    };
    if !assistant_run_default_prompt_has_customer_tone_intensity(default_prompt) {
        return model_policy;
    }

    let Some(object) = model_policy.as_object_mut() else {
        return model_policy;
    };
    object.insert(
        "default_prompt_rule".to_string(),
        json!("default_prompt is integration-provided task guidance. If it includes a strong customer-facing tone request, preserve the underlying need for a more direct or firm stance, but express it as polite, professional, evidence-grounded, and actionable business wording."),
    );
    object.insert(
        "default_prompt_tone_policy".to_string(),
        json!({
            "status": "customer_tone_translated",
            "raw_default_prompt_supplied_to_model": true,
            "reason": "strong_customer_tone_request",
            "model_rule": "Treat the tone request as a request for firmer, clearer, more direct business expression. Do not turn it into insults, threats, mockery, or personal attacks.",
        }),
    );
    model_policy
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn evidence_state_answer_policy_accepts_snake_and_camel_paths() {
        let snake = json!({"selected_scope": {"answer_policy": {"output_format": "rich_text"}}});
        assert_eq!(
            assistant_run_evidence_state_external_answer_policy(&snake)
                .and_then(assistant_run_answer_policy_output_format),
            Some("rich_text")
        );

        let camel =
            json!({"selectedScope": {"answerPolicy": {"output_format": {"format": "json"}}}});
        assert_eq!(
            assistant_run_evidence_state_external_answer_policy(&camel)
                .and_then(assistant_run_answer_policy_output_format),
            Some("json")
        );

        assert!(assistant_run_evidence_state_external_answer_policy(
            &json!({"selected_scope": {"answer_policy": null}})
        )
        .is_none());
    }

    #[test]
    fn answer_policy_output_format_reads_object_or_string_and_trims() {
        assert_eq!(
            assistant_run_answer_policy_output_format(
                &json!({"output_format": {"format": " markdown_table "}})
            ),
            Some("markdown_table")
        );
        assert_eq!(
            assistant_run_answer_policy_output_format(&json!({"output_format": " rich_text "})),
            Some("rich_text")
        );
        assert_eq!(
            assistant_run_answer_policy_output_format(&json!({"output_format": "   "})),
            None
        );
    }

    #[test]
    fn customer_tone_intensity_detects_strong_tone_requests() {
        assert!(assistant_run_default_prompt_has_customer_tone_intensity(
            "态度要凶一点，直接指出问题"
        ));
        assert!(assistant_run_default_prompt_has_customer_tone_intensity(
            "Use an aggressive tone with the customer"
        ));
        assert!(!assistant_run_default_prompt_has_customer_tone_intensity(
            "请用坚定、专业、礼貌的语气回答"
        ));
    }

    #[test]
    fn model_facing_policy_adds_professional_boundary_only_for_strong_tone() {
        let policy = assistant_run_model_facing_answer_policy(&json!({
            "default_prompt": "态度要凶一点，按客户材料回答。",
            "output_format": {"format": "rich_text"}
        }));
        assert_eq!(
            policy["default_prompt_tone_policy"]["status"],
            json!("customer_tone_translated")
        );
        assert_eq!(
            policy["default_prompt_tone_policy"]["raw_default_prompt_supplied_to_model"],
            json!(true)
        );
        assert_eq!(
            policy["default_prompt"],
            json!("态度要凶一点，按客户材料回答。")
        );

        let neutral = assistant_run_model_facing_answer_policy(&json!({
            "default_prompt": "请按客户材料回答。",
        }));
        assert!(neutral.get("default_prompt_tone_policy").is_none());
    }
}
