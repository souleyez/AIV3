use domain_model::{AssistantRunId, ChatMessageRole, ConversationMemoryItem, TenantId, UserId};
use serde_json::{json, Value};
use uuid::Uuid;

const ASSISTANT_RUN_CONVERSATION_MEMORY_DEFAULT_LIMIT: i64 = 4;
const ASSISTANT_RUN_CONVERSATION_MEMORY_MAX_LIMIT: i64 = 8;
pub(crate) const CONVERSATION_MEMORY_SCOPE_CURRENT_THREAD: &str = "local-thread";
pub(crate) const CONVERSATION_MEMORY_SCOPE_CURRENT_THREAD_ALIAS: &str = "current_thread";
const CONVERSATION_MEMORY_SCOPE_LOCAL_THREAD_PREFIX: &str = "local-thread:";
const CONVERSATION_MEMORY_SCOPE_USER_CONTEXT_PREFIX: &str = "user-context:";
const CONVERSATION_MEMORY_SCOPE_EXTERNAL_USER_PREFIX: &str = "external-user:";

pub(crate) fn selected_scope_conversation_memory_ids(scope: &Value) -> Vec<String> {
    scope
        .as_object()
        .and_then(|object| object.get("conversation_memory"))
        .and_then(Value::as_array)
        .map(|items| {
            dedupe_conversation_memory_strings(
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
                    .collect(),
            )
        })
        .unwrap_or_default()
}

pub(crate) fn selected_scope_requests_conversation_memory(scope: &Value) -> bool {
    !selected_scope_conversation_memory_ids(scope).is_empty()
}

pub(crate) fn set_selected_scope_conversation_memory(scope: &mut Value, memory_ids: Vec<String>) {
    let memory_ids = dedupe_conversation_memory_strings(
        memory_ids
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect(),
    );
    if !scope.is_object() {
        *scope = json!({});
    }
    if let Some(object) = scope.as_object_mut() {
        object.insert(
            "conversation_memory".to_string(),
            Value::Array(memory_ids.into_iter().map(Value::String).collect()),
        );
    }
}

fn conversation_memory_scope_id_to_local_thread_id(
    scope_id: &str,
    current_local_thread_id: Option<&str>,
) -> Option<String> {
    let scope_id = scope_id.trim();
    if scope_id.is_empty() {
        return None;
    }
    if matches!(
        scope_id,
        CONVERSATION_MEMORY_SCOPE_CURRENT_THREAD | CONVERSATION_MEMORY_SCOPE_CURRENT_THREAD_ALIAS
    ) {
        return current_local_thread_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
    }
    if let Some(local_thread_id) =
        scope_id.strip_prefix(CONVERSATION_MEMORY_SCOPE_LOCAL_THREAD_PREFIX)
    {
        return Some(local_thread_id.trim().to_string()).filter(|value| !value.is_empty());
    }
    if scope_id.starts_with(CONVERSATION_MEMORY_SCOPE_USER_CONTEXT_PREFIX)
        || scope_id.starts_with(CONVERSATION_MEMORY_SCOPE_EXTERNAL_USER_PREFIX)
    {
        return Some(scope_id.to_string());
    }
    None
}

pub(crate) fn selected_scope_conversation_memory_local_thread_ids(
    scope: &Value,
    current_local_thread_id: Option<&str>,
) -> Vec<String> {
    dedupe_conversation_memory_strings(
        selected_scope_conversation_memory_ids(scope)
            .into_iter()
            .filter_map(|scope_id| {
                conversation_memory_scope_id_to_local_thread_id(&scope_id, current_local_thread_id)
            })
            .collect(),
    )
}

pub(crate) fn conversation_memory_scope_candidate_value(
    id: &str,
    label: &str,
    reason: &str,
    source: &str,
) -> Value {
    json!({
        "type": "conversation_memory",
        "id": id,
        "label": label,
        "confidence": "medium",
        "reason": reason,
        "source": source,
    })
}

pub(crate) fn global_user_context_memory_key(tenant_id: TenantId, user_id: UserId) -> String {
    format!("user-context:user:{tenant_id}:{user_id}")
}

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

fn dedupe_conversation_memory_strings(values: Vec<String>) -> Vec<String> {
    let mut deduped = Vec::new();
    for value in values {
        if !deduped.contains(&value) {
            deduped.push(value);
        }
    }
    deduped
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
    fn scope_memory_ids_trim_dedupe_and_ignore_invalid_values() {
        let ids = selected_scope_conversation_memory_ids(&json!({
            "conversation_memory": [
                " local-thread ",
                "local-thread",
                "",
                1,
                "local-thread:abc"
            ]
        }));

        assert_eq!(ids, vec!["local-thread", "local-thread:abc"]);
        assert!(selected_scope_requests_conversation_memory(&json!({
            "conversation_memory": ["current_thread"]
        })));
        assert!(!selected_scope_requests_conversation_memory(&json!({
            "conversation_memory": []
        })));
    }

    #[test]
    fn set_scope_conversation_memory_normalizes_scope_and_ids() {
        let mut scope = json!("not-object");

        set_selected_scope_conversation_memory(
            &mut scope,
            vec![
                " local-thread ".to_string(),
                "local-thread".to_string(),
                "".to_string(),
                "user-context:user:t:u".to_string(),
            ],
        );

        assert_eq!(
            scope["conversation_memory"],
            json!(["local-thread", "user-context:user:t:u"])
        );
    }

    #[test]
    fn scope_memory_local_thread_ids_resolve_supported_scope_ids() {
        let scope = json!({
            "conversation_memory": [
                "local-thread",
                "current_thread",
                "local-thread: explicit-thread ",
                "user-context:user:t:u",
                "external-user:tenant:bot:user",
                "unsupported",
                "local-thread:"
            ]
        });

        assert_eq!(
            selected_scope_conversation_memory_local_thread_ids(
                &scope,
                Some(" current-thread-id ")
            ),
            vec![
                "current-thread-id",
                "explicit-thread",
                "user-context:user:t:u",
                "external-user:tenant:bot:user"
            ]
        );
    }

    #[test]
    fn scope_candidate_and_global_user_key_keep_public_shape() {
        let candidate = conversation_memory_scope_candidate_value(
            "local-thread",
            "当前对话记忆",
            "用户要求继续刚才内容",
            "current_thread",
        );

        assert_eq!(candidate["type"], json!("conversation_memory"));
        assert_eq!(candidate["id"], json!("local-thread"));
        assert_eq!(candidate["confidence"], json!("medium"));
        assert_eq!(candidate["label"], json!("当前对话记忆"));
        assert_eq!(candidate["reason"], json!("用户要求继续刚才内容"));
        assert_eq!(candidate["source"], json!("current_thread"));

        let tenant_id = TenantId::new();
        let user_id = UserId::new();
        assert_eq!(
            global_user_context_memory_key(tenant_id, user_id),
            format!("user-context:user:{tenant_id}:{user_id}")
        );
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
