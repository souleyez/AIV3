use crate::assistant_run_text_support::collect_string_list;
use serde_json::{json, Value};

pub(crate) fn static_page_evidence_value_lines(item: &Value) -> Vec<String> {
    let mut text = String::new();
    for key in ["content_excerpt", "summary"] {
        if let Some(value) = item.get(key).and_then(Value::as_str) {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(value);
        }
    }
    text.split(|character| matches!(character, '\n' | '\r' | ';' | '；'))
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub(crate) fn static_page_metric_label_from_line(
    line: &str,
    item: &Value,
    evidence_index: usize,
    line_index: usize,
    keywords: &[&str],
) -> String {
    for part in line.split([',', '，', '|', '\t']) {
        let candidate = part.trim();
        if candidate.is_empty() {
            continue;
        }
        let candidate_lower = candidate.to_lowercase();
        if static_page_text_contains_any(&candidate_lower, keywords) {
            continue;
        }
        if static_page_string_is_numeric_only(candidate) {
            continue;
        }
        return candidate.chars().take(18).collect();
    }

    if line_index == 0 {
        static_page_evidence_point_label(item, evidence_index)
    } else {
        format!(
            "{}-{}",
            static_page_evidence_point_label(item, evidence_index),
            line_index + 1
        )
    }
}

pub(crate) fn static_page_string_is_numeric_only(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return false;
    }
    let numeric_chars = trimmed
        .chars()
        .filter(|character| {
            character.is_ascii_digit()
                || matches!(character, '.' | ',' | '-' | '+' | '%' | ' ' | '万' | '亿')
        })
        .count();
    numeric_chars == trimmed.chars().count()
}

pub(crate) fn static_page_field_keywords(field_path: &str) -> Vec<&'static str> {
    let normalized = field_path.to_ascii_lowercase();
    if normalized.contains("orders.amount") || normalized.contains("revenue") {
        return vec![
            "order", "orders", "amount", "revenue", "sales", "gmv", "订单", "金额", "收入",
        ];
    }
    if normalized.contains("orders.count") || normalized.contains("order_count") {
        return vec!["order", "orders", "count", "volume", "订单", "数量", "单量"];
    }
    if normalized.contains("customer") {
        return vec!["customer", "customers", "client", "客户", "用户"];
    }
    if normalized.contains("profit") || normalized.contains("margin") {
        return vec!["profit", "margin", "gross", "利润", "毛利"];
    }
    if normalized.contains("risk") {
        return vec!["risk", "delay", "warning", "风险", "延期", "预警"];
    }
    if normalized.contains("time") || normalized.contains("month") || normalized.contains("date") {
        return vec![
            "month", "date", "time", "period", "月份", "日期", "时间", "周期",
        ];
    }
    if normalized.contains("engagement") {
        return vec![
            "engagement",
            "newsletter",
            "open",
            "click",
            "触达",
            "互动",
            "打开",
            "点击",
        ];
    }
    Vec::new()
}

pub(crate) fn static_page_keyword_signal_score(item: &Value, keywords: &[&str]) -> f64 {
    let text = static_page_evidence_text(item).to_lowercase();
    let mut score = keywords
        .iter()
        .filter(|keyword| text.contains(&keyword.to_lowercase()))
        .count() as f64;

    if let Some(term_weights) = item
        .get("evidence_manifest")
        .and_then(|manifest| manifest.pointer("/embedding/term_weights"))
        .and_then(Value::as_object)
    {
        for (term, weight) in term_weights {
            if keywords
                .iter()
                .any(|keyword| keyword.eq_ignore_ascii_case(term))
            {
                score += weight.as_f64().unwrap_or(0.0).max(0.0);
            }
        }
    }

    if let Some(score_value) = item.get("score").and_then(Value::as_f64) {
        score += score_value.clamp(0.0, 1.0);
    } else if let Some(recall_score) = item.get("recall_score").and_then(Value::as_f64) {
        score += recall_score.clamp(0.0, 1.0);
    }

    (score * 10.0).round() / 10.0
}

pub(crate) fn static_page_evidence_point_label(item: &Value, index: usize) -> String {
    item.get("source_locator")
        .and_then(Value::as_str)
        .map(|value| {
            value
                .rsplit('/')
                .next()
                .unwrap_or(value)
                .split('#')
                .next()
                .unwrap_or(value)
                .trim()
                .to_string()
        })
        .filter(|value| !value.is_empty())
        .or_else(|| {
            item.get("summary")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.chars().take(18).collect::<String>())
        })
        .unwrap_or_else(|| format!("证据{}", index + 1))
}

pub(crate) fn static_page_evidence_ids(item: &Value) -> Vec<Value> {
    item.get("retrieval_evidence_id")
        .cloned()
        .map(|value| vec![value])
        .unwrap_or_default()
}

pub(crate) fn static_page_evidence_ref(item: &Value) -> Value {
    json!({
        "retrievalEvidenceId": item.get("retrieval_evidence_id").cloned().unwrap_or(Value::Null),
        "datasetId": item.get("dataset_id").cloned().unwrap_or(Value::Null),
        "documentId": item.get("document_id").cloned().unwrap_or(Value::Null),
        "documentChunkId": item.get("document_chunk_id").cloned().unwrap_or(Value::Null),
        "sourceLocator": item.get("source_locator").cloned().unwrap_or(Value::Null),
        "sectionTitleHints": static_page_evidence_section_title_hints(item),
    })
}

pub(crate) fn static_page_evidence_text(item: &Value) -> String {
    let mut parts = Vec::new();
    for key in [
        "summary",
        "content_excerpt",
        "source_locator",
        "payload_filter_key",
    ] {
        if let Some(value) = item.get(key).and_then(Value::as_str) {
            parts.push(value.to_string());
        }
    }
    parts.extend(static_page_evidence_section_title_hints(item));
    if let Some(term_weights) = item
        .get("evidence_manifest")
        .and_then(|manifest| manifest.pointer("/embedding/term_weights"))
        .and_then(Value::as_object)
    {
        parts.extend(term_weights.keys().cloned());
    }
    parts.join(" ")
}

pub(crate) fn static_page_evidence_section_title_hints(item: &Value) -> Vec<String> {
    let mut hints = Vec::new();
    for pointer in [
        "/evidence_manifest/evidence/section_title_hints",
        "/evidence_manifest/section_title_hints",
        "/evidence_manifest/metadata/section_title_hints",
        "/evidence/section_title_hints",
        "/section_title_hints",
        "/metadata/section_title_hints",
    ] {
        if let Some(value) = item.pointer(pointer) {
            collect_string_list(value, &mut hints);
        }
    }
    hints.truncate(6);
    hints
}

pub(crate) fn static_page_text_contains_any(text: &str, keywords: &[&str]) -> bool {
    keywords
        .iter()
        .any(|keyword| text.contains(&keyword.to_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn evidence_value_lines_split_excerpt_and_summary() {
        let item = json!({
            "content_excerpt": "收入 120；风险 2",
            "summary": "客流 30\n门店 A"
        });

        assert_eq!(
            static_page_evidence_value_lines(&item),
            vec![
                "收入 120".to_string(),
                "风险 2".to_string(),
                "客流 30".to_string(),
                "门店 A".to_string()
            ]
        );
    }

    #[test]
    fn metric_label_skips_keywords_and_numeric_parts() {
        let item = json!({
            "source_locator": "s3://bucket/report.xlsx#sheet1"
        });
        let label =
            static_page_metric_label_from_line("收入, 1,234, 门店A", &item, 0, 0, &["收入"]);
        assert_eq!(label, "门店A");

        let fallback = static_page_metric_label_from_line("收入, 1,234", &item, 0, 1, &["收入"]);
        assert_eq!(fallback, "report.xlsx-2");
    }

    #[test]
    fn numeric_only_accepts_common_units_and_rejects_words() {
        assert!(static_page_string_is_numeric_only("+1,234.5 万"));
        assert!(static_page_string_is_numeric_only("-20%"));
        assert!(!static_page_string_is_numeric_only("门店A 20"));
        assert!(!static_page_string_is_numeric_only(""));
    }

    #[test]
    fn field_keywords_cover_core_metric_families() {
        assert!(static_page_field_keywords("orders.amount")
            .iter()
            .any(|keyword| *keyword == "收入"));
        assert!(static_page_field_keywords("risk.level")
            .iter()
            .any(|keyword| *keyword == "预警"));
        assert!(static_page_field_keywords("unknown.field").is_empty());
    }

    #[test]
    fn keyword_signal_score_uses_text_weights_and_score() {
        let item = json!({
            "summary": "收入 revenue",
            "score": 2.0,
            "evidence_manifest": {
                "embedding": {
                    "term_weights": {
                        "revenue": 0.45,
                        "ignored": 100
                    }
                }
            }
        });

        assert_eq!(
            static_page_keyword_signal_score(&item, &["收入", "revenue"]),
            3.5
        );
    }

    #[test]
    fn evidence_ref_collects_section_title_hints_with_limit() {
        let item = json!({
            "retrieval_evidence_id": "ev-1",
            "dataset_id": "ds-1",
            "document_id": "doc-1",
            "document_chunk_id": "chunk-1",
            "source_locator": "docs/a.pdf#p=1",
            "evidence_manifest": {
                "evidence": {
                    "section_title_hints": [" 一 ", ["二", "三", "四", "五", "六", "七"]]
                }
            }
        });

        assert_eq!(
            static_page_evidence_section_title_hints(&item),
            vec![
                "一".to_string(),
                "二".to_string(),
                "三".to_string(),
                "四".to_string(),
                "五".to_string(),
                "六".to_string()
            ]
        );
        assert_eq!(
            static_page_evidence_ref(&item).pointer("/sectionTitleHints/0"),
            Some(&json!("一"))
        );
    }
}
