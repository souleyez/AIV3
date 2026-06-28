use serde_json::Value;

use crate::assistant_run_react_support::assistant_run_react_json_payload_candidate;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AssistantRunAnswerQualityJudgeDecision {
    pub(crate) verdict: AssistantRunAnswerQualityJudgeVerdict,
    pub(crate) reason: String,
    pub(crate) confidence: f64,
    pub(crate) customer_safe: bool,
    pub(crate) required_actions: Vec<String>,
    pub(crate) premium_action_allowed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AssistantRunAnswerQualityJudgeVerdict {
    Accept,
    Retry,
    ControlledFallback,
}

pub(crate) fn assistant_run_answer_quality_retry_reason_from_judge_decision(
    decision: &AssistantRunAnswerQualityJudgeDecision,
) -> Option<&'static str> {
    if !decision.customer_safe {
        return Some("model_judge_customer_unsafe_answer");
    }
    match decision.verdict {
        AssistantRunAnswerQualityJudgeVerdict::Accept => None,
        AssistantRunAnswerQualityJudgeVerdict::Retry => match decision.reason.as_str() {
            "parse_quality_insufficient" => Some("model_judge_parse_quality_insufficient"),
            "incomplete_task" => Some("model_judge_incomplete_task"),
            "ungrounded" => Some("model_judge_ungrounded_answer"),
            "low_customer_confidence" => Some("model_judge_low_customer_confidence"),
            "unsafe_internal_leak" => Some("model_judge_customer_unsafe_answer"),
            _ => Some("model_judge_retry"),
        },
        AssistantRunAnswerQualityJudgeVerdict::ControlledFallback => None,
    }
}

pub(crate) fn parse_assistant_run_answer_quality_judge_decision(
    raw: &str,
) -> Option<AssistantRunAnswerQualityJudgeDecision> {
    let candidate = assistant_run_react_json_payload_candidate(raw)?;
    let value = serde_json::from_str::<Value>(&candidate).ok()?;
    let verdict = value.get("verdict").and_then(Value::as_str)?;
    let verdict = match verdict.trim().to_ascii_lowercase().as_str() {
        "accept" => AssistantRunAnswerQualityJudgeVerdict::Accept,
        "retry" => AssistantRunAnswerQualityJudgeVerdict::Retry,
        "controlled_fallback" | "controlled-fallback" | "fallback" => {
            AssistantRunAnswerQualityJudgeVerdict::ControlledFallback
        }
        _ => return None,
    };
    let reason = value
        .get("reason")
        .and_then(Value::as_str)
        .map(|reason| reason.trim().to_ascii_lowercase())
        .filter(|reason| !reason.is_empty())
        .unwrap_or_else(|| "ok".to_string());
    let confidence = value
        .get("confidence")
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    let customer_safe = value
        .get("customer_safe")
        .or_else(|| value.get("customerSafe"))
        .and_then(Value::as_bool)
        .unwrap_or(verdict == AssistantRunAnswerQualityJudgeVerdict::Accept);
    let required_actions = value
        .get("required_actions")
        .or_else(|| value.get("requiredActions"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|action| !action.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let premium_action_allowed = value
        .get("premium_action_allowed")
        .or_else(|| value.get("premiumActionAllowed"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Some(AssistantRunAnswerQualityJudgeDecision {
        verdict,
        reason,
        confidence,
        customer_safe,
        required_actions,
        premium_action_allowed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn judge_decision_parses_fenced_json_and_maps_unsafe_retry() {
        let decision = parse_assistant_run_answer_quality_judge_decision(
            r#"```json
{
  "verdict": "retry",
  "reason": "parse_quality_insufficient",
  "confidence": 0.72,
  "customer_safe": false,
  "required_actions": ["read_document_detail", "upgrade_parse_vlm"],
  "premium_action_allowed": true
}
```"#,
        )
        .expect("judge decision should parse");

        assert_eq!(
            decision.verdict,
            AssistantRunAnswerQualityJudgeVerdict::Retry
        );
        assert_eq!(decision.reason, "parse_quality_insufficient");
        assert_eq!(decision.confidence, 0.72);
        assert!(!decision.customer_safe);
        assert!(decision
            .required_actions
            .iter()
            .any(|action| action == "upgrade_parse_vlm"));
        assert!(decision.premium_action_allowed);
        assert_eq!(
            assistant_run_answer_quality_retry_reason_from_judge_decision(&decision),
            Some("model_judge_customer_unsafe_answer")
        );
    }

    #[test]
    fn judge_decision_supports_camel_case_fields_and_reason_mapping() {
        let decision = parse_assistant_run_answer_quality_judge_decision(
            r#"{
  "verdict": "retry",
  "reason": "incomplete_task",
  "confidence": 1.8,
  "customerSafe": true,
  "requiredActions": ["retrieve_evidence", ""],
  "premiumActionAllowed": true
}"#,
        )
        .expect("camel-case judge decision should parse");

        assert_eq!(decision.confidence, 1.0);
        assert_eq!(decision.required_actions, vec!["retrieve_evidence"]);
        assert!(decision.premium_action_allowed);
        assert_eq!(
            assistant_run_answer_quality_retry_reason_from_judge_decision(&decision),
            Some("model_judge_incomplete_task")
        );
    }

    #[test]
    fn judge_decision_defaults_accept_to_customer_safe() {
        let decision = parse_assistant_run_answer_quality_judge_decision(r#"{"verdict":"accept"}"#)
            .expect("accept decision should parse");

        assert_eq!(
            decision.verdict,
            AssistantRunAnswerQualityJudgeVerdict::Accept
        );
        assert_eq!(decision.reason, "ok");
        assert!(decision.customer_safe);
        assert_eq!(
            assistant_run_answer_quality_retry_reason_from_judge_decision(&decision),
            None
        );
    }
}
