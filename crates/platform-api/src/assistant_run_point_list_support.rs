use std::collections::BTreeMap;

use serde_json::Value;

use crate::prompt_match_support::prompt_contains_any;

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
}
