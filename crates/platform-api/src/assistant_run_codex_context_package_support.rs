use contracts::{
    AssistantRunCodexContextPackageView, AssistantRunExecutorTransportView, AssistantRunMessageView,
};
use domain_model::AssistantRunId;
use llm_gateway::{
    resolve_runtime_selection_from_env, LlmRuntimeSelection, MODEL_LANE_CODEX_CONVERSATION,
};
use serde_json::Value;

use crate::assistant_run_codex_action_contract_support::assistant_run_codex_action_contracts;
use crate::assistant_run_codex_context_budget_support::{
    assistant_run_codex_context_budget, assistant_run_codex_hidden_memory_candidates,
};
use crate::assistant_run_codex_tool_output_support::{
    assistant_run_codex_bounded_evidence_state, assistant_run_codex_tool_output_policy,
};
use crate::assistant_run_supply_quality_support::assistant_run_codex_supply_quality;

const DEFAULT_ASSISTANT_RUN_CODEX_RUNTIME_MODEL: &str = "codex-conversation-placeholder";

pub(crate) fn build_assistant_run_codex_context_package(
    assistant_run_id: AssistantRunId,
    local_thread_id: Option<&str>,
    user_prompt: &str,
    messages: &[AssistantRunMessageView],
    startup_briefing: &Value,
    selected_scope: &Value,
    scope_candidates: &[Value],
    evidence_state: &Value,
    current_artifact: Option<&Value>,
    model_gateway: &Value,
    executor_transport: AssistantRunExecutorTransportView,
) -> AssistantRunCodexContextPackageView {
    let mut package =
        AssistantRunCodexContextPackageView::new(assistant_run_id, user_prompt.trim());
    let tool_output_policy = assistant_run_codex_tool_output_policy();
    let bounded_evidence_state =
        assistant_run_codex_bounded_evidence_state(evidence_state, &tool_output_policy);
    package.executor_transport = executor_transport;
    package.local_thread_id = local_thread_id.map(ToString::to_string);
    package.messages = messages.to_vec();
    package.startup_briefing = startup_briefing.clone();
    package.selected_scope = selected_scope.clone();
    package.inferred_scope_candidates = scope_candidates.to_vec();
    package.evidence_state = bounded_evidence_state.clone();
    package.supply_quality = assistant_run_codex_supply_quality(&bounded_evidence_state);
    package.model_gateway = model_gateway.clone();
    package.hidden_memory_candidates =
        assistant_run_codex_hidden_memory_candidates(&bounded_evidence_state);
    package.current_artifact = current_artifact.cloned();
    package.available_actions = assistant_run_codex_action_contracts(selected_scope);
    package.tool_output_policy = tool_output_policy;
    package.context_budget = assistant_run_codex_context_budget(
        user_prompt,
        messages,
        startup_briefing,
        selected_scope,
        scope_candidates,
        &bounded_evidence_state,
        current_artifact,
    );
    package
}

pub(crate) fn assistant_run_codex_runtime_selection() -> LlmRuntimeSelection {
    resolve_runtime_selection_from_env(
        "ASSISTANT_RUN_CODEX",
        MODEL_LANE_CODEX_CONVERSATION,
        DEFAULT_ASSISTANT_RUN_CODEX_RUNTIME_MODEL,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::AssistantRunExecutorTransportView;
    use domain_model::{ChatMessageRole, DatasetId};
    use serde_json::json;

    #[test]
    fn assistant_run_codex_context_package_preserves_scope_supply_and_action_contracts() {
        let dataset_id = DatasetId::new();
        let messages = vec![AssistantRunMessageView {
            role: ChatMessageRole::User,
            content: "继续生成报表".to_string(),
        }];
        let selected_scope = json!({
            "intent": "static_page",
            "datasets": [{"type": "dataset", "id": dataset_id.to_string()}],
            "conversation_memory": ["current_thread"]
        });
        let evidence_state = json!({
            "status": "supplied",
            "supply_quality": {
                "status": "sufficient",
                "citationLocatorCount": 2
            },
            "supplied_items": [{
                "type": "retrieval_evidence",
                "summary": "销售风险证据"
            }],
            "conversation_memory_items": [{"summary": "用户关注取高机会"}],
            "tool_outputs": [{
                "tool_call_id": "tool-1",
                "stdout": format!("start-{}-end", "x".repeat(40_000)),
                "evidence_ref": "evidence-1"
            }]
        });
        let artifact = json!({"backendDraftId": "draft-1"});
        let model_gateway = json!({
            "lane": "codex_conversation",
            "selected_model": {
                "provider": "minimax",
                "model": "MiniMax-M2.7"
            }
        });

        let package = build_assistant_run_codex_context_package(
            AssistantRunId::new(),
            Some("thread-1"),
            "  做一版取高机会报表  ",
            &messages,
            &json!({"product": "DataMax"}),
            &selected_scope,
            &[json!({"type": "dataset", "id": dataset_id.to_string()})],
            &evidence_state,
            Some(&artifact),
            &model_gateway,
            AssistantRunExecutorTransportView::CodexPlanOnly,
        );

        assert_eq!(package.user_prompt, "做一版取高机会报表");
        assert_eq!(package.local_thread_id.as_deref(), Some("thread-1"));
        assert_eq!(
            package.executor_transport,
            AssistantRunExecutorTransportView::CodexPlanOnly
        );
        assert_eq!(package.messages.len(), 1);
        assert_eq!(package.messages[0].role, ChatMessageRole::User);
        assert_eq!(package.messages[0].content, messages[0].content);
        assert_eq!(package.selected_scope, selected_scope);
        assert_eq!(package.inferred_scope_candidates.len(), 1);
        assert_eq!(package.supply_quality["status"], json!("sufficient"));
        assert_eq!(package.hidden_memory_candidates.len(), 1);
        assert_eq!(package.current_artifact, Some(artifact));
        assert_eq!(package.model_gateway["lane"], json!("codex_conversation"));
        assert_eq!(package.context_budget.selected_dataset_count, 1);
        assert_eq!(package.context_budget.evidence_item_count, 1);
        assert_eq!(package.context_budget.hidden_memory_item_count, 1);
        assert_eq!(package.context_budget.trimmed_item_count, 1);
        assert!(package
            .action_types()
            .contains(&"submit_html_artifact_event".to_string()));
        assert!(package.tool_output_policy.preserve_evidence_refs);
        assert!(package.evidence_state["tool_outputs"][0]["stdout"]
            .as_str()
            .unwrap_or_default()
            .contains("[codex-trimmed"));
    }
}
