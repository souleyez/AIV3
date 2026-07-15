use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    assistant_run_model_supply_item_for_context, ASSISTANT_RUN_MODEL_CONTEXT_ASSET_PROFILE_LIMIT,
    ASSISTANT_RUN_MODEL_CONTEXT_DATABASE_LIMIT, ASSISTANT_RUN_MODEL_CONTEXT_MEMORY_LIMIT,
    ASSISTANT_RUN_MODEL_CONTEXT_OTHER_LIMIT, ASSISTANT_RUN_MODEL_CONTEXT_PARSE_STATUS_LIMIT,
    ASSISTANT_RUN_MODEL_CONTEXT_RETRIEVAL_LIMIT, ASSISTANT_RUN_MODEL_CONTEXT_SPREADSHEET_LIMIT,
    ASSISTANT_RUN_MODEL_CONTEXT_STRUCTURED_FACT_LIMIT,
    ASSISTANT_RUN_MODEL_CONTEXT_SUPPLIED_ITEM_LIMIT,
};

type AssistantRunModelBudgetedSupplyItems = (Vec<Value>, Value);

pub(crate) fn assistant_run_model_budgeted_supply_items(
    items: &[Value],
) -> AssistantRunModelBudgetedSupplyItems {
    let bucket_order = [
        "document_status",
        "structured_fact",
        "spreadsheet",
        "retrieval",
        "database",
        "memory",
        "asset_profile",
        "other",
    ];
    let mut selected_indices = BTreeSet::new();
    let mut included_by_type = BTreeMap::<String, usize>::new();
    let mut omitted_by_type = BTreeMap::<String, usize>::new();

    for bucket in bucket_order {
        let mut included_in_bucket = 0usize;
        let bucket_limit = assistant_run_model_supply_bucket_limit(bucket);
        for (index, item) in items.iter().enumerate() {
            if assistant_run_model_supply_bucket(item) != bucket {
                continue;
            }
            let type_label = assistant_run_model_supply_type_label(item);
            if selected_indices.len() < ASSISTANT_RUN_MODEL_CONTEXT_SUPPLIED_ITEM_LIMIT
                && included_in_bucket < bucket_limit
            {
                selected_indices.insert(index);
                included_in_bucket += 1;
                *included_by_type.entry(type_label).or_insert(0) += 1;
            } else {
                *omitted_by_type.entry(type_label).or_insert(0) += 1;
            }
        }
    }

    let compacted_items = selected_indices
        .iter()
        .filter_map(|index| items.get(*index))
        .map(assistant_run_model_supply_item_for_context)
        .collect::<Vec<_>>();
    let asset_profile_evidence_ref_count = compacted_items
        .iter()
        .filter(|item| {
            item.get("type").and_then(Value::as_str) == Some("asset_profile_hint")
                && item
                    .get("evidence_ref")
                    .and_then(Value::as_str)
                    .is_some_and(|value| !value.trim().is_empty())
        })
        .count();
    let budget = json!({
        "policy": "model_input_balanced_supply_buckets",
        "original_supplied_item_count": items.len(),
        "model_supplied_item_count": compacted_items.len(),
        "omitted_supplied_item_count": items.len().saturating_sub(compacted_items.len()),
        "asset_profile_evidence_ref_count": asset_profile_evidence_ref_count,
        "global_item_limit": ASSISTANT_RUN_MODEL_CONTEXT_SUPPLIED_ITEM_LIMIT,
        "bucket_limits": {
            "document_status": ASSISTANT_RUN_MODEL_CONTEXT_PARSE_STATUS_LIMIT,
            "structured_fact": ASSISTANT_RUN_MODEL_CONTEXT_STRUCTURED_FACT_LIMIT,
            "spreadsheet": ASSISTANT_RUN_MODEL_CONTEXT_SPREADSHEET_LIMIT,
            "retrieval": ASSISTANT_RUN_MODEL_CONTEXT_RETRIEVAL_LIMIT,
            "database": ASSISTANT_RUN_MODEL_CONTEXT_DATABASE_LIMIT,
            "memory": ASSISTANT_RUN_MODEL_CONTEXT_MEMORY_LIMIT,
            "asset_profile": ASSISTANT_RUN_MODEL_CONTEXT_ASSET_PROFILE_LIMIT,
            "other": ASSISTANT_RUN_MODEL_CONTEXT_OTHER_LIMIT,
        },
        "included_by_type": included_by_type,
        "omitted_by_type": omitted_by_type,
        "model_rule": "The model input keeps separate quotas for document evidence, database aggregates, structured scans, spreadsheet analyses, parse status, and memory so one source cannot crowd out the others. Omitted items remain available to host tools/detail reads; do not infer that omitted means absent from the dataset.",
    });

    (compacted_items, budget)
}

pub(crate) fn assistant_run_asset_document_supply_gate_dry_run(items: &[Value]) -> Value {
    let (model_items, budget) = assistant_run_model_budgeted_supply_items(items);
    let mut source_kind_counts = BTreeMap::<String, usize>::new();
    let mut citation_label_counts = BTreeMap::<String, usize>::new();
    let mut source_kind_order = Vec::<String>::new();
    let mut approximate_prompt_chars = 0usize;
    let mut document_evidence_count = 0usize;
    let mut asset_profile_signal_count = 0usize;

    for item in &model_items {
        approximate_prompt_chars += serde_json::to_string(item)
            .map(|text| text.chars().count())
            .unwrap_or_default();
        let source_kind = assistant_run_model_supply_source_kind_for_gate(item);
        if !source_kind_order.contains(&source_kind) {
            source_kind_order.push(source_kind.clone());
        }
        *source_kind_counts.entry(source_kind.clone()).or_insert(0) += 1;
        let citation_label = assistant_run_model_supply_citation_label_for_gate(item);
        *citation_label_counts
            .entry(citation_label.to_string())
            .or_insert(0) += 1;
        if citation_label == "document_evidence" {
            document_evidence_count += 1;
        } else if citation_label == "asset_profile_signal" {
            asset_profile_signal_count += 1;
        }
    }

    let approximate_prompt_tokens = approximate_prompt_chars.div_ceil(4);
    let omitted_count = budget
        .get("omitted_supplied_item_count")
        .and_then(Value::as_u64)
        .unwrap_or_default() as usize;
    let has_document_evidence = document_evidence_count > 0;
    let has_asset_profile_signal = asset_profile_signal_count > 0;
    let needs_more_supply =
        !has_document_evidence || (has_asset_profile_signal && omitted_count > 0);
    let gate_status = if has_document_evidence && has_asset_profile_signal {
        "grounded_with_asset_context"
    } else if has_document_evidence {
        "grounded_without_asset_context"
    } else if has_asset_profile_signal {
        "partial_asset_context_only"
    } else {
        "missing_evidence"
    };
    let next_action = if needs_more_supply {
        "continue_expand_supply"
    } else {
        "answer_with_citation_constraints"
    };

    json!({
        "contract": "assistant_run_asset_document_supply_budget_quality_gate_dry_run_v1",
        "mode": "dry_run",
        "no_write": true,
        "budget_policy": {
            "source": "assistant_run_model_budgeted_supply_items",
            "global_item_limit": budget.get("global_item_limit").cloned().unwrap_or(Value::Null),
            "bucket_limits": budget.get("bucket_limits").cloned().unwrap_or(Value::Null),
            "included_by_type": budget.get("included_by_type").cloned().unwrap_or(Value::Null),
            "omitted_by_type": budget.get("omitted_by_type").cloned().unwrap_or(Value::Null),
            "model_supplied_item_count": budget.get("model_supplied_item_count").cloned().unwrap_or(Value::Null),
            "omitted_supplied_item_count": budget.get("omitted_supplied_item_count").cloned().unwrap_or(Value::Null),
            "approx_prompt_chars": approximate_prompt_chars,
            "approx_prompt_tokens": approximate_prompt_tokens,
            "token_estimate_policy": "chars_div_4_ceil_dry_run_only",
        },
        "source_kind_dedupe": {
            "policy": "preserve_first_seen_source_kind_then_count_duplicates",
            "source_kind_order": source_kind_order,
            "source_kind_counts": source_kind_counts,
            "document_chunk_and_asset_profile_keep_separate": true,
        },
        "citation_policy": {
            "labels": citation_label_counts,
            "document_evidence_label": "document_evidence",
            "asset_profile_label": "asset_profile_signal",
            "asset_profile_exact_citation_allowed": false,
            "exact_claims_require_document_database_or_media_evidence": true,
        },
        "quality_gate": {
            "status": gate_status,
            "document_evidence_count": document_evidence_count,
            "asset_profile_signal_count": asset_profile_signal_count,
            "needs_more_supply": needs_more_supply,
            "next_action": next_action,
            "continue_when": [
                "missing_document_evidence_for_exact_claim",
                "only_asset_profile_signal_available",
                "budget_omitted_relevant_evidence_available"
            ],
        },
        "model_guidance": [
            "Use asset_profile_signal only as compact asset understanding, not as an exact citation.",
            "Use document_evidence, database aggregates, or media evidence for exact claims, counts, and quotations.",
            "If quality_gate.next_action is continue_expand_supply, answer with current evidence if possible and keep expanding supply before final high-confidence claims."
        ],
        "production_write_allowed": false,
    })
}

pub(crate) fn assistant_run_supply_progress_events_dry_run(items: &[Value]) -> Value {
    let supply_gate = assistant_run_asset_document_supply_gate_dry_run(items);
    let mut events = Vec::new();
    let model_supplied_item_count = supply_gate
        .pointer("/budget_policy/model_supplied_item_count")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let quality_status = supply_gate
        .pointer("/quality_gate/status")
        .and_then(Value::as_str)
        .unwrap_or("missing_evidence");
    let next_action = supply_gate
        .pointer("/quality_gate/next_action")
        .and_then(Value::as_str)
        .unwrap_or("continue_expand_supply");
    let asset_profile_signal_count = supply_gate
        .pointer("/quality_gate/asset_profile_signal_count")
        .and_then(Value::as_u64)
        .unwrap_or_default();

    if model_supplied_item_count > 0 {
        events.push(assistant_run_supply_progress_event(
            "supply_ready",
            "supply_ready",
            "ready",
            "已供料：当前轮次已有可压缩供料，模型可先回答并继续处理后续动作。",
            true,
        ));
    }

    if asset_profile_signal_count > 0 {
        events.push(assistant_run_supply_progress_event(
            "asset_profile_signal",
            "asset_profile_signal",
            "ready",
            "资产画像已作为理解信号供给模型；它不能作为精确文档引用，精确结论仍需文档、数据库或媒体证据支撑。",
            true,
        ));
    }

    if next_action == "continue_expand_supply" {
        events.push(assistant_run_supply_progress_event(
            "supply_expanding",
            "supply_expanding",
            quality_status,
            "供料不足时不中断回答：先基于现有证据给出一轮答复，同时继续扩展文档、数据库或媒体证据。",
            true,
        ));
    }

    if assistant_run_supply_items_need_parse_wait_or_retry(items) {
        events.push(assistant_run_supply_progress_event(
            "parse_waiting_or_retry",
            "parse_waiting_or_retry",
            "processing",
            "部分资产或文档仍在解析、失败重试或降级处理中；该状态会作为可见进度供给模型和任务卡，不阻断当前回答。",
            true,
        ));
    }

    json!({
        "contract": "assistant_run_supply_progress_events_dry_run_v1",
        "mode": "dry_run",
        "no_write": true,
        "non_blocking": true,
        "consumer_targets": ["main_site_task_card", "third_party_stream"],
        "source_gate_contract": supply_gate.get("contract").cloned().unwrap_or(Value::Null),
        "quality_gate_status": quality_status,
        "quality_gate_next_action": next_action,
        "events": events,
        "continuation_policy": {
            "answer_with_current_evidence_first": true,
            "continue_actions_after_reply": true,
            "final_failure_without_answer": false,
            "parse_or_supply_gap_is_not_terminal": true,
        },
        "redaction": {
            "raw_locator_excluded": true,
            "provider_payload_excluded": true,
            "auth_material_excluded": true,
            "local_path_excluded": true,
            "raw_source_body_excluded": true,
        },
        "production_write_allowed": false,
    })
}

pub(crate) fn assistant_run_supply_progress_contract_drift_guard_dry_run(
    progress_events: &Value,
) -> Value {
    let event_types = assistant_run_supply_progress_event_types(progress_events);
    let all_events_non_blocking = progress_events
        .get("events")
        .and_then(Value::as_array)
        .map(|events| {
            events.iter().all(|event| {
                event.get("blocking").and_then(Value::as_bool) == Some(false)
                    && event
                        .get("sensitive_payload_included")
                        .and_then(Value::as_bool)
                        == Some(false)
            })
        })
        .unwrap_or(false);

    json!({
        "contract": "assistant_run_supply_progress_contract_drift_guard_dry_run_v1",
        "mode": "dry_run",
        "no_write": true,
        "source_contract": progress_events.get("contract").cloned().unwrap_or(Value::Null),
        "source_event_types": event_types,
        "external_sse_guard": {
            "schema": "v3.external_channel.sse.v1",
            "public_envelope_fields": [
                "schema",
                "event_id",
                "sequence",
                "assistant_run_id",
                "idempotency_key",
                "conversation_external_id",
                "phase",
                "status",
                "display_text",
                "status_url",
                "poll_after_seconds",
                "data"
            ],
            "public_stream_field_mutation_allowed": false,
            "new_progress_events_emit_live_sse": false,
            "new_progress_events_are_internal_dry_run_only": true,
        },
        "third_party_guard": {
            "callback_triggered": false,
            "public_request_or_response_field_added": false,
            "requires_new_third_party_contract": false,
            "third_party_visible_progress_is_summary_only": true,
        },
        "main_site_task_card_guard": {
            "expected_consumer": "artifact_task_cards",
            "allowed_surface": "detail_progress_summary",
            "creates_task_card_without_user_action": false,
            "mutates_task_card_status_enum": false,
            "task_card_detail_only": true,
        },
        "answer_liveness_guard": {
            "failure_status_blocks_answer": false,
            "parse_or_supply_gap_is_terminal": false,
            "answer_with_current_evidence_first": progress_events
                .pointer("/continuation_policy/answer_with_current_evidence_first")
                .cloned()
                .unwrap_or(Value::Null),
            "continue_actions_after_reply": progress_events
                .pointer("/continuation_policy/continue_actions_after_reply")
                .cloned()
                .unwrap_or(Value::Null),
            "final_failure_without_answer": progress_events
                .pointer("/continuation_policy/final_failure_without_answer")
                .cloned()
                .unwrap_or(Value::Null),
        },
        "redaction_guard": {
            "all_events_non_blocking": all_events_non_blocking,
            "raw_locator_excluded": progress_events
                .pointer("/redaction/raw_locator_excluded")
                .cloned()
                .unwrap_or(Value::Bool(true)),
            "provider_payload_excluded": progress_events
                .pointer("/redaction/provider_payload_excluded")
                .cloned()
                .unwrap_or(Value::Bool(true)),
            "auth_material_excluded": progress_events
                .pointer("/redaction/auth_material_excluded")
                .cloned()
                .unwrap_or(Value::Bool(true)),
        },
        "production_write_allowed": false,
    })
}

fn assistant_run_supply_progress_event(
    event_type: &str,
    phase: &str,
    status: &str,
    message: &str,
    model_visible: bool,
) -> Value {
    json!({
        "event_type": event_type,
        "phase": phase,
        "status": status,
        "message": message,
        "blocking": false,
        "model_visible": model_visible,
        "third_party_visible": true,
        "task_card_visible": true,
        "sensitive_payload_included": false,
    })
}

fn assistant_run_supply_progress_event_types(progress_events: &Value) -> Value {
    Value::Array(
        progress_events
            .get("events")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|event| event.get("event_type").and_then(Value::as_str))
            .map(|event_type| json!(event_type))
            .collect::<Vec<_>>(),
    )
}

fn assistant_run_supply_items_need_parse_wait_or_retry(items: &[Value]) -> bool {
    items.iter().any(|item| {
        matches!(
            item.get("type").and_then(Value::as_str),
            Some("asset_parse_status" | "document_parse_status")
        ) && (assistant_run_supply_numeric_field(
            item,
            &[
                "not_ready_asset_count",
                "notReadyAssetCount",
                "failed_asset_count",
                "failedAssetCount",
                "retrying_asset_count",
                "retryingAssetCount",
                "pending_document_count",
                "pendingDocumentCount",
                "failed_document_count",
                "failedDocumentCount",
                "retrying_document_count",
                "retryingDocumentCount",
                "degraded_document_count",
                "degradedDocumentCount",
            ],
        ) > 0
            || assistant_run_supply_status_counts_include_wait_or_retry(item))
    })
}

fn assistant_run_supply_numeric_field(item: &Value, names: &[&str]) -> u64 {
    names
        .iter()
        .filter_map(|name| item.get(*name))
        .filter_map(Value::as_u64)
        .sum()
}

fn assistant_run_supply_status_counts_include_wait_or_retry(item: &Value) -> bool {
    item.get("status_counts")
        .or_else(|| item.get("statusCounts"))
        .and_then(Value::as_object)
        .is_some_and(|counts| {
            counts.iter().any(|(status, count)| {
                matches!(
                    status.as_str(),
                    "pending" | "parsing" | "retrying" | "failed" | "degraded"
                ) && count.as_u64().unwrap_or_default() > 0
            })
        })
}

fn assistant_run_model_supply_bucket(item: &Value) -> &'static str {
    match item.get("type").and_then(Value::as_str) {
        Some("document_parse_status") => "document_status",
        Some("dataset_entity_scan" | "dataset_fact_snapshot") => "structured_fact",
        Some("spreadsheet_row_analysis") => "spreadsheet",
        Some("database_schema_context" | "database_aggregate" | "database_aggregate_error") => {
            "database"
        }
        Some("conversation_memory_item") => "memory",
        Some("asset_profile_hint" | "asset_parse_status") => "asset_profile",
        Some("retrieval_evidence" | "search_evidence") => "retrieval",
        _ => "other",
    }
}

fn assistant_run_model_supply_source_kind_for_gate(item: &Value) -> String {
    if item.get("type").and_then(Value::as_str) == Some("asset_profile_hint") {
        return "asset_profile".to_string();
    }
    item.get("source_kind")
        .or_else(|| item.get("sourceKind"))
        .or_else(|| item.get("source"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            if value == "document_chunk_fallback" {
                "document_chunk".to_string()
            } else {
                value.to_string()
            }
        })
        .unwrap_or_else(|| {
            item.get("type")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string()
        })
}

fn assistant_run_model_supply_citation_label_for_gate(item: &Value) -> &'static str {
    match item.get("type").and_then(Value::as_str) {
        Some("asset_profile_hint") => "asset_profile_signal",
        Some("retrieval_evidence" | "search_evidence") => "document_evidence",
        Some("database_aggregate" | "dataset_fact_snapshot" | "dataset_entity_scan") => {
            "deterministic_structured_evidence"
        }
        Some("spreadsheet_row_analysis") => "spreadsheet_evidence",
        Some("document_parse_status" | "asset_parse_status") => "parse_status",
        Some("conversation_memory_item") => "conversation_memory",
        _ => "other_context",
    }
}

fn assistant_run_model_supply_bucket_limit(bucket: &str) -> usize {
    match bucket {
        "document_status" => ASSISTANT_RUN_MODEL_CONTEXT_PARSE_STATUS_LIMIT,
        "structured_fact" => ASSISTANT_RUN_MODEL_CONTEXT_STRUCTURED_FACT_LIMIT,
        "spreadsheet" => ASSISTANT_RUN_MODEL_CONTEXT_SPREADSHEET_LIMIT,
        "retrieval" => ASSISTANT_RUN_MODEL_CONTEXT_RETRIEVAL_LIMIT,
        "database" => ASSISTANT_RUN_MODEL_CONTEXT_DATABASE_LIMIT,
        "memory" => ASSISTANT_RUN_MODEL_CONTEXT_MEMORY_LIMIT,
        "asset_profile" => ASSISTANT_RUN_MODEL_CONTEXT_ASSET_PROFILE_LIMIT,
        _ => ASSISTANT_RUN_MODEL_CONTEXT_OTHER_LIMIT,
    }
}

fn assistant_run_model_supply_type_label(item: &Value) -> String {
    item.get("type")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn item(item_type: &str, index: usize) -> Value {
        json!({
            "type": item_type,
            "summary": format!("summary-{index}"),
            "content_excerpt": format!("content-{index}"),
            "rows": [{"name": format!("row-{index}")}],
        })
    }

    #[test]
    fn budget_keeps_retrieval_and_database_bucket_quotas_separate() {
        let mut items = Vec::new();
        for index in 0..10 {
            items.push(item("database_aggregate", index));
        }
        for index in 0..10 {
            items.push(item("retrieval_evidence", index));
        }

        let fields: AssistantRunModelBudgetedSupplyItems =
            assistant_run_model_budgeted_supply_items(&items);
        let (model_items, budget) = fields;
        let database_count = model_items
            .iter()
            .filter(|item| item.get("type").and_then(Value::as_str) == Some("database_aggregate"))
            .count();
        let retrieval_count = model_items
            .iter()
            .filter(|item| item.get("type").and_then(Value::as_str) == Some("retrieval_evidence"))
            .count();

        assert_eq!(database_count, ASSISTANT_RUN_MODEL_CONTEXT_DATABASE_LIMIT);
        assert_eq!(retrieval_count, ASSISTANT_RUN_MODEL_CONTEXT_RETRIEVAL_LIMIT);
        assert_eq!(
            budget["included_by_type"]["database_aggregate"],
            json!(ASSISTANT_RUN_MODEL_CONTEXT_DATABASE_LIMIT)
        );
        assert_eq!(
            budget["omitted_by_type"]["database_aggregate"],
            json!(10 - ASSISTANT_RUN_MODEL_CONTEXT_DATABASE_LIMIT)
        );
        assert_eq!(
            budget["omitted_by_type"]["retrieval_evidence"],
            json!(10 - ASSISTANT_RUN_MODEL_CONTEXT_RETRIEVAL_LIMIT)
        );
    }

    #[test]
    fn semantic_supply_model_autonomy_budget_keeps_only_attributable_retrieval_fields() {
        let items = vec![json!({
            "type": "retrieval_evidence",
            "source": "document_chunk_fallback",
            "dataset_id": "dataset-visible",
            "document_id": "document-visible",
            "document_chunk_id": "chunk-visible",
            "retrieval_evidence_id": "evidence-visible",
            "source_locator": "documents/real-policy.pdf#chunk=4",
            "summary": "真实制度资料摘要",
            "content_excerpt": "真实制度资料正文。",
            "semantic_score": 0.12,
            "snapshot_id": "budget-semantic-snapshot-secret",
            "matched_node_ids": ["budget-semantic-node-secret"],
            "query_aliases": ["budget-semantic-alias-secret"],
            "semantic_supply_trace": {
                "match_ids": ["budget-semantic-match-secret"],
                "answer_template": "budget-answer-template-secret",
                "intent": "budget-intent-secret",
                "route": "budget-route-secret",
                "action": "budget-action-secret"
            }
        })];

        let (model_items, budget) = assistant_run_model_budgeted_supply_items(&items);
        let model_item = &model_items[0];
        let serialized = serde_json::to_string(&json!({
            "model_items": model_items,
            "budget": budget,
        }))
        .unwrap();

        assert_eq!(model_item["type"], json!("retrieval_evidence"));
        assert_eq!(model_item["source"], json!("document_chunk_fallback"));
        assert_eq!(model_item["document_id"], json!("document-visible"));
        assert_eq!(model_item["document_chunk_id"], json!("chunk-visible"));
        assert_eq!(
            model_item["retrieval_evidence_id"],
            json!("evidence-visible")
        );
        assert_eq!(
            model_item["source_locator"],
            json!("documents/real-policy.pdf#chunk=4")
        );
        for internal_key in [
            "semantic_score",
            "snapshot_id",
            "matched_node_ids",
            "query_aliases",
            "semantic_supply_trace",
        ] {
            assert!(
                model_item.get(internal_key).is_none(),
                "internal semantic field must not survive model budgeting: {internal_key}"
            );
        }
        for secret in [
            "budget-semantic-snapshot-secret",
            "budget-semantic-node-secret",
            "budget-semantic-alias-secret",
            "budget-semantic-match-secret",
            "budget-answer-template-secret",
            "budget-intent-secret",
            "budget-route-secret",
            "budget-action-secret",
        ] {
            assert!(
                !serialized.contains(secret),
                "internal semantic value leaked through model budgeting: {secret}"
            );
        }
    }

    #[test]
    fn budget_preserves_original_order_after_bucket_selection() {
        let items = vec![
            item("database_aggregate", 0),
            item("retrieval_evidence", 1),
            item("database_aggregate", 2),
            item("retrieval_evidence", 3),
        ];

        let fields: AssistantRunModelBudgetedSupplyItems =
            assistant_run_model_budgeted_supply_items(&items);
        let (model_items, budget) = fields;

        assert_eq!(
            model_items
                .iter()
                .map(|item| item["summary"].as_str().unwrap_or_default())
                .collect::<Vec<_>>(),
            vec!["summary-0", "summary-1", "summary-2", "summary-3"]
        );
        assert_eq!(budget["omitted_supplied_item_count"], json!(0));
    }

    #[test]
    fn budget_labels_missing_type_as_unknown_other() {
        let items = vec![json!({"summary": "missing type"})];

        let fields: AssistantRunModelBudgetedSupplyItems =
            assistant_run_model_budgeted_supply_items(&items);
        let (model_items, budget) = fields;

        assert_eq!(model_items.len(), 1);
        assert_eq!(budget["included_by_type"]["unknown"], json!(1));
        assert_eq!(
            budget["bucket_limits"]["other"],
            json!(ASSISTANT_RUN_MODEL_CONTEXT_OTHER_LIMIT)
        );
    }

    #[test]
    fn budget_keeps_asset_profile_hints_in_their_own_bucket() {
        let mut items = Vec::new();
        for index in 0..8 {
            items.push(item("asset_profile_hint", index));
        }
        for index in 0..8 {
            items.push(item("retrieval_evidence", index));
        }

        let fields: AssistantRunModelBudgetedSupplyItems =
            assistant_run_model_budgeted_supply_items(&items);
        let (model_items, budget) = fields;
        let asset_profile_count = model_items
            .iter()
            .filter(|item| item.get("type").and_then(Value::as_str) == Some("asset_profile_hint"))
            .count();
        let retrieval_count = model_items
            .iter()
            .filter(|item| item.get("type").and_then(Value::as_str) == Some("retrieval_evidence"))
            .count();

        assert_eq!(
            asset_profile_count,
            ASSISTANT_RUN_MODEL_CONTEXT_ASSET_PROFILE_LIMIT
        );
        assert_eq!(retrieval_count, ASSISTANT_RUN_MODEL_CONTEXT_RETRIEVAL_LIMIT);
        assert_eq!(
            budget["included_by_type"]["asset_profile_hint"],
            json!(ASSISTANT_RUN_MODEL_CONTEXT_ASSET_PROFILE_LIMIT)
        );
        assert_eq!(
            budget["bucket_limits"]["asset_profile"],
            json!(ASSISTANT_RUN_MODEL_CONTEXT_ASSET_PROFILE_LIMIT)
        );
    }

    #[test]
    fn budget_counts_attributable_asset_evidence_refs() {
        let items = vec![json!({
            "type": "asset_profile_hint",
            "source": "asset_retrieval_evidence",
            "source_kind": "asset_profile",
            "evidence_ref": "asset-evidence://evidence-1",
            "summary": "春夏连衣裙"
        })];

        let (model_items, budget) = assistant_run_model_budgeted_supply_items(&items);

        assert_eq!(
            model_items[0]["evidence_ref"],
            json!("asset-evidence://evidence-1")
        );
        assert_eq!(budget["asset_profile_evidence_ref_count"], json!(1));
    }

    #[test]
    fn budget_keeps_asset_parse_status_with_asset_profile_bucket() {
        let items = vec![
            item("asset_parse_status", 0),
            item("asset_profile_hint", 1),
            item("retrieval_evidence", 2),
        ];

        let (model_items, budget) = assistant_run_model_budgeted_supply_items(&items);

        assert!(model_items
            .iter()
            .any(|item| item.get("type").and_then(Value::as_str) == Some("asset_parse_status")));
        assert_eq!(budget["included_by_type"]["asset_parse_status"], json!(1));
        assert_eq!(budget["included_by_type"]["asset_profile_hint"], json!(1));
        assert_eq!(
            budget["bucket_limits"]["asset_profile"],
            json!(ASSISTANT_RUN_MODEL_CONTEXT_ASSET_PROFILE_LIMIT)
        );
    }

    #[test]
    fn supply_gate_dry_run_keeps_document_and_asset_evidence_separate() {
        let report = assistant_run_asset_document_supply_gate_dry_run(&[
            json!({
                "type": "retrieval_evidence",
                "source": "document_chunk_fallback",
                "summary": "合同条款证据",
                "content_excerpt": "合同中约定提前通知。"
            }),
            json!({
                "type": "asset_profile_hint",
                "source": "asset_profile",
                "asset_id": "asset-1",
                "title": "服装设计图",
                "summary": "春夏泡泡袖连衣裙",
                "noun_terms": ["泡泡袖", "连衣裙"],
                "object_key": "objects/private/look.png",
                "raw_provider_payload": {"should_not_surface": true}
            }),
        ]);

        assert_eq!(
            report["contract"],
            json!("assistant_run_asset_document_supply_budget_quality_gate_dry_run_v1")
        );
        assert_eq!(report["no_write"], json!(true));
        assert_eq!(
            report["source_kind_dedupe"]["source_kind_order"],
            json!(["document_chunk", "asset_profile"])
        );
        assert_eq!(
            report["citation_policy"]["labels"]["document_evidence"],
            json!(1)
        );
        assert_eq!(
            report["citation_policy"]["labels"]["asset_profile_signal"],
            json!(1)
        );
        assert_eq!(
            report["citation_policy"]["asset_profile_exact_citation_allowed"],
            json!(false)
        );
        assert_eq!(
            report["quality_gate"]["status"],
            json!("grounded_with_asset_context")
        );
        assert_eq!(
            report["quality_gate"]["next_action"],
            json!("answer_with_citation_constraints")
        );
        assert_eq!(report["production_write_allowed"], json!(false));

        let serialized = serde_json::to_string(&report).expect("report should serialize");
        assert!(!serialized.contains("objects/private"));
        assert!(!serialized.contains("raw_provider_payload"));
        assert!(!serialized.contains("should_not_surface"));
    }

    #[test]
    fn supply_gate_dry_run_continues_when_only_asset_profile_is_available() {
        let report = assistant_run_asset_document_supply_gate_dry_run(&[json!({
            "type": "asset_profile_hint",
            "source": "asset_profile",
            "asset_id": "asset-1",
            "title": "服装设计图",
            "summary": "春夏泡泡袖连衣裙",
            "noun_terms": ["泡泡袖", "连衣裙"]
        })]);

        assert_eq!(
            report["quality_gate"]["status"],
            json!("partial_asset_context_only")
        );
        assert_eq!(report["quality_gate"]["needs_more_supply"], json!(true));
        assert_eq!(
            report["quality_gate"]["next_action"],
            json!("continue_expand_supply")
        );
        assert_eq!(
            report["citation_policy"]["exact_claims_require_document_database_or_media_evidence"],
            json!(true)
        );
    }

    #[test]
    fn supply_progress_events_are_non_blocking_for_document_and_asset_supply() {
        let report = assistant_run_supply_progress_events_dry_run(&[
            json!({
                "type": "retrieval_evidence",
                "source": "document_chunk_fallback",
                "summary": "合同条款证据",
                "content_excerpt": "合同中约定提前通知。"
            }),
            json!({
                "type": "asset_profile_hint",
                "source": "asset_profile",
                "asset_id": "asset-1",
                "title": "服装设计图",
                "summary": "春夏泡泡袖连衣裙",
                "object_key": "objects/private/look.png",
                "raw_provider_payload": {"should_not_surface": true}
            }),
        ]);

        assert_eq!(
            report["contract"],
            json!("assistant_run_supply_progress_events_dry_run_v1")
        );
        assert_eq!(report["non_blocking"], json!(true));
        assert_eq!(
            report["continuation_policy"]["answer_with_current_evidence_first"],
            json!(true)
        );
        assert_eq!(
            report["continuation_policy"]["continue_actions_after_reply"],
            json!(true)
        );
        assert_eq!(
            report["continuation_policy"]["final_failure_without_answer"],
            json!(false)
        );
        let event_types = report["events"]
            .as_array()
            .expect("events should be array")
            .iter()
            .map(|event| event["event_type"].as_str().unwrap_or_default())
            .collect::<Vec<_>>();
        assert!(event_types.contains(&"supply_ready"));
        assert!(event_types.contains(&"asset_profile_signal"));
        assert!(!event_types.contains(&"supply_expanding"));
        assert!(report["events"].as_array().unwrap().iter().all(|event| {
            event["blocking"] == json!(false)
                && event["third_party_visible"] == json!(true)
                && event["sensitive_payload_included"] == json!(false)
        }));
        assert_eq!(report["production_write_allowed"], json!(false));

        let serialized = serde_json::to_string(&report).expect("report should serialize");
        assert!(!serialized.contains("objects/private"));
        assert!(!serialized.contains("raw_provider_payload"));
        assert!(!serialized.contains("should_not_surface"));
    }

    #[test]
    fn supply_progress_events_surface_expand_and_parse_retry_without_blocking() {
        let report = assistant_run_supply_progress_events_dry_run(&[
            json!({
                "type": "asset_profile_hint",
                "source": "asset_profile",
                "asset_id": "asset-1",
                "title": "服装设计图",
                "summary": "春夏泡泡袖连衣裙"
            }),
            json!({
                "type": "asset_parse_status",
                "not_ready_asset_count": 2,
                "failed_asset_count": 1,
                "retrying_asset_count": 1,
                "status_counts": {
                    "pending": 1,
                    "retrying": 1
                },
                "object_key": "objects/private/pending.png"
            }),
        ]);

        let event_types = report["events"]
            .as_array()
            .expect("events should be array")
            .iter()
            .map(|event| event["event_type"].as_str().unwrap_or_default())
            .collect::<Vec<_>>();
        assert!(event_types.contains(&"supply_ready"));
        assert!(event_types.contains(&"asset_profile_signal"));
        assert!(event_types.contains(&"supply_expanding"));
        assert!(event_types.contains(&"parse_waiting_or_retry"));
        assert_eq!(
            report["quality_gate_next_action"],
            json!("continue_expand_supply")
        );
        assert_eq!(
            report["continuation_policy"]["parse_or_supply_gap_is_not_terminal"],
            json!(true)
        );

        let serialized = serde_json::to_string(&report).expect("report should serialize");
        assert!(!serialized.contains("objects/private"));
    }

    #[test]
    fn supply_progress_contract_drift_guard_keeps_sse_and_task_card_boundaries() {
        let progress = assistant_run_supply_progress_events_dry_run(&[
            json!({
                "type": "asset_profile_hint",
                "source": "asset_profile",
                "asset_id": "asset-1",
                "title": "服装设计图",
                "summary": "春夏泡泡袖连衣裙"
            }),
            json!({
                "type": "asset_parse_status",
                "not_ready_asset_count": 1,
                "status_counts": {"pending": 1}
            }),
        ]);
        let guard = assistant_run_supply_progress_contract_drift_guard_dry_run(&progress);

        assert_eq!(
            guard["contract"],
            json!("assistant_run_supply_progress_contract_drift_guard_dry_run_v1")
        );
        assert_eq!(
            guard["external_sse_guard"]["schema"],
            json!("v3.external_channel.sse.v1")
        );
        assert_eq!(
            guard["external_sse_guard"]["public_stream_field_mutation_allowed"],
            json!(false)
        );
        assert_eq!(
            guard["external_sse_guard"]["new_progress_events_emit_live_sse"],
            json!(false)
        );
        assert_eq!(
            guard["third_party_guard"]["callback_triggered"],
            json!(false)
        );
        assert_eq!(
            guard["third_party_guard"]["public_request_or_response_field_added"],
            json!(false)
        );
        assert_eq!(
            guard["main_site_task_card_guard"]["task_card_detail_only"],
            json!(true)
        );
        assert_eq!(
            guard["answer_liveness_guard"]["failure_status_blocks_answer"],
            json!(false)
        );
        assert_eq!(
            guard["answer_liveness_guard"]["final_failure_without_answer"],
            json!(false)
        );
        assert_eq!(
            guard["redaction_guard"]["all_events_non_blocking"],
            json!(true)
        );
        assert_eq!(guard["production_write_allowed"], json!(false));
    }
}
