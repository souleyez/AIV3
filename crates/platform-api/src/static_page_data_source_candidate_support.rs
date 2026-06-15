use crate::assistant_run_conversation_memory_support::selected_scope_requests_conversation_memory;
use crate::assistant_run_scope_selection_support::selected_dataset_ids_from_scope;
use crate::static_page_dataset_fact_snapshot_sample_support::static_page_dataset_fact_snapshot_rows_by_type;
use serde_json::{json, Value};
use std::collections::BTreeSet;

pub(crate) fn build_static_page_data_source_candidates(
    selected_scope: &Value,
    evidence_state: Option<&Value>,
) -> Value {
    let mut candidates = vec![
        json!({
            "sourceId": "model",
            "type": "model_summary",
            "label": "模型总结",
            "available": true,
        }),
        json!({
            "sourceId": "conversation_memory",
            "type": "conversation_memory",
            "label": "对话历史",
            "available": selected_scope_requests_conversation_memory(selected_scope),
        }),
    ];

    let selected_dataset_ids = selected_dataset_ids_from_scope(selected_scope);
    if !selected_dataset_ids.is_empty() {
        candidates.push(json!({
            "sourceId": "selected_scope",
            "type": "selected_scope",
            "label": "当前选中范围",
            "available": true,
            "datasetIds": selected_dataset_ids.iter().map(ToString::to_string).collect::<Vec<_>>(),
        }));
        candidates.push(json!({
            "sourceId": "dataset",
            "type": "dataset_metrics",
            "label": "数据集指标摘要",
            "available": true,
            "datasetIds": selected_dataset_ids.iter().map(ToString::to_string).collect::<Vec<_>>(),
        }));
        candidates.push(json!({
            "sourceId": "evidence",
            "type": "retrieval_evidence",
            "label": "检索证据",
            "available": true,
            "datasetIds": selected_dataset_ids.iter().map(ToString::to_string).collect::<Vec<_>>(),
        }));
    }

    push_static_page_database_data_source_candidates(&mut candidates, evidence_state);

    Value::Array(candidates)
}

pub(crate) fn push_static_page_database_data_source_candidates(
    candidates: &mut Vec<Value>,
    evidence_state: Option<&Value>,
) {
    let Some(evidence_items) = evidence_state
        .and_then(|state| state.get("supplied_items"))
        .and_then(Value::as_array)
    else {
        return;
    };

    let mut schema_dataset_ids = BTreeSet::new();
    let mut schema_source_ids = BTreeSet::new();
    let mut schema_tables = BTreeSet::new();
    let mut aggregate_dataset_ids = BTreeSet::new();
    let mut aggregate_source_ids = BTreeSet::new();
    let mut aggregate_tables = BTreeSet::new();
    let mut aggregate_fields = BTreeSet::new();
    let mut fact_snapshot_dataset_ids = BTreeSet::new();
    let mut fact_snapshot_sources = BTreeSet::new();
    let mut fact_snapshot_types = BTreeSet::new();

    for item in evidence_items {
        match item.get("type").and_then(Value::as_str).unwrap_or_default() {
            "database_schema_context" => {
                collect_static_page_candidate_string(item, "dataset_id", &mut schema_dataset_ids);
                collect_static_page_candidate_string(item, "source_id", &mut schema_source_ids);
                collect_static_page_candidate_string(item, "table", &mut schema_tables);
            }
            "database_aggregate" => {
                collect_static_page_candidate_string(
                    item,
                    "dataset_id",
                    &mut aggregate_dataset_ids,
                );
                collect_static_page_candidate_string(item, "source_id", &mut aggregate_source_ids);
                collect_static_page_candidate_string(item, "table", &mut aggregate_tables);
                collect_static_page_candidate_string(item, "value_label", &mut aggregate_fields);
                collect_static_page_candidate_string(item, "metric", &mut aggregate_fields);
            }
            "dataset_fact_snapshot" => {
                collect_static_page_candidate_string(
                    item,
                    "dataset_id",
                    &mut fact_snapshot_dataset_ids,
                );
                collect_static_page_candidate_string(item, "source", &mut fact_snapshot_sources);
                if let Some(rows_by_type) = static_page_dataset_fact_snapshot_rows_by_type(item) {
                    for (fact_type, rows) in rows_by_type {
                        if rows.as_array().is_some_and(|items| !items.is_empty()) {
                            fact_snapshot_types.insert(fact_type.clone());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    if !schema_tables.is_empty() {
        candidates.push(json!({
            "sourceId": "database_schema",
            "type": "database_schema_context",
            "label": "数据库结构语义",
            "available": true,
            "datasetIds": schema_dataset_ids.into_iter().collect::<Vec<_>>(),
            "sourceDatabaseIds": schema_source_ids.into_iter().collect::<Vec<_>>(),
            "tables": schema_tables.into_iter().collect::<Vec<_>>(),
        }));
    }
    if !aggregate_tables.is_empty() {
        candidates.push(json!({
            "sourceId": "database_aggregate",
            "type": "database_aggregate",
            "label": "数据库聚合样本",
            "available": true,
            "datasetIds": aggregate_dataset_ids.into_iter().collect::<Vec<_>>(),
            "sourceDatabaseIds": aggregate_source_ids.into_iter().collect::<Vec<_>>(),
            "tables": aggregate_tables.into_iter().collect::<Vec<_>>(),
            "fields": aggregate_fields.into_iter().collect::<Vec<_>>(),
        }));
    }
    if !fact_snapshot_types.is_empty() {
        candidates.push(json!({
            "sourceId": "dataset_fact_snapshot",
            "type": "dataset_fact_snapshot",
            "label": "文档结构化事实快照",
            "available": true,
            "datasetIds": fact_snapshot_dataset_ids.into_iter().collect::<Vec<_>>(),
            "sources": fact_snapshot_sources.into_iter().collect::<Vec<_>>(),
            "factTypes": fact_snapshot_types.into_iter().collect::<Vec<_>>(),
        }));
    }
}

fn collect_static_page_candidate_string(item: &Value, key: &str, output: &mut BTreeSet<String>) {
    if let Some(value) = item.get(key).and_then(Value::as_str) {
        let value = value.trim();
        if !value.is_empty() {
            output.insert(value.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain_model::DatasetId;
    use serde_json::json;

    #[test]
    fn data_source_candidates_include_base_and_selected_dataset_sources() {
        let dataset_id = DatasetId::new();
        let selected_scope = json!({
            "datasets": [
                dataset_id.to_string(),
                dataset_id.to_string()
            ],
            "conversation_memory": ["current_thread"]
        });

        let candidates = build_static_page_data_source_candidates(&selected_scope, None);
        let items = candidates.as_array().expect("candidates");

        assert_eq!(items[0].get("sourceId"), Some(&json!("model")));
        assert_eq!(
            items[1].get("sourceId"),
            Some(&json!("conversation_memory"))
        );
        assert_eq!(items[1].get("available"), Some(&json!(true)));
        assert!(items.iter().any(|candidate| {
            candidate.get("sourceId") == Some(&json!("selected_scope"))
                && candidate.get("datasetIds") == Some(&json!([dataset_id.to_string()]))
        }));
        assert!(items.iter().any(|candidate| {
            candidate.get("sourceId") == Some(&json!("dataset"))
                && candidate.get("type") == Some(&json!("dataset_metrics"))
        }));
        assert!(items.iter().any(|candidate| {
            candidate.get("sourceId") == Some(&json!("evidence"))
                && candidate.get("type") == Some(&json!("retrieval_evidence"))
        }));
    }

    #[test]
    fn data_source_candidates_default_conversation_memory_unavailable_without_scope_request() {
        let candidates = build_static_page_data_source_candidates(&json!({}), None);
        let items = candidates.as_array().expect("candidates");

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].get("sourceId"), Some(&json!("model")));
        assert_eq!(
            items[1].get("sourceId"),
            Some(&json!("conversation_memory"))
        );
        assert_eq!(items[1].get("available"), Some(&json!(false)));
    }

    #[test]
    fn database_data_source_candidates_collect_schema_aggregate_and_fact_snapshot() {
        let mut candidates = Vec::new();
        let evidence_state = json!({
            "supplied_items": [
                {
                    "type": "database_schema_context",
                    "dataset_id": "dataset-b",
                    "source_id": "db-2",
                    "table": " bi_store "
                },
                {
                    "type": "database_schema_context",
                    "dataset_id": "dataset-a",
                    "source_id": "db-1",
                    "table": "bi_sales"
                },
                {
                    "type": "database_schema_context",
                    "dataset_id": "",
                    "source_id": "db-1",
                    "table": "bi_sales"
                },
                {
                    "type": "database_aggregate",
                    "dataset_id": "dataset-a",
                    "source_id": "db-1",
                    "table": "bi_sales",
                    "metric": "sales",
                    "value_label": "amount"
                },
                {
                    "type": "database_aggregate",
                    "dataset_id": "dataset-a",
                    "source_id": "db-1",
                    "table": "bi_sales",
                    "metric": "sales",
                    "value_label": "amount"
                },
                {
                    "type": "dataset_fact_snapshot",
                    "dataset_id": "dataset-a",
                    "source": "dataset_fact_snapshots",
                    "entity_rows_by_type": {
                        "organization": [{"name": "品牌A"}],
                        "person": []
                    }
                }
            ]
        });

        push_static_page_database_data_source_candidates(&mut candidates, Some(&evidence_state));

        assert_eq!(candidates.len(), 3);
        assert_eq!(
            candidates[0].get("sourceId"),
            Some(&json!("database_schema"))
        );
        assert_eq!(
            candidates[0].get("datasetIds"),
            Some(&json!(["dataset-a", "dataset-b"]))
        );
        assert_eq!(
            candidates[0].get("sourceDatabaseIds"),
            Some(&json!(["db-1", "db-2"]))
        );
        assert_eq!(
            candidates[0].get("tables"),
            Some(&json!(["bi_sales", "bi_store"]))
        );

        assert_eq!(
            candidates[1].get("sourceId"),
            Some(&json!("database_aggregate"))
        );
        assert_eq!(candidates[1].get("tables"), Some(&json!(["bi_sales"])));
        assert_eq!(
            candidates[1].get("fields"),
            Some(&json!(["amount", "sales"]))
        );

        assert_eq!(
            candidates[2].get("sourceId"),
            Some(&json!("dataset_fact_snapshot"))
        );
        assert_eq!(
            candidates[2].get("factTypes"),
            Some(&json!(["organization"]))
        );
    }

    #[test]
    fn database_data_source_candidates_skip_empty_evidence() {
        let mut candidates = vec![json!({"sourceId": "model"})];

        push_static_page_database_data_source_candidates(
            &mut candidates,
            Some(&json!({"supplied_items": [
                {"type": "database_schema_context", "table": ""},
                {"type": "database_aggregate", "rows": []},
                {"type": "dataset_fact_snapshot", "entity_rows_by_type": {"person": []}}
            ]})),
        );

        assert_eq!(candidates, vec![json!({"sourceId": "model"})]);
    }
}
