use std::collections::BTreeSet;

use contracts::ExternalBotMessageView;

use crate::{
    is_cjk_query_token_char, lexical_query_tokens, text_normalization::non_empty_trimmed_string,
};

pub(crate) fn external_channel_attachment_title_hints(
    message: &ExternalBotMessageView,
) -> Vec<String> {
    let mut hints = Vec::new();
    for attachment in &message.attachment_refs {
        if let Some(filename) = attachment
            .filename
            .as_deref()
            .and_then(non_empty_trimmed_string)
        {
            if !hints.contains(&filename) {
                hints.push(filename);
            }
        }
    }
    if let Some(text) = message.text.as_deref() {
        for hint in external_channel_attachment_title_hints_from_text(text) {
            if !hints.contains(&hint) {
                hints.push(hint);
            }
        }
    }
    hints
}

fn external_channel_attachment_title_hints_from_text(text: &str) -> Vec<String> {
    let mut hints = Vec::new();
    let markers = ["[附件:", "【附件:", "附件：", "附件:"];
    let chars = text.chars().collect::<Vec<_>>();
    let mut index = 0usize;
    while index < text.len() {
        let slice = &text[index..];
        let Some((marker_offset, marker)) = markers
            .iter()
            .filter_map(|marker| slice.find(marker).map(|offset| (offset, *marker)))
            .min_by_key(|(offset, _)| *offset)
        else {
            break;
        };
        let start_byte = index + marker_offset + marker.len();
        let start_char = text[..start_byte].chars().count();
        let mut end_char = start_char;
        while end_char < chars.len() {
            let value = chars[end_char];
            if matches!(value, ']' | '】' | '\n' | '\r') {
                break;
            }
            end_char += 1;
        }
        let raw = chars[start_char..end_char].iter().collect::<String>();
        if let Some(hint) = non_empty_trimmed_string(&raw) {
            if !hints.contains(&hint) {
                hints.push(hint);
            }
        }
        index = text
            .char_indices()
            .nth(end_char.saturating_add(1))
            .map(|(byte_index, _)| byte_index)
            .unwrap_or_else(|| text.len());
    }
    hints
}

pub(crate) fn external_channel_attachment_title_match_score(
    hint: &str,
    document_title: &str,
) -> Option<i64> {
    let normalized_hint = external_channel_attachment_title_normalized(hint);
    let normalized_title = external_channel_attachment_title_normalized(document_title);
    if normalized_hint.is_empty() || normalized_title.is_empty() {
        return None;
    }
    if let Some(primary_name) = external_channel_attachment_title_primary_cjk_name(hint) {
        if !document_title.contains(&primary_name) {
            return None;
        }
    }
    if normalized_hint == normalized_title {
        return Some(10_000 + normalized_title.chars().count() as i64);
    }
    if normalized_hint.contains(&normalized_title) && normalized_title.chars().count() >= 4 {
        return Some(5_000 + normalized_title.chars().count() as i64);
    }
    if normalized_title.contains(&normalized_hint) && normalized_hint.chars().count() >= 4 {
        return Some(4_000 + normalized_hint.chars().count() as i64);
    }

    let hint_tokens = external_channel_attachment_title_match_tokens(hint);
    let title_tokens = external_channel_attachment_title_match_tokens(document_title);
    let strict_ascii_hint_tokens = hint_tokens
        .iter()
        .filter(|token| {
            token.chars().count() >= 4
                && token.chars().all(|ch| ch.is_ascii_alphanumeric())
                && token.chars().any(|ch| ch.is_ascii_alphabetic())
                && !external_channel_attachment_title_generic_token(token)
        })
        .collect::<Vec<_>>();
    if strict_ascii_hint_tokens.len() >= 2
        && strict_ascii_hint_tokens
            .iter()
            .any(|token| !title_tokens.contains(*token))
    {
        return None;
    }
    let mut score = 0i64;
    let mut has_specific_overlap = false;
    for token in &title_tokens {
        if !hint_tokens.contains(token) {
            continue;
        }
        let token_len = token.chars().count() as i64;
        score += token_len * token_len;
        if !external_channel_attachment_title_generic_token(token) {
            has_specific_overlap = true;
        }
    }
    if has_specific_overlap && score >= 9 {
        Some(score)
    } else {
        None
    }
}

fn external_channel_attachment_title_primary_cjk_name(value: &str) -> Option<String> {
    let mut cjk_run = String::new();
    for ch in value.chars() {
        if is_cjk_query_token_char(ch) {
            cjk_run.push(ch);
            continue;
        }
        if !cjk_run.is_empty() {
            break;
        }
    }
    if cjk_run.chars().count() >= 2
        && !external_channel_attachment_title_generic_token(cjk_run.as_str())
    {
        Some(cjk_run)
    } else {
        None
    }
}

fn external_channel_attachment_title_match_tokens(value: &str) -> BTreeSet<String> {
    lexical_query_tokens(value)
        .into_iter()
        .filter(|token| token.chars().count() >= 2)
        .collect()
}

fn external_channel_attachment_title_generic_token(token: &str) -> bool {
    matches!(
        token,
        "附件"
            | "文件"
            | "文档"
            | "资料"
            | "材料"
            | "简历"
            | "优化"
            | "pdf"
            | "doc"
            | "docx"
            | "xlsx"
            | "xls"
            | "ppt"
            | "pptx"
    )
}

fn external_channel_attachment_title_normalized(value: &str) -> String {
    let mut normalized = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch.to_ascii_lowercase());
        } else if is_cjk_query_token_char(ch) {
            normalized.push(ch);
        }
    }
    for suffix in ["pdf", "docx", "doc", "xlsx", "xls", "pptx", "ppt"] {
        if normalized.ends_with(suffix) {
            let new_len = normalized.len().saturating_sub(suffix.len());
            normalized.truncate(new_len);
            break;
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_message() -> ExternalBotMessageView {
        serde_json::from_value(json!({
            "platform": "generic_chat",
            "tenant_external_id": "tenant-ext-001",
            "bot_external_id": "bot-v3",
            "conversation_external_id": "conv-001",
            "sender_external_id": "user-default",
            "message_external_id": "msg-001",
            "message_type": "text",
            "text": "[附件: 郑宇宁_AI全栈产品技术主管_优化简历.pdf] 分析下简历，列一下时间线。",
            "attachment_refs": [
                {
                    "attachment_external_id": "att-001",
                    "filename": " 郑宇宁_AI全栈产品技术主管_优化简历.pdf "
                },
                {
                    "attachment_external_id": "att-002",
                    "filename": "郑宇宁_AI全栈产品技术主管_优化简历.pdf"
                }
            ],
            "idempotency_key": "third-party:tenant-ext-001:msg-001",
            "received_at": "2026-06-14T00:00:00Z"
        }))
        .expect("sample message")
    }

    #[test]
    fn attachment_title_hints_merge_files_and_inline_markers_without_duplicates() {
        assert_eq!(
            external_channel_attachment_title_hints(&sample_message()),
            vec!["郑宇宁_AI全栈产品技术主管_优化简历.pdf".to_string()]
        );
    }

    #[test]
    fn attachment_title_hints_extract_inline_attachment_markers() {
        let hints = external_channel_attachment_title_hints_from_text(
            "[附件: 郑宇宁_AI全栈产品技术主管_优化简历.pdf] 分析。\n附件：日报.xlsx\n【附件: 合同.docx】",
        );

        assert_eq!(
            hints,
            vec![
                "郑宇宁_AI全栈产品技术主管_优化简历.pdf".to_string(),
                "日报.xlsx".to_string(),
                "合同.docx".to_string(),
            ]
        );
    }

    #[test]
    fn attachment_title_score_matches_resume_title_and_rejects_unrelated_documents() {
        let hint = "郑宇宁_AI全栈产品技术主管_优化简历.pdf";
        let score = external_channel_attachment_title_match_score(hint, "郑宇宁简历.pdf")
            .expect("resume title should match by explicit attachment title tokens");

        assert!(score >= 9);
        assert!(
            external_channel_attachment_title_match_score(hint, "李想周报0518-0522.docx").is_none()
        );
        assert!(external_channel_attachment_title_match_score(
            hint,
            "李越-8年+技术-产品(即做技术又做产品）.pdf",
        )
        .is_none());
    }

    #[test]
    fn attachment_title_score_rejects_strict_ascii_missing_tokens() {
        let smoke_hint = "DataMax Scope Smoke Attachment 20260606094114.md";

        assert!(external_channel_attachment_title_match_score(
            smoke_hint,
            "DataMax Scope Smoke Attachment 20260606094114.md",
        )
        .is_some());
        assert!(external_channel_attachment_title_match_score(
            smoke_hint,
            "DataMax Scope Smoke Extra 20260606094114.md",
        )
        .is_none());
    }
}
