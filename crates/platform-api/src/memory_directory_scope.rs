use std::collections::HashSet;

use domain_model::{DocumentId, MemoryDirectory, UserId};
use serde_json::Value;
use uuid::Uuid;

use crate::resource_access::owner_user_id_is_visible;

pub(crate) fn memory_directory_source_document_ids(directory: &MemoryDirectory) -> Vec<DocumentId> {
    if !directory.source_document_ids.is_empty() {
        return directory.source_document_ids.clone();
    }

    let mut ids = Vec::new();
    collect_memory_manifest_document_ids(&directory.directory_manifest, &mut ids);
    ids
}

fn collect_memory_manifest_document_ids(value: &Value, ids: &mut Vec<DocumentId>) {
    match value {
        Value::Object(object) => {
            if let Some(document_id) = object
                .get("document_id")
                .and_then(Value::as_str)
                .and_then(|value| Uuid::parse_str(value).ok())
                .map(DocumentId)
            {
                push_unique_document_id(ids, document_id);
            }
            if let Some(source_ids) = object.get("source_document_ids").and_then(Value::as_array) {
                for source_id in source_ids {
                    if let Some(document_id) = source_id
                        .as_str()
                        .and_then(|value| Uuid::parse_str(value).ok())
                        .map(DocumentId)
                    {
                        push_unique_document_id(ids, document_id);
                    }
                }
            }
            for child in object.values() {
                collect_memory_manifest_document_ids(child, ids);
            }
        }
        Value::Array(entries) => {
            for entry in entries {
                collect_memory_manifest_document_ids(entry, ids);
            }
        }
        _ => {}
    }
}

fn push_unique_document_id(ids: &mut Vec<DocumentId>, document_id: DocumentId) {
    if !ids.iter().any(|existing| *existing == document_id) {
        ids.push(document_id);
    }
}

pub(crate) fn memory_directory_matches_visible_scope(
    directory: &MemoryDirectory,
    visible_document_ids: &HashSet<DocumentId>,
    current_user_id: Option<UserId>,
) -> bool {
    owner_user_id_is_visible(directory.owner_user_id, current_user_id)
        && memory_directory_source_document_ids(directory)
            .into_iter()
            .all(|document_id| visible_document_ids.contains(&document_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use domain_model::{DatasetId, MemoryDirectoryId, TenantId, WorkflowExecutionId};
    use serde_json::json;

    fn directory(
        owner_user_id: Option<UserId>,
        source_document_ids: Vec<DocumentId>,
        directory_manifest: Value,
    ) -> MemoryDirectory {
        let now = Utc.timestamp_opt(1_700_000_000, 0).single().unwrap();
        MemoryDirectory {
            id: MemoryDirectoryId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            execution_id: WorkflowExecutionId::new(),
            owner_user_id,
            source_document_ids,
            version_no: 1,
            directory_nodes: 0,
            refreshed_chunks: 0,
            directory_manifest,
            created_at: now,
        }
    }

    #[test]
    fn memory_directory_source_document_ids_preserve_explicit_source_ids() {
        let first = DocumentId::new();
        let second = DocumentId::new();
        let manifest_only = DocumentId::new();
        let directory = directory(
            None,
            vec![first, second],
            json!({
                "document_id": manifest_only.to_string(),
                "source_document_ids": [manifest_only.to_string()]
            }),
        );

        assert_eq!(
            memory_directory_source_document_ids(&directory),
            vec![first, second]
        );
    }

    #[test]
    fn memory_directory_source_document_ids_collect_manifest_recursively_and_dedupe() {
        let first = DocumentId::new();
        let second = DocumentId::new();
        let third = DocumentId::new();
        let directory = directory(
            None,
            Vec::new(),
            json!([
                {"document_id": first.to_string()},
                {
                    "source_document_ids": [
                        second.to_string(),
                        first.to_string(),
                        "not-a-uuid"
                    ],
                    "children": [
                        {"document_id": third.to_string()},
                        {"document_id": second.to_string()}
                    ]
                }
            ]),
        );

        assert_eq!(
            memory_directory_source_document_ids(&directory),
            vec![first, second, third]
        );
    }

    #[test]
    fn memory_directory_matches_visible_scope_preserves_owner_and_document_acl_rules() {
        let owner = UserId::new();
        let allowed_document = DocumentId::new();
        let hidden_document = DocumentId::new();
        let mut visible_document_ids = HashSet::new();
        visible_document_ids.insert(allowed_document);

        let public_directory = directory(None, vec![allowed_document], json!({}));
        assert!(memory_directory_matches_visible_scope(
            &public_directory,
            &visible_document_ids,
            None
        ));

        let owned_directory = directory(Some(owner), vec![allowed_document], json!({}));
        assert!(memory_directory_matches_visible_scope(
            &owned_directory,
            &visible_document_ids,
            Some(owner)
        ));
        assert!(!memory_directory_matches_visible_scope(
            &owned_directory,
            &visible_document_ids,
            None
        ));
        assert!(!memory_directory_matches_visible_scope(
            &owned_directory,
            &visible_document_ids,
            Some(UserId::new())
        ));

        let out_of_scope_directory = directory(None, vec![hidden_document], json!({}));
        assert!(!memory_directory_matches_visible_scope(
            &out_of_scope_directory,
            &visible_document_ids,
            None
        ));
    }
}
