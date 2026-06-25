use serde_json::Value;

pub(crate) fn collect_string_list(value: &Value, output: &mut Vec<String>) {
    match value {
        Value::String(text) => {
            push_string_hint(output, text);
        }
        Value::Array(items) => {
            for item in items {
                collect_string_list(item, output);
            }
        }
        _ => {}
    }
}

pub(crate) fn push_string_hint(output: &mut Vec<String>, text: impl AsRef<str>) {
    let normalized = text.as_ref().trim().chars().take(80).collect::<String>();
    if !normalized.is_empty() && !output.iter().any(|existing| existing == &normalized) {
        output.push(normalized);
    }
}

pub(crate) fn truncate_assistant_supply_text(value: &str, max_chars: usize) -> String {
    value
        .trim()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(max_chars)
        .collect()
}

pub(crate) fn truncate_assistant_context_text(value: &str, max_chars: usize) -> String {
    let mut output = String::new();
    let mut previous_blank = false;
    for line in value.trim().lines() {
        let normalized_line = line.split_whitespace().collect::<Vec<_>>().join(" ");
        let trimmed = normalized_line.trim();
        if trimmed.is_empty() {
            if !output.is_empty() && !previous_blank {
                output.push('\n');
                previous_blank = true;
            }
            continue;
        }
        if !output.is_empty() && !output.ends_with('\n') {
            output.push('\n');
        }
        output.push_str(trimmed);
        previous_blank = false;
    }

    output.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn push_string_hint_trims_dedupes_and_limits_length() {
        let mut output = Vec::new();
        push_string_hint(&mut output, "  门店销售  ");
        push_string_hint(&mut output, "门店销售");
        push_string_hint(&mut output, "");
        push_string_hint(&mut output, "a".repeat(90));

        assert_eq!(output.len(), 2);
        assert_eq!(output[0], "门店销售");
        assert_eq!(output[1].chars().count(), 80);
    }

    #[test]
    fn collect_string_list_recurses_arrays_and_ignores_non_strings() {
        let mut output = Vec::new();
        collect_string_list(
            &json!([" 分区 ", ["门店", 42, {"ignored": "品牌"}], "门店"]),
            &mut output,
        );

        assert_eq!(output, vec!["分区".to_string(), "门店".to_string()]);
    }

    #[test]
    fn truncate_assistant_supply_text_normalizes_whitespace_and_caps_chars() {
        assert_eq!(truncate_assistant_supply_text("  A\tB\nC  ", 4), "A B ");
        assert_eq!(
            truncate_assistant_supply_text("新世界 百货 经营 管理", 5),
            "新世界 百"
        );
    }

    #[test]
    fn truncate_assistant_context_text_preserves_line_structure() {
        let text = "  请选择：\nA. 继续问答\n\nB. 生成表格\t\nC. 生成报表页面  ";

        assert_eq!(
            truncate_assistant_context_text(text, 80),
            "请选择：\nA. 继续问答\nB. 生成表格\nC. 生成报表页面"
        );
    }
}
