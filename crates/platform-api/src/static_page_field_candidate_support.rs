use crate::assistant_run_scope_selection_support::selected_dataset_ids_from_scope;
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
    static_page_evidence_ids, static_page_evidence_ref, static_page_evidence_section_title_hints,
    static_page_evidence_text, static_page_text_contains_any,
};
use serde_json::{json, Value};
use std::collections::BTreeSet;

pub(crate) fn build_static_page_field_candidates(
    selected_scope: &Value,
    evidence_state: Option<&Value>,
) -> Value {
    const FIELD_CANDIDATE_LIMIT: usize = 24;

    let mut candidates = Vec::new();
    let mut seen = BTreeSet::new();
    let selected_dataset_ids = selected_dataset_ids_from_scope(selected_scope);
    if !selected_dataset_ids.is_empty() {
        push_static_page_field_candidate(
            &mut candidates,
            &mut seen,
            json!({
                "sourceId": "dataset",
                "fieldPath": "dataset.metrics_summary",
                "label": "数据集指标摘要",
                "kind": "summary",
                "recommendedAggregation": Value::Null,
                "confidence": 0.55,
                "datasetIds": selected_dataset_ids.iter().map(ToString::to_string).collect::<Vec<_>>(),
            }),
            FIELD_CANDIDATE_LIMIT,
        );
    }

    let Some(evidence_items) = evidence_state
        .and_then(|state| state.get("supplied_items"))
        .and_then(Value::as_array)
    else {
        return Value::Array(candidates);
    };

    for item in evidence_items {
        match item.get("type").and_then(Value::as_str).unwrap_or_default() {
            "database_schema_context" => {
                push_static_page_database_schema_field_candidates(
                    &mut candidates,
                    &mut seen,
                    item,
                    FIELD_CANDIDATE_LIMIT,
                );
            }
            "database_aggregate" => {
                push_static_page_database_aggregate_field_candidates(
                    &mut candidates,
                    &mut seen,
                    item,
                    FIELD_CANDIDATE_LIMIT,
                );
            }
            "dataset_fact_snapshot" => {
                push_static_page_dataset_fact_snapshot_field_candidates(
                    &mut candidates,
                    &mut seen,
                    item,
                    FIELD_CANDIDATE_LIMIT,
                );
            }
            "retrieval_evidence" => {
                let evidence_ref = static_page_evidence_ref(item);
                let evidence_ids = static_page_evidence_ids(item);
                push_static_page_field_candidate(
                    &mut candidates,
                    &mut seen,
                    json!({
                        "sourceId": "evidence",
                        "fieldPath": "retrieval.summary",
                        "label": "证据摘要",
                        "kind": "text",
                        "recommendedAggregation": Value::Null,
                        "confidence": 0.72,
                        "evidenceIds": evidence_ids,
                        "evidenceRef": evidence_ref,
                    }),
                    FIELD_CANDIDATE_LIMIT,
                );
                push_static_page_field_candidate(
                    &mut candidates,
                    &mut seen,
                    json!({
                        "sourceId": "evidence",
                        "fieldPath": "retrieval.content_excerpt",
                        "label": "证据原文片段",
                        "kind": "text",
                        "recommendedAggregation": Value::Null,
                        "confidence": 0.70,
                        "evidenceIds": static_page_evidence_ids(item),
                        "evidenceRef": static_page_evidence_ref(item),
                    }),
                    FIELD_CANDIDATE_LIMIT,
                );
                let section_title_hints = static_page_evidence_section_title_hints(item);
                if !section_title_hints.is_empty() {
                    push_static_page_field_candidate(
                        &mut candidates,
                        &mut seen,
                        json!({
                            "sourceId": "evidence",
                            "fieldPath": "retrieval.section_title_hints",
                            "label": format!("段落标题：{}", section_title_hints.join(" / ")),
                            "kind": "section_title",
                            "recommendedAggregation": Value::Null,
                            "confidence": 0.74,
                            "evidenceIds": static_page_evidence_ids(item),
                            "evidenceRef": static_page_evidence_ref(item),
                            "sectionTitleHints": section_title_hints,
                        }),
                        FIELD_CANDIDATE_LIMIT,
                    );
                }

                if let Some(media_context) =
                    item.get("media_context").filter(|value| value.is_object())
                {
                    push_static_page_media_field_candidates(
                        &mut candidates,
                        &mut seen,
                        media_context,
                        item,
                        FIELD_CANDIDATE_LIMIT,
                    );
                }

                let evidence_text = static_page_evidence_text(item).to_lowercase();
                for (field_path, label, kind, aggregation, keywords, confidence) in [
                    (
                        "orders.amount",
                        "订单金额/收入",
                        "metric",
                        Some("sum"),
                        &[
                            "order", "orders", "amount", "revenue", "sales", "gmv", "订单", "金额",
                            "收入",
                        ][..],
                        0.84,
                    ),
                    (
                        "orders.count",
                        "订单数量",
                        "metric",
                        Some("count"),
                        &["order", "orders", "count", "volume", "订单", "数量", "单量"][..],
                        0.78,
                    ),
                    (
                        "customers.count",
                        "客户数量",
                        "metric",
                        Some("count"),
                        &["customer", "customers", "client", "客户", "用户"][..],
                        0.76,
                    ),
                    (
                        "store.area",
                        "门店/合同面积",
                        "metric",
                        Some("sum"),
                        &[
                            "area",
                            "store_area",
                            "contract_area",
                            "leased_area",
                            "business_area",
                            "合同面积",
                            "租赁面积",
                            "建筑面积",
                            "经营面积",
                            "门店面积",
                            "铺位面积",
                            "面积",
                            "坪效",
                        ][..],
                        0.82,
                    ),
                    (
                        "traffic.count",
                        "客流/人流统计",
                        "metric",
                        Some("sum"),
                        &[
                            "traffic",
                            "visitor",
                            "visitors",
                            "customer_flow",
                            "footfall",
                            "passenger",
                            "客流",
                            "客流量",
                            "人流",
                            "人流量",
                            "客数",
                            "进店",
                            "到店",
                        ][..],
                        0.82,
                    ),
                    (
                        "profit.margin",
                        "利润/毛利",
                        "metric",
                        Some("sum"),
                        &["profit", "margin", "gross", "利润", "毛利"][..],
                        0.76,
                    ),
                    (
                        "risk.level",
                        "风险等级",
                        "dimension",
                        None,
                        &["risk", "delay", "warning", "风险", "延期", "预警"][..],
                        0.80,
                    ),
                    (
                        "time.month",
                        "月份/时间",
                        "dimension",
                        None,
                        &[
                            "month", "date", "time", "period", "月份", "日期", "时间", "周期",
                        ][..],
                        0.72,
                    ),
                    (
                        "engagement.rate",
                        "触达/互动",
                        "metric",
                        Some("avg"),
                        &[
                            "engagement",
                            "newsletter",
                            "open",
                            "click",
                            "触达",
                            "互动",
                            "打开",
                            "点击",
                        ][..],
                        0.70,
                    ),
                ] {
                    if !static_page_text_contains_any(&evidence_text, keywords) {
                        continue;
                    }
                    push_static_page_field_candidate(
                        &mut candidates,
                        &mut seen,
                        json!({
                            "sourceId": "evidence",
                            "fieldPath": field_path,
                            "label": label,
                            "kind": kind,
                            "recommendedAggregation": aggregation,
                            "confidence": confidence,
                            "evidenceIds": static_page_evidence_ids(item),
                            "evidenceRef": static_page_evidence_ref(item),
                        }),
                        FIELD_CANDIDATE_LIMIT,
                    );
                }
            }
            "conversation_memory_item" => {
                push_static_page_field_candidate(
                    &mut candidates,
                    &mut seen,
                    json!({
                        "sourceId": "conversation_memory",
                        "fieldPath": "conversation.summary",
                        "label": "相关历史对话摘要",
                        "kind": "text",
                        "recommendedAggregation": Value::Null,
                        "confidence": 0.66,
                        "conversationMemoryItemId": item
                            .get("conversation_memory_item_id")
                            .cloned()
                            .unwrap_or(Value::Null),
                    }),
                    FIELD_CANDIDATE_LIMIT,
                );
            }
            _ => {}
        }
    }

    Value::Array(candidates)
}

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
    use domain_model::DatasetId;
    use serde_json::json;

    #[test]
    fn field_candidates_build_dataset_retrieval_and_keyword_candidates() {
        let dataset_id = DatasetId::new();
        let selected_scope = json!({
            "datasets": [dataset_id.to_string()]
        });
        let evidence_state = json!({
            "supplied_items": [
                {
                    "type": "retrieval_evidence",
                    "retrieval_evidence_id": "ev-1",
                    "dataset_id": dataset_id.to_string(),
                    "source_locator": "docs/report.md#chunk=1",
                    "summary": "门店经营风险与订单收入摘要",
                    "content_excerpt": "订单金额 revenue 增长，但风险 warning 和门店面积 area 需要关注。",
                    "section_title_hints": ["经营风险", "订单收入"]
                }
            ]
        });

        let candidates = build_static_page_field_candidates(&selected_scope, Some(&evidence_state));
        let candidates = candidates
            .as_array()
            .expect("field candidates should be an array");

        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.get("fieldPath")
                    == Some(&json!("dataset.metrics_summary")))
        );
        assert!(candidates
            .iter()
            .any(|candidate| candidate.get("fieldPath") == Some(&json!("retrieval.summary"))));
        assert!(candidates
            .iter()
            .any(|candidate| candidate.get("fieldPath") == Some(&json!("orders.amount"))));
        assert!(candidates
            .iter()
            .any(|candidate| candidate.get("fieldPath") == Some(&json!("risk.level"))));
        assert!(candidates.iter().any(|candidate| {
            candidate.get("fieldPath") == Some(&json!("retrieval.section_title_hints"))
        }));
    }

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
