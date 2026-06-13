use serde_json::{json, Map, Value};

use crate::truncate_assistant_supply_text;

pub(crate) fn build_static_page_report_snapshot(
    module_bindings: &[Value],
    evidence_state: Option<&Value>,
    validation_summary: &Value,
) -> Value {
    let mut kpis = Vec::new();
    let mut chart_series = Vec::new();
    let mut business_tables = Vec::new();
    let mut business_tables_by_id = Map::new();

    for (index, binding) in module_bindings.iter().enumerate() {
        let module_id = binding
            .get("moduleId")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| format!("module_{index}"));
        let title = binding
            .get("title")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| module_id.clone());
        let visualization_type = binding
            .get("visualizationType")
            .and_then(Value::as_str)
            .unwrap_or("text-insight");
        let sample_rows = binding
            .get("sampleData")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if sample_rows.is_empty() {
            continue;
        }

        let chart_points = sample_rows
            .iter()
            .filter_map(static_page_report_chart_point_from_sample)
            .collect::<Vec<_>>();
        if !chart_points.is_empty() {
            chart_series.push(json!({
                "moduleId": module_id,
                "title": title,
                "type": visualization_type,
                "points": chart_points,
            }));
        }

        if let Some(kpi) = static_page_report_kpi_from_binding(binding, &sample_rows) {
            kpis.push(kpi);
        }

        let business_rows = sample_rows
            .iter()
            .filter(|row| static_page_report_sample_row_is_business(row))
            .filter_map(static_page_report_business_row_from_sample)
            .collect::<Vec<_>>();
        if !business_rows.is_empty() {
            let table_id = static_page_report_table_id(&module_id);
            business_tables_by_id.insert(table_id.clone(), Value::Array(business_rows.clone()));
            business_tables.push(json!({
                "id": table_id,
                "moduleId": module_id,
                "title": title,
                "rows": business_rows,
            }));
        }
    }

    let evidence_notes = static_page_report_evidence_notes(evidence_state);
    let filters = json!([
        {
            "id": "time_range",
            "label": "时间范围",
            "required": true,
            "default": "latest_available_month"
        },
        {
            "id": "primary_partition",
            "label": "主筛选维度",
            "required": false,
            "default": "auto"
        }
    ]);
    let data_quality = json!({
        "status": validation_summary
            .get("status")
            .cloned()
            .unwrap_or_else(|| json!("unknown")),
        "sampleRowCount": validation_summary
            .get("sampleRowCount")
            .cloned()
            .unwrap_or(Value::Null),
        "detailRowCount": validation_summary
            .get("detailRowCount")
            .cloned()
            .unwrap_or(Value::Null),
        "source": "report_snapshot_v1",
    });

    json!({
        "schema": "v3.report_snapshot",
        "schemaVersion": 1,
        "kpis": kpis,
        "filters": filters,
        "chartSeries": chart_series,
        "chart_series": chart_series,
        "businessTables": business_tables,
        "business_tables": Value::Object(business_tables_by_id),
        "evidenceNotes": evidence_notes,
        "evidence_notes": evidence_notes,
        "dataQuality": data_quality,
        "data_quality": data_quality,
    })
}

fn static_page_report_chart_point_from_sample(row: &Value) -> Option<Value> {
    let value = row.get("value").and_then(Value::as_f64)?;
    let label = row
        .get("label")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("未命名");
    let mut point = Map::new();
    point.insert("label".to_string(), json!(label));
    point.insert("value".to_string(), json!(value));
    for key in ["unit", "metric", "table", "kind", "fieldPath"] {
        if let Some(value) = row.get(key).filter(|value| !value.is_null()) {
            point.insert(key.to_string(), value.clone());
        }
    }
    Some(Value::Object(point))
}

fn static_page_report_kpi_from_binding(binding: &Value, sample_rows: &[Value]) -> Option<Value> {
    let first = sample_rows
        .iter()
        .find(|row| row.get("value").and_then(Value::as_f64).is_some())?;
    Some(json!({
        "moduleId": binding.get("moduleId").cloned().unwrap_or(Value::Null),
        "title": binding.get("title").cloned().unwrap_or(Value::Null),
        "label": first.get("label").cloned().unwrap_or(Value::Null),
        "value": first.get("value").cloned().unwrap_or(Value::Null),
        "unit": first.get("unit").cloned().unwrap_or(Value::Null),
        "source": first.get("source").cloned().unwrap_or(Value::Null),
    }))
}

fn static_page_report_sample_row_is_business(row: &Value) -> bool {
    let kind = row.get("kind").and_then(Value::as_str).unwrap_or_default();
    let source = row
        .get("source")
        .and_then(Value::as_str)
        .unwrap_or_default();
    matches!(
        kind,
        "database_aggregate"
            | "dataset_fact_snapshot"
            | "field_candidate_sample"
            | "explicit_metric"
            | "explicit_data"
    ) || matches!(
        source,
        "database_aggregate" | "dataset_fact_snapshot" | "field_candidate" | "dataset"
    )
}

fn static_page_report_business_row_from_sample(row: &Value) -> Option<Value> {
    let object = row.as_object()?;
    let mut cleaned = Map::new();
    for (key, value) in object {
        if static_page_report_business_row_key_is_internal_metadata(key) {
            continue;
        }
        if value.is_null() {
            continue;
        }
        cleaned.insert(key.clone(), value.clone());
    }
    if cleaned.is_empty() {
        return None;
    }
    Some(Value::Object(cleaned))
}

fn static_page_report_business_row_key_is_internal_metadata(key: &str) -> bool {
    let compact = key.to_ascii_lowercase().replace(['_', '-', ' '], "");
    [
        "datasetid",
        "documentchunkid",
        "documentid",
        "retrievalevidenceid",
        "evidenceids",
        "evidenceref",
        "sourcedocumentid",
        "sourcedocumentids",
        "sourcelocator",
        "sourcelocators",
        "sourceid",
        "sourcefactcount",
        "sourcedocumentcount",
        "scanlimit",
        "rowlimit",
    ]
    .iter()
    .any(|term| compact.contains(term))
}

pub(crate) fn static_page_report_table_id(module_id: &str) -> String {
    let mut output = module_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    while output.contains("__") {
        output = output.replace("__", "_");
    }
    let output = output.trim_matches('_');
    if output.is_empty() {
        "business_table".to_string()
    } else {
        output.to_string()
    }
}

fn static_page_report_evidence_notes(evidence_state: Option<&Value>) -> Vec<Value> {
    let Some(items) = evidence_state
        .and_then(|state| state.get("supplied_items"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };

    let mut notes = Vec::new();
    for item in items {
        match item.get("type").and_then(Value::as_str) {
            Some("retrieval_evidence") => {
                let Some(summary) = item
                    .get("summary")
                    .or_else(|| item.get("content_excerpt"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                else {
                    continue;
                };
                notes.push(json!({
                    "title": item
                        .get("title")
                        .or_else(|| item.get("document_title"))
                        .cloned()
                        .unwrap_or(Value::Null),
                    "summary": truncate_assistant_supply_text(summary, 600),
                    "sourceLocator": item.get("source_locator").cloned().unwrap_or(Value::Null),
                    "role": "supporting_evidence",
                }));
            }
            Some("dataset_fact_snapshot") => {
                let row_count_by_type = item
                    .get("row_count_by_type")
                    .or_else(|| item.pointer("/snapshot_manifest/row_count_by_type"))
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                let Some(summary) = item.get("summary").and_then(Value::as_str).map(str::trim)
                else {
                    continue;
                };
                notes.push(json!({
                    "title": item
                        .get("dataset_title")
                        .cloned()
                        .unwrap_or_else(|| json!("数据集结构化事实快照")),
                    "summary": truncate_assistant_supply_text(summary, 600),
                    "role": "structured_fact_snapshot",
                    "source": item.get("source").cloned().unwrap_or_else(|| json!("dataset_fact_snapshots")),
                    "snapshotKind": item.get("snapshot_kind").cloned().unwrap_or(Value::Null),
                    "snapshotKey": item.get("snapshot_key").cloned().unwrap_or(Value::Null),
                    "scannedDocumentCount": item
                        .get("scanned_document_count")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "sourceFactCount": item
                        .get("source_fact_count")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "rowCountByType": row_count_by_type,
                }));
            }
            _ => {}
        }
        if notes.len() >= 12 {
            break;
        }
    }
    notes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_snapshot_builds_charts_kpis_and_business_tables() {
        let bindings = vec![json!({
            "moduleId": "top-area",
            "title": "区域排行",
            "visualizationType": "bar-chart",
            "sampleData": [
                {
                    "label": "百货",
                    "value": 1856.0,
                    "unit": "MB",
                    "source": "database_aggregate",
                    "kind": "database_aggregate",
                    "datasetId": "hidden",
                    "sourceId": "hidden",
                    "scanLimit": 5000
                }
            ]
        })];

        let snapshot = build_static_page_report_snapshot(
            &bindings,
            None,
            &json!({"status": "ready", "sampleRowCount": 1, "detailRowCount": 1}),
        );

        assert_eq!(snapshot["schema"], json!("v3.report_snapshot"));
        assert_eq!(snapshot["kpis"][0]["label"], json!("百货"));
        assert_eq!(
            snapshot["chartSeries"][0]["points"][0]["value"],
            json!(1856.0)
        );
        assert_eq!(
            snapshot["business_tables"]["top_area"][0]["label"],
            json!("百货")
        );
        assert!(snapshot["business_tables"]["top_area"][0]
            .get("datasetId")
            .is_none());
        assert!(snapshot["business_tables"]["top_area"][0]
            .get("sourceId")
            .is_none());
        assert_eq!(snapshot["dataQuality"]["status"], json!("ready"));
    }

    #[test]
    fn table_id_normalizes_module_ids() {
        assert_eq!(
            static_page_report_table_id("Top Area / 销售-榜"),
            "top_area"
        );
        assert_eq!(static_page_report_table_id("___"), "business_table");
    }

    #[test]
    fn evidence_notes_include_supported_evidence_types() {
        let evidence_state = json!({
            "supplied_items": [
                {
                    "type": "retrieval_evidence",
                    "document_title": "合同补充",
                    "source_locator": "contracts/a.pdf#p=1",
                    "summary": "合同面积和坪效证据"
                },
                {
                    "type": "dataset_fact_snapshot",
                    "dataset_title": "结构化事实",
                    "summary": "dataset fact snapshot",
                    "snapshot_kind": "entity_rows_by_type",
                    "snapshot_key": "default",
                    "scanned_document_count": 3,
                    "source_fact_count": 9,
                    "row_count_by_type": {"organization": 2}
                }
            ]
        });

        let snapshot = build_static_page_report_snapshot(
            &[],
            Some(&evidence_state),
            &json!({"status": "ready"}),
        );

        assert_eq!(
            snapshot["evidenceNotes"][0]["role"],
            json!("supporting_evidence")
        );
        assert_eq!(
            snapshot["evidenceNotes"][1]["role"],
            json!("structured_fact_snapshot")
        );
        assert_eq!(
            snapshot["evidenceNotes"][1]["rowCountByType"]["organization"],
            json!(2)
        );
    }
}
