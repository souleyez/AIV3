use contracts::{
    AssistantRunCodexContextBudgetItemView, AssistantRunCodexContextBudgetView,
    AssistantRunMessageView,
};
#[cfg(test)]
use domain_model::ChatMessageRole;
use serde_json::Value;

use crate::assistant_run_codex_tool_output_support::{
    assistant_run_codex_value_chars, assistant_run_codex_values_chars,
};
use crate::assistant_run_evidence_state_support::assistant_run_evidence_supplied_count;
use crate::assistant_run_scope_selection_support::{
    selected_dataset_ids_from_scope, selected_document_ids_from_scope,
};

pub(crate) fn assistant_run_codex_hidden_memory_candidates(evidence_state: &Value) -> Vec<Value> {
    evidence_state
        .get("conversation_memory_items")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

pub(crate) fn assistant_run_codex_context_budget(
    user_prompt: &str,
    messages: &[AssistantRunMessageView],
    startup_briefing: &Value,
    selected_scope: &Value,
    scope_candidates: &[Value],
    evidence_state: &Value,
    current_artifact: Option<&Value>,
) -> AssistantRunCodexContextBudgetView {
    let max_prompt_chars = std::env::var("ASSISTANT_RUN_CODEX_MAX_PROMPT_CHARS")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok());
    let category_soft_limit = std::env::var("ASSISTANT_RUN_CODEX_CATEGORY_SOFT_LIMIT_CHARS")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .or_else(|| max_prompt_chars.map(|value| value.saturating_div(4).max(1)))
        .or(Some(12_000));
    let supplied_items = evidence_state
        .get("supplied_items")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let hidden_memory_items = assistant_run_codex_hidden_memory_candidates(evidence_state);
    let detail_targets = evidence_state
        .get("detail_targets")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let artifact_state_chars = current_artifact
        .and_then(|artifact| serde_json::to_string(artifact).ok())
        .map(|value| value.chars().count())
        .unwrap_or(0);
    let tool_output_chars = serde_json::to_string(evidence_state)
        .map(|value| value.chars().count())
        .unwrap_or(0);
    let trimmed_item_count = evidence_state
        .get("trimmed_item_count")
        .or_else(|| evidence_state.get("trimmedItemCount"))
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or(0);
    let prompt_and_history_chars = user_prompt.chars().count()
        + messages
            .iter()
            .map(|message| message.content.chars().count())
            .sum::<usize>();
    let startup_briefing_chars = assistant_run_codex_value_chars(startup_briefing);
    let selected_scope_chars = assistant_run_codex_value_chars(selected_scope);
    let scope_candidate_chars = assistant_run_codex_values_chars(scope_candidates);
    let supplied_item_chars = assistant_run_codex_values_chars(&supplied_items);
    let hidden_memory_chars = assistant_run_codex_values_chars(&hidden_memory_items);
    let detail_target_chars = assistant_run_codex_values_chars(&detail_targets);
    let media_summary_count = assistant_run_codex_media_summary_count(&supplied_items);
    let media_summary_chars = assistant_run_codex_media_summary_chars(&supplied_items);
    let estimated_prompt_chars = prompt_and_history_chars
        + startup_briefing_chars
        + selected_scope_chars
        + scope_candidate_chars
        + tool_output_chars
        + artifact_state_chars;
    let budget_pressure =
        assistant_run_codex_budget_pressure(estimated_prompt_chars, max_prompt_chars);
    let items = vec![
        AssistantRunCodexContextBudgetItemView::new(
            "prompt_and_history",
            messages.len() + 1,
            prompt_and_history_chars,
            category_soft_limit,
            0,
            Some("user prompt plus recent visible conversation messages".to_string()),
        ),
        AssistantRunCodexContextBudgetItemView::new(
            "startup_briefing",
            usize::from(!startup_briefing.is_null()),
            startup_briefing_chars,
            category_soft_limit,
            0,
            Some("product and database briefing supplied by DataMax".to_string()),
        ),
        AssistantRunCodexContextBudgetItemView::new(
            "selected_scope",
            selected_dataset_ids_from_scope(selected_scope).len()
                + selected_document_ids_from_scope(selected_scope).len(),
            selected_scope_chars,
            category_soft_limit,
            0,
            Some("selected datasets, documents, memory flags, and supply policy".to_string()),
        ),
        AssistantRunCodexContextBudgetItemView::new(
            "inferred_scope_candidates",
            scope_candidates.len(),
            scope_candidate_chars,
            category_soft_limit,
            0,
            Some("visible scope candidates for model awareness, not direct evidence".to_string()),
        ),
        AssistantRunCodexContextBudgetItemView::new(
            "retrieval_evidence",
            supplied_items.len(),
            supplied_item_chars,
            category_soft_limit,
            trimmed_item_count,
            Some(
                "DataMax-visible supplied evidence; high-value ids and citations must survive trimming"
                    .to_string(),
            ),
        ),
        AssistantRunCodexContextBudgetItemView::new(
            "hidden_conversation_memory",
            hidden_memory_items.len(),
            hidden_memory_chars,
            category_soft_limit,
            0,
            Some("intent-gated conversation memory candidates".to_string()),
        ),
        AssistantRunCodexContextBudgetItemView::new(
            "detail_targets",
            detail_targets.len(),
            detail_target_chars,
            category_soft_limit,
            0,
            Some(
                "documents recommended for deeper read; not directly citable evidence".to_string(),
            ),
        ),
        AssistantRunCodexContextBudgetItemView::new(
            "media_payload_summaries",
            media_summary_count,
            media_summary_chars,
            category_soft_limit,
            0,
            Some("transcript, scene, keyframe OCR, and media evidence summaries".to_string()),
        ),
        AssistantRunCodexContextBudgetItemView::new(
            "current_artifact",
            usize::from(current_artifact.is_some()),
            artifact_state_chars,
            category_soft_limit,
            0,
            Some("current static page/report artifact skeleton supplied by DataMax".to_string()),
        ),
        AssistantRunCodexContextBudgetItemView::new(
            "tool_outputs",
            usize::from(!evidence_state.is_null()),
            tool_output_chars,
            max_prompt_chars.or(category_soft_limit),
            trimmed_item_count,
            Some("complete evidence/tool state currently supplied to the executor".to_string()),
        ),
    ];

    AssistantRunCodexContextBudgetView {
        max_prompt_chars,
        estimated_prompt_chars,
        budget_pressure,
        included_message_count: messages.len(),
        selected_dataset_count: selected_dataset_ids_from_scope(selected_scope).len(),
        evidence_item_count: assistant_run_evidence_supplied_count(evidence_state),
        hidden_memory_item_count: hidden_memory_items.len(),
        artifact_state_chars,
        tool_output_chars,
        trimmed_item_count,
        items,
        ..AssistantRunCodexContextBudgetView::default()
    }
}

fn assistant_run_codex_budget_pressure(char_count: usize, limit: Option<usize>) -> String {
    match limit {
        Some(limit) if limit > 0 && char_count > limit => "over_limit",
        Some(limit) if limit > 0 && char_count * 10 >= limit * 7 => "attention",
        Some(_) => "ok",
        None => "unbounded",
    }
    .to_string()
}

fn assistant_run_codex_media_summary_count(items: &[Value]) -> usize {
    items
        .iter()
        .filter(|item| {
            item.get("media_context")
                .or_else(|| item.get("mediaContext"))
                .is_some()
        })
        .count()
}

fn assistant_run_codex_media_summary_chars(items: &[Value]) -> usize {
    items
        .iter()
        .filter_map(|item| {
            item.get("media_context")
                .or_else(|| item.get("mediaContext"))
        })
        .map(assistant_run_codex_value_chars)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn hidden_memory_candidates_reads_only_array_items() {
        assert_eq!(
            assistant_run_codex_hidden_memory_candidates(&json!({
                "conversation_memory_items": [{"id": "m1"}, {"id": "m2"}]
            })),
            vec![json!({"id": "m1"}), json!({"id": "m2"})]
        );
        assert!(assistant_run_codex_hidden_memory_candidates(&json!({
            "conversation_memory_items": "none"
        }))
        .is_empty());
        assert!(assistant_run_codex_hidden_memory_candidates(&json!({})).is_empty());
    }

    #[test]
    fn budget_pressure_labels_unbounded_ok_attention_and_over_limit() {
        assert_eq!(
            assistant_run_codex_budget_pressure(12_000, None),
            "unbounded"
        );
        assert_eq!(assistant_run_codex_budget_pressure(6, Some(10)), "ok");
        assert_eq!(
            assistant_run_codex_budget_pressure(7, Some(10)),
            "attention"
        );
        assert_eq!(
            assistant_run_codex_budget_pressure(11, Some(10)),
            "over_limit"
        );
        assert_eq!(assistant_run_codex_budget_pressure(11, Some(0)), "ok");
    }

    #[test]
    fn media_summary_helpers_read_snake_and_camel_case_payloads() {
        let items = vec![
            json!({"media_context": {"transcript": "第一段"}}),
            json!({"mediaContext": {"ocr": "第二段"}}),
            json!({"text": "not media"}),
        ];

        assert_eq!(assistant_run_codex_media_summary_count(&items), 2);
        assert!(assistant_run_codex_media_summary_chars(&items) > 0);
    }

    #[test]
    fn context_budget_summarizes_scope_evidence_and_artifact_counts() {
        let dataset_id = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
        let document_id = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
        let messages = vec![AssistantRunMessageView {
            role: ChatMessageRole::User,
            content: "上一轮问题".to_string(),
        }];
        let selected_scope = json!({
            "datasets": [{"id": dataset_id}],
            "documents": [{"type": "document", "id": document_id}]
        });
        let evidence_state = json!({
            "supplied_items": [
                {"id": "e1", "text": "证据", "mediaContext": {"ocr": "画面文字"}}
            ],
            "conversation_memory_items": [{"id": "m1"}],
            "detail_targets": [{"document_id": document_id}],
            "trimmedItemCount": 3
        });
        let budget = assistant_run_codex_context_budget(
            "本轮问题",
            &messages,
            &json!({"product": "DataMax"}),
            &selected_scope,
            &[json!({"type": "dataset", "id": dataset_id})],
            &evidence_state,
            Some(&json!({"artifact": "current"})),
        );

        assert_eq!(budget.included_message_count, 1);
        assert_eq!(budget.selected_dataset_count, 1);
        assert_eq!(budget.evidence_item_count, 1);
        assert_eq!(budget.hidden_memory_item_count, 1);
        assert_eq!(budget.trimmed_item_count, 3);
        assert!(budget.artifact_state_chars > 0);
        assert!(budget.tool_output_chars > 0);
        assert!(budget.estimated_prompt_chars > 0);
        assert_eq!(budget.budget_pressure, "unbounded");
        assert!(budget
            .items
            .iter()
            .any(|item| { item.category == "selected_scope" && item.item_count == 2 }));
        assert!(budget
            .items
            .iter()
            .any(|item| { item.category == "media_payload_summaries" && item.item_count == 1 }));
        assert!(budget
            .items
            .iter()
            .any(|item| { item.category == "retrieval_evidence" && item.trimmed_item_count == 3 }));
    }
}
