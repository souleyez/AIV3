use contracts::AssistantRunMessageView;
use domain_model::{AssistantRun, ChatMessageRole};
use serde_json::Value;

use crate::truncate_assistant_supply_text;

const EXTERNAL_CHANNEL_CONVERSATION_HISTORY_TEXT_LIMIT: usize = 1200;

pub(crate) fn external_channel_conversation_history_messages_from_recent_runs(
    recent_runs_newest_first: &[AssistantRun],
) -> Vec<AssistantRunMessageView> {
    let mut messages = Vec::new();
    for run in recent_runs_newest_first
        .iter()
        .rev()
        .filter(|run| run.service_lane == "external_channel")
    {
        let user_prompt = truncate_assistant_supply_text(
            &run.user_prompt,
            EXTERNAL_CHANNEL_CONVERSATION_HISTORY_TEXT_LIMIT,
        );
        if !user_prompt.is_empty() {
            messages.push(AssistantRunMessageView {
                role: ChatMessageRole::User,
                content: user_prompt,
            });
        }

        if let Some(reply) = external_channel_assistant_reply_from_run(run) {
            messages.push(AssistantRunMessageView {
                role: ChatMessageRole::Assistant,
                content: reply,
            });
        }
    }
    messages
}

pub(crate) fn external_channel_assistant_reply_from_run(run: &AssistantRun) -> Option<String> {
    external_channel_assistant_reply_from_output_artifacts(&run.output_artifacts)
}

pub(crate) fn external_channel_conversation_external_id_from_run(
    run: &AssistantRun,
) -> Option<String> {
    run.selected_scope
        .get("conversation_external_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(crate) fn external_channel_assistant_reply_from_output_artifacts(
    output_artifacts: &Value,
) -> Option<String> {
    output_artifacts
        .as_array()?
        .iter()
        .rev()
        .find_map(|artifact| {
            (artifact.get("type").and_then(Value::as_str) == Some("assistant_message"))
                .then(|| artifact.get("content").and_then(Value::as_str))
                .flatten()
                .map(|content| {
                    truncate_assistant_supply_text(
                        content,
                        EXTERNAL_CHANNEL_CONVERSATION_HISTORY_TEXT_LIMIT,
                    )
                })
                .filter(|content| !content.is_empty())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Duration, Utc};
    use domain_model::{AssistantRunId, TenantId};
    use serde_json::json;

    fn test_run(
        prompt: &str,
        reply: Option<&str>,
        service_lane: &str,
        selected_scope: Value,
        created_at: DateTime<Utc>,
    ) -> AssistantRun {
        AssistantRun {
            id: AssistantRunId::new(),
            tenant_id: TenantId::new(),
            user_id: None,
            local_thread_id: None,
            user_prompt: prompt.to_string(),
            startup_briefing: Value::Null,
            selected_scope,
            scope_candidates: Value::Null,
            context_policy: Value::Null,
            evidence_state: Value::Null,
            service_lane: service_lane.to_string(),
            execution_trail: Value::Null,
            output_artifacts: reply
                .map(|content| json!([{"type": "assistant_message", "content": content}]))
                .unwrap_or_else(|| json!([])),
            runtime_manifest: Value::Null,
            created_at,
            updated_at: created_at,
        }
    }

    #[test]
    fn conversation_history_orders_external_channel_runs_oldest_first() {
        let now = Utc::now();
        let older = test_run(
            "上一轮问：这份制度讲什么？",
            Some("上一轮答：主要讲采购审批权限。"),
            "external_channel",
            json!({}),
            now - Duration::minutes(2),
        );
        let ignored_other_lane = test_run(
            "不应进入外部通道上下文",
            Some("不应进入"),
            "assistant_run",
            json!({}),
            now - Duration::minutes(1),
        );
        let newer = test_run(
            "继续问：有哪些风险？",
            Some("继续答：审批超时和权限错配。"),
            "external_channel",
            json!({}),
            now,
        );

        let messages = external_channel_conversation_history_messages_from_recent_runs(&[
            newer,
            ignored_other_lane,
            older,
        ]);

        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].role, ChatMessageRole::User);
        assert!(messages[0].content.contains("上一轮问"));
        assert_eq!(messages[1].role, ChatMessageRole::Assistant);
        assert!(messages[1].content.contains("采购审批权限"));
        assert_eq!(messages[2].role, ChatMessageRole::User);
        assert!(messages[2].content.contains("继续问"));
        assert_eq!(messages[3].role, ChatMessageRole::Assistant);
        assert!(messages[3].content.contains("权限错配"));
    }

    #[test]
    fn assistant_reply_from_output_artifacts_uses_latest_non_empty_message() {
        let artifacts = json!([
            {"type": "assistant_message", "content": "旧回复"},
            {"type": "trace", "content": "忽略"},
            {"type": "assistant_message", "content": "  最新  回复  "}
        ]);

        assert_eq!(
            external_channel_assistant_reply_from_output_artifacts(&artifacts).as_deref(),
            Some("最新 回复")
        );
    }

    #[test]
    fn conversation_external_id_from_run_trims_and_rejects_empty_values() {
        let now = Utc::now();
        let run = test_run(
            "prompt",
            None,
            "external_channel",
            json!({"conversation_external_id": "  conv-1  "}),
            now,
        );
        let blank = test_run(
            "prompt",
            None,
            "external_channel",
            json!({"conversation_external_id": "   "}),
            now,
        );

        assert_eq!(
            external_channel_conversation_external_id_from_run(&run).as_deref(),
            Some("conv-1")
        );
        assert_eq!(
            external_channel_conversation_external_id_from_run(&blank),
            None
        );
    }

    #[test]
    fn conversation_history_truncates_user_prompt_and_reply() {
        let long_prompt = "问".repeat(EXTERNAL_CHANNEL_CONVERSATION_HISTORY_TEXT_LIMIT + 20);
        let long_reply = "答".repeat(EXTERNAL_CHANNEL_CONVERSATION_HISTORY_TEXT_LIMIT + 20);
        let run = test_run(
            &long_prompt,
            Some(&long_reply),
            "external_channel",
            json!({}),
            Utc::now(),
        );

        let messages = external_channel_conversation_history_messages_from_recent_runs(&[run]);

        assert_eq!(messages.len(), 2);
        assert_eq!(
            messages[0].content.chars().count(),
            EXTERNAL_CHANNEL_CONVERSATION_HISTORY_TEXT_LIMIT
        );
        assert_eq!(
            messages[1].content.chars().count(),
            EXTERNAL_CHANNEL_CONVERSATION_HISTORY_TEXT_LIMIT
        );
    }
}
