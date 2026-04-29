use chrono::Utc;
use domain_model::SecretBindingId;
use serde_json::{json, Value};

use crate::{
    build_assistant_run_evidence_state, ensure_react_requested_dataset_is_selected,
    ensure_scope_requests_conversation_memory, react_static_page_operations_from_arguments,
    ApiError, AppState,
};

use crate::react_agent_contract::{
    AssistantRunReActActionType as AssistantRunReactActionType,
    AssistantRunReActDecision as AssistantRunNextAction,
};

#[derive(Clone, Debug)]
pub(crate) struct AssistantRunReactToolResult {
    pub(crate) observation: Value,
    pub(crate) trail_step: Value,
    pub(crate) final_answer: Option<String>,
}

pub(crate) async fn execute_assistant_run_react_action(
    state: &AppState,
    action: &AssistantRunNextAction,
    selected_scope: &Value,
    evidence_state: &mut Value,
    prompt: &str,
    local_thread_id: Option<&str>,
    active_secret_binding_ids: &[SecretBindingId],
) -> std::result::Result<AssistantRunReactToolResult, ApiError> {
    match action.action_type {
        AssistantRunReactActionType::FinalAnswer => Ok(final_answer_result(action)),
        AssistantRunReactActionType::RetrieveEvidence => {
            ensure_react_requested_dataset_is_selected(&action.arguments, selected_scope)?;
            let query = action
                .arguments
                .get("query")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(prompt);
            let refreshed = build_assistant_run_evidence_state(
                state,
                selected_scope,
                query,
                local_thread_id,
                active_secret_binding_ids,
            )
            .await?;
            let supplied_count = crate::assistant_run_evidence_supplied_count(&refreshed);
            *evidence_state = refreshed.clone();
            Ok(AssistantRunReactToolResult {
                observation: json!({
                    "status": "completed",
                    "action_type": action.action_type.as_str(),
                    "actionType": action.action_type.as_str(),
                    "message": "retrieval evidence supplied",
                    "items": [],
                    "limits": {},
                    "supplied_count": supplied_count,
                    "evidence_status": refreshed.get("status").and_then(Value::as_str).unwrap_or("unknown"),
                }),
                trail_step: json!({
                    "status": "completed",
                    "label": "检索供料证据",
                    "react_action": action.action_type.as_str(),
                    "supplied_count": supplied_count,
                    "at": Utc::now(),
                }),
                final_answer: None,
            })
        }
        AssistantRunReactActionType::RecallConversationMemory => {
            let memory_scope = ensure_scope_requests_conversation_memory(selected_scope.clone());
            let refreshed = build_assistant_run_evidence_state(
                state,
                &memory_scope,
                prompt,
                local_thread_id,
                active_secret_binding_ids,
            )
            .await?;
            let memory_count = refreshed
                .get("conversation_memory_items")
                .and_then(Value::as_array)
                .map(Vec::len)
                .unwrap_or(0);
            *evidence_state = refreshed.clone();
            Ok(AssistantRunReactToolResult {
                observation: json!({
                    "status": "completed",
                    "action_type": action.action_type.as_str(),
                    "actionType": action.action_type.as_str(),
                    "message": "conversation memory recalled",
                    "items": [],
                    "limits": {},
                    "memory_count": memory_count,
                }),
                trail_step: json!({
                    "status": "completed",
                    "label": "召回对话记忆",
                    "react_action": action.action_type.as_str(),
                    "memory_count": memory_count,
                    "at": Utc::now(),
                }),
                final_answer: None,
            })
        }
        AssistantRunReactActionType::UpdateStaticPageModule => {
            let operations = react_static_page_operations_from_arguments(&action.arguments)?;
            Ok(AssistantRunReactToolResult {
                observation: json!({
                    "status": "completed",
                    "action_type": action.action_type.as_str(),
                    "actionType": action.action_type.as_str(),
                    "message": "static page module operation sanitized",
                    "items": [],
                    "limits": {},
                    "operation_count": operations.len(),
                    "operations": operations,
                }),
                trail_step: json!({
                    "status": "completed",
                    "label": "更新静态页模块",
                    "react_action": action.action_type.as_str(),
                    "operation_count": operations.len(),
                    "at": Utc::now(),
                }),
                final_answer: None,
            })
        }
        _ => Ok(rejected_react_tool_result(
            action,
            "action_not_implemented_in_first_slice",
        )),
    }
}

pub(crate) fn assistant_run_react_action_label(
    action_type: &AssistantRunReactActionType,
) -> &'static str {
    match action_type {
        AssistantRunReactActionType::RetrieveEvidence => "检索供料证据",
        AssistantRunReactActionType::ReadDocumentDetail => "读取文档详情",
        AssistantRunReactActionType::RecallConversationMemory => "召回对话记忆",
        AssistantRunReactActionType::ListReportOptions => "列出报表选项",
        AssistantRunReactActionType::CreateStaticPageDraft => "创建静态页草稿",
        AssistantRunReactActionType::UpdateStaticPageModule => "更新静态页模块",
        AssistantRunReactActionType::SubmitStaticPageImagePreview => "提交效果图生成",
        AssistantRunReactActionType::RenderStaticPage => "制作最终静态页",
        AssistantRunReactActionType::CreateReportDraft => "创建报表草稿",
        AssistantRunReactActionType::ReportChoice => "选择报表流向",
        AssistantRunReactActionType::OpenClawMemoryRecall => "调用 OpenClaw 记忆",
        AssistantRunReactActionType::OpenClawReadonlyExecution => "调用 OpenClaw 只读执行",
        AssistantRunReactActionType::FinalAnswer => "模型生成最终回答",
    }
}

pub(crate) fn assistant_run_react_policy_observation(
    action: &AssistantRunNextAction,
    message: &str,
    step_index: usize,
) -> AssistantRunReactToolResult {
    AssistantRunReactToolResult {
        observation: json!({
            "status": "rejected",
            "action_type": "policy_observation",
            "actionType": "policy_observation",
            "message": message,
            "denied": [format!("terminal:{}", action.action_type.as_str())],
            "items": [],
            "limits": {},
            "repair_required": true,
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

fn final_answer_result(action: &AssistantRunNextAction) -> AssistantRunReactToolResult {
    let content = action
        .arguments
        .get("content")
        .or_else(|| action.arguments.get("answer"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(action.reason_summary.as_str())
        .to_string();
    AssistantRunReactToolResult {
        observation: json!({
            "status": "completed",
            "action_type": action.action_type.as_str(),
            "actionType": action.action_type.as_str(),
            "message": "final answer produced",
            "items": [],
            "limits": {},
            "content_length": content.chars().count(),
        }),
        trail_step: json!({
            "status": "completed",
            "label": "模型生成最终回答",
            "react_action": action.action_type.as_str(),
            "reason_summary": action.reason_summary.clone(),
            "at": Utc::now(),
        }),
        final_answer: Some(content),
    }
}

fn rejected_react_tool_result(
    action: &AssistantRunNextAction,
    reason: &str,
) -> AssistantRunReactToolResult {
    AssistantRunReactToolResult {
        observation: json!({
            "status": "rejected",
            "action_type": action.action_type.as_str(),
            "actionType": action.action_type.as_str(),
            "message": reason,
            "denied": [action.action_type.as_str()],
            "items": [],
            "limits": {},
            "reason": reason,
        }),
        trail_step: json!({
            "status": "rejected",
            "label": assistant_run_react_action_label(&action.action_type),
            "react_action": action.action_type.as_str(),
            "reason": "首版暂未启用该动作",
            "at": Utc::now(),
        }),
        final_answer: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::react_agent_contract::AssistantRunReActStatus;

    fn test_action(action_type: AssistantRunReactActionType) -> AssistantRunNextAction {
        AssistantRunNextAction {
            status: AssistantRunReActStatus::Act,
            intent: Some("question".to_string()),
            action_type,
            reason_summary: "测试动作".to_string(),
            arguments: json!({}),
            requires_confirmation: false,
            answer: None,
            citations: Vec::new(),
            conversation_state: json!({}),
        }
    }

    #[test]
    fn rejected_tool_result_is_structured_and_non_panicking() {
        let action = test_action(AssistantRunReactActionType::ListReportOptions);
        let result = rejected_react_tool_result(&action, "not_registered");

        assert_eq!(result.observation["status"], json!("rejected"));
        assert_eq!(
            result.observation["actionType"],
            json!("list_report_options")
        );
        assert_eq!(result.observation["message"], json!("not_registered"));
        assert_eq!(
            result.observation["denied"][0],
            json!("list_report_options")
        );
        assert_eq!(result.observation["items"], json!([]));
        assert_eq!(result.observation["limits"], json!({}));
        assert!(result.final_answer.is_none());
    }

    #[test]
    fn policy_observation_returns_denied_identifier_only() {
        let action = test_action(AssistantRunReactActionType::FinalAnswer);
        let result = assistant_run_react_policy_observation(&action, "need evidence first", 2);

        assert_eq!(result.observation["status"], json!("rejected"));
        assert_eq!(
            result.observation["actionType"],
            json!("policy_observation")
        );
        assert_eq!(
            result.observation["denied"][0],
            json!("terminal:final_answer")
        );
        assert_eq!(result.observation["items"], json!([]));
        assert_eq!(result.observation["limits"], json!({}));
        assert!(result.final_answer.is_none());
    }
}
