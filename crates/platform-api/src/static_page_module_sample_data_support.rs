use crate::static_page_database_aggregate_sample_support::build_static_page_database_aggregate_sample_points;
use crate::static_page_database_schema_sample_support::build_static_page_database_schema_sample_points;
use crate::static_page_dataset_fact_snapshot_sample_support::build_static_page_dataset_fact_snapshot_sample_points;
use crate::static_page_evidence_signal_support::{
    build_static_page_explicit_metric_points, static_page_evidence_ids,
    static_page_evidence_point_label, static_page_field_keywords, static_page_keyword_signal_score,
};
use crate::static_page_explicit_sample_support::build_static_page_module_explicit_points;
use crate::static_page_field_candidate_sample_support::build_static_page_field_candidate_sample_points;
use crate::static_page_media_sample_support::build_static_page_media_sample_points;
use crate::static_page_module_binding_support::static_page_module_field_path;
use serde_json::{json, Value};

pub(crate) fn build_static_page_module_sample_data(
    module: &Value,
    evidence_state: Option<&Value>,
    field_candidates: &Value,
) -> Value {
    let field_path = static_page_module_field_path(module);
    let explicit_points = build_static_page_module_explicit_points(module, field_path);
    if !explicit_points.is_empty() {
        return Value::Array(explicit_points);
    }
    let evidence_items = evidence_state
        .and_then(|state| state.get("supplied_items"))
        .and_then(Value::as_array);
    if let Some(evidence_items) = evidence_items {
        if let Some(field_path) = field_path.filter(|field_path| field_path.starts_with("media.")) {
            let media_points = build_static_page_media_sample_points(evidence_items, field_path);
            if !media_points.is_empty() {
                return Value::Array(media_points);
            }
        }

        if field_path.is_some_and(|field_path| field_path.starts_with("dataset.fact_snapshot")) {
            let fact_snapshot_points = build_static_page_dataset_fact_snapshot_sample_points(
                evidence_items,
                module,
                field_path,
            );
            if !fact_snapshot_points.is_empty() {
                return Value::Array(fact_snapshot_points);
            }
        }

        let schema_points =
            build_static_page_database_schema_sample_points(evidence_items, module, field_path);
        if !schema_points.is_empty() {
            return Value::Array(schema_points);
        }

        let database_points =
            build_static_page_database_aggregate_sample_points(evidence_items, module, field_path);
        if !database_points.is_empty() {
            return Value::Array(database_points);
        }

        let fact_snapshot_points = build_static_page_dataset_fact_snapshot_sample_points(
            evidence_items,
            module,
            field_path,
        );
        if !fact_snapshot_points.is_empty() {
            return Value::Array(fact_snapshot_points);
        }
    }

    let Some(field_path) = field_path else {
        return json!([]);
    };

    let candidate_points =
        build_static_page_field_candidate_sample_points(field_candidates, module, field_path);
    if !candidate_points.is_empty() {
        return Value::Array(candidate_points);
    }

    let Some(evidence_items) = evidence_items else {
        return json!([]);
    };

    let keywords = static_page_field_keywords(field_path);
    if keywords.is_empty() {
        return json!([]);
    }

    let explicit_points =
        build_static_page_explicit_metric_points(evidence_items, field_path, &keywords);
    if !explicit_points.is_empty() {
        return Value::Array(explicit_points);
    }

    let points = evidence_items
        .iter()
        .filter(|item| {
            item.get("type").and_then(Value::as_str).unwrap_or_default() == "retrieval_evidence"
        })
        .enumerate()
        .filter_map(|(index, item)| {
            let value = static_page_keyword_signal_score(item, &keywords);
            if value <= 0.0 {
                return None;
            }
            Some(json!({
                "label": static_page_evidence_point_label(item, index),
                "value": value,
                "kind": "evidence_signal",
                "fieldPath": field_path,
                "evidenceIds": static_page_evidence_ids(item),
            }))
        })
        .take(6)
        .collect::<Vec<_>>();
    Value::Array(points)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn module_sample_data_prefers_explicit_module_rows() {
        let module = json!({
            "visualization": {
                "type": "bar-chart",
                "sampleData": [
                    { "label": "显式模块", "value": "1,200" }
                ]
            },
            "dataBinding": {
                "sourceId": "dataset",
                "fieldPath": "orders.amount"
            }
        });
        let evidence_state = json!({
            "supplied_items": [{
                "type": "retrieval_evidence",
                "retrieval_evidence_id": "ev-1",
                "content_excerpt": "收入, 999, 门店A"
            }]
        });
        let field_candidates = json!([{
            "sourceId": "dataset",
            "fieldPath": "orders.amount",
            "sampleData": [{ "label": "候选字段", "value": 10 }]
        }]);

        let sample_data =
            build_static_page_module_sample_data(&module, Some(&evidence_state), &field_candidates);

        assert_eq!(sample_data.pointer("/0/label"), Some(&json!("显式模块")));
        assert_eq!(sample_data.pointer("/0/value"), Some(&json!(1200.0)));
        assert_eq!(
            sample_data.pointer("/0/source"),
            Some(&json!("module_explicit_data"))
        );
    }

    #[test]
    fn module_sample_data_prefers_media_windows_before_field_candidates() {
        let module = json!({
            "visualization": { "type": "table" },
            "dataBinding": {
                "sourceId": "evidence",
                "fieldPath": "media.transcript_windows"
            }
        });
        let evidence_state = json!({
            "supplied_items": [{
                "type": "retrieval_evidence",
                "retrieval_evidence_id": "ev-1",
                "source_locator": "call.mp3#chunk=1",
                "media_context": {
                    "transcript_windows": [{
                        "start_seconds": 1.0,
                        "text": "客户咨询经营报表"
                    }]
                }
            }]
        });
        let field_candidates = json!([{
            "sourceId": "evidence",
            "fieldPath": "media.transcript_windows",
            "sampleData": [{ "label": "字段候选", "value": 9 }]
        }]);

        let sample_data =
            build_static_page_module_sample_data(&module, Some(&evidence_state), &field_candidates);

        assert_eq!(sample_data.pointer("/0/kind"), Some(&json!("media_window")));
        assert_eq!(
            sample_data.pointer("/0/text"),
            Some(&json!("客户咨询经营报表"))
        );
    }

    #[test]
    fn module_sample_data_uses_field_candidate_when_evidence_has_no_rows() {
        let module = json!({
            "visualization": { "type": "bar-chart" },
            "dataBinding": {
                "sourceId": "dataset",
                "fieldPath": "orders.amount"
            }
        });
        let field_candidates = json!([{
            "sourceId": "dataset",
            "fieldPath": "orders.amount",
            "sampleData": [{ "label": "门店A", "value": "2,345" }]
        }]);

        let sample_data = build_static_page_module_sample_data(&module, None, &field_candidates);

        assert_eq!(sample_data.pointer("/0/label"), Some(&json!("门店A")));
        assert_eq!(sample_data.pointer("/0/value"), Some(&json!(2345.0)));
        assert_eq!(
            sample_data.pointer("/0/kind"),
            Some(&json!("field_candidate_sample"))
        );
    }

    #[test]
    fn module_sample_data_extracts_explicit_metric_points_before_signal() {
        let module = json!({
            "visualization": { "type": "line-chart" },
            "dataBinding": {
                "fieldPath": "orders.amount"
            }
        });
        let evidence_state = json!({
            "supplied_items": [{
                "type": "retrieval_evidence",
                "retrieval_evidence_id": "ev-1",
                "source_locator": "report.xlsx#sheet1",
                "content_excerpt": "收入, 1,234, 门店A",
                "score": 0.9
            }]
        });

        let sample_data =
            build_static_page_module_sample_data(&module, Some(&evidence_state), &json!([]));

        assert_eq!(
            sample_data.pointer("/0/kind"),
            Some(&json!("evidence_value"))
        );
        assert_eq!(sample_data.pointer("/0/label"), Some(&json!("门店A")));
        assert_eq!(sample_data.pointer("/0/value"), Some(&json!(1234.0)));
        assert_eq!(
            sample_data.pointer("/0/evidenceIds/0"),
            Some(&json!("ev-1"))
        );
    }

    #[test]
    fn module_sample_data_falls_back_to_keyword_signal() {
        let module = json!({
            "visualization": { "type": "bar-chart" },
            "dataBinding": {
                "fieldPath": "orders.amount"
            }
        });
        let evidence_state = json!({
            "supplied_items": [{
                "type": "retrieval_evidence",
                "retrieval_evidence_id": "ev-1",
                "source_locator": "report.md#section",
                "summary": "本月收入和销售表现需要关注",
                "score": 0.4
            }]
        });

        let sample_data =
            build_static_page_module_sample_data(&module, Some(&evidence_state), &json!([]));

        assert_eq!(
            sample_data.pointer("/0/kind"),
            Some(&json!("evidence_signal"))
        );
        assert_eq!(
            sample_data.pointer("/0/fieldPath"),
            Some(&json!("orders.amount"))
        );
        assert_eq!(
            sample_data.pointer("/0/evidenceIds/0"),
            Some(&json!("ev-1"))
        );
    }

    #[test]
    fn module_sample_data_returns_empty_without_field_path_or_explicit_rows() {
        let module = json!({
            "visualization": { "type": "text-insight" }
        });
        let evidence_state = json!({
            "supplied_items": [{
                "type": "retrieval_evidence",
                "content_excerpt": "收入, 1,234, 门店A"
            }]
        });

        let sample_data =
            build_static_page_module_sample_data(&module, Some(&evidence_state), &json!([]));

        assert_eq!(sample_data, json!([]));
    }
}
