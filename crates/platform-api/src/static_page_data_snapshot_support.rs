use serde_json::{json, Value};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use crate::{
    build_static_page_data_source_candidates, build_static_page_field_candidates,
    build_static_page_module_sample_data, build_static_page_report_snapshot,
    build_static_page_structure_signals,
    build_static_page_supplemental_metrics_summary_from_candidates,
    enrich_docs_page_heading_binding, push_string_hint, static_page_artifact_string,
    static_page_heading_field_candidate, static_page_module_binding_quality,
    static_page_module_chart_runtime, static_page_payload_modules, static_page_sample_data_quality,
    static_page_template_reference_id_from_payload,
};

pub(crate) fn build_static_page_data_snapshot(payload: &Value, selected_scope: &Value) -> Value {
    let evidence_state = payload
        .get("assistant_context")
        .and_then(|context| context.get("evidence_state"));
    build_static_page_data_snapshot_with_evidence(
        payload,
        selected_scope,
        evidence_state,
        "static_page_draft",
    )
}

pub(crate) fn build_static_page_data_snapshot_with_evidence(
    payload: &Value,
    selected_scope: &Value,
    evidence_state: Option<&Value>,
    source: &str,
) -> Value {
    let data_source_candidates =
        build_static_page_data_source_candidates(selected_scope, evidence_state);
    let field_candidates = build_static_page_field_candidates(selected_scope, evidence_state);
    let supplemental_metrics = build_static_page_supplemental_metrics_summary_from_candidates(
        &field_candidates,
        evidence_state,
    );
    let is_docs_page_template =
        static_page_template_reference_id_from_payload(payload) == Some("docs-page");
    let heading_candidate = if is_docs_page_template {
        static_page_heading_field_candidate(&field_candidates)
    } else {
        None
    };
    let module_bindings = static_page_payload_modules(payload)
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|mut module| {
            let sample_data =
                build_static_page_module_sample_data(&module, evidence_state, &field_candidates);
            let data_quality = static_page_sample_data_quality(&sample_data);
            let mut binding = module
                .get("dataBinding")
                .or_else(|| module.get("data_binding"))
                .cloned()
                .unwrap_or(Value::Null);
            binding = enrich_docs_page_heading_binding(&module, binding, heading_candidate);
            if binding.is_object() {
                if let Some(object) = module.as_object_mut() {
                    object.insert("dataBinding".to_string(), binding.clone());
                    object.insert("data_binding".to_string(), binding.clone());
                }
            }
            let visualization_type = module
                .get("visualization")
                .and_then(|visualization| visualization.get("type"))
                .cloned()
                .unwrap_or_else(|| json!("text-insight"));
            let chart_runtime = static_page_module_chart_runtime(&module);
            let chart_options = module
                .get("visualization")
                .and_then(|visualization| visualization.get("chartOptions"))
                .or_else(|| module.get("chartOptions"))
                .cloned()
                .unwrap_or_else(|| json!({}));
            let binding_quality = static_page_module_binding_quality(
                &module,
                &binding,
                &field_candidates,
                &sample_data,
                data_quality,
            );
            json!({
                "moduleId": module.get("id").cloned().unwrap_or(Value::Null),
                "title": module.get("title").cloned().unwrap_or(Value::Null),
                "binding": binding,
                "visualizationType": visualization_type,
                "chartRuntime": chart_runtime,
                "chartOptions": chart_options,
                "sampleData": sample_data,
                "dataQuality": data_quality,
                "bindingQuality": binding_quality.clone(),
                "bindingQualityStatus": binding_quality
                    .get("status")
                    .cloned()
                    .unwrap_or_else(|| json!("partial")),
                "chartDataFit": binding_quality
                    .get("chartDataFit")
                    .cloned()
                    .unwrap_or_else(|| json!("unknown")),
                "recommendedAction": binding_quality
                    .get("recommendedAction")
                    .cloned()
                    .unwrap_or(Value::Null),
            })
        })
        .collect::<Vec<_>>();
    let structure_signals =
        build_static_page_structure_signals(&field_candidates, &module_bindings);
    let snapshot_version = static_page_data_snapshot_version(
        source,
        selected_scope,
        &data_source_candidates,
        &field_candidates,
        &module_bindings,
    );
    let updated_at = static_page_data_snapshot_updated_at(evidence_state);
    let validation_summary = build_static_page_data_snapshot_validation_summary(
        &data_source_candidates,
        &field_candidates,
        &module_bindings,
        &updated_at,
    );
    let report_snapshot =
        build_static_page_report_snapshot(&module_bindings, evidence_state, &validation_summary);
    json!({
        "version": 1,
        "snapshotVersion": snapshot_version,
        "snapshot_version": snapshot_version,
        "updatedAt": updated_at,
        "updated_at": updated_at,
        "source": source,
        "selected_scope": selected_scope,
        "evidence_status": evidence_state
            .and_then(|state| state.get("status"))
            .cloned()
            .unwrap_or(Value::Null),
        "refresh": {
            "dataFile": "data.json",
            "sourceSnapshotFile": "data-snapshot.json",
            "policy": "replace_data_json_then_client_refresh",
            "manualRefresh": true,
            "autoRefresh": true,
            "pollIntervalSeconds": 60,
            "changeDetectionFields": ["snapshotVersion", "updatedAt", "snapshot_version", "updated_at"],
        },
        "refresh_policy": {
            "data_file": "data.json",
            "source_snapshot_file": "data-snapshot.json",
            "policy": "replace_data_json_then_client_refresh",
            "manual_refresh": true,
            "auto_refresh": true,
            "poll_interval_seconds": 60,
            "change_detection_fields": ["snapshotVersion", "updatedAt", "snapshot_version", "updated_at"],
        },
        "data_source_candidates": data_source_candidates,
        "field_candidates": field_candidates,
        "module_bindings": module_bindings,
        "reportSnapshot": report_snapshot.clone(),
        "report_snapshot": report_snapshot.clone(),
        "kpis": report_snapshot.get("kpis").cloned().unwrap_or_else(|| json!([])),
        "filters": report_snapshot.get("filters").cloned().unwrap_or_else(|| json!([])),
        "chartSeries": report_snapshot.get("chartSeries").cloned().unwrap_or_else(|| json!([])),
        "chart_series": report_snapshot.get("chart_series").cloned().unwrap_or_else(|| json!([])),
        "businessTables": report_snapshot.get("businessTables").cloned().unwrap_or_else(|| json!([])),
        "business_tables": report_snapshot.get("business_tables").cloned().unwrap_or_else(|| json!({})),
        "evidenceNotes": report_snapshot.get("evidenceNotes").cloned().unwrap_or_else(|| json!([])),
        "evidence_notes": report_snapshot.get("evidence_notes").cloned().unwrap_or_else(|| json!([])),
        "dataQuality": report_snapshot.get("dataQuality").cloned().unwrap_or_else(|| json!({})),
        "data_quality": report_snapshot.get("data_quality").cloned().unwrap_or_else(|| json!({})),
        "structure_signals": structure_signals,
        "supplemental_metrics": supplemental_metrics.clone(),
        "supplementalMetrics": supplemental_metrics,
        "validation_summary": validation_summary,
        "sampleRowCount": validation_summary.get("sampleRowCount").cloned().unwrap_or(Value::Null),
        "detailRowCount": validation_summary.get("detailRowCount").cloned().unwrap_or(Value::Null),
        "unitHints": validation_summary.get("unitHints").cloned().unwrap_or_else(|| json!([])),
    })
}

pub(crate) fn static_page_data_snapshot_version(
    source: &str,
    selected_scope: &Value,
    data_source_candidates: &Value,
    field_candidates: &Value,
    module_bindings: &[Value],
) -> String {
    let fingerprint = json!({
        "source": source,
        "selected_scope": selected_scope,
        "data_source_candidates": data_source_candidates,
        "field_candidates": field_candidates,
        "module_bindings": module_bindings,
    });
    let serialized = serde_json::to_string(&fingerprint).unwrap_or_else(|_| source.to_string());
    let mut hasher = DefaultHasher::new();
    serialized.hash(&mut hasher);
    format!("static-page-data-v1-{:016x}", hasher.finish())
}

pub(crate) fn static_page_data_snapshot_updated_at(evidence_state: Option<&Value>) -> Value {
    let Some(evidence_state) = evidence_state else {
        return Value::Null;
    };
    let mut timestamps = Vec::<String>::new();
    collect_static_page_data_snapshot_timestamp_hints(evidence_state, &mut timestamps);
    if let Some(items) = evidence_state
        .get("supplied_items")
        .or_else(|| evidence_state.get("suppliedItems"))
        .and_then(Value::as_array)
    {
        for item in items {
            collect_static_page_data_snapshot_timestamp_hints(item, &mut timestamps);
        }
    }
    timestamps.sort();
    timestamps.pop().map(Value::String).unwrap_or(Value::Null)
}

fn collect_static_page_data_snapshot_timestamp_hints(value: &Value, timestamps: &mut Vec<String>) {
    for key in [
        "updated_at",
        "updatedAt",
        "indexed_at",
        "indexedAt",
        "generated_at",
        "generatedAt",
        "snapshot_at",
        "snapshotAt",
        "completed_at",
        "completedAt",
        "created_at",
        "createdAt",
    ] {
        if let Some(timestamp) = value.get(key).and_then(Value::as_str) {
            let timestamp = timestamp.trim();
            if !timestamp.is_empty() {
                push_string_hint(timestamps, timestamp);
            }
        }
    }
}

pub(crate) fn build_static_page_data_snapshot_validation_summary(
    data_source_candidates: &Value,
    field_candidates: &Value,
    module_bindings: &[Value],
    updated_at: &Value,
) -> Value {
    let mut module_count = 0usize;
    let mut bound_module_count = 0usize;
    let mut ready_module_count = 0usize;
    let mut missing_binding_count = 0usize;
    let mut needs_sample_rows_count = 0usize;
    let mut sample_row_count = 0usize;
    let mut detail_row_count = 0usize;
    let mut unit_hints = Vec::<String>::new();

    if let Some(candidates) = field_candidates.as_array() {
        for candidate in candidates {
            collect_static_page_unit_hints(candidate, &mut unit_hints);
        }
    }

    for module in module_bindings {
        module_count += 1;
        let status = module
            .get("bindingQualityStatus")
            .or_else(|| module.get("binding_quality_status"))
            .and_then(Value::as_str)
            .unwrap_or("partial");
        let chart_data_fit = module
            .get("chartDataFit")
            .or_else(|| module.get("chart_data_fit"))
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        if status != "missing" {
            bound_module_count += 1;
        }
        if status == "confirmed" || matches!(chart_data_fit, "ready" | "not_required") {
            ready_module_count += 1;
        }
        if status == "missing" {
            missing_binding_count += 1;
        }
        if chart_data_fit == "needs_sample_rows" {
            needs_sample_rows_count += 1;
        }
        if let Some(rows) = module.get("sampleData").and_then(Value::as_array) {
            sample_row_count += rows.len();
            detail_row_count += rows.iter().filter(|row| row.is_object()).count();
            for row in rows {
                collect_static_page_unit_hints(row, &mut unit_hints);
            }
        }
    }

    unit_hints.truncate(12);
    let status = if module_count == 0 {
        "missing"
    } else if missing_binding_count > 0 || needs_sample_rows_count > 0 {
        "partial"
    } else {
        "ready"
    };

    json!({
        "version": 1,
        "status": status,
        "moduleCount": module_count,
        "boundModuleCount": bound_module_count,
        "readyModuleCount": ready_module_count,
        "missingBindingCount": missing_binding_count,
        "needsSampleRowsCount": needs_sample_rows_count,
        "sampleRowCount": sample_row_count,
        "detailRowCount": detail_row_count,
        "fieldCandidateCount": field_candidates.as_array().map(Vec::len).unwrap_or(0),
        "dataSourceCandidateCount": data_source_candidates.as_array().map(Vec::len).unwrap_or(0),
        "snapshotDate": updated_at.clone(),
        "unitHints": unit_hints,
    })
}

fn collect_static_page_unit_hints(value: &Value, hints: &mut Vec<String>) {
    let metric = static_page_artifact_string(value, &["metric", "valueLabel", "value_label"]);
    let aggregation =
        static_page_artifact_string(value, &["aggregation", "recommendedAggregation"]);
    if let Some(metric) = metric.as_deref() {
        push_string_hint(hints, metric);
        if let Some(aggregation) = aggregation.as_deref() {
            push_string_hint(hints, &format!("{aggregation}({metric})"));
        }
    }
    for key in [
        "unit",
        "unitHint",
        "unit_hint",
        "valueUnit",
        "value_unit",
        "displayUnit",
        "display_unit",
    ] {
        if let Some(unit) = value.get(key).and_then(Value::as_str) {
            let unit = unit.trim();
            if !unit.is_empty() {
                push_string_hint(hints, unit);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_version_is_stable_for_same_inputs() {
        let selected_scope = json!({"datasets": ["dataset-1"]});
        let data_sources = json!([{"sourceId": "dataset"}]);
        let field_candidates = json!([{"fieldPath": "dataset.metrics_summary"}]);
        let modules = vec![json!({"moduleId": "sales", "sampleData": []})];

        let first = static_page_data_snapshot_version(
            "assistant_run",
            &selected_scope,
            &data_sources,
            &field_candidates,
            &modules,
        );
        let second = static_page_data_snapshot_version(
            "assistant_run",
            &selected_scope,
            &data_sources,
            &field_candidates,
            &modules,
        );

        assert_eq!(first, second);
        assert!(first.starts_with("static-page-data-v1-"));
    }

    #[test]
    fn data_snapshot_wrapper_uses_assistant_context_evidence_state() {
        let payload = json!({
            "modules": [],
            "assistant_context": {
                "evidence_state": {
                    "status": "ready",
                    "updated_at": "2026-06-20T00:00:00Z",
                    "supplied_items": []
                }
            }
        });
        let selected_scope = json!({"dataset_ids": ["dataset-1"]});

        let snapshot = build_static_page_data_snapshot(&payload, &selected_scope);

        assert_eq!(snapshot["source"], json!("static_page_draft"));
        assert_eq!(snapshot["selected_scope"], selected_scope);
        assert_eq!(snapshot["evidence_status"], json!("ready"));
        assert_eq!(snapshot["updatedAt"], json!("2026-06-20T00:00:00Z"));
        assert_eq!(snapshot["refresh"]["manualRefresh"], json!(true));
        assert!(snapshot["snapshotVersion"]
            .as_str()
            .is_some_and(|value| value.starts_with("static-page-data-v1-")));
    }

    #[test]
    fn updated_at_uses_latest_timestamp_hint_from_state_or_items() {
        let evidence_state = json!({
            "updated_at": "2026-05-01T00:00:00Z",
            "supplied_items": [
                {"type": "retrieval_evidence", "created_at": "2026-05-02T00:00:00Z"},
                {"type": "database_aggregate", "updatedAt": "2026-05-03T00:00:00Z"}
            ]
        });

        assert_eq!(
            static_page_data_snapshot_updated_at(Some(&evidence_state)),
            json!("2026-05-03T00:00:00Z")
        );
        assert_eq!(static_page_data_snapshot_updated_at(None), Value::Null);
    }

    #[test]
    fn validation_summary_counts_modules_rows_and_units() {
        let data_sources = json!([{"sourceId": "dataset"}, {"sourceId": "database_aggregate"}]);
        let field_candidates = json!([
            {"fieldPath": "database.aggregate.sales", "metric": "sales", "aggregation": "sum", "unit": "元"},
            {"fieldPath": "traffic.count", "valueLabel": "traffic", "unitHint": "人"}
        ]);
        let modules = vec![
            json!({
                "moduleId": "sales",
                "bindingQualityStatus": "confirmed",
                "chartDataFit": "ready",
                "sampleData": [
                    {"label": "A", "value": 10, "metric": "sales", "aggregation": "sum", "unit": "元"},
                    {"label": "B", "value": 8, "unit": "元"}
                ]
            }),
            json!({
                "moduleId": "missing",
                "binding_quality_status": "missing",
                "chart_data_fit": "needs_sample_rows"
            }),
        ];

        let summary = build_static_page_data_snapshot_validation_summary(
            &data_sources,
            &field_candidates,
            &modules,
            &json!("2026-05-03T00:00:00Z"),
        );

        assert_eq!(summary["status"], json!("partial"));
        assert_eq!(summary["moduleCount"], json!(2));
        assert_eq!(summary["boundModuleCount"], json!(1));
        assert_eq!(summary["readyModuleCount"], json!(1));
        assert_eq!(summary["missingBindingCount"], json!(1));
        assert_eq!(summary["needsSampleRowsCount"], json!(1));
        assert_eq!(summary["sampleRowCount"], json!(2));
        assert_eq!(summary["detailRowCount"], json!(2));
        assert_eq!(summary["fieldCandidateCount"], json!(2));
        assert_eq!(summary["dataSourceCandidateCount"], json!(2));
        assert_eq!(summary["snapshotDate"], json!("2026-05-03T00:00:00Z"));
        assert_eq!(summary["unitHints"][0], json!("sales"));
        assert!(summary["unitHints"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item == "sum(sales)")));
        assert!(summary["unitHints"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item == "元")));
    }
}
