use crate::external_channel_text_has_any;

pub(crate) fn external_static_page_prompt_contains_any(prompt: &str, needles: &[&str]) -> bool {
    let normalized = prompt
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    external_channel_text_has_any(&normalized, prompt, needles)
}

pub(crate) fn external_channel_static_page_project_name(prompt: &str) -> Option<String> {
    for marker in ["项目", "门店", "商场", "客户"] {
        if let Some(index) = prompt.find(marker) {
            let prefix = prompt[..index].trim();
            let value = prefix
                .split(['，', '。', ',', '.', '\n', '\r'])
                .next_back()
                .unwrap_or(prefix)
                .trim();
            if !value.is_empty() && value.chars().count() <= 32 {
                return Some(value.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_contains_any_matches_ascii_case_and_whitespace_insensitively() {
        assert!(external_static_page_prompt_contains_any(
            "请生成 SALES   GAP  静态页",
            &["salesgap"]
        ));
        assert!(external_static_page_prompt_contains_any(
            "请生成 Sales Gap 静态页",
            &["SALESgap"]
        ));
    }

    #[test]
    fn prompt_contains_any_matches_non_ascii_against_original_prompt() {
        assert!(external_static_page_prompt_contains_any(
            "按区域和门店生成经营报表",
            &["门店", "总部"]
        ));
        assert!(!external_static_page_prompt_contains_any(
            "按区域生成经营报表",
            &["门店", "总部"]
        ));
    }

    #[test]
    fn project_name_uses_text_before_marker_from_latest_sentence_segment() {
        assert_eq!(
            external_channel_static_page_project_name("请给新世界项目做经营分析页"),
            Some("请给新世界".to_string())
        );
        assert_eq!(
            external_channel_static_page_project_name("先看数据，给新百门店做取高报表"),
            Some("给新百".to_string())
        );
    }

    #[test]
    fn project_name_rejects_empty_or_too_long_prefixes() {
        assert_eq!(
            external_channel_static_page_project_name("项目经营分析"),
            None
        );
        assert_eq!(
            external_channel_static_page_project_name(
                "这是一个特别长特别长特别长特别长特别长特别长特别长特别长超过三十二字符项目"
            ),
            None
        );
    }
}
