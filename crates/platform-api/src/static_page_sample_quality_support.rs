use serde_json::Value;

pub(crate) fn static_page_sample_data_quality(sample_data: &Value) -> &'static str {
    let Some(items) = sample_data.as_array() else {
        return "not_available";
    };
    if items.is_empty() {
        return "not_available";
    }
    if items
        .iter()
        .any(|item| item.get("kind").and_then(Value::as_str) == Some("evidence_value"))
    {
        return "evidence_value";
    }
    if items
        .iter()
        .any(|item| item.get("kind").and_then(Value::as_str) == Some("database_aggregate"))
    {
        return "evidence_value";
    }
    if items
        .iter()
        .any(|item| item.get("kind").and_then(Value::as_str) == Some("dataset_fact_snapshot"))
    {
        return "evidence_value";
    }
    if items
        .iter()
        .any(|item| item.get("kind").and_then(Value::as_str) == Some("field_candidate_sample"))
    {
        return "evidence_value";
    }
    if items
        .iter()
        .any(|item| item.get("kind").and_then(Value::as_str) == Some("module_data"))
    {
        return "module_data";
    }
    if items
        .iter()
        .any(|item| item.get("kind").and_then(Value::as_str) == Some("database_schema"))
    {
        return "schema_context";
    }
    "evidence_signal"
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    #[test]
    fn sample_quality_reports_not_available_for_missing_or_empty_arrays() {
        assert_eq!(
            static_page_sample_data_quality(&Value::Null),
            "not_available"
        );
        assert_eq!(static_page_sample_data_quality(&json!([])), "not_available");
    }

    #[test]
    fn sample_quality_prioritizes_evidence_value_like_kinds() {
        for kind in [
            "evidence_value",
            "database_aggregate",
            "dataset_fact_snapshot",
            "field_candidate_sample",
        ] {
            let sample_data = json!([
                { "kind": "module_data" },
                { "kind": kind }
            ]);

            assert_eq!(
                static_page_sample_data_quality(&sample_data),
                "evidence_value"
            );
        }
    }

    #[test]
    fn sample_quality_reports_module_data_before_schema_context() {
        let sample_data = json!([
            { "kind": "database_schema" },
            { "kind": "module_data" }
        ]);

        assert_eq!(static_page_sample_data_quality(&sample_data), "module_data");
    }

    #[test]
    fn sample_quality_reports_schema_or_fallback_signal() {
        assert_eq!(
            static_page_sample_data_quality(&json!([{ "kind": "database_schema" }])),
            "schema_context"
        );
        assert_eq!(
            static_page_sample_data_quality(&json!([{ "kind": "media_window" }])),
            "evidence_signal"
        );
    }
}
