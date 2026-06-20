use contracts::AssistantRunMessageView;
use domain_model::{AssistantRun, StaticPageDraft};
use llm_gateway::build_provider_from_env;
use prompt_registry::bootstrap_default_prompt_registry;
use serde_json::{json, Value};
use static_page_runtime::{
    interpret_static_page_intent_deterministic, interpret_static_page_intent_with_provider,
    StaticPageIntentOutcome, StaticPageIntentRequest,
};

use crate::static_page_conversation_memory_support::static_page_conversation_memory_refs;
use crate::static_page_template_reference_support::{
    static_page_template_missing_evidence_for_intent,
    static_page_template_reference_payload_for_intent,
};
use crate::{ApiError, AppState};

const DEFAULT_STATIC_PAGE_INTENT_RUNTIME_MODEL: &str = "static-page-intent-v1";

pub(crate) async fn interpret_static_page_draft_intent_for_api(
    state: &AppState,
    draft: &StaticPageDraft,
    prompt: &str,
    draft_payload: &Value,
    messages: Vec<AssistantRunMessageView>,
) -> std::result::Result<StaticPageIntentOutcome, ApiError> {
    let run = state
        .storage
        .assistant_runs()
        .get_by_id(state.tenant_id, draft.assistant_run_id)
        .await
        .map_err(ApiError::from_storage)?
        .ok_or_else(|| {
            ApiError::not_found(
                "assistant_run_not_found",
                format!("assistant run {} was not found", draft.assistant_run_id),
            )
        })?;
    let runtime_request =
        static_page_intent_request_for_draft(&run, draft, prompt, draft_payload, messages)?;
    interpret_static_page_intent_for_request(runtime_request).await
}

fn static_page_intent_request_for_draft(
    run: &AssistantRun,
    draft: &StaticPageDraft,
    prompt: &str,
    draft_payload: &Value,
    messages: Vec<AssistantRunMessageView>,
) -> std::result::Result<StaticPageIntentRequest, ApiError> {
    let template_reference =
        static_page_template_reference_payload_for_intent(draft_payload, &draft.source_refs)?;
    let missing_evidence = static_page_template_missing_evidence_for_intent(
        draft_payload,
        &draft.source_refs,
        &run.evidence_state,
    )?;
    Ok(StaticPageIntentRequest {
        prompt: prompt.to_string(),
        draft_payload: draft_payload.clone(),
        assistant_run_id: Some(draft.assistant_run_id.to_string()),
        startup_briefing: run.startup_briefing.clone(),
        selected_scope: draft.selected_scope.clone(),
        evidence_state: run.evidence_state.clone(),
        template_reference,
        missing_evidence,
        conversation_memory_refs: static_page_conversation_memory_refs(run),
        messages: messages
            .into_iter()
            .map(|message| {
                json!({
                    "role": message.role.as_str(),
                    "content": message.content,
                })
            })
            .collect(),
    })
}

async fn interpret_static_page_intent_for_request(
    runtime_request: StaticPageIntentRequest,
) -> std::result::Result<StaticPageIntentOutcome, ApiError> {
    let runtime_mode = std::env::var("STATIC_PAGE_INTENT_RUNTIME_MODE")
        .unwrap_or_else(|_| "deterministic".to_string());
    if runtime_mode != "provider" {
        return interpret_static_page_intent_deterministic(&runtime_request).map_err(|error| {
            ApiError::internal(
                "static_page_intent_runtime_failed",
                format!("static page deterministic intent failed: {error}"),
            )
        });
    }

    let runtime_provider = std::env::var("STATIC_PAGE_INTENT_RUNTIME_PROVIDER")
        .unwrap_or_else(|_| "static_page_intent_provider".to_string());
    let runtime_model = std::env::var("STATIC_PAGE_INTENT_RUNTIME_MODEL")
        .unwrap_or_else(|_| DEFAULT_STATIC_PAGE_INTENT_RUNTIME_MODEL.to_string());
    let provider_request = runtime_request.clone();
    let provider_result = tokio::task::spawn_blocking(move || {
        let provider = build_provider_from_env(
            "STATIC_PAGE_INTENT",
            "provider",
            runtime_provider,
            bootstrap_default_prompt_registry(),
        )?;
        interpret_static_page_intent_with_provider(
            &provider_request,
            provider.as_ref(),
            Some(&runtime_model),
        )
    })
    .await
    .map_err(|error| {
        ApiError::internal(
            "static_page_intent_join_failed",
            format!("static page intent worker join failed: {error}"),
        )
    })?;

    match provider_result {
        Ok(outcome) => Ok(outcome),
        Err(error) => {
            let mut fallback =
                interpret_static_page_intent_deterministic(&runtime_request).map_err(|fallback| {
                    ApiError::internal(
                        "static_page_intent_runtime_failed",
                        format!(
                            "static page provider failed ({error}); deterministic fallback also failed: {fallback}"
                        ),
                    )
                })?;
            fallback.runtime = json!({
                "source": "provider_fallback",
                "provider_failure": error.to_string(),
                "fallback": fallback.runtime,
            });
            Ok(fallback)
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use contracts::AssistantRunMessageView;
    use domain_model::{
        AssistantRunId, ChatMessageRole, StaticPageDraftId, StaticPageDraftStatus, TenantId,
    };
    use serde_json::json;

    use super::*;

    fn test_run(evidence_state: Value) -> AssistantRun {
        let now = Utc::now();
        AssistantRun {
            id: AssistantRunId::new(),
            tenant_id: TenantId::new(),
            user_id: None,
            local_thread_id: Some("static-page-intent-test".to_string()),
            user_prompt: "修改报表".to_string(),
            startup_briefing: json!({ "mode": "briefing" }),
            selected_scope: json!({ "ignored": "draft-selected-scope-wins" }),
            scope_candidates: json!([]),
            context_policy: json!({}),
            evidence_state,
            service_lane: "assistant".to_string(),
            execution_trail: json!([]),
            output_artifacts: json!([]),
            runtime_manifest: json!({}),
            created_at: now,
            updated_at: now,
        }
    }

    fn test_draft(assistant_run_id: AssistantRunId) -> StaticPageDraft {
        let now = Utc::now();
        StaticPageDraft {
            id: StaticPageDraftId::new(),
            tenant_id: TenantId::new(),
            owner_user_id: None,
            assistant_run_id,
            title: "经营分析".to_string(),
            status: StaticPageDraftStatus::Planned,
            selected_scope: json!({ "dataset_ids": ["dataset-1"] }),
            visibility_snapshot: json!({}),
            source_refs: json!({}),
            draft_payload: json!({}),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn intent_request_preserves_draft_scope_messages_and_memory_refs() {
        let run = test_run(json!({
            "supplied_items": [
                {"type": "conversation_memory_item", "id": "mem-1", "content": "把取高模块放前面"},
                {"type": "retrieval_evidence", "id": "doc-1"}
            ]
        }));
        let draft = test_draft(run.id);
        let messages = vec![
            AssistantRunMessageView {
                role: ChatMessageRole::User,
                content: "把门店取高放第一屏".to_string(),
            },
            AssistantRunMessageView {
                role: ChatMessageRole::Assistant,
                content: "收到".to_string(),
            },
        ];

        let request = static_page_intent_request_for_draft(
            &run,
            &draft,
            "修改报表",
            &json!({ "title": "原页面" }),
            messages,
        )
        .expect("request should build");

        assert_eq!(request.prompt, "修改报表");
        let expected_run_id = run.id.to_string();
        assert_eq!(
            request.assistant_run_id.as_deref(),
            Some(expected_run_id.as_str())
        );
        assert_eq!(request.draft_payload["title"], json!("原页面"));
        assert_eq!(
            request.selected_scope,
            json!({ "dataset_ids": ["dataset-1"] })
        );
        assert_eq!(request.startup_briefing["mode"], json!("briefing"));
        assert_eq!(request.conversation_memory_refs.len(), 1);
        assert_eq!(request.conversation_memory_refs[0]["id"], json!("mem-1"));
        assert_eq!(
            request.messages,
            vec![
                json!({"role": "user", "content": "把门店取高放第一屏"}),
                json!({"role": "assistant", "content": "收到"})
            ]
        );
    }
}
