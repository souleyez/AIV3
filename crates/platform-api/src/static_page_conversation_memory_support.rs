use domain_model::AssistantRun;
use serde_json::Value;

use crate::value_array;

pub(crate) fn static_page_conversation_memory_refs(run: &AssistantRun) -> Vec<Value> {
    value_array(
        run.evidence_state
            .get("supplied_items")
            .cloned()
            .unwrap_or(Value::Null),
    )
    .into_iter()
    .filter(|item| item.get("type").and_then(Value::as_str) == Some("conversation_memory_item"))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{AssistantRunId, TenantId};
    use serde_json::json;

    fn run_with_evidence_state(evidence_state: Value) -> AssistantRun {
        let now = Utc::now();
        AssistantRun {
            id: AssistantRunId::new(),
            tenant_id: TenantId::new(),
            user_id: None,
            local_thread_id: Some("static-page-memory-test".to_string()),
            user_prompt: "继续修改静态页".to_string(),
            startup_briefing: json!({}),
            selected_scope: json!({}),
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

    #[test]
    fn conversation_memory_refs_keep_only_conversation_memory_items_in_order() {
        let run = run_with_evidence_state(json!({
            "supplied_items": [
                {"type": "retrieval_evidence", "content": "文档供料"},
                {"type": "conversation_memory_item", "id": "mem-1", "content": "上一轮强调暗色风格"},
                {"type": "conversation_memory_item", "id": "mem-2", "content": "客户关注取高机会"},
                {"type": "dataset_fact", "content": "事实供料"}
            ]
        }));

        let refs = static_page_conversation_memory_refs(&run);

        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0]["id"], json!("mem-1"));
        assert_eq!(refs[1]["id"], json!("mem-2"));
    }

    #[test]
    fn conversation_memory_refs_ignore_missing_or_non_array_supplied_items() {
        assert!(
            static_page_conversation_memory_refs(&run_with_evidence_state(json!({}))).is_empty()
        );
        assert!(
            static_page_conversation_memory_refs(&run_with_evidence_state(json!({
                "supplied_items": {"type": "conversation_memory_item"}
            })))
            .is_empty()
        );
    }

    #[test]
    fn conversation_memory_refs_require_exact_type() {
        let run = run_with_evidence_state(json!({
            "supplied_items": [
                {"type": " conversation_memory_item ", "id": "trimmed"},
                {"type": "conversation_memory", "id": "alias"},
                {"type": "conversation_memory_item", "id": "exact"}
            ]
        }));

        let refs = static_page_conversation_memory_refs(&run);

        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0]["id"], json!("exact"));
    }
}
