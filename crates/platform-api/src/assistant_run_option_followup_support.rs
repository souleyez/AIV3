use contracts::AssistantRunMessageView;
use domain_model::ChatMessageRole;

use crate::truncate_assistant_supply_text;

const OPTION_FOLLOWUP_TEXT_LIMIT: usize = 700;

#[derive(Clone, Debug, PartialEq, Eq)]
struct AssistantRunChoiceOption {
    key: String,
    text: String,
}

pub(crate) fn assistant_run_short_option_followup_context(
    prompt: &str,
    messages: &[AssistantRunMessageView],
) -> Option<String> {
    let selected_key = assistant_run_short_selected_option_key(prompt)?;
    let assistant_message = messages
        .iter()
        .rev()
        .filter(|message| message.role == ChatMessageRole::Assistant)
        .find(|message| {
            assistant_run_extract_choice_options(&message.content)
                .iter()
                .any(|option| option.key == selected_key)
        })?;
    let selected_option = assistant_run_extract_choice_options(&assistant_message.content)
        .into_iter()
        .find(|option| option.key == selected_key)?;
    let option_text =
        truncate_assistant_supply_text(&selected_option.text, OPTION_FOLLOWUP_TEXT_LIMIT);

    Some(format!(
        "用户本轮只回复了短选项 `{}`。系统已解析为上一轮助手给出的选项 {}：{}\n请按该选项继续执行或回答；不要把 `{}` 当作孤立问题，也不要重复要求用户选择 A/B/C。",
        prompt.trim(),
        selected_option.key,
        option_text,
        prompt.trim()
    ))
}

fn assistant_run_short_selected_option_key(prompt: &str) -> Option<String> {
    let compact = prompt
        .trim()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    if compact.chars().count() > 8 {
        return None;
    }
    let compact = assistant_run_trim_choice_punctuation(&compact);
    if let Some(key) = assistant_run_normalize_choice_key(&compact) {
        return Some(key);
    }

    let leading_words = [
        "我选", "就选", "选择", "确认", "确定", "按", "用", "选", "第", "要",
    ];
    let trailing_words = ["方案", "选项", "项", "个", "种", "吧", "好了", "即可"];
    for leading in leading_words {
        if let Some(rest) = compact.strip_prefix(leading) {
            let rest = assistant_run_trim_choice_punctuation(rest);
            if let Some(key) = assistant_run_normalize_choice_key(rest) {
                return Some(key);
            }
        }
    }
    for trailing in trailing_words {
        if let Some(rest) = compact.strip_suffix(trailing) {
            let rest = assistant_run_trim_choice_punctuation(rest);
            if let Some(key) = assistant_run_normalize_choice_key(rest) {
                return Some(key);
            }
        }
    }
    for leading in leading_words {
        for trailing in trailing_words {
            if let Some(rest) = compact.strip_prefix(leading) {
                if let Some(rest) = rest.strip_suffix(trailing) {
                    let rest = assistant_run_trim_choice_punctuation(rest);
                    if let Some(key) = assistant_run_normalize_choice_key(rest) {
                        return Some(key);
                    }
                }
            }
        }
    }

    None
}

fn assistant_run_extract_choice_options(content: &str) -> Vec<AssistantRunChoiceOption> {
    let mut options = Vec::new();
    let mut current: Option<AssistantRunChoiceOption> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((key, text)) = assistant_run_parse_choice_option_line(trimmed) {
            if let Some(option) = current.take() {
                if !option.text.trim().is_empty() {
                    options.push(option);
                }
            }
            current = Some(AssistantRunChoiceOption { key, text });
            continue;
        }

        if let Some(option) = current.as_mut() {
            if option.text.chars().count() < OPTION_FOLLOWUP_TEXT_LIMIT {
                option.text.push(' ');
                option.text.push_str(trimmed);
            }
        }
    }

    if let Some(option) = current {
        if !option.text.trim().is_empty() {
            options.push(option);
        }
    }

    options
}

fn assistant_run_parse_choice_option_line(line: &str) -> Option<(String, String)> {
    let mut value = line.trim_start();
    value = value
        .strip_prefix("- ")
        .or_else(|| value.strip_prefix("* "))
        .or_else(|| value.strip_prefix("• "))
        .unwrap_or(value)
        .trim_start();
    value = value.strip_prefix("**").unwrap_or(value).trim_start();
    value = value.strip_prefix("选项").unwrap_or(value).trim_start();

    if let Some(rest) = value.strip_prefix("方案") {
        let (key, text) = assistant_run_parse_choice_key_and_text(rest.trim_start())?;
        return Some((key, text));
    }

    assistant_run_parse_choice_key_and_text(value)
}

fn assistant_run_parse_choice_key_and_text(value: &str) -> Option<(String, String)> {
    let mut chars = value.chars();
    let key_char = chars.next()?;
    let key = assistant_run_normalize_choice_key_char(key_char)?;
    let rest = chars.as_str().trim_start();
    let rest = rest.strip_prefix("**").unwrap_or(rest).trim_start();
    let text = assistant_run_strip_choice_separator(rest)?;
    let text = text.trim().trim_start_matches("**").trim().to_string();
    if text.is_empty() {
        return None;
    }
    Some((key, text))
}

fn assistant_run_strip_choice_separator(value: &str) -> Option<&str> {
    let mut chars = value.chars();
    let first = chars.next()?;
    if matches!(
        first,
        '.' | '。' | '、' | ')' | '）' | ':' | '：' | '-' | '—' | '/'
    ) {
        return Some(chars.as_str());
    }
    None
}

fn assistant_run_normalize_choice_key(value: &str) -> Option<String> {
    let mut chars = value.chars();
    let first = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    assistant_run_normalize_choice_key_char(first)
}

fn assistant_run_normalize_choice_key_char(value: char) -> Option<String> {
    match value {
        'A'..='F' | 'a'..='f' => Some(value.to_ascii_uppercase().to_string()),
        '1'..='6' => Some(value.to_string()),
        _ => None,
    }
}

fn assistant_run_trim_choice_punctuation(value: &str) -> &str {
    value.trim_matches(|ch: char| {
        matches!(
            ch,
            '。' | '！'
                | '!'
                | '？'
                | '?'
                | ','
                | '，'
                | '、'
                | ':'
                | '：'
                | ';'
                | '；'
                | '.'
                | '('
                | ')'
                | '（'
                | '）'
                | '['
                | ']'
                | '【'
                | '】'
                | '"'
                | '\''
                | '“'
                | '”'
                | '`'
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assistant(content: &str) -> AssistantRunMessageView {
        AssistantRunMessageView {
            role: ChatMessageRole::Assistant,
            content: content.to_string(),
        }
    }

    #[test]
    fn short_option_followup_context_maps_bare_letter_to_previous_assistant_option() {
        let messages = vec![assistant(
            "请选择下一步：\nA. 只继续问答\nB. 生成 Markdown 表格\nC. 生成一张经营分析报表页面",
        )];

        let context = assistant_run_short_option_followup_context("C", &messages)
            .expect("bare C should map to option C");

        assert!(context.contains("选项 C"));
        assert!(context.contains("生成一张经营分析报表页面"));
        assert!(context.contains("不要把 `C` 当作孤立问题"));
    }

    #[test]
    fn short_option_followup_context_accepts_chinese_selection_phrasing() {
        let messages = vec![assistant("A、继续解释\nB、生成清单\nC、按现有产物修改页面")];

        let context = assistant_run_short_option_followup_context("我选 C 方案", &messages)
            .expect("Chinese selection phrasing should map to option C");

        assert!(context.contains("按现有产物修改页面"));
    }

    #[test]
    fn short_option_followup_context_supports_numbered_options() {
        let messages = vec![assistant("1）摘要\n2）表格\n3）报表页面")];

        let context = assistant_run_short_option_followup_context("第3项", &messages)
            .expect("numbered selection should map to option 3");

        assert!(context.contains("选项 3"));
        assert!(context.contains("报表页面"));
    }

    #[test]
    fn short_option_followup_context_ignores_plain_followup_without_matching_options() {
        let messages = vec![assistant("这是一段普通回复，没有选项。")];

        assert!(assistant_run_short_option_followup_context("C", &messages).is_none());
        assert!(assistant_run_short_option_followup_context("继续生成报表", &messages).is_none());
    }
}
