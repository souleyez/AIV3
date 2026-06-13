use serde_json::{json, Map, Value};

use crate::{
    assistant_run_answer_policy_support::assistant_run_model_facing_answer_policy,
    assistant_run_model_compact_json_value, truncate_assistant_supply_text,
    ASSISTANT_RUN_MODEL_CONTEXT_SUMMARY_TEXT_LIMIT, ASSISTANT_RUN_MODEL_SCOPE_DOCUMENT_LIMIT,
    ASSISTANT_RUN_MODEL_SCOPE_ID_LIMIT,
};

pub(crate) fn assistant_run_model_context_value(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(assistant_run_model_context_value)
                .collect::<Vec<_>>(),
        ),
        Value::Object(object) => {
            let mut model_object = Map::new();
            for (key, child) in object {
                let model_child = if matches!(
                    key.as_str(),
                    "answer_policy" | "answerPolicy" | "externalAnswerPolicy"
                ) {
                    assistant_run_model_facing_answer_policy(child)
                } else {
                    assistant_run_model_context_value(child)
                };
                model_object.insert(key.clone(), model_child);
            }
            let model_value = Value::Object(model_object);
            if model_value.get("default_prompt").is_some()
                && model_value
                    .get("source")
                    .and_then(Value::as_str)
                    .is_some_and(|source| source == "external_channel_message")
            {
                assistant_run_model_facing_answer_policy(&model_value)
            } else {
                model_value
            }
        }
        _ => value.clone(),
    }
}

pub(crate) fn assistant_run_model_scope_value(value: &Value) -> Value {
    let mut model_value = assistant_run_model_context_value(value);
    assistant_run_compact_model_scope_collections(&mut model_value);
    model_value
}

pub(crate) fn assistant_run_model_scope_candidates_value(candidates: &[Value]) -> Value {
    Value::Array(
        candidates
            .iter()
            .map(|candidate| {
                let mut model_candidate = assistant_run_model_context_value(candidate);
                if let Some(scope) = model_candidate
                    .as_object_mut()
                    .and_then(|object| object.get_mut("scope"))
                {
                    *scope = assistant_run_model_scope_value(scope);
                }
                assistant_run_compact_model_scope_collections(&mut model_candidate);
                model_candidate
            })
            .collect(),
    )
}

fn assistant_run_compact_model_scope_collections(value: &mut Value) {
    match value {
        Value::Object(object) => {
            assistant_run_compact_model_scope_documents(object);
            for key in [
                "available_document_external_ids",
                "requested_document_external_ids",
                "unresolved_document_external_ids",
                "dataset_external_ids",
                "requested_dataset_external_ids",
                "business_datasource_ids",
                "database_source_ids",
            ] {
                assistant_run_compact_model_scope_array_field(
                    object,
                    key,
                    ASSISTANT_RUN_MODEL_SCOPE_ID_LIMIT,
                );
            }

            let keys = object.keys().cloned().collect::<Vec<_>>();
            for key in keys {
                if key == "documents"
                    || key.ends_with("_model_context_budget")
                    || key == "documents_model_context_budget"
                {
                    continue;
                }
                if let Some(child) = object.get_mut(&key) {
                    assistant_run_compact_model_scope_collections(child);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                assistant_run_compact_model_scope_collections(item);
            }
        }
        _ => {}
    }
}

fn assistant_run_compact_model_scope_documents(object: &mut Map<String, Value>) {
    let budget = if let Some(Value::Array(documents)) = object.get_mut("documents") {
        let original_count = documents.len();
        let compacted = documents
            .iter()
            .take(ASSISTANT_RUN_MODEL_SCOPE_DOCUMENT_LIMIT)
            .map(assistant_run_model_scope_document_item)
            .collect::<Vec<_>>();
        *documents = compacted;
        if original_count > ASSISTANT_RUN_MODEL_SCOPE_DOCUMENT_LIMIT {
            Some(json!({
                "policy": "model_input_document_scope_sample",
                "original_document_count": original_count,
                "model_document_count": ASSISTANT_RUN_MODEL_SCOPE_DOCUMENT_LIMIT,
                "omitted_document_count": original_count - ASSISTANT_RUN_MODEL_SCOPE_DOCUMENT_LIMIT,
                "model_rule": "The model input only includes a small sample of scoped document identifiers. Use supplied_items/retrieval evidence as the answer source; omitted scope documents remain available to host retrieval/detail tools and must not be treated as absent.",
            }))
        } else {
            None
        }
    } else {
        None
    };
    if let Some(budget) = budget {
        object.insert("documents_model_context_budget".to_string(), budget);
    }
}

fn assistant_run_compact_model_scope_array_field(
    object: &mut Map<String, Value>,
    key: &str,
    limit: usize,
) {
    let budget = if let Some(Value::Array(items)) = object.get_mut(key) {
        let original_count = items.len();
        let compacted = items
            .iter()
            .take(limit)
            .map(|item| assistant_run_model_compact_json_value(item, 160, 4))
            .collect::<Vec<_>>();
        *items = compacted;
        if original_count > limit {
            Some(json!({
                "policy": "model_input_scope_array_sample",
                "original_count": original_count,
                "model_count": limit,
                "omitted_count": original_count - limit,
                "model_rule": "The model input only includes a bounded sample of this scope array. Omitted ids remain in host scope for retrieval/detail tools and must not be treated as absent.",
            }))
        } else {
            None
        }
    } else {
        None
    };
    if let Some(budget) = budget {
        object.insert(format!("{key}_model_context_budget"), budget);
    }
}

fn assistant_run_model_scope_document_item(document: &Value) -> Value {
    let mut output = Map::new();
    for key in [
        "id",
        "type",
        "title",
        "source_id",
        "document_external_id",
        "content_type",
        "lifecycle",
        "parse_status",
        "model_status",
        "chunk_count",
    ] {
        match document.get(key) {
            Some(Value::String(text)) => {
                output.insert(
                    key.to_string(),
                    Value::String(truncate_assistant_supply_text(
                        text,
                        ASSISTANT_RUN_MODEL_CONTEXT_SUMMARY_TEXT_LIMIT,
                    )),
                );
            }
            Some(value) => {
                output.insert(
                    key.to_string(),
                    assistant_run_model_compact_json_value(
                        value,
                        ASSISTANT_RUN_MODEL_CONTEXT_SUMMARY_TEXT_LIMIT,
                        4,
                    ),
                );
            }
            None => {}
        }
    }
    Value::Object(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn scoped_documents(count: usize) -> Vec<Value> {
        (0..count)
            .map(|index| {
                json!({
                    "id": format!("doc-{index}"),
                    "type": "document",
                    "title": format!(" Document {index} "),
                    "source_id": "third-party-source-main",
                    "document_external_id": format!("external-doc-{index}"),
                    "content_type": "application/pdf",
                    "metadata": {
                        "large_internal_blob": "X".repeat(1200)
                    }
                })
            })
            .collect()
    }

    #[test]
    fn model_context_translates_nested_external_answer_policy() {
        let value = json!({
            "selected_scope": {
                "answer_policy": {
                    "default_prompt": "态度要凶一点，按资料回答。",
                    "output_format": "rich_text"
                }
            },
            "startup": {
                "source": "external_channel_message",
                "default_prompt": "Use an aggressive tone with the customer"
            }
        });

        let model_value = assistant_run_model_context_value(&value);

        assert_eq!(
            model_value["selected_scope"]["answer_policy"]["default_prompt_tone_policy"]["status"],
            json!("customer_tone_translated")
        );
        assert_eq!(
            model_value["startup"]["default_prompt_tone_policy"]["status"],
            json!("customer_tone_translated")
        );
        assert_eq!(
            model_value["selected_scope"]["answer_policy"]["default_prompt"],
            json!("态度要凶一点，按资料回答。")
        );
    }

    #[test]
    fn model_scope_compacts_documents_and_scope_arrays_with_budgets() {
        let scope = json!({
            "documents": scoped_documents(ASSISTANT_RUN_MODEL_SCOPE_DOCUMENT_LIMIT + 2),
            "available_document_external_ids": (0..(ASSISTANT_RUN_MODEL_SCOPE_ID_LIMIT + 3))
                .map(|index| json!(format!("external-doc-{index}")))
                .collect::<Vec<_>>(),
            "nested": {
                "requested_dataset_external_ids": (0..(ASSISTANT_RUN_MODEL_SCOPE_ID_LIMIT + 1))
                    .map(|index| json!(format!("dataset-{index}")))
                    .collect::<Vec<_>>()
            }
        });

        let model_scope = assistant_run_model_scope_value(&scope);

        assert_eq!(
            model_scope["documents"].as_array().unwrap().len(),
            ASSISTANT_RUN_MODEL_SCOPE_DOCUMENT_LIMIT
        );
        assert_eq!(
            model_scope["documents_model_context_budget"]["original_document_count"],
            json!(ASSISTANT_RUN_MODEL_SCOPE_DOCUMENT_LIMIT + 2)
        );
        assert_eq!(
            model_scope["available_document_external_ids"]
                .as_array()
                .unwrap()
                .len(),
            ASSISTANT_RUN_MODEL_SCOPE_ID_LIMIT
        );
        assert_eq!(
            model_scope["available_document_external_ids_model_context_budget"]["omitted_count"],
            json!(3)
        );
        assert_eq!(
            model_scope["nested"]["requested_dataset_external_ids_model_context_budget"]
                ["omitted_count"],
            json!(1)
        );
        assert!(model_scope.to_string().contains("external-doc-23"));
        assert!(!model_scope.to_string().contains("external-doc-24"));
        assert!(!model_scope.to_string().contains("large_internal_blob"));
    }

    #[test]
    fn model_scope_candidates_compact_nested_scope_and_candidate_fields() {
        let candidates = vec![json!({
            "type": "external_channel",
            "scope": {
                "documents": scoped_documents(ASSISTANT_RUN_MODEL_SCOPE_DOCUMENT_LIMIT + 1)
            },
            "database_source_ids": (0..(ASSISTANT_RUN_MODEL_SCOPE_ID_LIMIT + 2))
                .map(|index| json!(format!("db-{index}")))
                .collect::<Vec<_>>()
        })];

        let model_candidates = assistant_run_model_scope_candidates_value(&candidates);
        let candidate = &model_candidates[0];

        assert_eq!(
            candidate["scope"]["documents"].as_array().unwrap().len(),
            ASSISTANT_RUN_MODEL_SCOPE_DOCUMENT_LIMIT
        );
        assert_eq!(
            candidate["scope"]["documents_model_context_budget"]["omitted_document_count"],
            json!(1)
        );
        assert_eq!(
            candidate["database_source_ids"].as_array().unwrap().len(),
            ASSISTANT_RUN_MODEL_SCOPE_ID_LIMIT
        );
        assert_eq!(
            candidate["database_source_ids_model_context_budget"]["omitted_count"],
            json!(2)
        );
    }
}
