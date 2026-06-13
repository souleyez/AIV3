use domain_model::{AssistantRunId, ChatMessageRole, ConversationMemoryItem};
use serde_json::{json, Value};
use uuid::Uuid;

const ASSISTANT_RUN_CONVERSATION_MEMORY_DEFAULT_LIMIT: i64 = 4;
const ASSISTANT_RUN_CONVERSATION_MEMORY_MAX_LIMIT: i64 = 8;

pub(crate) fn conversation_memory_item_external_conversation_id(
    item: &ConversationMemoryItem,
) -> Option<&str> {
    item.metadata
        .get("conversation_external_id")
        .and_then(Value::as_str)
        .or_else(|| {
            item.source_message_refs.as_array().and_then(|refs| {
                refs.iter().find_map(|reference| {
                    reference
                        .get("conversation_external_id")
                        .and_then(Value::as_str)
                })
            })
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub(crate) fn conversation_memory_item_source_assistant_run_ids(
    item: &ConversationMemoryItem,
) -> Vec<AssistantRunId> {
    let mut run_ids = Vec::new();
    for value in item
        .artifact_refs
        .as_array()
        .into_iter()
        .flat_map(|items| items.iter())
        .chain(
            item.source_message_refs
                .as_array()
                .into_iter()
                .flat_map(|items| items.iter()),
        )
    {
        let Some(raw) = value
            .get("assistant_run_id")
            .or_else(|| value.get("assistantRunId"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        if let Ok(run_id) = Uuid::parse_str(raw.trim()).map(AssistantRunId) {
            if !run_ids.contains(&run_id) {
                run_ids.push(run_id);
            }
        }
    }
    run_ids
}

pub(crate) fn conversation_memory_item_supply_value(item: ConversationMemoryItem) -> Value {
    json!({
        "type": "conversation_memory_item",
        "conversation_memory_item_id": item.id,
        "local_thread_id": item.local_thread_id,
        "role": item.role.as_str(),
        "item_kind": item.item_kind,
        "summary": item.summary,
        "source_message_refs": item.source_message_refs,
        "artifact_refs": item.artifact_refs,
        "metadata": item.metadata,
        "created_at": item.created_at,
        "updated_at": item.updated_at,
    })
}

pub(crate) fn assistant_run_conversation_memory_limit() -> i64 {
    conversation_memory_limit_from_env_value(
        std::env::var("ASSISTANT_RUN_CONVERSATION_MEMORY_LIMIT")
            .ok()
            .as_deref(),
    )
}

fn conversation_memory_limit_from_env_value(value: Option<&str>) -> i64 {
    value
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(ASSISTANT_RUN_CONVERSATION_MEMORY_DEFAULT_LIMIT)
        .clamp(1, ASSISTANT_RUN_CONVERSATION_MEMORY_MAX_LIMIT)
}

pub(crate) fn assistant_run_memory_item_is_supply_eligible(item: &ConversationMemoryItem) -> bool {
    item.role == ChatMessageRole::User
        && !matches!(
            item.item_kind.as_str(),
            "assistant_output" | "artifact_output" | "generated_artifact"
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{ConversationMemoryItemId, TenantId};
    use serde_json::json;

    fn sample_item() -> ConversationMemoryItem {
        ConversationMemoryItem {
            id: ConversationMemoryItemId::new(),
            tenant_id: TenantId::new(),
            user_id: None,
            local_thread_id: "thread-1".to_string(),
            role: ChatMessageRole::User,
            item_kind: "user_statement".to_string(),
            summary: "用户关注经营风险。".to_string(),
            source_message_refs: json!([]),
            artifact_refs: json!([]),
            metadata: json!({}),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn memory_limit_parses_default_and_clamped_values() {
        assert_eq!(conversation_memory_limit_from_env_value(None), 4);
        assert_eq!(conversation_memory_limit_from_env_value(Some("invalid")), 4);
        assert_eq!(conversation_memory_limit_from_env_value(Some("-2")), 1);
        assert_eq!(conversation_memory_limit_from_env_value(Some("0")), 1);
        assert_eq!(conversation_memory_limit_from_env_value(Some("6")), 6);
        assert_eq!(conversation_memory_limit_from_env_value(Some("99")), 8);
    }

    #[test]
    fn memory_item_eligibility_allows_only_user_non_output_items() {
        let mut item = sample_item();
        assert!(assistant_run_memory_item_is_supply_eligible(&item));

        item.role = ChatMessageRole::Assistant;
        assert!(!assistant_run_memory_item_is_supply_eligible(&item));

        item.role = ChatMessageRole::User;
        item.item_kind = "assistant_output".to_string();
        assert!(!assistant_run_memory_item_is_supply_eligible(&item));

        item.item_kind = "artifact_output".to_string();
        assert!(!assistant_run_memory_item_is_supply_eligible(&item));

        item.item_kind = "generated_artifact".to_string();
        assert!(!assistant_run_memory_item_is_supply_eligible(&item));
    }

    #[test]
    fn external_conversation_id_prefers_metadata_then_refs() {
        let mut item = sample_item();
        item.metadata = json!({"conversation_external_id": "  conv-meta  "});
        item.source_message_refs = json!([{"conversation_external_id": "conv-ref"}]);
        assert_eq!(
            conversation_memory_item_external_conversation_id(&item),
            Some("conv-meta")
        );

        item.metadata = json!({});
        assert_eq!(
            conversation_memory_item_external_conversation_id(&item),
            Some("conv-ref")
        );

        item.source_message_refs = json!([{"conversation_external_id": "   "}]);
        assert_eq!(
            conversation_memory_item_external_conversation_id(&item),
            None
        );
    }

    #[test]
    fn source_assistant_run_ids_are_collected_deduped_and_trimmed() {
        let first = AssistantRunId::new();
        let second = AssistantRunId::new();
        let mut item = sample_item();
        item.artifact_refs = json!([
            {"assistant_run_id": format!("  {first}  ")},
            {"assistantRunId": second.to_string()},
            {"assistant_run_id": "not-a-uuid"}
        ]);
        item.source_message_refs = json!([
            {"assistant_run_id": first.to_string()}
        ]);

        assert_eq!(
            conversation_memory_item_source_assistant_run_ids(&item),
            vec![first, second]
        );
    }

    #[test]
    fn supply_value_preserves_public_memory_payload_shape() {
        let mut item = sample_item();
        item.source_message_refs = json!([{"message_id": "m1"}]);
        item.artifact_refs = json!([{"type": "report"}]);
        item.metadata = json!({"source": "browser_summary"});

        let payload = conversation_memory_item_supply_value(item);

        assert_eq!(payload["type"], json!("conversation_memory_item"));
        assert_eq!(payload["local_thread_id"], json!("thread-1"));
        assert_eq!(payload["role"], json!("user"));
        assert_eq!(payload["item_kind"], json!("user_statement"));
        assert_eq!(payload["summary"], json!("用户关注经营风险。"));
        assert_eq!(payload["source_message_refs"][0]["message_id"], json!("m1"));
        assert_eq!(payload["artifact_refs"][0]["type"], json!("report"));
        assert_eq!(payload["metadata"]["source"], json!("browser_summary"));
        assert!(payload.get("created_at").is_some());
        assert!(payload.get("updated_at").is_some());
    }
}
