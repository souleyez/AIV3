use domain_model::{Document, DocumentChunk, DocumentLifecycle};
use serde_json::{json, Map, Value};

use crate::json_value_support::{copy_json_fields, value_at_any_key};

const ASSISTANT_RUN_INFERRED_LOW_TEXT_PARSE_MIN_CHARS: usize = 20;

pub(crate) fn assistant_run_document_parse_model_status(
    document: &Document,
    parse_status: &str,
    chunk_count: usize,
    parse_quality_status: Option<&str>,
    workflow: Option<&Value>,
) -> String {
    let workflow_status = workflow
        .and_then(|value| value.get("status"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let workflow_stage = workflow
        .and_then(|value| value.get("stage"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let workflow_active = matches!(workflow_status, "pending" | "running");

    if workflow_active && document.lifecycle == DocumentLifecycle::Failed {
        return if workflow_status == "pending" {
            "reparse_queued".to_string()
        } else {
            "reparsing".to_string()
        };
    }
    if workflow_active && workflow_stage == "index_retrieval_artifacts" {
        return "indexing".to_string();
    }
    if workflow_active {
        return if workflow_status == "pending" {
            "queued".to_string()
        } else {
            "parsing".to_string()
        };
    }
    if document.lifecycle == DocumentLifecycle::Failed || parse_status == "failed" {
        return "failed".to_string();
    }
    if matches!(parse_status, "parse_degraded" | "placeholder")
        || parse_quality_status
            .map(|status| status.contains("low_text_coverage"))
            .unwrap_or(false)
    {
        return "parse_degraded".to_string();
    }
    if document.lifecycle == DocumentLifecycle::Received {
        return "received".to_string();
    }
    if document.lifecycle == DocumentLifecycle::Indexed {
        return "ready".to_string();
    }
    if document.lifecycle == DocumentLifecycle::Extracted && chunk_count > 0 {
        return "extracted_pending_index".to_string();
    }
    document.lifecycle.as_str().to_string()
}

pub(crate) fn assistant_run_document_parse_status_is_active(status: &str) -> bool {
    matches!(
        status,
        "queued" | "parsing" | "indexing" | "reparsing" | "reparse_queued"
    )
}

pub(crate) fn assistant_run_document_parse_status_needs_attention(status: &str) -> bool {
    !matches!(status, "ready" | "extracted_pending_index" | "archived")
}

pub(crate) fn assistant_run_document_parse_quality_status(document: &Document) -> Option<String> {
    assistant_run_document_parse_quality_metadata(document)
        .and_then(|parse_quality| value_at_any_key(parse_quality, &["status"]))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub(crate) fn assistant_run_document_parse_quality_summary(document: &Document) -> Option<Value> {
    let ingest = document
        .metadata
        .get("ingest")
        .filter(|value| value.is_object())?;
    let parse_quality = assistant_run_document_parse_quality_metadata(document)?;
    let mut summary = Map::new();

    copy_json_fields(
        ingest,
        &mut summary,
        &[
            "parse_method",
            "parse_status",
            "parse_quality_status",
            "cloud_structured_provider",
        ],
    );
    copy_json_fields(
        parse_quality,
        &mut summary,
        &[
            "kind",
            "status",
            "text_chars",
            "min_usable_text_chars",
            "fallback_status",
            "recommended_fallback",
        ],
    );
    if let Some(fallback_from) = value_at_any_key(parse_quality, &["fallback_from", "fallbackFrom"])
        .and_then(compact_parse_quality_candidate_report)
    {
        summary.insert("fallback_from".to_string(), fallback_from);
    }
    if let Some(candidate_selection) =
        assistant_run_parse_quality_candidate_selection_summary(parse_quality)
    {
        summary.insert("candidate_selection".to_string(), candidate_selection);
    }
    if let Some(vlm_rescue) = assistant_run_parse_quality_vlm_rescue_summary(parse_quality) {
        summary.insert("vlm_rescue".to_string(), vlm_rescue);
    }
    if let Some(auto_reparse) = value_at_any_key(ingest, &["auto_reparse", "autoReparse"])
        .and_then(assistant_run_auto_reparse_summary)
    {
        summary.insert("auto_reparse".to_string(), auto_reparse);
    }

    if summary.is_empty() {
        None
    } else {
        Some(Value::Object(summary))
    }
}

pub(crate) fn assistant_run_inferred_low_text_parse_quality_summary(
    document: &Document,
    chunks: &[DocumentChunk],
) -> Option<Value> {
    if !assistant_run_document_is_pdf_like(document) || chunks.is_empty() {
        return None;
    }
    let text_chars = chunks
        .iter()
        .map(|chunk| assistant_run_low_text_parse_signal_chars(&chunk.content))
        .sum::<usize>();
    if text_chars >= ASSISTANT_RUN_INFERRED_LOW_TEXT_PARSE_MIN_CHARS {
        return None;
    }
    Some(json!({
        "kind": "pdf_text_extraction",
        "status": "low_text_coverage",
        "text_chars": text_chars,
        "min_usable_text_chars": ASSISTANT_RUN_INFERRED_LOW_TEXT_PARSE_MIN_CHARS,
        "inferred_from": "stored_document_chunks",
        "fallback_status": "recommended",
        "recommended_fallback": "ocr_reparse"
    }))
}

pub(crate) fn assistant_run_document_ingest_summary(document: &Document) -> Value {
    let Some(ingest) = document
        .metadata
        .get("ingest")
        .filter(|value| value.is_object())
    else {
        return Value::Null;
    };
    let mut summary = Map::new();
    for key in [
        "processor",
        "parse_method",
        "cloud_structured_provider",
        "chunk_count",
        "extracted_chars",
        "failed_at",
        "last_error",
    ] {
        if let Some(value) = ingest.get(key).cloned() {
            summary.insert(key.to_string(), value);
        }
    }
    if let Some(parse_quality_status) = assistant_run_document_parse_quality_status(document) {
        summary.insert(
            "parse_quality_status".to_string(),
            json!(parse_quality_status),
        );
    }
    if let Some(parse_quality_summary) = assistant_run_document_parse_quality_summary(document) {
        summary.insert("parse_quality_summary".to_string(), parse_quality_summary);
    }
    Value::Object(summary)
}

fn assistant_run_document_parse_quality_metadata(document: &Document) -> Option<&Value> {
    let ingest = document
        .metadata
        .get("ingest")
        .filter(|value| value.is_object())?;
    value_at_any_key(ingest, &["parse_metadata", "parseMetadata"])
        .and_then(|parse_metadata| {
            value_at_any_key(parse_metadata, &["parse_quality", "parseQuality"])
        })
        .filter(|value| value.is_object())
}

fn assistant_run_document_is_pdf_like(document: &Document) -> bool {
    document
        .content_type
        .eq_ignore_ascii_case("application/pdf")
        || document.title.to_ascii_lowercase().ends_with(".pdf")
        || document.object_key.to_ascii_lowercase().ends_with(".pdf")
}

fn assistant_run_low_text_parse_signal_chars(text: &str) -> usize {
    text.chars()
        .filter(|ch| {
            !ch.is_whitespace()
                && !ch.is_ascii_punctuation()
                && !matches!(
                    *ch,
                    '。' | '，'
                        | '、'
                        | '；'
                        | '：'
                        | '！'
                        | '？'
                        | '（'
                        | '）'
                        | '【'
                        | '】'
                        | '《'
                        | '》'
                        | '“'
                        | '”'
                        | '‘'
                        | '’'
                        | '-'
                        | '—'
                        | '_'
                        | '|'
                )
        })
        .count()
}

fn assistant_run_parse_quality_candidate_selection_summary(parse_quality: &Value) -> Option<Value> {
    let candidate_selection = value_at_any_key(
        parse_quality,
        &["candidate_selection", "candidateSelection"],
    )?;
    let mut summary = Map::new();
    copy_json_fields(
        candidate_selection,
        &mut summary,
        &["policy", "selected_method"],
    );
    if let Some(selected) = value_at_any_key(candidate_selection, &["selected"])
        .and_then(compact_parse_quality_candidate_report)
    {
        summary.insert("selected".to_string(), selected);
    }
    if let Some(candidate_count) = value_at_any_key(candidate_selection, &["candidates"])
        .and_then(Value::as_array)
        .map(Vec::len)
    {
        summary.insert("candidate_count".to_string(), json!(candidate_count));
    }
    if summary.is_empty() {
        None
    } else {
        Some(Value::Object(summary))
    }
}

fn assistant_run_parse_quality_vlm_rescue_summary(parse_quality: &Value) -> Option<Value> {
    let vlm_rescue = value_at_any_key(parse_quality, &["vlm_rescue", "vlmRescue"])?;
    let mut summary = Map::new();
    copy_json_fields(vlm_rescue, &mut summary, &["policy", "selected"]);
    for key in ["existing", "vlm"] {
        if let Some(report) =
            value_at_any_key(vlm_rescue, &[key]).and_then(compact_parse_quality_candidate_report)
        {
            summary.insert(key.to_string(), report);
        }
    }
    if summary.is_empty() {
        None
    } else {
        Some(Value::Object(summary))
    }
}

fn assistant_run_auto_reparse_summary(auto_reparse: &Value) -> Option<Value> {
    let mut summary = Map::new();
    copy_json_fields(
        auto_reparse,
        &mut summary,
        &[
            "status",
            "reason",
            "attempt_count",
            "max_attempts",
            "updated_at",
        ],
    );
    if summary.is_empty() {
        None
    } else {
        Some(Value::Object(summary))
    }
}

fn compact_parse_quality_candidate_report(value: &Value) -> Option<Value> {
    let mut summary = Map::new();
    copy_json_fields(
        value,
        &mut summary,
        &[
            "method",
            "text_chars",
            "structure_block_count",
            "heading_count",
            "table_signal_count",
            "quality_score",
        ],
    );
    if summary.is_empty() {
        None
    } else {
        Some(Value::Object(summary))
    }
}
