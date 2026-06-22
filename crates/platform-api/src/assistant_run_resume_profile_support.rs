use serde_json::Value;
use std::cmp::Ordering;

use crate::{degree_rank, escape_markdown_table_cell, normalize_document_entity_value};

type ResumeProfileGenderCounts = (usize, usize, usize);

pub(crate) fn assistant_run_resume_profile_table<F>(
    rows: &[Value],
    title: &str,
    headers: &[&str],
    row_values: F,
) -> String
where
    F: Fn(&Value) -> Vec<String>,
{
    let mut lines = vec![
        format!("{title}（共 {} 份可见简历）：", rows.len()),
        String::new(),
    ];
    lines.push(format!("| {} |", headers.join(" | ")));
    lines.push(format!(
        "| {} |",
        headers
            .iter()
            .map(|_| "---")
            .collect::<Vec<_>>()
            .join(" | ")
    ));
    for row in rows {
        lines.push(format!(
            "| {} |",
            row_values(row)
                .into_iter()
                .map(|value| escape_markdown_table_cell(&value))
                .collect::<Vec<_>>()
                .join(" | ")
        ));
    }
    lines.join("\n")
}

pub(crate) fn resume_profile_candidate_name(row: &Value) -> String {
    value_string(row, "candidate_name")
}

pub(crate) fn resume_profile_array_values(row: &Value, key: &str) -> Vec<String> {
    row.get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(normalize_document_entity_value)
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub(crate) fn resume_profile_array_string(row: &Value, key: &str, limit: usize) -> String {
    let values = resume_profile_array_values(row, key);
    if values.is_empty() {
        "-".to_string()
    } else {
        values
            .into_iter()
            .take(limit)
            .collect::<Vec<_>>()
            .join("；")
    }
}

pub(crate) fn value_string(row: &Value, key: &str) -> String {
    row.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("-")
        .to_string()
}

pub(crate) fn value_u64_string(row: &Value, key: &str) -> String {
    row.get(key)
        .and_then(Value::as_u64)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string())
}

pub(crate) fn value_i64_string(row: &Value, key: &str) -> String {
    row.get(key)
        .and_then(Value::as_i64)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string())
}

pub(crate) fn compare_resume_profile_u64_field(
    left: &Value,
    right: &Value,
    key: &str,
    ascending: bool,
) -> Ordering {
    compare_resume_profile_u64_options(
        left.get(key).and_then(Value::as_u64),
        right.get(key).and_then(Value::as_u64),
        ascending,
    )
}

pub(crate) fn compare_resume_profile_i64_field(
    left: &Value,
    right: &Value,
    key: &str,
    ascending: bool,
) -> Ordering {
    compare_resume_profile_i64_options(
        left.get(key).and_then(Value::as_i64),
        right.get(key).and_then(Value::as_i64),
        ascending,
    )
}

pub(crate) fn compare_resume_profile_u64_options(
    left: Option<u64>,
    right: Option<u64>,
    ascending: bool,
) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => {
            if ascending {
                left.cmp(&right)
            } else {
                right.cmp(&left)
            }
        }
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

pub(crate) fn compare_resume_profile_i64_options(
    left: Option<i64>,
    right: Option<i64>,
    ascending: bool,
) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => {
            if ascending {
                left.cmp(&right)
            } else {
                right.cmp(&left)
            }
        }
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

pub(crate) fn resume_profile_gender_sort_rank(row: &Value) -> usize {
    match row.get("gender").and_then(Value::as_str).map(str::trim) {
        Some("男") => 0,
        Some("女") => 1,
        Some(value) if !value.is_empty() => 2,
        _ => 3,
    }
}

pub(crate) fn resume_profile_gender_counts(rows: &[Value]) -> ResumeProfileGenderCounts {
    let mut male_count = 0;
    let mut female_count = 0;
    let mut unknown_count = 0;
    for row in rows {
        match row.get("gender").and_then(Value::as_str).map(str::trim) {
            Some("男") => male_count += 1,
            Some("女") => female_count += 1,
            _ => unknown_count += 1,
        }
    }
    (male_count, female_count, unknown_count)
}

pub(crate) fn resume_profile_year_span(row: &Value) -> Option<i64> {
    let earliest_year = row.get("earliest_year").and_then(Value::as_i64)?;
    let latest_year = row.get("latest_year").and_then(Value::as_i64)?;
    (latest_year >= earliest_year).then_some(latest_year - earliest_year + 1)
}

pub(crate) fn resume_profile_year_span_string(row: &Value) -> String {
    resume_profile_year_span(row)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string())
}

pub(crate) fn resume_profile_degree_rank(row: &Value) -> Option<i64> {
    resume_profile_array_values(row, "degree_names")
        .into_iter()
        .filter_map(|degree| degree_rank(&degree))
        .max()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resume_profile_table_escapes_cells_and_formats_missing_values() {
        let rows = vec![json!({
            "candidate_name": "张|三",
            "skill_names": [" Rust ", "", "React"],
            "age": 34
        })];

        let table = assistant_run_resume_profile_table(
            &rows,
            "候选人",
            &["姓名", "技能", "年龄", "文档"],
            |row| {
                vec![
                    resume_profile_candidate_name(row),
                    resume_profile_array_string(row, "skill_names", 3),
                    value_u64_string(row, "age"),
                    value_string(row, "document_title"),
                ]
            },
        );

        assert!(table.contains("候选人（共 1 份可见简历）："));
        assert!(table.contains("张\\|三"));
        assert!(table.contains("Rust；React"));
        assert!(table.contains("| 张\\|三 | Rust；React | 34 | - |"));
    }

    #[test]
    fn resume_profile_compare_helpers_keep_missing_values_last() {
        let older = json!({"latest_year": 2020, "skill_count": 2});
        let newer = json!({"latest_year": 2024, "skill_count": 5});
        let missing = json!({});

        assert_eq!(
            compare_resume_profile_u64_field(&older, &newer, "skill_count", false),
            Ordering::Greater
        );
        assert_eq!(
            compare_resume_profile_u64_field(&newer, &older, "skill_count", false),
            Ordering::Less
        );
        assert_eq!(
            compare_resume_profile_i64_field(&older, &newer, "latest_year", true),
            Ordering::Less
        );
        assert_eq!(
            compare_resume_profile_i64_field(&missing, &newer, "latest_year", true),
            Ordering::Greater
        );
    }

    #[test]
    fn resume_profile_distribution_and_rank_helpers_preserve_semantics() {
        let rows = vec![
            json!({"gender": "男", "earliest_year": 2020, "latest_year": 2024, "degree_names": ["本科"]}),
            json!({"gender": "女", "degree_names": ["硕士"]}),
            json!({"gender": "未知"}),
            json!({}),
        ];

        let counts: ResumeProfileGenderCounts = resume_profile_gender_counts(&rows);
        assert_eq!(counts, (1, 1, 2));
        assert_eq!(resume_profile_gender_sort_rank(&rows[0]), 0);
        assert_eq!(resume_profile_year_span(&rows[0]), Some(5));
        assert_eq!(resume_profile_year_span_string(&rows[1]), "-");
        assert!(resume_profile_degree_rank(&rows[1]) > resume_profile_degree_rank(&rows[0]));
    }
}
