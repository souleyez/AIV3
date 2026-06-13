use contracts::{ConversationMemoryItemView, WorkflowEventView};
use domain_model::{ConversationMemoryItem, WorkflowEventRecord};

pub(crate) fn to_conversation_memory_item_view(
    item: ConversationMemoryItem,
) -> ConversationMemoryItemView {
    ConversationMemoryItemView {
        id: item.id,
        local_thread_id: item.local_thread_id,
        role: item.role,
        item_kind: item.item_kind,
        summary: item.summary,
        source_message_refs: item.source_message_refs,
        artifact_refs: item.artifact_refs,
        metadata: item.metadata,
        created_at: item.created_at,
        updated_at: item.updated_at,
    }
}

pub(crate) fn to_workflow_event_view(event: WorkflowEventRecord) -> WorkflowEventView {
    WorkflowEventView {
        id: event.id,
        sequence_no: event.sequence_no,
        event_name: event.event_name,
        payload: event.payload,
        created_at: event.created_at,
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use domain_model::{
        ChatMessageRole, ConversationMemoryItemId, TenantId, UserId, WorkflowEventId,
        WorkflowExecutionId,
    };
    use serde_json::json;

    use super::*;

    fn fixed_time() -> DateTime<Utc> {
        "2026-06-14T00:00:00Z"
            .parse()
            .expect("fixed timestamp should parse")
    }

    #[test]
    fn conversation_memory_item_view_maps_customer_visible_fields_only() {
        let now = fixed_time();
        let item = ConversationMemoryItem {
            id: ConversationMemoryItemId::new(),
            tenant_id: TenantId::new(),
            user_id: Some(UserId::new()),
            local_thread_id: "thread-1".to_string(),
            role: ChatMessageRole::Assistant,
            item_kind: "summary".to_string(),
            summary: "stable memory summary".to_string(),
            source_message_refs: json!(["m1"]),
            artifact_refs: json!([{"id": "artifact-1"}]),
            metadata: json!({"scope": "conversation"}),
            created_at: now,
            updated_at: now,
        };
        let id = item.id;

        let view = to_conversation_memory_item_view(item);

        assert_eq!(view.id, id);
        assert_eq!(view.local_thread_id, "thread-1");
        assert_eq!(view.role, ChatMessageRole::Assistant);
        assert_eq!(view.item_kind, "summary");
        assert_eq!(view.summary, "stable memory summary");
        assert_eq!(view.source_message_refs, json!(["m1"]));
        assert_eq!(view.artifact_refs, json!([{"id": "artifact-1"}]));
        assert_eq!(view.metadata, json!({"scope": "conversation"}));
        assert_eq!(view.created_at, now);
        assert_eq!(view.updated_at, now);
    }

    #[test]
    fn workflow_event_view_preserves_sequence_payload_and_timestamp() {
        let now = fixed_time();
        let event = WorkflowEventRecord {
            id: WorkflowEventId::new(),
            execution_id: WorkflowExecutionId::new(),
            sequence_no: 42,
            event_name: "workflow.completed".to_string(),
            payload: json!({"status": "succeeded"}),
            created_at: now,
        };
        let id = event.id;

        let view = to_workflow_event_view(event);

        assert_eq!(view.id, id);
        assert_eq!(view.sequence_no, 42);
        assert_eq!(view.event_name, "workflow.completed");
        assert_eq!(view.payload, json!({"status": "succeeded"}));
        assert_eq!(view.created_at, now);
    }
}
