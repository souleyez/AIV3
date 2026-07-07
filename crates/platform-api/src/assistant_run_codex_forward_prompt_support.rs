use crate::prompt_match_support::{ascii_prompt_contains_any, prompt_contains_any};

pub(crate) fn assistant_run_prompt_requests_codex_forward(prompt: &str) -> bool {
    let trimmed = prompt.trim_start();
    if trimmed.eq_ignore_ascii_case("cc") {
        return true;
    }
    let lower = trimmed.to_ascii_lowercase();
    lower.starts_with("cc ")
        || lower.starts_with("cc:")
        || lower.starts_with("cc：")
        || lower.starts_with("cc,")
        || lower.starts_with("cc，")
        || lower.starts_with("cc.")
        || lower.starts_with("cc。")
        || lower.starts_with("cc;")
        || lower.starts_with("cc；")
        || lower.starts_with("cc-")
        || lower.starts_with("cc\n")
        || lower.starts_with("cc\t")
}

pub(crate) fn assistant_run_prompt_without_codex_forward_prefix(prompt: &str) -> &str {
    let trimmed = prompt.trim_start();
    if !assistant_run_prompt_requests_codex_forward(trimmed) {
        return prompt;
    }
    if trimmed.eq_ignore_ascii_case("cc") {
        return "";
    }
    trimmed[2..].trim_start_matches(|ch: char| {
        ch.is_whitespace() || matches!(ch, ':' | '：' | ',' | '，' | '.' | '。' | ';' | '；' | '-')
    })
}

pub(crate) fn assistant_run_prompt_explicit_customer_codex_signal(
    compact: &str,
    lower: &str,
) -> bool {
    prompt_contains_any(
        compact,
        &[
            "用Codex",
            "让Codex",
            "走Codex",
            "Codex执行",
            "codex执行器",
            "复杂需求",
            "复杂任务",
            "客户复杂需求",
            "执行器处理",
            "执行计划",
        ],
    ) || ascii_prompt_contains_any(lower, &["codex"])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_short_trigger_boundaries() {
        assert!(assistant_run_prompt_requests_codex_forward("cc"));
        assert!(assistant_run_prompt_requests_codex_forward(" CC 帮我处理"));
        assert!(assistant_run_prompt_requests_codex_forward("cc: 帮我处理"));
        assert!(assistant_run_prompt_requests_codex_forward("cc：帮我处理"));
        assert!(assistant_run_prompt_requests_codex_forward("cc，帮我处理"));
        assert!(assistant_run_prompt_requests_codex_forward("cc. 帮我处理"));
        assert!(assistant_run_prompt_requests_codex_forward("cc；帮我处理"));
        assert!(assistant_run_prompt_requests_codex_forward("cc-帮我处理"));
        assert!(assistant_run_prompt_requests_codex_forward("cc\n帮我处理"));
        assert!(assistant_run_prompt_requests_codex_forward("cc\t帮我处理"));
    }

    #[test]
    fn rejects_embedded_or_longer_words() {
        assert!(!assistant_run_prompt_requests_codex_forward(
            "account cc 帮我处理"
        ));
        assert!(!assistant_run_prompt_requests_codex_forward(
            "cc123 帮我处理"
        ));
        assert!(!assistant_run_prompt_requests_codex_forward("c c 帮我处理"));
        assert!(!assistant_run_prompt_requests_codex_forward(""));
    }

    #[test]
    fn removes_trigger_prefix_and_separators() {
        assert_eq!(
            assistant_run_prompt_without_codex_forward_prefix("cc，帮我处理"),
            "帮我处理"
        );
        assert_eq!(
            assistant_run_prompt_without_codex_forward_prefix(" CC: 帮我处理"),
            "帮我处理"
        );
        assert_eq!(
            assistant_run_prompt_without_codex_forward_prefix("cc\n帮我处理"),
            "帮我处理"
        );
        assert_eq!(assistant_run_prompt_without_codex_forward_prefix("cc"), "");
    }

    #[test]
    fn keeps_non_trigger_prompt_unchanged() {
        assert_eq!(
            assistant_run_prompt_without_codex_forward_prefix("account cc 帮我处理"),
            "account cc 帮我处理"
        );
        assert_eq!(
            assistant_run_prompt_without_codex_forward_prefix("  不是转发"),
            "  不是转发"
        );
    }

    #[test]
    fn explicit_customer_codex_signal_matches_cn_and_ascii_tokens() {
        let compact = "让Codex执行这个复杂任务";
        let lower = compact.to_ascii_lowercase();
        assert!(assistant_run_prompt_explicit_customer_codex_signal(
            compact, &lower
        ));

        let compact = "please use codex for this customer artifact";
        let lower = compact.to_ascii_lowercase();
        assert!(assistant_run_prompt_explicit_customer_codex_signal(
            compact, &lower
        ));
    }

    #[test]
    fn explicit_customer_codex_signal_uses_ascii_token_boundary() {
        let compact = "codex执行器处理";
        let lower = compact.to_ascii_lowercase();
        assert!(assistant_run_prompt_explicit_customer_codex_signal(
            compact, &lower
        ));

        let compact = "codec pipeline";
        let lower = compact.to_ascii_lowercase();
        assert!(!assistant_run_prompt_explicit_customer_codex_signal(
            compact, &lower
        ));
    }
}
