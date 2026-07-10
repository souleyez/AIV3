use std::collections::BTreeSet;

use crate::react_agent_contract::{
    AssistantRunReActActionType, AssistantRunReActDecision, AssistantRunReActStatus,
};
use crate::react_agent_tools::{
    assistant_run_react_policy_observation, react_final_answer_content_is_raw_observation,
    AssistantRunReactToolResult,
};
use crate::{
    assistant_run_evidence_supplied_count, assistant_run_scope_intent,
    complete_assistant_run_provider, env_flag, selected_dataset_ids_from_scope,
    selected_document_ids_from_scope, selected_scope_requests_conversation_memory,
    truncate_assistant_supply_text, value_array,
};
use chrono::Utc;
use domain_model::{AssistantRunId, DatasetId, DocumentId};
use llm_gateway::{render_runtime_manifest, LlmRuntimeSelection, MODEL_LANE_ASSISTANT_CHAT};
use serde_json::{json, Value};
use uuid::Uuid;

pub(crate) const ASSISTANT_RUN_REACT_DEFAULT_MAX_STEPS: usize = 3;
pub(crate) const ASSISTANT_RUN_REACT_MAX_STEPS: usize = 5;
pub(crate) const ASSISTANT_RUN_REACT_REASON_TRACE_LIMIT: usize = 240;

const ASSISTANT_RUN_REACT_MAX_STEPS_ENV: &str = "ASSISTANT_RUN_REACT_MAX_STEPS";
const ASSISTANT_RUN_REACT_MESSAGE_TRACE_LIMIT: usize = 240;

pub(crate) fn assistant_run_react_max_steps() -> usize {
    assistant_run_react_max_steps_from_value(
        std::env::var(ASSISTANT_RUN_REACT_MAX_STEPS_ENV)
            .ok()
            .as_deref(),
    )
}

fn assistant_run_react_max_steps_from_value(value: Option<&str>) -> usize {
    value
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(ASSISTANT_RUN_REACT_DEFAULT_MAX_STEPS)
        .clamp(1, ASSISTANT_RUN_REACT_MAX_STEPS)
}

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

pub(crate) fn assistant_run_assistant_message_content_from_artifacts(
    artifacts: &[Value],
) -> Option<String> {
    artifacts
        .iter()
        .find(|artifact| artifact.get("type").and_then(Value::as_str) == Some("assistant_message"))
        .and_then(|artifact| artifact.get("content").and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub(crate) fn assistant_run_replace_assistant_message_content(
    mut output_artifacts: Vec<Value>,
    content: &str,
) -> Vec<Value> {
    for artifact in output_artifacts.iter_mut() {
        if artifact.get("type").and_then(Value::as_str) == Some("assistant_message") {
            artifact["content"] = json!(content);
            return output_artifacts;
        }
    }
    output_artifacts.push(json!({
        "type": "assistant_message",
        "role": "assistant",
        "content": content,
        "source": "answer_quality_gate_exhausted_controlled_fallback",
    }));
    output_artifacts
}

pub(crate) fn assistant_run_sanitize_customer_facing_output_artifacts(
    output_artifacts: Vec<Value>,
) -> Vec<Value> {
    output_artifacts
        .into_iter()
        .map(|mut artifact| {
            if artifact.get("type").and_then(Value::as_str) == Some("assistant_message") {
                if let Some(content) = artifact
                    .get("content")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
                {
                    artifact["content"] =
                        json!(assistant_run_sanitize_customer_facing_answer_text(&content));
                }
            }
            artifact
        })
        .collect()
}

pub(crate) fn assistant_run_sanitize_customer_facing_answer_text(output_text: &str) -> String {
    let mut sanitized = output_text.trim().to_string();
    for (raw, replacement) in [
        (
            "low_text_coverage_fallback_unavailable",
            "解析质量较低，正文未成功提取",
        ),
        ("low_text_coverage", "解析质量较低"),
        ("parse_degraded", "解析质量较低"),
        ("document_parse_status", "文档解析状态"),
        ("asset_parse_status", "资产解析状态"),
        ("model_status", "解析状态"),
    ] {
        sanitized = sanitized.replace(raw, replacement);
    }
    if assistant_run_react_output_contains_internal_marker(&sanitized) {
        return "本轮回答包含内部检索指令，系统已拦截未直接展示。请稍后重试，我会基于当前可见资料直接给出结论。".to_string();
    }
    sanitized
}

pub(crate) async fn complete_assistant_run_react_natural_answer_fallback(
    chat_runtime: &LlmRuntimeSelection,
    provider_input: String,
) -> Option<(String, Value)> {
    if chat_runtime.mode == "placeholder" {
        return None;
    }
    let response = complete_assistant_run_provider(
        MODEL_LANE_ASSISTANT_CHAT,
        chat_runtime.mode.clone(),
        chat_runtime.provider.clone(),
        chat_runtime.model.clone(),
        provider_input,
    )
    .await
    .ok()?;
    let answer = response.output_text.trim().to_string();
    if answer.is_empty() {
        return None;
    }
    Some((answer, render_runtime_manifest(&response.runtime)))
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

pub(crate) fn assistant_run_react_attach_trace(
    runtime_manifest: &mut Value,
    trace_id: Uuid,
    assistant_run_id: Option<AssistantRunId>,
    trace_steps: &[Value],
) {
    let trace = assistant_run_react_trace_manifest(trace_id, assistant_run_id, trace_steps);
    if let Some(object) = runtime_manifest.as_object_mut() {
        object.insert("react_trace".to_string(), trace);
    } else {
        *runtime_manifest = json!({
            "runtime": runtime_manifest.clone(),
            "react_trace": trace,
        });
    }
}

pub(crate) fn assistant_run_react_trace_manifest(
    trace_id: Uuid,
    assistant_run_id: Option<AssistantRunId>,
    trace_steps: &[Value],
) -> Value {
    json!({
        "trace_id": trace_id.to_string(),
        "assistant_run_id": assistant_run_id.map(|id| id.to_string()),
        "steps": trace_steps,
    })
}

pub(crate) fn assistant_run_react_trace_trail_step(
    trace_id: Uuid,
    assistant_run_id: Option<AssistantRunId>,
    trace_steps: &[Value],
) -> Value {
    json!({
        "status": "completed",
        "label": "ReAct 安全追踪",
        "trace_id": trace_id.to_string(),
        "assistant_run_id": assistant_run_id.map(|id| id.to_string()),
        "step_count": trace_steps.len(),
        "react_trace": trace_steps,
        "at": Utc::now(),
    })
}

pub(crate) fn assistant_run_react_invalid_trace_step(
    trace_id: Uuid,
    assistant_run_id: Option<AssistantRunId>,
    pass_number: usize,
    safe_error_code: &str,
    duration_ms: u128,
) -> Value {
    json!({
        "trace_id": trace_id.to_string(),
        "assistant_run_id": assistant_run_id.map(|id| id.to_string()),
        "pass_number": pass_number,
        "action_type": "invalid_action",
        "reason_summary": "",
        "status": "failed",
        "denied_count": 0,
        "returned_count": 0,
        "duration_ms": bounded_duration_ms(duration_ms),
        "safe_error_code": safe_error_code,
        "safe_message": "模型未返回有效动作",
    })
}

pub(crate) fn assistant_run_react_trace_step(
    trace_id: Uuid,
    assistant_run_id: Option<AssistantRunId>,
    pass_number: usize,
    action: &AssistantRunReActDecision,
    observation_summary: &Value,
    duration_ms: u128,
) -> Value {
    json!({
        "trace_id": trace_id.to_string(),
        "assistant_run_id": assistant_run_id.map(|id| id.to_string()),
        "pass_number": pass_number,
        "action_type": action.action_type.as_str(),
        "reason_summary": redact_react_trace_text(&action.reason_summary, ASSISTANT_RUN_REACT_REASON_TRACE_LIMIT),
        "status": observation_summary.get("status").and_then(Value::as_str).unwrap_or("unknown"),
        "denied_count": observation_summary.get("denied_count").and_then(Value::as_u64).unwrap_or(0),
        "returned_count": observation_summary.get("returned_count").and_then(Value::as_u64).unwrap_or(0),
        "detail_target_count": observation_summary.get("detail_target_count").and_then(Value::as_u64).unwrap_or(0),
        "duration_ms": bounded_duration_ms(duration_ms),
        "safe_error_code": observation_summary.get("safe_error_code").cloned().unwrap_or(Value::Null),
        "safe_message": observation_summary.get("safe_message").and_then(Value::as_str).unwrap_or(""),
    })
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

pub(crate) fn build_assistant_run_react_natural_fallback_input(
    base_input: String,
    observations: &[Value],
    reason: &str,
) -> String {
    let mut sections = vec![
        base_input,
        "ReAct 自然回答兜底要求：上一轮工具规划没有产出可直接展示给用户的最终回答。请改为面向用户直接自然语言作答；不要输出 JSON、observation、execution_trail、react_trace、tool_trace、runtime_manifest 或 provider 原始载荷。".to_string(),
        "如果当前没有拿到 DataMax 可见证据，只在涉及 DataMax 数据/文档/权限/产物状态时说明“当前不可见/未供料”；普通问题继续用你的通用能力回答。".to_string(),
        format!("兜底原因：{reason}"),
    ];
    if !observations.is_empty() {
        let summaries = observations
            .iter()
            .map(assistant_run_react_observation_summary)
            .collect::<Vec<_>>();
        sections.push(format!(
            "已执行动作摘要（仅用于判断下一句回答，不要原样输出）：{}",
            serde_json::to_string(&summaries).unwrap_or_else(|_| "[]".to_string())
        ));
    }
    sections.join("\n\n")
}

pub(crate) fn assistant_run_react_step_limit_followup_message(evidence_state: &Value) -> String {
    let supplied_count = assistant_run_evidence_supplied_count(evidence_state);
    if supplied_count > 0 {
        return format!(
            "我已检索到 {supplied_count} 条相关资料，但还没有定位到足够明确的专门流程条款。可以继续：请补充制度名称、页码或关键词；如果没有专门制度，我也可以先按已检索到的突发事件处置线索和通用养老机构应急规范，整理一版“现场处置、家属沟通、上报记录、后续复盘”的流程。"
        );
    }
    "当前没有检索到可见资料。请补充相关制度文件、文档范围或关键词，我会继续检索并整理可执行流程。"
        .to_string()
}

pub(crate) fn assistant_run_react_attach_natural_fallback_runtime(
    runtime_manifest: &mut Value,
    reason: &str,
    fallback_runtime_manifest: Value,
) {
    if let Some(object) = runtime_manifest.as_object_mut() {
        object.insert(
            "natural_answer_fallback".to_string(),
            json!({
                "reason": reason,
                "runtime": fallback_runtime_manifest,
            }),
        );
    }
}

pub(crate) fn assistant_run_react_unavailable_natural_answer_message() -> String {
    "模型暂时没有返回可展示的自然语言回答，请稍后重试或换一种问法。".to_string()
}

pub(crate) fn assistant_run_compact_evidence_items_for_natural_fallback(
    evidence_state: &Value,
    limit: usize,
) -> Vec<Value> {
    value_array(
        evidence_state
            .get("supplied_items")
            .cloned()
            .unwrap_or_else(|| json!([])),
    )
    .into_iter()
    .take(limit)
    .map(|item| {
        json!({
            "type": item.get("type").and_then(Value::as_str).unwrap_or("evidence"),
            "summary": item
                .get("summary")
                .and_then(Value::as_str)
                .map(|value| truncate_assistant_supply_text(value, 260))
                .unwrap_or_default(),
            "source_locator": item.get("source_locator").cloned().unwrap_or(Value::Null),
            "document_id": item.get("document_id").cloned().unwrap_or(Value::Null),
            "chunk_index": item.get("chunk_index").cloned().unwrap_or(Value::Null),
            "content_excerpt": item
                .get("content_excerpt")
                .or_else(|| item.get("content"))
                .and_then(Value::as_str)
                .map(|value| truncate_assistant_supply_text(value, 700))
                .unwrap_or_default(),
            "fallback_reason": item.get("fallback_reason").cloned().unwrap_or(Value::Null),
        })
    })
    .collect()
}

pub(crate) fn assistant_run_react_call_id_from_value(value: &Value) -> Option<String> {
    value
        .get("tool_call_id")
        .or_else(|| value.get("toolCallId"))
        .or_else(|| value.get("call_id"))
        .or_else(|| value.get("callId"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub(crate) fn assistant_run_react_observation_call_id(observation: &Value) -> Option<String> {
    assistant_run_react_call_id_from_value(observation).or_else(|| {
        observation
            .get("tool_call")
            .or_else(|| observation.get("toolCall"))
            .and_then(assistant_run_react_call_id_from_value)
    })
}

pub(crate) fn assistant_run_react_observation_status(observation: &Value) -> Option<&str> {
    observation
        .get("status")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub(crate) fn assistant_run_react_has_completed_action(
    observations: &[Value],
    action_type: &str,
) -> bool {
    observations.iter().any(|observation| {
        observation
            .get("status")
            .and_then(Value::as_str)
            .is_some_and(|status| status == "completed")
            && observation
                .get("action_type")
                .or_else(|| observation.get("actionType"))
                .and_then(Value::as_str)
                .is_some_and(|value| value == action_type)
    })
}

pub(crate) struct AssistantRunReactPendingToolOutput {
    pub(crate) call_id: String,
    pub(crate) repeated: bool,
}

pub(crate) fn assistant_run_react_replays_completed_tool_call(
    decision: &AssistantRunReActDecision,
    observations: &[Value],
) -> Option<String> {
    let call_id = assistant_run_react_call_id_from_value(&decision.arguments)?;
    let already_completed = observations.iter().any(|observation| {
        assistant_run_react_observation_call_id(observation).as_deref() == Some(call_id.as_str())
            && assistant_run_react_observation_status(observation).is_some_and(|status| {
                matches!(status, "completed" | "failed" | "rejected" | "denied")
            })
    });
    already_completed.then_some(call_id)
}

pub(crate) fn assistant_run_react_pending_tool_output(
    observations: &[Value],
) -> Option<AssistantRunReactPendingToolOutput> {
    let pending_observation = observations.iter().rev().find(|observation| {
        assistant_run_react_observation_status(observation).is_some_and(|status| {
            matches!(
                status,
                "tool_calls_emitted"
                    | "tool_call_requested"
                    | "pending_tool_output"
                    | "tool_output_missing"
            )
        })
    })?;
    let call_id = assistant_run_react_observation_call_id(pending_observation)?;
    let resolved = observations.iter().any(|observation| {
        assistant_run_react_observation_call_id(observation).as_deref() == Some(call_id.as_str())
            && assistant_run_react_observation_status(observation).is_some_and(|status| {
                matches!(status, "completed" | "failed" | "rejected" | "denied")
            })
    });
    if resolved {
        return None;
    }
    let repeated = observations
        .iter()
        .rev()
        .take(2)
        .filter(|observation| {
            assistant_run_react_observation_call_id(observation).as_deref()
                == Some(call_id.as_str())
                && assistant_run_react_observation_status(observation).is_some_and(|status| {
                    matches!(
                        status,
                        "tool_calls_emitted"
                            | "tool_call_requested"
                            | "pending_tool_output"
                            | "tool_output_missing"
                    )
                })
        })
        .count()
        >= 2;

    Some(AssistantRunReactPendingToolOutput { call_id, repeated })
}

pub(crate) fn assistant_run_react_repeats_no_progress_action(
    decision: &AssistantRunReActDecision,
    observations: &[Value],
) -> bool {
    let action_type = decision.action_type.as_str();
    observations
        .iter()
        .rev()
        .take(2)
        .filter(|observation| {
            observation
                .get("status")
                .and_then(Value::as_str)
                .is_some_and(|status| matches!(status, "rejected" | "failed" | "denied"))
                && observation
                    .get("action_type")
                    .or_else(|| observation.get("actionType"))
                    .and_then(Value::as_str)
                    .is_some_and(|value| value == action_type)
        })
        .count()
        >= 2
}

pub(crate) fn assistant_run_react_should_repair_terminal_action(
    action: &AssistantRunReActDecision,
    selected_scope: &Value,
    evidence_state: &Value,
    observations: &[Value],
) -> bool {
    action.action_type == AssistantRunReActActionType::FinalAnswer
        && assistant_run_react_scope_requires_supply(selected_scope)
        && !assistant_run_react_has_supply_observation(evidence_state, observations)
}

pub(crate) fn assistant_run_react_scope_requires_supply(selected_scope: &Value) -> bool {
    assistant_run_scope_intent(selected_scope) != "ordinary_chat"
        && (!selected_dataset_ids_from_scope(selected_scope).is_empty()
            || selected_scope_requests_conversation_memory(selected_scope))
}

pub(crate) fn assistant_run_react_has_supply_observation(
    evidence_state: &Value,
    observations: &[Value],
) -> bool {
    assistant_run_evidence_supplied_count(evidence_state) > 0
        || observations.iter().any(|observation| {
            observation
                .get("status")
                .and_then(Value::as_str)
                .is_some_and(|status| status == "completed")
                && observation
                    .get("action_type")
                    .or_else(|| observation.get("actionType"))
                    .and_then(Value::as_str)
                    .is_some_and(|action_type| {
                        matches!(
                            action_type,
                            "retrieve_evidence"
                                | "read_document_detail"
                                | "upgrade_parse_vlm"
                                | "recall_conversation_memory"
                        )
                    })
        })
}

pub(crate) fn assistant_run_react_enabled(runtime_mode: &str) -> bool {
    runtime_mode != "placeholder" && env_flag("ASSISTANT_RUN_REACT_ENABLED", false)
}

pub(crate) fn assistant_run_is_plain_ordinary_chat_scope(
    selected_scope: Option<&Value>,
    evidence_state: Option<&Value>,
    current_artifact: Option<&Value>,
) -> bool {
    if current_artifact.is_some() {
        return false;
    }
    let Some(selected_scope) = selected_scope else {
        return true;
    };
    let no_selected_supply = selected_dataset_ids_from_scope(selected_scope).is_empty()
        && selected_document_ids_from_scope(selected_scope).is_empty()
        && !selected_scope_requests_conversation_memory(selected_scope);
    let ordinary_intent = assistant_run_scope_intent(selected_scope) == "ordinary_chat";
    let no_evidence = evidence_state
        .and_then(|state| state.get("status"))
        .and_then(Value::as_str)
        .map(|status| matches!(status, "not_requested" | "empty"))
        .unwrap_or(true)
        && evidence_state
            .map(|state| assistant_run_evidence_supplied_count(state) == 0)
            .unwrap_or(true);

    ordinary_intent && no_selected_supply && no_evidence
}

pub(crate) fn assistant_run_react_enabled_for_scope(
    runtime_mode: &str,
    selected_scope: &Value,
    current_artifact: Option<&Value>,
) -> bool {
    assistant_run_react_enabled(runtime_mode)
        && assistant_run_react_scope_allows_tools(selected_scope, current_artifact)
}

pub(crate) fn assistant_run_react_scope_allows_tools(
    selected_scope: &Value,
    current_artifact: Option<&Value>,
) -> bool {
    (assistant_run_scope_intent(selected_scope) != "ordinary_chat" || current_artifact.is_some())
        && !assistant_run_is_plain_ordinary_chat_scope(Some(selected_scope), None, current_artifact)
}

#[allow(dead_code)]
pub(crate) fn build_react_protocol_repair(
    decision: &AssistantRunReActDecision,
    observations: &[Value],
    selected_scope: &Value,
    evidence_state: &Value,
) -> Option<AssistantRunReactToolResult> {
    build_react_protocol_repair_at_step(decision, observations, selected_scope, evidence_state, 0)
}

pub(crate) fn build_react_protocol_repair_at_step(
    decision: &AssistantRunReActDecision,
    observations: &[Value],
    selected_scope: &Value,
    evidence_state: &Value,
    step_index: usize,
) -> Option<AssistantRunReactToolResult> {
    if let Some(call_id) = assistant_run_react_replays_completed_tool_call(decision, observations) {
        return Some(build_assistant_run_react_policy_repair_result(
            decision,
            "duplicate tool-call replay detected; continue from the existing observation instead of replaying the same call.",
            step_index,
            vec![format!("tool_call:{call_id}")],
            "duplicate_tool_call_replay",
        ));
    }

    if let Some(pending) = assistant_run_react_pending_tool_output(observations) {
        let (message, repair_code) = if pending.repeated {
            (
                "tool-call liveness stall detected; request a continuation or choose a different whitelisted action.",
                "tool_call_liveness_stall",
            )
        } else {
            (
                "tool-call output is missing; wait for or repair the tool observation before continuing.",
                "missing_tool_output",
            )
        };
        return Some(build_assistant_run_react_policy_repair_result(
            decision,
            message,
            step_index,
            vec![format!("tool_call:{}", pending.call_id)],
            repair_code,
        ));
    }

    if assistant_run_react_should_repair_terminal_action(
        decision,
        selected_scope,
        evidence_state,
        observations,
    ) {
        return Some(assistant_run_react_policy_observation(
            decision,
            "final_answer requires a supply observation; choose a whitelisted action before answering.",
            step_index,
        ));
    }

    if decision.status == AssistantRunReActStatus::ReportChoice
        && !assistant_run_react_has_completed_action(observations, "list_report_options")
    {
        return Some(assistant_run_react_policy_observation(
            decision,
            "report_choice requires list_report_options observation before choosing a report path.",
            step_index,
        ));
    }

    if let Some(denied) = react_requested_scope_denial(decision, selected_scope) {
        return Some(build_assistant_run_react_policy_repair_result(
            decision,
            "requested dataset or document is outside the current selected scope.",
            step_index,
            vec![denied],
            "scope_denied",
        ));
    }

    if assistant_run_react_repeats_no_progress_action(decision, observations) {
        return Some(build_assistant_run_react_policy_repair_result(
            decision,
            "same no-progress action repeated; choose a different whitelisted action or cannot_answer.",
            step_index,
            vec![format!("repeated:{}", decision.action_type.as_str())],
            "repeated_no_progress_action",
        ));
    }

    None
}

fn build_assistant_run_react_policy_repair_result(
    action: &AssistantRunReActDecision,
    message: &str,
    step_index: usize,
    denied: Vec<String>,
    repair_code: &str,
) -> AssistantRunReactToolResult {
    AssistantRunReactToolResult {
        observation: json!({
            "status": "rejected",
            "action_type": "policy_observation",
            "actionType": "policy_observation",
            "message": message,
            "denied": denied,
            "items": [],
            "limits": {},
            "repair_required": true,
            "repair_code": repair_code,
            "step": step_index,
        }),
        trail_step: json!({
            "status": "rejected",
            "label": "ReAct 协议修复",
            "react_action": action.action_type.as_str(),
            "message": message,
            "react_step": step_index,
            "at": Utc::now(),
        }),
        final_answer: None,
    }
}

fn react_requested_scope_denial(
    decision: &AssistantRunReActDecision,
    selected_scope: &Value,
) -> Option<String> {
    let requested_dataset_id = decision
        .arguments
        .get("dataset_id")
        .or_else(|| decision.arguments.get("datasetId"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(requested_dataset_id) = requested_dataset_id {
        let selected_dataset_ids = selected_dataset_ids_from_scope(selected_scope);
        let requested = Uuid::parse_str(requested_dataset_id).ok().map(DatasetId);
        if requested.is_none_or(|requested| !selected_dataset_ids.contains(&requested)) {
            return Some(format!("dataset:{requested_dataset_id}"));
        }
    }

    let requested_document_id = decision
        .arguments
        .get("document_id")
        .or_else(|| decision.arguments.get("documentId"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(requested_document_id) = requested_document_id {
        let selected_document_ids = selected_document_ids_from_scope(selected_scope);
        let requested = Uuid::parse_str(requested_document_id).ok().map(DocumentId);
        if !selected_document_ids.is_empty()
            && requested.is_none_or(|requested| !selected_document_ids.contains(&requested))
        {
            return Some(format!("document:{requested_document_id}"));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::react_agent_contract::AssistantRunReActStatus;
    use serde_json::json;

    fn react_test_decision(
        action_type: AssistantRunReActActionType,
        arguments: Value,
    ) -> AssistantRunReActDecision {
        AssistantRunReActDecision {
            status: AssistantRunReActStatus::Act,
            intent: None,
            action_type,
            reason_summary: "test".to_string(),
            arguments,
            requires_confirmation: false,
            answer: None,
            citations: Vec::new(),
            conversation_state: json!({}),
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
    fn react_max_steps_uses_default_and_clamps_env_values() {
        assert_eq!(
            assistant_run_react_max_steps_from_value(None),
            ASSISTANT_RUN_REACT_DEFAULT_MAX_STEPS
        );
        assert_eq!(
            assistant_run_react_max_steps_from_value(Some("bad")),
            ASSISTANT_RUN_REACT_DEFAULT_MAX_STEPS
        );
        assert_eq!(assistant_run_react_max_steps_from_value(Some("0")), 1);
        assert_eq!(
            assistant_run_react_max_steps_from_value(Some("9")),
            ASSISTANT_RUN_REACT_MAX_STEPS
        );
        assert_eq!(assistant_run_react_max_steps_from_value(Some("4")), 4);
    }

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
    fn assistant_message_content_from_artifacts_extracts_first_non_empty_message() {
        let artifacts = vec![
            json!({"type": "html_artifact", "content": "ignored"}),
            json!({"type": "assistant_message", "content": "  可以直接展示  "}),
            json!({"type": "assistant_message", "content": "later ignored"}),
        ];

        assert_eq!(
            assistant_run_assistant_message_content_from_artifacts(&artifacts),
            Some("可以直接展示".to_string())
        );
    }

    #[test]
    fn assistant_message_content_from_artifacts_ignores_missing_or_blank_content() {
        assert!(assistant_run_assistant_message_content_from_artifacts(&[
            json!({"type": "assistant_message", "content": "   "}),
            json!({"type": "assistant_message"}),
            json!({"type": "html_artifact", "content": "not a message"})
        ])
        .is_none());
    }

    #[test]
    fn replace_assistant_message_content_updates_existing_message_and_preserves_other_artifacts() {
        let artifacts = vec![
            json!({"type": "html_artifact", "id": "artifact-1"}),
            json!({
                "type": "assistant_message",
                "role": "assistant",
                "content": "old",
                "source": "existing_source",
                "extra": {"keep": true}
            }),
            json!({"type": "assistant_message", "content": "later untouched"}),
        ];

        assert_eq!(
            assistant_run_replace_assistant_message_content(artifacts, "new answer"),
            vec![
                json!({"type": "html_artifact", "id": "artifact-1"}),
                json!({
                    "type": "assistant_message",
                    "role": "assistant",
                    "content": "new answer",
                    "source": "existing_source",
                    "extra": {"keep": true}
                }),
                json!({"type": "assistant_message", "content": "later untouched"}),
            ]
        );
    }

    #[test]
    fn replace_assistant_message_content_adds_message_when_missing() {
        assert_eq!(
            assistant_run_replace_assistant_message_content(
                vec![json!({"type": "html_artifact", "id": "artifact-1"})],
                "fallback answer",
            ),
            vec![
                json!({"type": "html_artifact", "id": "artifact-1"}),
                json!({
                    "type": "assistant_message",
                    "role": "assistant",
                    "content": "fallback answer",
                    "source": "answer_quality_gate_exhausted_controlled_fallback",
                }),
            ]
        );
    }

    #[test]
    fn sanitize_customer_facing_answer_text_replaces_internal_parse_status_identifiers() {
        let sanitized = assistant_run_sanitize_customer_facing_answer_text(
            "当前解析状态为 parse_degraded，原因是 low_text_coverage。",
        );

        assert!(!sanitized.contains("parse_degraded"));
        assert!(!sanitized.contains("low_text_coverage"));
        assert!(sanitized.contains("解析质量较低"));
    }

    #[test]
    fn sanitize_customer_facing_answer_text_blocks_internal_markers() {
        let sanitized = assistant_run_sanitize_customer_facing_answer_text(
            "Observation: {\"items\":[{\"summary\":\"内部供料\"}]}",
        );

        assert!(sanitized.contains("内部检索指令"));
        assert!(!sanitized.contains("Observation:"));
    }

    #[test]
    fn sanitize_customer_facing_output_artifacts_only_changes_assistant_messages() {
        let sanitized = assistant_run_sanitize_customer_facing_output_artifacts(vec![
            json!({
                "type": "assistant_message",
                "content": "当前解析状态为 parse_degraded，原因是 low_text_coverage。",
                "source": "keep"
            }),
            json!({
                "type": "html_artifact",
                "content": "parse_degraded"
            }),
        ]);

        let assistant_content = sanitized[0]["content"].as_str().unwrap_or_default();
        assert!(!assistant_content.contains("parse_degraded"));
        assert!(!assistant_content.contains("low_text_coverage"));
        assert!(assistant_content.contains("解析质量较低"));
        assert_eq!(sanitized[0]["source"], json!("keep"));
        assert_eq!(sanitized[1]["content"], json!("parse_degraded"));
    }

    #[tokio::test]
    async fn natural_answer_fallback_skips_placeholder_runtime() {
        assert_eq!(
            complete_assistant_run_react_natural_answer_fallback(
                &placeholder_runtime(),
                "请直接回答。".to_string(),
            )
            .await,
            None
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
    fn react_trace_step_manifest_and_trail_are_safe_and_counted() {
        let mut decision = react_test_decision(
            AssistantRunReActActionType::ReadDocumentDetail,
            json!({"document_id": Uuid::nil().to_string()}),
        );
        decision.reason_summary = "读取公开摘要".to_string();
        let trace_id = Uuid::nil();
        let summary = json!({
            "status": "completed",
            "denied_count": 1,
            "returned_count": 2,
            "detail_target_count": 3,
            "safe_error_code": "ok",
            "safe_message": "工具完成",
        });

        let trace_step =
            assistant_run_react_trace_step(trace_id, None, 2, &decision, &summary, u128::MAX);
        assert_eq!(trace_step["trace_id"], trace_id.to_string());
        assert_eq!(trace_step["pass_number"], 2);
        assert_eq!(trace_step["action_type"], "read_document_detail");
        assert_eq!(trace_step["reason_summary"], "读取公开摘要");
        assert_eq!(trace_step["safe_message"], "工具完成");
        assert_eq!(trace_step["duration_ms"], u64::MAX);

        let trace_steps = vec![trace_step.clone()];
        let manifest = assistant_run_react_trace_manifest(trace_id, None, &trace_steps);
        assert_eq!(manifest["steps"][0], trace_step);

        let trail_step = assistant_run_react_trace_trail_step(trace_id, None, &trace_steps);
        assert_eq!(trail_step["step_count"], 1);
        assert_eq!(trail_step["react_trace"][0], trace_step);

        let mut runtime_manifest = json!({"runtime": "test"});
        assistant_run_react_attach_trace(&mut runtime_manifest, trace_id, None, &trace_steps);
        assert_eq!(runtime_manifest["react_trace"]["steps"][0], trace_step);
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

    #[test]
    fn react_natural_fallback_input_summarizes_observations_without_raw_output() {
        let input = build_assistant_run_react_natural_fallback_input(
            "用户问题：最近流程怎么走？".to_string(),
            &[json!({
                "status": "completed",
                "action_type": "retrieve_evidence",
                "items": [{"id": "doc-1"}]
            })],
            "react_step_limit",
        );

        assert!(input.contains("自然回答兜底"));
        assert!(input.contains("不要输出 JSON"));
        assert!(input.contains("不要原样输出"));
        assert!(input.contains("当前不可见/未供料"));
        assert!(input.contains("兜底原因：react_step_limit"));
        assert!(input.contains("returned_count"));
    }

    #[test]
    fn react_step_limit_followup_and_runtime_fallback_are_customer_safe() {
        let message = assistant_run_react_step_limit_followup_message(&json!({
            "status": "supplied",
            "supplied_items": [{"type": "retrieval_evidence"}]
        }));

        assert!(message.contains("已检索到 1 条相关资料"));
        assert!(message.contains("现场处置"));
        assert!(!message.contains("连续执行步数上限"));

        let empty_message = assistant_run_react_step_limit_followup_message(&json!({}));
        assert!(empty_message.contains("当前没有检索到可见资料"));

        let mut runtime_manifest = json!({"runtime": "test"});
        assistant_run_react_attach_natural_fallback_runtime(
            &mut runtime_manifest,
            "react_step_limit",
            json!({"provider": "chat"}),
        );
        assert_eq!(
            runtime_manifest["natural_answer_fallback"]["reason"],
            "react_step_limit"
        );
        assert_eq!(
            runtime_manifest["natural_answer_fallback"]["runtime"]["provider"],
            "chat"
        );
        assert!(assistant_run_react_unavailable_natural_answer_message().contains("请稍后重试"));
    }

    #[test]
    fn react_compact_fallback_evidence_items_are_limited_and_safe() {
        let long_summary = "摘要".repeat(180);
        let long_content = "正文".repeat(420);
        let items = assistant_run_compact_evidence_items_for_natural_fallback(
            &json!({
                "supplied_items": [
                    {
                        "type": "retrieval_evidence",
                        "summary": long_summary,
                        "source_locator": "document://doc-1/chunks/2",
                        "document_id": "doc-1",
                        "chunk_index": 2,
                        "content": long_content,
                        "fallback_reason": "expanded_supply"
                    },
                    {
                        "summary": "second",
                        "content_excerpt": "explicit excerpt"
                    }
                ]
            }),
            1,
        );

        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["type"], "retrieval_evidence");
        assert_eq!(items[0]["source_locator"], "document://doc-1/chunks/2");
        assert_eq!(items[0]["document_id"], "doc-1");
        assert_eq!(items[0]["chunk_index"], 2);
        assert_eq!(items[0]["fallback_reason"], "expanded_supply");
        assert!(items[0]["summary"].as_str().unwrap_or("").chars().count() <= 260);
        assert!(
            items[0]["content_excerpt"]
                .as_str()
                .unwrap_or("")
                .chars()
                .count()
                <= 700
        );
    }

    #[test]
    fn react_compact_fallback_evidence_items_default_missing_fields() {
        let items = assistant_run_compact_evidence_items_for_natural_fallback(
            &json!({
                "supplied_items": [{
                    "content_excerpt": "explicit excerpt"
                }]
            }),
            4,
        );

        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["type"], "evidence");
        assert_eq!(items[0]["summary"], "");
        assert_eq!(items[0]["content_excerpt"], "explicit excerpt");
        assert!(items[0]["source_locator"].is_null());
        assert!(items[0]["document_id"].is_null());
        assert!(items[0]["chunk_index"].is_null());
        assert!(items[0]["fallback_reason"].is_null());
        assert!(
            assistant_run_compact_evidence_items_for_natural_fallback(&json!({}), 4).is_empty()
        );
    }

    #[test]
    fn react_call_id_accepts_snake_camel_and_nested_tool_call() {
        assert_eq!(
            assistant_run_react_call_id_from_value(&json!({"tool_call_id": " call-1 "})),
            Some("call-1".to_string())
        );
        assert_eq!(
            assistant_run_react_call_id_from_value(&json!({"toolCallId": "call-2"})),
            Some("call-2".to_string())
        );
        assert_eq!(
            assistant_run_react_observation_call_id(&json!({"tool_call": {"callId": "call-3"}})),
            Some("call-3".to_string())
        );
        assert!(assistant_run_react_call_id_from_value(&json!({"call_id": "  "})).is_none());
    }

    #[test]
    fn react_observation_status_trims_and_rejects_empty() {
        assert_eq!(
            assistant_run_react_observation_status(&json!({"status": " completed "})),
            Some("completed")
        );
        assert!(assistant_run_react_observation_status(&json!({"status": " "})).is_none());
        assert!(assistant_run_react_observation_status(&json!({"status": 1})).is_none());
    }

    #[test]
    fn react_has_completed_action_requires_completed_matching_action() {
        let observations = vec![
            json!({"status": "failed", "action_type": "list_report_options"}),
            json!({"status": "completed", "actionType": "retrieve_evidence"}),
            json!({"status": "completed", "action_type": "list_report_options"}),
        ];

        assert!(assistant_run_react_has_completed_action(
            &observations,
            "list_report_options"
        ));
        assert!(assistant_run_react_has_completed_action(
            &observations,
            "retrieve_evidence"
        ));
        assert!(!assistant_run_react_has_completed_action(
            &observations,
            "read_document_detail"
        ));
    }

    #[test]
    fn react_replays_completed_tool_call_detects_completed_terminal_observation() {
        let decision = react_test_decision(
            AssistantRunReActActionType::RetrieveEvidence,
            json!({"tool_call_id": "call-1"}),
        );

        for status in ["completed", "failed", "rejected", "denied"] {
            assert_eq!(
                assistant_run_react_replays_completed_tool_call(
                    &decision,
                    &[json!({"status": status, "tool_call_id": "call-1"})],
                ),
                Some("call-1".to_string())
            );
        }

        assert_eq!(
            assistant_run_react_replays_completed_tool_call(
                &decision,
                &[json!({"status": "pending_tool_output", "tool_call_id": "call-1"})],
            ),
            None
        );
        assert_eq!(
            assistant_run_react_replays_completed_tool_call(
                &decision,
                &[json!({"status": "completed", "tool_call_id": "call-2"})],
            ),
            None
        );
    }

    #[test]
    fn react_pending_tool_output_detects_missing_and_repeated_pending_calls() {
        let pending = assistant_run_react_pending_tool_output(&[json!({
            "status": "tool_call_requested",
            "tool_call": {"toolCallId": "call-1"}
        })])
        .unwrap();
        assert_eq!(pending.call_id, "call-1");
        assert!(!pending.repeated);

        let repeated = assistant_run_react_pending_tool_output(&[
            json!({"status": "tool_calls_emitted", "tool_call_id": "call-2"}),
            json!({"status": "tool_output_missing", "toolCallId": "call-2"}),
        ])
        .unwrap();
        assert_eq!(repeated.call_id, "call-2");
        assert!(repeated.repeated);

        assert!(assistant_run_react_pending_tool_output(&[
            json!({"status": "pending_tool_output", "tool_call_id": "call-3"}),
            json!({"status": "completed", "tool_call_id": "call-3"}),
        ])
        .is_none());
        assert!(assistant_run_react_pending_tool_output(&[json!({
            "status": "pending_tool_output"
        })])
        .is_none());
    }

    #[test]
    fn react_repeats_no_progress_action_requires_two_recent_terminal_failures() {
        let decision = react_test_decision(
            AssistantRunReActActionType::RetrieveEvidence,
            json!({"query": "test"}),
        );

        assert!(assistant_run_react_repeats_no_progress_action(
            &decision,
            &[
                json!({"status": "rejected", "action_type": "retrieve_evidence"}),
                json!({"status": "failed", "actionType": "retrieve_evidence"}),
            ],
        ));
        assert!(!assistant_run_react_repeats_no_progress_action(
            &decision,
            &[json!({"status": "rejected", "action_type": "retrieve_evidence"})],
        ));
        assert!(!assistant_run_react_repeats_no_progress_action(
            &decision,
            &[
                json!({"status": "rejected", "action_type": "retrieve_evidence"}),
                json!({"status": "failed", "actionType": "read_document_detail"}),
            ],
        ));
        assert!(!assistant_run_react_repeats_no_progress_action(
            &decision,
            &[
                json!({"status": "rejected", "action_type": "retrieve_evidence"}),
                json!({"status": "denied", "actionType": "retrieve_evidence"}),
                json!({"status": "completed", "action_type": "retrieve_evidence"}),
            ],
        ));
    }

    #[test]
    fn react_protocol_repair_rejects_premature_final_answer_and_scope_escape() {
        let selected_dataset_id = DatasetId::new();
        let denied_dataset_id = DatasetId::new();
        let selected_scope = json!({
            "mode": "selected",
            "selected": [{"type": "dataset", "id": selected_dataset_id.to_string()}],
            "intent": "data_question",
        });
        let empty_evidence = json!({"status": "empty", "supplied_items": []});

        let final_action = react_test_decision(
            AssistantRunReActActionType::FinalAnswer,
            json!({"content": "未供料回答"}),
        );
        let final_repair =
            build_react_protocol_repair(&final_action, &[], &selected_scope, &empty_evidence)
                .expect("scoped final answer before supply should be repaired");
        assert_eq!(final_repair.observation["actionType"], "policy_observation");
        assert!(final_repair.final_answer.is_none());

        let denied_action = react_test_decision(
            AssistantRunReActActionType::RetrieveEvidence,
            json!({"dataset_id": denied_dataset_id.to_string()}),
        );
        let denied_repair =
            build_react_protocol_repair(&denied_action, &[], &selected_scope, &empty_evidence)
                .expect("outside selected dataset scope should be repaired");
        assert_eq!(denied_repair.observation["repair_code"], "scope_denied");
        assert_eq!(
            denied_repair.observation["denied"][0],
            format!("dataset:{denied_dataset_id}")
        );
    }

    #[test]
    fn react_terminal_action_requires_supply_for_selected_data_scope() {
        let selected_scope = json!({
            "mode": "selected",
            "selected": [{"type": "dataset", "id": "00000000-0000-0000-0000-000000000001"}],
            "intent": "data_question",
        });
        let ordinary_scope = json!({"mode": "ordinary_chat"});
        let ordinary_selected_scope = json!({
            "mode": "user_selected",
            "selected": [{"type": "dataset", "id": "00000000-0000-0000-0000-000000000001"}],
            "intent": "ordinary_chat",
        });
        let final_action = react_test_decision(
            AssistantRunReActActionType::FinalAnswer,
            json!({"content": "未供料回答"}),
        );
        let retrieve_action =
            react_test_decision(AssistantRunReActActionType::RetrieveEvidence, json!({}));

        assert!(assistant_run_react_should_repair_terminal_action(
            &final_action,
            &selected_scope,
            &json!({"status": "empty", "supplied_items": []}),
            &[],
        ));
        assert!(!assistant_run_react_should_repair_terminal_action(
            &final_action,
            &ordinary_scope,
            &json!({"status": "not_requested", "supplied_items": []}),
            &[],
        ));
        assert!(!assistant_run_react_should_repair_terminal_action(
            &final_action,
            &ordinary_selected_scope,
            &json!({"status": "empty", "supplied_items": []}),
            &[],
        ));
        assert!(!assistant_run_react_should_repair_terminal_action(
            &retrieve_action,
            &selected_scope,
            &json!({"status": "empty", "supplied_items": []}),
            &[],
        ));
        assert!(!assistant_run_react_should_repair_terminal_action(
            &final_action,
            &selected_scope,
            &json!({"status": "empty", "supplied_items": []}),
            &[json!({
                "status": "completed",
                "action_type": "retrieve_evidence",
                "supplied_count": 0,
            })],
        ));
        assert!(!assistant_run_react_should_repair_terminal_action(
            &final_action,
            &selected_scope,
            &json!({
                "status": "supplied",
                "supplied_items": [{"type": "retrieval_evidence"}],
            }),
            &[],
        ));
    }

    #[test]
    fn react_supply_observation_accepts_whitelisted_completed_actions_only() {
        assert!(assistant_run_react_has_supply_observation(
            &json!({"status": "empty", "supplied_items": []}),
            &[json!({"status": "completed", "actionType": "read_document_detail"})],
        ));
        assert!(assistant_run_react_has_supply_observation(
            &json!({"status": "empty", "supplied_items": []}),
            &[json!({"status": "completed", "action_type": "upgrade_parse_vlm"})],
        ));
        assert!(assistant_run_react_has_supply_observation(
            &json!({"status": "empty", "supplied_items": []}),
            &[json!({"status": "completed", "action_type": "recall_conversation_memory"})],
        ));
        assert!(!assistant_run_react_has_supply_observation(
            &json!({"status": "empty", "supplied_items": []}),
            &[json!({"status": "failed", "action_type": "retrieve_evidence"})],
        ));
        assert!(!assistant_run_react_has_supply_observation(
            &json!({"status": "empty", "supplied_items": []}),
            &[json!({"status": "completed", "action_type": "web_search"})],
        ));
    }

    #[test]
    fn react_plain_ordinary_chat_scope_requires_no_supply_evidence_or_artifact() {
        assert!(assistant_run_is_plain_ordinary_chat_scope(None, None, None));
        assert!(assistant_run_is_plain_ordinary_chat_scope(
            Some(&json!({
                "mode": "ordinary_chat",
                "intent": "ordinary_chat",
                "datasets": [],
                "conversation_memory": [],
            })),
            Some(&json!({"status": "empty", "supplied_items": []})),
            None,
        ));
        assert!(!assistant_run_is_plain_ordinary_chat_scope(
            Some(&json!({
                "mode": "ordinary_chat",
                "intent": "ordinary_chat",
                "datasets": ["00000000-0000-0000-0000-000000000001"],
            })),
            Some(&json!({"status": "empty", "supplied_items": []})),
            None,
        ));
        assert!(!assistant_run_is_plain_ordinary_chat_scope(
            Some(&json!({
                "mode": "ordinary_chat",
                "intent": "ordinary_chat",
                "datasets": [],
            })),
            Some(&json!({"status": "supplied", "supplied_items": [{"type": "evidence"}]})),
            None,
        ));
        assert!(!assistant_run_is_plain_ordinary_chat_scope(
            Some(&json!({
                "mode": "ordinary_chat",
                "intent": "ordinary_chat",
                "datasets": [],
            })),
            Some(&json!({"status": "empty", "supplied_items": []})),
            Some(&json!({"id": "artifact-1"})),
        ));
    }

    #[test]
    fn react_scope_allows_tools_for_data_scope_or_artifact_context() {
        assert!(!assistant_run_react_scope_allows_tools(
            &json!({
                "mode": "ordinary_chat",
                "intent": "ordinary_chat",
                "datasets": [],
                "conversation_memory": [],
            }),
            None,
        ));
        assert!(!assistant_run_react_scope_allows_tools(
            &json!({
                "mode": "user_selected",
                "intent": "ordinary_chat",
                "datasets": ["00000000-0000-0000-0000-000000000001"],
            }),
            None,
        ));
        assert!(assistant_run_react_scope_allows_tools(
            &json!({
                "mode": "user_selected",
                "intent": "data_question",
                "datasets": ["00000000-0000-0000-0000-000000000001"],
            }),
            None,
        ));
        assert!(assistant_run_react_scope_allows_tools(
            &json!({
                "mode": "ordinary_chat",
                "intent": "ordinary_chat",
                "datasets": [],
            }),
            Some(&json!({"id": "artifact-1"})),
        ));
    }
}
