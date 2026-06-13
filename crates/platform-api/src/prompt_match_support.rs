pub(crate) fn prompt_contains_any(prompt: &str, hints: &[&str]) -> bool {
    hints.iter().any(|hint| prompt.contains(hint))
}

pub(crate) fn ascii_prompt_contains_any(lower_prompt: &str, hints: &[&str]) -> bool {
    hints.iter().any(|hint| {
        if hint.contains(' ') {
            lower_prompt.contains(hint)
        } else {
            lower_prompt
                .split(|ch: char| !ch.is_ascii_alphanumeric())
                .any(|token| token == *hint)
        }
    })
}

pub(crate) fn prompt_has_any(prompt: &str, needles: &[&str]) -> bool {
    let lower = prompt.to_ascii_lowercase();
    needles.iter().any(|needle| {
        let needle = needle.to_ascii_lowercase();
        !needle.is_empty() && lower.contains(&needle)
    })
}

pub(crate) fn prompt_has_metric_terms(prompt: &str, needles: &[&str]) -> bool {
    let lower = prompt.to_ascii_lowercase();
    needles.iter().any(|needle| {
        let needle = needle.to_ascii_lowercase();
        if needle.is_empty() {
            return false;
        }
        if needle
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            return contains_ascii_token(&lower, &needle);
        }
        lower.contains(&needle)
    })
}

pub(crate) fn contains_ascii_token(haystack: &str, needle: &str) -> bool {
    haystack.match_indices(needle).any(|(index, _)| {
        let before = haystack[..index].chars().next_back();
        let after = haystack[index + needle.len()..].chars().next();
        !before.is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
            && !after.is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_contains_any_keeps_case_sensitive_substring_semantics() {
        assert!(prompt_contains_any("生成经营报表", &["经营", "风险"]));
        assert!(!prompt_contains_any("Generate report", &["generate"]));
        assert!(!prompt_contains_any("没有目标", &[]));
    }

    #[test]
    fn ascii_prompt_contains_any_matches_tokens_or_phrase() {
        let lower = "show risk report and static page status";
        assert!(ascii_prompt_contains_any(lower, &["risk"]));
        assert!(ascii_prompt_contains_any(lower, &["static page"]));
        assert!(!ascii_prompt_contains_any(
            "static-page status",
            &["static-page"]
        ));
        assert!(!ascii_prompt_contains_any("markdown table", &["down"]));
    }

    #[test]
    fn prompt_has_any_is_case_insensitive_substring() {
        assert!(prompt_has_any("Monthly SALES Report", &["sales"]));
        assert!(prompt_has_any("经营健康度", &["健康"]));
        assert!(!prompt_has_any("abc", &[""]));
    }

    #[test]
    fn metric_terms_use_ascii_token_boundaries_for_ascii_needles() {
        assert!(prompt_has_metric_terms("rank by down", &["down"]));
        assert!(prompt_has_metric_terms("统计下行流量", &["下行"]));
        assert!(!prompt_has_metric_terms("Markdown table", &["down"]));
        assert!(!contains_ascii_token("sales_count", "sales"));
        assert!(contains_ascii_token("sales-count", "sales"));
    }
}
