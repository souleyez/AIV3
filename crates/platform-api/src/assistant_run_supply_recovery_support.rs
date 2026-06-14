use serde_json::{json, Value};

use crate::assistant_run_lexical_query_support::{
    prompt_contains_elder_death_signal, prompt_contains_elder_fall_signal,
    prompt_requests_procedure_or_action,
};
use crate::assistant_run_scope_policy_support::assistant_run_scope_intent;
use crate::assistant_run_scope_selection_support::{
    selected_dataset_ids_from_scope, selected_document_ids_for_evidence_from_scope,
};
use crate::prompt_match_support::prompt_contains_any;

pub(crate) fn assistant_run_recovery_followup_for_weak_supply(
    selected_scope: &Value,
    prompt: &str,
    status: &str,
    supplied_items: &[Value],
    supply_quality: &Value,
    unavailable_dataset_ids: &[String],
) -> Option<Value> {
    let supply_requested = supply_quality
        .get("supplyRequested")
        .and_then(Value::as_bool)
        .unwrap_or(status != "not_requested");
    if !supply_requested {
        return None;
    }

    let quality_status = supply_quality
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let answerable_supply_count = [
        "indexedEvidenceCount",
        "fallbackChunkCount",
        "datasetEntityScanCount",
        "datasetFactSnapshotCount",
        "spreadsheetRowAnalysisCount",
        "mediaContextCount",
        "conversationMemoryItemCount",
    ]
    .iter()
    .filter_map(|key| supply_quality.get(*key).and_then(Value::as_u64))
    .sum::<u64>();
    let has_weak_expansion =
        assistant_run_supply_has_weak_indexed_evidence_expansion(supplied_items, supply_quality);
    let has_low_text_evidence = supply_quality
        .get("lowTextEvidenceCount")
        .and_then(Value::as_u64)
        .map(|count| count > 0)
        .unwrap_or_else(|| {
            assistant_run_supply_quality_has_note(supply_quality, "low_text_document_evidence")
        });
    let has_parse_blocker = [
        "documentNotReadyCount",
        "documentFailedCount",
        "documentReparsingCount",
        "documentDegradedParseCount",
    ]
    .iter()
    .any(|key| {
        supply_quality
            .get(*key)
            .and_then(Value::as_u64)
            .map(|count| count > 0)
            .unwrap_or(false)
    });
    let procedure_question = prompt_requests_procedure_or_action(prompt);
    let no_answerable_supply = status == "empty"
        || quality_status == "missing"
        || (answerable_supply_count == 0 && (supplied_items.is_empty() || has_parse_blocker));
    let weak_but_answerable = answerable_supply_count > 0
        && (has_weak_expansion
            || has_low_text_evidence
            || has_parse_blocker
            || (procedure_question && quality_status == "partial"));
    if !no_answerable_supply && !weak_but_answerable {
        return None;
    }

    let selected_dataset_count = supply_quality
        .get("selectedDatasetCount")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| selected_dataset_ids_from_scope(selected_scope).len() as u64);
    let selected_document_count =
        selected_document_ids_for_evidence_from_scope(selected_scope).len();
    let (trigger, status, question, model_rule) = if no_answerable_supply {
        (
            "no_answerable_supply",
            "needs_user_clarification",
            "当前可见资料里还没定位到可引用内容。请补充制度名称、章节/页码、关键词，或确认本轮要搜索的文档范围；收到补充后，我会沿用本会话继续扩大检索并完成回答。",
            "If the answer cannot be verified from supplied evidence, state the visible status briefly, ask this exact follow-up, and make it clear the same conversation can continue with expanded retrieval.",
        )
    } else if has_low_text_evidence || has_parse_blocker {
        (
            "parse_or_low_text_quality",
            "answer_with_current_evidence_then_followup_if_needed",
            "当前资料存在解析质量或就绪状态风险。请补充清晰文件、制度名称、章节/页码或关键词；我会沿用本会话继续重检索/细读并修正答案。",
            "Give a useful answer from current evidence and general professional knowledge first; clearly mark unverified parts, then ask this follow-up if exact source wording is still needed.",
        )
    } else if assistant_run_prompt_contains_elder_death_followup_signal(prompt) {
        (
            "weak_procedure_evidence_expansion",
            "answer_with_current_evidence_then_followup_if_needed",
            "我可以先按已检索到的突发事件/善后相关章节整理现场处置、家属沟通、上报记录和后续复盘；如需核对原文，请补充制度名称、章节/页码或关键词。",
            "Use the expanded chunks before saying the document lacks a direct section; organize the answer as a procedure, then ask this follow-up only for exact verification.",
        )
    } else if prompt_contains_elder_fall_signal(prompt) {
        (
            "weak_procedure_evidence_expansion",
            "answer_with_current_evidence_then_followup_if_needed",
            "我可以先按已检索到的防跌倒/突发事件相关章节整理现场处置、家属沟通、上报记录和后续复盘；如需核对原文，请补充制度名称、章节/页码或关键词。",
            "Use the expanded chunks before saying the document lacks a direct section; organize the answer as a procedure, then ask this follow-up only for exact verification.",
        )
    } else if procedure_question {
        (
            "weak_procedure_evidence_expansion",
            "answer_with_current_evidence_then_followup_if_needed",
            "我可以先按已检索到的相关章节整理可执行流程；如需核对原文，请补充制度名称、章节/页码或关键词。",
            "Use the expanded chunks before saying the document lacks a direct process; give a procedure-style answer, then ask this follow-up only for exact verification.",
        )
    } else {
        (
            "weak_document_evidence",
            "answer_with_current_evidence_then_followup_if_needed",
            "我可以先按已检索到的资料给出当前结论；如需更准确，请补充文档名称、页码、关键词或统计口径。",
            "Answer with current evidence first; if the requested fact cannot be verified, ask this follow-up instead of ending with a generic unavailable message.",
        )
    };

    Some(json!({
        "status": status,
        "trigger": trigger,
        "question": question,
        "model_rule": model_rule,
        "can_continue_same_conversation": true,
        "next_action": "continue_same_conversation_expanded_retrieval",
        "suggested_user_inputs": [
            "制度名称",
            "章节或页码",
            "关键词",
            "文档范围或数据集分组",
            "统计口径或对象"
        ],
        "prompt_kind": if procedure_question { "procedure_or_action" } else { "data_or_document_question" },
        "scope_summary": {
            "intent": assistant_run_scope_intent(selected_scope),
            "selected_dataset_count": selected_dataset_count,
            "selected_document_count": selected_document_count,
            "unavailable_dataset_count": unavailable_dataset_ids.len(),
            "unavailable_dataset_ids": unavailable_dataset_ids,
        },
        "answerable_supply_count": answerable_supply_count,
        "quality_status": quality_status,
    }))
}

fn assistant_run_supply_has_weak_indexed_evidence_expansion(
    supplied_items: &[Value],
    supply_quality: &Value,
) -> bool {
    supplied_items.iter().any(|item| {
        item.get("source").and_then(Value::as_str) == Some("document_chunk_fallback")
            && item.get("fallback_reason").and_then(Value::as_str)
                == Some("weak_indexed_evidence_expansion")
    }) || assistant_run_supply_quality_has_note(
        supply_quality,
        "supply_selection:weak_indexed_evidence_expanded_with_visible_chunks",
    )
}

fn assistant_run_supply_quality_has_note(supply_quality: &Value, expected: &str) -> bool {
    supply_quality
        .get("notes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|note| note.as_str() == Some(expected))
}

fn assistant_run_prompt_contains_elder_death_followup_signal(prompt: &str) -> bool {
    prompt_contains_elder_death_signal(prompt)
        || prompt_contains_any(
            prompt,
            &[
                "在院离世",
                "院内离世",
                "长者离世",
                "老人离世",
                "离世善后",
                "身故善后",
                "善后流程",
            ],
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_followup_skips_when_supply_was_not_requested() {
        let followup = assistant_run_recovery_followup_for_weak_supply(
            &json!({"mode": "all_visible"}),
            "普通寒暄",
            "not_requested",
            &[],
            &json!({"supplyRequested": false, "status": "missing"}),
            &[],
        );

        assert!(followup.is_none());
    }

    #[test]
    fn recovery_followup_requests_clarification_for_empty_visible_supply() {
        let selected_scope = json!({
            "mode": "user_selected",
            "datasets": ["00000000-0000-0000-0000-000000000001"],
            "documents": ["00000000-0000-0000-0000-000000000002"]
        });
        let followup = assistant_run_recovery_followup_for_weak_supply(
            &selected_scope,
            "护理交接班流程",
            "empty",
            &[],
            &json!({"supplyRequested": true, "status": "missing"}),
            &["dataset-unavailable".to_string()],
        )
        .expect("empty requested supply should produce a recovery follow-up");

        assert_eq!(followup["trigger"], json!("no_answerable_supply"));
        assert_eq!(followup["status"], json!("needs_user_clarification"));
        assert_eq!(
            followup["scope_summary"]["selected_dataset_count"],
            json!(1)
        );
        assert_eq!(
            followup["scope_summary"]["selected_document_count"],
            json!(1)
        );
        assert_eq!(
            followup["scope_summary"]["unavailable_dataset_ids"],
            json!(["dataset-unavailable"])
        );
    }

    #[test]
    fn recovery_followup_prefers_answer_first_for_low_text_answerable_supply() {
        let supplied_items = vec![json!({
            "type": "retrieval_evidence",
            "content_excerpt": "发药核对"
        })];
        let followup = assistant_run_recovery_followup_for_weak_supply(
            &json!({"mode": "user_selected"}),
            "给老人发药时需要哪些核对步骤",
            "supplied",
            &supplied_items,
            &json!({
                "supplyRequested": true,
                "status": "partial",
                "indexedEvidenceCount": 1,
                "notes": ["low_text_document_evidence"]
            }),
            &[],
        )
        .expect("low-text but answerable supply should guide answer-first recovery");

        assert_eq!(followup["trigger"], json!("parse_or_low_text_quality"));
        assert_eq!(
            followup["status"],
            json!("answer_with_current_evidence_then_followup_if_needed")
        );
        assert_eq!(followup["answerable_supply_count"], json!(1));
    }
}
