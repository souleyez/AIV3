use axum::{
    extract::{Path, State},
    Json,
};
use chrono::Utc;
use contracts::{CreateStaticPageImageJobRequest, CreateStaticPageRenderRequest};
use domain_model::{
    AssistantRunId, Document, DocumentChunk, DocumentId, SecretBindingId, StaticPageDraftId,
    StaticPageImageJobId, WorkflowEventId, WorkflowEventRecord, WorkflowExecution,
    WorkflowExecutionId, WorkflowKind,
};
use serde_json::{json, Value};
use std::{collections::BTreeMap, env};
use uuid::Uuid;

use crate::{
    append_static_page_draft_run_event, append_static_page_operations_metadata,
    apply_static_page_operations_to_payload, build_assistant_run_evidence_state,
    ensure_react_requested_dataset_is_selected, ensure_scope_requests_conversation_memory,
    load_visible_document, react_static_page_operations_from_arguments,
    status_from_static_page_operations, status_from_static_page_payload,
    summarize_static_page_operations, ApiError, AppState,
};
use workflow_engine::WorkflowSignal;

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
    current_artifact: Option<&Value>,
    active_assistant_run_id: Option<AssistantRunId>,
    prompt: &str,
    local_thread_id: Option<&str>,
    active_secret_binding_ids: &[SecretBindingId],
) -> std::result::Result<AssistantRunReactToolResult, ApiError> {
    match action.action_type {
        AssistantRunReactActionType::FinalAnswer => Ok(final_answer_result(action)),
        AssistantRunReactActionType::ReadDocumentDetail => {
            read_document_detail_result(state, action, selected_scope, active_secret_binding_ids)
                .await
        }
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
        AssistantRunReactActionType::ListReportOptions => {
            ensure_react_requested_dataset_is_selected(&action.arguments, selected_scope)?;
            Ok(list_report_options_result(action, selected_scope))
        }
        AssistantRunReactActionType::ReportChoice => Ok(report_choice_result(action)),
        AssistantRunReactActionType::OpenClawMemoryRecall => {
            Ok(openclaw_memory_recall_result(action, local_thread_id))
        }
        AssistantRunReactActionType::OpenClawReadonlyExecution => {
            Ok(openclaw_readonly_execution_result(action))
        }
        AssistantRunReactActionType::CodexHostTask => {
            codex_host_task_result(state, action, active_assistant_run_id, local_thread_id).await
        }
        AssistantRunReactActionType::UpdateStaticPageModule => {
            let operations = react_static_page_operations_from_arguments(&action.arguments)?;
            if let (Some(draft_id), Some(active_assistant_run_id)) = (
                current_static_page_draft_id(current_artifact),
                active_assistant_run_id,
            ) {
                apply_static_page_module_operations_to_current_draft(
                    state,
                    action,
                    draft_id,
                    active_assistant_run_id,
                    operations,
                    prompt,
                )
                .await
            } else {
                Ok(static_page_module_operations_sanitized_result(
                    action, operations, true,
                ))
            }
        }
        AssistantRunReactActionType::SubmitStaticPageImagePreview => {
            submit_static_page_image_preview_for_current_draft(
                state,
                action,
                current_artifact,
                active_assistant_run_id,
                prompt,
            )
            .await
        }
        AssistantRunReactActionType::RenderStaticPage => {
            render_static_page_for_current_draft(
                state,
                action,
                current_artifact,
                active_assistant_run_id,
            )
            .await
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
        AssistantRunReactActionType::CodexHostTask => "调用 Codex Host 任务",
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

fn static_page_module_operations_sanitized_result(
    action: &AssistantRunNextAction,
    operations: Vec<Value>,
    requires_host_application: bool,
) -> AssistantRunReactToolResult {
    AssistantRunReactToolResult {
        observation: json!({
            "status": "completed",
            "action_type": action.action_type.as_str(),
            "actionType": action.action_type.as_str(),
            "message": if requires_host_application {
                "static page module operation sanitized; no current persisted draft was supplied"
            } else {
                "static page module operation sanitized"
            },
            "items": [],
            "limits": {},
            "operation_count": operations.len(),
            "operations": operations,
            "requires_host_application": requires_host_application,
        }),
        trail_step: json!({
            "status": "completed",
            "label": "更新静态页模块",
            "react_action": action.action_type.as_str(),
            "operation_count": operations.len(),
            "requires_host_application": requires_host_application,
            "at": Utc::now(),
        }),
        final_answer: None,
    }
}

async fn apply_static_page_module_operations_to_current_draft(
    state: &AppState,
    action: &AssistantRunNextAction,
    draft_id: StaticPageDraftId,
    active_assistant_run_id: AssistantRunId,
    operations: Vec<Value>,
    prompt: &str,
) -> std::result::Result<AssistantRunReactToolResult, ApiError> {
    let mut draft = state
        .storage
        .static_page_drafts()
        .get_by_id(state.tenant_id, draft_id)
        .await
        .map_err(ApiError::from_storage)?
        .ok_or_else(|| {
            ApiError::not_found(
                "static_page_draft_not_found",
                format!("static page draft {} was not found", draft_id),
            )
        })?;
    if draft.assistant_run_id != active_assistant_run_id {
        return Ok(rejected_react_tool_result(
            action,
            "current_static_page_draft_run_mismatch",
        ));
    }

    let summary = summarize_static_page_operations(&operations);
    let mut draft_payload = apply_static_page_operations_to_payload(
        draft.draft_payload.clone(),
        &operations,
        Some(&summary),
    );
    append_static_page_operations_metadata(&mut draft_payload, &operations, Some(prompt), &summary);
    draft.status = status_from_static_page_payload(&draft_payload)
        .or_else(|| status_from_static_page_operations(&operations))
        .unwrap_or(draft.status);
    draft.draft_payload = draft_payload;

    let updated = state
        .storage
        .static_page_drafts()
        .update(state.tenant_id, &draft)
        .await
        .map_err(ApiError::from_storage)?;
    append_static_page_draft_run_event(
        state,
        &updated,
        "static_page_draft.react_operations_applied",
        json!({
            "draft_id": updated.id,
            "operation_count": operations.len(),
            "summary": summary,
            "react_action": action.action_type.as_str(),
        }),
    )
    .await?;

    Ok(AssistantRunReactToolResult {
        observation: json!({
            "status": "completed",
            "action_type": action.action_type.as_str(),
            "actionType": action.action_type.as_str(),
            "message": "static page draft updated",
            "items": [{
                "type": "static_page_draft",
                "draft_id": updated.id.to_string(),
                "status": updated.status.as_str(),
            }],
            "limits": {},
            "operation_count": operations.len(),
            "operations": operations,
            "draft_id": updated.id.to_string(),
            "draft_status": updated.status.as_str(),
        }),
        trail_step: json!({
            "status": "completed",
            "label": "更新静态页模块",
            "react_action": action.action_type.as_str(),
            "operation_count": operations.len(),
            "draft_id": updated.id.to_string(),
            "draft_status": updated.status.as_str(),
            "at": Utc::now(),
        }),
        final_answer: None,
    })
}

async fn submit_static_page_image_preview_for_current_draft(
    state: &AppState,
    action: &AssistantRunNextAction,
    current_artifact: Option<&Value>,
    active_assistant_run_id: Option<AssistantRunId>,
    prompt: &str,
) -> std::result::Result<AssistantRunReactToolResult, ApiError> {
    let Some(draft_id) = current_static_page_draft_id(current_artifact) else {
        return Ok(rejected_react_tool_result(
            action,
            "current_static_page_draft_required",
        ));
    };
    let Some(active_assistant_run_id) = active_assistant_run_id else {
        return Ok(rejected_react_tool_result(
            action,
            "active_assistant_run_required",
        ));
    };
    if let Some(rejected) =
        reject_if_static_page_draft_not_current(state, action, draft_id, active_assistant_run_id)
            .await?
    {
        return Ok(rejected);
    }

    let image_prompt_payload = action
        .arguments
        .get("image_prompt_payload")
        .or_else(|| action.arguments.get("imagePromptPayload"))
        .cloned()
        .unwrap_or(Value::Null);
    let image_prompt = action
        .arguments
        .get("prompt")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(prompt)
        .to_string();
    let (_status, Json(response)) = crate::create_static_page_image_job(
        State(state.clone()),
        Path(draft_id.to_string()),
        Json(CreateStaticPageImageJobRequest {
            prompt: Some(image_prompt),
            image_prompt_payload,
        }),
    )
    .await?;
    let image_job_status =
        serde_json::to_value(&response.image_job.status).unwrap_or_else(|_| json!("unknown"));

    Ok(AssistantRunReactToolResult {
        observation: json!({
            "status": "completed",
            "action_type": action.action_type.as_str(),
            "actionType": action.action_type.as_str(),
            "message": "static page image preview queued",
            "items": [{
                "type": "static_page_image_job",
                "image_job_id": response.image_job.id.to_string(),
                "draft_id": response.image_job.draft_id.to_string(),
                "status": image_job_status,
                "queue_position": response.image_job.queue_position,
            }],
            "limits": {},
            "draft_id": response.image_job.draft_id.to_string(),
            "image_job_id": response.image_job.id.to_string(),
            "queue_position": response.image_job.queue_position,
        }),
        trail_step: json!({
            "status": "completed",
            "label": "提交效果图生成",
            "react_action": action.action_type.as_str(),
            "draft_id": response.image_job.draft_id.to_string(),
            "image_job_id": response.image_job.id.to_string(),
            "queue_position": response.image_job.queue_position,
            "at": Utc::now(),
        }),
        final_answer: None,
    })
}

async fn render_static_page_for_current_draft(
    state: &AppState,
    action: &AssistantRunNextAction,
    current_artifact: Option<&Value>,
    active_assistant_run_id: Option<AssistantRunId>,
) -> std::result::Result<AssistantRunReactToolResult, ApiError> {
    let Some(draft_id) = current_static_page_draft_id(current_artifact) else {
        return Ok(rejected_react_tool_result(
            action,
            "current_static_page_draft_required",
        ));
    };
    let Some(active_assistant_run_id) = active_assistant_run_id else {
        return Ok(rejected_react_tool_result(
            action,
            "active_assistant_run_required",
        ));
    };
    if let Some(rejected) =
        reject_if_static_page_draft_not_current(state, action, draft_id, active_assistant_run_id)
            .await?
    {
        return Ok(rejected);
    }

    let image_job_id = match static_page_image_job_id_from_arguments(&action.arguments) {
        Ok(value) => value,
        Err(message) => return Ok(rejected_react_tool_result(action, message)),
    };
    let response = crate::create_static_page_render(
        State(state.clone()),
        Path(draft_id.to_string()),
        Json(CreateStaticPageRenderRequest {
            image_job_id,
            background: false,
        }),
    )
    .await;
    let (_status, Json(response)) = match response {
        Ok(response) => response,
        Err(error) if error.payload.code == "static_page_preview_not_confirmed" => {
            return Ok(rejected_react_tool_result(
                action,
                "static_page_preview_not_confirmed",
            ));
        }
        Err(error) if error.payload.code == "static_page_image_job_mismatch" => {
            return Ok(rejected_react_tool_result(
                action,
                "static_page_image_job_mismatch",
            ));
        }
        Err(error) => return Err(error),
    };
    let render_status =
        serde_json::to_value(&response.render_output.status).unwrap_or_else(|_| json!("unknown"));

    Ok(AssistantRunReactToolResult {
        observation: json!({
            "status": "completed",
            "action_type": action.action_type.as_str(),
            "actionType": action.action_type.as_str(),
            "message": "static page rendered",
            "items": [{
                "type": "static_page_render_output",
                "render_output_id": response.render_output.id.to_string(),
                "draft_id": response.render_output.draft_id.to_string(),
                "status": render_status,
                "image_job_id": response.render_output.image_job_id.map(|id| id.to_string()),
            }],
            "limits": {},
            "draft_id": response.render_output.draft_id.to_string(),
            "render_output_id": response.render_output.id.to_string(),
            "draft_status": response.draft.status,
        }),
        trail_step: json!({
            "status": "completed",
            "label": "制作最终静态页",
            "react_action": action.action_type.as_str(),
            "draft_id": response.render_output.draft_id.to_string(),
            "render_output_id": response.render_output.id.to_string(),
            "at": Utc::now(),
        }),
        final_answer: None,
    })
}

async fn reject_if_static_page_draft_not_current(
    state: &AppState,
    action: &AssistantRunNextAction,
    draft_id: StaticPageDraftId,
    active_assistant_run_id: AssistantRunId,
) -> std::result::Result<Option<AssistantRunReactToolResult>, ApiError> {
    let Some(draft) = state
        .storage
        .static_page_drafts()
        .get_by_id(state.tenant_id, draft_id)
        .await
        .map_err(ApiError::from_storage)?
    else {
        return Ok(Some(rejected_react_tool_result(
            action,
            "static_page_draft_not_found",
        )));
    };
    if draft.assistant_run_id != active_assistant_run_id {
        return Ok(Some(rejected_react_tool_result(
            action,
            "current_static_page_draft_run_mismatch",
        )));
    }
    Ok(None)
}

fn static_page_image_job_id_from_arguments(
    arguments: &Value,
) -> std::result::Result<Option<StaticPageImageJobId>, &'static str> {
    let Some(raw) = [
        "image_job_id",
        "imageJobId",
        "static_page_image_job_id",
        "job_id",
        "jobId",
    ]
    .iter()
    .find_map(|key| {
        arguments
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }) else {
        return Ok(None);
    };
    Uuid::parse_str(raw)
        .map(StaticPageImageJobId)
        .map(Some)
        .map_err(|_| "invalid_static_page_image_job_id")
}

fn current_static_page_draft_id(current_artifact: Option<&Value>) -> Option<StaticPageDraftId> {
    let artifact = current_artifact?;
    [
        "backendDraftId",
        "backend_draft_id",
        "staticPageDraftId",
        "static_page_draft_id",
        "draft_id",
        "id",
    ]
    .iter()
    .find_map(|key| {
        artifact
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .and_then(|value| Uuid::parse_str(value).ok())
            .map(StaticPageDraftId)
    })
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

fn list_report_options_result(
    action: &AssistantRunNextAction,
    selected_scope: &Value,
) -> AssistantRunReactToolResult {
    let choices = report_choice_items();
    let choice_count = choices.len();
    AssistantRunReactToolResult {
        observation: json!({
            "status": "completed",
            "action_type": action.action_type.as_str(),
            "actionType": action.action_type.as_str(),
            "message": "report choices listed",
            "items": choices,
            "limits": {},
            "selectedScope": selected_scope,
        }),
        trail_step: json!({
            "status": "completed",
            "label": "列出报表选项",
            "react_action": action.action_type.as_str(),
            "choice_count": choice_count,
            "at": Utc::now(),
        }),
        final_answer: None,
    }
}

fn report_choice_result(action: &AssistantRunNextAction) -> AssistantRunReactToolResult {
    let choice = action
        .arguments
        .get("choice")
        .or_else(|| action.arguments.get("key"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("continue_qa");
    let (choice, message) = match choice {
        "create_report" => ("create_report", "已选择生成报表，下一步进入报表流程。"),
        _ => ("continue_qa", "已选择继续问答。"),
    };

    AssistantRunReactToolResult {
        observation: json!({
            "status": "completed",
            "action_type": action.action_type.as_str(),
            "actionType": action.action_type.as_str(),
            "message": "report choice recorded",
            "items": [{
                "key": choice,
                "label": if choice == "create_report" { "生成报表" } else { "继续问答" },
                "type": "report_choice"
            }],
            "limits": {},
            "choice": choice,
        }),
        trail_step: json!({
            "status": "completed",
            "label": "选择报表流向",
            "react_action": action.action_type.as_str(),
            "choice": choice,
            "at": Utc::now(),
        }),
        final_answer: Some(message.to_string()),
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
            "reason": reason,
            "at": Utc::now(),
        }),
        final_answer: None,
    }
}

fn openclaw_memory_recall_result(
    action: &AssistantRunNextAction,
    local_thread_id: Option<&str>,
) -> AssistantRunReactToolResult {
    if !react_env_flag("OPENCLAW_EXTENSION_ENABLED", false) {
        return rejected_react_tool_result(action, "openclaw_extension_disabled");
    }
    if !react_env_flag("OPENCLAW_MEMORY_ENABLED", false) {
        return rejected_react_tool_result(action, "openclaw_memory_disabled");
    }

    let query = action
        .arguments
        .get("query")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("current conversation");
    ok_openclaw_stub_result(
        action,
        "OpenClaw memory bridge enabled",
        json!({
            "type": "openclaw_memory",
            "status": "bridge_stub",
            "query": truncate_for_openclaw_stub(query, 160),
            "local_thread_id": local_thread_id.unwrap_or_default(),
        }),
    )
}

fn openclaw_readonly_execution_result(
    action: &AssistantRunNextAction,
) -> AssistantRunReactToolResult {
    if !react_env_flag("OPENCLAW_EXTENSION_ENABLED", false) {
        return rejected_react_tool_result(action, "openclaw_extension_disabled");
    }
    if !react_env_flag("OPENCLAW_READONLY_EXECUTION_ENABLED", false) {
        return rejected_react_tool_result(action, "openclaw_readonly_execution_disabled");
    }
    let capability = action
        .arguments
        .get("capability")
        .or_else(|| action.arguments.get("tool"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let Some(capability) = capability else {
        return rejected_react_tool_result(
            action,
            "openclaw_readonly_execution_capability_required",
        );
    };
    if !openclaw_readonly_capability_allowed(capability) {
        return rejected_react_tool_result(action, "openclaw_readonly_execution_not_allowlisted");
    }

    ok_openclaw_stub_result(
        action,
        "OpenClaw readonly execution bridge enabled",
        json!({
            "type": "openclaw_readonly_execution",
            "status": "bridge_stub",
            "capability": truncate_for_openclaw_stub(capability, 96),
        }),
    )
}

async fn codex_host_task_result(
    state: &AppState,
    action: &AssistantRunNextAction,
    active_assistant_run_id: Option<AssistantRunId>,
    local_thread_id: Option<&str>,
) -> std::result::Result<AssistantRunReactToolResult, ApiError> {
    if let Some(rejection) = codex_host_task_preflight_rejection(action, active_assistant_run_id) {
        return Ok(rejection);
    }
    let active_assistant_run_id =
        active_assistant_run_id.expect("preflight requires active assistant run");
    let capability = codex_host_task_capability(action).expect("preflight requires capability");

    let execution = build_initial_codex_host_task_execution(
        state,
        action,
        active_assistant_run_id,
        capability,
        local_thread_id,
    )?;
    let initial_event = build_initial_codex_host_task_event(&execution, active_assistant_run_id);
    state
        .storage
        .workflow_executions()
        .create_with_initial_event(&execution, &initial_event)
        .await
        .map_err(ApiError::from_storage)?;
    let start_response =
        crate::apply_workflow_signal(state, execution.id, WorkflowSignal::Start).await?;
    let tasks = start_response.enqueued_tasks;
    let task_items = tasks
        .iter()
        .map(|task| {
            json!({
                "type": "workflow_task",
                "workflow_task_id": task.id.to_string(),
                "queue": task.queue,
                "task_key": task.task_key,
                "status": task.status.as_str(),
                "available_at": task.available_at,
            })
        })
        .collect::<Vec<_>>();

    Ok(AssistantRunReactToolResult {
        observation: json!({
            "status": "completed",
            "action_type": action.action_type.as_str(),
            "actionType": action.action_type.as_str(),
            "message": "Codex Host task queued",
            "items": [{
                "type": "codex_host_task",
                "status": "queued",
                "capability": truncate_for_openclaw_stub(capability, 96),
                "workflow_execution_id": execution.id.to_string(),
                "assistant_run_id": active_assistant_run_id.to_string(),
            }],
            "tasks": task_items,
            "limits": {
                "mode": "queued",
                "externalExecution": true,
                "taskMemoryIsolated": true,
            },
            "workflow_execution_id": execution.id.to_string(),
        }),
        trail_step: json!({
            "status": "completed",
            "label": assistant_run_react_action_label(&action.action_type),
            "react_action": action.action_type.as_str(),
            "returned_count": 1,
            "workflow_execution_id": execution.id.to_string(),
            "external_execution": true,
            "at": Utc::now(),
        }),
        final_answer: None,
    })
}

fn build_initial_codex_host_task_execution(
    state: &AppState,
    action: &AssistantRunNextAction,
    assistant_run_id: AssistantRunId,
    capability: &str,
    local_thread_id: Option<&str>,
) -> std::result::Result<WorkflowExecution, ApiError> {
    let definition = state
        .workflow_catalog
        .find_definition(WorkflowKind::CodexHostTask)
        .ok_or_else(|| {
            ApiError::internal(
                "workflow_definition_missing",
                "codex_host_task workflow definition is not registered".to_string(),
            )
        })?;
    let now = Utc::now();
    let execution_id = WorkflowExecutionId::new();
    let runtime_state = definition.initial_state(execution_id, now);
    let mut context = runtime_state.context;
    context.insert(
        "retries_remaining".to_string(),
        Value::Number(runtime_state.retries_remaining.into()),
    );
    context.insert(
        "assistant_run_id".to_string(),
        Value::String(assistant_run_id.to_string()),
    );
    context.insert(
        "capability".to_string(),
        Value::String(truncate_for_openclaw_stub(capability, 96)),
    );
    if let Some(task) = codex_host_task_text_from_arguments(&action.arguments) {
        context.insert("task".to_string(), Value::String(task));
    }
    if let Some(local_thread_id) = local_thread_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        context.insert(
            "local_thread_id".to_string(),
            Value::String(local_thread_id.to_string()),
        );
    }
    context.insert(
        "task_memory_policy".to_string(),
        json!({
            "kind": "task",
            "isolated": true,
            "promote_summary_to_conversation": false,
        }),
    );
    context.insert(
        "safety".to_string(),
        json!({
            "user_cli_flags_allowed": false,
            "secrets_in_prompt_allowed": false,
            "raw_logs_require_redaction": true,
        }),
    );

    Ok(WorkflowExecution {
        id: execution_id,
        tenant_id: state.tenant_id,
        dataset_id: None,
        report_plan_id: None,
        kind: WorkflowKind::CodexHostTask,
        version: runtime_state.version,
        stage: runtime_state.stage,
        status: runtime_state.status,
        attempt: 0,
        context: Value::Object(context),
        created_at: now,
        updated_at: now,
    })
}

fn build_initial_codex_host_task_event(
    execution: &WorkflowExecution,
    assistant_run_id: AssistantRunId,
) -> WorkflowEventRecord {
    WorkflowEventRecord {
        id: WorkflowEventId::new(),
        execution_id: execution.id,
        sequence_no: 1,
        event_name: "codex_host_task.created".to_string(),
        payload: json!({
            "kind": execution.kind.as_str(),
            "version": execution.version,
            "status": execution.status.as_str(),
            "stage": execution.stage,
            "assistant_run_id": assistant_run_id.to_string(),
            "capability": execution.context.get("capability").cloned().unwrap_or(Value::Null),
            "task_memory_policy": execution.context.get("task_memory_policy").cloned().unwrap_or(Value::Null),
        }),
        created_at: execution.created_at,
    }
}

fn codex_host_task_text_from_arguments(arguments: &Value) -> Option<String> {
    arguments
        .get("task")
        .or_else(|| arguments.get("prompt"))
        .or_else(|| arguments.get("instruction"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| truncate_for_openclaw_stub(value, 1_200))
}

fn codex_host_task_preflight_rejection(
    action: &AssistantRunNextAction,
    active_assistant_run_id: Option<AssistantRunId>,
) -> Option<AssistantRunReactToolResult> {
    if !react_env_flag("CODEX_HOST_TASK_ENABLED", false) {
        return Some(rejected_react_tool_result(
            action,
            "codex_host_execution_disabled",
        ));
    }
    if active_assistant_run_id.is_none() {
        return Some(rejected_react_tool_result(
            action,
            "active_assistant_run_required",
        ));
    }
    let Some(capability) = codex_host_task_capability(action) else {
        return Some(rejected_react_tool_result(
            action,
            "codex_host_task_capability_required",
        ));
    };
    if !codex_host_capability_allowed(capability) {
        return Some(rejected_react_tool_result(
            action,
            "codex_host_task_not_allowlisted",
        ));
    }
    None
}

fn codex_host_task_capability(action: &AssistantRunNextAction) -> Option<&str> {
    action
        .arguments
        .get("capability")
        .or_else(|| action.arguments.get("task_type"))
        .or_else(|| action.arguments.get("taskType"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn ok_openclaw_stub_result(
    action: &AssistantRunNextAction,
    message: &str,
    item: Value,
) -> AssistantRunReactToolResult {
    AssistantRunReactToolResult {
        observation: json!({
            "status": "completed",
            "action_type": action.action_type.as_str(),
            "actionType": action.action_type.as_str(),
            "message": message,
            "items": [item],
            "limits": {
                "mode": "stub",
                "externalExecution": false,
            },
        }),
        trail_step: json!({
            "status": "completed",
            "label": assistant_run_react_action_label(&action.action_type),
            "react_action": action.action_type.as_str(),
            "returned_count": 1,
            "at": Utc::now(),
        }),
        final_answer: None,
    }
}

fn react_env_flag(key: &str, default_value: bool) -> bool {
    env::var(key)
        .ok()
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            matches!(normalized.as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(default_value)
}

fn openclaw_readonly_capability_allowed(capability: &str) -> bool {
    env::var("OPENCLAW_READONLY_EXECUTION_ALLOWLIST")
        .ok()
        .map(|raw| {
            raw.split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .any(|item| item == capability)
        })
        .unwrap_or(false)
}

fn codex_host_capability_allowed(capability: &str) -> bool {
    env::var("CODEX_HOST_TASK_ALLOWLIST")
        .ok()
        .map(|raw| {
            raw.split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .any(|item| item == capability)
        })
        .unwrap_or(false)
}

fn truncate_for_openclaw_stub(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn report_choice_items() -> Vec<Value> {
    vec![
        json!({"key": "continue_qa", "label": "继续问答", "type": "report_choice"}),
        json!({"key": "create_report", "label": "生成报表", "type": "report_choice"}),
    ]
}

const REACT_READ_DOCUMENT_MAX_DOCUMENTS: usize = 3;
const REACT_READ_DOCUMENT_MAX_CHUNKS_PER_DOCUMENT: usize = 8;
const REACT_READ_DOCUMENT_MAX_CHARS: usize = 8000;
const REACT_READ_DOCUMENT_FIELD_CHAR_LIMIT: usize = 1200;

async fn read_document_detail_result(
    state: &AppState,
    action: &AssistantRunNextAction,
    selected_scope: &Value,
    active_secret_binding_ids: &[SecretBindingId],
) -> std::result::Result<AssistantRunReactToolResult, ApiError> {
    let requested_document_ids = requested_document_ids_from_action(action, selected_scope);
    if requested_document_ids.is_empty() {
        return Ok(rejected_react_tool_result(action, "document_id_required"));
    }

    let selected_dataset_ids = selected_scope_id_strings(selected_scope, "dataset");
    let selected_document_ids = selected_scope_id_strings(selected_scope, "document");
    let mut items = Vec::new();
    let mut denied = Vec::new();
    let mut returned_chars = 0usize;

    for document_id in requested_document_ids
        .into_iter()
        .take(REACT_READ_DOCUMENT_MAX_DOCUMENTS)
    {
        let document =
            match load_visible_document(state, document_id, active_secret_binding_ids).await {
                Ok(document) => document,
                Err(_) => {
                    denied.push(format!("document:{document_id}"));
                    continue;
                }
            };

        if !document_allowed_by_selected_scope(
            &document,
            &selected_dataset_ids,
            &selected_document_ids,
        ) {
            denied.push(format!("document:{document_id}"));
            continue;
        }

        let chunks = state
            .storage
            .document_chunks()
            .list_by_document(state.tenant_id, document_id)
            .await
            .map_err(ApiError::from_storage)?;
        items.push(document_detail_item(
            &document,
            chunks,
            &mut returned_chars,
            REACT_READ_DOCUMENT_MAX_CHARS,
        ));
        if returned_chars >= REACT_READ_DOCUMENT_MAX_CHARS {
            break;
        }
    }

    let status = if items.is_empty() && !denied.is_empty() {
        "rejected"
    } else {
        "completed"
    };
    let item_count = items.len();
    let denied_count = denied.len();

    Ok(AssistantRunReactToolResult {
        observation: json!({
            "status": status,
            "action_type": action.action_type.as_str(),
            "actionType": action.action_type.as_str(),
            "message": if status == "completed" { "document detail supplied" } else { "document detail denied" },
            "items": items,
            "denied": denied,
            "limits": {
                "maxDocuments": REACT_READ_DOCUMENT_MAX_DOCUMENTS,
                "maxChunksPerDocument": REACT_READ_DOCUMENT_MAX_CHUNKS_PER_DOCUMENT,
                "maxCharacters": REACT_READ_DOCUMENT_MAX_CHARS,
                "returnedCharacters": returned_chars,
            },
        }),
        trail_step: json!({
            "status": status,
            "label": "读取文档详情",
            "react_action": action.action_type.as_str(),
            "item_count": item_count,
            "denied_count": denied_count,
            "at": Utc::now(),
        }),
        final_answer: None,
    })
}

fn requested_document_ids_from_action(
    action: &AssistantRunNextAction,
    selected_scope: &Value,
) -> Vec<DocumentId> {
    let mut ids = Vec::new();
    if let Some(raw) = action
        .arguments
        .get("document_id")
        .or_else(|| action.arguments.get("documentId"))
        .and_then(Value::as_str)
    {
        push_document_id(&mut ids, raw);
    }
    if let Some(raw_items) = action
        .arguments
        .get("document_ids")
        .or_else(|| action.arguments.get("documentIds"))
        .and_then(Value::as_array)
    {
        for item in raw_items {
            if let Some(raw) = item.as_str() {
                push_document_id(&mut ids, raw);
            }
        }
    }
    if ids.is_empty() {
        for raw in selected_scope_id_strings(selected_scope, "document") {
            push_document_id(&mut ids, &raw);
        }
    }
    ids.truncate(REACT_READ_DOCUMENT_MAX_DOCUMENTS);
    ids
}

fn push_document_id(ids: &mut Vec<DocumentId>, raw: &str) {
    let Some(id) = Uuid::parse_str(raw.trim()).ok().map(DocumentId) else {
        return;
    };
    if !ids.contains(&id) {
        ids.push(id);
    }
}

fn selected_scope_id_strings(selected_scope: &Value, expected_type: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let Some(object) = selected_scope.as_object() else {
        return ids;
    };
    let keys: &[&str] = match expected_type {
        "dataset" => &["datasets", "selected"],
        "document" => &["documents", "selected"],
        _ => &["selected"],
    };
    for key in keys {
        let Some(items) = object.get(*key).and_then(Value::as_array) else {
            continue;
        };
        for item in items {
            let item_type = item
                .as_object()
                .and_then(|object| object.get("type"))
                .and_then(Value::as_str);
            if item_type.is_some_and(|item_type| item_type != expected_type) {
                continue;
            }
            let raw = item.as_str().or_else(|| {
                item.as_object()
                    .and_then(|object| object.get("id"))
                    .and_then(Value::as_str)
            });
            if let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) {
                let id = raw.to_string();
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
        }
    }
    ids
}

fn document_allowed_by_selected_scope(
    document: &Document,
    selected_dataset_ids: &[String],
    selected_document_ids: &[String],
) -> bool {
    if !selected_document_ids.is_empty() {
        return selected_document_ids.contains(&document.id.to_string());
    }
    !selected_dataset_ids.is_empty()
        && selected_dataset_ids.contains(&document.dataset_id.to_string())
}

fn document_detail_item(
    document: &Document,
    chunks: Vec<DocumentChunk>,
    returned_chars: &mut usize,
    max_chars: usize,
) -> Value {
    let mut chunk_items = Vec::new();
    for chunk in chunks
        .into_iter()
        .take(REACT_READ_DOCUMENT_MAX_CHUNKS_PER_DOCUMENT)
    {
        if *returned_chars >= max_chars {
            break;
        }
        let remaining = max_chars.saturating_sub(*returned_chars);
        let content = truncate_chars(&chunk.content, remaining);
        *returned_chars += content.chars().count();
        chunk_items.push(json!({
            "chunk_id": chunk.id.to_string(),
            "chunk_index": chunk.chunk_index,
            "state": chunk.state.as_str(),
            "token_count": chunk.token_count,
            "content": content,
            "fields": bounded_chunk_fields(&chunk.metadata),
        }));
    }

    json!({
        "document_id": document.id.to_string(),
        "dataset_id": document.dataset_id.to_string(),
        "title": document.title,
        "content_type": document.content_type,
        "lifecycle": document.lifecycle.as_str(),
        "chunks": chunk_items,
    })
}

fn bounded_chunk_fields(metadata: &BTreeMap<String, Value>) -> Value {
    let mut fields = serde_json::Map::new();
    for (output_key, keys) in [
        ("ocr", ["ocr", "ocr_text", "ocrText"]),
        ("table", ["table", "table_text", "tableText"]),
        ("profile", ["profile", "profile_values", "profileValues"]),
    ] {
        for key in keys {
            if let Some(value) = metadata.get(key) {
                fields.insert(
                    output_key.to_string(),
                    Value::String(truncate_chars(
                        &value_to_bounded_string(value),
                        REACT_READ_DOCUMENT_FIELD_CHAR_LIMIT,
                    )),
                );
                break;
            }
        }
    }
    Value::Object(fields)
}

fn value_to_bounded_string(value: &Value) -> String {
    value
        .as_str()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| serde_json::to_string(value).unwrap_or_default())
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::react_agent_contract::AssistantRunReActStatus;
    use domain_model::{
        DatasetId, DocumentChunkId, DocumentChunkState, DocumentLifecycle, TenantId,
    };
    use std::sync::{Mutex, OnceLock};

    fn openclaw_env_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn clear_openclaw_tool_env() {
        for key in [
            "OPENCLAW_EXTENSION_ENABLED",
            "OPENCLAW_MEMORY_ENABLED",
            "OPENCLAW_READONLY_EXECUTION_ENABLED",
            "OPENCLAW_READONLY_EXECUTION_ALLOWLIST",
            "CODEX_HOST_TASK_ENABLED",
            "CODEX_HOST_TASK_ALLOWLIST",
        ] {
            env::remove_var(key);
        }
    }

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
    fn openclaw_react_memory_recall_is_disabled_by_default() {
        let _guard = openclaw_env_test_lock().lock().expect("openclaw env lock");
        clear_openclaw_tool_env();
        let action = test_action(AssistantRunReactActionType::OpenClawMemoryRecall);
        let result = openclaw_memory_recall_result(&action, Some("thread-a"));

        assert_eq!(result.observation["status"], json!("rejected"));
        assert_eq!(
            result.observation["message"],
            json!("openclaw_extension_disabled")
        );
        assert!(result.final_answer.is_none());
    }

    #[test]
    fn openclaw_react_memory_recall_enabled_returns_labeled_stub() {
        let _guard = openclaw_env_test_lock().lock().expect("openclaw env lock");
        clear_openclaw_tool_env();
        env::set_var("OPENCLAW_EXTENSION_ENABLED", "true");
        env::set_var("OPENCLAW_MEMORY_ENABLED", "true");
        let mut action = test_action(AssistantRunReactActionType::OpenClawMemoryRecall);
        action.arguments = json!({"query": "客户上次说了什么"});
        let result = openclaw_memory_recall_result(&action, Some("thread-a"));
        clear_openclaw_tool_env();

        assert_eq!(result.observation["status"], json!("completed"));
        assert_eq!(
            result.observation["items"][0]["type"],
            json!("openclaw_memory")
        );
        assert_eq!(
            result.observation["items"][0]["status"],
            json!("bridge_stub")
        );
        assert_eq!(
            result.observation["limits"]["externalExecution"],
            json!(false)
        );
    }

    #[test]
    fn openclaw_react_readonly_execution_requires_enablement_and_allowlist() {
        let _guard = openclaw_env_test_lock().lock().expect("openclaw env lock");
        clear_openclaw_tool_env();
        let mut action = test_action(AssistantRunReactActionType::OpenClawReadonlyExecution);
        action.arguments = json!({"capability": "inspect_local_index"});

        let disabled = openclaw_readonly_execution_result(&action);
        assert_eq!(
            disabled.observation["message"],
            json!("openclaw_extension_disabled")
        );

        env::set_var("OPENCLAW_EXTENSION_ENABLED", "true");
        env::set_var("OPENCLAW_READONLY_EXECUTION_ENABLED", "true");
        let not_allowlisted = openclaw_readonly_execution_result(&action);
        assert_eq!(
            not_allowlisted.observation["message"],
            json!("openclaw_readonly_execution_not_allowlisted")
        );

        env::set_var(
            "OPENCLAW_READONLY_EXECUTION_ALLOWLIST",
            "inspect_local_index",
        );
        let allowed = openclaw_readonly_execution_result(&action);
        clear_openclaw_tool_env();

        assert_eq!(allowed.observation["status"], json!("completed"));
        assert_eq!(
            allowed.observation["items"][0]["type"],
            json!("openclaw_readonly_execution")
        );
        assert_eq!(
            allowed.observation["limits"]["externalExecution"],
            json!(false)
        );
    }

    #[test]
    fn codex_host_task_is_disabled_by_default() {
        let _guard = openclaw_env_test_lock().lock().expect("openclaw env lock");
        clear_openclaw_tool_env();
        let mut action = test_action(AssistantRunReactActionType::CodexHostTask);
        action.arguments = json!({"capability": "inspect_project"});

        let result = codex_host_task_preflight_rejection(&action, None)
            .expect("disabled Codex Host task should be rejected");

        assert_eq!(result.observation["status"], json!("rejected"));
        assert_eq!(
            result.observation["message"],
            json!("codex_host_execution_disabled")
        );
        assert_eq!(result.observation["actionType"], json!("codex_host_task"));
        assert!(result.final_answer.is_none());
    }

    #[test]
    fn codex_host_task_rejects_unallowlisted_capability() {
        let _guard = openclaw_env_test_lock().lock().expect("openclaw env lock");
        clear_openclaw_tool_env();
        env::set_var("CODEX_HOST_TASK_ENABLED", "true");
        let mut action = test_action(AssistantRunReactActionType::CodexHostTask);
        action.arguments = json!({"capability": "write_outside_workspace"});

        let result = codex_host_task_preflight_rejection(&action, Some(AssistantRunId::new()))
            .expect("unallowlisted Codex Host task should be rejected");
        clear_openclaw_tool_env();

        assert_eq!(result.observation["status"], json!("rejected"));
        assert_eq!(
            result.observation["message"],
            json!("codex_host_task_not_allowlisted")
        );
    }

    #[test]
    fn codex_host_task_preflight_allows_enabled_allowlisted_capability() {
        let _guard = openclaw_env_test_lock().lock().expect("openclaw env lock");
        clear_openclaw_tool_env();
        env::set_var("CODEX_HOST_TASK_ENABLED", "true");
        env::set_var("CODEX_HOST_TASK_ALLOWLIST", "inspect_project");
        let mut action = test_action(AssistantRunReactActionType::CodexHostTask);
        action.arguments = json!({"capability": "inspect_project"});

        let result = codex_host_task_preflight_rejection(&action, Some(AssistantRunId::new()));
        clear_openclaw_tool_env();

        assert!(result.is_none());
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

    #[test]
    fn list_report_options_returns_exact_product_choices() {
        let action = test_action(AssistantRunReactActionType::ListReportOptions);
        let selected_scope = json!({
            "mode": "selected",
            "selected": [{"type": "dataset", "id": "dataset-1"}],
        });
        let result = list_report_options_result(&action, &selected_scope);

        assert_eq!(result.observation["status"], json!("completed"));
        assert_eq!(
            result.observation["items"],
            json!([
                {"key": "continue_qa", "label": "继续问答", "type": "report_choice"},
                {"key": "create_report", "label": "生成报表", "type": "report_choice"}
            ])
        );
        assert!(result.final_answer.is_none());
    }

    #[test]
    fn report_choice_records_handoff_without_report_body() {
        let mut action = test_action(AssistantRunReactActionType::ReportChoice);
        action.status = AssistantRunReActStatus::ReportChoice;
        action.arguments = json!({"choice": "create_report"});

        let result = report_choice_result(&action);
        let observation = serde_json::to_string(&result.observation).expect("observation");

        assert_eq!(result.observation["choice"], json!("create_report"));
        assert_eq!(
            result.final_answer.as_deref(),
            Some("已选择生成报表，下一步进入报表流程。")
        );
        assert!(!observation.contains("report_body"));
        assert!(!observation.contains("sections"));
        assert!(!observation.contains("markdown"));
    }

    #[test]
    fn react_read_document_detail_bounds_chunks_characters_and_metadata_fields() {
        let tenant_id = TenantId::new();
        let dataset_id = DatasetId::new();
        let document_id = DocumentId::new();
        let document = Document {
            id: document_id,
            tenant_id,
            dataset_id,
            owner_user_id: None,
            title: "订单明细".to_string(),
            object_key: "uploads/orders.csv".to_string(),
            content_type: "text/csv".to_string(),
            lifecycle: DocumentLifecycle::Indexed,
            secret_binding_ids: Vec::new(),
            metadata: BTreeMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let chunks = (0..12)
            .map(|index| {
                let mut metadata = BTreeMap::new();
                if index == 0 {
                    metadata.insert("ocrText".to_string(), json!("OCR".repeat(700)));
                    metadata.insert("tableText".to_string(), json!({"rows": ["A", "B"]}));
                    metadata.insert("profileValues".to_string(), json!({"amount": "number"}));
                }
                DocumentChunk {
                    id: DocumentChunkId::new(),
                    tenant_id,
                    dataset_id,
                    document_id,
                    chunk_index: index,
                    content: "明细内容".repeat(20),
                    token_count: 100,
                    state: DocumentChunkState::Indexed,
                    metadata,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                }
            })
            .collect::<Vec<_>>();
        let mut returned_chars = 0usize;
        let item = document_detail_item(&document, chunks, &mut returned_chars, 1200);

        assert_eq!(item["document_id"], json!(document_id.to_string()));
        assert_eq!(
            item["chunks"].as_array().expect("chunks").len(),
            REACT_READ_DOCUMENT_MAX_CHUNKS_PER_DOCUMENT
        );
        assert!(returned_chars <= 1200);
        assert!(
            item["chunks"][0]["fields"]["ocr"]
                .as_str()
                .expect("ocr")
                .chars()
                .count()
                <= REACT_READ_DOCUMENT_FIELD_CHAR_LIMIT
        );
        assert!(item["chunks"][0]["fields"].get("table").is_some());
        assert!(item["chunks"][0]["fields"].get("profile").is_some());
    }

    #[test]
    fn react_read_document_detail_scope_check_denies_outside_document() {
        let tenant_id = TenantId::new();
        let dataset_id = DatasetId::new();
        let selected_document_id = DocumentId::new();
        let outside_document = Document {
            id: DocumentId::new(),
            tenant_id,
            dataset_id,
            owner_user_id: None,
            title: "未选中文档".to_string(),
            object_key: "uploads/outside.txt".to_string(),
            content_type: "text/plain".to_string(),
            lifecycle: DocumentLifecycle::Indexed,
            secret_binding_ids: Vec::new(),
            metadata: BTreeMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert!(!document_allowed_by_selected_scope(
            &outside_document,
            &[],
            &[selected_document_id.to_string()],
        ));
        assert!(document_allowed_by_selected_scope(
            &outside_document,
            &[dataset_id.to_string()],
            &[],
        ));
    }
}
