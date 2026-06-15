use serde_json::{json, Value};

pub(crate) fn build_static_page_dynamic_page_contract() -> Value {
    json!({
        "version": 1,
        "data_file": "data.json",
        "source_snapshot_file": "data-snapshot.json",
        "data_role": "client_refresh_snapshot",
        "default_controls": ["time_range", "primary_partition", "manual_refresh", "auto_refresh"],
        "refresh_policy": {
            "mode": "poll_data_json_when_published",
            "interval_seconds": 60,
            "change_detection_fields": ["snapshotVersion", "updatedAt", "snapshot_version", "updated_at"]
        },
        "rendering_policy": "final_html_should_render_stateful_business_modules_from_data_json_when_present"
    })
}

pub(crate) fn build_static_page_fixed_task_dynamic_page_contract() -> Value {
    json!({
        "required": true,
        "data_file": "data.json",
        "source_snapshot_file": "data-snapshot.json",
        "time_selector_required": true,
        "time_range_selector_required": true,
        "primary_partition_selector_required": true,
        "manual_refresh_required": true,
        "auto_refresh_required": true,
        "refresh_interval_seconds": 60,
        "change_detection_fields": ["snapshotVersion", "updatedAt", "snapshot_version", "updated_at"],
        "static_html_must_render_from_data_json": true,
        "report_time_range": build_static_page_report_time_range_contract(),
    })
}

pub(crate) fn build_static_page_report_time_range_contract() -> Value {
    json!({
        "required": true,
        "selector": "time_range",
        "default_granularity": "month",
        "default_preset": "latest_available_month",
        "operating_report_default": "month",
        "supported_granularities": ["month", "quarter", "year", "custom_range"],
        "field_hints": [
            "month",
            "stat_month",
            "biz_month",
            "period_month",
            "date",
            "stat_date",
            "txdate",
            "created_at",
            "updated_at"
        ],
        "fallback_policy": "when only daily dates are available, aggregate or label operating reports by month while preserving custom range selection"
    })
}

pub(crate) fn normalize_static_page_dynamic_page_contract(candidate: Value) -> Value {
    if candidate.is_null() {
        return build_static_page_dynamic_page_contract();
    }
    let mut normalized = build_static_page_dynamic_page_contract();
    if let (Some(normalized_object), Some(candidate_object)) =
        (normalized.as_object_mut(), candidate.as_object())
    {
        for (key, value) in candidate_object {
            if key == "report_time_range" || key == "time_range_selector_required" {
                continue;
            }
            normalized_object.insert(key.clone(), value.clone());
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dynamic_page_contract_contains_refresh_and_data_defaults() {
        let contract = build_static_page_dynamic_page_contract();

        assert_eq!(contract["version"], json!(1));
        assert_eq!(contract["data_file"], json!("data.json"));
        assert_eq!(
            contract["source_snapshot_file"],
            json!("data-snapshot.json")
        );
        assert_eq!(
            contract["default_controls"],
            json!([
                "time_range",
                "primary_partition",
                "manual_refresh",
                "auto_refresh"
            ])
        );
        assert_eq!(contract["refresh_policy"]["interval_seconds"], json!(60));
        assert_eq!(
            contract["rendering_policy"],
            json!("final_html_should_render_stateful_business_modules_from_data_json_when_present")
        );
    }

    #[test]
    fn fixed_task_dynamic_contract_requires_report_time_range() {
        let contract = build_static_page_fixed_task_dynamic_page_contract();

        assert_eq!(contract["required"], json!(true));
        assert_eq!(contract["time_selector_required"], json!(true));
        assert_eq!(contract["time_range_selector_required"], json!(true));
        assert_eq!(contract["primary_partition_selector_required"], json!(true));
        assert_eq!(
            contract["static_html_must_render_from_data_json"],
            json!(true)
        );
        assert_eq!(
            contract["report_time_range"]["default_preset"],
            json!("latest_available_month")
        );
        assert_eq!(
            contract["report_time_range"]["supported_granularities"],
            json!(["month", "quarter", "year", "custom_range"])
        );
    }

    #[test]
    fn report_time_range_contract_preserves_month_default_and_field_hints() {
        let contract = build_static_page_report_time_range_contract();

        assert_eq!(contract["required"], json!(true));
        assert_eq!(contract["selector"], json!("time_range"));
        assert_eq!(contract["default_granularity"], json!("month"));
        assert!(contract["field_hints"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item == "txdate")));
        assert!(contract["fallback_policy"]
            .as_str()
            .is_some_and(|policy| policy.contains("daily dates")));
    }

    #[test]
    fn normalize_dynamic_contract_uses_default_for_null() {
        let normalized = normalize_static_page_dynamic_page_contract(Value::Null);

        assert_eq!(normalized, build_static_page_dynamic_page_contract());
    }

    #[test]
    fn normalize_dynamic_contract_merges_candidate_but_keeps_controlled_time_contract_fields() {
        let normalized = normalize_static_page_dynamic_page_contract(json!({
            "data_file": "custom-data.json",
            "report_time_range": {"default_preset": "do-not-copy"},
            "time_range_selector_required": false,
            "custom_flag": true
        }));

        assert_eq!(normalized["data_file"], json!("custom-data.json"));
        assert_eq!(normalized["custom_flag"], json!(true));
        assert!(normalized.get("report_time_range").is_none());
        assert!(normalized.get("time_range_selector_required").is_none());
        assert_eq!(
            normalized["refresh_policy"]["change_detection_fields"],
            json!([
                "snapshotVersion",
                "updatedAt",
                "snapshot_version",
                "updated_at"
            ])
        );
    }
}
