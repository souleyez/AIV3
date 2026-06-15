use std::collections::BTreeSet;

use serde_json::Value;

use crate::assistant_run_scope_selection_support::selected_dataset_ids_from_scope;
use crate::static_page_template_match_support::{
    static_page_stable_key_insert_scope_item_external_id,
    static_page_stable_key_insert_scope_item_id, static_page_stable_key_insert_value,
    static_page_stable_key_token,
};

fn static_page_stable_key_collect_scope_parts(
    selected_scope: &Value,
    source_refs: &Value,
    connection_id: Option<&str>,
) -> BTreeSet<String> {
    let mut parts = BTreeSet::new();
    if let Some(connection_id) = connection_id.and_then(static_page_stable_key_token) {
        parts.insert(format!("channel:{connection_id}"));
    }
    for key in [
        "dataset_external_ids",
        "requested_dataset_external_ids",
        "available_dataset_external_ids",
    ] {
        static_page_stable_key_insert_value(
            &mut parts,
            key.trim_end_matches('s'),
            selected_scope.get(key),
        );
    }
    if let Some(dataset_scope) = selected_scope.get("dataset_document_scope") {
        for key in ["dataset_external_ids", "requested_dataset_external_ids"] {
            static_page_stable_key_insert_value(
                &mut parts,
                key.trim_end_matches('s'),
                dataset_scope.get(key),
            );
        }
    }

    let has_dataset_external_scope = parts.iter().any(|part| {
        part.starts_with("dataset_external_id:")
            || part.starts_with("requested_dataset_external_id:")
            || part.starts_with("available_dataset_external_id:")
    });

    if !has_dataset_external_scope {
        if let Some(items) = selected_scope
            .get("canonical_datasets")
            .and_then(Value::as_array)
        {
            for item in items {
                static_page_stable_key_insert_scope_item_id(
                    &mut parts,
                    "canonical_dataset_id",
                    item,
                );
            }
        }
    }

    let has_canonical_dataset_scope = parts
        .iter()
        .any(|part| part.starts_with("canonical_dataset_id:"));
    if !has_dataset_external_scope && !has_canonical_dataset_scope {
        for dataset_id in selected_dataset_ids_from_scope(selected_scope) {
            parts.insert(format!("dataset_id:{dataset_id}"));
        }
    }

    if !parts.iter().any(|part| {
        part.starts_with("dataset_external_id:")
            || part.starts_with("requested_dataset_external_id:")
            || part.starts_with("available_dataset_external_id:")
            || part.starts_with("canonical_dataset_id:")
            || part.starts_with("dataset_id:")
    }) {
        static_page_stable_key_insert_value(
            &mut parts,
            "available_document_external_id",
            selected_scope.get("available_document_external_ids"),
        );
        if let Some(items) = selected_scope.get("documents").and_then(Value::as_array) {
            for item in items {
                static_page_stable_key_insert_scope_item_external_id(
                    &mut parts,
                    "document_external_id",
                    item,
                );
                static_page_stable_key_insert_scope_item_id(&mut parts, "document_id", item);
            }
        }
    }

    for key in [
        "database_source_ids",
        "databaseSourceIds",
        "database_source_id",
        "databaseSourceId",
    ] {
        static_page_stable_key_insert_value(
            &mut parts,
            key.trim_end_matches('s'),
            source_refs.get(key).or_else(|| selected_scope.get(key)),
        );
    }
    if let Some(recipient_delivery) = source_refs.get("recipient_delivery") {
        static_page_stable_key_insert_value(
            &mut parts,
            "recipient_user",
            recipient_delivery.get("target_external_user_ids"),
        );
        if let Some(role_scope_candidates) = recipient_delivery
            .get("role_scope_candidates")
            .and_then(Value::as_array)
        {
            for candidate in role_scope_candidates {
                if let Some(role) = candidate
                    .get("role")
                    .and_then(Value::as_str)
                    .and_then(static_page_stable_key_token)
                {
                    parts.insert(format!("recipient_role:{role}"));
                }
                if let Some(default_scope) = candidate
                    .get("default_scope")
                    .and_then(Value::as_str)
                    .and_then(static_page_stable_key_token)
                {
                    parts.insert(format!("recipient_scope:{default_scope}"));
                }
            }
        }
        if let Some(provided_mapping) = recipient_delivery
            .get("provided_mapping")
            .filter(|value| !value.is_null())
        {
            if let Ok(mapping_text) = serde_json::to_string(provided_mapping) {
                if let Some(token) = static_page_stable_key_token(&mapping_text) {
                    parts.insert(format!("recipient_mapping:{token}"));
                }
            }
        }
    }
    parts
}

pub(crate) fn static_page_dataset_artifact_key(
    selected_scope: &Value,
    source_refs: &Value,
    template_stability_key: &str,
    connection_id: Option<&str>,
) -> Option<String> {
    let scope_parts =
        static_page_stable_key_collect_scope_parts(selected_scope, source_refs, connection_id);
    let has_material_scope = scope_parts.iter().any(|part| {
        part.starts_with("dataset_external_id:")
            || part.starts_with("requested_dataset_external_id:")
            || part.starts_with("canonical_dataset_id:")
            || part.starts_with("dataset_id:")
            || part.starts_with("available_document_external_id:")
            || part.starts_with("document_external_id:")
            || part.starts_with("document_id:")
            || part.starts_with("database_source_id:")
            || part.starts_with("databaseSourceId:")
    });
    if !has_material_scope {
        return None;
    }
    let mut parts = vec!["v3-static-page".to_string()];
    parts.push(template_stability_key.to_string());
    parts.extend(scope_parts);
    Some(parts.join("|"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain_model::DatasetId;
    use serde_json::json;

    #[test]
    fn dataset_artifact_key_prefers_external_dataset_scope_over_temporary_dataset_id() {
        let temporary_dataset_id = DatasetId::new();
        let selected_scope = json!({
            "dataset_external_ids": ["xinbai-main"],
            "datasets": [{"type": "dataset", "id": temporary_dataset_id}]
        });
        let source_refs = json!({
            "channel_connection_id": "generic-chat-main"
        });

        let key = static_page_dataset_artifact_key(
            &selected_scope,
            &source_refs,
            "template:default",
            Some("generic-chat-main"),
        )
        .expect("dataset artifact key");

        assert!(key.contains("dataset_external_id:xinbai-main"));
        assert!(key.contains("channel:generic-chat-main"));
        assert!(!key.contains(&temporary_dataset_id.to_string()));
    }

    #[test]
    fn dataset_artifact_key_uses_canonical_dataset_before_selected_dataset_id() {
        let canonical_dataset_id = DatasetId::new();
        let temporary_dataset_id = DatasetId::new();
        let selected_scope = json!({
            "canonical_datasets": [{"type": "dataset", "id": canonical_dataset_id}],
            "datasets": [{"type": "dataset", "id": temporary_dataset_id}]
        });

        let key =
            static_page_dataset_artifact_key(&selected_scope, &json!({}), "template:default", None)
                .expect("canonical dataset key");

        assert!(key.contains(&format!("canonical_dataset_id:{canonical_dataset_id}")));
        assert!(!key.contains(&format!("dataset_id:{temporary_dataset_id}")));
    }

    #[test]
    fn dataset_artifact_key_accepts_document_and_database_source_material_scope() {
        let selected_scope = json!({
            "available_document_external_ids": ["doc-001"]
        });
        let source_refs = json!({
            "databaseSourceIds": ["source-main"]
        });

        let key = static_page_dataset_artifact_key(
            &selected_scope,
            &source_refs,
            "template:report",
            None,
        )
        .expect("document and database source key");

        assert!(key.contains("available_document_external_id:doc-001"));
        assert!(key.contains("databaseSourceId:source-main"));
    }

    #[test]
    fn dataset_artifact_key_requires_material_scope() {
        let selected_scope = json!({});
        let source_refs = json!({
            "channel_connection_id": "generic-chat-main",
            "recipient_delivery": {
                "target_external_user_ids": ["store-manager"]
            }
        });

        assert!(static_page_dataset_artifact_key(
            &selected_scope,
            &source_refs,
            "template:default",
            Some("generic-chat-main"),
        )
        .is_none());
    }
}
