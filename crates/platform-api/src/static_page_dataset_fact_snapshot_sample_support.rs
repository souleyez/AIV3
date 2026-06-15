use crate::prompt_match_support::prompt_has_any;
use crate::static_page_database_aggregate_sample_support::static_page_database_aggregate_module_text;
use crate::static_page_explicit_sample_support::static_page_json_number;
use crate::static_page_module_binding_support::static_page_visualization_needs_sample_rows;
use crate::static_page_report_snapshot::static_page_report_table_id;
use serde_json::{json, Map, Value};

pub(crate) fn static_page_dataset_fact_snapshot_rows_by_type(
    item: &Value,
) -> Option<&Map<String, Value>> {
    item.get("entity_rows_by_type")
        .or_else(|| item.pointer("/snapshot_manifest/entity_rows_by_type"))
        .and_then(Value::as_object)
}

pub(crate) fn static_page_dataset_fact_snapshot_field_path(fact_type: &str) -> String {
    format!(
        "dataset.fact_snapshot.{}",
        static_page_report_table_id(fact_type)
    )
}

fn static_page_dataset_fact_snapshot_field_path_fact_type(field_path: &str) -> Option<&str> {
    field_path
        .strip_prefix("dataset.fact_snapshot.")
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub(crate) fn static_page_dataset_fact_snapshot_type_label(fact_type: &str) -> String {
    match fact_type {
        "organization" => "组织/公司",
        "person" => "人员",
        "role_position" => "角色/岗位",
        "skill_technology" => "技能/技术",
        "project_product_system" => "项目/产品/系统",
        "location_area" => "地点/区域",
        "education_certificate" => "教育/证书",
        "date_period" => "日期/周期",
        "section" => "章节",
        "procedure_step" => "流程步骤",
        "time_threshold" => "时间阈值",
        "keyword" => "关键词",
        _ => fact_type,
    }
    .to_string()
}

pub(crate) fn build_static_page_dataset_fact_snapshot_sample_points(
    evidence_items: &[Value],
    module: &Value,
    field_path: Option<&str>,
) -> Vec<Value> {
    let visualization_type = module
        .get("visualization")
        .and_then(|visualization| visualization.get("type"))
        .and_then(Value::as_str)
        .unwrap_or("text-insight");
    let can_drive_module = static_page_visualization_needs_sample_rows(visualization_type)
        || field_path
            .map(|path| path.starts_with("dataset.fact_snapshot"))
            .unwrap_or(false);
    if !can_drive_module {
        return Vec::new();
    }

    let Some((item, fact_type, rows)) =
        static_page_dataset_fact_snapshot_best_rows(evidence_items, module, field_path)
    else {
        return Vec::new();
    };
    static_page_dataset_fact_snapshot_sample_points_from_rows(item, fact_type, rows, field_path)
}

fn static_page_dataset_fact_snapshot_best_rows<'a>(
    evidence_items: &'a [Value],
    module: &Value,
    field_path: Option<&str>,
) -> Option<(&'a Value, &'a str, &'a [Value])> {
    let requested_fact_type =
        field_path.and_then(static_page_dataset_fact_snapshot_field_path_fact_type);
    let module_text =
        static_page_database_aggregate_module_text(module, field_path).to_ascii_lowercase();
    let mut best: Option<(&Value, &str, &[Value], i32)> = None;

    for item in evidence_items
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("dataset_fact_snapshot"))
    {
        let Some(rows_by_type) = static_page_dataset_fact_snapshot_rows_by_type(item) else {
            continue;
        };
        for (fact_type, rows) in rows_by_type {
            let Some(rows) = rows.as_array() else {
                continue;
            };
            if rows.is_empty() {
                continue;
            }
            if let Some(requested_fact_type) = requested_fact_type {
                if requested_fact_type != fact_type {
                    continue;
                }
            }
            let score =
                static_page_dataset_fact_snapshot_type_score(fact_type, &module_text, rows.len());
            if best
                .as_ref()
                .is_none_or(|(_, _, _, best_score)| score > *best_score)
            {
                best = Some((item, fact_type.as_str(), rows.as_slice(), score));
            }
        }
    }

    best.map(|(item, fact_type, rows, _score)| (item, fact_type, rows))
}

fn static_page_dataset_fact_snapshot_type_score(
    fact_type: &str,
    module_text: &str,
    row_count: usize,
) -> i32 {
    let mut score = row_count.min(24) as i32;
    let label = static_page_dataset_fact_snapshot_type_label(fact_type).to_lowercase();
    let path = static_page_dataset_fact_snapshot_field_path(fact_type);
    if module_text.contains(fact_type)
        || module_text.contains(&label)
        || module_text.contains(&path)
    {
        score += 80;
    }
    match fact_type {
        "organization"
            if prompt_has_any(module_text, &["公司", "组织", "客户", "门店", "品牌"]) =>
        {
            score += 40
        }
        "person" if prompt_has_any(module_text, &["人员", "联系人", "姓名", "员工"]) => {
            score += 35
        }
        "role_position" if prompt_has_any(module_text, &["角色", "岗位", "职位"]) => {
            score += 35
        }
        "location_area" if prompt_has_any(module_text, &["地点", "区域", "门店", "位置"]) => {
            score += 35
        }
        "date_period" if prompt_has_any(module_text, &["日期", "时间", "周期", "月份", "趋势"]) => {
            score += 30
        }
        "keyword" if prompt_has_any(module_text, &["关键词", "主题", "标签"]) => score += 25,
        _ => {}
    }
    score
}

pub(crate) fn static_page_dataset_fact_snapshot_sample_points_from_rows(
    item: &Value,
    fact_type: &str,
    rows: &[Value],
    field_path: Option<&str>,
) -> Vec<Value> {
    let field_path = field_path
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| static_page_dataset_fact_snapshot_field_path(fact_type));
    rows.iter()
        .enumerate()
        .filter_map(|(index, row)| {
            let label = static_page_dataset_fact_snapshot_row_label(row, index)?;
            let value = static_page_dataset_fact_snapshot_row_value(row)?;
            Some(json!({
                "label": label,
                "value": value,
                "kind": "dataset_fact_snapshot",
                "source": "dataset_fact_snapshot",
                "fieldPath": field_path,
                "datasetId": item.get("dataset_id").cloned().unwrap_or(Value::Null),
                "datasetKey": item.get("dataset_key").cloned().unwrap_or(Value::Null),
                "factType": fact_type,
                "name": row.get("name").cloned().unwrap_or_else(|| json!(label)),
                "normalizedName": row.get("normalized_name").cloned().unwrap_or(Value::Null),
                "factCount": row.get("fact_count").cloned().unwrap_or(Value::Null),
                "documentCount": row.get("document_count").cloned().unwrap_or(Value::Null),
                "metric": "fact_count",
                "sourceDocumentIds": row.get("source_document_ids").cloned().unwrap_or_else(|| json!([])),
                "sourceLocators": row.get("source_locators").cloned().unwrap_or_else(|| json!([])),
                "sourceFactCount": item.get("source_fact_count").cloned().unwrap_or(Value::Null),
                "sourceDocumentCount": item.get("source_document_count").cloned().unwrap_or(Value::Null),
            }))
        })
        .take(12)
        .collect()
}

fn static_page_dataset_fact_snapshot_row_label(row: &Value, index: usize) -> Option<String> {
    row.get("name")
        .or_else(|| row.get("normalized_name"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(32).collect())
        .or_else(|| Some(format!("事实 {}", index + 1)))
}

fn static_page_dataset_fact_snapshot_row_value(row: &Value) -> Option<f64> {
    row.get("fact_count")
        .and_then(static_page_json_number)
        .or_else(|| row.get("document_count").and_then(static_page_json_number))
        .or(Some(1.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn fact_snapshot_rows_by_type_reads_top_level_and_manifest() {
        let top_level = json!({
            "entity_rows_by_type": {
                "organization": [{"name": "A", "fact_count": 2}]
            }
        });
        assert!(static_page_dataset_fact_snapshot_rows_by_type(&top_level)
            .and_then(|rows| rows.get("organization"))
            .is_some());

        let manifest = json!({
            "snapshot_manifest": {
                "entity_rows_by_type": {
                    "person": [{"name": "张三", "fact_count": 1}]
                }
            }
        });
        assert!(static_page_dataset_fact_snapshot_rows_by_type(&manifest)
            .and_then(|rows| rows.get("person"))
            .is_some());
    }

    #[test]
    fn fact_snapshot_sample_points_keep_metadata_and_cap_rows() {
        let item = json!({
            "type": "dataset_fact_snapshot",
            "dataset_id": "dataset-1",
            "dataset_key": "xinbai",
            "source_fact_count": 99,
            "source_document_count": 3
        });
        let rows = (0..14)
            .map(|index| {
                json!({
                    "name": format!("品牌{index}"),
                    "normalized_name": format!("brand-{index}"),
                    "fact_count": index + 1,
                    "document_count": 1,
                    "source_document_ids": [format!("doc-{index}")]
                })
            })
            .collect::<Vec<_>>();

        let points = static_page_dataset_fact_snapshot_sample_points_from_rows(
            &item,
            "organization",
            &rows,
            None,
        );

        assert_eq!(points.len(), 12);
        assert_eq!(points[0].get("label"), Some(&json!("品牌0")));
        assert_eq!(points[0].get("value"), Some(&json!(1.0)));
        assert_eq!(points[0].get("kind"), Some(&json!("dataset_fact_snapshot")));
        assert_eq!(
            points[0].get("fieldPath"),
            Some(&json!("dataset.fact_snapshot.organization"))
        );
        assert_eq!(points[0].get("datasetKey"), Some(&json!("xinbai")));
        assert_eq!(points[0].get("sourceFactCount"), Some(&json!(99)));
        assert_eq!(points[11].get("label"), Some(&json!("品牌11")));
    }

    #[test]
    fn fact_snapshot_sample_points_use_requested_fact_type() {
        let evidence_items = vec![json!({
            "type": "dataset_fact_snapshot",
            "dataset_id": "dataset-1",
            "entity_rows_by_type": {
                "organization": [
                    {"name": "公司A", "fact_count": 9}
                ],
                "person": [
                    {"name": "张三", "fact_count": 2}
                ]
            }
        })];
        let module = json!({
            "title": "人员名单",
            "visualization": {"type": "bar-chart"},
            "dataBinding": {"fieldPath": "dataset.fact_snapshot.person"}
        });

        let points = build_static_page_dataset_fact_snapshot_sample_points(
            &evidence_items,
            &module,
            Some("dataset.fact_snapshot.person"),
        );

        assert_eq!(points.len(), 1);
        assert_eq!(points[0].get("label"), Some(&json!("张三")));
        assert_eq!(points[0].get("factType"), Some(&json!("person")));
    }

    #[test]
    fn fact_snapshot_sample_points_score_module_intent() {
        let evidence_items = vec![json!({
            "type": "dataset_fact_snapshot",
            "dataset_id": "dataset-1",
            "entity_rows_by_type": {
                "keyword": [
                    {"name": "销售", "fact_count": 20}
                ],
                "location_area": [
                    {"name": "华东一区", "fact_count": 3}
                ]
            }
        })];
        let module = json!({
            "title": "门店区域分布",
            "visualization": {"type": "bar-chart"}
        });

        let points =
            build_static_page_dataset_fact_snapshot_sample_points(&evidence_items, &module, None);

        assert_eq!(points.len(), 1);
        assert_eq!(points[0].get("label"), Some(&json!("华东一区")));
        assert_eq!(points[0].get("factType"), Some(&json!("location_area")));
    }

    #[test]
    fn fact_snapshot_sample_points_return_empty_when_module_does_not_need_rows() {
        let evidence_items = vec![json!({
            "type": "dataset_fact_snapshot",
            "entity_rows_by_type": {
                "organization": [{"name": "公司A", "fact_count": 1}]
            }
        })];
        let module = json!({
            "title": "普通说明",
            "visualization": {"type": "text-insight"}
        });

        let points =
            build_static_page_dataset_fact_snapshot_sample_points(&evidence_items, &module, None);

        assert!(points.is_empty());
    }

    #[test]
    fn fact_snapshot_field_path_and_label_keep_known_mappings() {
        assert_eq!(
            static_page_dataset_fact_snapshot_field_path("skill_technology"),
            "dataset.fact_snapshot.skill_technology"
        );
        assert_eq!(
            static_page_dataset_fact_snapshot_type_label("skill_technology"),
            "技能/技术"
        );
        assert_eq!(
            static_page_dataset_fact_snapshot_type_label("custom_fact"),
            "custom_fact"
        );
    }
}
