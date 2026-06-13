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
    use serde_json::json;

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
