use std::collections::BTreeMap;

use contracts::CreateAssistantRunRequest;
use serde_json::{json, Value};

use crate::prompt_match_support::prompt_contains_any;
use crate::{
    assistant_run_answer_contains_insufficient_evidence_marker,
    assistant_run_prompt_requests_point_list_table,
    assistant_run_react_output_contains_internal_marker, assistant_run_request_wants_json_output,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AssistantRunPointListRow {
    pub(crate) floor: String,
    pub(crate) location: String,
    pub(crate) name: String,
    pub(crate) area_id: String,
    pub(crate) type_label: String,
}

pub(crate) type AssistantRunFloorLocation = (String, String);

pub(crate) fn assistant_run_point_list_rows_from_retrieval_evidence(
    evidence_state: &Value,
) -> Vec<AssistantRunPointListRow> {
    let mut by_name = BTreeMap::<String, AssistantRunPointListRow>::new();
    for item in evidence_state
        .get("supplied_items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("retrieval_evidence"))
    {
        let Some(content) = item.get("content_excerpt").and_then(Value::as_str) else {
            continue;
        };
        let Some(name) = assistant_run_marker_value(content, "areaname") else {
            continue;
        };
        if !assistant_run_text_has_elevator_point_signal(&name) {
            continue;
        }
        let area_id = assistant_run_marker_value(content, "areaid").unwrap_or_default();
        let (floor, location) = assistant_run_split_floor_location(&name);
        let type_label = assistant_run_point_type_label(&name);
        by_name
            .entry(name.clone())
            .or_insert(AssistantRunPointListRow {
                floor,
                location,
                name,
                area_id,
                type_label,
            });
    }
    by_name.into_values().collect()
}

pub(crate) fn assistant_run_answer_quality_point_list_controlled_answer(
    evidence_state: &Value,
    request: &CreateAssistantRunRequest,
) -> Option<String> {
    if !assistant_run_prompt_requests_point_list_table(&request.prompt) {
        return None;
    }
    let rows = assistant_run_point_list_rows_from_retrieval_evidence(evidence_state);
    if rows.is_empty() {
        return None;
    }
    if assistant_run_request_wants_json_output(request) {
        let rows = rows
            .iter()
            .map(|row| {
                json!({
                    "floor": row.floor,
                    "location": row.location,
                    "name": row.name,
                    "areaid": row.area_id,
                    "type": row.type_label,
                })
            })
            .collect::<Vec<_>>();
        return serde_json::to_string_pretty(&json!({
            "status": "answered",
            "source": "retrieval_point_list",
            "question": request.prompt.trim(),
            "rows": rows,
        }))
        .ok();
    }
    let mut lines = vec![
        "根据已检索到的点位证据，智能梯控/电梯点位如下：".to_string(),
        String::new(),
        "| 楼层 | 位置 | 点位名称 | areaid | 类型 |".to_string(),
        "|---|---|---|---|---|".to_string(),
    ];
    for row in rows {
        lines.push(format!(
            "| {} | {} | {} | {} | {} |",
            row.floor, row.location, row.name, row.area_id, row.type_label
        ));
    }
    Some(lines.join("\n"))
}

pub(crate) fn assistant_run_answer_satisfies_retrieval_point_list(
    output_text: &str,
    request: &CreateAssistantRunRequest,
    evidence_state: &Value,
) -> bool {
    if !assistant_run_prompt_requests_point_list_table(&request.prompt) {
        return false;
    }
    if assistant_run_answer_contains_insufficient_evidence_marker(output_text)
        || assistant_run_react_output_contains_internal_marker(output_text)
        || !output_text.contains('|')
    {
        return false;
    }
    let rows = assistant_run_point_list_rows_from_retrieval_evidence(evidence_state);
    if rows.is_empty() {
        return false;
    }
    let matched = rows
        .iter()
        .filter(|row| output_text.contains(&row.name))
        .count();
    matched >= rows.len().min(3)
}

fn assistant_run_marker_value(content: &str, marker: &str) -> Option<String> {
    let marker = format!("## {marker}");
    let tail = content.split_once(&marker)?.1.trim();
    let value = tail.split("##").next()?.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn assistant_run_text_has_elevator_point_signal(value: &str) -> bool {
    prompt_contains_any(value, &["电梯", "扶梯", "梯控"])
        || value.to_ascii_lowercase().contains("elevator")
}

fn assistant_run_point_type_label(name: &str) -> String {
    if prompt_contains_any(name, &["扶梯", "手扶"]) {
        "扶梯".to_string()
    } else if prompt_contains_any(name, &["电梯", "直梯", "观光梯"]) {
        "电梯".to_string()
    } else {
        "点位".to_string()
    }
}

pub(crate) fn assistant_run_split_floor_location(name: &str) -> AssistantRunFloorLocation {
    let mut split_at = 0usize;
    for (index, ch) in name.char_indices() {
        if ch.is_ascii_alphanumeric() {
            split_at = index + ch.len_utf8();
            continue;
        }
        break;
    }
    if split_at == 0 {
        return ("".to_string(), name.to_string());
    }
    let floor = name[..split_at].to_string();
    let location = name[split_at..].trim().to_string();
    (floor, location)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn point_list_request(prompt: &str, wants_json: bool) -> CreateAssistantRunRequest {
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

    fn point_list_evidence_state() -> Value {
        json!({
            "supplied_items": [
                {
                    "type": "retrieval_evidence",
                    "content_excerpt": "## areaname B1F东电梯 ## areaid 4301116 ## areatype 4"
                },
                {
                    "type": "retrieval_evidence",
                    "content_excerpt": "## areaname B2观光电梯口 ## areaid 4301101 ## areatype 4"
                }
            ]
        })
    }

    #[test]
    fn point_list_support_extracts_unique_elevator_rows() {
        let rows = assistant_run_point_list_rows_from_retrieval_evidence(&json!({
            "supplied_items": [
                {
                    "type": "retrieval_evidence",
                    "content_excerpt": "## areaname B1F东电梯 ## areaid 4301116 ## areatype 4"
                },
                {
                    "type": "retrieval_evidence",
                    "content_excerpt": "## areaname B1F东电梯 ## areaid duplicate ## areatype 4"
                },
                {
                    "type": "retrieval_evidence",
                    "content_excerpt": "## areaname B2观光电梯口 ## areaid 4301101 ## areatype 4"
                },
                {
                    "type": "retrieval_evidence",
                    "content_excerpt": "## areaname 普通门禁 ## areaid 999"
                }
            ]
        }));

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "B1F东电梯");
        assert_eq!(rows[0].floor, "B1F");
        assert_eq!(rows[0].location, "东电梯");
        assert_eq!(rows[0].area_id, "4301116");
        assert_eq!(rows[0].type_label, "电梯");
        assert_eq!(rows[1].name, "B2观光电梯口");
    }

    #[test]
    fn point_list_support_splits_floor_prefix_and_location() {
        assert_eq!(
            assistant_run_split_floor_location("3F东区电梯"),
            ("3F".to_string(), "东区电梯".to_string())
        );
        assert_eq!(
            assistant_run_split_floor_location("东区观光梯"),
            ("".to_string(), "东区观光梯".to_string())
        );
    }

    #[test]
    fn point_list_support_builds_table_controlled_answer() {
        let answer = assistant_run_answer_quality_point_list_controlled_answer(
            &point_list_evidence_state(),
            &point_list_request("智能梯控/电梯点位有哪些？请按楼层和位置出表。", false),
        )
        .expect("point rows should build a table answer");

        assert!(answer.contains("| 楼层 | 位置 | 点位名称 | areaid | 类型 |"));
        assert!(answer.contains("| B1F | 东电梯 | B1F东电梯 | 4301116 | 电梯 |"));
        assert!(answer.contains("| B2 | 观光电梯口 | B2观光电梯口 | 4301101 | 电梯 |"));
    }

    #[test]
    fn point_list_support_builds_json_controlled_answer() {
        let answer = assistant_run_answer_quality_point_list_controlled_answer(
            &point_list_evidence_state(),
            &point_list_request("智能梯控/电梯点位有哪些？请输出 JSON。", true),
        )
        .expect("point rows should build a json answer");
        let parsed: Value = serde_json::from_str(&answer).expect("answer should be valid json");

        assert_eq!(parsed["status"], json!("answered"));
        assert_eq!(parsed["source"], json!("retrieval_point_list"));
        assert_eq!(parsed["rows"][0]["name"], json!("B1F东电梯"));
        assert_eq!(parsed["rows"][0]["areaid"], json!("4301116"));
        assert!(!answer.contains("| 楼层 |"));
    }

    #[test]
    fn point_list_support_skips_non_point_prompts_or_empty_evidence() {
        assert!(assistant_run_answer_quality_point_list_controlled_answer(
            &point_list_evidence_state(),
            &point_list_request("帮我总结电梯安全注意事项", false),
        )
        .is_none());
        assert!(assistant_run_answer_quality_point_list_controlled_answer(
            &json!({"supplied_items": []}),
            &point_list_request("智能梯控/电梯点位有哪些？请按楼层和位置出表。", false),
        )
        .is_none());
    }

    #[test]
    fn point_list_support_detects_satisfied_retrieval_answer() {
        let answer = "| 楼层 | 位置 | 点位名称 |\n| B1F | 东电梯 | B1F东电梯 |\n| B2 | 观光电梯口 | B2观光电梯口 |";

        assert!(assistant_run_answer_satisfies_retrieval_point_list(
            answer,
            &point_list_request("智能梯控/电梯点位有哪些？请按楼层和位置出表。", false),
            &point_list_evidence_state(),
        ));
    }

    #[test]
    fn point_list_support_rejects_unsatisfied_or_internal_answers() {
        let request = point_list_request("智能梯控/电梯点位有哪些？请按楼层和位置出表。", false);

        assert!(!assistant_run_answer_satisfies_retrieval_point_list(
            "当前资料不足，无法确认。",
            &request,
            &point_list_evidence_state(),
        ));
        assert!(!assistant_run_answer_satisfies_retrieval_point_list(
            "[tool_call] retrieve_evidence",
            &request,
            &point_list_evidence_state(),
        ));
        assert!(!assistant_run_answer_satisfies_retrieval_point_list(
            "B1F东电梯、B2观光电梯口",
            &request,
            &point_list_evidence_state(),
        ));
    }
}
