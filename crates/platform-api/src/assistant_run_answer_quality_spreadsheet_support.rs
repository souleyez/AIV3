use std::collections::BTreeSet;

use contracts::CreateAssistantRunRequest;
use serde_json::{json, Value};

use crate::{
    assistant_run_answer_quality_judge_support::assistant_run_answer_contains_insufficient_evidence_marker,
    assistant_run_prompt_request_support::prompt_requests_spreadsheet_row_level_analysis,
    assistant_run_react_output_contains_internal_marker, assistant_run_request_wants_json_output,
    prompt_match_support::prompt_contains_any,
};

pub(crate) fn assistant_run_answer_quality_spreadsheet_controlled_answer(
    evidence_state: &Value,
    request: &CreateAssistantRunRequest,
) -> Option<String> {
    if !prompt_requests_spreadsheet_row_level_analysis(&request.prompt) {
        return None;
    }
    let rows = assistant_run_spreadsheet_row_analysis_rows(evidence_state)?;
    let requests_absence = prompt_contains_any(&request.prompt, &["缺勤", "未打卡", "没打卡"]);
    let requests_work_hours =
        prompt_contains_any(&request.prompt, &["最长", "最短", "长短", "工时"]);
    let mut absence_rows = rows
        .iter()
        .copied()
        .filter(|row| row.get("category").is_none())
        .collect::<Vec<_>>();
    absence_rows.sort_by(|left, right| {
        assistant_run_spreadsheet_row_text(right, "date")
            .cmp(assistant_run_spreadsheet_row_text(left, "date"))
            .then_with(|| {
                assistant_run_spreadsheet_row_text(left, "employee")
                    .cmp(assistant_run_spreadsheet_row_text(right, "employee"))
            })
    });
    let longest = rows
        .iter()
        .copied()
        .find(|row| row.get("category").and_then(Value::as_str) == Some("longest"));
    let shortest = rows
        .iter()
        .copied()
        .find(|row| row.get("category").and_then(Value::as_str) == Some("shortest"));
    if (!requests_absence || absence_rows.is_empty())
        && (!requests_work_hours || (longest.is_none() && shortest.is_none()))
    {
        return None;
    }
    if assistant_run_request_wants_json_output(request) {
        let absence_json = if requests_absence {
            absence_rows
                .iter()
                .take(20)
                .map(|row| {
                    let category = if row
                        .get("counts_as_absence")
                        .and_then(Value::as_bool)
                        .unwrap_or(true)
                    {
                        "缺勤/未打卡"
                    } else {
                        "未打卡/不考勤"
                    };
                    json!({
                        "category": category,
                        "date": assistant_run_spreadsheet_row_text(row, "date"),
                        "employee": assistant_run_spreadsheet_row_text(row, "employee"),
                        "shift": assistant_run_spreadsheet_row_text(row, "shift"),
                        "status": assistant_run_spreadsheet_row_text(row, "status"),
                    })
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let mut work_hour_extremes = Vec::new();
        if requests_work_hours {
            if let Some(row) = longest {
                work_hour_extremes.push(assistant_run_spreadsheet_work_hour_result_json(
                    "longest",
                    "最长工时",
                    row,
                ));
            }
            if let Some(row) = shortest {
                work_hour_extremes.push(assistant_run_spreadsheet_work_hour_result_json(
                    "shortest",
                    "最短工时",
                    row,
                ));
            }
        }
        return serde_json::to_string_pretty(&json!({
            "status": "answered",
            "source": "spreadsheet_row_analysis",
            "question": request.prompt.trim(),
            "documents": assistant_run_spreadsheet_row_analysis_document_titles(evidence_state),
            "absence_rows": absence_json,
            "work_hour_extremes": work_hour_extremes,
        }))
        .ok();
    }

    let mut lines = vec!["根据已解析的考勤明细，结果如下：".to_string()];
    if requests_absence && !absence_rows.is_empty() {
        lines.push(String::new());
        lines.push("| 类别 | 日期 | 员工 | 班次 | 状态 |".to_string());
        lines.push("|---|---|---|---|---|".to_string());
        for row in absence_rows.into_iter().take(20) {
            let category = if row
                .get("counts_as_absence")
                .and_then(Value::as_bool)
                .unwrap_or(true)
            {
                "缺勤/未打卡"
            } else {
                "未打卡/不考勤"
            };
            lines.push(format!(
                "| {category} | {} | {} | {} | {} |",
                assistant_run_spreadsheet_row_text(row, "date"),
                assistant_run_spreadsheet_row_text(row, "employee"),
                assistant_run_spreadsheet_row_text(row, "shift"),
                assistant_run_spreadsheet_row_text(row, "status"),
            ));
        }
    }
    if requests_work_hours && (longest.is_some() || shortest.is_some()) {
        lines.push(String::new());
        lines.push("| 类型 | 日期 | 员工 | 班次 | 工时 | 状态 |".to_string());
        lines.push("|---|---|---|---|---|---|".to_string());
        if let Some(row) = longest {
            lines.push(assistant_run_spreadsheet_work_hour_result_row(
                "最长工时",
                row,
            ));
        }
        if let Some(row) = shortest {
            lines.push(assistant_run_spreadsheet_work_hour_result_row(
                "最短工时",
                row,
            ));
        }
    }
    let titles = assistant_run_spreadsheet_row_analysis_document_titles(evidence_state);
    if !titles.is_empty() {
        lines.push(String::new());
        lines.push(format!("来源：{}", titles.join("、")));
    }
    Some(lines.join("\n"))
}

fn assistant_run_spreadsheet_work_hour_result_row(label: &str, row: &Value) -> String {
    format!(
        "| {label} | {} | {} | {} | {} | {} |",
        assistant_run_spreadsheet_row_text(row, "date"),
        assistant_run_spreadsheet_row_text(row, "employee"),
        assistant_run_spreadsheet_row_text(row, "shift"),
        assistant_run_spreadsheet_row_text(row, "work_hours_text"),
        assistant_run_spreadsheet_row_text(row, "status"),
    )
}

fn assistant_run_spreadsheet_work_hour_result_json(kind: &str, label: &str, row: &Value) -> Value {
    json!({
        "kind": kind,
        "label": label,
        "date": assistant_run_spreadsheet_row_text(row, "date"),
        "employee": assistant_run_spreadsheet_row_text(row, "employee"),
        "shift": assistant_run_spreadsheet_row_text(row, "shift"),
        "work_hours_text": assistant_run_spreadsheet_row_text(row, "work_hours_text"),
        "status": assistant_run_spreadsheet_row_text(row, "status"),
    })
}

fn assistant_run_spreadsheet_row_text<'a>(row: &'a Value, key: &str) -> &'a str {
    row.get(key).and_then(Value::as_str).unwrap_or("")
}

fn assistant_run_spreadsheet_row_analysis_document_titles(evidence_state: &Value) -> Vec<String> {
    evidence_state
        .get("supplied_items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("spreadsheet_row_analysis"))
        .flat_map(|item| {
            item.get("documents")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .filter_map(|document| document.get("title").and_then(Value::as_str))
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(ToOwned::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(crate) fn assistant_run_answer_satisfies_spreadsheet_row_analysis(
    output_text: &str,
    request: &CreateAssistantRunRequest,
    evidence_state: &Value,
) -> bool {
    if !prompt_requests_spreadsheet_row_level_analysis(&request.prompt) {
        return false;
    }
    if assistant_run_answer_contains_insufficient_evidence_marker(output_text)
        || assistant_run_react_output_contains_internal_marker(output_text)
    {
        return false;
    }
    let Some(rows) = assistant_run_spreadsheet_row_analysis_rows(evidence_state) else {
        return false;
    };
    if !output_text.contains('|') {
        return false;
    }

    let requests_absence = prompt_contains_any(&request.prompt, &["缺勤", "未打卡", "没打卡"]);
    let requests_work_hours =
        prompt_contains_any(&request.prompt, &["最长", "最短", "长短", "工时"]);
    let mut saw_absence_answer = !requests_absence;
    let mut saw_longest_answer = !requests_work_hours;
    let mut saw_shortest_answer = !requests_work_hours;

    for row in rows {
        match row.get("category").and_then(Value::as_str) {
            Some("longest") => {
                saw_longest_answer = assistant_run_answer_contains_row_terms(
                    output_text,
                    row,
                    &["最长", "最长工时"],
                );
            }
            Some("shortest") => {
                saw_shortest_answer = assistant_run_answer_contains_row_terms(
                    output_text,
                    row,
                    &["最短", "最短工时"],
                );
            }
            _ if requests_absence && !saw_absence_answer => {
                saw_absence_answer =
                    assistant_run_answer_contains_row_terms(output_text, row, &["缺勤", "未打卡"]);
            }
            _ => {}
        }
    }

    saw_absence_answer && saw_longest_answer && saw_shortest_answer
}

fn assistant_run_spreadsheet_row_analysis_rows(evidence_state: &Value) -> Option<Vec<&Value>> {
    let rows = evidence_state
        .get("supplied_items")
        .and_then(Value::as_array)?
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("spreadsheet_row_analysis"))
        .filter_map(|item| item.get("rows").and_then(Value::as_array))
        .flatten()
        .collect::<Vec<_>>();
    (!rows.is_empty()).then_some(rows)
}

fn assistant_run_answer_contains_row_terms(
    output_text: &str,
    row: &Value,
    category_terms: &[&str],
) -> bool {
    let has_category = category_terms.iter().any(|term| output_text.contains(term));
    if !has_category {
        return false;
    }
    ["date", "employee", "work_hours_text"]
        .iter()
        .filter_map(|key| row.get(*key).and_then(Value::as_str))
        .filter(|term| !term.trim().is_empty())
        .all(|term| output_text.contains(term))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn spreadsheet_request(prompt: &str, wants_json: bool) -> CreateAssistantRunRequest {
        CreateAssistantRunRequest {
            prompt: prompt.to_string(),
            local_thread_id: None,
            startup_briefing: None,
            selected_scope: Some(json!({"datasets": ["00000000-0000-0000-0000-000000000001"]})),
            scope_candidates: Vec::new(),
            context_policy_hint: wants_json.then(|| {
                json!({
                    "answer_policy": {
                        "output_format": {"format": "json"}
                    }
                })
            }),
            current_artifact: None,
            messages: Vec::new(),
        }
    }

    fn spreadsheet_evidence_state() -> Value {
        json!({
            "status": "supplied",
            "supplied_items": [{
                "type": "spreadsheet_row_analysis",
                "analysis_kind": "absence_and_work_hour_extremes",
                "documents": [{
                    "title": "A3-坐班0900考勤记录.xlsx"
                }],
                "rows": [
                    {
                        "date": "2026-05-13",
                        "employee": "A5",
                        "shift": "坐班0900",
                        "status": "未打卡 不考勤",
                        "counts_as_absence": false
                    },
                    {
                        "category": "longest",
                        "date": "2026-02-07",
                        "employee": "A8",
                        "shift": "休息",
                        "work_hours_text": "12.55小时",
                        "status": "正常考勤"
                    },
                    {
                        "category": "shortest",
                        "date": "2026-02-25",
                        "employee": "A8",
                        "shift": "坐班0930",
                        "work_hours_text": "4.30小时",
                        "status": "正常考勤"
                    }
                ]
            }]
        })
    }

    #[test]
    fn spreadsheet_controlled_answer_renders_markdown_tables() {
        let request = spreadsheet_request(
            "这份考勤表里最近有没缺勤的人？工时最长和最短分别是谁？请按日期、员工、班次、工时出表。",
            false,
        );

        let answer = assistant_run_answer_quality_spreadsheet_controlled_answer(
            &spreadsheet_evidence_state(),
            &request,
        )
        .expect("spreadsheet rows should produce a controlled answer");

        assert!(answer.contains("| 类别 | 日期 | 员工 | 班次 | 状态 |"));
        assert!(answer.contains("| 未打卡/不考勤 | 2026-05-13 | A5 | 坐班0900 | 未打卡 不考勤 |"));
        assert!(answer.contains("| 最长工时 | 2026-02-07 | A8 | 休息 | 12.55小时 | 正常考勤 |"));
        assert!(answer.contains("来源：A3-坐班0900考勤记录.xlsx"));
    }

    #[test]
    fn spreadsheet_controlled_answer_honors_json_output_format() {
        let request = spreadsheet_request(
            "这份考勤表里最近有没缺勤的人？工时最长和最短分别是谁？请输出 JSON。",
            true,
        );

        let answer = assistant_run_answer_quality_spreadsheet_controlled_answer(
            &spreadsheet_evidence_state(),
            &request,
        )
        .expect("spreadsheet rows should produce a controlled answer");
        let parsed: Value =
            serde_json::from_str(&answer).expect("controlled answer should be JSON");

        assert_eq!(parsed["status"], json!("answered"));
        assert_eq!(parsed["source"], json!("spreadsheet_row_analysis"));
        assert_eq!(parsed["absence_rows"][0]["employee"], json!("A5"));
        assert_eq!(parsed["work_hour_extremes"][0]["kind"], json!("longest"));
        assert!(!answer.contains("| 类别 |"));
    }

    #[test]
    fn spreadsheet_satisfied_answer_requires_requested_rows() {
        let request = spreadsheet_request(
            "这份考勤表里最近有没缺勤的人？工时最长和最短分别是谁？请出表。",
            false,
        );
        let answer = "| 类型 | 日期 | 员工 | 班次 | 工时 | 状态 |\n| 缺勤 | 2026-05-13 | A5 | 坐班0900 |  | 未打卡 不考勤 |\n| 最长工时 | 2026-02-07 | A8 | 休息 | 12.55小时 | 正常考勤 |\n| 最短工时 | 2026-02-25 | A8 | 坐班0930 | 4.30小时 | 正常考勤 |";

        assert!(assistant_run_answer_satisfies_spreadsheet_row_analysis(
            answer,
            &request,
            &spreadsheet_evidence_state()
        ));
        assert!(!assistant_run_answer_satisfies_spreadsheet_row_analysis(
            "| 类型 | 日期 | 员工 |\n| 最长工时 | 2026-02-07 | A8 |",
            &request,
            &spreadsheet_evidence_state()
        ));
    }
}
