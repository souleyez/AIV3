use serde_json::{json, Value};
use std::collections::BTreeSet;

use crate::static_page_payload_support::set_payload_value;

pub(crate) fn external_channel_static_page_report_scope(selected_scope: &Value) -> Value {
    let Some(database_source_scope) = selected_scope.get("database_source_scope") else {
        return selected_scope.clone();
    };
    let Some(bindings) = database_source_scope
        .get("default_dataset_bindings")
        .and_then(Value::as_array)
    else {
        return selected_scope.clone();
    };

    let mut report_datasets = Vec::new();
    let mut database_source_ids = BTreeSet::new();
    for binding in bindings {
        let Some(dataset_id) = binding
            .get("dataset_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let source_id = binding
            .get("source_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if let Some(source_id) = source_id {
            database_source_ids.insert(source_id.to_string());
        }
        let mut item = json!({
            "type": "dataset",
            "id": dataset_id,
            "source": "database_source_default_dataset",
            "report_scope": true,
        });
        if let Some(source_id) = source_id {
            set_payload_value(&mut item, "database_source_id", json!(source_id));
        }
        report_datasets.push(item);
    }

    if report_datasets.is_empty() {
        return selected_scope.clone();
    }

    if let Some(ids) = database_source_scope
        .get("source_ids")
        .and_then(Value::as_array)
    {
        for id in ids {
            if let Some(id) = id.as_str().map(str::trim).filter(|value| !value.is_empty()) {
                database_source_ids.insert(id.to_string());
            }
        }
    }

    let mut report_scope = json!({
        "type": "external_channel_report",
        "mode": "report_fixed_dataset",
        "scope_source": "database_source_default_dataset_bindings",
        "database_report_scope_policy": "fixed_dataset_independent_of_chat_selection",
        "datasets": report_datasets,
        "selected": report_datasets,
        "database_source_scope": database_source_scope,
        "database_source_ids": database_source_ids.into_iter().collect::<Vec<_>>(),
    });

    for key in [
        "v3_system_user_id",
        "answer_policy",
        "user_context",
        "external_user_context",
    ] {
        if let Some(value) = selected_scope.get(key) {
            set_payload_value(&mut report_scope, key, value.clone());
        }
    }

    report_scope
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_scope_without_database_scope_returns_original_scope() {
        let selected_scope = json!({
            "type": "external_channel",
            "datasets": [{"type": "dataset", "id": "chat-dataset"}],
            "documents": [{"type": "document", "id": "chat-document"}],
        });

        assert_eq!(
            external_channel_static_page_report_scope(&selected_scope),
            selected_scope
        );
    }

    #[test]
    fn report_scope_without_valid_default_bindings_returns_original_scope() {
        let selected_scope = json!({
            "type": "external_channel",
            "datasets": [{"type": "dataset", "id": "chat-dataset"}],
            "database_source_scope": {
                "source_ids": ["db-main"],
                "default_dataset_bindings": [
                    {"source_id": "db-main", "dataset_id": " "}
                ]
            },
        });

        assert_eq!(
            external_channel_static_page_report_scope(&selected_scope),
            selected_scope
        );
    }

    #[test]
    fn report_scope_uses_default_dataset_bindings_and_preserves_safe_context() {
        let selected_scope = json!({
            "type": "external_channel",
            "dataset_external_ids": ["chat-group"],
            "requested_dataset_external_ids": ["chat-group"],
            "documents": [{"type": "document", "id": "chat-document"}],
            "answer_policy": {"output_format": "rich_text"},
            "user_context": {"role": "manager"},
            "external_user_context": {"sender": "third-party-user"},
            "v3_system_user_id": "system-user",
            "database_source_scope": {
                "source": "external_channel_allowed_database_sources",
                "source_ids": ["db-source-b", "db-source-a", " "],
                "default_dataset_bindings": [
                    {"source_id": "db-source-b", "dataset_id": " report-dataset-b "},
                    {"source_id": "db-source-a", "dataset_id": "report-dataset-a"}
                ],
                "policy": "connection_allowed_database_sources_for_report_workflow"
            },
        });

        let report_scope = external_channel_static_page_report_scope(&selected_scope);

        assert_eq!(report_scope["type"], json!("external_channel_report"));
        assert_eq!(report_scope["mode"], json!("report_fixed_dataset"));
        assert_eq!(
            report_scope["scope_source"],
            json!("database_source_default_dataset_bindings")
        );
        assert_eq!(
            report_scope["database_report_scope_policy"],
            json!("fixed_dataset_independent_of_chat_selection")
        );
        assert_eq!(
            report_scope["database_source_ids"],
            json!(["db-source-a", "db-source-b"])
        );
        assert_eq!(
            report_scope["datasets"],
            json!([
                {
                    "type": "dataset",
                    "id": "report-dataset-b",
                    "source": "database_source_default_dataset",
                    "report_scope": true,
                    "database_source_id": "db-source-b",
                },
                {
                    "type": "dataset",
                    "id": "report-dataset-a",
                    "source": "database_source_default_dataset",
                    "report_scope": true,
                    "database_source_id": "db-source-a",
                }
            ])
        );
        assert_eq!(report_scope["selected"], report_scope["datasets"]);
        assert_eq!(
            report_scope["answer_policy"],
            json!({"output_format": "rich_text"})
        );
        assert_eq!(report_scope["user_context"], json!({"role": "manager"}));
        assert_eq!(
            report_scope["external_user_context"],
            json!({"sender": "third-party-user"})
        );
        assert_eq!(report_scope["v3_system_user_id"], json!("system-user"));
        assert!(report_scope.get("dataset_external_ids").is_none());
        assert!(report_scope.get("requested_dataset_external_ids").is_none());
        assert!(report_scope.get("documents").is_none());
    }
}
