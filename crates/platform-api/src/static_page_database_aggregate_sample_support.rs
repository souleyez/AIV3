use crate::prompt_match_support::prompt_has_any;
use crate::static_page_artifact_summary_support::static_page_artifact_string;
use crate::static_page_explicit_sample_support::{
    static_page_explicit_point_label, static_page_json_number,
};
use crate::static_page_module_binding_support::static_page_visualization_needs_sample_rows;
use serde_json::{json, Value};

pub(crate) fn build_static_page_database_aggregate_sample_points(
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
            .map(|path| {
                path.starts_with("database.aggregate")
                    || path == "dataset.metrics_summary"
                    || path == "selected_scope.metrics_summary"
            })
            .unwrap_or(false);
    if !can_drive_module {
        return Vec::new();
    }

    let Some(item) = static_page_database_aggregate_best_item(
        evidence_items,
        module,
        field_path,
        visualization_type,
    ) else {
        return Vec::new();
    };
    static_page_database_aggregate_sample_points_from_item(item, field_path)
}

pub(crate) fn static_page_database_aggregate_sample_points_from_item(
    item: &Value,
    field_path: Option<&str>,
) -> Vec<Value> {
    let Some(rows) = item.get("rows").and_then(Value::as_array) else {
        return Vec::new();
    };

    rows.iter()
        .enumerate()
        .filter_map(move |(index, row)| {
            let value = static_page_database_aggregate_row_value(row, item)?;
            Some(json!({
                "label": static_page_database_aggregate_row_label(row, item, index),
                "value": value,
                "kind": "database_aggregate",
                "source": "database_aggregate",
                "fieldPath": field_path
                    .map(ToString::to_string)
                    .unwrap_or_else(|| static_page_database_aggregate_field_path(item)),
                "datasetId": item.get("dataset_id").cloned().unwrap_or(Value::Null),
                "sourceId": item.get("source_id").cloned().unwrap_or(Value::Null),
                "table": item.get("table").cloned().unwrap_or(Value::Null),
                "aggregateRole": item.get("aggregate_role").cloned().unwrap_or(Value::Null),
                "aggregateIntent": item.get("aggregate_intent").cloned().unwrap_or(Value::Null),
                "dimensions": item.get("dimensions").cloned().unwrap_or_else(|| json!([])),
                "metric": item
                    .get("metric")
                    .cloned()
                    .or_else(|| item.get("value_label").cloned())
                    .unwrap_or(Value::Null),
                "unit": item.get("unit").cloned().unwrap_or(Value::Null),
                "aggregation": item.get("aggregation").cloned().unwrap_or(Value::Null),
                "scanLimit": item.get("scan_limit").cloned().unwrap_or(Value::Null),
                "rowLimit": item.get("row_limit").cloned().unwrap_or(Value::Null),
            }))
        })
        .take(12)
        .collect()
}

fn static_page_database_aggregate_best_item<'a>(
    evidence_items: &'a [Value],
    module: &Value,
    field_path: Option<&str>,
    visualization_type: &str,
) -> Option<&'a Value> {
    let module_text = static_page_database_aggregate_module_text(module, field_path);
    let mut best_item: Option<&Value> = None;
    let mut best_score = i32::MIN;
    for item in evidence_items.iter().filter(|item| {
        item.get("type").and_then(Value::as_str) == Some("database_aggregate")
            && item
                .get("rows")
                .and_then(Value::as_array)
                .is_some_and(|rows| !rows.is_empty())
    }) {
        let score =
            static_page_database_aggregate_module_score(item, &module_text, visualization_type);
        if score > best_score {
            best_score = score;
            best_item = Some(item);
        }
    }
    best_item
}

fn static_page_database_aggregate_module_score(
    item: &Value,
    module_text: &str,
    visualization_type: &str,
) -> i32 {
    let role = item
        .get("aggregate_role")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let dimension_text = static_page_database_aggregate_dimensions_text(item);
    let visualization_type = visualization_type.to_ascii_lowercase();
    let module_wants_trend = visualization_type == "line-chart"
        || prompt_has_any(
            module_text,
            &[
                "趋势", "变化", "时间", "日期", "日", "周", "月", "txdate", "date", "time", "trend",
            ],
        );
    let module_wants_comparison = prompt_has_any(
        module_text,
        &[
            "对比",
            "分类",
            "类型",
            "类别",
            "占比",
            "分布",
            "category",
            "type",
            "class",
            "comparison",
        ],
    );
    let module_wants_ranking = visualization_type == "kpi-cards"
        || prompt_has_any(
            module_text,
            &[
                "top", "排名", "排行", "排序", "前", "最高", "最大", "区域", "门店", "点位",
                "area", "rank",
            ],
        );
    let aggregate_is_time = role == "trend"
        || prompt_has_any(
            &dimension_text,
            &["txdate", "date", "time", "day", "month", "year"],
        );
    let aggregate_is_category = role == "comparison"
        || prompt_has_any(
            &dimension_text,
            &[
                "areatype", "category", "type", "class", "kind", "status", "level",
            ],
        );
    let aggregate_is_ranking = role == "ranking"
        || prompt_has_any(
            &dimension_text,
            &["areaname", "area", "name", "title", "store", "region"],
        );

    let mut score = 1;
    if module_wants_trend {
        score += if aggregate_is_time { 60 } else { -15 };
    }
    if module_wants_comparison {
        score += if aggregate_is_category { 60 } else { -10 };
    }
    if module_wants_ranking {
        score += if aggregate_is_ranking { 45 } else { -8 };
    }
    if !module_wants_trend && !module_wants_comparison && !module_wants_ranking {
        match role {
            "ranking" => score += 12,
            "comparison" => score += 8,
            "trend" => score += 6,
            _ => {}
        }
    }
    score
}

pub(crate) fn static_page_database_aggregate_module_text(
    module: &Value,
    field_path: Option<&str>,
) -> String {
    let mut parts = Vec::new();
    for key in ["id", "title", "subtitle", "role", "description", "summary"] {
        if let Some(value) = module.get(key).and_then(Value::as_str) {
            parts.push(value);
        }
    }
    if let Some(binding) = module
        .get("dataBinding")
        .or_else(|| module.get("data_binding"))
    {
        for key in [
            "sourceId",
            "source_id",
            "fieldPath",
            "field_path",
            "field",
            "label",
        ] {
            if let Some(value) = binding.get(key).and_then(Value::as_str) {
                parts.push(value);
            }
        }
    }
    if let Some(visualization) = module.get("visualization") {
        if let Some(value) = visualization.get("type").and_then(Value::as_str) {
            parts.push(value);
        }
        if let Some(chart_options) = visualization.get("chartOptions") {
            for key in ["dataKey", "labelKey", "valueKey", "categoryKey"] {
                if let Some(value) = chart_options.get(key).and_then(Value::as_str) {
                    parts.push(value);
                }
            }
        }
    }
    if let Some(chart_options) = module.get("chartOptions") {
        for key in ["dataKey", "labelKey", "valueKey", "categoryKey"] {
            if let Some(value) = chart_options.get(key).and_then(Value::as_str) {
                parts.push(value);
            }
        }
    }
    if let Some(field_path) = field_path {
        parts.push(field_path);
    }
    parts.join(" ")
}

fn static_page_database_aggregate_dimensions_text(item: &Value) -> String {
    item.get("dimensions")
        .and_then(Value::as_array)
        .map(|dimensions| {
            dimensions
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default()
}

pub(crate) fn static_page_database_aggregate_field_path(item: &Value) -> String {
    static_page_artifact_string(item, &["metric", "value_label"])
        .map(|metric| format!("database.aggregate.{metric}"))
        .unwrap_or_else(|| "database.aggregate.value".to_string())
}

fn static_page_database_aggregate_row_value(row: &Value, item: &Value) -> Option<f64> {
    let object = row.as_object()?;
    if let Some(value) = object.get("value").and_then(static_page_json_number) {
        return Some(value);
    }
    if let Some(metric) = static_page_artifact_string(item, &["metric", "value_label"]) {
        if let Some(value) = object
            .get(metric.as_str())
            .and_then(static_page_json_number)
        {
            return Some(value);
        }
    }
    object.values().find_map(static_page_json_number)
}

fn static_page_database_aggregate_row_label(row: &Value, item: &Value, index: usize) -> String {
    let Some(object) = row.as_object() else {
        return format!("数据 {}", index + 1);
    };
    if let Some(dimensions) = item.get("dimensions").and_then(Value::as_array) {
        let label_parts = dimensions
            .iter()
            .filter_map(Value::as_str)
            .filter_map(|dimension| object.get(dimension).and_then(Value::as_str))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        if !label_parts.is_empty() {
            return label_parts.join(" / ").chars().take(32).collect();
        }
    }
    static_page_explicit_point_label(row, index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn aggregate_sample_points_from_item_use_dimensions_metric_and_metadata() {
        let item = json!({
            "type": "database_aggregate",
            "dataset_id": "dataset-1",
            "source_id": "hy-sql",
            "table": "bi_sales",
            "aggregate_role": "ranking",
            "aggregate_intent": "entity_topn",
            "dimensions": ["store", "category"],
            "metric": "sales",
            "unit": "元",
            "aggregation": "sum",
            "scan_limit": 5000,
            "row_limit": 8,
            "rows": [
                {"store": "A店", "category": "茶饮", "sales": "1,234.5"}
            ]
        });

        let points = static_page_database_aggregate_sample_points_from_item(
            &item,
            Some("dataset.metrics_summary"),
        );

        assert_eq!(points.len(), 1);
        assert_eq!(points[0].get("label"), Some(&json!("A店 / 茶饮")));
        assert_eq!(points[0].get("value"), Some(&json!(1234.5)));
        assert_eq!(points[0].get("kind"), Some(&json!("database_aggregate")));
        assert_eq!(
            points[0].get("fieldPath"),
            Some(&json!("dataset.metrics_summary"))
        );
        assert_eq!(points[0].get("metric"), Some(&json!("sales")));
        assert_eq!(points[0].get("unit"), Some(&json!("元")));
        assert_eq!(points[0].get("scanLimit"), Some(&json!(5000)));
    }

    #[test]
    fn aggregate_sample_points_from_item_cap_at_twelve_rows() {
        let rows = (0..14)
            .map(|index| json!({"store": format!("门店{index}"), "value": index + 1}))
            .collect::<Vec<_>>();
        let mut item = aggregate_item("ranking", &["store"], json!([{"store": "A", "value": 1}]));
        item["rows"] = json!(rows);

        let points = static_page_database_aggregate_sample_points_from_item(&item, None);

        assert_eq!(points.len(), 12);
        assert_eq!(points[0].get("label"), Some(&json!("门店0")));
        assert_eq!(points[11].get("label"), Some(&json!("门店11")));
        assert_eq!(
            points[0].get("fieldPath"),
            Some(&json!("database.aggregate.value"))
        );
    }

    #[test]
    fn aggregate_sample_points_pick_trend_for_line_chart() {
        let ranking = aggregate_item(
            "ranking",
            &["areaname"],
            json!([{"areaname": "百货", "value": 1856}]),
        );
        let trend = aggregate_item(
            "trend",
            &["txdate"],
            json!([{"txdate": "2026-05-01", "value": 9280}]),
        );
        let module = json!({
            "title": "日期趋势变化",
            "visualization": {"type": "line-chart"},
            "dataBinding": {"fieldPath": "dataset.metrics_summary"}
        });

        let points = build_static_page_database_aggregate_sample_points(
            &[ranking, trend],
            &module,
            Some("dataset.metrics_summary"),
        );

        assert_eq!(points.len(), 1);
        assert_eq!(points[0].get("label"), Some(&json!("2026-05-01")));
        assert_eq!(points[0].get("aggregateRole"), Some(&json!("trend")));
    }

    #[test]
    fn aggregate_sample_points_return_empty_when_module_does_not_need_rows() {
        let module = json!({
            "title": "普通说明",
            "visualization": {"type": "text-insight"}
        });

        let points = build_static_page_database_aggregate_sample_points(
            &[aggregate_item(
                "ranking",
                &["store"],
                json!([{"store": "A", "value": 1}]),
            )],
            &module,
            None,
        );

        assert!(points.is_empty());
    }

    #[test]
    fn aggregate_field_path_prefers_metric_then_value_label() {
        assert_eq!(
            static_page_database_aggregate_field_path(&json!({"metric": "sales"})),
            "database.aggregate.sales"
        );
        assert_eq!(
            static_page_database_aggregate_field_path(&json!({"value_label": "rent"})),
            "database.aggregate.rent"
        );
        assert_eq!(
            static_page_database_aggregate_field_path(&json!({})),
            "database.aggregate.value"
        );
    }

    #[test]
    fn aggregate_module_text_includes_bindings_and_chart_options() {
        let module = json!({
            "id": "trend",
            "title": "销售趋势",
            "dataBinding": {"fieldPath": "dataset.metrics_summary"},
            "visualization": {
                "type": "line-chart",
                "chartOptions": {"dataKey": "sales"}
            }
        });

        let text =
            static_page_database_aggregate_module_text(&module, Some("database.aggregate.sales"));

        assert!(text.contains("销售趋势"));
        assert!(text.contains("dataset.metrics_summary"));
        assert!(text.contains("line-chart"));
        assert!(text.contains("sales"));
        assert!(text.contains("database.aggregate.sales"));
    }

    fn aggregate_item(role: &str, dimensions: &[&str], rows: Value) -> Value {
        json!({
            "type": "database_aggregate",
            "source_id": "hy-sql",
            "table": "bi_traffic_area",
            "aggregate_role": role,
            "aggregate_intent": format!("{role}_intent"),
            "dimensions": dimensions,
            "value_label": "value",
            "rows": rows
        })
    }
}
