use contracts::{ContinueAssistantRunRequest, CreateAssistantRunRequest};
use domain_model::AssistantRun;
use serde_json::Value;

use crate::json_value_support::value_array;

pub(crate) fn assistant_run_create_request_from_continue(
    run: &AssistantRun,
    request: &ContinueAssistantRunRequest,
    continue_prompt: &str,
    selected_scope: &Value,
) -> CreateAssistantRunRequest {
    CreateAssistantRunRequest {
        prompt: continue_prompt.trim().to_string(),
        local_thread_id: run.local_thread_id.clone(),
        startup_briefing: Some(run.startup_briefing.clone()),
        selected_scope: Some(selected_scope.clone()),
        scope_candidates: value_array(run.scope_candidates.clone()),
        context_policy_hint: Some(run.context_policy.clone()),
        current_artifact: request.current_artifact.clone(),
        messages: request.messages.clone(),
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use contracts::{AssistantRunMessageView, ContinueAssistantRunRequest};
    use domain_model::{AssistantRun, AssistantRunId, ChatMessageRole, TenantId};
    use serde_json::json;

    use super::*;

    fn continue_test_run() -> AssistantRun {
        let now = Utc::now();
        AssistantRun {
            id: AssistantRunId::new(),
            tenant_id: TenantId::new(),
            user_id: None,
            local_thread_id: Some("local-thread-continue".to_string()),
            user_prompt: "生成报表".to_string(),
            startup_briefing: json!({"surface": "main"}),
            selected_scope: json!({"datasets": ["old-dataset"]}),
            scope_candidates: json!([
                {"id": "candidate-a"},
                {"id": "candidate-b"}
            ]),
            context_policy: json!({"answer_policy": {"output_format": {"format": "rich_text"}}}),
            evidence_state: json!({"status": "supplied"}),
            service_lane: "assistant".to_string(),
            execution_trail: json!([]),
            output_artifacts: json!([]),
            runtime_manifest: json!({}),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn continue_request_support_preserves_run_context_for_quality_retry() {
        let run = continue_test_run();
        let request = ContinueAssistantRunRequest {
            prompt: Some("不用这个原始 prompt".to_string()),
            max_steps: Some(3),
            current_artifact: Some(json!({"type": "static_page", "id": "artifact-1"})),
            messages: vec![AssistantRunMessageView {
                role: ChatMessageRole::User,
                content: "继续修改报表".to_string(),
            }],
        };

        let create_request = assistant_run_create_request_from_continue(
            &run,
            &request,
            "  继续把低活跃模块前置  ",
            &json!({"datasets": ["new-dataset"], "currentArtifact": {"id": "artifact-1"}}),
        );

        assert_eq!(create_request.prompt, "继续把低活跃模块前置");
        assert_eq!(create_request.local_thread_id, run.local_thread_id);
        assert_eq!(
            create_request.startup_briefing,
            Some(json!({"surface": "main"}))
        );
        assert_eq!(
            create_request.selected_scope,
            Some(json!({"datasets": ["new-dataset"], "currentArtifact": {"id": "artifact-1"}}))
        );
        assert_eq!(create_request.scope_candidates.len(), 2);
        assert_eq!(
            create_request.context_policy_hint,
            Some(json!({"answer_policy": {"output_format": {"format": "rich_text"}}}))
        );
        assert_eq!(
            create_request.current_artifact,
            Some(json!({"type": "static_page", "id": "artifact-1"}))
        );
        assert_eq!(create_request.messages.len(), 1);
        assert_eq!(create_request.messages[0].content, "继续修改报表");
    }

    #[test]
    fn continue_request_support_uses_empty_scope_candidates_for_non_array_state() {
        let mut run = continue_test_run();
        run.scope_candidates = json!({"not": "an-array"});
        let request = ContinueAssistantRunRequest {
            prompt: None,
            max_steps: None,
            current_artifact: None,
            messages: Vec::new(),
        };

        let create_request = assistant_run_create_request_from_continue(
            &run,
            &request,
            "继续执行当前 AssistantRun。",
            &run.selected_scope,
        );

        assert!(create_request.scope_candidates.is_empty());
        assert_eq!(create_request.current_artifact, None);
        assert!(create_request.messages.is_empty());
    }
}
