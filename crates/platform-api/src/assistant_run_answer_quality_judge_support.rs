use contracts::CreateAssistantRunRequest;
use llm_gateway::{LlmRuntimeSelection, MODEL_LANE_ASSISTANT_CHAT};
use serde_json::{json, Value};

use crate::{
    assistant_run_answer_quality_budget_support::{
        assistant_run_prompt_is_high_risk_quality_task,
        assistant_run_request_expresses_dissatisfaction,
    },
    assistant_run_answer_quality_retry_support::assistant_run_answer_quality_retry_reason,
    assistant_run_answer_quality_spreadsheet_support::assistant_run_answer_satisfies_spreadsheet_row_analysis,
    assistant_run_model_evidence_state,
    assistant_run_point_list_support::assistant_run_answer_satisfies_retrieval_point_list,
    assistant_run_react_support::assistant_run_react_json_payload_candidate,
    build_assistant_run_model_supply_brief, complete_assistant_run_provider,
    prompt_match_support::prompt_contains_any,
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

pub(crate) fn assistant_run_supply_quality_needs_judge(evidence_state: &Value) -> bool {
    let Some(supply_quality) = evidence_state.get("supply_quality") else {
        return false;
    };
    [
        "fallbackChunkCount",
        "lowTextEvidenceCount",
        "documentDegradedParseCount",
        "documentNotReadyCount",
        "documentFailedCount",
        "documentReparsingCount",
    ]
    .iter()
    .any(|key| {
        supply_quality
            .get(*key)
            .and_then(Value::as_u64)
            .map(|count| count > 0)
            .unwrap_or(false)
    }) || supply_quality
        .get("notes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|note| {
            matches!(
                note.as_str(),
                Some("fallback_visible_document_chunks_used")
                    | Some("low_text_document_evidence")
                    | Some("parse_quality_degraded")
            )
        })
}

pub(crate) fn assistant_run_answer_is_short_for_structured_request(
    output_text: &str,
    request: &CreateAssistantRunRequest,
    evidence_state: &Value,
) -> bool {
    if !assistant_run_prompt_is_high_risk_quality_task(&request.prompt) {
        return false;
    }
    let supplied_count = evidence_state
        .get("supply_quality")
        .and_then(|quality| quality.get("suppliedItemCount"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    supplied_count > 0 && output_text.chars().count() < 80
}

pub(crate) fn assistant_run_answer_quality_judge_should_run(
    output_text: &str,
    evidence_state: &Value,
    request: &CreateAssistantRunRequest,
) -> bool {
    if !assistant_run_answer_quality_retry_allowed(output_text, evidence_state) {
        return false;
    }
    if assistant_run_request_expresses_dissatisfaction(request) {
        return true;
    }
    if assistant_run_answer_contains_weak_confidence_marker(output_text) {
        return true;
    }
    if assistant_run_answer_satisfies_spreadsheet_row_analysis(output_text, request, evidence_state)
    {
        return false;
    }
    if assistant_run_answer_satisfies_retrieval_point_list(output_text, request, evidence_state) {
        return false;
    }
    if assistant_run_prompt_is_high_risk_quality_task(&request.prompt) {
        return true;
    }
    if assistant_run_supply_quality_needs_judge(evidence_state) {
        return true;
    }
    assistant_run_answer_is_short_for_structured_request(output_text, request, evidence_state)
}

pub(crate) fn assistant_run_answer_quality_retry_allowed(
    output_text: &str,
    evidence_state: &Value,
) -> bool {
    if evidence_state.get("status").and_then(Value::as_str) == Some("not_requested") {
        return false;
    }
    let Some(supply_quality) = evidence_state.get("supply_quality") else {
        return false;
    };
    let selected_dataset_count = supply_quality
        .get("selectedDatasetCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let supplied_item_count = supply_quality
        .get("suppliedItemCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let conversation_memory_count = supply_quality
        .get("conversationMemoryItemCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if selected_dataset_count == 0 && supplied_item_count == 0 && conversation_memory_count == 0 {
        return false;
    }
    if assistant_run_answer_reports_actual_parse_unavailable(output_text, supply_quality) {
        return false;
    }
    true
}

pub(crate) fn assistant_run_answer_contains_weak_confidence_marker(output_text: &str) -> bool {
    let lower = output_text.to_ascii_lowercase();
    prompt_contains_any(
        output_text,
        &[
            "可能",
            "大概",
            "似乎",
            "推测",
            "猜测",
            "不确定",
            "不完整",
            "部分数据",
            "部分资料",
        ],
    ) || ["maybe", "probably", "likely", "uncertain", "partial"]
        .iter()
        .any(|marker| lower.contains(marker))
}

pub(crate) fn assistant_run_answer_contains_insufficient_evidence_marker(
    output_text: &str,
) -> bool {
    let lower = output_text.to_ascii_lowercase();
    prompt_contains_any(
        output_text,
        &[
            "资料不足",
            "材料不足",
            "信息不足",
            "证据不足",
            "数据不足",
            "上下文不足",
            "供料不足",
            "没有足够",
            "未提供足够",
            "不够回答",
            "不足以回答",
            "无法回答",
            "无法确认",
            "无法判断",
            "无法确定",
            "不能确认",
            "不能确定",
            "暂无法",
            "暂时无法",
            "未找到相关",
            "没有找到相关",
            "未直接检索到",
            "没有直接检索到",
            "未检索到",
            "文档未提及",
            "文档中未提及",
            "当前可见信息不足",
            "当前资料不足",
            "当前资料无法",
            "当前信息无法",
            "基于当前可见",
            "基于通用知识",
            "通用知识的建议",
            "当前可见",
            "部分解析状态",
            "部分解析",
            "解析不完整",
            "仅返回了标题字段",
            "只返回了标题字段",
            "原文内容尚未被平台解析",
            "仅基于当前可见",
            "只基于当前可见",
            "只能基于当前可见",
            "需要执行一次",
            "需要先检索",
            "需要先读取",
            "需要先获取",
            "请允许我先",
            "请回复\"继续\"",
            "请回复“继续”",
            "建议发起检索",
            "建议重新检索",
            "没有全量",
            "未全量",
            "完整、无遗漏",
            "需要补充资料",
            "建议补充资料",
            "建议上传",
        ],
    ) || [
        "insufficient information",
        "not enough information",
        "insufficient evidence",
        "cannot determine",
        "can't determine",
        "unable to determine",
        "unable to answer",
        "cannot answer",
        "not enough context",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn assistant_run_answer_reports_actual_parse_unavailable(
    output_text: &str,
    supply_quality: &Value,
) -> bool {
    let unavailable_count = [
        "documentNotReadyCount",
        "documentFailedCount",
        "documentReparsingCount",
    ]
    .iter()
    .filter_map(|key| supply_quality.get(*key).and_then(Value::as_u64))
    .sum::<u64>();
    let low_text_evidence_present = supply_quality
        .get("lowTextEvidenceCount")
        .and_then(Value::as_u64)
        .map(|count| count > 0)
        .unwrap_or_else(|| {
            supply_quality
                .get("notes")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .any(|note| note.as_str() == Some("low_text_document_evidence"))
        });
    if unavailable_count == 0 && !low_text_evidence_present {
        return false;
    }
    let answerable_supply_count = [
        "indexedEvidenceCount",
        "fallbackChunkCount",
        "datasetEntityScanCount",
        "datasetFactSnapshotCount",
        "spreadsheetRowAnalysisCount",
        "mediaContextCount",
        "conversationMemoryItemCount",
    ]
    .iter()
    .filter_map(|key| supply_quality.get(*key).and_then(Value::as_u64))
    .sum::<u64>();
    let low_text_only_answerable_limit = u64::from(low_text_evidence_present);
    if answerable_supply_count > low_text_only_answerable_limit {
        return false;
    }
    prompt_contains_any(
        output_text,
        &[
            "正在解析",
            "解析中",
            "解析失败",
            "解析未完成",
            "未解析完成",
            "资料不足",
            "信息不足",
            "无法回答",
            "无法确认",
            "重解析",
            "重试中",
            "文档未就绪",
            "低质量",
            "解析质量",
            "解析不完整",
            "仅返回了标题字段",
            "只返回了标题字段",
            "原文内容尚未被平台解析",
        ],
    )
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

pub(crate) async fn assistant_run_answer_quality_retry_reason_with_judge(
    output_text: &str,
    evidence_state: &Value,
    request: &CreateAssistantRunRequest,
    chat_runtime: &LlmRuntimeSelection,
) -> Option<&'static str> {
    if let Some(reason) =
        assistant_run_answer_quality_retry_reason(output_text, evidence_state, request)
    {
        return Some(reason);
    }
    if !assistant_run_answer_quality_judge_should_run(output_text, evidence_state, request) {
        return None;
    }
    let decision = complete_assistant_run_answer_quality_judge(
        chat_runtime,
        request,
        evidence_state,
        output_text,
    )
    .await?;
    assistant_run_answer_quality_retry_reason_from_judge_decision(&decision)
}

pub(crate) async fn complete_assistant_run_answer_quality_judge(
    chat_runtime: &LlmRuntimeSelection,
    request: &CreateAssistantRunRequest,
    evidence_state: &Value,
    output_text: &str,
) -> Option<AssistantRunAnswerQualityJudgeDecision> {
    if chat_runtime.mode == "placeholder" || !assistant_run_answer_quality_judge_enabled() {
        return None;
    }
    let provider_input =
        build_assistant_run_answer_quality_judge_input(request, evidence_state, output_text);
    let response = complete_assistant_run_provider(
        MODEL_LANE_ASSISTANT_CHAT,
        chat_runtime.mode.clone(),
        chat_runtime.provider.clone(),
        chat_runtime.model.clone(),
        provider_input,
    )
    .await
    .ok()?;
    parse_assistant_run_answer_quality_judge_decision(&response.output_text)
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

    fn placeholder_runtime() -> LlmRuntimeSelection {
        LlmRuntimeSelection {
            mode: "placeholder".to_string(),
            provider: "none".to_string(),
            model: "none".to_string(),
            lane: "assistant_chat".to_string(),
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
    fn supply_quality_needs_judge_for_counts_and_notes() {
        assert!(assistant_run_supply_quality_needs_judge(&json!({
            "supply_quality": {
                "fallbackChunkCount": 1
            }
        })));
        assert!(assistant_run_supply_quality_needs_judge(&json!({
            "supply_quality": {
                "fallbackChunkCount": 0,
                "notes": ["parse_quality_degraded"]
            }
        })));
        assert!(!assistant_run_supply_quality_needs_judge(&json!({
            "supply_quality": {
                "fallbackChunkCount": 0,
                "lowTextEvidenceCount": 0,
                "notes": ["spreadsheet_row_analysis_available"]
            }
        })));
        assert!(!assistant_run_supply_quality_needs_judge(&json!({})));
    }

    #[test]
    fn short_structured_answer_requires_high_risk_prompt_and_supply() {
        let structured = judge_request("这份 doc 里邓工是谁？请直接回答。");
        let casual = judge_request("帮我润色一句普通问候。");
        let evidence_state = json!({
            "supply_quality": {
                "suppliedItemCount": 2
            }
        });

        assert!(assistant_run_answer_is_short_for_structured_request(
            "邓工是项目负责人。",
            &structured,
            &evidence_state
        ));
        assert!(!assistant_run_answer_is_short_for_structured_request(
            "普通问候可以写得更自然。",
            &casual,
            &evidence_state
        ));
        assert!(!assistant_run_answer_is_short_for_structured_request(
            "邓工是项目负责人。",
            &structured,
            &json!({"supply_quality": {"suppliedItemCount": 0}})
        ));
    }

    #[test]
    fn retry_allowed_requires_requested_or_available_supply() {
        assert!(!assistant_run_answer_quality_retry_allowed(
            "需要补充资料。",
            &json!({
                "status": "not_requested",
                "supply_quality": {
                    "selectedDatasetCount": 1,
                    "suppliedItemCount": 1
                }
            })
        ));
        assert!(!assistant_run_answer_quality_retry_allowed(
            "需要补充资料。",
            &json!({
                "status": "supplied",
                "supply_quality": {
                    "selectedDatasetCount": 0,
                    "suppliedItemCount": 0,
                    "conversationMemoryItemCount": 0
                }
            })
        ));
        assert!(assistant_run_answer_quality_retry_allowed(
            "当前回答缺少细节。",
            &json!({
                "status": "supplied",
                "supply_quality": {
                    "selectedDatasetCount": 1,
                    "suppliedItemCount": 0,
                    "conversationMemoryItemCount": 0
                }
            })
        ));
        assert!(assistant_run_answer_quality_retry_allowed(
            "当前回答缺少细节。",
            &json!({
                "status": "supplied",
                "supply_quality": {
                    "selectedDatasetCount": 0,
                    "suppliedItemCount": 0,
                    "conversationMemoryItemCount": 1
                }
            })
        ));
    }

    #[test]
    fn retry_allowed_respects_real_parse_unavailable_answer() {
        let parse_blocked = json!({
            "status": "supplied",
            "supply_quality": {
                "selectedDatasetCount": 1,
                "suppliedItemCount": 1,
                "documentFailedCount": 1,
                "indexedEvidenceCount": 0,
                "fallbackChunkCount": 0,
                "datasetEntityScanCount": 0,
                "datasetFactSnapshotCount": 0,
                "spreadsheetRowAnalysisCount": 0,
                "mediaContextCount": 0,
                "conversationMemoryItemCount": 0
            }
        });
        assert!(!assistant_run_answer_quality_retry_allowed(
            "这份文档解析失败，当前无法回答。",
            &parse_blocked
        ));

        let expanded_supply = json!({
            "status": "supplied",
            "supply_quality": {
                "selectedDatasetCount": 1,
                "suppliedItemCount": 2,
                "documentFailedCount": 1,
                "indexedEvidenceCount": 1,
                "fallbackChunkCount": 1,
                "datasetEntityScanCount": 0,
                "datasetFactSnapshotCount": 0,
                "spreadsheetRowAnalysisCount": 0,
                "mediaContextCount": 0,
                "conversationMemoryItemCount": 0
            }
        });
        assert!(assistant_run_answer_quality_retry_allowed(
            "这份文档解析失败，当前无法回答。",
            &expanded_supply
        ));

        let low_text_only = json!({
            "status": "supplied",
            "supply_quality": {
                "selectedDatasetCount": 1,
                "suppliedItemCount": 1,
                "lowTextEvidenceCount": 1,
                "indexedEvidenceCount": 0,
                "fallbackChunkCount": 0,
                "datasetEntityScanCount": 0,
                "datasetFactSnapshotCount": 0,
                "spreadsheetRowAnalysisCount": 0,
                "mediaContextCount": 0,
                "conversationMemoryItemCount": 0
            }
        });
        assert!(!assistant_run_answer_quality_retry_allowed(
            "原文内容尚未被平台解析。",
            &low_text_only
        ));
    }

    #[test]
    fn judge_should_run_respects_retry_gate_and_quality_signals() {
        let no_supply = json!({
            "status": "supplied",
            "supply_quality": {
                "selectedDatasetCount": 0,
                "suppliedItemCount": 0,
                "conversationMemoryItemCount": 0
            }
        });
        let supplied = json!({
            "status": "supplied",
            "supply_quality": {
                "selectedDatasetCount": 1,
                "suppliedItemCount": 1,
                "conversationMemoryItemCount": 0,
                "indexedEvidenceCount": 1
            }
        });
        let casual = judge_request("帮我润色一句问候。");

        assert!(!assistant_run_answer_quality_judge_should_run(
            "问候语可以更自然。",
            &no_supply,
            &casual
        ));
        assert!(!assistant_run_answer_quality_judge_should_run(
            "问候语可以更自然。",
            &supplied,
            &casual
        ));

        assert!(assistant_run_answer_quality_judge_should_run(
            "根据当前资料，可能是门店店总负责。",
            &supplied,
            &casual
        ));
        assert!(assistant_run_answer_quality_judge_should_run(
            "已重新整理。",
            &supplied,
            &judge_request("客户不满意，重新查一次。")
        ));
        assert!(assistant_run_answer_quality_judge_should_run(
            "邓工是项目负责人。",
            &supplied,
            &judge_request("这份文档里邓工是谁？")
        ));
    }

    #[test]
    fn weak_confidence_marker_matches_chinese_and_ascii_terms() {
        assert!(assistant_run_answer_contains_weak_confidence_marker(
            "根据当前资料，可能是门店店总负责。"
        ));
        assert!(assistant_run_answer_contains_weak_confidence_marker(
            "This is likely a partial answer."
        ));
        assert!(!assistant_run_answer_contains_weak_confidence_marker(
            "总部视角应先展示区域、门店和品牌指标。"
        ));
    }

    #[test]
    fn insufficient_evidence_marker_matches_chinese_and_ascii_terms() {
        assert!(assistant_run_answer_contains_insufficient_evidence_marker(
            "供料中未直接检索到老年人摔倒后的处理流程。"
        ));
        assert!(assistant_run_answer_contains_insufficient_evidence_marker(
            "There is insufficient evidence to determine the answer."
        ));
        assert!(assistant_run_answer_contains_insufficient_evidence_marker(
            "当前文档为部分解析状态，需要先读取详情。"
        ));
    }

    #[test]
    fn insufficient_evidence_marker_ignores_grounded_answers() {
        assert!(!assistant_run_answer_contains_insufficient_evidence_marker(
            "长者摔倒后应先评估意识和疼痛，必要时联系医护并通知家属。"
        ));
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

    #[tokio::test]
    async fn retry_reason_with_judge_prefers_deterministic_reason() {
        let request = judge_request("老年人摔倒后怎么办？");
        let evidence_state = json!({
            "status": "supplied",
            "supply_quality": {
                "selectedDatasetCount": 1,
                "suppliedItemCount": 2,
                "conversationMemoryItemCount": 0
            }
        });

        let reason = assistant_run_answer_quality_retry_reason_with_judge(
            "当前资料不足，无法回答。",
            &evidence_state,
            &request,
            &placeholder_runtime(),
        )
        .await;

        assert_eq!(reason, Some("insufficient_or_uncertain_answer"));
    }

    #[tokio::test]
    async fn complete_judge_skips_placeholder_runtime() {
        let request = judge_request("请判断这份文档里邓工是谁？");
        let evidence_state = json!({
            "status": "supplied",
            "supply_quality": {
                "selectedDatasetCount": 1,
                "suppliedItemCount": 1,
                "conversationMemoryItemCount": 0
            }
        });

        let decision = complete_assistant_run_answer_quality_judge(
            &placeholder_runtime(),
            &request,
            &evidence_state,
            "邓工是项目负责人。",
        )
        .await;

        assert_eq!(decision, None);
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
