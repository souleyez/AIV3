use domain_model::Dataset;
use serde_json::{json, Value};

pub(crate) fn assistant_run_fact_index_enabled() -> bool {
    std::env::var("ASSISTANT_RUN_FACT_INDEX_ENABLED")
        .ok()
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            !matches!(normalized.as_str(), "" | "0" | "false" | "off" | "no")
        })
        .unwrap_or(true)
}

pub(crate) fn assistant_run_dataset_fact_snapshot_row_summary(row_count_by_type: &Value) -> String {
    row_count_by_type
        .as_object()
        .map(|object| {
            object
                .iter()
                .filter_map(|(fact_type, count)| {
                    let count = count.as_u64()?;
                    (count > 0).then(|| format!("{fact_type}={count}"))
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default()
}

pub(crate) fn assistant_run_dataset_fact_snapshot_item(
    dataset: &Dataset,
    snapshot: &storage::DatasetFactSnapshot,
) -> Value {
    assistant_run_dataset_fact_snapshot_item_from_manifest(
        dataset,
        "dataset_fact_snapshots",
        &snapshot.snapshot_kind,
        &snapshot.snapshot_key,
        snapshot.snapshot_manifest.clone(),
        snapshot.source_fact_count,
        snapshot.source_document_count,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn assistant_run_dataset_fact_snapshot_item_from_manifest(
    dataset: &Dataset,
    source: &str,
    snapshot_kind: &str,
    snapshot_key: &str,
    manifest: Value,
    source_fact_count: i64,
    source_document_count: i64,
    scope_filter: Option<Value>,
) -> Value {
    let mut manifest = manifest;
    if let Some(scope_filter) = scope_filter.as_ref() {
        if let Some(object) = manifest.as_object_mut() {
            object.insert("scope_filter".to_string(), scope_filter.clone());
        }
    }
    let entity_rows_by_type = manifest
        .get("entity_rows_by_type")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let row_count_by_type = manifest
        .get("row_count_by_type")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let scanned_document_count = manifest
        .get("scanned_document_count")
        .and_then(Value::as_i64)
        .unwrap_or(source_document_count);
    let row_summary = assistant_run_dataset_fact_snapshot_row_summary(&row_count_by_type);
    let summary = if row_summary.is_empty() {
        format!(
            "dataset fact snapshot: {scanned_document_count} documents, {} facts",
            source_fact_count
        )
    } else {
        format!(
            "dataset fact snapshot: {scanned_document_count} documents, {} facts, rows {row_summary}",
            source_fact_count
        )
    };
    let model_note = if scope_filter.is_some() {
        "Use this as the authoritative aggregate for the selected or authorized document scope. Cite scanned_document_count, source_document_count, source_fact_count, and row_count_by_type when giving totals. Use retrieval evidence only for examples, quotes, and validation; never infer totals from retrieval chunks."
    } else {
        "Use this as the authoritative dataset-level aggregate for count/list/rank questions. Cite scanned_document_count, source_document_count, source_fact_count, and row_count_by_type when giving totals. Use retrieval evidence only for examples, quotes, and validation; never infer totals from retrieval chunks."
    };

    json!({
        "type": "dataset_fact_snapshot",
        "source": source,
        "dataset_id": dataset.id,
        "dataset_key": dataset.key.clone(),
        "dataset_title": dataset.title.clone(),
        "snapshot_kind": snapshot_kind,
        "snapshot_key": snapshot_key,
        "scanned_document_count": scanned_document_count,
        "source_document_count": source_document_count,
        "source_fact_count": source_fact_count,
        "row_count_by_type": row_count_by_type,
        "entity_rows_by_type": entity_rows_by_type,
        "snapshot_manifest": manifest,
        "summary": summary,
        "model_note": model_note,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use chrono::Utc;
    use domain_model::{DatasetId, DatasetLifecycle, DatasetVisibility, TenantId};
    use serde_json::json;

    use super::*;

    #[test]
    fn dataset_fact_snapshot_row_summary_skips_empty_and_non_numeric_rows() {
        let summary = assistant_run_dataset_fact_snapshot_row_summary(&json!({
            "company_rows": 3,
            "keyword_rows": 0,
            "table_rows": "2"
        }));

        assert_eq!(summary, "company_rows=3");
    }

    #[test]
    fn dataset_fact_snapshot_item_from_manifest_preserves_scope_and_summary() {
        let now = Utc::now();
        let dataset = Dataset {
            id: DatasetId::new(),
            tenant_id: TenantId::new(),
            owner_user_id: None,
            key: "dataset-key".to_string(),
            title: "Dataset Title".to_string(),
            description: None,
            lifecycle: DatasetLifecycle::Active,
            visibility: DatasetVisibility::Private,
            default_secret_binding_ids: Vec::new(),
            metadata: BTreeMap::new(),
            created_at: now,
            updated_at: now,
        };
        let scope_filter = json!({"mode": "selected_or_authorized_documents"});
        let item = assistant_run_dataset_fact_snapshot_item_from_manifest(
            &dataset,
            "document_facts_scoped_aggregate",
            "entity_rows_by_type",
            "default",
            json!({
                "scanned_document_count": 2,
                "row_count_by_type": {"organization": 1},
                "entity_rows_by_type": {
                    "organization": [{"name": "广州冠晚网络有限公司"}]
                }
            }),
            3,
            2,
            Some(scope_filter.clone()),
        );

        assert_eq!(item["type"], json!("dataset_fact_snapshot"));
        assert_eq!(item["dataset_key"], json!("dataset-key"));
        assert_eq!(item["snapshot_manifest"]["scope_filter"], scope_filter);
        assert_eq!(item["row_count_by_type"]["organization"], json!(1));
        assert!(item["summary"]
            .as_str()
            .unwrap_or_default()
            .contains("rows organization=1"));
        assert!(item["model_note"]
            .as_str()
            .unwrap_or_default()
            .contains("selected or authorized document scope"));
    }
}
