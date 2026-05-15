use axum::Json;
use chrono::Utc;
use contracts::{
    CodexHostTaskMemoryPolicyView, CodexHostTaskRequestView, CodexHostTaskSafetyPolicyView,
    CreateStaticPageImageJobRequest, CreateStaticPageRenderRequest, HtmlArtifactDataRefView,
    HtmlArtifactInteractionModeView, HtmlArtifactManifestView, HtmlArtifactOwnerScopeView,
    HtmlArtifactProvenanceView, HtmlArtifactSourceTypeView, HtmlArtifactTemplateIdView,
    VideoExtractionArtifactKindView, VideoExtractionRequestView, VideoExtractionSourceKindView,
    VideoExtractionSourceRefView,
};
use domain_model::{
    AssistantRunId, Document, DocumentChunk, DocumentId, SecretBindingId, StaticPageDraftId,
    StaticPageImageJobId, UserId, WorkflowEventId, WorkflowEventRecord, WorkflowExecution,
    WorkflowExecutionId, WorkflowKind,
};
use serde_json::{json, Value};
use std::{collections::BTreeMap, env, net::IpAddr, time::Duration};
use storage::NewDocument;
use uuid::Uuid;

use crate::{
    append_static_page_draft_run_event, append_static_page_operations_metadata,
    apply_static_page_operations_to_payload, build_assistant_run_evidence_state,
    build_initial_upload_ingest_event, build_initial_upload_ingest_execution,
    ensure_react_requested_dataset_is_selected, ensure_scope_requests_conversation_memory,
    html_artifact_safe_summary_text, load_visible_dataset_for_user, load_visible_document_for_user,
    react_static_page_operations_from_arguments, status_from_static_page_operations,
    status_from_static_page_payload, summarize_static_page_operations,
    to_document_media_detail_view, ApiError, AppState,
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
    current_user_id: Option<UserId>,
) -> std::result::Result<AssistantRunReactToolResult, ApiError> {
    match action.action_type {
        AssistantRunReactActionType::FinalAnswer => Ok(final_answer_result(action)),
        AssistantRunReactActionType::ReadDocumentDetail => {
            read_document_detail_result(
                state,
                action,
                selected_scope,
                active_secret_binding_ids,
                current_user_id,
            )
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
                current_user_id,
            )
            .await?;
            let supplied_count = crate::assistant_run_evidence_supplied_count(&refreshed);
            let detail_target_count = crate::assistant_run_detail_target_count(&refreshed);
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
                    "detail_target_count": detail_target_count,
                    "evidence_status": refreshed.get("status").and_then(Value::as_str).unwrap_or("unknown"),
                }),
                trail_step: json!({
                    "status": "completed",
                    "label": "检索供料证据",
                    "react_action": action.action_type.as_str(),
                    "supplied_count": supplied_count,
                    "detail_target_count": detail_target_count,
                    "at": Utc::now(),
                }),
                final_answer: None,
            })
        }
        AssistantRunReactActionType::WebSearch => Ok(web_search_evidence_required_result(action)),
        AssistantRunReactActionType::RecallConversationMemory => {
            let memory_scope = ensure_scope_requests_conversation_memory(selected_scope.clone());
            let refreshed = build_assistant_run_evidence_state(
                state,
                &memory_scope,
                prompt,
                local_thread_id,
                active_secret_binding_ids,
                current_user_id,
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
        AssistantRunReactActionType::ResolveVideoUrl => {
            resolve_video_url_result(
                state,
                action,
                selected_scope,
                prompt,
                active_secret_binding_ids,
                current_user_id,
            )
            .await
        }
        AssistantRunReactActionType::ExtractVideoPptTranscript => {
            video_ppt_extraction_result(
                state,
                action,
                selected_scope,
                active_assistant_run_id,
                local_thread_id,
                active_secret_binding_ids,
                current_user_id,
            )
            .await
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
        AssistantRunReactActionType::WebSearch => "请求外部/网页搜索证据",
        AssistantRunReactActionType::ReadDocumentDetail => "读取文档详情",
        AssistantRunReactActionType::RecallConversationMemory => "召回对话记忆",
        AssistantRunReactActionType::ListReportOptions => "列出报表选项",
        AssistantRunReactActionType::ResolveVideoUrl => "解析公开视频地址",
        AssistantRunReactActionType::ExtractVideoPptTranscript => "提取视频 PPT 和原文",
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

fn web_search_evidence_required_result(
    action: &AssistantRunNextAction,
) -> AssistantRunReactToolResult {
    let query_chars = action
        .arguments
        .get("query")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().count())
        .unwrap_or(0);
    let freshness = action
        .arguments
        .get("freshness")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unspecified");
    let language_present = action
        .arguments
        .get("language")
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());

    AssistantRunReactToolResult {
        observation: json!({
            "status": "completed",
            "action_type": action.action_type.as_str(),
            "actionType": action.action_type.as_str(),
            "task_status": "v3_search_evidence_required",
            "message": "v3_search_evidence_required",
            "items": [],
            "limits": {},
            "search_evidence_required": true,
            "query_present": query_chars > 0,
            "query_chars": query_chars,
            "freshness": freshness,
            "language_present": language_present,
            "evidence_contract": {
                "requires_source_url": true,
                "requires_source_title": true,
                "requires_retrieved_at": true,
                "requires_query_metadata": true
            },
            "no_live_search_claim": true,
            "allow_general_model_answer": true,
            "next_step": "等待 V3 搜索证据供料；未收到 search evidence 前，回答需说明当前不可见/未供料，并把通用知识或判断清楚标注为非 V3 搜索证据。",
        }),
        trail_step: json!({
            "status": "completed",
            "label": "等待 V3 搜索证据",
            "react_action": action.action_type.as_str(),
            "task_status": "v3_search_evidence_required",
            "search_evidence_required": true,
            "query_present": query_chars > 0,
            "query_chars": query_chars,
            "freshness": freshness,
            "language_present": language_present,
            "at": Utc::now(),
        }),
        final_answer: None,
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
    let draft = crate::load_static_page_draft_or_404(state, draft_id).await?;
    let response = crate::create_static_page_image_job_for_draft(
        state,
        draft,
        CreateStaticPageImageJobRequest {
            prompt: Some(image_prompt),
            image_prompt_payload,
        },
    )
    .await;
    let (_status, Json(response)) = match response {
        Ok(response) => response,
        Err(error) if error.payload.code == "static_page_preview_data_quality_gate" => {
            return Ok(rejected_react_tool_result_with_details(
                action,
                "static_page_preview_data_quality_gate",
                json!({
                    "message": error.payload.message,
                    "dataQualityGate": error.payload.details.clone(),
                    "data_quality_gate": error.payload.details,
                }),
            ));
        }
        Err(error) => return Err(error),
    };
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
    let draft = crate::load_static_page_draft_or_404(state, draft_id).await?;
    let response = crate::create_static_page_render_for_draft(
        state,
        draft,
        CreateStaticPageRenderRequest {
            image_job_id,
            background: false,
        },
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
        Err(error) if error.payload.code == "static_page_preview_stale" => {
            return Ok(static_page_preview_stale_react_result(action));
        }
        Err(error) if error.payload.code == "static_page_final_render_data_quality_gate" => {
            return Ok(rejected_react_tool_result_with_details(
                action,
                "static_page_final_render_data_quality_gate",
                json!({
                    "message": error.payload.message,
                    "dataQualityGate": error.payload.details.clone(),
                    "data_quality_gate": error.payload.details,
                }),
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
    rejected_react_tool_result_with_optional_details(action, reason, None)
}

fn rejected_react_tool_result_with_details(
    action: &AssistantRunNextAction,
    reason: &str,
    details: Value,
) -> AssistantRunReactToolResult {
    rejected_react_tool_result_with_optional_details(action, reason, Some(details))
}

fn rejected_react_tool_result_with_optional_details(
    action: &AssistantRunNextAction,
    reason: &str,
    details: Option<Value>,
) -> AssistantRunReactToolResult {
    let mut observation = json!({
        "status": "rejected",
        "action_type": action.action_type.as_str(),
        "actionType": action.action_type.as_str(),
        "message": reason,
        "denied": [action.action_type.as_str()],
        "items": [],
        "limits": {},
        "reason": reason,
    });
    let mut trail_step = json!({
        "status": "rejected",
        "label": assistant_run_react_action_label(&action.action_type),
        "react_action": action.action_type.as_str(),
        "reason": reason,
        "at": Utc::now(),
    });
    if let Some(details) = details {
        observation["details"] = details.clone();
        trail_step["details"] = details;
    }
    AssistantRunReactToolResult {
        observation,
        trail_step,
        final_answer: None,
    }
}

fn video_url_resolution_placeholder_result(
    action: &AssistantRunNextAction,
    prompt: &str,
) -> AssistantRunReactToolResult {
    let source_url = requested_video_source_url(action, prompt);
    let source_text = source_url.unwrap_or_default();
    let (status, reason, items) = if source_text.is_empty() {
        (
            "rejected",
            "direct_video_url_or_upload_required",
            Vec::new(),
        )
    } else if is_login_gated_video_source(source_text) || prompt.contains("视频号") {
        (
            "rejected",
            "login_gated_video_source_not_supported",
            Vec::new(),
        )
    } else if is_direct_video_url(source_text) {
        (
            "completed",
            "direct_video_url_resolved",
            vec![json!({
                "type": "resolved_video_source",
                "source_type": "direct_video_url",
                "source_url": source_text,
                "asset_state": "remote_unregistered",
                "content_type_guess": guess_video_content_type_from_url(source_text),
                "next_action": "register_remote_video_asset",
            })],
        )
    } else {
        (
            "rejected",
            "video_url_resolution_worker_pending",
            Vec::new(),
        )
    };
    let failure_kind = (status == "rejected").then(|| video_resolution_failure_kind(reason));
    let failure_next_action = failure_kind.map(video_resolution_failure_next_action);
    AssistantRunReactToolResult {
        observation: json!({
            "status": status,
            "action_type": action.action_type.as_str(),
            "actionType": action.action_type.as_str(),
            "message": reason,
            "denied": if status == "rejected" { json!([action.action_type.as_str()]) } else { json!([]) },
            "items": items,
            "limits": {},
            "reason": reason,
            "failure_kind": failure_kind,
            "failure_next_action": failure_next_action,
            "source_present": !source_text.is_empty(),
            "supported_sources": ["uploaded_video_file", "direct_video_url", "public_page_resolvable_video"],
            "unsupported_sources": ["login_gated_page", "qr_login", "cookies", "screen_recording_bypass"],
            "next_step": "请上传视频文件，或提供可直接访问的视频 URL；后台解析器落地后再登记素材并排队提取 PPT/原文。",
        }),
        trail_step: json!({
            "status": status,
            "label": "解析公开视频地址",
            "react_action": action.action_type.as_str(),
            "reason": reason,
            "at": Utc::now(),
        }),
        final_answer: None,
    }
}

fn requested_video_source_url<'a>(
    action: &'a AssistantRunNextAction,
    prompt: &'a str,
) -> Option<&'a str> {
    action
        .arguments
        .get("source_url")
        .or_else(|| action.arguments.get("sourceUrl"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| first_url_in_text(prompt))
}

#[derive(Clone, Debug)]
struct ResolvedVideoSource {
    source_url: String,
    source_page_url: Option<String>,
    source_type: &'static str,
    registration_reason: &'static str,
    title_hint: Option<String>,
}

#[derive(Clone, Debug)]
struct PublicVideoPageResolutionFailure {
    reason: &'static str,
    detail: String,
}

const PUBLIC_VIDEO_PAGE_MAX_BYTES: usize = 512 * 1024;

async fn resolve_public_video_page_source(
    source_url: &str,
) -> std::result::Result<ResolvedVideoSource, PublicVideoPageResolutionFailure> {
    let page_url = reqwest::Url::parse(source_url.trim()).map_err(|error| {
        PublicVideoPageResolutionFailure {
            reason: "public_page_invalid_url",
            detail: format!("invalid public page URL: {error}"),
        }
    })?;
    validate_public_video_page_url(&page_url)?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| PublicVideoPageResolutionFailure {
            reason: "public_page_resolver_client_failed",
            detail: format!("failed to build resolver client: {error}"),
        })?;
    let mut response = client.get(page_url.clone()).send().await.map_err(|error| {
        PublicVideoPageResolutionFailure {
            reason: "public_page_fetch_failed",
            detail: format!("failed to fetch public page: {error}"),
        }
    })?;
    if response.status().is_redirection() {
        return Err(PublicVideoPageResolutionFailure {
            reason: "public_page_redirect_not_followed",
            detail: format!("public page returned redirect HTTP {}", response.status()),
        });
    }
    if !response.status().is_success() {
        return Err(PublicVideoPageResolutionFailure {
            reason: "public_page_fetch_failed",
            detail: format!("public page returned HTTP {}", response.status()),
        });
    }
    let response_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !public_video_page_response_type_allowed(&response_type) {
        return Err(PublicVideoPageResolutionFailure {
            reason: "public_page_not_html",
            detail: "public page response is not HTML".to_string(),
        });
    }
    if response
        .content_length()
        .is_some_and(|content_length| content_length as usize > PUBLIC_VIDEO_PAGE_MAX_BYTES)
    {
        return Err(PublicVideoPageResolutionFailure {
            reason: "public_page_too_large",
            detail: "public page exceeds resolver byte limit".to_string(),
        });
    }

    let mut bytes = Vec::new();
    while let Some(chunk) =
        response
            .chunk()
            .await
            .map_err(|error| PublicVideoPageResolutionFailure {
                reason: "public_page_fetch_failed",
                detail: format!("failed to read public page: {error}"),
            })?
    {
        if bytes.len() + chunk.len() > PUBLIC_VIDEO_PAGE_MAX_BYTES {
            return Err(PublicVideoPageResolutionFailure {
                reason: "public_page_too_large",
                detail: "public page exceeds resolver byte limit".to_string(),
            });
        }
        bytes.extend_from_slice(&chunk);
    }
    let html = String::from_utf8_lossy(&bytes);
    let title_hint = extract_html_title(&html);
    let candidates = extract_video_source_candidates_from_html(&html);
    for candidate in candidates {
        if let Some(video_url) = resolve_public_video_candidate(&page_url, &candidate) {
            return Ok(ResolvedVideoSource {
                source_url: video_url.as_str().to_string(),
                source_page_url: Some(page_url.as_str().to_string()),
                source_type: "public_page_resolvable_video",
                registration_reason: "public_page_video_url_registered",
                title_hint: title_hint.clone(),
            });
        }
    }

    Err(PublicVideoPageResolutionFailure {
        reason: "public_page_video_not_found",
        detail: "public page did not expose a direct video URL in supported fields".to_string(),
    })
}

fn public_video_page_resolution_failure_result(
    action: &AssistantRunNextAction,
    source_url: &str,
    failure: PublicVideoPageResolutionFailure,
) -> AssistantRunReactToolResult {
    AssistantRunReactToolResult {
        observation: json!({
            "status": "rejected",
            "action_type": action.action_type.as_str(),
            "actionType": action.action_type.as_str(),
            "message": failure.reason,
            "denied": [action.action_type.as_str()],
            "items": [],
            "limits": {
                "maxPageBytes": PUBLIC_VIDEO_PAGE_MAX_BYTES,
                "redirectsFollowed": false,
                "loginGatedSources": false,
            },
            "reason": failure.reason,
            "failure_kind": video_resolution_failure_kind(failure.reason),
            "failure_next_action": video_resolution_failure_next_action(video_resolution_failure_kind(failure.reason)),
            "detail": failure.detail,
            "source_present": true,
            "source_host": public_url_host_label(source_url),
            "supported_sources": ["uploaded_video_file", "direct_video_url", "public_page_resolvable_video"],
            "unsupported_sources": ["login_gated_page", "qr_login", "cookies", "screen_recording_bypass"],
            "next_step": "请提供可直接访问的视频 URL，或上传视频文件；公开视频页面必须在 HTML 中暴露 video/source/og:video 直连地址。",
        }),
        trail_step: json!({
            "status": "rejected",
            "label": "解析公开视频地址",
            "react_action": action.action_type.as_str(),
            "reason": failure.reason,
            "at": Utc::now(),
        }),
        final_answer: None,
    }
}

fn video_resolution_failure_kind(reason: &str) -> &'static str {
    match reason {
        "direct_video_url_or_upload_required" => "missing_source",
        "login_gated_video_source_not_supported" | "public_page_invalid_scheme" => {
            "unsupported_source"
        }
        "public_page_host_not_allowed"
        | "public_page_redirect_not_followed"
        | "public_page_too_large" => "resolver_blocked",
        "public_page_fetch_failed" | "public_page_not_html" | "public_page_video_not_found" => {
            "unavailable_video"
        }
        _ => "resolver_failed",
    }
}

fn video_resolution_failure_next_action(failure_kind: &str) -> &'static str {
    match failure_kind {
        "missing_source" => "provide_direct_video_url_or_upload",
        "unsupported_source" => "provide_supported_public_or_uploaded_video",
        "resolver_blocked" => "provide_direct_video_url_or_upload",
        "unavailable_video" => "check_public_video_availability_or_upload",
        _ => "retry_video_url_resolution_or_upload",
    }
}

fn validate_public_video_page_url(
    url: &reqwest::Url,
) -> std::result::Result<(), PublicVideoPageResolutionFailure> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err(PublicVideoPageResolutionFailure {
            reason: "public_page_invalid_scheme",
            detail: "public page URL must use HTTP or HTTPS".to_string(),
        });
    }
    if is_login_gated_video_source(url.as_str()) {
        return Err(PublicVideoPageResolutionFailure {
            reason: "login_gated_video_source_not_supported",
            detail: "login-gated video sources are not supported".to_string(),
        });
    }
    let host = url.host_str().unwrap_or_default();
    if !public_video_page_host_allowed(host) {
        return Err(PublicVideoPageResolutionFailure {
            reason: "public_page_host_not_allowed",
            detail: "public page host is local, private, or otherwise blocked".to_string(),
        });
    }
    Ok(())
}

fn public_video_page_host_allowed(host: &str) -> bool {
    let lower = host.trim().trim_matches(['[', ']']).to_ascii_lowercase();
    if lower.is_empty()
        || lower == "localhost"
        || lower.ends_with(".localhost")
        || lower.ends_with(".local")
    {
        return false;
    }
    if let Ok(ip) = lower.parse::<IpAddr>() {
        return match ip {
            IpAddr::V4(ip) => {
                !(ip.is_private()
                    || ip.is_loopback()
                    || ip.is_link_local()
                    || ip.is_broadcast()
                    || ip.is_documentation()
                    || ip.octets()[0] == 0)
            }
            IpAddr::V6(ip) => !(ip.is_loopback() || ip.is_unspecified() || ip.is_unique_local()),
        };
    }
    true
}

fn public_video_page_response_type_allowed(response_type: &str) -> bool {
    let media_type = response_type.split(';').next().unwrap_or_default().trim();
    media_type.is_empty() || matches!(media_type, "text/html" | "application/xhtml+xml")
}

fn resolve_public_video_candidate(
    page_url: &reqwest::Url,
    candidate: &str,
) -> Option<reqwest::Url> {
    let trimmed = candidate.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('#')
        || trimmed.to_ascii_lowercase().starts_with("javascript:")
        || trimmed.to_ascii_lowercase().starts_with("data:")
    {
        return None;
    }
    let url = reqwest::Url::parse(trimmed)
        .or_else(|_| page_url.join(trimmed))
        .ok()?;
    if validate_public_video_page_url(&url).is_err() || !is_direct_video_url(url.as_str()) {
        return None;
    }
    Some(url)
}

fn public_url_host_label(source_url: &str) -> String {
    reqwest::Url::parse(source_url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_string))
        .unwrap_or_default()
}

fn extract_video_source_candidates_from_html(html: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut cursor = 0usize;
    while let Some(relative_start) = html[cursor..].find('<') {
        let start = cursor + relative_start + 1;
        let Some(relative_end) = html[start..].find('>') else {
            break;
        };
        let end = start + relative_end;
        let tag = &html[start..end];
        let lower = tag.trim_start().to_ascii_lowercase();
        if lower.starts_with("video") || lower.starts_with("source") {
            if let Some(src) = html_attr_value(tag, "src") {
                candidates.push(src);
            }
        } else if lower.starts_with("meta") && html_meta_tag_is_video(tag) {
            if let Some(content) = html_attr_value(tag, "content") {
                candidates.push(content);
            }
        }
        cursor = end + 1;
    }
    candidates
}

fn html_meta_tag_is_video(tag: &str) -> bool {
    ["property", "name", "itemprop"]
        .into_iter()
        .filter_map(|attr| html_attr_value(tag, attr))
        .any(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "og:video"
                    | "og:video:url"
                    | "og:video:secure_url"
                    | "twitter:player:stream"
                    | "video"
            )
        })
}

fn extract_html_title(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let start = lower.find("<title")?;
    let open_end = lower[start..].find('>')? + start + 1;
    let close = lower[open_end..].find("</title>")? + open_end;
    let title = html_attr_unescape(&html[open_end..close]);
    let safe = html_artifact_safe_summary_text(&title, 120);
    (!safe.is_empty()).then_some(safe)
}

fn html_attr_value(tag: &str, attr: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let bytes = tag.as_bytes();
    let lower_bytes = lower.as_bytes();
    let attr_bytes = attr.as_bytes();
    let mut search_from = 0usize;
    while search_from < lower_bytes.len() {
        let relative = lower[search_from..].find(attr)?;
        let start = search_from + relative;
        let end = start + attr_bytes.len();
        if !html_attr_boundary_before(lower_bytes, start)
            || !html_attr_boundary_after(lower_bytes, end)
        {
            search_from = end;
            continue;
        }
        let mut cursor = end;
        while cursor < lower_bytes.len() && lower_bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if lower_bytes.get(cursor) != Some(&b'=') {
            search_from = end;
            continue;
        }
        cursor += 1;
        while cursor < lower_bytes.len() && lower_bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        let quote = lower_bytes.get(cursor).copied();
        if matches!(quote, Some(b'"' | b'\'')) {
            cursor += 1;
            let value_start = cursor;
            while cursor < lower_bytes.len() && lower_bytes[cursor] != quote.unwrap() {
                cursor += 1;
            }
            return Some(html_attr_unescape(&tag[value_start..cursor]));
        }
        let value_start = cursor;
        while cursor < lower_bytes.len()
            && !lower_bytes[cursor].is_ascii_whitespace()
            && lower_bytes[cursor] != b'>'
        {
            cursor += 1;
        }
        return Some(html_attr_unescape(
            std::str::from_utf8(&bytes[value_start..cursor]).unwrap_or_default(),
        ));
    }
    None
}

fn html_attr_boundary_before(bytes: &[u8], start: usize) -> bool {
    start == 0 || !html_attr_name_char(bytes[start - 1])
}

fn html_attr_boundary_after(bytes: &[u8], end: usize) -> bool {
    bytes
        .get(end)
        .is_some_and(|value| value.is_ascii_whitespace() || *value == b'=')
}

fn html_attr_name_char(value: u8) -> bool {
    value.is_ascii_alphanumeric() || matches!(value, b'-' | b'_' | b':')
}

fn html_attr_unescape(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

fn video_ppt_extraction_placeholder_result(
    action: &AssistantRunNextAction,
) -> AssistantRunReactToolResult {
    AssistantRunReactToolResult {
        observation: json!({
            "status": "rejected",
            "action_type": action.action_type.as_str(),
            "actionType": action.action_type.as_str(),
            "message": "uploaded_or_resolved_video_required",
            "denied": [action.action_type.as_str()],
            "items": [],
            "limits": {},
            "reason": "uploaded_or_resolved_video_required",
            "required_asset_state": "uploaded_or_resolved_video",
            "deliverables": ["transcript_text", "slide_image_candidates", "ppt_outline_or_pptx", "timestamp_map"],
            "next_step": "先通过上传或公开视频地址解析拿到 V3 登记的视频素材，再排后台任务提取原文和 PPT。",
        }),
        trail_step: json!({
            "status": "rejected",
            "label": "提取视频 PPT 和原文",
            "react_action": action.action_type.as_str(),
            "reason": "uploaded_or_resolved_video_required",
            "at": Utc::now(),
        }),
        final_answer: None,
    }
}

async fn resolve_video_url_result(
    state: &AppState,
    action: &AssistantRunNextAction,
    selected_scope: &Value,
    prompt: &str,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
) -> std::result::Result<AssistantRunReactToolResult, ApiError> {
    let base_result = video_url_resolution_placeholder_result(action, prompt);
    let base_reason = base_result
        .observation
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or_default();

    let Some(requested_source_url) = requested_video_source_url(action, prompt).map(str::to_string)
    else {
        return Ok(base_result);
    };

    let resolved = if base_reason == "direct_video_url_resolved" {
        ResolvedVideoSource {
            source_url: requested_source_url.clone(),
            source_page_url: None,
            source_type: "direct_video_url",
            registration_reason: "direct_video_url_registered",
            title_hint: Some(direct_video_title_from_url(&requested_source_url)),
        }
    } else if base_reason == "video_url_resolution_worker_pending" {
        match resolve_public_video_page_source(&requested_source_url).await {
            Ok(resolved) => resolved,
            Err(failure) => {
                return Ok(public_video_page_resolution_failure_result(
                    action,
                    &requested_source_url,
                    failure,
                ));
            }
        }
    } else {
        return Ok(base_result);
    };

    let Some(dataset_id) = first_selected_dataset_id(selected_scope) else {
        if resolved.source_type == "public_page_resolvable_video" {
            return Ok(public_video_page_resolution_unregistered_result(
                action, &resolved,
            ));
        }
        return Ok(base_result);
    };
    let dataset = load_visible_dataset_for_user(
        state,
        dataset_id,
        active_secret_binding_ids,
        current_user_id,
    )
    .await?;

    let content_type = guess_video_content_type_from_url(&resolved.source_url).to_string();
    let title = action
        .arguments
        .get("title")
        .or_else(|| action.arguments.get("document_title"))
        .or_else(|| action.arguments.get("documentTitle"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or(resolved.title_hint.clone())
        .unwrap_or_else(|| direct_video_title_from_url(&resolved.source_url));
    let document = state
        .storage
        .documents()
        .create(
            state.tenant_id,
            NewDocument {
                dataset_id,
                title,
                object_key: resolved.source_url.clone(),
                content_type,
                secret_binding_ids: dataset.default_secret_binding_ids.clone(),
                owner_user_id: current_user_id,
                metadata: json!({
                    "source": "react_resolve_video_url",
                    "remote_media": {
                        "source_url": resolved.source_url.clone(),
                        "source_page_url": resolved.source_page_url.clone(),
                        "source_type": resolved.source_type,
                        "asset_state": "remote_registered",
                        "ingest_requires_env": "INGEST_REMOTE_MEDIA_ENABLED",
                    },
                    "processing_policy": {
                        "foreground_allowed": ["register_document", "enqueue_ingest"],
                        "background_required": ["download_remote_media", "parse_content", "media_transcription", "scene_detection", "keyframe_ocr", "indexing"],
                    },
                    "parse_state": {
                        "stage": "queued",
                        "user_blocking": false,
                    },
                }),
            },
        )
        .await
        .map_err(ApiError::from_storage)?;

    let execution = build_initial_upload_ingest_execution(state, &document)?;
    let initial_event = build_initial_upload_ingest_event(&execution, &document);
    state
        .storage
        .workflow_executions()
        .create_with_initial_event(&execution, &initial_event)
        .await
        .map_err(ApiError::from_storage)?;
    let started = crate::apply_workflow_signal(state, execution.id, WorkflowSignal::Start).await?;
    let task_items = started
        .enqueued_tasks
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
            "message": resolved.registration_reason,
            "items": [{
                "type": "resolved_video_source",
                "source_type": resolved.source_type,
                "source_url": document.object_key,
                "source_page_url": resolved.source_page_url,
                "asset_state": "remote_registered",
                "document_id": document.id.to_string(),
                "dataset_id": document.dataset_id.to_string(),
                "title": document.title,
                "content_type": document.content_type,
                "workflow_execution_id": execution.id.to_string(),
                "next_action": "extract_video_ppt_transcript",
            }],
            "tasks": task_items,
            "limits": {
                "remoteDownloadRequiresEnv": "INGEST_REMOTE_MEDIA_ENABLED",
                "backgroundOnly": true,
                "loginGatedSources": false,
            },
            "reason": resolved.registration_reason,
            "source_present": true,
            "supported_sources": ["uploaded_video_file", "direct_video_url", "public_page_resolvable_video"],
            "unsupported_sources": ["login_gated_page", "qr_login", "cookies", "screen_recording_bypass"],
        }),
        trail_step: json!({
            "status": "completed",
            "label": "解析公开视频地址",
            "react_action": action.action_type.as_str(),
            "reason": resolved.registration_reason,
            "document_id": document.id.to_string(),
            "workflow_execution_id": execution.id.to_string(),
            "at": Utc::now(),
        }),
        final_answer: None,
    })
}

fn public_video_page_resolution_unregistered_result(
    action: &AssistantRunNextAction,
    resolved: &ResolvedVideoSource,
) -> AssistantRunReactToolResult {
    AssistantRunReactToolResult {
        observation: json!({
            "status": "completed",
            "action_type": action.action_type.as_str(),
            "actionType": action.action_type.as_str(),
            "message": "public_page_video_url_resolved",
            "denied": [],
            "items": [{
                "type": "resolved_video_source",
                "source_type": resolved.source_type,
                "source_url": resolved.source_url,
                "source_page_url": resolved.source_page_url,
                "asset_state": "remote_unregistered",
                "content_type_guess": guess_video_content_type_from_url(&resolved.source_url),
                "next_action": "register_remote_video_asset",
            }],
            "limits": {
                "maxPageBytes": PUBLIC_VIDEO_PAGE_MAX_BYTES,
                "backgroundOnly": true,
                "loginGatedSources": false,
            },
            "reason": "public_page_video_url_resolved",
            "source_present": true,
            "supported_sources": ["uploaded_video_file", "direct_video_url", "public_page_resolvable_video"],
            "unsupported_sources": ["login_gated_page", "qr_login", "cookies", "screen_recording_bypass"],
            "next_step": "已解析到公开视频直连地址；选择一个可见数据集后，V3 可以登记该视频并排后台解析。",
        }),
        trail_step: json!({
            "status": "completed",
            "label": "解析公开视频地址",
            "react_action": action.action_type.as_str(),
            "reason": "public_page_video_url_resolved",
            "at": Utc::now(),
        }),
        final_answer: None,
    }
}

fn first_selected_dataset_id(selected_scope: &Value) -> Option<domain_model::DatasetId> {
    selected_scope_id_strings(selected_scope, "dataset")
        .into_iter()
        .find_map(|raw| {
            Uuid::parse_str(raw.trim())
                .ok()
                .map(domain_model::DatasetId)
        })
}

fn direct_video_title_from_url(source_url: &str) -> String {
    reqwest::Url::parse(source_url)
        .ok()
        .and_then(|url| {
            url.path_segments()
                .and_then(|mut segments| segments.next_back().map(str::to_string))
        })
        .map(|value| value.trim().trim_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "远程视频素材".to_string())
}

const REACT_VIDEO_PPT_MAX_DOCUMENTS: usize = 2;
const REACT_VIDEO_PPT_MAX_TRANSCRIPT_SEGMENTS: usize = 20;
const REACT_VIDEO_PPT_MAX_SCENES: usize = 12;
const REACT_VIDEO_PPT_MAX_OCR_SNIPPETS: usize = 30;

async fn video_ppt_extraction_result(
    state: &AppState,
    action: &AssistantRunNextAction,
    selected_scope: &Value,
    active_assistant_run_id: Option<AssistantRunId>,
    local_thread_id: Option<&str>,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
) -> std::result::Result<AssistantRunReactToolResult, ApiError> {
    let requested_document_ids = requested_document_ids_from_action(action, selected_scope);
    if requested_document_ids.is_empty() {
        return Ok(video_ppt_extraction_placeholder_result(action));
    }

    let selected_dataset_ids = selected_scope_id_strings(selected_scope, "dataset");
    let selected_document_ids = selected_scope_id_strings(selected_scope, "document");
    let mut items = Vec::new();
    let mut denied = Vec::new();
    let mut queued_workflows = Vec::new();

    for document_id in requested_document_ids
        .into_iter()
        .take(REACT_VIDEO_PPT_MAX_DOCUMENTS)
    {
        let document = match load_visible_document_for_user(
            state,
            document_id,
            active_secret_binding_ids,
            current_user_id,
        )
        .await
        {
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

        if !is_video_document_material(&document) {
            denied.push(format!("document:{document_id}:not_video"));
            continue;
        }

        let extraction_execution = build_initial_video_extraction_execution(
            state,
            action,
            active_assistant_run_id,
            local_thread_id,
            &document,
        )?;
        let chunks = state
            .storage
            .document_chunks()
            .list_by_document(state.tenant_id, document_id)
            .await
            .map_err(ApiError::from_storage)?;
        let detail = to_document_media_detail_view(document, chunks);
        let item = video_ppt_extraction_item(detail);
        if !video_ppt_extraction_item_has_evidence(&item) {
            queued_workflows.push(
                create_and_start_video_extraction_execution(state, extraction_execution).await?,
            );
        }
        items.push(item);
    }

    let has_items = !items.is_empty();
    let has_extracted_evidence = items.iter().any(|item| {
        item.get("evidence_status")
            .and_then(Value::as_str)
            .is_some_and(|status| status == "available")
    });
    let status = if has_extracted_evidence {
        "completed"
    } else if !queued_workflows.is_empty() {
        "queued"
    } else if has_items {
        "partial"
    } else {
        "rejected"
    };
    let message = match status {
        "completed" => "video PPT/transcript evidence supplied",
        "queued" => "video PPT/transcript extraction queued",
        "partial" => "video registered but parsed PPT/transcript evidence is partial or missing",
        _ => "uploaded_or_resolved_video_required",
    };
    let item_count = items.len();
    let denied_count = denied.len();
    let html_artifacts = video_extraction_summary_artifacts_from_items(&items);
    let html_artifact_count = html_artifacts.len();

    Ok(AssistantRunReactToolResult {
        observation: json!({
            "status": status,
            "action_type": action.action_type.as_str(),
            "actionType": action.action_type.as_str(),
            "message": message,
            "items": items,
            "denied": denied,
            "limits": {
                "maxDocuments": REACT_VIDEO_PPT_MAX_DOCUMENTS,
                "maxTranscriptSegments": REACT_VIDEO_PPT_MAX_TRANSCRIPT_SEGMENTS,
                "maxScenes": REACT_VIDEO_PPT_MAX_SCENES,
                "maxOcrSnippets": REACT_VIDEO_PPT_MAX_OCR_SNIPPETS,
            },
            "deliverables": ["transcript_text", "slide_image_candidates", "ppt_outline_or_pptx", "timestamp_map"],
            "html_artifacts": html_artifacts,
            "workflow_executions": queued_workflows,
            "no_host_composed_answer": true,
        }),
        trail_step: json!({
            "status": status,
            "label": "提取视频 PPT 和原文",
            "react_action": action.action_type.as_str(),
            "item_count": item_count,
            "denied_count": denied_count,
            "html_artifact_count": html_artifact_count,
            "workflow_execution_count": queued_workflows.len(),
            "at": Utc::now(),
        }),
        final_answer: None,
    })
}

fn video_ppt_extraction_item_has_evidence(item: &Value) -> bool {
    item.get("evidence_status")
        .and_then(Value::as_str)
        .is_some_and(|status| status == "available")
}

async fn create_and_start_video_extraction_execution(
    state: &AppState,
    execution: WorkflowExecution,
) -> std::result::Result<Value, ApiError> {
    let initial_event = build_initial_video_extraction_event(&execution);
    state
        .storage
        .workflow_executions()
        .create_with_initial_event(&execution, &initial_event)
        .await
        .map_err(ApiError::from_storage)?;
    let started = crate::apply_workflow_signal(state, execution.id, WorkflowSignal::Start).await?;
    let tasks = started
        .enqueued_tasks
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

    Ok(json!({
        "type": "video_extraction_workflow",
        "workflow_execution_id": execution.id.to_string(),
        "stage": started.execution.stage,
        "status": started.execution.status.as_str(),
        "tasks": tasks,
    }))
}

fn build_initial_video_extraction_execution(
    state: &AppState,
    action: &AssistantRunNextAction,
    assistant_run_id: Option<AssistantRunId>,
    local_thread_id: Option<&str>,
    document: &Document,
) -> std::result::Result<WorkflowExecution, ApiError> {
    let definition = state
        .workflow_catalog
        .find_definition(WorkflowKind::VideoExtraction)
        .ok_or_else(|| {
            ApiError::internal(
                "workflow_definition_missing",
                "video_extraction workflow definition is not registered".to_string(),
            )
        })?;
    let now = Utc::now();
    let execution_id = WorkflowExecutionId::new();
    let runtime_state = definition.initial_state(execution_id, now);
    let request = VideoExtractionRequestView {
        assistant_run_id,
        local_thread_id: local_thread_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        source: video_extraction_source_ref_for_document(document),
        target_dataset_id: Some(document.dataset_id),
        requested_outputs: video_extraction_requested_outputs(action),
        background_only: true,
    };
    let mut context = match request.to_workflow_context() {
        Value::Object(map) => map,
        _ => runtime_state.context,
    };
    context.insert(
        "document_id".to_string(),
        Value::String(document.id.to_string()),
    );
    context.insert(
        "dataset_id".to_string(),
        Value::String(document.dataset_id.to_string()),
    );
    context.insert(
        "document_title".to_string(),
        Value::String(html_artifact_safe_summary_text(&document.title, 160)),
    );
    context.insert(
        "content_type".to_string(),
        Value::String(document.content_type.clone()),
    );
    context.insert(
        "retries_remaining".to_string(),
        Value::Number(runtime_state.retries_remaining.into()),
    );

    Ok(WorkflowExecution {
        id: execution_id,
        tenant_id: state.tenant_id,
        dataset_id: Some(document.dataset_id),
        report_plan_id: None,
        kind: WorkflowKind::VideoExtraction,
        version: runtime_state.version,
        stage: runtime_state.stage,
        status: runtime_state.status,
        attempt: 0,
        context: Value::Object(context),
        created_at: now,
        updated_at: now,
    })
}

fn build_initial_video_extraction_event(execution: &WorkflowExecution) -> WorkflowEventRecord {
    WorkflowEventRecord {
        id: WorkflowEventId::new(),
        execution_id: execution.id,
        sequence_no: 1,
        event_name: "video_extraction.created".to_string(),
        payload: json!({
            "kind": execution.kind.as_str(),
            "version": execution.version,
            "status": execution.status.as_str(),
            "stage": execution.stage,
            "document_id": execution.context.get("document_id").cloned().unwrap_or(Value::Null),
            "dataset_id": execution.context.get("dataset_id").cloned().unwrap_or(Value::Null),
            "source": execution.context.get("source").cloned().unwrap_or(Value::Null),
            "requested_outputs": execution.context.get("requested_outputs").cloned().unwrap_or(Value::Null),
        }),
        created_at: execution.created_at,
    }
}

fn video_extraction_source_ref_for_document(document: &Document) -> VideoExtractionSourceRefView {
    let remote_media = document.metadata.get("remote_media");
    let source_type = remote_media
        .and_then(|value| value.get("source_type"))
        .and_then(Value::as_str)
        .and_then(video_extraction_source_kind_from_str)
        .unwrap_or_else(|| {
            if document.object_key.starts_with("http://")
                || document.object_key.starts_with("https://")
            {
                VideoExtractionSourceKindView::DirectVideoUrl
            } else {
                VideoExtractionSourceKindView::UploadedVideoFile
            }
        });
    let source_url = remote_media
        .and_then(|value| value.get("source_url"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            (document.object_key.starts_with("http://")
                || document.object_key.starts_with("https://"))
            .then(|| document.object_key.clone())
        });
    let source_page_url = remote_media
        .and_then(|value| value.get("source_page_url"))
        .and_then(Value::as_str)
        .map(str::to_string);

    VideoExtractionSourceRefView {
        source_type,
        document_id: Some(document.id),
        source_url,
        source_page_url,
        title_hint: Some(document.title.clone()),
    }
}

fn video_extraction_source_kind_from_str(value: &str) -> Option<VideoExtractionSourceKindView> {
    match value {
        "uploaded_video_file" => Some(VideoExtractionSourceKindView::UploadedVideoFile),
        "direct_video_url" => Some(VideoExtractionSourceKindView::DirectVideoUrl),
        "public_page_resolvable_video" => {
            Some(VideoExtractionSourceKindView::PublicPageResolvableVideo)
        }
        _ => None,
    }
}

fn video_extraction_requested_outputs(
    action: &AssistantRunNextAction,
) -> Vec<VideoExtractionArtifactKindView> {
    let requested = action
        .arguments
        .get("deliverables")
        .or_else(|| action.arguments.get("requested_outputs"))
        .or_else(|| action.arguments.get("requestedOutputs"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter_map(video_extraction_artifact_kind_from_str)
        .collect::<Vec<_>>();

    if requested.is_empty() {
        vec![
            VideoExtractionArtifactKindView::TranscriptText,
            VideoExtractionArtifactKindView::SlideImageCandidates,
            VideoExtractionArtifactKindView::PptOutline,
            VideoExtractionArtifactKindView::Pptx,
            VideoExtractionArtifactKindView::Markdown,
            VideoExtractionArtifactKindView::TimestampMap,
            VideoExtractionArtifactKindView::HtmlSummary,
        ]
    } else {
        requested
    }
}

fn video_extraction_artifact_kind_from_str(value: &str) -> Option<VideoExtractionArtifactKindView> {
    match value {
        "transcript_text" => Some(VideoExtractionArtifactKindView::TranscriptText),
        "slide_image_candidates" => Some(VideoExtractionArtifactKindView::SlideImageCandidates),
        "ppt_outline" | "ppt_outline_or_pptx" => Some(VideoExtractionArtifactKindView::PptOutline),
        "pptx" => Some(VideoExtractionArtifactKindView::Pptx),
        "markdown" => Some(VideoExtractionArtifactKindView::Markdown),
        "source_text" => Some(VideoExtractionArtifactKindView::SourceText),
        "timestamp_map" => Some(VideoExtractionArtifactKindView::TimestampMap),
        "html_summary" => Some(VideoExtractionArtifactKindView::HtmlSummary),
        _ => None,
    }
}

fn video_ppt_extraction_item(detail: contracts::DocumentMediaDetailView) -> Value {
    let transcript_count = detail.transcript_segments.len();
    let scene_count = detail.scenes.len();
    let ocr_count = detail.keyframe_ocr_snippets.len();
    let evidence_status = if transcript_count > 0 || scene_count > 0 || ocr_count > 0 {
        "available"
    } else {
        "missing"
    };
    let missing = [
        (transcript_count == 0, "transcript_text"),
        (ocr_count == 0, "slide_image_candidates"),
        (scene_count == 0, "scene_windows"),
    ]
    .into_iter()
    .filter_map(|(is_missing, key)| is_missing.then_some(key))
    .collect::<Vec<_>>();

    json!({
        "type": "video_ppt_transcript_evidence",
        "document": detail.document,
        "media_kind": detail.media_kind,
        "parse_status": detail.parse_status,
        "evidence_status": evidence_status,
        "transcript_segments": detail.transcript_segments
            .into_iter()
            .take(REACT_VIDEO_PPT_MAX_TRANSCRIPT_SEGMENTS)
            .collect::<Vec<_>>(),
        "scenes": detail.scenes
            .into_iter()
            .take(REACT_VIDEO_PPT_MAX_SCENES)
            .collect::<Vec<_>>(),
        "keyframe_ocr_snippets": detail.keyframe_ocr_snippets
            .into_iter()
            .take(REACT_VIDEO_PPT_MAX_OCR_SNIPPETS)
            .collect::<Vec<_>>(),
        "provider_evidence": detail.provider_evidence,
        "missing": missing,
        "model_facing": detail.model_facing,
        "raw_media_metadata": detail.raw_media_metadata,
    })
}

fn video_extraction_summary_artifacts_from_items(items: &[Value]) -> Vec<HtmlArtifactManifestView> {
    items
        .iter()
        .filter_map(video_extraction_summary_artifact_from_item)
        .collect()
}

fn video_extraction_summary_artifact_from_item(item: &Value) -> Option<HtmlArtifactManifestView> {
    let document = item.get("document")?;
    let document_id = video_artifact_string(document, "id", 80)?;
    let dataset_id = video_artifact_string(document, "dataset_id", 80).unwrap_or_default();
    let title = video_artifact_string(document, "title", 120)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "视频素材".to_string());
    let content_type = video_artifact_string(document, "content_type", 80).unwrap_or_default();
    let lifecycle = video_artifact_string(document, "lifecycle", 40).unwrap_or_default();
    let transcript_segments = video_artifact_transcript_segments(item.get("transcript_segments"));
    let scenes = video_artifact_scenes(item.get("scenes"));
    let keyframe_ocr_snippets =
        video_artifact_keyframe_ocr_snippets(item.get("keyframe_ocr_snippets"));
    let provider_evidence = video_artifact_provider_evidence(item.get("provider_evidence"));
    let missing = video_artifact_string_array(item.get("missing"), 12, 80);
    let evidence_status =
        video_artifact_string(item, "evidence_status", 40).unwrap_or_else(|| "missing".to_string());
    let parse_status =
        video_artifact_string(item, "parse_status", 60).unwrap_or_else(|| "unknown".to_string());
    let media_kind =
        video_artifact_string(item, "media_kind", 40).unwrap_or_else(|| "video".to_string());
    let safe_title = html_artifact_safe_summary_text(&title, 120);
    let artifact_title = if safe_title.is_empty() {
        "视频提取摘要".to_string()
    } else {
        format!("{safe_title} · 视频提取摘要")
    };
    let mut data_refs = vec![HtmlArtifactDataRefView {
        kind: "document".to_string(),
        id: document_id.clone(),
        label: safe_title.clone(),
    }];
    if !dataset_id.is_empty() {
        data_refs.push(HtmlArtifactDataRefView {
            kind: "dataset".to_string(),
            id: dataset_id.clone(),
            label: "Dataset".to_string(),
        });
    }

    Some(HtmlArtifactManifestView {
        kind: "html_artifact".to_string(),
        version: 1,
        id: format!("html-video-extraction-{document_id}"),
        title: artifact_title,
        source_type: HtmlArtifactSourceTypeView::VideoExtraction,
        template_id: HtmlArtifactTemplateIdView::VideoExtractionSummary,
        owner_scope: HtmlArtifactOwnerScopeView {
            scope_type: "document".to_string(),
            id: document_id.clone(),
        },
        data_refs,
        provenance: HtmlArtifactProvenanceView {
            producer: "v3-media-runtime".to_string(),
            reason: "video PPT/transcript extraction evidence summary".to_string(),
            source_run_id: None,
        },
        interaction_mode: HtmlArtifactInteractionModeView::ReadOnly,
        created_at: Utc::now(),
        payload: json!({
            "document": {
                "id": document_id,
                "datasetId": dataset_id,
                "title": safe_title,
                "contentType": content_type,
                "lifecycle": lifecycle,
            },
            "mediaKind": media_kind,
            "parseStatus": parse_status,
            "evidenceStatus": evidence_status,
            "summary": {
                "transcriptSegmentCount": transcript_segments.len(),
                "sceneCount": scenes.len(),
                "keyframeOcrSnippetCount": keyframe_ocr_snippets.len(),
                "providerEvidenceCount": provider_evidence.len(),
            },
            "transcriptSegments": transcript_segments,
            "scenes": scenes,
            "keyframeOcrSnippets": keyframe_ocr_snippets,
            "providerEvidence": provider_evidence,
            "missing": missing,
            "deliverables": ["transcript_text", "slide_image_candidates", "ppt_outline_or_pptx", "timestamp_map"],
            "note": "该产物只展示已解析的视频原文、场景和关键帧 OCR 证据；没有证据的部分保持缺失说明，不补造内容。"
        }),
    })
}

fn video_artifact_string(value: &Value, key: &str, max_chars: usize) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(|raw| html_artifact_safe_summary_text(raw, max_chars))
}

fn video_artifact_number(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(Value::as_f64)
}

fn video_artifact_bool(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

fn video_artifact_string_array(
    value: Option<&Value>,
    limit: usize,
    max_chars: usize,
) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .take(limit)
        .map(|raw| html_artifact_safe_summary_text(raw, max_chars))
        .filter(|value| !value.is_empty())
        .collect()
}

fn video_artifact_transcript_segments(value: Option<&Value>) -> Vec<Value> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(REACT_VIDEO_PPT_MAX_TRANSCRIPT_SEGMENTS)
        .map(|segment| {
            json!({
                "startSeconds": video_artifact_number(segment, "start_seconds"),
                "endSeconds": video_artifact_number(segment, "end_seconds"),
                "text": video_artifact_string(segment, "text", 500).unwrap_or_default(),
                "source": video_artifact_string(segment, "source", 80).unwrap_or_default(),
                "language": video_artifact_string(segment, "language", 40).unwrap_or_default(),
                "confidence": video_artifact_number(segment, "confidence"),
            })
        })
        .collect()
}

fn video_artifact_scenes(value: Option<&Value>) -> Vec<Value> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(REACT_VIDEO_PPT_MAX_SCENES)
        .map(|scene| {
            json!({
                "startSeconds": video_artifact_number(scene, "start_seconds"),
                "endSeconds": video_artifact_number(scene, "end_seconds"),
                "representativeSeconds": video_artifact_number(scene, "representative_seconds"),
                "summary": video_artifact_string(scene, "summary", 360).unwrap_or_default(),
                "source": video_artifact_string(scene, "source", 80).unwrap_or_default(),
            })
        })
        .collect()
}

fn video_artifact_keyframe_ocr_snippets(value: Option<&Value>) -> Vec<Value> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(REACT_VIDEO_PPT_MAX_OCR_SNIPPETS)
        .map(|snippet| {
            json!({
                "timestampSeconds": video_artifact_number(snippet, "timestamp_seconds"),
                "text": video_artifact_string(snippet, "text", 360).unwrap_or_default(),
                "source": video_artifact_string(snippet, "source", 80).unwrap_or_default(),
            })
        })
        .collect()
}

fn video_artifact_provider_evidence(value: Option<&Value>) -> Vec<Value> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(8)
        .map(|evidence| {
            json!({
                "provider": video_artifact_string(evidence, "provider", 80).unwrap_or_default(),
                "capability": video_artifact_string(evidence, "capability", 80).unwrap_or_default(),
                "status": video_artifact_string(evidence, "status", 60).unwrap_or_default(),
                "supported": video_artifact_bool(evidence, "supported").unwrap_or(false),
                "detail": video_artifact_string(evidence, "detail", 260).unwrap_or_default(),
                "model": video_artifact_string(evidence, "model", 100).unwrap_or_default(),
            })
        })
        .collect()
}

fn is_video_document_material(document: &Document) -> bool {
    let content_type = document.content_type.trim().to_ascii_lowercase();
    if content_type.starts_with("video/") {
        return true;
    }
    let object_key = document.object_key.to_ascii_lowercase();
    [".mp4", ".mov", ".m4v", ".webm", ".mkv", ".avi"]
        .into_iter()
        .any(|extension| object_key.ends_with(extension))
}

fn first_url_in_text(text: &str) -> Option<&str> {
    text.split_whitespace()
        .find(|part| part.starts_with("http://") || part.starts_with("https://"))
        .map(|part| {
            part.trim_matches(|ch: char| {
                matches!(ch, '，' | '。' | ',' | '.' | ')' | '）' | ']' | '】')
            })
        })
        .filter(|value| !value.is_empty())
}

fn is_login_gated_video_source(source: &str) -> bool {
    let lower = source.to_ascii_lowercase();
    lower.contains("weixin.qq.com/sph/") || lower.contains("channels.weixin.qq.com/sph/")
}

fn is_direct_video_url(source: &str) -> bool {
    let lower = source.trim().to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return false;
    }
    let path = lower
        .split(['?', '#'])
        .next()
        .unwrap_or(lower.as_str())
        .trim_end_matches('/');
    [".mp4", ".mov", ".m4v", ".webm", ".mkv", ".avi"]
        .into_iter()
        .any(|extension| path.ends_with(extension))
}

fn guess_video_content_type_from_url(source: &str) -> &'static str {
    let path = source
        .trim()
        .to_ascii_lowercase()
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .to_string();
    if path.ends_with(".mov") {
        "video/quicktime"
    } else if path.ends_with(".webm") {
        "video/webm"
    } else if path.ends_with(".mkv") {
        "video/x-matroska"
    } else if path.ends_with(".avi") {
        "video/x-msvideo"
    } else {
        "video/mp4"
    }
}

fn static_page_preview_stale_react_result(
    action: &AssistantRunNextAction,
) -> AssistantRunReactToolResult {
    AssistantRunReactToolResult {
        observation: json!({
            "status": "rejected",
            "action_type": action.action_type.as_str(),
            "actionType": action.action_type.as_str(),
            "message": "static_page_preview_stale",
            "denied": [action.action_type.as_str()],
            "items": [],
            "limits": {},
            "reason": "static_page_preview_stale",
            "recommendedActions": ["submit_static_page_image_preview"],
            "nextStep": "规划已经改过，旧效果图和最终页不能继续复用。请先重新提交效果图预览，等待用户确认后再制作最终静态页。",
        }),
        trail_step: json!({
            "status": "rejected",
            "label": "制作最终静态页",
            "react_action": action.action_type.as_str(),
            "reason": "static_page_preview_stale",
            "recommended_action": "submit_static_page_image_preview",
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
                "task_memory_space_id": execution.context.get("task_memory_space_id").cloned().unwrap_or(Value::Null),
            }],
            "tasks": task_items,
            "limits": {
                "mode": "queued",
                "externalExecution": true,
                "taskMemoryIsolated": true,
                "taskMemorySpaceId": execution.context.get("task_memory_space_id").cloned().unwrap_or(Value::Null),
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
    let task_memory_policy =
        CodexHostTaskMemoryPolicyView::task_scoped(assistant_run_id, execution_id);
    let request = CodexHostTaskRequestView {
        assistant_run_id,
        capability: truncate_for_openclaw_stub(capability, 96),
        task: codex_host_task_text_from_arguments(&action.arguments),
        local_thread_id: local_thread_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        task_memory_policy,
        safety: CodexHostTaskSafetyPolicyView::default(),
    };
    let mut context = match request.to_workflow_context() {
        Value::Object(map) => map,
        _ => runtime_state.context,
    };
    context.insert(
        "retries_remaining".to_string(),
        Value::Number(runtime_state.retries_remaining.into()),
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
            "task_memory_space_id": execution.context.get("task_memory_space_id").cloned().unwrap_or(Value::Null),
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
    current_user_id: Option<UserId>,
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
        let document = match load_visible_document_for_user(
            state,
            document_id,
            active_secret_binding_ids,
            current_user_id,
        )
        .await
        {
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
        DatasetId, DocumentChunkId, DocumentChunkState, DocumentId, DocumentLifecycle, TenantId,
    };
    use std::{
        collections::BTreeMap,
        sync::{Mutex, OnceLock},
    };

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

    fn test_document(object_key: &str, content_type: &str) -> Document {
        Document {
            id: DocumentId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            owner_user_id: None,
            title: "测试视频".to_string(),
            object_key: object_key.to_string(),
            content_type: content_type.to_string(),
            lifecycle: DocumentLifecycle::Extracted,
            secret_binding_ids: Vec::new(),
            metadata: BTreeMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
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
        assert!(result.observation.get("details").is_none());
        assert!(result.final_answer.is_none());
    }

    #[test]
    fn rejected_tool_result_can_carry_structured_details() {
        let action = test_action(AssistantRunReactActionType::SubmitStaticPageImagePreview);
        let result = rejected_react_tool_result_with_details(
            &action,
            "static_page_preview_data_quality_gate",
            json!({
                "dataQualityGate": {
                    "attentionModuleCount": 1,
                    "recommendedActions": ["static_page.update_draft"]
                }
            }),
        );

        assert_eq!(result.observation["status"], json!("rejected"));
        assert_eq!(
            result.observation["details"]["dataQualityGate"]["attentionModuleCount"],
            json!(1)
        );
        assert_eq!(
            result.trail_step["details"]["dataQualityGate"]["recommendedActions"][0],
            json!("static_page.update_draft")
        );
        assert!(result.final_answer.is_none());
    }

    #[test]
    fn video_url_resolution_placeholder_blocks_login_gated_sources() {
        let mut action = test_action(AssistantRunReactActionType::ResolveVideoUrl);
        action.arguments = json!({"source_url": "https://weixin.qq.com/sph/ActLMg4yTD"});

        let result = video_url_resolution_placeholder_result(&action, "");

        assert_eq!(result.observation["status"], json!("rejected"));
        assert_eq!(
            result.observation["reason"],
            json!("login_gated_video_source_not_supported")
        );
        assert_eq!(
            result.observation["failure_kind"],
            json!("unsupported_source")
        );
        assert_eq!(
            result.observation["failure_next_action"],
            json!("provide_supported_public_or_uploaded_video")
        );
        assert_eq!(
            result.observation["unsupported_sources"],
            json!([
                "login_gated_page",
                "qr_login",
                "cookies",
                "screen_recording_bypass"
            ])
        );
        assert!(result.final_answer.is_none());
    }

    #[test]
    fn video_url_resolution_accepts_direct_video_urls() {
        let mut action = test_action(AssistantRunReactActionType::ResolveVideoUrl);
        action.arguments =
            json!({"source_url": "https://cdn.example.com/course/lesson-01.MP4?token=redacted"});

        let result = video_url_resolution_placeholder_result(&action, "");

        assert_eq!(result.observation["status"], json!("completed"));
        assert_eq!(
            result.observation["reason"],
            json!("direct_video_url_resolved")
        );
        assert_eq!(result.observation["denied"], json!([]));
        assert_eq!(
            result.observation["items"][0]["source_type"],
            json!("direct_video_url")
        );
        assert_eq!(
            result.observation["items"][0]["content_type_guess"],
            json!("video/mp4")
        );
        assert!(result.final_answer.is_none());
    }

    #[test]
    fn video_url_resolution_helpers_select_dataset_and_title() {
        let dataset_id = DatasetId::new();
        let selected_scope = json!({
            "datasets": [{
                "type": "dataset",
                "id": dataset_id.to_string(),
                "label": "视频库"
            }]
        });

        assert_eq!(first_selected_dataset_id(&selected_scope), Some(dataset_id));
        assert_eq!(
            direct_video_title_from_url(
                "https://cdn.example.com/course/lesson-01.mp4?token=redacted"
            ),
            "lesson-01.mp4"
        );
    }

    #[test]
    fn public_video_page_extracts_video_sources_from_html() {
        let html = r#"
            <html>
              <head>
                <title>公开课视频</title>
                <meta property="og:video" content="/assets/intro.mp4?token=redacted&amp;v=1">
              </head>
              <body>
                <video data-src="/ignored.mp4" src="./lesson-01.webm"></video>
                <source src='https://cdn.example.com/course/lesson-02.mp4'>
              </body>
            </html>
        "#;
        let page_url = reqwest::Url::parse("https://example.com/course/page.html")
            .expect("page URL should parse");
        let candidates = extract_video_source_candidates_from_html(html);
        let resolved = candidates
            .iter()
            .filter_map(|candidate| resolve_public_video_candidate(&page_url, candidate))
            .map(|url| url.as_str().to_string())
            .collect::<Vec<_>>();

        assert_eq!(extract_html_title(html), Some("公开课视频".to_string()));
        assert_eq!(candidates[0], "/assets/intro.mp4?token=redacted&v=1");
        assert_eq!(
            resolved,
            vec![
                "https://example.com/assets/intro.mp4?token=redacted&v=1".to_string(),
                "https://example.com/course/lesson-01.webm".to_string(),
                "https://cdn.example.com/course/lesson-02.mp4".to_string(),
            ]
        );
    }

    #[test]
    fn public_video_page_blocks_private_or_login_gated_sources() {
        let private_url =
            reqwest::Url::parse("http://127.0.0.1/video-page").expect("private URL should parse");
        let login_url = reqwest::Url::parse("https://weixin.qq.com/sph/ActLMg4yTD")
            .expect("login URL should parse");
        let page_url =
            reqwest::Url::parse("https://example.com/page").expect("page URL should parse");

        assert_eq!(
            validate_public_video_page_url(&private_url)
                .expect_err("private host should be rejected")
                .reason,
            "public_page_host_not_allowed"
        );
        assert_eq!(
            validate_public_video_page_url(&login_url)
                .expect_err("login gated source should be rejected")
                .reason,
            "login_gated_video_source_not_supported"
        );
        assert!(resolve_public_video_candidate(&page_url, "http://localhost/video.mp4").is_none());
        assert!(resolve_public_video_candidate(&page_url, "javascript:alert(1)").is_none());
    }

    #[test]
    fn public_video_page_failure_result_is_structured() {
        let action = test_action(AssistantRunReactActionType::ResolveVideoUrl);
        let result = public_video_page_resolution_failure_result(
            &action,
            "https://example.com/page",
            PublicVideoPageResolutionFailure {
                reason: "public_page_video_not_found",
                detail: "no supported video tag".to_string(),
            },
        );

        assert_eq!(result.observation["status"], json!("rejected"));
        assert_eq!(
            result.observation["reason"],
            json!("public_page_video_not_found")
        );
        assert_eq!(
            result.observation["failure_kind"],
            json!("unavailable_video")
        );
        assert_eq!(
            result.observation["failure_next_action"],
            json!("check_public_video_availability_or_upload")
        );
        assert_eq!(result.observation["source_host"], json!("example.com"));
        assert_eq!(
            result.observation["limits"]["redirectsFollowed"],
            json!(false)
        );
        assert!(result.final_answer.is_none());
    }

    #[test]
    fn public_video_page_failure_result_classifies_resolver_blocks() {
        let action = test_action(AssistantRunReactActionType::ResolveVideoUrl);
        let result = public_video_page_resolution_failure_result(
            &action,
            "http://127.0.0.1/video-page",
            PublicVideoPageResolutionFailure {
                reason: "public_page_host_not_allowed",
                detail: "public page host is local, private, or otherwise blocked".to_string(),
            },
        );

        assert_eq!(result.observation["status"], json!("rejected"));
        assert_eq!(
            result.observation["failure_kind"],
            json!("resolver_blocked")
        );
        assert_eq!(
            result.observation["failure_next_action"],
            json!("provide_direct_video_url_or_upload")
        );
        assert_eq!(result.observation["source_host"], json!("127.0.0.1"));
        assert!(result.final_answer.is_none());
    }

    #[test]
    fn public_video_page_unregistered_result_keeps_resolved_video() {
        let action = test_action(AssistantRunReactActionType::ResolveVideoUrl);
        let resolved = ResolvedVideoSource {
            source_url: "https://cdn.example.com/lesson.mp4".to_string(),
            source_page_url: Some("https://example.com/course".to_string()),
            source_type: "public_page_resolvable_video",
            registration_reason: "public_page_video_url_registered",
            title_hint: Some("公开课".to_string()),
        };

        let result = public_video_page_resolution_unregistered_result(&action, &resolved);

        assert_eq!(result.observation["status"], json!("completed"));
        assert_eq!(
            result.observation["reason"],
            json!("public_page_video_url_resolved")
        );
        assert_eq!(
            result.observation["items"][0]["asset_state"],
            json!("remote_unregistered")
        );
        assert_eq!(
            result.observation["items"][0]["source_type"],
            json!("public_page_resolvable_video")
        );
        assert!(result.final_answer.is_none());
    }

    #[test]
    fn video_ppt_extraction_placeholder_requires_registered_video() {
        let action = test_action(AssistantRunReactActionType::ExtractVideoPptTranscript);

        let result = video_ppt_extraction_placeholder_result(&action);

        assert_eq!(result.observation["status"], json!("rejected"));
        assert_eq!(
            result.observation["required_asset_state"],
            json!("uploaded_or_resolved_video")
        );
        assert_eq!(
            result.observation["deliverables"],
            json!([
                "transcript_text",
                "slide_image_candidates",
                "ppt_outline_or_pptx",
                "timestamp_map"
            ])
        );
        assert!(result.final_answer.is_none());
    }

    #[test]
    fn video_document_material_accepts_video_content_type_and_extension() {
        let mut document = test_document("training.mov", "application/octet-stream");

        assert!(is_video_document_material(&document));

        document.object_key = "file:///tmp/training.bin".to_string();
        document.content_type = "video/mp4".to_string();
        assert!(is_video_document_material(&document));

        document.content_type = "application/pdf".to_string();
        assert!(!is_video_document_material(&document));
    }

    #[test]
    fn video_ppt_extraction_item_keeps_media_evidence_for_model() {
        let document = test_document("training.mp4", "video/mp4");
        let detail = to_document_media_detail_view(
            document,
            vec![DocumentChunk {
                id: DocumentChunkId::new(),
                tenant_id: TenantId::new(),
                dataset_id: DatasetId::new(),
                document_id: DocumentId::new(),
                chunk_index: 0,
                content: "视频解析摘要".to_string(),
                token_count: 6,
                state: DocumentChunkState::Extracted,
                metadata: BTreeMap::from_iter([(
                    "parse_metadata".to_string(),
                    json!({
                        "media": {
                            "kind": "video",
                            "parse_status": "transcribed",
                            "transcript_segments": [{
                                "start_seconds": 1.0,
                                "end_seconds": 3.0,
                                "text": "第一页介绍系统目标",
                                "source": "MEDIA_TRANSCRIBE_BIN"
                            }],
                            "scenes": [{
                                "start_seconds": 1.0,
                                "end_seconds": 8.0,
                                "summary": "标题页",
                                "source": "MEDIA_SCENE_BIN"
                            }],
                            "keyframe_ocr_snippets": [{
                                "timestamp_seconds": 2.0,
                                "text": "AI 数据智能助手",
                                "source": "MEDIA_KEYFRAME_OCR_BIN"
                            }],
                            "provider_evidence": []
                        }
                    }),
                )]),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }],
        );

        let item = video_ppt_extraction_item(detail);

        assert_eq!(item["type"], json!("video_ppt_transcript_evidence"));
        assert_eq!(item["evidence_status"], json!("available"));
        assert_eq!(
            item["transcript_segments"][0]["text"],
            json!("第一页介绍系统目标")
        );
        assert_eq!(
            item["keyframe_ocr_snippets"][0]["text"],
            json!("AI 数据智能助手")
        );
        assert_eq!(item["missing"], json!([]));
        assert!(video_ppt_extraction_item_has_evidence(&item));
    }

    #[test]
    fn video_extraction_source_ref_preserves_remote_media_metadata() {
        let mut document =
            test_document("https://cdn.example.com/course/lesson-01.mp4", "video/mp4");
        document.metadata.insert(
            "remote_media".to_string(),
            json!({
                "source_url": "https://cdn.example.com/course/lesson-01.mp4",
                "source_page_url": "https://example.com/course",
                "source_type": "public_page_resolvable_video"
            }),
        );

        let source = video_extraction_source_ref_for_document(&document);

        assert_eq!(
            source.source_type,
            VideoExtractionSourceKindView::PublicPageResolvableVideo
        );
        assert_eq!(source.document_id, Some(document.id));
        assert_eq!(
            source.source_url.as_deref(),
            Some("https://cdn.example.com/course/lesson-01.mp4")
        );
        assert_eq!(
            source.source_page_url.as_deref(),
            Some("https://example.com/course")
        );
    }

    #[test]
    fn video_extraction_requested_outputs_default_and_normalize_aliases() {
        let mut action = test_action(AssistantRunReactActionType::ExtractVideoPptTranscript);

        let defaults = video_extraction_requested_outputs(&action);
        assert!(defaults.contains(&VideoExtractionArtifactKindView::TranscriptText));
        assert!(defaults.contains(&VideoExtractionArtifactKindView::Pptx));
        assert!(defaults.contains(&VideoExtractionArtifactKindView::HtmlSummary));

        action.arguments = json!({
            "deliverables": ["ppt_outline_or_pptx", "markdown", "unknown"]
        });
        let requested = video_extraction_requested_outputs(&action);

        assert_eq!(
            requested,
            vec![
                VideoExtractionArtifactKindView::PptOutline,
                VideoExtractionArtifactKindView::Markdown,
            ]
        );
    }

    #[test]
    fn video_extraction_summary_artifact_uses_safe_manifest() {
        let document = test_document("training.mp4", "video/mp4");
        let detail = to_document_media_detail_view(
            document,
            vec![DocumentChunk {
                id: DocumentChunkId::new(),
                tenant_id: TenantId::new(),
                dataset_id: DatasetId::new(),
                document_id: DocumentId::new(),
                chunk_index: 0,
                content: "视频解析摘要".to_string(),
                token_count: 6,
                state: DocumentChunkState::Extracted,
                metadata: BTreeMap::from_iter([(
                    "parse_metadata".to_string(),
                    json!({
                        "media": {
                            "kind": "video",
                            "parse_status": "transcribed",
                            "transcript_segments": [{
                                "start_seconds": 1.0,
                                "end_seconds": 3.0,
                                "text": "第一页介绍系统目标",
                                "source": "MEDIA_TRANSCRIBE_BIN"
                            }],
                            "scenes": [{
                                "start_seconds": 1.0,
                                "end_seconds": 8.0,
                                "summary": "标题页",
                                "source": "MEDIA_SCENE_BIN"
                            }],
                            "keyframe_ocr_snippets": [{
                                "timestamp_seconds": 2.0,
                                "text": "AI 数据智能助手",
                                "source": "MEDIA_KEYFRAME_OCR_BIN"
                            }],
                            "provider_evidence": [{
                                "provider": "minimax",
                                "capability": "media_understanding",
                                "status": "ready",
                                "supported": true,
                                "detail": "已启用视频解析",
                                "model": "minimax-video"
                            }]
                        }
                    }),
                )]),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }],
        );
        let item = video_ppt_extraction_item(detail);

        let artifacts = video_extraction_summary_artifacts_from_items(&[item]);
        let encoded = serde_json::to_value(&artifacts[0]).expect("artifact should serialize");

        assert_eq!(artifacts.len(), 1);
        assert_eq!(encoded["source_type"], json!("video_extraction"));
        assert_eq!(encoded["template_id"], json!("video_extraction_summary"));
        assert_eq!(encoded["owner_scope"]["type"], json!("document"));
        assert_eq!(
            encoded["payload"]["transcriptSegments"][0]["text"],
            json!("第一页介绍系统目标")
        );
        assert_eq!(
            encoded["payload"]["keyframeOcrSnippets"][0]["text"],
            json!("AI 数据智能助手")
        );
        assert_eq!(
            encoded["payload"]["providerEvidence"][0]["provider"],
            json!("minimax")
        );
        assert!(!encoded.to_string().contains("object_key"));
        assert!(!encoded.to_string().contains("file://"));
    }

    #[test]
    fn static_page_preview_stale_rejection_points_model_to_preview_regeneration() {
        let action = test_action(AssistantRunReactActionType::RenderStaticPage);
        let result = static_page_preview_stale_react_result(&action);

        assert_eq!(result.observation["status"], json!("rejected"));
        assert_eq!(
            result.observation["message"],
            json!("static_page_preview_stale")
        );
        assert_eq!(
            result.observation["recommendedActions"],
            json!(["submit_static_page_image_preview"])
        );
        assert_eq!(
            result.observation["nextStep"],
            json!("规划已经改过，旧效果图和最终页不能继续复用。请先重新提交效果图预览，等待用户确认后再制作最终静态页。")
        );
        assert_eq!(
            result.trail_step["recommended_action"],
            json!("submit_static_page_image_preview")
        );
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
    fn web_search_returns_pending_evidence_without_raw_query() {
        let mut action = test_action(AssistantRunReactActionType::WebSearch);
        action.arguments = json!({
            "query": "2026 年 V3 对外集成最新状态",
            "reason": "用户询问最新进展",
            "freshness": "latest",
            "language": "zh-CN"
        });

        let result = web_search_evidence_required_result(&action);
        let serialized = serde_json::to_string(&result.observation).expect("observation");

        assert_eq!(result.observation["status"], json!("completed"));
        assert_eq!(result.observation["actionType"], json!("web_search"));
        assert_eq!(
            result.observation["task_status"],
            json!("v3_search_evidence_required")
        );
        assert_eq!(result.observation["search_evidence_required"], json!(true));
        assert_eq!(result.observation["no_live_search_claim"], json!(true));
        assert_eq!(
            result.observation["allow_general_model_answer"],
            json!(true)
        );
        assert_eq!(result.observation["query_present"], json!(true));
        assert!(result.observation["query_chars"].as_u64().unwrap_or(0) > 0);
        assert_eq!(result.trail_step["label"], json!("等待 V3 搜索证据"));
        assert!(result.final_answer.is_none());
        assert!(!serialized.contains("对外集成最新状态"));
        assert!(!serialized.contains("用户询问最新进展"));
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
