use contracts::CreateAssistantRunRequest;
use serde_json::{json, Value};

use crate::{
    assistant_run_answer_quality_budget_support::assistant_run_request_expresses_dissatisfaction,
    assistant_run_answer_quality_spreadsheet_support::assistant_run_answer_quality_spreadsheet_controlled_answer,
    assistant_run_point_list_support::assistant_run_answer_quality_point_list_controlled_answer,
    assistant_run_request_wants_json_output,
};

pub(crate) fn assistant_run_answer_quality_exhausted_controlled_answer(
    evidence_state: &Value,
    request: &CreateAssistantRunRequest,
    _remaining_reason: &str,
) -> String {
    if let Some(answer) =
        assistant_run_answer_quality_spreadsheet_controlled_answer(evidence_state, request)
    {
        return answer;
    }
    if let Some(answer) =
        assistant_run_answer_quality_point_list_controlled_answer(evidence_state, request)
    {
        return answer;
    }

    let supplied_count = evidence_state
        .get("supply_quality")
        .and_then(|quality| quality.get("suppliedItemCount"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if assistant_run_request_wants_json_output(request) {
        let status = if supplied_count > 0 {
            "needs_more_input_after_retry"
        } else {
            "no_verifiable_evidence"
        };
        let message = if supplied_count > 0 {
            "我已重新核对当前可见材料，但这轮仍未形成可核验结论。"
        } else {
            "我已尝试重新获取可见材料，但这轮没有形成可核验结论。"
        };
        return serde_json::to_string_pretty(&json!({
            "status": status,
            "message": message,
            "question": request.prompt.trim(),
            "supplied_item_count": supplied_count,
            "next_action": "continue_same_conversation_with_more_scope_or_clarification",
            "suggested_user_inputs": ["文档ID", "数据集分组", "页码或章节", "关键词", "统计口径或对象"],
        }))
        .unwrap_or_else(|_| {
            "{\"status\":\"needs_more_input_after_retry\",\"message\":\"需要补充信息后继续。\"}"
                .to_string()
        });
    }
    let is_dissatisfied = assistant_run_request_expresses_dissatisfaction(request);
    if supplied_count > 0 {
        if is_dissatisfied {
            return "我已重新核对可见材料并扩大读取范围，但这轮仍未形成可核验结论。为避免误判，我先不编造结论；请补充具体文档 ID、页码/章节、对象或统计口径。收到补充后，我会沿用本会话已授权的资料范围继续检索并完成回答。".to_string();
        }
        return "我已重新核对当前可见材料，但这轮仍未形成可核验结论。为避免误判，我先不编造结论；请补充具体文档 ID、页码/章节、对象或统计口径。收到补充后，我会沿用本会话已授权的资料范围继续检索并完成回答。".to_string();
    }
    "我已尝试重新获取可见材料，但这轮没有形成可核验结论。为避免误判，我先不编造结论；请补充可用文档 ID、数据集分组或更明确的问题对象。收到补充后，我会沿用本会话继续执行。".to_string()
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

    fn json_request(prompt: &str) -> CreateAssistantRunRequest {
        CreateAssistantRunRequest {
            context_policy_hint: Some(json!({
                "answer_policy": {
                    "output_format": {"format": "json"}
                }
            })),
            ..request(prompt)
        }
    }

    #[test]
    fn exhausted_controlled_answer_returns_json_when_requested() {
        let answer = assistant_run_answer_quality_exhausted_controlled_answer(
            &json!({
                "supply_quality": {
                    "suppliedItemCount": 2
                }
            }),
            &json_request("请用 JSON 说明为什么还不能回答"),
            "model_judge_customer_unsafe_answer",
        );
        let parsed: Value =
            serde_json::from_str(&answer).expect("controlled fallback should be JSON");

        assert_eq!(parsed["status"], json!("needs_more_input_after_retry"));
        assert_eq!(parsed["supplied_item_count"], json!(2));
        assert_eq!(
            parsed["next_action"],
            json!("continue_same_conversation_with_more_scope_or_clarification")
        );
    }

    #[test]
    fn exhausted_controlled_answer_uses_dissatisfied_copy_after_supplied_retry() {
        let answer = assistant_run_answer_quality_exhausted_controlled_answer(
            &json!({
                "supply_quality": {
                    "suppliedItemCount": 3
                }
            }),
            &request("客户不满意，重新查一下这个合同。"),
            "insufficient_or_uncertain_answer",
        );

        assert!(answer.contains("重新核对可见材料并扩大读取范围"));
        assert!(answer.contains("沿用本会话已授权的资料范围"));
    }

    #[test]
    fn exhausted_controlled_answer_uses_no_evidence_copy_without_supply() {
        let answer = assistant_run_answer_quality_exhausted_controlled_answer(
            &json!({}),
            &request("查一下这个合同。"),
            "empty_answer",
        );

        assert!(answer.contains("尝试重新获取可见材料"));
        assert!(answer.contains("补充可用文档 ID"));
    }
}
