use crate::static_page_explicit_sample_support::{
    static_page_explicit_point_label, static_page_explicit_point_value,
};
use crate::static_page_module_binding_support::{
    static_page_matching_field_candidate, static_page_module_source_id,
};
use serde_json::{json, Value};

pub(crate) fn build_static_page_field_candidate_sample_points(
    field_candidates: &Value,
    module: &Value,
    field_path: &str,
) -> Vec<Value> {
    let source_id = static_page_module_source_id(module);
    let Some(candidate) =
        static_page_matching_field_candidate(field_candidates, &source_id, field_path)
    else {
        return Vec::new();
    };
    static_page_field_candidate_sample_points(candidate, field_path)
}

pub(crate) fn static_page_field_candidate_sample_points(
    candidate: &Value,
    field_path: &str,
) -> Vec<Value> {
    for key in [
        "sampleData",
        "sample_data",
        "rows",
        "items",
        "values",
        "data",
        "sampleRows",
        "sample_rows",
    ] {
        let Some(array) = candidate.get(key).and_then(Value::as_array) else {
            continue;
        };
        let points = array
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                static_page_field_candidate_sample_point(item, index, field_path)
            })
            .take(12)
            .collect::<Vec<_>>();
        if !points.is_empty() {
            return points;
        }
    }
    Vec::new()
}

pub(crate) fn static_page_field_candidate_sample_point(
    item: &Value,
    index: usize,
    field_path: &str,
) -> Option<Value> {
    let value = static_page_explicit_point_value(item)?;
    let label = static_page_explicit_point_label(item, index);
    let mut point = item.as_object().cloned().unwrap_or_default();
    point
        .entry("label".to_string())
        .or_insert_with(|| json!(label));
    point.insert("value".to_string(), json!(value));
    point
        .entry("kind".to_string())
        .or_insert_with(|| json!("field_candidate_sample"));
    point
        .entry("source".to_string())
        .or_insert_with(|| json!("field_candidate"));
    point
        .entry("fieldPath".to_string())
        .or_insert_with(|| json!(field_path));
    Some(Value::Object(point))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn field_candidate_sample_points_prefers_first_non_empty_sample_key() {
        let candidate = json!({
            "sampleData": [
                { "label": "无值" }
            ],
            "rows": [
                { "label": "门店A", "value": "1,200" }
            ]
        });

        let points = static_page_field_candidate_sample_points(&candidate, "sales.amount");

        assert_eq!(points.len(), 1);
        assert_eq!(points[0].get("label"), Some(&json!("门店A")));
        assert_eq!(points[0].get("value"), Some(&json!(1200.0)));
        assert_eq!(
            points[0].get("kind"),
            Some(&json!("field_candidate_sample"))
        );
        assert_eq!(points[0].get("source"), Some(&json!("field_candidate")));
        assert_eq!(points[0].get("fieldPath"), Some(&json!("sales.amount")));
    }

    #[test]
    fn field_candidate_sample_points_caps_at_twelve_rows() {
        let rows = (0..14)
            .map(|index| json!({ "name": format!("门店{}", index), "amount": index + 1 }))
            .collect::<Vec<_>>();
        let candidate = json!({ "sample_rows": rows });

        let points = static_page_field_candidate_sample_points(&candidate, "orders.amount");

        assert_eq!(points.len(), 12);
        assert_eq!(points[0].get("label"), Some(&json!("门店0")));
        assert_eq!(points[11].get("label"), Some(&json!("门店11")));
    }

    #[test]
    fn field_candidate_sample_point_preserves_existing_metadata_but_refreshes_value() {
        let point = static_page_field_candidate_sample_point(
            &json!({
                "label": "已有标签",
                "value": "88.5%",
                "kind": "custom_kind",
                "source": "custom_source",
                "fieldPath": "custom.path"
            }),
            0,
            "orders.amount",
        )
        .expect("point");

        assert_eq!(point.get("label"), Some(&json!("已有标签")));
        assert_eq!(point.get("value"), Some(&json!(88.5)));
        assert_eq!(point.get("kind"), Some(&json!("custom_kind")));
        assert_eq!(point.get("source"), Some(&json!("custom_source")));
        assert_eq!(point.get("fieldPath"), Some(&json!("custom.path")));
    }

    #[test]
    fn field_candidate_sample_point_supports_array_items() {
        let point = static_page_field_candidate_sample_point(
            &json!(["2026-06", "3,456.7"]),
            0,
            "time.month",
        )
        .expect("point");

        assert_eq!(point.get("label"), Some(&json!("2026-06")));
        assert_eq!(point.get("value"), Some(&json!(3456.7)));
        assert_eq!(point.get("kind"), Some(&json!("field_candidate_sample")));
        assert_eq!(point.get("fieldPath"), Some(&json!("time.month")));
    }

    #[test]
    fn build_field_candidate_sample_points_matches_module_source_id() {
        let field_candidates = json!([
            {
                "sourceId": "database_aggregate",
                "fieldPath": "orders.amount",
                "sampleData": [
                    {"label": "门店A", "value": "1,200"}
                ]
            },
            {
                "sourceId": "dataset",
                "fieldPath": "orders.amount",
                "sampleData": [
                    {"label": "数据集", "value": 1}
                ]
            }
        ]);
        let module = json!({
            "dataBinding": {
                "sourceId": "database_aggregate",
                "fieldPath": "orders.amount"
            }
        });

        let points = build_static_page_field_candidate_sample_points(
            &field_candidates,
            &module,
            "orders.amount",
        );

        assert_eq!(points.len(), 1);
        assert_eq!(points[0].get("label"), Some(&json!("门店A")));
        assert_eq!(points[0].get("value"), Some(&json!(1200.0)));
    }

    #[test]
    fn build_field_candidate_sample_points_accepts_empty_module_source_id() {
        let field_candidates = json!([
            {
                "sourceId": "dataset",
                "fieldPath": "risk.level",
                "sampleData": [
                    {"label": "风险", "value": 3}
                ]
            }
        ]);
        let module = json!({
            "dataBinding": {
                "fieldPath": "risk.level"
            }
        });

        let points = build_static_page_field_candidate_sample_points(
            &field_candidates,
            &module,
            "risk.level",
        );

        assert_eq!(points.len(), 1);
        assert_eq!(points[0].get("label"), Some(&json!("风险")));
    }

    #[test]
    fn build_field_candidate_sample_points_rejects_wrong_source_id() {
        let field_candidates = json!([
            {
                "sourceId": "dataset",
                "fieldPath": "orders.amount",
                "sampleData": [
                    {"label": "数据集", "value": 1}
                ]
            }
        ]);
        let module = json!({
            "dataBinding": {
                "sourceId": "database_aggregate",
                "fieldPath": "orders.amount"
            }
        });

        let points = build_static_page_field_candidate_sample_points(
            &field_candidates,
            &module,
            "orders.amount",
        );

        assert!(points.is_empty());
    }
}
