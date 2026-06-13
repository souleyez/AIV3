use domain_model::{Document, DocumentChunk};
use serde_json::Value;

use crate::{collect_string_list, lexical_query_tokens, push_string_hint};

pub(crate) fn document_chunk_search_text(document: &Document, chunk: &DocumentChunk) -> String {
    let section_title_hints = document_chunk_section_title_hints(chunk).join("\n");
    let noun_terms = document_chunk_noun_terms(chunk).join("\n");
    [
        document.title.trim(),
        document.object_key.trim(),
        section_title_hints.trim(),
        noun_terms.trim(),
        chunk.content.trim(),
    ]
    .into_iter()
    .filter(|value| !value.is_empty())
    .collect::<Vec<_>>()
    .join("\n")
}

pub(crate) fn assistant_run_fallback_supply_count(supplied_items: &[Value]) -> usize {
    supplied_items
        .iter()
        .filter(|item| {
            item.get("source")
                .and_then(Value::as_str)
                .is_some_and(|source| source == "document_chunk_fallback")
        })
        .count()
}

pub(crate) fn document_chunk_fallback_source_locator(
    document: &Document,
    chunk: &DocumentChunk,
) -> String {
    let base = if document.object_key.trim().is_empty() {
        document.title.trim()
    } else {
        document.object_key.trim()
    };
    format!("{}#chunk={}", base, chunk.chunk_index)
}

pub(crate) fn document_chunk_fallback_summary(
    document: &Document,
    chunk: &DocumentChunk,
) -> String {
    let title = if document.title.trim().is_empty() {
        document.object_key.trim()
    } else {
        document.title.trim()
    };
    let section = document_chunk_section_title_hints(chunk)
        .first()
        .map(|value| format!(" / {value}"))
        .unwrap_or_default();
    let excerpt = crate::truncate_assistant_supply_text(&chunk.content, 180);
    if excerpt.is_empty() {
        format!("{title} chunk {}{section}", chunk.chunk_index)
    } else {
        format!("{title} chunk {}{section}: {excerpt}", chunk.chunk_index)
    }
}

pub(crate) fn assistant_run_query_centered_supply_excerpt(
    content: &str,
    prompt: &str,
    max_chars: usize,
) -> String {
    let normalized = content
        .trim()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let total_chars = normalized.chars().count();
    if total_chars <= max_chars {
        return normalized;
    }

    let start = assistant_run_prompt_match_char_index(&normalized, prompt)
        .map(|index| index.saturating_sub(max_chars / 4))
        .unwrap_or(0);
    let mut excerpt = normalized
        .chars()
        .skip(start)
        .take(max_chars)
        .collect::<String>();
    if start > 0 {
        excerpt = format!("...{excerpt}");
    }
    if start + max_chars < total_chars {
        excerpt.push_str("...");
    }
    excerpt
}

pub(crate) fn assistant_run_prompt_match_char_index(content: &str, prompt: &str) -> Option<usize> {
    let content_lower = content.to_lowercase();
    let mut tokens = lexical_query_tokens(prompt)
        .into_iter()
        .filter(|token| token.chars().count() >= 2)
        .collect::<Vec<_>>();
    tokens.sort_by(|left, right| {
        right
            .chars()
            .count()
            .cmp(&left.chars().count())
            .then_with(|| left.cmp(right))
    });
    tokens.dedup();
    tokens.into_iter().find_map(|token| {
        let token_lower = token.to_lowercase();
        content_lower
            .find(&token_lower)
            .map(|byte_index| content_lower[..byte_index].chars().count())
    })
}

pub(crate) fn document_chunk_section_title_hints(chunk: &DocumentChunk) -> Vec<String> {
    let mut hints = Vec::new();
    for key in [
        "section_title_hints",
        "sectionTitleHints",
        "section_titles",
        "sectionTitles",
        "heading_hints",
        "headingHints",
    ] {
        if let Some(value) = chunk.metadata.get(key) {
            collect_string_list(value, &mut hints);
        }
    }
    if let Some(value) = chunk
        .metadata
        .get("parse_metadata")
        .and_then(|value| value.get("section_title_hints"))
    {
        collect_string_list(value, &mut hints);
    }
    if let Some(value) = chunk
        .metadata
        .get("parse_metadata")
        .and_then(|value| value.get("document_structure"))
    {
        for hint in document_structure_section_title_hints(value, 6) {
            push_string_hint(&mut hints, hint);
        }
    }
    if let Some(value) = chunk.metadata.get("understanding").and_then(|value| {
        value
            .get("section_title_hints")
            .or_else(|| value.get("sectionTitleHints"))
    }) {
        collect_string_list(value, &mut hints);
    }
    if hints.is_empty() {
        for hint in infer_section_title_hints_from_text(&chunk.content, 6) {
            push_string_hint(&mut hints, hint);
        }
    }
    hints.truncate(6);
    hints
}

pub(crate) fn document_chunk_value_section_title_hints(
    metadata: &Value,
    content: &str,
    limit: usize,
) -> Vec<String> {
    let mut hints = Vec::new();
    for key in [
        "section_title_hints",
        "sectionTitleHints",
        "section_titles",
        "sectionTitles",
        "heading_hints",
        "headingHints",
    ] {
        if let Some(value) = metadata.get(key) {
            collect_string_list(value, &mut hints);
        }
    }
    if let Some(value) = metadata
        .get("parse_metadata")
        .and_then(|value| value.get("section_title_hints"))
    {
        collect_string_list(value, &mut hints);
    }
    if let Some(value) = metadata
        .get("parse_metadata")
        .and_then(|value| value.get("document_structure"))
    {
        for hint in document_structure_section_title_hints(value, limit) {
            push_string_hint(&mut hints, hint);
        }
    }
    if let Some(value) = metadata.get("understanding").and_then(|value| {
        value
            .get("section_title_hints")
            .or_else(|| value.get("sectionTitleHints"))
    }) {
        collect_string_list(value, &mut hints);
    }
    if hints.is_empty() {
        for hint in infer_section_title_hints_from_text(content, limit) {
            push_string_hint(&mut hints, hint);
        }
    }
    hints.truncate(limit);
    hints
}

pub(crate) fn document_chunk_noun_terms(chunk: &DocumentChunk) -> Vec<String> {
    let mut terms = Vec::new();
    if let Some(value) = chunk
        .metadata
        .get("understanding")
        .and_then(|value| value.get("noun_terms").or_else(|| value.get("nounTerms")))
    {
        collect_string_list(value, &mut terms);
    }
    for key in ["noun_terms", "nounTerms", "term_hints", "termHints"] {
        if let Some(value) = chunk.metadata.get(key) {
            collect_string_list(value, &mut terms);
        }
    }
    if let Some(value) = chunk
        .metadata
        .get("parse_metadata")
        .and_then(|value| value.get("document_structure"))
    {
        for term in document_structure_candidate_terms(value, 64) {
            push_string_hint(&mut terms, term);
        }
    }
    terms.truncate(64);
    terms
}

pub(crate) fn document_chunk_value_noun_terms(metadata: &Value) -> Vec<String> {
    let mut terms = Vec::new();
    if let Some(value) = metadata
        .get("understanding")
        .and_then(|value| value.get("noun_terms").or_else(|| value.get("nounTerms")))
    {
        collect_string_list(value, &mut terms);
    }
    for key in ["noun_terms", "nounTerms", "term_hints", "termHints"] {
        if let Some(value) = metadata.get(key) {
            collect_string_list(value, &mut terms);
        }
    }
    if let Some(value) = metadata
        .get("parse_metadata")
        .and_then(|value| value.get("document_structure"))
    {
        for term in document_structure_candidate_terms(value, 64) {
            push_string_hint(&mut terms, term);
        }
    }
    terms.truncate(64);
    terms
}

pub(crate) fn document_structure_section_title_hints(
    structure: &Value,
    limit: usize,
) -> Vec<String> {
    let mut hints = Vec::new();
    let Some(blocks) = structure.get("blocks").and_then(Value::as_array) else {
        return hints;
    };
    for block in blocks {
        if !document_structure_block_is_title(block) {
            continue;
        }
        let Some(text) = document_structure_block_text(block) else {
            continue;
        };
        let title = normalize_section_title_hint(&text).unwrap_or_else(|| {
            text.trim()
                .chars()
                .take(80)
                .collect::<String>()
                .trim()
                .to_string()
        });
        push_string_hint(&mut hints, title);
        if hints.len() >= limit {
            break;
        }
    }
    hints
}

fn document_structure_candidate_terms(structure: &Value, limit: usize) -> Vec<String> {
    let mut terms = Vec::new();
    let Some(blocks) = structure.get("blocks").and_then(Value::as_array) else {
        return terms;
    };
    for block in blocks {
        if !document_structure_block_is_term_source(block) {
            continue;
        }
        let Some(text) = document_structure_block_text(block) else {
            continue;
        };
        push_string_hint(&mut terms, &text);
        for token in lexical_query_tokens(&text) {
            if token.chars().count() >= 2 && token.chars().count() <= 16 {
                push_string_hint(&mut terms, token);
            }
            if terms.len() >= limit {
                return terms;
            }
        }
        if terms.len() >= limit {
            break;
        }
    }
    terms
}

pub(crate) fn document_structure_block_text(block: &Value) -> Option<String> {
    for key in ["text", "content", "rec_text", "markdown", "html"] {
        let Some(text) = block.get(key).and_then(Value::as_str) else {
            continue;
        };
        let normalized = text.trim();
        if !normalized.is_empty() {
            return Some(normalized.chars().take(240).collect());
        }
    }
    None
}

pub(crate) fn document_structure_block_type(block: &Value) -> String {
    for key in ["block_type", "type", "label", "category"] {
        let Some(value) = block.get(key).and_then(Value::as_str) else {
            continue;
        };
        let normalized = value.trim().to_ascii_lowercase();
        if !normalized.is_empty() {
            return normalized;
        }
    }
    String::new()
}

pub(crate) fn document_structure_block_is_title(block: &Value) -> bool {
    let block_type = document_structure_block_type(block);
    [
        "doc_title",
        "paragraph_title",
        "title",
        "header",
        "heading",
        "section_title",
    ]
    .iter()
    .any(|value| block_type.contains(value))
}

pub(crate) fn document_structure_block_is_term_source(block: &Value) -> bool {
    if document_structure_block_is_title(block) {
        return true;
    }
    let block_type = document_structure_block_type(block);
    ["table", "cell", "table_title", "figure_title", "caption"]
        .iter()
        .any(|value| block_type.contains(value))
}

pub(crate) fn infer_section_title_hints_from_text(text: &str, limit: usize) -> Vec<String> {
    let mut hints = Vec::new();
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if let Some(title) = normalize_section_title_hint(line) {
            if !hints.contains(&title) {
                hints.push(title);
            }
            if hints.len() >= limit {
                break;
            }
        }
    }
    hints
}

pub(crate) fn normalize_section_title_hint(line: &str) -> Option<String> {
    let trimmed = line.trim().trim_matches(|ch: char| ch == '*' || ch == '`');
    if trimmed.is_empty() {
        return None;
    }
    let candidate = trimmed
        .strip_prefix('#')
        .map(|value| value.trim_start_matches('#').trim())
        .or_else(|| {
            if let Some((index, _)) = trimmed
                .char_indices()
                .find(|(_, value)| value.is_whitespace())
            {
                let marker = trimmed[..index].trim();
                if marker.chars().count() <= 12 && looks_like_heading_marker(marker) {
                    return Some(trimmed[index..].trim());
                }
            }
            let marker_end = trimmed
                .char_indices()
                .find_map(|(index, value)| {
                    if matches!(value, '、' | '.' | '．' | ')' | '）' | ':' | '：') {
                        Some(index + value.len_utf8())
                    } else {
                        None
                    }
                })
                .filter(|index| *index <= 12)?;
            let marker = trimmed[..marker_end].trim();
            looks_like_heading_marker(marker).then(|| trimmed[marker_end..].trim())
        })
        .or_else(|| {
            (trimmed.starts_with('第')
                && trimmed
                    .chars()
                    .take(8)
                    .any(|value| value == '章' || value == '节'))
            .then_some(trimmed)
        })
        .or_else(|| looks_like_standalone_heading(trimmed).then_some(trimmed))?;
    let normalized = normalize_toc_section_title_candidate(candidate)
        .trim_matches(|ch: char| ch.is_whitespace() || "#*-_".contains(ch))
        .chars()
        .take(80)
        .collect::<String>();
    (!normalized.is_empty()).then_some(normalized)
}

fn normalize_toc_section_title_candidate(candidate: &str) -> String {
    let mut value = candidate.trim().to_string();
    if let Some(index) = value.find("....") {
        value.truncate(index);
    } else if let Some(index) = value.find('…') {
        value.truncate(index);
    }
    value
        .trim_matches(|ch: char| {
            ch.is_whitespace()
                || matches!(ch, '.' | '．' | '。' | '·' | '•' | '-' | '_' | '—' | '–')
        })
        .to_string()
}

fn looks_like_heading_marker(value: &str) -> bool {
    value.chars().any(|ch| ch.is_ascii_digit())
        || value.chars().any(|ch| "一二三四五六七八九十".contains(ch))
}

fn looks_like_standalone_heading(value: &str) -> bool {
    let char_count = value.chars().count();
    char_count >= 2
        && char_count <= 32
        && !value.ends_with('。')
        && !value.ends_with('！')
        && !value.ends_with('？')
        && !value.ends_with(';')
        && !value.ends_with('；')
        && !value.contains('|')
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{DatasetId, DocumentChunkId, DocumentChunkState, DocumentId, TenantId};
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn document_chunk_support_reads_value_section_and_terms() {
        let metadata = json!({
            "heading_hints": ["工作经历"],
            "parse_metadata": {
                "document_structure": {
                    "blocks": [
                        {"type": "paragraph_title", "text": "项目经验"},
                        {"type": "table", "text": "核心技能 Java 微服务"}
                    ]
                }
            },
            "understanding": {
                "noun_terms": ["供应商确认"]
            }
        });

        let title_hints = document_chunk_value_section_title_hints(&metadata, "", 6);
        let noun_terms = document_chunk_value_noun_terms(&metadata);

        assert_eq!(title_hints, vec!["工作经历", "项目经验"]);
        assert!(noun_terms.iter().any(|term| term == "供应商确认"));
        assert!(noun_terms.iter().any(|term| term == "核心技能 Java 微服务"));
        assert!(noun_terms.iter().any(|term| term == "微服务"));
    }

    #[test]
    fn fallback_supply_helpers_keep_existing_source_and_summary_shape() {
        let now = Utc::now();
        let tenant_id = TenantId::new();
        let dataset_id = DatasetId::new();
        let document_id = DocumentId::new();
        let document = Document {
            id: document_id,
            tenant_id,
            dataset_id,
            owner_user_id: None,
            title: "护理手册".to_string(),
            object_key: "documents/care.md".to_string(),
            content_type: "text/markdown".to_string(),
            lifecycle: domain_model::DocumentLifecycle::Extracted,
            secret_binding_ids: Vec::new(),
            metadata: BTreeMap::new(),
            created_at: now,
            updated_at: now,
        };
        let mut metadata = BTreeMap::new();
        metadata.insert("section_title_hints".to_string(), json!(["发药核对"]));
        let chunk = DocumentChunk {
            id: DocumentChunkId::new(),
            tenant_id,
            dataset_id,
            document_id,
            chunk_index: 12,
            content: "发药前需要核对老人姓名、床号、药品名称和剂量。".to_string(),
            token_count: 18,
            state: DocumentChunkState::Extracted,
            metadata,
            created_at: now,
            updated_at: now,
        };

        assert_eq!(
            document_chunk_fallback_source_locator(&document, &chunk),
            "documents/care.md#chunk=12"
        );
        assert_eq!(
            document_chunk_fallback_summary(&document, &chunk),
            "护理手册 chunk 12 / 发药核对: 发药前需要核对老人姓名、床号、药品名称和剂量。"
        );
        assert_eq!(
            assistant_run_fallback_supply_count(&[
                json!({"source": "document_chunk_fallback"}),
                json!({"source": "retrieval_evidence"}),
                json!({"source": "document_chunk_fallback"}),
            ]),
            2
        );
    }

    #[test]
    fn query_centered_excerpt_keeps_prompt_matched_answer_window() {
        let content = format!(
            "{}可视对讲分机上默认配置有6个场景：回家、离家、用餐、会客、观影、休息。{}",
            "项目背景说明。".repeat(120),
            "其他介绍。".repeat(40)
        );

        let excerpt = assistant_run_query_centered_supply_excerpt(
            &content,
            "可视对讲分机上默认配置有几个场景",
            220,
        );

        assert!(excerpt.starts_with("..."));
        assert!(excerpt.contains("默认配置有6个场景"));
        assert!(excerpt.contains("回家、离家、用餐、会客、观影、休息"));
    }

    #[test]
    fn query_centered_excerpt_falls_back_to_leading_window_without_match() {
        let content = format!("{}{}", "alpha beta ".repeat(30), "omega");

        let excerpt = assistant_run_query_centered_supply_excerpt(&content, "unmatched", 24);

        assert!(excerpt.starts_with("alpha beta alpha beta"));
        assert!(excerpt.ends_with("..."));
    }

    #[test]
    fn prompt_match_char_index_prefers_longer_prompt_token() {
        let content = "前言 智能家居系统 其他 智能";

        let match_index = assistant_run_prompt_match_char_index(content, "智能 智能家居系统");

        assert_eq!(match_index, Some(3));
    }

    #[test]
    fn normalize_section_title_hint_strips_markers_and_toc_tail() {
        assert_eq!(
            normalize_section_title_hint("1. 项目经验 .... 12"),
            Some("项目经验".to_string())
        );
        assert_eq!(
            infer_section_title_hints_from_text("一、护理流程\n长期卧床老人需要定时翻身。", 2),
            vec!["护理流程"]
        );
    }
}
