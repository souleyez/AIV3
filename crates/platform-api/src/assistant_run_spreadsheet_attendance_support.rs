use crate::prompt_match_support::prompt_contains_any;
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub(crate) struct AssistantRunAttendanceRow {
    pub(crate) employee: String,
    pub(crate) date: String,
    pub(crate) shift: String,
    pub(crate) first_punch: Option<String>,
    pub(crate) last_punch: Option<String>,
    pub(crate) work_hours: Option<f64>,
    pub(crate) status: String,
    pub(crate) source_locator: String,
}

pub(crate) fn parse_assistant_run_attendance_row(
    line: &str,
    source_locator: String,
) -> Option<AssistantRunAttendanceRow> {
    let tokens = line
        .split_whitespace()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    let date_index = tokens
        .iter()
        .position(|token| assistant_run_token_is_iso_date(token))?;
    if date_index == 0 || date_index + 1 >= tokens.len() {
        return None;
    }
    let employee = tokens[date_index - 1].trim_matches('|').to_string();
    let date = tokens[date_index].to_string();
    let shift = tokens[date_index + 1].to_string();
    if employee.is_empty() || employee == "员工" || shift == "班次" {
        return None;
    }

    let mut first_punch = None;
    let mut last_punch = None;
    let mut work_hours = None;
    let mut status_parts = Vec::new();
    for token in tokens.into_iter().skip(date_index + 2) {
        if let Some(time) = assistant_run_normalize_hhmm_token(token) {
            if first_punch.is_none() {
                first_punch = Some(time);
            } else if last_punch.is_none() {
                last_punch = Some(time);
            }
            continue;
        }
        if work_hours.is_none() {
            if let Some(hours) = assistant_run_parse_work_hours_token(token) {
                work_hours = Some(hours);
                continue;
            }
        }
        if token != "|" {
            status_parts.push(token.trim_matches('|').to_string());
        }
    }

    Some(AssistantRunAttendanceRow {
        employee,
        date,
        shift,
        first_punch,
        last_punch,
        work_hours,
        status: status_parts.join(" "),
        source_locator,
    })
}

fn assistant_run_token_is_iso_date(token: &str) -> bool {
    let bytes = token.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
}

fn assistant_run_normalize_hhmm_token(token: &str) -> Option<String> {
    let token = token.trim_matches(|ch: char| ch == '|' || ch == ',' || ch == ';');
    let (hour, minute) = token.split_once(':')?;
    if hour.len() > 2 || minute.len() != 2 {
        return None;
    }
    let hour = hour.parse::<u32>().ok()?;
    let minute = minute.parse::<u32>().ok()?;
    if hour > 23 || minute > 59 {
        return None;
    }
    Some(format!("{hour:02}:{minute:02}"))
}

fn assistant_run_parse_work_hours_token(token: &str) -> Option<f64> {
    let token = token.trim_matches('|').trim_end_matches("小时");
    if token.is_empty() || token == "未打卡" {
        return None;
    }
    token.parse::<f64>().ok()
}

fn assistant_run_hhmm_minutes(value: &str) -> Option<u32> {
    let (hour, minute) = value.split_once(':')?;
    let hour = hour.parse::<u32>().ok()?;
    let minute = minute.parse::<u32>().ok()?;
    Some(hour * 60 + minute)
}

pub(crate) fn assistant_run_attendance_analysis_rows(
    prompt: &str,
    rows: &[AssistantRunAttendanceRow],
) -> Option<(&'static str, Vec<Value>, String)> {
    let requests_absence = prompt_contains_any(prompt, &["缺勤", "未打卡", "没打卡"]);
    let requests_work_hours = prompt_contains_any(prompt, &["最长", "最短", "长短", "工时"]);
    if requests_absence && requests_work_hours {
        let mut analysis_rows = assistant_run_absence_attendance_rows(rows);
        analysis_rows.extend(assistant_run_work_hour_extreme_rows(rows));
        let content_excerpt = assistant_run_combined_attendance_rows_markdown(&analysis_rows);
        return Some((
            "absence_and_work_hour_extremes",
            analysis_rows,
            content_excerpt,
        ));
    }
    if prompt_contains_any(prompt, &["最早", "上班"]) {
        let analysis_rows = assistant_run_daily_earliest_attendance_rows(rows);
        let content_excerpt = assistant_run_attendance_rows_markdown(
            "日期 | 最早上班员工 | 首打卡时间",
            &analysis_rows,
        );
        return Some(("daily_earliest_first_punch", analysis_rows, content_excerpt));
    }
    if prompt_contains_any(prompt, &["最长", "最短", "长短", "工时"]) {
        let analysis_rows = assistant_run_work_hour_extreme_rows(rows);
        let content_excerpt =
            assistant_run_attendance_rows_markdown("类别 | 日期 | 员工 | 工时", &analysis_rows);
        return Some(("work_hour_extremes", analysis_rows, content_excerpt));
    }
    if requests_absence {
        let analysis_rows = assistant_run_absence_attendance_rows(rows);
        let content_excerpt =
            assistant_run_attendance_rows_markdown("日期 | 员工 | 班次 | 状态", &analysis_rows);
        return Some(("absence_candidates", analysis_rows, content_excerpt));
    }
    None
}

pub(crate) fn assistant_run_daily_earliest_attendance_rows(
    rows: &[AssistantRunAttendanceRow],
) -> Vec<Value> {
    let mut by_date = BTreeMap::<String, (&AssistantRunAttendanceRow, u32)>::new();
    for row in rows {
        let Some(first_punch) = row.first_punch.as_deref() else {
            continue;
        };
        let Some(minutes) = assistant_run_hhmm_minutes(first_punch) else {
            continue;
        };
        match by_date.get(&row.date) {
            Some((_, current_minutes)) if *current_minutes <= minutes => {}
            _ => {
                by_date.insert(row.date.clone(), (row, minutes));
            }
        }
    }
    by_date
        .into_iter()
        .rev()
        .map(|(date, (row, _))| {
            json!({
                "date": date,
                "employee": row.employee.clone(),
                "first_punch": row.first_punch.clone(),
                "shift": row.shift.clone(),
                "source_locator": row.source_locator.clone(),
            })
        })
        .collect()
}

pub(crate) fn assistant_run_work_hour_extreme_rows(
    rows: &[AssistantRunAttendanceRow],
) -> Vec<Value> {
    let hour_rows = rows
        .iter()
        .filter_map(|row| {
            row.work_hours
                .filter(|hours| assistant_run_attendance_row_has_countable_work_hours(row, *hours))
                .map(|hours| (row, hours))
        })
        .collect::<Vec<_>>();
    let Some((shortest_row, shortest_hours)) = hour_rows
        .iter()
        .min_by(|left, right| left.1.total_cmp(&right.1))
        .copied()
    else {
        return Vec::new();
    };
    let Some((longest_row, longest_hours)) = hour_rows
        .iter()
        .max_by(|left, right| left.1.total_cmp(&right.1))
        .copied()
    else {
        return Vec::new();
    };
    vec![
        assistant_run_work_hour_extreme_value("longest", longest_row, longest_hours),
        assistant_run_work_hour_extreme_value("shortest", shortest_row, shortest_hours),
    ]
}

fn assistant_run_attendance_row_has_countable_work_hours(
    row: &AssistantRunAttendanceRow,
    hours: f64,
) -> bool {
    hours > 0.0
        && row.first_punch.is_some()
        && row.last_punch.is_some()
        && !row.status.contains("未打卡")
        && !row.status.contains("不考勤")
}

fn assistant_run_work_hour_extreme_value(
    category: &str,
    row: &AssistantRunAttendanceRow,
    hours: f64,
) -> Value {
    json!({
        "category": category,
        "date": row.date.clone(),
        "employee": row.employee.clone(),
        "work_hours": hours,
        "work_hours_text": format!("{hours:.2}小时"),
        "shift": row.shift.clone(),
        "first_punch": row.first_punch.clone(),
        "last_punch": row.last_punch.clone(),
        "status": row.status.clone(),
        "source_locator": row.source_locator.clone(),
    })
}

fn assistant_run_absence_attendance_rows(rows: &[AssistantRunAttendanceRow]) -> Vec<Value> {
    rows.iter()
        .filter(|row| row.first_punch.is_none() && row.status.contains("未打卡"))
        .filter(|row| row.shift.contains("坐班"))
        .map(|row| {
            json!({
                "date": row.date.clone(),
                "employee": row.employee.clone(),
                "shift": row.shift.clone(),
                "status": row.status.clone(),
                "counts_as_absence": !row.status.contains("不考勤"),
                "source_locator": row.source_locator.clone(),
            })
        })
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .take(80)
        .collect()
}

fn assistant_run_attendance_rows_markdown(header: &str, rows: &[Value]) -> String {
    let mut lines = vec![header.to_string()];
    for row in rows.iter().take(80) {
        if let Some(category) = row.get("category").and_then(Value::as_str) {
            lines.push(format!(
                "{} | {} | {} | {}",
                category,
                row.get("date").and_then(Value::as_str).unwrap_or(""),
                row.get("employee").and_then(Value::as_str).unwrap_or(""),
                row.get("work_hours_text")
                    .and_then(Value::as_str)
                    .unwrap_or("")
            ));
        } else if let Some(first_punch) = row.get("first_punch").and_then(Value::as_str) {
            lines.push(format!(
                "{} | {} | {}",
                row.get("date").and_then(Value::as_str).unwrap_or(""),
                row.get("employee").and_then(Value::as_str).unwrap_or(""),
                first_punch
            ));
        } else {
            lines.push(format!(
                "{} | {} | {} | {}",
                row.get("date").and_then(Value::as_str).unwrap_or(""),
                row.get("employee").and_then(Value::as_str).unwrap_or(""),
                row.get("shift").and_then(Value::as_str).unwrap_or(""),
                row.get("status").and_then(Value::as_str).unwrap_or("")
            ));
        }
    }
    lines.join("\n")
}

fn assistant_run_combined_attendance_rows_markdown(rows: &[Value]) -> String {
    let mut lines = vec!["类型 | 日期 | 员工 | 明细 | 状态".to_string()];
    for row in rows.iter().take(82) {
        if let Some(category) = row.get("category").and_then(Value::as_str) {
            lines.push(format!(
                "{} | {} | {} | {} | {}",
                category,
                row.get("date").and_then(Value::as_str).unwrap_or(""),
                row.get("employee").and_then(Value::as_str).unwrap_or(""),
                row.get("work_hours_text")
                    .and_then(Value::as_str)
                    .unwrap_or(""),
                row.get("status").and_then(Value::as_str).unwrap_or("")
            ));
        } else {
            lines.push(format!(
                "absence_candidate | {} | {} | {} | {}",
                row.get("date").and_then(Value::as_str).unwrap_or(""),
                row.get("employee").and_then(Value::as_str).unwrap_or(""),
                row.get("shift").and_then(Value::as_str).unwrap_or(""),
                row.get("status").and_then(Value::as_str).unwrap_or("")
            ));
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attendance_rows(lines: &[&str]) -> Vec<AssistantRunAttendanceRow> {
        lines
            .iter()
            .filter_map(|line| {
                parse_assistant_run_attendance_row(
                    line,
                    "document://attendance/chunks/0".to_string(),
                )
            })
            .collect()
    }

    #[test]
    fn spreadsheet_attendance_support_computes_earliest_and_extremes() {
        let rows = attendance_rows(&[
            "A3 2026-05-14 坐班0900 08:51 18:10 9.32小时 正常考勤",
            "A4 2026-05-14 坐班0900 08:56 18:02 9.10小时 正常考勤",
            "A5 2026-05-13 坐班0900 未打卡 不考勤",
            "A5 2026-05-19 坐班0900 0.00小时 不考勤",
            "A8 2026-02-07 休息 08:18 20:51 12.55小时 正常考勤",
            "A8 2026-02-25 坐班0930 09:25 13:43 4.30小时 正常考勤",
        ]);

        let daily_rows = assistant_run_daily_earliest_attendance_rows(&rows);
        assert_eq!(daily_rows[0]["employee"], json!("A3"));
        assert_eq!(daily_rows[0]["first_punch"], json!("08:51"));
        let hour_rows = assistant_run_work_hour_extreme_rows(&rows);
        assert_eq!(hour_rows[0]["category"], json!("longest"));
        assert_eq!(hour_rows[0]["employee"], json!("A8"));
        assert_eq!(hour_rows[1]["category"], json!("shortest"));
        assert_eq!(hour_rows[1]["employee"], json!("A8"));
    }

    #[test]
    fn spreadsheet_attendance_support_handles_absence_workhour_prompt() {
        let rows = attendance_rows(&[
            "A3 2026-05-20 坐班0900 08:59 18:15 9.27小时 正常考勤",
            "A5 2026-05-13 坐班0900 未打卡 不考勤",
            "A5 2026-05-19 坐班0900 0.00小时 不考勤",
            "A8 2026-05-15 坐班0930 未打卡 0.00小时 正常考勤",
            "A8 2026-02-07 休息 08:18 20:51 12.55小时 正常考勤",
            "A8 2026-02-25 坐班0930 09:25 13:43 4.30小时 正常考勤",
            "A9 2026-02-25 坐班0930 09:31 18:01 8.50小时 正常考勤",
        ]);

        let (analysis_kind, analysis_rows, excerpt) = assistant_run_attendance_analysis_rows(
            "这份考勤表里最近有没缺勤的人？工时最长和最短分别是谁？请按日期、员工、班次、工时出表。",
            &rows,
        )
        .expect("frequent attendance prompt should compute deterministic rows");

        assert_eq!(analysis_kind, "absence_and_work_hour_extremes");
        assert_eq!(analysis_rows.len(), 4);
        assert!(excerpt.contains("absence_candidate | 2026-05-13 | A5 | 坐班0900 | 未打卡 不考勤"));
        assert!(excerpt.contains("longest | 2026-02-07 | A8 | 12.55小时"));
        assert!(excerpt.contains("shortest | 2026-02-25 | A8 | 4.30小时"));
    }
}
