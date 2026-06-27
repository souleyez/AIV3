use contracts::CreateAssistantRunRequest;
use serde_json::Value;

use crate::{
    assistant_run_prompt_request_support::{
        prompt_requests_deterministic_aggregate_supply, prompt_requests_document_entity_scan,
    },
    assistant_run_react_support::{
        ASSISTANT_RUN_REACT_DEFAULT_MAX_STEPS, ASSISTANT_RUN_REACT_MAX_STEPS,
    },
    assistant_run_supply_quality_suggests_parse_recovery,
    prompt_match_support::{ascii_prompt_contains_any, prompt_contains_any},
};

pub(crate) const ASSISTANT_RUN_ANSWER_QUALITY_DEFAULT_RETRY_BUDGET: usize = 1;
pub(crate) const ASSISTANT_RUN_ANSWER_QUALITY_DISSATISFIED_RETRY_BUDGET: usize = 3;
pub(crate) const ASSISTANT_RUN_ANSWER_QUALITY_STRONG_COMPLAINT_RETRY_BUDGET: usize = 4;
pub(crate) const ASSISTANT_RUN_ANSWER_QUALITY_MAX_RETRY_BUDGET: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AssistantRunQualityBudget {
    pub(crate) answer_retry_budget: usize,
    pub(crate) react_step_budget: usize,
    pub(crate) premium_action_budget: usize,
    pub(crate) reason: &'static str,
}

#[cfg(test)]
pub(crate) fn assistant_run_answer_quality_retry_budget(
    request: &CreateAssistantRunRequest,
) -> usize {
    assistant_run_answer_quality_budget(request).answer_retry_budget
}

pub(crate) fn assistant_run_answer_quality_budget(
    request: &CreateAssistantRunRequest,
) -> AssistantRunQualityBudget {
    let mut budget = if assistant_run_request_expresses_strong_complaint(request) {
        AssistantRunQualityBudget {
            answer_retry_budget: ASSISTANT_RUN_ANSWER_QUALITY_STRONG_COMPLAINT_RETRY_BUDGET,
            react_step_budget: ASSISTANT_RUN_REACT_MAX_STEPS,
            premium_action_budget: 1,
            reason: "strong_complaint",
        }
    } else if assistant_run_request_expresses_dissatisfaction(request) {
        AssistantRunQualityBudget {
            answer_retry_budget: ASSISTANT_RUN_ANSWER_QUALITY_DISSATISFIED_RETRY_BUDGET,
            react_step_budget: ASSISTANT_RUN_REACT_DEFAULT_MAX_STEPS,
            premium_action_budget: 1,
            reason: "dissatisfied",
        }
    } else {
        AssistantRunQualityBudget {
            answer_retry_budget: ASSISTANT_RUN_ANSWER_QUALITY_DEFAULT_RETRY_BUDGET,
            react_step_budget: ASSISTANT_RUN_REACT_DEFAULT_MAX_STEPS,
            premium_action_budget: 0,
            reason: "default",
        }
    };
    if let Some(env_budget) = assistant_run_answer_quality_retry_budget_from_env() {
        budget.answer_retry_budget = env_budget.min(ASSISTANT_RUN_ANSWER_QUALITY_MAX_RETRY_BUDGET);
        budget.reason = "env_override";
    }
    budget.answer_retry_budget = budget
        .answer_retry_budget
        .min(ASSISTANT_RUN_ANSWER_QUALITY_MAX_RETRY_BUDGET);
    budget
}

pub(crate) fn assistant_run_answer_quality_budget_for_evidence(
    request: &CreateAssistantRunRequest,
    evidence_state: &Value,
) -> AssistantRunQualityBudget {
    let mut budget = assistant_run_answer_quality_budget(request);
    if budget.reason == "default"
        && assistant_run_prompt_is_high_risk_quality_task(&request.prompt)
        && assistant_run_supply_quality_suggests_parse_recovery(evidence_state)
    {
        budget.answer_retry_budget = budget.answer_retry_budget.max(2);
        budget.premium_action_budget = budget.premium_action_budget.max(1);
        budget.reason = "parse_quality_recovery";
    }
    budget
}

fn assistant_run_answer_quality_retry_budget_from_env() -> Option<usize> {
    std::env::var("ASSISTANT_RUN_ANSWER_QUALITY_RETRY_BUDGET")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
}

pub(crate) fn assistant_run_request_expresses_dissatisfaction(
    request: &CreateAssistantRunRequest,
) -> bool {
    let mut texts = vec![request.prompt.as_str()];
    texts.extend(
        request
            .messages
            .iter()
            .rev()
            .take(6)
            .map(|message| message.content.as_str()),
    );
    texts.into_iter().any(|text| {
        let lower = text.to_ascii_lowercase();
        prompt_contains_any(
            text,
            &[
                "不满意",
                "不太满意",
                "客户不满",
                "客户有些不满意",
                "观感差",
                "回答很差",
                "回答太差",
                "答得差",
                "答非所问",
                "不准确",
                "不准",
                "不对",
                "错了",
                "错误",
                "乱答",
                "瞎答",
                "投诉",
            ],
        ) || ascii_prompt_contains_any(
            &lower,
            &[
                "dissatisfied",
                "unsatisfied",
                "unhappy",
                "wrong",
                "incorrect",
                "inaccurate",
                "poor",
                "bad",
                "complaint",
            ],
        )
    })
}

pub(crate) fn assistant_run_request_expresses_strong_complaint(
    request: &CreateAssistantRunRequest,
) -> bool {
    let mut texts = vec![request.prompt.as_str()];
    texts.extend(
        request
            .messages
            .iter()
            .rev()
            .take(6)
            .map(|message| message.content.as_str()),
    );
    texts.into_iter().any(|text| {
        let lower = text.to_ascii_lowercase();
        prompt_contains_any(
            text,
            &[
                "严重不满",
                "很不满意",
                "非常不满意",
                "客户很不满意",
                "反复答错",
                "多次答错",
                "严重错误",
                "线上事故",
                "投诉",
            ],
        ) || ascii_prompt_contains_any(
            &lower,
            &[
                "strong complaint",
                "formal complaint",
                "very dissatisfied",
                "repeatedly wrong",
                "critical failure",
                "incident",
                "escalation",
            ],
        )
    })
}

pub(crate) fn assistant_run_prompt_is_high_risk_quality_task(prompt: &str) -> bool {
    let lower = prompt.to_ascii_lowercase();
    prompt_contains_any(
        prompt,
        &[
            "是谁",
            "谁是",
            "哪些",
            "多少",
            "统计",
            "汇总",
            "排序",
            "排行",
            "排名",
            "表格",
            "出表",
            "缺勤",
            "工时",
            "最长",
            "最短",
            "公司名",
            "智能家居",
            "智能梯控",
        ],
    ) || ascii_prompt_contains_any(
        &lower,
        &[
            "who",
            "which",
            "how many",
            "count",
            "statistic",
            "sort",
            "rank",
            "table",
            "absence",
            "work hour",
        ],
    ) || prompt_requests_document_entity_scan(prompt)
        || prompt_requests_deterministic_aggregate_supply(prompt)
}

#[cfg(test)]
mod tests {
    use contracts::AssistantRunMessageView;
    use domain_model::ChatMessageRole;
    use serde_json::json;

    use super::*;

    fn quality_request(prompt: &str) -> CreateAssistantRunRequest {
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
    fn answer_quality_budget_increases_for_dissatisfaction_and_strong_complaint() {
        std::env::remove_var("ASSISTANT_RUN_ANSWER_QUALITY_RETRY_BUDGET");

        let dissatisfied = quality_request("客户有些不满意，回答不准确，再查一次");
        let strong = quality_request("客户很不满意，已经投诉了，前面反复答错。");

        assert!(assistant_run_request_expresses_dissatisfaction(
            &dissatisfied
        ));
        assert!(!assistant_run_request_expresses_strong_complaint(
            &dissatisfied
        ));
        assert_eq!(
            assistant_run_answer_quality_budget(&dissatisfied),
            AssistantRunQualityBudget {
                answer_retry_budget: ASSISTANT_RUN_ANSWER_QUALITY_DISSATISFIED_RETRY_BUDGET,
                react_step_budget: ASSISTANT_RUN_REACT_DEFAULT_MAX_STEPS,
                premium_action_budget: 1,
                reason: "dissatisfied",
            }
        );

        assert!(assistant_run_request_expresses_dissatisfaction(&strong));
        assert!(assistant_run_request_expresses_strong_complaint(&strong));
        assert_eq!(
            assistant_run_answer_quality_budget(&strong),
            AssistantRunQualityBudget {
                answer_retry_budget: ASSISTANT_RUN_ANSWER_QUALITY_STRONG_COMPLAINT_RETRY_BUDGET,
                react_step_budget: ASSISTANT_RUN_REACT_MAX_STEPS,
                premium_action_budget: 1,
                reason: "strong_complaint",
            }
        );
    }

    #[test]
    fn answer_quality_budget_uses_recent_messages_for_complaint_detection() {
        std::env::remove_var("ASSISTANT_RUN_ANSWER_QUALITY_RETRY_BUDGET");
        let mut request = quality_request("继续");
        request.messages = vec![
            AssistantRunMessageView {
                role: ChatMessageRole::User,
                content: "这次回答不准确".to_string(),
            },
            AssistantRunMessageView {
                role: ChatMessageRole::Assistant,
                content: "我重新检查。".to_string(),
            },
        ];

        assert!(assistant_run_request_expresses_dissatisfaction(&request));
        assert_eq!(
            assistant_run_answer_quality_budget(&request).reason,
            "dissatisfied"
        );
    }

    #[test]
    fn answer_quality_budget_allows_parse_recovery_premium_action() {
        std::env::remove_var("ASSISTANT_RUN_ANSWER_QUALITY_RETRY_BUDGET");
        let request = quality_request("这份扫描 PDF 里邓工是谁？请直接回答。");
        let evidence_state = json!({
            "status": "supplied",
            "supply_quality": {
                "selectedDatasetCount": 1,
                "suppliedItemCount": 1,
                "lowTextEvidenceCount": 1,
                "documentDegradedParseCount": 1,
                "notes": ["low_text_document_evidence"]
            }
        });

        let budget = assistant_run_answer_quality_budget_for_evidence(&request, &evidence_state);

        assert_eq!(budget.answer_retry_budget, 2);
        assert_eq!(budget.premium_action_budget, 1);
        assert_eq!(budget.reason, "parse_quality_recovery");
    }

    #[test]
    fn high_risk_quality_task_detects_structured_questions() {
        assert!(assistant_run_prompt_is_high_risk_quality_task(
            "请按公司名统计并出表"
        ));
        assert!(assistant_run_prompt_is_high_risk_quality_task(
            "who has the longest work hour"
        ));
        assert!(!assistant_run_prompt_is_high_risk_quality_task(
            "帮我润色一句普通问候"
        ));
    }
}
