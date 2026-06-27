use contracts::{ContinueAssistantRunRequest, CreateAssistantRunRequest};
use domain_model::{AssistantRun, AssistantRunEvent};
use serde_json::{Map, Value};

use crate::json_value_support::value_array;
use crate::ApiError;
use crate::{assistant_run_insert_safe_scalar_field, assistant_run_safe_artifact_scalar};

const ASSISTANT_RUN_COMPLETION_DISPATCH_DEFAULT_PROMPT: &str =
    "后台视频/PPT提取已完成，请基于待模型接手请求和已完成 observation，用模型自己的口吻输出下一条结果说明。";

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

pub(crate) fn assistant_run_continue_request_from_completion_dispatch(
    dispatch_request: &Value,
) -> std::result::Result<ContinueAssistantRunRequest, ApiError> {
    let continue_request = dispatch_request
        .get("continue_request")
        .filter(|value| value.is_object())
        .ok_or_else(|| {
            ApiError::bad_request(
                "missing_model_completion_turn_continue_request",
                "dispatch request requires continue_request".to_string(),
            )
        })?;
    let prompt = continue_request
        .get("prompt")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .or_else(|| Some(ASSISTANT_RUN_COMPLETION_DISPATCH_DEFAULT_PROMPT.to_string()));
    let max_steps = continue_request
        .get("max_steps")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .or(Some(1));
    let current_artifact = continue_request
        .get("current_artifact")
        .filter(|value| !value.is_null())
        .cloned();

    Ok(ContinueAssistantRunRequest {
        prompt,
        max_steps,
        current_artifact,
        messages: Vec::new(),
    })
}

pub(crate) fn assistant_run_model_completion_turn_consumed_event<'a>(
    events: &'a [AssistantRunEvent],
    idempotency_key: &str,
) -> Option<&'a AssistantRunEvent> {
    events.iter().rev().find(|event| {
        event.event_name == "assistant_run.model_completion_turn_consumed"
            && event.payload.get("idempotency_key").and_then(Value::as_str) == Some(idempotency_key)
    })
}

pub(crate) fn assistant_run_pending_model_completion_requests(
    output_artifacts: &Value,
) -> Vec<Value> {
    value_array(output_artifacts.clone())
        .into_iter()
        .rev()
        .filter_map(|artifact| assistant_run_safe_model_completion_request(&artifact))
        .take(3)
        .collect()
}

fn assistant_run_safe_model_completion_request(artifact: &Value) -> Option<Value> {
    let request = assistant_run_model_completion_request_candidate(artifact)?;
    if request.get("kind").and_then(Value::as_str)
        != Some("video_extraction_model_completion_turn_request")
    {
        return None;
    }
    if request.get("required").and_then(Value::as_bool) != Some(true) {
        return None;
    }

    let mut safe = Map::new();
    assistant_run_copy_safe_scalar_fields(
        &mut safe,
        request,
        &[
            "kind",
            "version",
            "required",
            "turn_owner",
            "source_event",
            "instruction",
        ],
    );

    if let Some(context) = request.get("completion_context") {
        let mut safe_context = Map::new();
        assistant_run_copy_safe_scalar_fields(
            &mut safe_context,
            context,
            &[
                "title",
                "status",
                "warning_count",
                "has_pptx",
                "has_video_slides_markdown",
                "has_subtitle_page_map",
            ],
        );
        for key in [
            "ready_file_kinds",
            "html_artifact_ids",
            "warning_codes",
            "missing_required_file_kinds",
        ] {
            assistant_run_insert_safe_scalar_array_field(&mut safe_context, context, key, key, 20);
        }
        if let Some(primary_next_action) = context.get("primary_next_action") {
            if let Some(safe_action) =
                assistant_run_safe_model_completion_action(primary_next_action)
            {
                safe_context.insert("primary_next_action".to_string(), safe_action);
            }
        }
        if !safe_context.is_empty() {
            safe.insert(
                "completion_context".to_string(),
                Value::Object(safe_context),
            );
        }
    }

    if let Some(answer_contract) = request.get("answer_contract") {
        let mut safe_contract = Map::new();
        assistant_run_copy_safe_scalar_fields(
            &mut safe_contract,
            answer_contract,
            &[
                "must_write_in_model_voice",
                "must_reference_observation_only",
                "must_not_claim_missing_files",
                "must_not_include_private_paths_or_urls",
                "must_not_request_login_cookie_or_recording_bypass",
                "must_keep_missing_items_explicit",
                "no_host_composed_answer",
            ],
        );
        if !safe_contract.is_empty() {
            safe.insert("answer_contract".to_string(), Value::Object(safe_contract));
        }
    }

    let mut source_artifact = Map::new();
    assistant_run_copy_safe_scalar_fields(
        &mut source_artifact,
        artifact,
        &[
            "type",
            "id",
            "title",
            "assistant_run_id",
            "document_id",
            "dataset_id",
            "status",
        ],
    );
    if !source_artifact.is_empty() {
        safe.insert(
            "source_artifact".to_string(),
            Value::Object(source_artifact),
        );
    }

    (!safe.is_empty()).then_some(Value::Object(safe))
}

fn assistant_run_model_completion_request_candidate<'a>(artifact: &'a Value) -> Option<&'a Value> {
    for candidate in [
        artifact.get("model_completion_turn_request"),
        artifact.pointer("/completion_follow_up/model_follow_up"),
        artifact.pointer("/completionFollowUp/modelFollowUp"),
        artifact.pointer("/payload/model_completion_turn_request"),
        artifact.pointer("/payload/completion_follow_up/model_follow_up"),
        artifact.pointer("/payload/completionFollowUp/modelFollowUp"),
    ] {
        if candidate
            .and_then(|value| value.get("kind"))
            .and_then(Value::as_str)
            == Some("video_extraction_model_completion_turn_request")
        {
            return candidate;
        }
    }
    None
}

fn assistant_run_copy_safe_scalar_fields(
    target: &mut Map<String, Value>,
    source: &Value,
    keys: &[&str],
) {
    for key in keys {
        assistant_run_insert_safe_scalar_field(target, source, key, key);
    }
}

fn assistant_run_insert_safe_scalar_array_field(
    target: &mut Map<String, Value>,
    source: &Value,
    source_key: &str,
    target_key: &str,
    limit: usize,
) {
    if let Some(items) = source.get(source_key).and_then(Value::as_array) {
        let safe_items = items
            .iter()
            .take(limit)
            .filter_map(assistant_run_safe_artifact_scalar)
            .collect::<Vec<_>>();
        target.insert(target_key.to_string(), Value::Array(safe_items));
    }
}

pub(crate) fn assistant_run_safe_model_completion_action(action: &Value) -> Option<Value> {
    if let Some(value) = assistant_run_safe_artifact_scalar(action) {
        return Some(value);
    }

    let mut safe_action = Map::new();
    assistant_run_copy_safe_scalar_fields(
        &mut safe_action,
        action,
        &["kind", "action", "code", "label", "summary"],
    );
    (!safe_action.is_empty()).then_some(Value::Object(safe_action))
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use contracts::{AssistantRunMessageView, ContinueAssistantRunRequest};
    use domain_model::{
        AssistantRun, AssistantRunEvent, AssistantRunEventId, AssistantRunId, ChatMessageRole,
        TenantId,
    };
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

    #[test]
    fn continue_request_support_parses_completion_dispatch_request() {
        let request = assistant_run_continue_request_from_completion_dispatch(&json!({
            "continue_request": {
                "prompt": "  请继续输出最终结果  ",
                "max_steps": 4,
                "current_artifact": {"type": "video_ppt", "id": "artifact-1"}
            }
        }))
        .expect("completion dispatch request should parse");

        assert_eq!(request.prompt.as_deref(), Some("请继续输出最终结果"));
        assert_eq!(request.max_steps, Some(4));
        assert_eq!(
            request.current_artifact,
            Some(json!({"type": "video_ppt", "id": "artifact-1"}))
        );
        assert!(request.messages.is_empty());
    }

    #[test]
    fn continue_request_support_defaults_completion_dispatch_values() {
        let request = assistant_run_continue_request_from_completion_dispatch(&json!({
            "continue_request": {
                "prompt": "  ",
                "current_artifact": null
            }
        }))
        .expect("completion dispatch defaults should parse");

        assert_eq!(
            request.prompt.as_deref(),
            Some(ASSISTANT_RUN_COMPLETION_DISPATCH_DEFAULT_PROMPT)
        );
        assert_eq!(request.max_steps, Some(1));
        assert_eq!(request.current_artifact, None);
        assert!(request.messages.is_empty());
    }

    #[test]
    fn continue_request_support_rejects_missing_completion_dispatch_request() {
        let error = assistant_run_continue_request_from_completion_dispatch(&json!({
            "continueRequest": {}
        }))
        .expect_err("missing continue_request should be rejected");

        assert_eq!(
            error.payload.code,
            "missing_model_completion_turn_continue_request"
        );
    }

    fn test_consumed_event(sequence_no: i32, key: &str, event_name: &str) -> AssistantRunEvent {
        AssistantRunEvent {
            id: AssistantRunEventId::new(),
            tenant_id: TenantId::new(),
            run_id: AssistantRunId::new(),
            sequence_no,
            event_name: event_name.to_string(),
            payload: json!({"idempotency_key": key}),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn continue_request_support_finds_latest_consumed_event_for_key() {
        let events = vec![
            test_consumed_event(
                1,
                "same-key",
                "assistant_run.model_completion_turn_consumed",
            ),
            test_consumed_event(
                2,
                "other-key",
                "assistant_run.model_completion_turn_consumed",
            ),
            test_consumed_event(
                3,
                "same-key",
                "assistant_run.model_completion_turn_consumed",
            ),
        ];

        let event = assistant_run_model_completion_turn_consumed_event(&events, "same-key")
            .expect("matching consumed event should be found");

        assert_eq!(event.sequence_no, 3);
    }

    #[test]
    fn continue_request_support_ignores_non_consumed_events_or_wrong_keys() {
        let events = vec![
            test_consumed_event(1, "same-key", "assistant_run.completed"),
            test_consumed_event(
                2,
                "other-key",
                "assistant_run.model_completion_turn_consumed",
            ),
        ];

        assert!(assistant_run_model_completion_turn_consumed_event(&events, "same-key").is_none());
    }

    fn model_completion_artifact(id: &str, required: bool) -> Value {
        json!({
            "type": "video_ppt",
            "id": id,
            "title": format!("artifact-{id}"),
            "status": "completed",
            "model_completion_turn_request": {
                "kind": "video_extraction_model_completion_turn_request",
                "version": 1,
                "required": required,
                "instruction": "请基于已完成 observation 输出结果",
                "completion_context": {
                    "title": "视频/PPT提取结果",
                    "status": "completed",
                    "ready_file_kinds": ["pptx", "markdown", {"unsafe": "object"}],
                    "html_artifact_ids": ["html-1"],
                    "warning_codes": ["missing_subtitles"],
                    "missing_required_file_kinds": ["subtitle_page_map"],
                    "private_url": "https://internal.invalid/secret",
                    "primary_next_action": {
                        "kind": "final_answer",
                        "summary": "给用户说明文件已生成"
                    }
                },
                "answer_contract": {
                    "must_write_in_model_voice": true,
                    "must_not_include_private_paths_or_urls": true,
                    "private_rule_blob": {"raw": true}
                }
            }
        })
    }

    #[test]
    fn continue_request_support_compacts_pending_model_completion_requests() {
        let requests = assistant_run_pending_model_completion_requests(&json!([
            model_completion_artifact("old", true),
            model_completion_artifact("skipped", false),
            model_completion_artifact("new", true)
        ]));

        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0]["source_artifact"]["id"], "new");
        assert_eq!(requests[1]["source_artifact"]["id"], "old");
        assert_eq!(
            requests[0]["completion_context"]["ready_file_kinds"],
            json!(["pptx", "markdown"])
        );
        assert!(requests[0]["completion_context"]
            .get("private_url")
            .is_none());
        assert!(requests[0]["answer_contract"]
            .get("private_rule_blob")
            .is_none());
    }

    #[test]
    fn continue_request_support_limits_pending_model_completion_requests_to_latest_three() {
        let requests = assistant_run_pending_model_completion_requests(&json!([
            model_completion_artifact("1", true),
            model_completion_artifact("2", true),
            model_completion_artifact("3", true),
            model_completion_artifact("4", true)
        ]));

        let ids = requests
            .iter()
            .map(|request| request["source_artifact"]["id"].as_str().unwrap())
            .collect::<Vec<_>>();

        assert_eq!(ids, vec!["4", "3", "2"]);
    }

    #[test]
    fn continue_request_support_sanitizes_model_completion_action() {
        let action = assistant_run_safe_model_completion_action(&json!({
            "kind": "open_file",
            "label": "查看 PPT",
            "private_path": "C:/secret/file.pptx",
            "nested": {"raw": true}
        }))
        .expect("safe action should be produced");

        assert_eq!(action["kind"], "open_file");
        assert_eq!(action["label"], "查看 PPT");
        assert!(action.get("private_path").is_none());
        assert!(action.get("nested").is_none());
    }
}
