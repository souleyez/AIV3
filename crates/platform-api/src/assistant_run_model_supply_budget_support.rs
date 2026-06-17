use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    assistant_run_model_supply_item_for_context, ASSISTANT_RUN_MODEL_CONTEXT_ASSET_PROFILE_LIMIT,
    ASSISTANT_RUN_MODEL_CONTEXT_DATABASE_LIMIT, ASSISTANT_RUN_MODEL_CONTEXT_MEMORY_LIMIT,
    ASSISTANT_RUN_MODEL_CONTEXT_OTHER_LIMIT, ASSISTANT_RUN_MODEL_CONTEXT_PARSE_STATUS_LIMIT,
    ASSISTANT_RUN_MODEL_CONTEXT_RETRIEVAL_LIMIT, ASSISTANT_RUN_MODEL_CONTEXT_SPREADSHEET_LIMIT,
    ASSISTANT_RUN_MODEL_CONTEXT_STRUCTURED_FACT_LIMIT,
    ASSISTANT_RUN_MODEL_CONTEXT_SUPPLIED_ITEM_LIMIT,
};

pub(crate) fn assistant_run_model_budgeted_supply_items(items: &[Value]) -> (Vec<Value>, Value) {
    let bucket_order = [
        "document_status",
        "structured_fact",
        "spreadsheet",
        "retrieval",
        "database",
        "memory",
        "asset_profile",
        "other",
    ];
    let mut selected_indices = BTreeSet::new();
    let mut included_by_type = BTreeMap::<String, usize>::new();
    let mut omitted_by_type = BTreeMap::<String, usize>::new();

    for bucket in bucket_order {
        let mut included_in_bucket = 0usize;
        let bucket_limit = assistant_run_model_supply_bucket_limit(bucket);
        for (index, item) in items.iter().enumerate() {
            if assistant_run_model_supply_bucket(item) != bucket {
                continue;
            }
            let type_label = assistant_run_model_supply_type_label(item);
            if selected_indices.len() < ASSISTANT_RUN_MODEL_CONTEXT_SUPPLIED_ITEM_LIMIT
                && included_in_bucket < bucket_limit
            {
                selected_indices.insert(index);
                included_in_bucket += 1;
                *included_by_type.entry(type_label).or_insert(0) += 1;
            } else {
                *omitted_by_type.entry(type_label).or_insert(0) += 1;
            }
        }
    }

    let compacted_items = selected_indices
        .iter()
        .filter_map(|index| items.get(*index))
        .map(assistant_run_model_supply_item_for_context)
        .collect::<Vec<_>>();
    let budget = json!({
        "policy": "model_input_balanced_supply_buckets",
        "original_supplied_item_count": items.len(),
        "model_supplied_item_count": compacted_items.len(),
        "omitted_supplied_item_count": items.len().saturating_sub(compacted_items.len()),
        "global_item_limit": ASSISTANT_RUN_MODEL_CONTEXT_SUPPLIED_ITEM_LIMIT,
        "bucket_limits": {
            "document_status": ASSISTANT_RUN_MODEL_CONTEXT_PARSE_STATUS_LIMIT,
            "structured_fact": ASSISTANT_RUN_MODEL_CONTEXT_STRUCTURED_FACT_LIMIT,
            "spreadsheet": ASSISTANT_RUN_MODEL_CONTEXT_SPREADSHEET_LIMIT,
            "retrieval": ASSISTANT_RUN_MODEL_CONTEXT_RETRIEVAL_LIMIT,
            "database": ASSISTANT_RUN_MODEL_CONTEXT_DATABASE_LIMIT,
            "memory": ASSISTANT_RUN_MODEL_CONTEXT_MEMORY_LIMIT,
            "asset_profile": ASSISTANT_RUN_MODEL_CONTEXT_ASSET_PROFILE_LIMIT,
            "other": ASSISTANT_RUN_MODEL_CONTEXT_OTHER_LIMIT,
        },
        "included_by_type": included_by_type,
        "omitted_by_type": omitted_by_type,
        "model_rule": "The model input keeps separate quotas for document evidence, database aggregates, structured scans, spreadsheet analyses, parse status, and memory so one source cannot crowd out the others. Omitted items remain available to host tools/detail reads; do not infer that omitted means absent from the dataset.",
    });

    (compacted_items, budget)
}

fn assistant_run_model_supply_bucket(item: &Value) -> &'static str {
    match item.get("type").and_then(Value::as_str) {
        Some("document_parse_status") => "document_status",
        Some("dataset_entity_scan" | "dataset_fact_snapshot") => "structured_fact",
        Some("spreadsheet_row_analysis") => "spreadsheet",
        Some("database_schema_context" | "database_aggregate" | "database_aggregate_error") => {
            "database"
        }
        Some("conversation_memory_item") => "memory",
        Some("asset_profile_hint") => "asset_profile",
        Some("retrieval_evidence" | "search_evidence") => "retrieval",
        _ => "other",
    }
}

fn assistant_run_model_supply_bucket_limit(bucket: &str) -> usize {
    match bucket {
        "document_status" => ASSISTANT_RUN_MODEL_CONTEXT_PARSE_STATUS_LIMIT,
        "structured_fact" => ASSISTANT_RUN_MODEL_CONTEXT_STRUCTURED_FACT_LIMIT,
        "spreadsheet" => ASSISTANT_RUN_MODEL_CONTEXT_SPREADSHEET_LIMIT,
        "retrieval" => ASSISTANT_RUN_MODEL_CONTEXT_RETRIEVAL_LIMIT,
        "database" => ASSISTANT_RUN_MODEL_CONTEXT_DATABASE_LIMIT,
        "memory" => ASSISTANT_RUN_MODEL_CONTEXT_MEMORY_LIMIT,
        "asset_profile" => ASSISTANT_RUN_MODEL_CONTEXT_ASSET_PROFILE_LIMIT,
        _ => ASSISTANT_RUN_MODEL_CONTEXT_OTHER_LIMIT,
    }
}

fn assistant_run_model_supply_type_label(item: &Value) -> String {
    item.get("type")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn item(item_type: &str, index: usize) -> Value {
        json!({
            "type": item_type,
            "summary": format!("summary-{index}"),
            "content_excerpt": format!("content-{index}"),
            "rows": [{"name": format!("row-{index}")}],
        })
    }

    #[test]
    fn budget_keeps_retrieval_and_database_bucket_quotas_separate() {
        let mut items = Vec::new();
        for index in 0..10 {
            items.push(item("database_aggregate", index));
        }
        for index in 0..10 {
            items.push(item("retrieval_evidence", index));
        }

        let (model_items, budget) = assistant_run_model_budgeted_supply_items(&items);
        let database_count = model_items
            .iter()
            .filter(|item| item.get("type").and_then(Value::as_str) == Some("database_aggregate"))
            .count();
        let retrieval_count = model_items
            .iter()
            .filter(|item| item.get("type").and_then(Value::as_str) == Some("retrieval_evidence"))
            .count();

        assert_eq!(database_count, ASSISTANT_RUN_MODEL_CONTEXT_DATABASE_LIMIT);
        assert_eq!(retrieval_count, ASSISTANT_RUN_MODEL_CONTEXT_RETRIEVAL_LIMIT);
        assert_eq!(
            budget["included_by_type"]["database_aggregate"],
            json!(ASSISTANT_RUN_MODEL_CONTEXT_DATABASE_LIMIT)
        );
        assert_eq!(
            budget["omitted_by_type"]["database_aggregate"],
            json!(10 - ASSISTANT_RUN_MODEL_CONTEXT_DATABASE_LIMIT)
        );
        assert_eq!(
            budget["omitted_by_type"]["retrieval_evidence"],
            json!(10 - ASSISTANT_RUN_MODEL_CONTEXT_RETRIEVAL_LIMIT)
        );
    }

    #[test]
    fn budget_preserves_original_order_after_bucket_selection() {
        let items = vec![
            item("database_aggregate", 0),
            item("retrieval_evidence", 1),
            item("database_aggregate", 2),
            item("retrieval_evidence", 3),
        ];

        let (model_items, budget) = assistant_run_model_budgeted_supply_items(&items);

        assert_eq!(
            model_items
                .iter()
                .map(|item| item["summary"].as_str().unwrap_or_default())
                .collect::<Vec<_>>(),
            vec!["summary-0", "summary-1", "summary-2", "summary-3"]
        );
        assert_eq!(budget["omitted_supplied_item_count"], json!(0));
    }

    #[test]
    fn budget_labels_missing_type_as_unknown_other() {
        let items = vec![json!({"summary": "missing type"})];

        let (model_items, budget) = assistant_run_model_budgeted_supply_items(&items);

        assert_eq!(model_items.len(), 1);
        assert_eq!(budget["included_by_type"]["unknown"], json!(1));
        assert_eq!(
            budget["bucket_limits"]["other"],
            json!(ASSISTANT_RUN_MODEL_CONTEXT_OTHER_LIMIT)
        );
    }

    #[test]
    fn budget_keeps_asset_profile_hints_in_their_own_bucket() {
        let mut items = Vec::new();
        for index in 0..8 {
            items.push(item("asset_profile_hint", index));
        }
        for index in 0..8 {
            items.push(item("retrieval_evidence", index));
        }

        let (model_items, budget) = assistant_run_model_budgeted_supply_items(&items);
        let asset_profile_count = model_items
            .iter()
            .filter(|item| item.get("type").and_then(Value::as_str) == Some("asset_profile_hint"))
            .count();
        let retrieval_count = model_items
            .iter()
            .filter(|item| item.get("type").and_then(Value::as_str) == Some("retrieval_evidence"))
            .count();

        assert_eq!(
            asset_profile_count,
            ASSISTANT_RUN_MODEL_CONTEXT_ASSET_PROFILE_LIMIT
        );
        assert_eq!(retrieval_count, ASSISTANT_RUN_MODEL_CONTEXT_RETRIEVAL_LIMIT);
        assert_eq!(
            budget["included_by_type"]["asset_profile_hint"],
            json!(ASSISTANT_RUN_MODEL_CONTEXT_ASSET_PROFILE_LIMIT)
        );
        assert_eq!(
            budget["bucket_limits"]["asset_profile"],
            json!(ASSISTANT_RUN_MODEL_CONTEXT_ASSET_PROFILE_LIMIT)
        );
    }
}
