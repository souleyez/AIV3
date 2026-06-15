pub(crate) fn static_page_metric_value_from_line(line: &str) -> Option<f64> {
    let candidates = static_page_number_candidates(line);
    if candidates.is_empty() {
        return None;
    }
    if candidates.len() == 1 {
        let value = candidates.first().copied()?;
        if static_page_number_looks_like_year(value)
            || (static_page_number_looks_like_period_part(value)
                && static_page_line_mentions_period(line))
        {
            return None;
        }
        return Some(value);
    }

    candidates
        .iter()
        .copied()
        .filter(|value| !static_page_number_looks_like_year(*value))
        .next_back()
        .or_else(|| candidates.last().copied())
}

fn static_page_number_candidates(line: &str) -> Vec<f64> {
    let mut values = Vec::new();
    let mut token = String::new();
    for character in line.chars().chain(std::iter::once(' ')) {
        let can_continue_number = character.is_ascii_digit()
            || character == '.'
            || character == ','
            || ((character == '-' || character == '+') && token.is_empty());
        if can_continue_number {
            token.push(character);
            continue;
        }
        if token.chars().any(|candidate| candidate.is_ascii_digit()) {
            let normalized = token.replace(',', "");
            if let Ok(value) = normalized.parse::<f64>() {
                values.push(value);
            }
        }
        token.clear();
    }
    values
}

fn static_page_number_looks_like_year(value: f64) -> bool {
    (1900.0..=2100.0).contains(&value) && value.fract().abs() < f64::EPSILON
}

fn static_page_number_looks_like_period_part(value: f64) -> bool {
    (1.0..=31.0).contains(&value) && value.fract().abs() < f64::EPSILON
}

fn static_page_line_mentions_period(line: &str) -> bool {
    let lower = line.to_lowercase();
    ["月", "日", "date", "month", "period", "季度", "周"]
        .iter()
        .any(|marker| lower.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metric_value_support_reads_single_non_period_number() {
        assert_eq!(
            static_page_metric_value_from_line("本月销售额 1,234.50 万元"),
            Some(1234.5)
        );
        assert_eq!(
            static_page_metric_value_from_line("增长率 -12.5%"),
            Some(-12.5)
        );
    }

    #[test]
    fn metric_value_support_rejects_standalone_year_or_period_parts() {
        assert_eq!(static_page_metric_value_from_line("2026"), None);
        assert_eq!(static_page_metric_value_from_line("6月"), None);
        assert_eq!(static_page_metric_value_from_line("date 15"), None);
        assert_eq!(static_page_metric_value_from_line("第 3 周"), None);
    }

    #[test]
    fn metric_value_support_prefers_last_non_year_candidate() {
        assert_eq!(
            static_page_metric_value_from_line("2026 年收入 88.6，去年 72.4"),
            Some(72.4)
        );
        assert_eq!(
            static_page_metric_value_from_line("2025/2026 计划完成 93"),
            Some(93.0)
        );
    }

    #[test]
    fn metric_value_support_handles_signed_and_comma_values() {
        assert_eq!(
            static_page_metric_value_from_line("差额 -1,001.25 / 取高线 3,000"),
            Some(3000.0)
        );
        assert_eq!(
            static_page_metric_value_from_line("计划 +200 实际 +220"),
            Some(220.0)
        );
    }
}
