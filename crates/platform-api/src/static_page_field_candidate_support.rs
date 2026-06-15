use crate::static_page_artifact_summary_support::static_page_artifact_string;
use crate::static_page_database_aggregate_sample_support::{
    static_page_database_aggregate_field_path,
    static_page_database_aggregate_sample_points_from_item,
};
use crate::static_page_dataset_fact_snapshot_sample_support::{
    static_page_dataset_fact_snapshot_field_path, static_page_dataset_fact_snapshot_rows_by_type,
    static_page_dataset_fact_snapshot_sample_points_from_rows,
    static_page_dataset_fact_snapshot_type_label,
};
use crate::static_page_evidence_signal_support::{
    static_page_evidence_ids, static_page_evidence_ref,
};
use serde_json::{json, Value};
use std::collections::BTreeSet;

pub(crate) fn push_static_page_database_schema_field_candidates(
    candidates: &mut Vec<Value>,
    seen: &mut BTreeSet<String>,
    item: &Value,
    limit: usize,
) {
    let table = static_page_artifact_string(item, &["table"]).unwrap_or_else(|| "database".into());
    let dataset_id = item.get("dataset_id").cloned().unwrap_or(Value::Null);
    let source_id = item.get("source_id").cloned().unwrap_or(Value::Null);
    let mut push_role_group = |group_key: &str,
                               label: &str,
                               kind: &str,
                               confidence: f64,
                               recommended_aggregation: Value| {
        let fields = item
            .get(group_key)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if fields.is_empty() {
            return;
        }
        push_static_page_field_candidate(
            candidates,
            seen,
            json!({
                "sourceId": "database_schema",
                "fieldPath": format!("database.schema.{table}.{group_key}"),
                "label": format!("{label}：{}", fields.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" / ")),
                "kind": kind,
                "recommendedAggregation": recommended_aggregation,
                "confidence": confidence,
                "datasetId": dataset_id.clone(),
                "sourceDatabaseId": source_id.clone(),
                "table": table.clone(),
                "fields": fields,
                "summary": item.get("summary").cloned().unwrap_or(Value::Null),
            }),
            limit,
        );
    };

    push_role_group("metrics", "数据库指标字段", "metric", 0.86, json!("sum"));
    push_role_group(
        "entity_dimensions",
        "数据库实体维度",
        "dimension",
        0.82,
        Value::Null,
    );
    push_role_group(
        "time_dimensions",
        "数据库时间维度",
        "dimension",
        0.82,
        Value::Null,
    );
    push_role_group(
        "category_dimensions",
        "数据库分类维度",
        "dimension",
        0.80,
        Value::Null,
    );
}

pub(crate) fn push_static_page_database_aggregate_field_candidates(
    candidates: &mut Vec<Value>,
    seen: &mut BTreeSet<String>,
    item: &Value,
    limit: usize,
) {
    let rows = item
        .get("rows")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    if rows == 0 {
        return;
    }
    let table = static_page_artifact_string(item, &["table"]).unwrap_or_else(|| "database".into());
    let aggregation =
        static_page_artifact_string(item, &["aggregation"]).unwrap_or_else(|| "sum".into());
    let metric = static_page_artifact_string(item, &["metric", "value_label"])
        .unwrap_or_else(|| "record_count".into());
    let field_path = static_page_database_aggregate_field_path(item);
    let label = format!("数据库聚合：{table} {aggregation}({metric})");
    let aggregate_sample_data =
        static_page_database_aggregate_sample_points_from_item(item, Some(&field_path));

    push_static_page_field_candidate(
        candidates,
        seen,
        json!({
            "sourceId": "database_aggregate",
            "fieldPath": field_path,
            "label": label,
            "kind": "metric",
            "recommendedAggregation": aggregation,
            "confidence": 0.92,
            "datasetId": item.get("dataset_id").cloned().unwrap_or(Value::Null),
            "sourceDatabaseId": item.get("source_id").cloned().unwrap_or(Value::Null),
            "table": table,
            "dimensions": item.get("dimensions").cloned().unwrap_or_else(|| json!([])),
            "metric": metric,
            "unit": item.get("unit").cloned().unwrap_or(Value::Null),
            "columns": item.get("columns").cloned().unwrap_or_else(|| json!([])),
            "sampleRows": rows,
            "sampleData": aggregate_sample_data,
            "scanLimit": item.get("scan_limit").cloned().unwrap_or(Value::Null),
            "rowLimit": item.get("row_limit").cloned().unwrap_or(Value::Null),
        }),
        limit,
    );

    let dataset_sample_data = static_page_database_aggregate_sample_points_from_item(
        item,
        Some("dataset.metrics_summary"),
    );
    push_static_page_field_candidate(
        candidates,
        seen,
        json!({
            "sourceId": "dataset",
            "fieldPath": "dataset.metrics_summary",
            "label": format!("数据集聚合指标：{metric}"),
            "kind": "metric",
            "recommendedAggregation": aggregation,
            "confidence": 0.88,
            "datasetId": item.get("dataset_id").cloned().unwrap_or(Value::Null),
            "sourceIdAlias": "database_aggregate",
            "table": table,
            "dimensions": item.get("dimensions").cloned().unwrap_or_else(|| json!([])),
            "metric": metric,
            "unit": item.get("unit").cloned().unwrap_or(Value::Null),
            "sampleRows": rows,
            "sampleData": dataset_sample_data,
            "scanLimit": item.get("scan_limit").cloned().unwrap_or(Value::Null),
        }),
        limit,
    );
}

pub(crate) fn push_static_page_dataset_fact_snapshot_field_candidates(
    candidates: &mut Vec<Value>,
    seen: &mut BTreeSet<String>,
    item: &Value,
    limit: usize,
) {
    let Some(rows_by_type) = static_page_dataset_fact_snapshot_rows_by_type(item) else {
        return;
    };

    for (fact_type, rows) in rows_by_type {
        let Some(rows) = rows.as_array() else {
            continue;
        };
        if rows.is_empty() {
            continue;
        }
        let field_path = static_page_dataset_fact_snapshot_field_path(fact_type);
        let sample_data = static_page_dataset_fact_snapshot_sample_points_from_rows(
            item,
            fact_type,
            rows,
            Some(&field_path),
        );
        if sample_data.is_empty() {
            continue;
        }
        push_static_page_field_candidate(
            candidates,
            seen,
            json!({
                "sourceId": "dataset_fact_snapshot",
                "fieldPath": field_path,
                "label": format!(
                    "文档结构化事实：{}",
                    static_page_dataset_fact_snapshot_type_label(fact_type)
                ),
                "kind": "document_fact_table",
                "recommendedAggregation": "count",
                "confidence": 0.88,
                "datasetId": item.get("dataset_id").cloned().unwrap_or(Value::Null),
                "datasetKey": item.get("dataset_key").cloned().unwrap_or(Value::Null),
                "snapshotKind": item.get("snapshot_kind").cloned().unwrap_or(Value::Null),
                "snapshotKey": item.get("snapshot_key").cloned().unwrap_or(Value::Null),
                "source": item.get("source").cloned().unwrap_or_else(|| json!("dataset_fact_snapshots")),
                "factType": fact_type,
                "metric": "fact_count",
                "sampleRows": rows.len(),
                "sampleData": sample_data,
                "sourceFactCount": item.get("source_fact_count").cloned().unwrap_or(Value::Null),
                "sourceDocumentCount": item.get("source_document_count").cloned().unwrap_or(Value::Null),
                "scannedDocumentCount": item.get("scanned_document_count").cloned().unwrap_or(Value::Null),
            }),
            limit,
        );
    }
}

pub(crate) fn push_static_page_media_field_candidates(
    candidates: &mut Vec<Value>,
    seen: &mut BTreeSet<String>,
    media_context: &Value,
    item: &Value,
    limit: usize,
) {
    for (key, field_path, label, confidence) in [
        (
            "transcript_windows",
            "media.transcript_windows",
            "媒体转写时间窗",
            0.82,
        ),
        (
            "scene_windows",
            "media.scene_windows",
            "视频场景时间窗",
            0.78,
        ),
        (
            "keyframe_ocr_snippets",
            "media.keyframe_ocr_snippets",
            "关键帧 OCR 片段",
            0.76,
        ),
    ] {
        let has_items = media_context
            .get(key)
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty());
        if !has_items {
            continue;
        }
        push_static_page_field_candidate(
            candidates,
            seen,
            json!({
                "sourceId": "evidence",
                "fieldPath": field_path,
                "label": label,
                "kind": "media",
                "recommendedAggregation": Value::Null,
                "confidence": confidence,
                "evidenceIds": static_page_evidence_ids(item),
                "evidenceRef": static_page_evidence_ref(item),
                "mediaKind": media_context
                    .get("media_kind")
                    .cloned()
                    .unwrap_or_else(|| json!("unknown")),
                "timestamped": media_context
                    .get("has_timestamped_evidence")
                    .cloned()
                    .unwrap_or_else(|| json!(false)),
            }),
            limit,
        );
    }
}

pub(crate) fn push_static_page_field_candidate(
    candidates: &mut Vec<Value>,
    seen: &mut BTreeSet<String>,
    candidate: Value,
    limit: usize,
) {
    if candidates.len() >= limit {
        return;
    }
    let key = format!(
        "{}:{}",
        candidate
            .get("sourceId")
            .and_then(Value::as_str)
            .unwrap_or("unknown"),
        candidate
            .get("fieldPath")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    );
    if key.ends_with(':') || !seen.insert(key) {
        return;
    }
    candidates.push(candidate);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn field_candidate_push_dedupes_and_respects_limit() {
        let mut candidates = Vec::new();
        let mut seen = BTreeSet::new();

        push_static_page_field_candidate(
            &mut candidates,
            &mut seen,
            json!({"sourceId": "dataset", "fieldPath": "dataset.metrics_summary"}),
            2,
        );
        push_static_page_field_candidate(
            &mut candidates,
            &mut seen,
            json!({"sourceId": "dataset", "fieldPath": "dataset.metrics_summary"}),
            2,
        );
        push_static_page_field_candidate(
            &mut candidates,
            &mut seen,
            json!({"sourceId": "dataset", "fieldPath": ""}),
            2,
        );
        push_static_page_field_candidate(
            &mut candidates,
            &mut seen,
            json!({"sourceId": "evidence", "fieldPath": "retrieval.summary"}),
            2,
        );
        push_static_page_field_candidate(
            &mut candidates,
            &mut seen,
            json!({"sourceId": "evidence", "fieldPath": "retrieval.content_excerpt"}),
            2,
        );

        assert_eq!(candidates.len(), 2);
        assert_eq!(
            candidates[0].get("fieldPath"),
            Some(&json!("dataset.metrics_summary"))
        );
        assert_eq!(
            candidates[1].get("fieldPath"),
            Some(&json!("retrieval.summary"))
        );
    }

    #[test]
    fn schema_field_candidates_keep_role_groups_and_metadata() {
        let item = json!({
            "dataset_id": "dataset-1",
            "source_id": "db-1",
            "table": "bi_sales",
            "metrics": ["sales", "rent"],
            "entity_dimensions": ["store_name"],
            "time_dimensions": ["txdate"],
            "summary": "经营数据"
        });
        let mut candidates = Vec::new();
        let mut seen = BTreeSet::new();

        push_static_page_database_schema_field_candidates(&mut candidates, &mut seen, &item, 24);

        assert_eq!(candidates.len(), 3);
        assert_eq!(
            candidates[0].get("fieldPath"),
            Some(&json!("database.schema.bi_sales.metrics"))
        );
        assert_eq!(
            candidates[0].get("label"),
            Some(&json!("数据库指标字段：sales / rent"))
        );
        assert_eq!(
            candidates[0].get("recommendedAggregation"),
            Some(&json!("sum"))
        );
        assert_eq!(candidates[1].get("kind"), Some(&json!("dimension")));
        assert_eq!(candidates[2].get("sourceDatabaseId"), Some(&json!("db-1")));
    }

    #[test]
    fn aggregate_field_candidates_add_database_and_dataset_samples() {
        let item = json!({
            "dataset_id": "dataset-1",
            "source_id": "db-1",
            "table": "bi_contract_warning",
            "aggregation": "sum",
            "metric": "sales_amount",
            "unit": "元",
            "dimensions": ["store_name"],
            "rows": [
                {"store_name": "A店", "sales_amount": "1,234.5"}
            ]
        });
        let mut candidates = Vec::new();
        let mut seen = BTreeSet::new();

        push_static_page_database_aggregate_field_candidates(&mut candidates, &mut seen, &item, 24);

        assert_eq!(candidates.len(), 2);
        assert_eq!(
            candidates[0].get("fieldPath"),
            Some(&json!("database.aggregate.sales_amount"))
        );
        assert_eq!(candidates[0].get("sampleRows"), Some(&json!(1)));
        assert_eq!(
            candidates[0].pointer("/sampleData/0/label"),
            Some(&json!("A店"))
        );
        assert_eq!(
            candidates[1].get("fieldPath"),
            Some(&json!("dataset.metrics_summary"))
        );
        assert_eq!(
            candidates[1].get("sourceIdAlias"),
            Some(&json!("database_aggregate"))
        );
    }

    #[test]
    fn fact_snapshot_field_candidates_add_document_fact_table_samples() {
        let item = json!({
            "dataset_id": "dataset-1",
            "dataset_key": "xinbai",
            "entity_rows_by_type": {
                "organization": [
                    {"name": "品牌A", "fact_count": 3}
                ]
            },
            "source_fact_count": 3,
            "source_document_count": 1
        });
        let mut candidates = Vec::new();
        let mut seen = BTreeSet::new();

        push_static_page_dataset_fact_snapshot_field_candidates(
            &mut candidates,
            &mut seen,
            &item,
            24,
        );

        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].get("fieldPath"),
            Some(&json!("dataset.fact_snapshot.organization"))
        );
        assert_eq!(
            candidates[0].get("label"),
            Some(&json!("文档结构化事实：组织/公司"))
        );
        assert_eq!(
            candidates[0].pointer("/sampleData/0/label"),
            Some(&json!("品牌A"))
        );
        assert_eq!(candidates[0].get("sourceFactCount"), Some(&json!(3)));
    }

    #[test]
    fn media_field_candidates_keep_timestamp_metadata() {
        let item = json!({
            "retrieval_evidence_id": "ev-1",
            "dataset_id": "dataset-1",
            "source_locator": "docs/call.mp4#t=1"
        });
        let media_context = json!({
            "media_kind": "video",
            "has_timestamped_evidence": true,
            "transcript_windows": [{"text": "门店收入"}],
            "scene_windows": [],
            "keyframe_ocr_snippets": [{"text": "销售额"}]
        });
        let mut candidates = Vec::new();
        let mut seen = BTreeSet::new();

        push_static_page_media_field_candidates(
            &mut candidates,
            &mut seen,
            &media_context,
            &item,
            24,
        );

        assert_eq!(candidates.len(), 2);
        assert_eq!(
            candidates[0].get("fieldPath"),
            Some(&json!("media.transcript_windows"))
        );
        assert_eq!(candidates[0].get("mediaKind"), Some(&json!("video")));
        assert_eq!(candidates[0].get("timestamped"), Some(&json!(true)));
        assert_eq!(
            candidates[0].pointer("/evidenceRef/sourceLocator"),
            Some(&json!("docs/call.mp4#t=1"))
        );
        assert_eq!(
            candidates[1].get("fieldPath"),
            Some(&json!("media.keyframe_ocr_snippets"))
        );
    }
}
