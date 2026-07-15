use serde_json::{json, Map, Value};

use crate::{
    assistant_run_compact_dataset_entity_scan_payload, truncate_assistant_supply_text,
    ASSISTANT_RUN_DATABASE_AGGREGATE_RESULT_LIMIT, ASSISTANT_RUN_MODEL_CONTEXT_EVIDENCE_TEXT_LIMIT,
    ASSISTANT_RUN_MODEL_CONTEXT_ROW_TEXT_LIMIT, ASSISTANT_RUN_MODEL_CONTEXT_SUMMARY_TEXT_LIMIT,
    ASSISTANT_RUN_MODEL_HISTORY_TEXT_LIMIT, ASSISTANT_RUN_MODEL_SCAN_BRIEF_TEXT_LIMIT,
    ASSISTANT_RUN_MODEL_SUPPLY_BRIEF_TEXT_LIMIT,
};

pub(crate) fn assistant_run_model_supply_item_for_context(item: &Value) -> Value {
    match item.get("type").and_then(Value::as_str) {
        Some("dataset_entity_scan") => assistant_run_compact_dataset_entity_scan_payload(item)
            .unwrap_or_else(|| assistant_run_model_dataset_entity_scan_item(item)),
        Some("dataset_fact_snapshot") => assistant_run_model_dataset_fact_snapshot_item(item),
        Some("retrieval_evidence") => assistant_run_model_retrieval_evidence_item(item),
        Some("search_evidence") => assistant_run_model_search_evidence_item(item),
        Some("document_parse_status") => assistant_run_model_document_parse_status_item(item),
        Some("asset_parse_status") => assistant_run_model_asset_parse_status_item(item),
        Some("database_schema_context") => assistant_run_model_database_schema_context_item(item),
        Some("database_aggregate") => assistant_run_model_database_aggregate_item(item),
        Some("spreadsheet_row_analysis") => assistant_run_model_spreadsheet_row_analysis_item(item),
        Some("conversation_memory_item") => assistant_run_model_conversation_memory_item(item),
        Some("asset_profile_hint") => assistant_run_model_asset_profile_hint_item(item),
        _ => assistant_run_model_compact_json_value(
            item,
            ASSISTANT_RUN_MODEL_CONTEXT_SUMMARY_TEXT_LIMIT,
            12,
        ),
    }
}

pub(crate) fn assistant_run_model_compact_json_value(
    value: &Value,
    text_limit: usize,
    array_limit: usize,
) -> Value {
    match value {
        Value::String(text) => Value::String(truncate_assistant_supply_text(text, text_limit)),
        Value::Array(items) => Value::Array(
            items
                .iter()
                .take(array_limit)
                .map(|item| assistant_run_model_compact_json_value(item, text_limit, array_limit))
                .collect(),
        ),
        Value::Object(object) => {
            let mut compact = Map::new();
            for (key, child) in object {
                compact.insert(
                    key.clone(),
                    assistant_run_model_compact_json_value(child, text_limit, array_limit),
                );
            }
            Value::Object(compact)
        }
        _ => value.clone(),
    }
}

fn assistant_run_model_dataset_entity_scan_item(item: &Value) -> Value {
    json!({
        "type": "dataset_entity_scan",
        "source": item.get("source").cloned().unwrap_or(Value::Null),
        "dataset_id": item.get("dataset_id").cloned().unwrap_or(Value::Null),
        "summary": item.get("summary").cloned().unwrap_or(Value::Null),
        "score": item.get("score").cloned().unwrap_or(Value::Null),
        "scanned_document_count": item.get("scanned_document_count").cloned().unwrap_or(Value::Null),
        "entity_count": item.get("entity_count").cloned().unwrap_or(Value::Null),
        "organization_count": item.get("organization_count").cloned().unwrap_or(Value::Null),
        "company_count": item.get("company_count").cloned().unwrap_or(Value::Null),
        "candidate_term_count": item.get("candidate_term_count").cloned().unwrap_or(Value::Null),
        "company_rows": item.get("company_rows").cloned().unwrap_or(Value::Null),
        "skill_rows": item.get("skill_rows").cloned().unwrap_or(Value::Null),
        "project_rows": item.get("project_rows").cloned().unwrap_or(Value::Null),
        "position_rows": item.get("position_rows").cloned().unwrap_or(Value::Null),
        "location_rows": item.get("location_rows").cloned().unwrap_or(Value::Null),
        "person_rows": item.get("person_rows").cloned().unwrap_or(Value::Null),
        "school_rows": item.get("school_rows").cloned().unwrap_or(Value::Null),
        "degree_rows": item.get("degree_rows").cloned().unwrap_or(Value::Null),
        "certificate_rows": item.get("certificate_rows").cloned().unwrap_or(Value::Null),
        "keyword_rows": item.get("keyword_rows").cloned().unwrap_or(Value::Null),
        "year_rows": item.get("year_rows").cloned().unwrap_or(Value::Null),
        "section_rows": item.get("section_rows").cloned().unwrap_or(Value::Null),
        "paragraph_rows": item.get("paragraph_rows").cloned().unwrap_or(Value::Null),
        "table_rows": item.get("table_rows").cloned().unwrap_or(Value::Null),
        "entity_rows_by_type": item.get("entity_rows_by_type").cloned().unwrap_or(Value::Null),
        "resume_profile_rows": item.get("resume_profile_rows").cloned().unwrap_or(Value::Null),
        "resume_project_delivery_rows": item.get("resume_project_delivery_rows").cloned().unwrap_or(Value::Null),
        "company_names": item.get("company_names").cloned().unwrap_or(Value::Null),
        "entities": item.get("entities").cloned().unwrap_or(Value::Null),
        "answer_guidance": item.get("answer_guidance").cloned().unwrap_or(Value::Null),
        "limits": item.get("limits").cloned().unwrap_or(Value::Null),
        "model_note": "Use *_rows, keyword_rows, year_rows, section_rows, paragraph_rows, table_rows, resume_profile_rows, and resume_project_delivery_rows as authoritative structured scan tables when answering entity/document dimension questions. For multi-resume project delivery/detail questions, prefer resume_project_delivery_rows over top-k retrieval chunks. Cite scanned_document_count and row counts when giving totals. Use scanned_document_count as the document total; do not sum row document_count as total documents. Do not extend company lists from candidate_terms or document_hits.",
    })
}

fn assistant_run_model_dataset_fact_snapshot_item(item: &Value) -> Value {
    json!({
        "type": "dataset_fact_snapshot",
        "source": item.get("source").cloned().unwrap_or(Value::Null),
        "dataset_id": item.get("dataset_id").cloned().unwrap_or(Value::Null),
        "dataset_key": item.get("dataset_key").cloned().unwrap_or(Value::Null),
        "snapshot_kind": item.get("snapshot_kind").cloned().unwrap_or(Value::Null),
        "snapshot_key": item.get("snapshot_key").cloned().unwrap_or(Value::Null),
        "summary": item.get("summary").cloned().unwrap_or(Value::Null),
        "scanned_document_count": item.get("scanned_document_count").cloned().unwrap_or(Value::Null),
        "source_document_count": item.get("source_document_count").cloned().unwrap_or(Value::Null),
        "source_fact_count": item.get("source_fact_count").cloned().unwrap_or(Value::Null),
        "row_count_by_type": item.get("row_count_by_type").cloned().unwrap_or(Value::Null),
        "entity_rows_by_type": item.get("entity_rows_by_type").cloned().unwrap_or(Value::Null),
        "model_note": item.get("model_note").cloned().unwrap_or_else(|| json!("Use this as the authoritative dataset-level aggregate for count/list/rank questions. Cite scanned_document_count, source_document_count, source_fact_count, and row_count_by_type when giving totals. Use retrieval evidence only for examples, quotes, and validation; never infer totals from retrieval chunks.")),
    })
}

pub(crate) fn assistant_run_model_supply_item_brief(item: &Value) -> Option<String> {
    let item_type = item.get("type").and_then(Value::as_str).unwrap_or("item");
    let source = item
        .get("source")
        .and_then(Value::as_str)
        .unwrap_or("retrieval_evidence");
    let summary = assistant_run_model_text_field(item, &["summary", "content_excerpt"])?;
    let locator = item
        .get("source_locator")
        .or_else(|| item.get("sourceLocator"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let document_id = item
        .get("document_id")
        .or_else(|| item.get("documentId"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let mut parts = vec![
        format!("{item_type}/{source}"),
        format!(
            "摘要={}",
            truncate_assistant_supply_text(
                &summary,
                if matches!(item_type, "dataset_entity_scan" | "dataset_fact_snapshot") {
                    ASSISTANT_RUN_MODEL_SCAN_BRIEF_TEXT_LIMIT
                } else {
                    ASSISTANT_RUN_MODEL_SUPPLY_BRIEF_TEXT_LIMIT
                }
            )
        ),
    ];
    if let Some(locator) = locator {
        parts.push(format!("来源={locator}"));
    }
    if let Some(document_id) = document_id {
        parts.push(format!("document_id={document_id}"));
    }
    if item_type == "asset_profile_hint" {
        if let Some(title) = item
            .get("title")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            parts.push(format!(
                "标题={}",
                truncate_assistant_supply_text(title, ASSISTANT_RUN_MODEL_SUPPLY_BRIEF_TEXT_LIMIT)
            ));
        }
        if let Some(asset_kind) = item
            .get("asset_kind")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            parts.push(format!("资产={asset_kind}"));
        }
        let noun_terms = item
            .get("noun_terms")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .take(6)
            .collect::<Vec<_>>();
        if !noun_terms.is_empty() {
            parts.push(format!("名词={}", noun_terms.join("、")));
        }
    }
    if let Some(media) = assistant_run_model_media_brief(item) {
        parts.push(media);
    }
    Some(parts.join("；"))
}

pub(crate) fn assistant_run_model_memory_item_brief(item: &Value) -> Option<String> {
    let summary = assistant_run_model_text_field(item, &["summary"])?;
    Some(truncate_assistant_supply_text(
        &summary,
        ASSISTANT_RUN_MODEL_SUPPLY_BRIEF_TEXT_LIMIT,
    ))
}

pub(crate) fn assistant_run_model_detail_target_brief(target: &Value) -> Option<String> {
    let document_id = target
        .get("document_id")
        .or_else(|| target.get("documentId"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let reason = target
        .get("reason")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("detail_first_scope");
    let source_locator = target
        .get("source_locator")
        .or_else(|| target.get("sourceLocator"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("");
    if source_locator.is_empty() {
        Some(format!("document_id={document_id}, reason={reason}"))
    } else {
        Some(format!(
            "document_id={document_id}, reason={reason}, source={source_locator}"
        ))
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

fn assistant_run_model_asset_parse_status_item(item: &Value) -> Value {
    let attention_assets = item
        .get("attention_assets")
        .and_then(Value::as_array)
        .map(|assets| {
            assets
                .iter()
                .take(6)
                .map(|asset| {
                    json!({
                        "asset_id": asset.get("asset_id").cloned().unwrap_or(Value::Null),
                        "title": asset.get("title").and_then(Value::as_str).map(|value| truncate_assistant_supply_text(value, ASSISTANT_RUN_MODEL_CONTEXT_SUMMARY_TEXT_LIMIT)).unwrap_or_default(),
                        "asset_kind": asset.get("asset_kind").cloned().unwrap_or(Value::Null),
                        "source_kind": asset.get("source_kind").cloned().unwrap_or(Value::Null),
                        "content_type": asset.get("content_type").cloned().unwrap_or(Value::Null),
                        "profile_count": asset.get("profile_count").cloned().unwrap_or(Value::Null),
                        "parse_status": asset.get("parse_status").cloned().unwrap_or(Value::Null),
                        "model_status": asset.get("model_status").cloned().unwrap_or(Value::Null),
                        "parser_name": asset.get("parser_name").cloned().unwrap_or(Value::Null),
                        "parser_version": asset.get("parser_version").cloned().unwrap_or(Value::Null),
                        "error_code": asset.get("error_code").cloned().unwrap_or(Value::Null),
                        "updated_at": asset.get("updated_at").cloned().unwrap_or(Value::Null),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({
        "type": "asset_parse_status",
        "source": item.get("source").cloned().unwrap_or(Value::Null),
        "dataset_id": item.get("dataset_id").cloned().unwrap_or(Value::Null),
        "dataset_key": item.get("dataset_key").cloned().unwrap_or(Value::Null),
        "summary": item.get("summary").and_then(Value::as_str).map(|value| truncate_assistant_supply_text(value, ASSISTANT_RUN_MODEL_CONTEXT_SUMMARY_TEXT_LIMIT)).unwrap_or_default(),
        "scanned_asset_count": item.get("scanned_asset_count").cloned().unwrap_or(Value::Null),
        "status_summary": item.get("status_summary").cloned().unwrap_or(Value::Null),
        "status_counts": item.get("status_counts").cloned().unwrap_or(Value::Null),
        "active_parse_count": item.get("active_parse_count").cloned().unwrap_or(Value::Null),
        "pending_asset_count": item.get("pending_asset_count").cloned().unwrap_or(Value::Null),
        "failed_asset_count": item.get("failed_asset_count").cloned().unwrap_or(Value::Null),
        "retrying_asset_count": item.get("retrying_asset_count").cloned().unwrap_or(Value::Null),
        "completed_asset_count": item.get("completed_asset_count").cloned().unwrap_or(Value::Null),
        "not_ready_asset_count": item.get("not_ready_asset_count").cloned().unwrap_or(Value::Null),
        "attention_assets": attention_assets,
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

fn assistant_run_model_asset_profile_hint_item(item: &Value) -> Value {
    json!({
        "type": "asset_profile_hint",
        "source": item.get("source").cloned().unwrap_or(Value::Null),
        "dataset_id": item.get("dataset_id").cloned().unwrap_or(Value::Null),
        "dataset_key": item.get("dataset_key").cloned().unwrap_or(Value::Null),
        "asset_id": item.get("asset_id").cloned().unwrap_or(Value::Null),
        "title": item.get("title").and_then(Value::as_str).map(|value| truncate_assistant_supply_text(value, ASSISTANT_RUN_MODEL_CONTEXT_SUMMARY_TEXT_LIMIT)).unwrap_or_default(),
        "asset_kind": item.get("asset_kind").cloned().unwrap_or(Value::Null),
        "source_kind": item.get("source_kind").cloned().unwrap_or(Value::Null),
        "profile_kind": item.get("profile_kind").cloned().unwrap_or(Value::Null),
        "profile_version": item.get("profile_version").cloned().unwrap_or(Value::Null),
        "evidence_ref": item.get("evidence_ref").cloned().unwrap_or(Value::Null),
        "summary": item.get("summary").and_then(Value::as_str).map(|value| truncate_assistant_supply_text(value, ASSISTANT_RUN_MODEL_CONTEXT_SUMMARY_TEXT_LIMIT)).unwrap_or_default(),
        "noun_terms": item.get("noun_terms").map(|value| assistant_run_model_compact_json_value(value, ASSISTANT_RUN_MODEL_CONTEXT_ROW_TEXT_LIMIT, 24)).unwrap_or(Value::Null),
        "facets": item.get("facets").map(|value| assistant_run_model_compact_json_value(value, ASSISTANT_RUN_MODEL_CONTEXT_ROW_TEXT_LIMIT, 12)).unwrap_or(Value::Null),
        "model_guidance": item.get("model_guidance").map(|value| assistant_run_model_compact_json_value(value, ASSISTANT_RUN_MODEL_CONTEXT_SUMMARY_TEXT_LIMIT, 4)).unwrap_or_else(|| json!([
            "This is a compact asset profile hint, not direct source evidence.",
            "Use retrieval evidence or read_document_detail for exact claims."
        ])),
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

fn assistant_run_model_media_brief(item: &Value) -> Option<String> {
    let media = item
        .get("media_context")
        .or_else(|| item.get("mediaContext"))?;
    let kind = media
        .get("media_kind")
        .or_else(|| media.get("mediaKind"))
        .and_then(Value::as_str)
        .unwrap_or("media");
    let parse_status = media
        .get("parse_status")
        .or_else(|| media.get("parseStatus"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let transcript_count = media
        .get("transcript_windows")
        .or_else(|| media.get("transcriptWindows"))
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let scene_count = media
        .get("scene_windows")
        .or_else(|| media.get("sceneWindows"))
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let ocr_count = media
        .get("keyframe_ocr_snippets")
        .or_else(|| media.get("keyframeOcrSnippets"))
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    Some(format!(
        "媒体={kind}/{parse_status}/transcript:{transcript_count}/scene:{scene_count}/ocr:{ocr_count}"
    ))
}

fn assistant_run_model_text_field(item: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        item.get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn compact_json_value_truncates_nested_text_and_arrays() {
        let value = json!({
            "title": "ABCDEFG",
            "rows": [
                {"name": "row-1", "note": "123456"},
                {"name": "row-2", "note": "abcdef"},
                {"name": "row-3", "note": "ignored"}
            ],
            "metadata": {"description": "长文本内容"}
        });

        let compact = assistant_run_model_compact_json_value(&value, 3, 2);

        assert_eq!(compact["title"], json!("ABC"));
        assert_eq!(compact["rows"].as_array().unwrap().len(), 2);
        assert_eq!(compact["rows"][0]["note"], json!("123"));
        assert_eq!(compact["metadata"]["description"], json!("长文本"));
    }

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
    fn semantic_supply_model_autonomy_keeps_real_retrieval_schema_and_drops_internals() {
        let item = json!({
            "type": "retrieval_evidence",
            "source": "document_chunk_fallback",
            "dataset_id": "dataset-visible",
            "document_id": "document-visible",
            "document_chunk_id": "chunk-visible",
            "retrieval_evidence_id": "evidence-visible",
            "chunk_index": 4,
            "source_locator": "documents/real-policy.pdf#page=3&chunk=4",
            "summary": "真实制度资料摘要",
            "content_excerpt": "真实制度资料正文。",
            "score": 0.91,
            "semantic_score": 0.12,
            "snapshot_id": "semantic-snapshot-secret",
            "matched_node_ids": ["semantic-node-secret"],
            "query_aliases": ["semantic-alias-secret"],
            "semantic_supply": {
                "visible_source_document_ids": ["semantic-hidden-document"],
                "evidence_boosts": [{
                    "document_id": "document-visible",
                    "match_ids": ["semantic-match-secret"]
                }],
                "supplement_document_ids": ["semantic-supplement-secret"],
                "trace": {
                    "eligible_node_count": 9,
                    "matched_node_count": 2,
                    "unresolved_provenance_count": 0,
                    "opaque": "semantic-trace-secret"
                },
                "answer_template": "semantic-answer-template-secret",
                "intent": "semantic-intent-secret",
                "route": "semantic-route-secret",
                "action": "semantic-action-secret"
            },
            "evidence_manifest": {
                "evidence": {
                    "section_title_hints": ["制度条款"],
                    "noun_terms": ["通知期限"]
                },
                "semantic_supply": {
                    "trace": "semantic-manifest-trace-secret"
                }
            }
        });

        let model_item = assistant_run_model_supply_item_for_context(&item);
        let model_brief = assistant_run_model_supply_item_brief(&item).unwrap();
        let serialized = serde_json::to_string(&model_item).unwrap();

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
            json!("documents/real-policy.pdf#page=3&chunk=4")
        );
        assert_eq!(
            model_item["evidence_context"]["section_title_hints"],
            json!(["制度条款"])
        );
        assert!(model_brief.contains("documents/real-policy.pdf#page=3&chunk=4"));
        assert!(model_brief.contains("document_id=document-visible"));
        assert!(!model_brief.contains("semantic-"));
        for internal_key in [
            "semantic_score",
            "snapshot_id",
            "matched_node_ids",
            "query_aliases",
            "semantic_supply",
        ] {
            assert!(
                model_item.get(internal_key).is_none(),
                "internal semantic field must not enter model supply: {internal_key}"
            );
        }
        for secret in [
            "semantic-snapshot-secret",
            "semantic-node-secret",
            "semantic-alias-secret",
            "semantic-hidden-document",
            "semantic-match-secret",
            "semantic-supplement-secret",
            "semantic-trace-secret",
            "semantic-manifest-trace-secret",
            "semantic-answer-template-secret",
            "semantic-intent-secret",
            "semantic-route-secret",
            "semantic-action-secret",
        ] {
            assert!(
                !serialized.contains(secret),
                "internal semantic value leaked into model supply: {secret}"
            );
        }
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

    #[test]
    fn asset_profile_hint_keeps_compact_safe_fields_only() {
        let item = json!({
            "type": "asset_profile_hint",
            "source": "asset_profile",
            "dataset_id": "dataset-1",
            "asset_id": "asset-1",
            "title": "门店陈列视频",
            "asset_kind": "video",
            "profile_kind": "video_summary",
            "profile_version": "parser@v1",
            "evidence_ref": "asset-evidence://evidence-1",
            "summary": "夏季女装陈列和导购讲解",
            "noun_terms": ["女装", "陈列", "导购"],
            "facets": ["场景: 门店"],
            "attributes": {
                "raw_provider_payload": "must not enter model context"
            }
        });

        let model_item = assistant_run_model_supply_item_for_context(&item);
        let serialized = serde_json::to_string(&model_item).unwrap();

        assert_eq!(model_item["type"], json!("asset_profile_hint"));
        assert_eq!(model_item["summary"], json!("夏季女装陈列和导购讲解"));
        assert_eq!(model_item["noun_terms"][0], json!("女装"));
        assert_eq!(model_item["profile_version"], json!("parser@v1"));
        assert_eq!(
            model_item["evidence_ref"],
            json!("asset-evidence://evidence-1")
        );
        assert!(!serialized.contains("raw_provider_payload"));
        assert!(!serialized.contains("must not enter model context"));
    }

    #[test]
    fn asset_parse_status_keeps_compact_safe_fields_only() {
        let item = json!({
            "type": "asset_parse_status",
            "source": "visible_asset_parse_state",
            "dataset_id": "dataset-1",
            "summary": "资产解析状态：待解析 1 个。",
            "scanned_asset_count": 2,
            "status_counts": {"pending": 1, "completed": 1},
            "pending_asset_count": 1,
            "not_ready_asset_count": 1,
            "attention_assets": [{
                "asset_id": "asset-1",
                "title": "设计图 A",
                "asset_kind": "image",
                "source_kind": "upload",
                "parse_status": "pending",
                "model_status": "pending",
                "parser_name": "datamax-fashion-image-parser",
                "parser_version": "2026-06-17",
                "object_key": "must-not-enter-model-context",
                "metadata": {"raw_provider_payload": "must not enter model context"}
            }]
        });

        let model_item = assistant_run_model_supply_item_for_context(&item);
        let serialized = serde_json::to_string(&model_item).unwrap();

        assert_eq!(model_item["type"], json!("asset_parse_status"));
        assert_eq!(model_item["not_ready_asset_count"], json!(1));
        assert_eq!(
            model_item["attention_assets"][0]["title"],
            json!("设计图 A")
        );
        assert!(!serialized.contains("object_key"));
        assert!(!serialized.contains("must-not-enter-model-context"));
        assert!(!serialized.contains("raw_provider_payload"));
    }

    #[test]
    fn supply_item_brief_keeps_asset_terms_media_and_locator() {
        let item = json!({
            "type": "asset_profile_hint",
            "source": "asset_profile",
            "summary": "夏季女装陈列视频摘要",
            "source_locator": "视频 00:10",
            "document_id": "doc-video",
            "title": "门店陈列视频",
            "asset_kind": "video",
            "noun_terms": ["女装", "陈列", "导购", "", "夏季", "新品", "橱窗", "超出限制"],
            "media_context": {
                "media_kind": "video",
                "parse_status": "parsed",
                "transcript_windows": [{}, {}],
                "scene_windows": [{}],
                "keyframe_ocr_snippets": [{}, {}, {}]
            }
        });

        let brief = assistant_run_model_supply_item_brief(&item).unwrap();

        assert!(brief.contains("asset_profile_hint/asset_profile"));
        assert!(brief.contains("摘要=夏季女装陈列视频摘要"));
        assert!(brief.contains("来源=视频 00:10"));
        assert!(brief.contains("document_id=doc-video"));
        assert!(brief.contains("标题=门店陈列视频"));
        assert!(brief.contains("资产=video"));
        assert!(brief.contains("名词=女装、陈列、导购、夏季、新品、橱窗"));
        assert!(!brief.contains("超出限制"));
        assert!(brief.contains("媒体=video/parsed/transcript:2/scene:1/ocr:3"));
    }

    #[test]
    fn memory_and_detail_briefs_keep_existing_format() {
        let memory = json!({"summary": " 用户要求在已发布报表上继续修改 "});
        let target = json!({
            "documentId": "doc-1",
            "reason": "media_detail",
            "sourceLocator": "第 2 页"
        });

        assert_eq!(
            assistant_run_model_memory_item_brief(&memory),
            Some("用户要求在已发布报表上继续修改".to_string())
        );
        assert_eq!(
            assistant_run_model_detail_target_brief(&target),
            Some("document_id=doc-1, reason=media_detail, source=第 2 页".to_string())
        );
        assert_eq!(
            assistant_run_model_detail_target_brief(&json!({"reason": "missing_doc"})),
            None
        );
    }
}
