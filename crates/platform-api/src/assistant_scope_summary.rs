use std::collections::{BTreeMap, BTreeSet};

use domain_model::{Document, DocumentChunk};
use serde_json::Value;

pub(crate) fn assistant_scope_document_word_count(
    document: &Document,
    chunks: &[DocumentChunk],
) -> usize {
    let chunk_tokens = chunks
        .iter()
        .map(|chunk| chunk.token_count.max(0) as usize)
        .sum::<usize>();
    if chunk_tokens > 0 {
        return chunk_tokens;
    }
    for key in [
        "estimated_word_count",
        "estimatedWordCount",
        "word_count",
        "wordCount",
    ] {
        if let Some(value) = document.metadata.get(key).and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_str()?.parse::<u64>().ok())
        }) {
            return value as usize;
        }
    }
    0
}

pub(crate) fn assistant_scope_document_title_hint(document: &Document) -> Option<String> {
    let raw_title = document.title.trim();
    let raw = if raw_title.is_empty() {
        let filename = document
            .object_key
            .rsplit('/')
            .next()
            .unwrap_or(&document.object_key)
            .rsplit('\\')
            .next()
            .unwrap_or(&document.object_key);
        filename.trim()
    } else {
        raw_title
    };
    let without_extension = raw
        .rsplit_once('.')
        .and_then(|(stem, extension)| {
            let stem = stem.trim();
            let extension = extension.trim();
            if stem.is_empty() || extension.is_empty() || extension.chars().any(char::is_whitespace)
            {
                None
            } else {
                Some(stem)
            }
        })
        .unwrap_or(raw);
    let normalized =
        without_extension.trim_matches(|ch: char| ch.is_whitespace() || ".-_".contains(ch));
    if normalized.is_empty() {
        None
    } else {
        Some(normalized.chars().take(80).collect())
    }
}

pub(crate) fn assistant_scope_document_parse_status(
    document: &Document,
    chunks: &[DocumentChunk],
) -> String {
    if let Some(value) = document_metadata_parse_status(document) {
        return value;
    }
    if let Some(parse_status) =
        super::extract_media_metadata_from_chunks(chunks).and_then(|metadata| {
            metadata
                .get("parse_status")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
        })
    {
        return parse_status;
    }
    document.lifecycle.as_str().to_string()
}

pub(crate) fn document_metadata_parse_status(document: &Document) -> Option<String> {
    for key in ["parse_status", "parseStatus", "status"] {
        if let Some(value) = document
            .metadata
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(value.to_string());
        }
    }
    None
}

pub(crate) fn assistant_scope_content_kind(content_type: &str) -> &'static str {
    let lower = content_type.trim().to_ascii_lowercase();
    if lower.starts_with("audio/") {
        "audio"
    } else if lower.starts_with("video/") {
        "video"
    } else if lower.starts_with("image/") {
        "image"
    } else if lower.contains("pdf") {
        "pdf"
    } else if lower.contains("spreadsheet") || lower.contains("excel") || lower.contains("csv") {
        "spreadsheet"
    } else if lower.contains("presentation") || lower.contains("powerpoint") {
        "presentation"
    } else if lower.starts_with("text/") || lower.contains("document") || lower.contains("word") {
        "text"
    } else {
        "other"
    }
}

pub(crate) fn assistant_scope_collect_material_hints(
    document: &Document,
    chunks: &[DocumentChunk],
    material_hints: &mut BTreeSet<String>,
) {
    match super::infer_media_kind_from_content_type(&document.content_type) {
        "audio" | "video" => {
            material_hints.insert("audio_video".to_string());
        }
        _ => {}
    }
    if let Some(media_metadata) = super::extract_media_metadata_from_chunks(chunks) {
        material_hints.insert("audio_video".to_string());
        if media_metadata
            .get("transcript_segments")
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty())
        {
            material_hints.insert("transcript_possible".to_string());
        }
        if media_metadata
            .get("scenes")
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty())
        {
            material_hints.insert("scene_possible".to_string());
        }
        if media_metadata
            .get("keyframe_ocr_snippets")
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty())
        {
            material_hints.insert("keyframe_ocr_possible".to_string());
        }
    }
}

pub(crate) fn assistant_scope_collect_document_understanding_hints(
    chunks: &[DocumentChunk],
    noun_term_hints: &mut BTreeSet<String>,
    section_title_hints: &mut BTreeSet<String>,
    understanding_strategy_hints: &mut BTreeSet<String>,
) {
    for chunk in chunks {
        for term in super::document_chunk_noun_terms(chunk).into_iter().take(16) {
            noun_term_hints.insert(term);
            if noun_term_hints.len() >= 64 {
                break;
            }
        }
        for hint in super::document_chunk_section_title_hints(chunk)
            .into_iter()
            .take(8)
        {
            section_title_hints.insert(hint);
            if section_title_hints.len() >= 64 {
                break;
            }
        }
        if let Some(strategy) = chunk
            .metadata
            .get("understanding")
            .and_then(|value| value.get("strategy"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            understanding_strategy_hints.insert(strategy.to_string());
        }
        if noun_term_hints.len() >= 64
            && section_title_hints.len() >= 64
            && understanding_strategy_hints.len() >= 8
        {
            break;
        }
    }
}

pub(crate) fn assistant_scope_count_summary(counts: &BTreeMap<String, usize>) -> String {
    counts
        .iter()
        .map(|(key, count)| format!("{key}:{count}"))
        .collect::<Vec<_>>()
        .join("，")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use domain_model::{
        DatasetId, DocumentChunkId, DocumentChunkState, DocumentId, DocumentLifecycle, TenantId,
    };
    use serde_json::json;

    fn document(title: &str, object_key: &str, content_type: &str) -> Document {
        let now = Utc.timestamp_opt(1_700_000_000, 0).single().unwrap();
        Document {
            id: DocumentId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            owner_user_id: None,
            title: title.to_string(),
            object_key: object_key.to_string(),
            content_type: content_type.to_string(),
            lifecycle: DocumentLifecycle::Indexed,
            secret_binding_ids: Vec::new(),
            metadata: BTreeMap::new(),
            created_at: now,
            updated_at: now,
        }
    }

    fn chunk(document: &Document, token_count: i32, metadata: Value) -> DocumentChunk {
        let now = Utc.timestamp_opt(1_700_000_001, 0).single().unwrap();
        DocumentChunk {
            id: DocumentChunkId::new(),
            tenant_id: document.tenant_id,
            dataset_id: document.dataset_id,
            document_id: document.id,
            chunk_index: 0,
            content: String::new(),
            token_count,
            state: DocumentChunkState::Extracted,
            metadata: metadata
                .as_object()
                .map(|items| {
                    items
                        .iter()
                        .map(|(key, value)| (key.clone(), value.clone()))
                        .collect()
                })
                .unwrap_or_default(),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn assistant_scope_basic_helpers_preserve_metadata_and_filename_fallbacks() {
        let mut document = document(
            "   ",
            r"uploads\2026\--养老机构精细化运营实操手册.docx",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        );
        document
            .metadata
            .insert("estimatedWordCount".to_string(), json!("42"));
        document
            .metadata
            .insert("parseStatus".to_string(), json!(" parsed "));

        assert_eq!(
            assistant_scope_document_title_hint(&document).as_deref(),
            Some("养老机构精细化运营实操手册")
        );
        assert_eq!(assistant_scope_document_word_count(&document, &[]), 42);
        assert_eq!(
            assistant_scope_document_word_count(
                &document,
                &[
                    chunk(&document, -5, json!({})),
                    chunk(&document, 9, json!({}))
                ]
            ),
            9
        );
        assert_eq!(
            assistant_scope_document_parse_status(&document, &[]),
            "parsed"
        );
        assert_eq!(
            document_metadata_parse_status(&document).as_deref(),
            Some("parsed")
        );
        assert_eq!(assistant_scope_content_kind(&document.content_type), "text");
        assert_eq!(assistant_scope_content_kind(" text/csv "), "spreadsheet");
    }

    #[test]
    fn assistant_scope_hints_preserve_media_and_understanding_signals() {
        let document = document("演示视频", "video.mp4", "video/mp4");
        let chunks = vec![chunk(
            &document,
            0,
            json!({
                "media": {
                    "parse_status": "completed",
                    "transcript_segments": [{"text": "hello"}],
                    "scenes": [{"label": "slide"}],
                    "keyframe_ocr_snippets": ["标题"]
                },
                "understanding": {
                    "noun_terms": ["取高", "低活跃"],
                    "strategy": "hybrid"
                },
                "section_title_hints": ["经营总览"]
            }),
        )];

        let mut material_hints = BTreeSet::new();
        assistant_scope_collect_material_hints(&document, &chunks, &mut material_hints);
        assert_eq!(
            material_hints.into_iter().collect::<Vec<_>>(),
            vec![
                "audio_video".to_string(),
                "keyframe_ocr_possible".to_string(),
                "scene_possible".to_string(),
                "transcript_possible".to_string(),
            ]
        );
        assert_eq!(
            assistant_scope_document_parse_status(&document, &chunks),
            "completed"
        );

        let mut noun_term_hints = BTreeSet::new();
        let mut section_title_hints = BTreeSet::new();
        let mut understanding_strategy_hints = BTreeSet::new();
        assistant_scope_collect_document_understanding_hints(
            &chunks,
            &mut noun_term_hints,
            &mut section_title_hints,
            &mut understanding_strategy_hints,
        );
        assert_eq!(
            noun_term_hints.into_iter().collect::<Vec<_>>(),
            vec!["低活跃".to_string(), "取高".to_string()]
        );
        assert_eq!(
            section_title_hints.into_iter().collect::<Vec<_>>(),
            vec!["经营总览".to_string()]
        );
        assert_eq!(
            understanding_strategy_hints.into_iter().collect::<Vec<_>>(),
            vec!["hybrid".to_string()]
        );

        let mut counts = BTreeMap::new();
        counts.insert("indexed".to_string(), 2);
        counts.insert("received".to_string(), 1);
        assert_eq!(
            assistant_scope_count_summary(&counts),
            "indexed:2，received:1"
        );
    }
}
