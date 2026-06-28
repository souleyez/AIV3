use contracts::CreateAssistantRunRequest;
use serde_json::{json, Value};

use crate::{
    assistant_run_model_evidence_state,
    assistant_run_react_support::assistant_run_react_json_payload_candidate,
    build_assistant_run_model_supply_brief,
};

const ASSISTANT_RUN_ANSWER_QUALITY_JUDGE_ANSWER_CHARS: usize = 1200;

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

pub(crate) fn assistant_run_answer_quality_judge_enabled() -> bool {
    std::env::var("ASSISTANT_RUN_ANSWER_QUALITY_JUDGE_ENABLED")
        .map(|value| {
            !matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "0" | "false" | "off"
            )
        })
        .unwrap_or(true)
}

pub(crate) fn build_assistant_run_answer_quality_judge_input(
    request: &CreateAssistantRunRequest,
    evidence_state: &Value,
    output_text: &str,
) -> String {
    let supply_quality = evidence_state
        .get("supply_quality")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let model_evidence_state = assistant_run_model_evidence_state(evidence_state);
    let supply_brief = build_assistant_run_model_supply_brief(evidence_state)
        .unwrap_or_else(|| "无供料摘要。".to_string());
    let answer_excerpt = output_text
        .chars()
        .take(ASSISTANT_RUN_ANSWER_QUALITY_JUDGE_ANSWER_CHARS)
        .collect::<String>();
    [
        "你是 DataMax AssistantRun 的内部回答质量判卷器，只能输出 JSON，不能回答用户。".to_string(),
        "根据用户问题、供料状态、可回答证据摘要和候选答案，判断候选答案是否可以安全给客户。".to_string(),
        "硬规则：如果已有证据但候选答案推脱、遗漏表格/统计/排序任务、没有回答实体问题、或含内部状态泄露，应 verdict=retry。".to_string(),
        "如果解析质量明显阻塞且可通过升级解析恢复，required_actions 包含 upgrade_parse_vlm；但不要自行回答材料内容。".to_string(),
        "如果确实不可答且没有可靠升级路径，可 verdict=controlled_fallback。".to_string(),
        r#"只输出 JSON Schema：{"verdict":"accept|retry|controlled_fallback","reason":"ok|insufficient_evidence|ungrounded|incomplete_task|parse_quality_insufficient|low_customer_confidence|unsafe_internal_leak","confidence":0.0,"customer_safe":true,"required_actions":["retrieve_evidence","read_document_detail"],"premium_action_allowed":false}"#.to_string(),
        format!("用户问题：{}", request.prompt.trim()),
        format!(
            "供料质量：{}",
            serde_json::to_string(&supply_quality).unwrap_or_else(|_| "{}".to_string())
        ),
        format!("供料摘要：\n{supply_brief}"),
        format!(
            "可回答证据摘要：{}",
            serde_json::to_string(&model_evidence_state).unwrap_or_else(|_| "{}".to_string())
        ),
        format!("候选答案：\n{answer_excerpt}"),
    ]
    .join("\n\n")
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
    use contracts::CreateAssistantRunRequest;
    use serde_json::json;

    use super::*;

    fn judge_request(prompt: &str) -> CreateAssistantRunRequest {
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
    fn judge_input_contains_context_and_truncates_candidate_answer() {
        let request = judge_request("请判断这份文档里邓工是谁？");
        let evidence_state = json!({
            "status": "supplied",
            "supply_quality": {
                "status": "grounded",
                "suppliedItemCount": 1,
                "indexedEvidenceCount": 1
            },
            "supplied_items": [{
                "type": "retrieval_evidence",
                "content_excerpt": "邓工是项目负责人。"
            }]
        });
        let output_text = format!(
            "{}TAIL_SHOULD_BE_TRUNCATED",
            "候选答案".repeat(ASSISTANT_RUN_ANSWER_QUALITY_JUDGE_ANSWER_CHARS)
        );

        let input =
            build_assistant_run_answer_quality_judge_input(&request, &evidence_state, &output_text);

        assert!(input.contains("内部回答质量判卷器"));
        assert!(input.contains("用户问题：请判断这份文档里邓工是谁？"));
        assert!(input.contains("\"status\":\"grounded\""));
        assert!(input.contains("供料摘要"));
        assert!(input.contains("候选答案"));
        assert!(!input.contains("TAIL_SHOULD_BE_TRUNCATED"));
    }

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
