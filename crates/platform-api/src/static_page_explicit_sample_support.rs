use serde_json::{json, Map, Value};

pub(crate) fn build_static_page_module_explicit_points(
    module: &Value,
    field_path: Option<&str>,
) -> Vec<Value> {
    for candidate in static_page_module_explicit_data_candidates(module) {
        let points = static_page_explicit_points_from_value(candidate, field_path);
        if !points.is_empty() {
            return points;
        }
    }
    Vec::new()
}

fn static_page_module_explicit_data_candidates(module: &Value) -> Vec<&Value> {
    let mut candidates = Vec::new();
    if let Some(visualization) = module.get("visualization") {
        for key in [
            "sampleData",
            "sample_data",
            "data",
            "values",
            "rows",
            "items",
        ] {
            if let Some(value) = visualization.get(key) {
                candidates.push(value);
            }
        }
    }
    if let Some(binding) = module
        .get("dataBinding")
        .or_else(|| module.get("data_binding"))
    {
        for key in [
            "sampleData",
            "sample_data",
            "data",
            "values",
            "rows",
            "items",
        ] {
            if let Some(value) = binding.get(key) {
                candidates.push(value);
            }
        }
    }
    for key in [
        "sampleData",
        "sample_data",
        "data",
        "values",
        "rows",
        "items",
    ] {
        if let Some(value) = module.get(key) {
            candidates.push(value);
        }
    }
    candidates
}

fn static_page_explicit_points_from_value(value: &Value, field_path: Option<&str>) -> Vec<Value> {
    if let Some(array) = value.as_array() {
        return static_page_explicit_points_from_array(array, field_path);
    }
    for key in [
        "sampleData",
        "sample_data",
        "data",
        "values",
        "rows",
        "items",
    ] {
        if let Some(array) = value.get(key).and_then(Value::as_array) {
            let points = static_page_explicit_points_from_array(array, field_path);
            if !points.is_empty() {
                return points;
            }
        }
    }
    Vec::new()
}

fn static_page_explicit_points_from_array(array: &[Value], field_path: Option<&str>) -> Vec<Value> {
    array
        .iter()
        .enumerate()
        .filter_map(|(index, item)| static_page_explicit_point_from_item(item, index, field_path))
        .take(12)
        .collect()
}

fn static_page_explicit_point_from_item(
    item: &Value,
    index: usize,
    field_path: Option<&str>,
) -> Option<Value> {
    let value = static_page_explicit_point_value(item)?;
    let label = static_page_explicit_point_label(item, index);
    let mut point = Map::new();
    point.insert("label".to_string(), json!(label));
    point.insert("value".to_string(), json!(value));
    point.insert(
        "kind".to_string(),
        item.get("kind")
            .and_then(Value::as_str)
            .map(|kind| json!(kind))
            .unwrap_or_else(|| json!("module_data")),
    );
    point.insert("source".to_string(), json!("module_explicit_data"));
    if let Some(field_path) = field_path {
        point.insert("fieldPath".to_string(), json!(field_path));
    }
    Some(Value::Object(point))
}

pub(crate) fn static_page_explicit_point_value(item: &Value) -> Option<f64> {
    if let Some(value) = static_page_json_number(item) {
        return Some(value);
    }
    if let Some(array) = item.as_array() {
        return array.iter().find_map(static_page_json_number);
    }
    let object = item.as_object()?;
    for key in [
        "value",
        "amount",
        "count",
        "total",
        "score",
        "metric",
        "y",
        "订单金额",
        "金额",
        "收入",
        "数量",
    ] {
        if let Some(value) = object.get(key).and_then(static_page_json_number) {
            return Some(value);
        }
    }
    object.values().find_map(static_page_json_number)
}

pub(crate) fn static_page_explicit_point_label(item: &Value, index: usize) -> String {
    if let Some(array) = item.as_array() {
        if let Some(label) = array
            .iter()
            .find_map(|value| value.as_str().map(str::trim))
            .filter(|value| !value.is_empty())
        {
            return label.chars().take(18).collect();
        }
    }
    if let Some(object) = item.as_object() {
        for key in [
            "label", "name", "month", "date", "period", "category", "x", "月份", "日期", "分类",
        ] {
            if let Some(label) = object
                .get(key)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                return label.chars().take(18).collect();
            }
        }
        for value in object.values() {
            if let Some(label) = value
                .as_str()
                .map(str::trim)
                .filter(|candidate| !candidate.is_empty())
            {
                return label.chars().take(18).collect();
            }
        }
    }
    format!("数据 {}", index + 1)
}

pub(crate) fn static_page_json_number(value: &Value) -> Option<f64> {
    if let Some(number) = value.as_f64() {
        return Some(number);
    }
    let text = value.as_str()?.trim();
    if text.is_empty() {
        return None;
    }
    let normalized = text
        .trim_end_matches('%')
        .replace(',', "")
        .replace('，', "");
    normalized.parse::<f64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn explicit_sample_support_prefers_visualization_data_and_preserves_field_path() {
        let module = json!({
            "visualization": {
                "sampleData": [
                    { "label": "视觉A", "value": "1,200", "kind": "chart_seed" }
                ]
            },
            "dataBinding": {
                "sampleData": [
                    { "label": "绑定B", "value": 900 }
                ]
            },
            "sampleData": [
                { "label": "模块C", "value": 700 }
            ]
        });

        let points = build_static_page_module_explicit_points(&module, Some("sales.amount"));

        assert_eq!(points.len(), 1);
        assert_eq!(points[0].get("label"), Some(&json!("视觉A")));
        assert_eq!(points[0].get("value"), Some(&json!(1200.0)));
        assert_eq!(points[0].get("kind"), Some(&json!("chart_seed")));
        assert_eq!(
            points[0].get("source"),
            Some(&json!("module_explicit_data"))
        );
        assert_eq!(points[0].get("fieldPath"), Some(&json!("sales.amount")));
    }

    #[test]
    fn explicit_sample_support_reads_nested_rows_and_caps_points() {
        let rows = (0..14)
            .map(|index| json!({ "name": format!("门店{}", index), "amount": index + 1 }))
            .collect::<Vec<_>>();
        let module = json!({
            "data": {
                "rows": rows
            }
        });

        let points = build_static_page_module_explicit_points(&module, None);

        assert_eq!(points.len(), 12);
        assert_eq!(points[0].get("label"), Some(&json!("门店0")));
        assert_eq!(points[0].get("value"), Some(&json!(1.0)));
        assert!(points[0].get("fieldPath").is_none());
        assert_eq!(points[11].get("label"), Some(&json!("门店11")));
    }

    #[test]
    fn explicit_sample_support_extracts_value_and_label_from_arrays() {
        let item = json!(["2026-06", "3,456.7"]);

        assert_eq!(
            static_page_explicit_point_label(&item, 0),
            "2026-06".to_string()
        );
        assert_eq!(static_page_explicit_point_value(&item), Some(3456.7));
    }

    #[test]
    fn explicit_sample_support_parses_percent_and_chinese_numeric_keys() {
        let item = json!({
            "月份": "2026-06",
            "收入": "88.5%"
        });

        assert_eq!(
            static_page_explicit_point_label(&item, 0),
            "2026-06".to_string()
        );
        assert_eq!(static_page_explicit_point_value(&item), Some(88.5));
    }
}
