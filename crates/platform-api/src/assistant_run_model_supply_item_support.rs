use serde_json::{json, Map, Value};

use crate::{
    assistant_run_compact_dataset_entity_scan_payload, assistant_run_model_compact_json_value,
    assistant_run_model_dataset_entity_scan_item, assistant_run_model_dataset_fact_snapshot_item,
    truncate_assistant_supply_text, ASSISTANT_RUN_DATABASE_AGGREGATE_RESULT_LIMIT,
    ASSISTANT_RUN_MODEL_CONTEXT_EVIDENCE_TEXT_LIMIT, ASSISTANT_RUN_MODEL_CONTEXT_ROW_TEXT_LIMIT,
    ASSISTANT_RUN_MODEL_CONTEXT_SUMMARY_TEXT_LIMIT, ASSISTANT_RUN_MODEL_HISTORY_TEXT_LIMIT,
};

pub(crate) fn assistant_run_model_supply_item_for_context(item: &Value) -> Value {
    match item.get("type").and_then(Value::as_str) {
        Some("dataset_entity_scan") => assistant_run_compact_dataset_entity_scan_payload(item)
            .unwrap_or_else(|| assistant_run_model_dataset_entity_scan_item(item)),
        Some("dataset_fact_snapshot") => assistant_run_model_dataset_fact_snapshot_item(item),
        Some("retrieval_evidence") => assistant_run_model_retrieval_evidence_item(item),
        Some("search_evidence") => assistant_run_model_search_evidence_item(item),
        Some("document_parse_status") => assistant_run_model_document_parse_status_item(item),
        Some("database_schema_context") => assistant_run_model_database_schema_context_item(item),
        Some("database_aggregate") => assistant_run_model_database_aggregate_item(item),
        Some("spreadsheet_row_analysis") => assistant_run_model_spreadsheet_row_analysis_item(item),
        Some("conversation_memory_item") => assistant_run_model_conversation_memory_item(item),
        _ => assistant_run_model_compact_json_value(
            item,
            ASSISTANT_RUN_MODEL_CONTEXT_SUMMARY_TEXT_LIMIT,
            12,
        ),
    }
}

fn assistant_run_model_retrieval_evidence_item(item: &Value) -> Value {
    let mut output = Map::new();
    assistant_run_model_copy_value(&mut output, item, "type");
    assistant_run_model_copy_value(&mut output, item, "source");
    assistant_run_model_copy_value(&mut output, item, "fallback_reason");
    assistant_run_model_copy_value(&mut output, item, "dataset_id");
    assistant_run_model_copy_value(&mut output, item, "document_id");
    assistant_run_model_copy_value(&mut output, item, "document_chunk_id");
    assistant_run_model_copy_value(&mut output, item, "retrieval_evidence_id");
    assistant_run_model_copy_value(&mut output, item, "chunk_index");
    assistant_run_model_copy_text(
        &mut output,
        item,
        "source_locator",
        ASSISTANT_RUN_MODEL_CONTEXT_SUMMARY_TEXT_LIMIT,
    );
    assistant_run_model_copy_text(
        &mut output,
        item,
        "summary",
        ASSISTANT_RUN_MODEL_CONTEXT_SUMMARY_TEXT_LIMIT,
    );
    assistant_run_model_copy_text(
        &mut output,
        item,
        "content_excerpt",
        ASSISTANT_RUN_MODEL_CONTEXT_EVIDENCE_TEXT_LIMIT,
    );
    assistant_run_model_copy_value(&mut output, item, "score");
    assistant_run_model_copy_value(&mut output, item, "lexical_score");
    assistant_run_model_copy_value(&mut output, item, "recall_score");

    let mut evidence_context = Map::new();
    for key in ["section_title_hints", "noun_terms"] {
        if let Some(value) = item.pointer(&format!("/evidence_manifest/evidence/{key}")) {
            evidence_context.insert(
                key.to_string(),
                assistant_run_model_compact_json_value(
                    value,
                    ASSISTANT_RUN_MODEL_CONTEXT_ROW_TEXT_LIMIT,
                    16,
                ),
            );
        }
    }
    if let Some(value) = item.pointer("/evidence_manifest/embedding/term_weights") {
        evidence_context.insert(
            "term_weights".to_string(),
            assistant_run_model_compact_json_value(value, 80, 24),
        );
    }
    if !evidence_context.is_empty() {
        output.insert(
            "evidence_context".to_string(),
            Value::Object(evidence_context),
        );
    }
    if let Some(media_context) = item.get("media_context") {
        output.insert(
            "media_context".to_string(),
            assistant_run_model_compact_json_value(media_context, 360, 8),
        );
    }
    Value::Object(output)
}

fn assistant_run_model_search_evidence_item(item: &Value) -> Value {
    let mut output = Map::new();
    assistant_run_model_copy_value(&mut output, item, "type");
    assistant_run_model_copy_value(&mut output, item, "source");
    assistant_run_model_copy_value(&mut output, item, "provider");
    assistant_run_model_copy_value(&mut output, item, "rank");
    assistant_run_model_copy_text(
        &mut output,
        item,
        "title",
        ASSISTANT_RUN_MODEL_CONTEXT_SUMMARY_TEXT_LIMIT,
    );
    assistant_run_model_copy_text(
        &mut output,
        item,
        "source_locator",
        ASSISTANT_RUN_MODEL_CONTEXT_SUMMARY_TEXT_LIMIT,
    );
    assistant_run_model_copy_text(
        &mut output,
        item,
        "url",
        ASSISTANT_RUN_MODEL_CONTEXT_SUMMARY_TEXT_LIMIT,
    );
    assistant_run_model_copy_text(
        &mut output,
        item,
        "summary",
        ASSISTANT_RUN_MODEL_CONTEXT_SUMMARY_TEXT_LIMIT,
    );
    assistant_run_model_copy_text(
        &mut output,
        item,
        "content_excerpt",
        ASSISTANT_RUN_MODEL_CONTEXT_EVIDENCE_TEXT_LIMIT,
    );
    assistant_run_model_copy_value(&mut output, item, "retrieved_at");
    if let Some(contract) = item.get("evidence_contract") {
        output.insert(
            "evidence_contract".to_string(),
            assistant_run_model_compact_json_value(contract, 260, 6),
        );
    }
    output.insert(
        "model_rule".to_string(),
        json!("This is DataMax controlled web search evidence. It may be cited only with source URL/title and retrieved_at; do not infer facts beyond title, summary, and source."),
    );
    Value::Object(output)
}

fn assistant_run_model_document_parse_status_item(item: &Value) -> Value {
    let attention_documents = item
        .get("attention_documents")
        .and_then(Value::as_array)
        .map(|documents| {
            documents
                .iter()
                .take(6)
                .map(|document| {
                    json!({
                        "document_id": document.get("document_id").cloned().unwrap_or(Value::Null),
                        "title": document.get("title").and_then(Value::as_str).map(|value| truncate_assistant_supply_text(value, ASSISTANT_RUN_MODEL_CONTEXT_SUMMARY_TEXT_LIMIT)).unwrap_or_default(),
                        "content_type": document.get("content_type").cloned().unwrap_or(Value::Null),
                        "lifecycle": document.get("lifecycle").cloned().unwrap_or(Value::Null),
                        "parse_status": document.get("parse_status").cloned().unwrap_or(Value::Null),
                        "model_status": document.get("model_status").cloned().unwrap_or(Value::Null),
                        "chunk_count": document.get("chunk_count").cloned().unwrap_or(Value::Null),
                        "parse_quality_status": document.get("parse_quality_status").cloned().unwrap_or(Value::Null),
                        "parse_quality_summary": document.get("parse_quality_summary").map(|value| assistant_run_model_compact_json_value(value, ASSISTANT_RUN_MODEL_CONTEXT_SUMMARY_TEXT_LIMIT, 6)).unwrap_or(Value::Null),
                        "external_document": document.get("external_document").map(|value| assistant_run_model_compact_json_value(value, 160, 4)).unwrap_or(Value::Null),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({
        "type": "document_parse_status",
        "source": item.get("source").cloned().unwrap_or(Value::Null),
        "dataset_id": item.get("dataset_id").cloned().unwrap_or(Value::Null),
        "summary": item.get("summary").and_then(Value::as_str).map(|value| truncate_assistant_supply_text(value, ASSISTANT_RUN_MODEL_CONTEXT_SUMMARY_TEXT_LIMIT)).unwrap_or_default(),
        "scanned_document_count": item.get("scanned_document_count").cloned().unwrap_or(Value::Null),
        "status_summary": item.get("status_summary").cloned().unwrap_or(Value::Null),
        "status_counts": item.get("status_counts").cloned().unwrap_or(Value::Null),
        "active_parse_count": item.get("active_parse_count").cloned().unwrap_or(Value::Null),
        "failed_document_count": item.get("failed_document_count").cloned().unwrap_or(Value::Null),
        "reparsing_document_count": item.get("reparsing_document_count").cloned().unwrap_or(Value::Null),
        "degraded_parse_count": item.get("degraded_parse_count").cloned().unwrap_or(Value::Null),
        "not_ready_document_count": item.get("not_ready_document_count").cloned().unwrap_or(Value::Null),
        "attention_documents": attention_documents,
        "model_guidance": item.get("model_guidance").map(|value| assistant_run_model_compact_json_value(value, ASSISTANT_RUN_MODEL_CONTEXT_SUMMARY_TEXT_LIMIT, 5)).unwrap_or(Value::Null),
        "limits": item.get("limits").cloned().unwrap_or(Value::Null),
    })
}

fn assistant_run_model_database_schema_context_item(item: &Value) -> Value {
    json!({
        "type": "database_schema_context",
        "source": item.get("source").cloned().unwrap_or(Value::Null),
        "dataset_id": item.get("dataset_id").cloned().unwrap_or(Value::Null),
        "dataset_key": item.get("dataset_key").cloned().unwrap_or(Value::Null),
        "dataset_title": item.get("dataset_title").cloned().unwrap_or(Value::Null),
        "source_id": item.get("source_id").cloned().unwrap_or(Value::Null),
        "connector_kind": item.get("connector_kind").cloned().unwrap_or(Value::Null),
        "table": item.get("table").cloned().unwrap_or(Value::Null),
        "summary": item.get("summary").and_then(Value::as_str).map(|value| truncate_assistant_supply_text(value, ASSISTANT_RUN_MODEL_CONTEXT_SUMMARY_TEXT_LIMIT)).unwrap_or_default(),
        "field_roles": item.get("field_roles").map(|value| assistant_run_model_compact_json_value(value, ASSISTANT_RUN_MODEL_CONTEXT_ROW_TEXT_LIMIT, 24)).unwrap_or(Value::Null),
        "entity_dimensions": item.get("entity_dimensions").cloned().unwrap_or(Value::Null),
        "time_dimensions": item.get("time_dimensions").cloned().unwrap_or(Value::Null),
        "category_dimensions": item.get("category_dimensions").cloned().unwrap_or(Value::Null),
        "metrics": item.get("metrics").cloned().unwrap_or(Value::Null),
        "analysis_views": item.get("analysis_views").map(|value| assistant_run_model_compact_json_value(value, ASSISTANT_RUN_MODEL_CONTEXT_ROW_TEXT_LIMIT, 8)).unwrap_or(Value::Null),
        "answer_guidance": item.get("answer_guidance").map(|value| assistant_run_model_compact_json_value(value, ASSISTANT_RUN_MODEL_CONTEXT_SUMMARY_TEXT_LIMIT, 8)).unwrap_or(Value::Null),
    })
}

fn assistant_run_model_database_aggregate_item(item: &Value) -> Value {
    json!({
        "type": "database_aggregate",
        "source": item.get("source").cloned().unwrap_or(Value::Null),
        "dataset_id": item.get("dataset_id").cloned().unwrap_or(Value::Null),
        "dataset_key": item.get("dataset_key").cloned().unwrap_or(Value::Null),
        "dataset_title": item.get("dataset_title").cloned().unwrap_or(Value::Null),
        "source_id": item.get("source_id").cloned().unwrap_or(Value::Null),
        "connector_kind": item.get("connector_kind").cloned().unwrap_or(Value::Null),
        "table": item.get("table").cloned().unwrap_or(Value::Null),
        "aggregate_role": item.get("aggregate_role").cloned().unwrap_or(Value::Null),
        "aggregate_intent": item.get("aggregate_intent").cloned().unwrap_or(Value::Null),
        "dimensions": item.get("dimensions").cloned().unwrap_or(Value::Null),
        "metric": item.get("metric").cloned().unwrap_or(Value::Null),
        "aggregation": item.get("aggregation").cloned().unwrap_or(Value::Null),
        "sort_direction": item.get("sort_direction").cloned().unwrap_or(Value::Null),
        "sort_semantics": item.get("sort_semantics").cloned().unwrap_or(Value::Null),
        "time_filter": item.get("time_filter").cloned().unwrap_or(Value::Null),
        "value_label": item.get("value_label").cloned().unwrap_or(Value::Null),
        "summary": item.get("summary").and_then(Value::as_str).map(|value| truncate_assistant_supply_text(value, ASSISTANT_RUN_MODEL_CONTEXT_SUMMARY_TEXT_LIMIT)).unwrap_or_default(),
        "columns": item.get("columns").cloned().unwrap_or(Value::Null),
        "field_semantics": item.get("field_semantics").map(|value| assistant_run_model_compact_json_value(value, ASSISTANT_RUN_MODEL_CONTEXT_ROW_TEXT_LIMIT, 24)).unwrap_or(Value::Null),
        "rows": item.get("rows").map(|value| assistant_run_model_compact_json_value(value, ASSISTANT_RUN_MODEL_CONTEXT_ROW_TEXT_LIMIT, ASSISTANT_RUN_DATABASE_AGGREGATE_RESULT_LIMIT as usize)).unwrap_or(Value::Null),
        "row_limit": item.get("row_limit").cloned().unwrap_or(Value::Null),
        "scan_limit": item.get("scan_limit").cloned().unwrap_or(Value::Null),
        "source_scope": item.get("source_scope").cloned().unwrap_or(Value::Null),
        "policy": item.get("policy").cloned().unwrap_or(Value::Null),
        "note": item.get("note").and_then(Value::as_str).map(|value| truncate_assistant_supply_text(value, ASSISTANT_RUN_MODEL_CONTEXT_SUMMARY_TEXT_LIMIT)).unwrap_or_default(),
        "model_guidance": item.get("model_guidance").cloned().unwrap_or_else(|| json!([
            "Treat database_aggregate.rows as the deterministic aggregate for the requested metric/dimension.",
            "Cite dimensions, metric/value_label, row count, scan_limit, and any time_filter when summarizing totals or rankings.",
            "Use retrieval evidence only for explanation or source wording; do not recompute or estimate aggregate totals from retrieval chunks."
        ])),
    })
}

fn assistant_run_model_spreadsheet_row_analysis_item(item: &Value) -> Value {
    json!({
        "type": "spreadsheet_row_analysis",
        "source": item.get("source").cloned().unwrap_or(Value::Null),
        "dataset_id": item.get("dataset_id").cloned().unwrap_or(Value::Null),
        "analysis_kind": item.get("analysis_kind").cloned().unwrap_or(Value::Null),
        "summary": item.get("summary").and_then(Value::as_str).map(|value| truncate_assistant_supply_text(value, ASSISTANT_RUN_MODEL_CONTEXT_SUMMARY_TEXT_LIMIT)).unwrap_or_default(),
        "content_excerpt": item.get("content_excerpt").and_then(Value::as_str).map(|value| truncate_assistant_supply_text(value, ASSISTANT_RUN_MODEL_CONTEXT_EVIDENCE_TEXT_LIMIT)).unwrap_or_default(),
        "row_count": item.get("row_count").cloned().unwrap_or(Value::Null),
        "result_row_count": item.get("result_row_count").cloned().unwrap_or(Value::Null),
        "rows": item.get("rows").map(|value| assistant_run_model_compact_json_value(value, ASSISTANT_RUN_MODEL_CONTEXT_ROW_TEXT_LIMIT, 64)).unwrap_or(Value::Null),
        "documents": item.get("documents").map(|value| assistant_run_model_compact_json_value(value, ASSISTANT_RUN_MODEL_CONTEXT_ROW_TEXT_LIMIT, 16)).unwrap_or(Value::Null),
        "source_locator": item.get("source_locator").cloned().unwrap_or(Value::Null),
        "model_guidance": item.get("model_guidance").map(|value| assistant_run_model_compact_json_value(value, ASSISTANT_RUN_MODEL_CONTEXT_SUMMARY_TEXT_LIMIT, 5)).unwrap_or(Value::Null),
    })
}

pub(crate) fn assistant_run_model_conversation_memory_item(item: &Value) -> Value {
    json!({
        "type": "conversation_memory_item",
        "conversation_memory_item_id": item.get("conversation_memory_item_id").cloned().unwrap_or(Value::Null),
        "local_thread_id": item.get("local_thread_id").cloned().unwrap_or(Value::Null),
        "role": item.get("role").cloned().unwrap_or(Value::Null),
        "item_kind": item.get("item_kind").cloned().unwrap_or(Value::Null),
        "summary": item.get("summary").and_then(Value::as_str).map(|value| truncate_assistant_supply_text(value, ASSISTANT_RUN_MODEL_HISTORY_TEXT_LIMIT)).unwrap_or_default(),
        "source_message_refs": item.get("source_message_refs").map(|value| assistant_run_model_compact_json_value(value, 160, 8)).unwrap_or(Value::Null),
        "artifact_refs": item.get("artifact_refs").map(|value| assistant_run_model_compact_json_value(value, 160, 8)).unwrap_or(Value::Null),
        "created_at": item.get("created_at").cloned().unwrap_or(Value::Null),
        "updated_at": item.get("updated_at").cloned().unwrap_or(Value::Null),
    })
}

fn assistant_run_model_copy_value(output: &mut Map<String, Value>, item: &Value, key: &str) {
    if let Some(value) = item.get(key) {
        output.insert(key.to_string(), value.clone());
    }
}

fn assistant_run_model_copy_text(
    output: &mut Map<String, Value>,
    item: &Value,
    key: &str,
    max_chars: usize,
) {
    if let Some(value) = item.get(key).and_then(Value::as_str) {
        output.insert(
            key.to_string(),
            Value::String(truncate_assistant_supply_text(value, max_chars)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn retrieval_evidence_context_keeps_safe_manifest_hints_only() {
        let item = json!({
            "type": "retrieval_evidence",
            "source": "document_chunk_fallback",
            "document_id": "doc-1",
            "summary": "护理应急处置",
            "content_excerpt": "老人跌倒后应先评估意识和呼吸。",
            "evidence_manifest": {
                "evidence": {
                    "section_title_hints": ["突发事件应急预防与处置"],
                    "noun_terms": ["跌倒", "通知家属"]
                },
                "raw_internal_blob": "must not enter model context"
            }
        });

        let model_item = assistant_run_model_supply_item_for_context(&item);
        let serialized = serde_json::to_string(&model_item).unwrap();

        assert_eq!(model_item["type"], json!("retrieval_evidence"));
        assert_eq!(
            model_item["evidence_context"]["section_title_hints"][0],
            json!("突发事件应急预防与处置")
        );
        assert!(!serialized.contains("raw_internal_blob"));
        assert!(!serialized.contains("must not enter model context"));
    }

    #[test]
    fn search_evidence_keeps_contract_and_controlled_search_rule() {
        let item = json!({
            "type": "search_evidence",
            "source": "web_search",
            "provider": "fixture",
            "rank": 1,
            "title": "DataMax 官方说明",
            "url": "https://example.com/datamax",
            "retrieved_at": "2026-06-11T08:00:00Z",
            "evidence_contract": {
                "source_url": "https://example.com/datamax",
                "query_metadata": {"raw_query_omitted": true}
            }
        });

        let model_item = assistant_run_model_supply_item_for_context(&item);

        assert_eq!(model_item["type"], json!("search_evidence"));
        assert_eq!(model_item["provider"], json!("fixture"));
        assert_eq!(
            model_item["evidence_contract"]["query_metadata"]["raw_query_omitted"],
            json!(true)
        );
        assert!(model_item["model_rule"]
            .as_str()
            .unwrap_or_default()
            .contains("DataMax controlled web search evidence"));
    }

    #[test]
    fn database_aggregate_uses_default_model_guidance_and_compacts_rows() {
        let rows = (0..80)
            .map(|index| json!({"shop": format!("门店{index}"), "note": "X".repeat(200)}))
            .collect::<Vec<_>>();
        let item = json!({
            "type": "database_aggregate",
            "table": "bi_contract_warning",
            "summary": "取高机会门店",
            "rows": rows
        });

        let model_item = assistant_run_model_supply_item_for_context(&item);

        assert_eq!(model_item["type"], json!("database_aggregate"));
        assert_eq!(
            model_item["rows"].as_array().unwrap().len(),
            ASSISTANT_RUN_DATABASE_AGGREGATE_RESULT_LIMIT as usize
        );
        assert!(model_item["model_guidance"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value
                .as_str()
                .unwrap_or_default()
                .contains("deterministic aggregate")));
    }

    #[test]
    fn conversation_memory_item_truncates_summary_and_refs() {
        let item = json!({
            "type": "conversation_memory_item",
            "conversation_memory_item_id": "memory-1",
            "summary": "A".repeat(ASSISTANT_RUN_MODEL_HISTORY_TEXT_LIMIT + 20),
            "source_message_refs": (0..12).map(|index| json!({"id": format!("msg-{index}")})).collect::<Vec<_>>()
        });

        let model_item = assistant_run_model_conversation_memory_item(&item);

        assert_eq!(
            model_item["summary"]
                .as_str()
                .unwrap_or_default()
                .chars()
                .count(),
            ASSISTANT_RUN_MODEL_HISTORY_TEXT_LIMIT
        );
        assert_eq!(
            model_item["source_message_refs"].as_array().unwrap().len(),
            8
        );
    }
}
