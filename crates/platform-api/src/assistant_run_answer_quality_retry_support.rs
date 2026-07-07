use contracts::CreateAssistantRunRequest;
use serde_json::{json, Value};

use crate::{
    assistant_run_answer_quality_budget_support::{
        assistant_run_request_expresses_dissatisfaction, AssistantRunQualityBudget,
    },
    assistant_run_answer_quality_judge_support::{
        assistant_run_answer_contains_insufficient_evidence_marker,
        assistant_run_answer_contains_weak_confidence_marker,
        assistant_run_answer_quality_retry_allowed,
    },
    assistant_run_prompt_request_support::prompt_requests_document_entity_scan,
    assistant_run_react_support::assistant_run_react_output_contains_internal_marker,
    assistant_run_scope_policy_support::assistant_run_scope_supply_policy,
    assistant_run_scope_selection_support::assistant_run_scope_recommended_tool_actions,
    assistant_run_text_support::truncate_assistant_supply_text,
    external_channel_model_rejection_support::external_channel_output_is_generic_orchestration_ack,
    static_page_payload_support::{ensure_json_object, set_payload_value},
};

pub(crate) fn assistant_run_answer_quality_retry_reason(
    output_text: &str,
    evidence_state: &Value,
    request: &CreateAssistantRunRequest,
) -> Option<&'static str> {
    let output_text = output_text.trim();
    if output_text.is_empty() {
        return assistant_run_answer_quality_retry_allowed(output_text, evidence_state)
            .then_some("empty_answer");
    }
    if assistant_run_react_output_contains_internal_marker(output_text) {
        return assistant_run_answer_quality_retry_allowed(output_text, evidence_state)
            .then_some("internal_marker_answer");
    }
    if external_channel_output_is_generic_orchestration_ack(output_text) {
        return assistant_run_answer_quality_retry_allowed(output_text, evidence_state)
            .then_some("generic_orchestration_ack");
    }
    if assistant_run_answer_contains_insufficient_evidence_marker(output_text)
        && assistant_run_answer_quality_retry_allowed(output_text, evidence_state)
    {
        return Some("insufficient_or_uncertain_answer");
    }
    if assistant_run_request_expresses_dissatisfaction(request)
        && assistant_run_answer_contains_weak_confidence_marker(output_text)
        && assistant_run_answer_quality_retry_allowed(output_text, evidence_state)
    {
        return Some("dissatisfied_user_weak_confidence_answer");
    }
    None
}

pub(crate) fn assistant_run_answer_quality_retry_scope(
    selected_scope: &Value,
    prompt: &str,
    attempt_index: usize,
) -> Value {
    let mut retry_scope = selected_scope.clone();
    ensure_json_object(&mut retry_scope);
    let mut supply_policy = assistant_run_scope_supply_policy(&retry_scope);
    set_payload_value(&mut supply_policy, "retrievalPolicy", json!("detail_first"));
    set_payload_value(&mut supply_policy, "preferDetail", json!(true));
    set_payload_value(
        &mut supply_policy,
        "contextBudgetPolicy",
        json!("quality_first_token_tolerant"),
    );

    let mut actions = assistant_run_scope_recommended_tool_actions(selected_scope);
    actions.push("retrieval.search".to_string());
    actions.push("retrieval.read_detail".to_string());
    if prompt_requests_document_entity_scan(prompt) {
        actions.push("retrieval.scan_documents".to_string());
    }
    set_payload_value(
        &mut supply_policy,
        "recommendedActions",
        Value::Array(
            dedupe_strings(actions)
                .into_iter()
                .map(Value::String)
                .collect(),
        ),
    );
    set_payload_value(&mut retry_scope, "supply_policy", supply_policy);
    set_payload_value(
        &mut retry_scope,
        "answer_quality_gate",
        json!({
            "status": "retrying",
            "attempt": attempt_index,
            "strategy": "detail_first_react_expand_supply",
        }),
    );
    retry_scope
}

pub(crate) fn assistant_run_answer_quality_retry_request(
    request: &CreateAssistantRunRequest,
    retry_scope: &Value,
    attempt_index: usize,
    budget: usize,
    quality_budget: &AssistantRunQualityBudget,
    reason: &str,
    previous_answer: &str,
) -> CreateAssistantRunRequest {
    let mut retry_request = request.clone();
    retry_request.selected_scope = Some(retry_scope.clone());
    let mut startup_briefing = retry_request
        .startup_briefing
        .clone()
        .unwrap_or_else(|| json!({}));
    ensure_json_object(&mut startup_briefing);
    set_payload_value(
        &mut startup_briefing,
        "answerQualityGate",
        json!({
            "status": "retrying",
            "attempt": attempt_index,
            "budget": budget,
            "reactStepBudget": quality_budget.react_step_budget,
            "premiumActionBudget": quality_budget.premium_action_budget,
            "premiumActionUsed": 0,
            "reason": reason,
            "strategy": "Run ReAct to expand supply/read detail before producing customer-visible text.",
            "previousAnswerExcerpt": truncate_assistant_supply_text(previous_answer, 600),
            "instruction": "Do not repeat an insufficient/uncertain answer if supplied evidence can answer the question. Retrieve or read detail first, then answer directly from observations.",
        }),
    );
    retry_request.startup_briefing = Some(startup_briefing);
    retry_request
}

pub(crate) fn assistant_run_answer_quality_retry_evidence_state(
    mut evidence_state: Value,
    attempt_index: usize,
    quality_budget: &AssistantRunQualityBudget,
    reason: &str,
) -> Value {
    set_payload_value(
        &mut evidence_state,
        "answer_quality_gate",
        json!({
            "status": "retrying",
            "attempt": attempt_index,
            "budget": quality_budget.answer_retry_budget,
            "react_step_budget": quality_budget.react_step_budget,
            "premium_action_budget": quality_budget.premium_action_budget,
            "premium_action_used": 0,
            "vlm_upgrade_document_ids": [],
            "reason": reason,
            "strategy": "detail_first_react_expand_supply",
        }),
    );
    evidence_state
}

fn dedupe_strings(values: Vec<String>) -> Vec<String> {
    let mut deduped = Vec::new();
    for value in values {
        if !deduped.contains(&value) {
            deduped.push(value);
        }
    }
    deduped
}

#[cfg(test)]
mod tests {
    use contracts::CreateAssistantRunRequest;
    use serde_json::json;

    use super::*;

    fn retry_request(prompt: &str) -> CreateAssistantRunRequest {
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

    fn retryable_evidence_state() -> Value {
        json!({
            "status": "supplied",
            "supply_quality": {
                "selectedDatasetCount": 1,
                "suppliedItemCount": 2,
                "indexedEvidenceCount": 2,
                "fallbackChunkCount": 0,
                "datasetEntityScanCount": 0,
                "spreadsheetRowAnalysisCount": 0,
                "mediaContextCount": 0,
                "conversationMemoryItemCount": 0,
                "documentNotReadyCount": 0,
                "documentFailedCount": 0,
                "documentReparsingCount": 0
            }
        })
    }

    #[test]
    fn retry_reason_detects_empty_internal_ack_and_insufficient_answers() {
        let request = retry_request("老年人摔倒后怎么办？");
        let evidence_state = retryable_evidence_state();

        assert_eq!(
            assistant_run_answer_quality_retry_reason("", &evidence_state, &request),
            Some("empty_answer")
        );
        assert_eq!(
            assistant_run_answer_quality_retry_reason(
                "当前解析状态为 parse_degraded，无法回答。",
                &evidence_state,
                &request,
            ),
            Some("internal_marker_answer")
        );
        assert_eq!(
            assistant_run_answer_quality_retry_reason(
                "已收到指令。系统将结合知识库与数据源进行分析，并为您输出结论。",
                &evidence_state,
                &request,
            ),
            Some("generic_orchestration_ack")
        );
        assert_eq!(
            assistant_run_answer_quality_retry_reason(
                "供料中未直接检索到处理流程，只能基于通用知识建议。",
                &evidence_state,
                &request,
            ),
            Some("insufficient_or_uncertain_answer")
        );
    }

    #[test]
    fn retry_reason_respects_non_retryable_parse_unavailable_answer() {
        let request = retry_request("这份 PDF 写了什么？");
        let evidence_state = json!({
            "status": "supplied",
            "supply_quality": {
                "selectedDatasetCount": 1,
                "suppliedItemCount": 1,
                "indexedEvidenceCount": 0,
                "fallbackChunkCount": 0,
                "datasetEntityScanCount": 0,
                "datasetFactSnapshotCount": 0,
                "spreadsheetRowAnalysisCount": 0,
                "mediaContextCount": 0,
                "conversationMemoryItemCount": 0,
                "documentNotReadyCount": 1,
                "documentFailedCount": 0,
                "documentReparsingCount": 1
            }
        });

        assert_eq!(
            assistant_run_answer_quality_retry_reason(
                "文档正在解析中，目前无法确认完整内容。",
                &evidence_state,
                &request,
            ),
            None
        );
    }

    #[test]
    fn retry_scope_forces_detail_first_and_dedupes_actions() {
        let retry_scope = assistant_run_answer_quality_retry_scope(
            &json!({
                "datasets": ["dataset-1"],
                "supply_policy": {
                    "recommendedActions": ["retrieval.search", "report.plan"]
                }
            }),
            "请扫描文档公司名并统计。",
            2,
        );

        assert_eq!(
            retry_scope["supply_policy"]["retrievalPolicy"],
            json!("detail_first")
        );
        assert_eq!(retry_scope["supply_policy"]["preferDetail"], json!(true));
        assert_eq!(
            retry_scope["supply_policy"]["contextBudgetPolicy"],
            json!("quality_first_token_tolerant")
        );
        assert_eq!(
            retry_scope["supply_policy"]["recommendedActions"],
            json!([
                "retrieval.search",
                "report.plan",
                "retrieval.read_detail",
                "retrieval.scan_documents"
            ])
        );
        assert_eq!(retry_scope["answer_quality_gate"]["attempt"], json!(2));
    }

    #[test]
    fn retry_request_and_evidence_state_carry_budget_and_reason() {
        let request = retry_request("客户不满意，重新查。");
        let retry_scope = json!({"supply_policy": {"retrievalPolicy": "detail_first"}});
        let quality_budget = AssistantRunQualityBudget {
            answer_retry_budget: 3,
            react_step_budget: 4,
            premium_action_budget: 1,
            reason: "dissatisfied",
        };
        let previous_answer = "资料不足。".repeat(400);

        let retry_request = assistant_run_answer_quality_retry_request(
            &request,
            &retry_scope,
            1,
            3,
            &quality_budget,
            "insufficient_or_uncertain_answer",
            &previous_answer,
        );
        let gate = &retry_request.startup_briefing.as_ref().unwrap()["answerQualityGate"];

        assert_eq!(retry_request.selected_scope, Some(retry_scope));
        assert_eq!(gate["status"], json!("retrying"));
        assert_eq!(gate["reactStepBudget"], json!(4));
        assert_eq!(gate["premiumActionBudget"], json!(1));
        assert_eq!(gate["premiumActionUsed"], json!(0));
        assert!(
            gate["previousAnswerExcerpt"]
                .as_str()
                .unwrap()
                .chars()
                .count()
                <= 600
        );

        let retry_evidence_state = assistant_run_answer_quality_retry_evidence_state(
            json!({"status": "supplied"}),
            1,
            &quality_budget,
            "insufficient_or_uncertain_answer",
        );
        assert_eq!(
            retry_evidence_state["answer_quality_gate"]["budget"],
            json!(3)
        );
        assert_eq!(
            retry_evidence_state["answer_quality_gate"]["premium_action_used"],
            json!(0)
        );
    }
}
