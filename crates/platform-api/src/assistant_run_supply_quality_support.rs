use serde_json::{json, Value};
use std::collections::BTreeSet;

use crate::assistant_run_conversation_memory_support::selected_scope_requests_conversation_memory;
use crate::assistant_run_scope_policy_support::{
    assistant_run_scope_intent, assistant_run_scope_prefers_detail,
};
use crate::assistant_run_scope_selection_support::{
    assistant_run_scope_recommended_tool_actions, selected_dataset_ids_from_scope,
};

pub(crate) fn assistant_run_supply_quality_report(
    selected_scope: &Value,
    supply_requested: bool,
    supplied_items: &[Value],
    supplied_datasets: &[Value],
    supplied_memory_items: &[Value],
    detail_targets: &[Value],
    fallback_supply_count: usize,
    limit: usize,
) -> Value {
    let supplied_item_count = supplied_items.len();
    let indexed_evidence_count = supplied_items
        .iter()
        .filter(|item| {
            item.get("type").and_then(Value::as_str) == Some("retrieval_evidence")
                && item
                    .get("source")
                    .and_then(Value::as_str)
                    .map(|source| source != "document_chunk_fallback")
                    .unwrap_or(true)
        })
        .count();
    let media_context_count = supplied_items
        .iter()
        .filter(|item| item.get("media_context").is_some())
        .count();
    let dataset_entity_scan_count = supplied_items
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("dataset_entity_scan"))
        .count();
    let dataset_fact_snapshot_count = supplied_items
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("dataset_fact_snapshot"))
        .count();
    let spreadsheet_row_analysis_count = supplied_items
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("spreadsheet_row_analysis"))
        .count();
    let document_parse_status_count = supplied_items
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("document_parse_status"))
        .count();
    let document_not_ready_count: usize = supplied_items
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("document_parse_status"))
        .filter_map(|item| item.get("not_ready_document_count").and_then(Value::as_u64))
        .map(|value| value as usize)
        .sum();
    let document_failed_count: usize = supplied_items
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("document_parse_status"))
        .filter_map(|item| item.get("failed_document_count").and_then(Value::as_u64))
        .map(|value| value as usize)
        .sum();
    let document_reparsing_count: usize = supplied_items
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("document_parse_status"))
        .filter_map(|item| item.get("reparsing_document_count").and_then(Value::as_u64))
        .map(|value| value as usize)
        .sum();
    let document_degraded_parse_count: usize = supplied_items
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("document_parse_status"))
        .filter_map(|item| item.get("degraded_parse_count").and_then(Value::as_u64))
        .map(|value| value as usize)
        .sum();
    let low_text_evidence_count = assistant_run_low_text_evidence_count(supplied_items);
    let citation_locators = assistant_run_supply_citation_locators(supplied_items, 8);
    let prefer_detail = assistant_run_scope_prefers_detail(selected_scope);
    let status = if !supply_requested {
        "not_requested"
    } else if supplied_item_count == 0 {
        "missing"
    } else if document_not_ready_count > 0
        || document_failed_count > 0
        || document_reparsing_count > 0
        || document_degraded_parse_count > 0
        || low_text_evidence_count > 0
        || (prefer_detail && !detail_targets.is_empty())
    {
        "partial"
    } else {
        "grounded"
    };
    let mut notes = Vec::new();
    if !supply_requested {
        notes.push("ordinary_chat_without_forced_supply");
    }
    if indexed_evidence_count > 0 {
        notes.push("indexed_retrieval_evidence_available");
    }
    if fallback_supply_count > 0 {
        notes.push("fallback_visible_document_chunks_used");
    }
    if low_text_evidence_count > 0 {
        notes.push("low_text_document_evidence");
    }
    if !detail_targets.is_empty() {
        notes.push("detail_read_recommended_before_high_confidence_claims");
    }
    if dataset_entity_scan_count > 0 {
        notes.push("dataset_entity_scan_available");
    }
    if dataset_fact_snapshot_count > 0 {
        notes.push("dataset_fact_snapshot_available");
    }
    if spreadsheet_row_analysis_count > 0 {
        notes.push("spreadsheet_row_analysis_available");
    }
    if document_parse_status_count > 0 {
        notes.push("document_parse_status_available");
    }
    if document_not_ready_count > 0 {
        notes.push("some_documents_not_ready_or_reparsing");
    }
    if document_failed_count > 0 {
        notes.push("some_documents_failed_parse");
    }
    if document_reparsing_count > 0 {
        notes.push("document_reparse_in_progress");
    }
    if document_degraded_parse_count > 0 {
        notes.push("some_documents_parse_degraded");
    }
    if media_context_count > 0 {
        notes.push("media_context_available_with_timestamps_when_present");
    }
    if !supplied_memory_items.is_empty() {
        notes.push("conversation_memory_supplied_by_intent");
    }
    if citation_locators.is_empty() && supply_requested {
        notes.push("no_source_locator_available");
    }
    for note in assistant_run_supply_selection_notes(supplied_items) {
        if !notes.contains(&note) {
            notes.push(note);
        }
    }

    json!({
        "status": status,
        "intent": assistant_run_scope_intent(selected_scope),
        "supplyRequested": supply_requested,
        "qualityFirst": assistant_run_scope_prefers_detail(selected_scope)
            || selected_scope_requests_conversation_memory(selected_scope),
        "selectedDatasetCount": supplied_datasets.len(),
        "suppliedItemCount": supplied_item_count,
        "indexedEvidenceCount": indexed_evidence_count,
        "fallbackChunkCount": fallback_supply_count,
        "conversationMemoryItemCount": supplied_memory_items.len(),
        "mediaContextCount": media_context_count,
        "datasetEntityScanCount": dataset_entity_scan_count,
        "datasetFactSnapshotCount": dataset_fact_snapshot_count,
        "spreadsheetRowAnalysisCount": spreadsheet_row_analysis_count,
        "documentParseStatusCount": document_parse_status_count,
        "documentNotReadyCount": document_not_ready_count,
        "documentFailedCount": document_failed_count,
        "documentReparsingCount": document_reparsing_count,
        "documentDegradedParseCount": document_degraded_parse_count,
        "detailTargetCount": detail_targets.len(),
        "limit": limit,
        "citationLocatorCount": citation_locators.len(),
        "citationLocators": citation_locators,
        "notes": notes,
        "modelGuidance": [
            "treat supplied_items as citable context, not an answer template",
            "distinguish supplied document facts from general model knowledge",
            "when dataset_fact_snapshot is present, use it before runtime entity scans or retrieval for dataset-level count/list/rank questions",
            "when dataset_entity_scan contains company_count and company_rows, use those as the authoritative company statistics and do not extend the list from candidate_terms",
            "when dataset_entity_scan contains scanned_document_count, use it as the total scanned document count and do not sum company_rows.document_count as total documents",
            "when spreadsheet_row_analysis is present, use its rows as the deterministic computed table for attendance, work-hour, absence, and date/time row questions",
            "when document_parse_status reports not-ready, failed, reparsing, or degraded documents, tell the user the relevant document is still parsing or failed instead of claiming its contents",
            "fallback_visible_document_chunks_used means indexed retrieval was expanded with visible document chunks; do not describe that as parser-not-ready unless document_parse_status or low_text_document_evidence says so",
            "when fallback_reason is weak_indexed_evidence_expansion, use those expanded chunks before saying the document did not directly mention the requested flow",
            "when evidence_state.recovery_followup is present, answer with current evidence first; if still unverifiable, ask that follow-up and keep the task continuable in the same conversation",
            "when low_text_document_evidence is present, treat the document extraction as too sparse or low quality, avoid inferring contents from the title, and recommend OCR/reparse/manual review if needed",
            "read detail_targets before asserting exact source wording, tables, OCR, or media timestamps",
            "for aggregate/statistical questions, cite the deterministic supply scope and row counts: scanned_document_count/source_document_count/row_count_by_type for fact snapshots, rows.len/scan_limit for database aggregates, and result_row_count for spreadsheet_row_analysis",
            "ordinary retrieval top-k chunks are secondary support for examples and wording; they are not enough to prove full-dataset totals, rankings, or coverage",
        ],
    })
}

fn assistant_run_supply_selection_notes(supplied_items: &[Value]) -> Vec<&'static str> {
    let mut notes = Vec::new();
    let mut has_dataset_fact_snapshot = false;
    let mut has_scoped_fact_snapshot = false;
    let mut has_dataset_entity_scan = false;
    let mut has_spreadsheet_row_analysis = false;
    let mut has_weak_evidence_expansion = false;

    for item in supplied_items {
        match item.get("type").and_then(Value::as_str) {
            Some("dataset_fact_snapshot") => {
                has_dataset_fact_snapshot = true;
                if item.get("source").and_then(Value::as_str)
                    == Some("document_facts_scoped_aggregate")
                {
                    has_scoped_fact_snapshot = true;
                }
            }
            Some("dataset_entity_scan") => {
                has_dataset_entity_scan = true;
            }
            Some("spreadsheet_row_analysis") => {
                has_spreadsheet_row_analysis = true;
            }
            _ => {}
        }
        if item.get("source").and_then(Value::as_str) == Some("document_chunk_fallback")
            && item.get("fallback_reason").and_then(Value::as_str)
                == Some("weak_indexed_evidence_expansion")
        {
            has_weak_evidence_expansion = true;
        }
    }

    if has_scoped_fact_snapshot {
        notes.push(
            "supply_selection:document_facts_scoped_aggregate_selected_for_scoped_document_aggregate",
        );
    } else if has_dataset_fact_snapshot {
        notes
            .push("supply_selection:dataset_fact_snapshot_selected_for_dataset_aggregate_question");
    }

    if has_dataset_entity_scan {
        if has_dataset_fact_snapshot {
            notes.push(
                "supply_selection:dataset_entity_scan_kept_for_dimensions_not_covered_by_snapshot",
            );
        } else {
            notes.push(
                "supply_selection:dataset_entity_scan_selected_when_snapshot_missing_or_runtime_scan_needed",
            );
        }
    }

    if has_spreadsheet_row_analysis {
        notes.push(
            "supply_selection:spreadsheet_row_analysis_selected_for_attendance_or_workhour_table_question",
        );
    }

    if has_weak_evidence_expansion {
        notes.push("supply_selection:weak_indexed_evidence_expanded_with_visible_chunks");
    }

    notes
}

fn assistant_run_low_text_evidence_count(supplied_items: &[Value]) -> usize {
    supplied_items
        .iter()
        .filter(|item| {
            item.get("type").and_then(Value::as_str) == Some("retrieval_evidence")
                && item.get("source").and_then(Value::as_str) != Some("document_chunk_fallback")
        })
        .filter(|item| {
            let excerpt = item
                .get("content_excerpt")
                .and_then(Value::as_str)
                .or_else(|| item.get("summary").and_then(Value::as_str))
                .unwrap_or_default();
            let signal_chars = excerpt.chars().filter(|ch| !ch.is_whitespace()).count();
            signal_chars > 0 && signal_chars < 20
        })
        .count()
}

fn assistant_run_supply_citation_locators(items: &[Value], limit: usize) -> Vec<String> {
    let mut locators = Vec::new();
    let mut seen = BTreeSet::new();
    for item in items {
        let Some(locator) = item
            .get("source_locator")
            .or_else(|| item.get("sourceLocator"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        if seen.insert(locator.to_string()) {
            locators.push(locator.to_string());
        }
        if locators.len() >= limit {
            break;
        }
    }
    locators
}

pub(crate) fn assistant_run_recommended_supply_actions(
    selected_scope: &Value,
    has_supplied_items: bool,
    dataset_entity_scan_requested: bool,
) -> Vec<&'static str> {
    let mut actions = Vec::new();
    let recommended_tool_actions = assistant_run_scope_recommended_tool_actions(selected_scope);
    if !selected_dataset_ids_from_scope(selected_scope).is_empty() {
        actions.push("retrieve_evidence");
        if assistant_run_scope_prefers_detail(selected_scope) && has_supplied_items {
            actions.push("read_document_detail");
        }
        if dataset_entity_scan_requested {
            actions.push("scan_dataset_entities");
        }
    }
    if selected_scope_requests_conversation_memory(selected_scope) {
        actions.push("recall_conversation_memory");
    }
    match assistant_run_scope_intent(selected_scope) {
        "static_page" => actions.push("create_static_page_draft"),
        "report" => actions.push("list_report_options"),
        _ => {}
    }
    if recommended_tool_actions
        .iter()
        .any(|action| action == "media.resolve_video_url")
    {
        actions.push("resolve_video_url");
    }
    if recommended_tool_actions
        .iter()
        .any(|action| action == "media.extract_ppt_transcript")
    {
        actions.push("extract_video_ppt_transcript");
    }
    actions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supply_quality_report_marks_parse_and_selection_notes() {
        let report = assistant_run_supply_quality_report(
            &json!({"intent": "ordinary_chat"}),
            true,
            &[
                json!({
                    "type": "dataset_fact_snapshot",
                    "source": "document_facts_scoped_aggregate",
                    "source_locator": "facts#1"
                }),
                json!({
                    "type": "dataset_entity_scan"
                }),
                json!({
                    "type": "document_parse_status",
                    "not_ready_document_count": 1
                }),
            ],
            &[json!({"id": "dataset-1"})],
            &[],
            &[],
            0,
            8,
        );

        assert_eq!(report["status"], json!("partial"));
        assert_eq!(report["documentNotReadyCount"], json!(1));
        assert_eq!(report["citationLocators"], json!(["facts#1"]));
        let notes = report["notes"].as_array().expect("notes should be array");
        assert!(notes.iter().any(|note| {
            note.as_str()
                == Some(
                    "supply_selection:document_facts_scoped_aggregate_selected_for_scoped_document_aggregate",
                )
        }));
        assert!(notes
            .iter()
            .any(|note| note.as_str() == Some("document_parse_status_available")));
    }

    #[test]
    fn supply_citation_locators_dedupe_trim_and_limit() {
        let locators = assistant_run_supply_citation_locators(
            &[
                json!({"source_locator": " doc#1 "}),
                json!({"sourceLocator": "doc#1"}),
                json!({"sourceLocator": "doc#2"}),
                json!({"sourceLocator": "doc#3"}),
            ],
            2,
        );

        assert_eq!(locators, vec!["doc#1".to_string(), "doc#2".to_string()]);
    }

    #[test]
    fn recommended_supply_actions_include_scope_and_media_actions() {
        let actions = assistant_run_recommended_supply_actions(
            &json!({
                "intent": "static_page",
                "datasets": ["00000000-0000-0000-0000-000000000001"],
                "conversation_memory": ["local-thread"],
                "supply_policy": {
                    "preferDetail": true,
                    "recommendedActions": [
                        "media.resolve_video_url",
                        "media.extract_ppt_transcript"
                    ]
                }
            }),
            true,
            true,
        );

        assert_eq!(
            actions,
            vec![
                "retrieve_evidence",
                "read_document_detail",
                "scan_dataset_entities",
                "recall_conversation_memory",
                "create_static_page_draft",
                "resolve_video_url",
                "extract_video_ppt_transcript"
            ]
        );
    }
}
