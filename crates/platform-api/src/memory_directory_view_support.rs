use contracts::MemoryDirectoryView;
use domain_model::{DatasetId, DocumentId, MemoryDirectory};
use serde_json::Value;
use uuid::Uuid;

pub(crate) fn to_memory_directory_view(directory: MemoryDirectory) -> MemoryDirectoryView {
    MemoryDirectoryView {
        id: directory.id,
        dataset_id: directory.dataset_id,
        execution_id: directory.execution_id,
        owner_user_id: directory.owner_user_id,
        source_document_ids: directory.source_document_ids,
        version_no: directory.version_no,
        directory_nodes: directory.directory_nodes,
        refreshed_chunks: directory.refreshed_chunks,
        directory_tree: parse_memory_directory_manifest(
            &directory.directory_manifest,
            directory.version_no,
        ),
        directory_manifest: directory.directory_manifest,
        created_at: directory.created_at,
    }
}

fn parse_memory_directory_manifest(
    value: &Value,
    version_no: i32,
) -> Option<contracts::MemoryDirectoryManifestView> {
    let object = value.as_object()?;

    Some(contracts::MemoryDirectoryManifestView {
        schema_version: object.get("schema_version")?.as_str()?.to_string(),
        generator: object.get("generator")?.as_str()?.to_string(),
        dataset_id: DatasetId::from(Uuid::parse_str(object.get("dataset_id")?.as_str()?).ok()?),
        version_no: object
            .get("version_no")
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok())
            .unwrap_or(version_no),
        include_directory: object.get("include_directory")?.as_bool()?,
        root: parse_memory_directory_node(object.get("root")?, true, version_no)?,
    })
}

fn parse_memory_directory_scope(value: &str) -> Option<contracts::MemoryDirectoryNodeScopeView> {
    match value {
        "dataset" => Some(contracts::MemoryDirectoryNodeScopeView::Dataset),
        _ => None,
    }
}

fn parse_memory_directory_node(
    value: &Value,
    is_root: bool,
    root_version_no: i32,
) -> Option<contracts::MemoryDirectoryNodeView> {
    let object = value.as_object()?;
    let kind = match object.get("kind")?.as_str()? {
        "dataset" => contracts::MemoryDirectoryNodeKind::Dataset,
        "document" => contracts::MemoryDirectoryNodeKind::Document,
        _ => return None,
    };

    let document = match kind {
        contracts::MemoryDirectoryNodeKind::Dataset => None,
        contracts::MemoryDirectoryNodeKind::Document => {
            Some(contracts::MemoryDirectoryDocumentView {
                id: DocumentId::from(Uuid::parse_str(object.get("document_id")?.as_str()?).ok()?),
                lifecycle: contracts::DocumentLifecycleView::from_str(
                    object.get("lifecycle")?.as_str()?,
                )?,
                chunk_count: object.get("chunk_count")?.as_u64()? as usize,
            })
        }
    };

    let scope = object
        .get("scope")
        .and_then(Value::as_str)
        .and_then(parse_memory_directory_scope)
        .or_else(|| {
            if is_root {
                Some(contracts::MemoryDirectoryNodeScopeView::Dataset)
            } else {
                None
            }
        });
    let version_no = object
        .get("version_no")
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
        .or_else(|| if is_root { Some(root_version_no) } else { None });
    let children = object
        .get("children")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|child| parse_memory_directory_node(child, false, root_version_no))
        .collect::<Option<Vec<_>>>()?;

    Some(contracts::MemoryDirectoryNodeView {
        kind,
        title: object.get("title")?.as_str()?.to_string(),
        scope,
        version_no,
        document,
        children,
    })
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use domain_model::{MemoryDirectoryId, TenantId, WorkflowExecutionId};
    use serde_json::json;

    use super::*;

    fn fixed_time() -> DateTime<Utc> {
        "2026-06-14T00:00:00Z"
            .parse()
            .expect("fixed timestamp should parse")
    }

    #[test]
    fn memory_directory_view_preserves_counts_manifest_and_tree_defaults() {
        let now = fixed_time();
        let dataset_id = DatasetId::new();
        let document_id = DocumentId::new();
        let execution_id = WorkflowExecutionId::new();
        let directory = MemoryDirectory {
            id: MemoryDirectoryId::new(),
            tenant_id: TenantId::new(),
            dataset_id,
            execution_id,
            owner_user_id: None,
            source_document_ids: vec![document_id],
            version_no: 5,
            directory_nodes: 2,
            refreshed_chunks: 12,
            directory_manifest: json!({
                "schema_version": "0.1.0",
                "generator": "memory-worker",
                "dataset_id": dataset_id,
                "include_directory": true,
                "root": {
                    "kind": "dataset",
                    "title": "Dataset Memory",
                    "children": [
                        {
                            "kind": "document",
                            "title": "Alpha",
                            "document_id": document_id,
                            "lifecycle": "indexed",
                            "chunk_count": 12,
                            "children": []
                        }
                    ]
                }
            }),
            created_at: now,
        };
        let id = directory.id;

        let view = to_memory_directory_view(directory);

        assert_eq!(view.id, id);
        assert_eq!(view.dataset_id, dataset_id);
        assert_eq!(view.execution_id, execution_id);
        assert_eq!(view.source_document_ids, vec![document_id]);
        assert_eq!(view.version_no, 5);
        assert_eq!(view.directory_nodes, 2);
        assert_eq!(view.refreshed_chunks, 12);
        assert_eq!(
            view.directory_manifest["root"]["title"],
            json!("Dataset Memory")
        );

        let tree = view
            .directory_tree
            .as_ref()
            .expect("valid manifest should hydrate directory tree");
        assert_eq!(tree.version_no, 5);
        assert_eq!(
            tree.root.scope,
            Some(contracts::MemoryDirectoryNodeScopeView::Dataset)
        );
        assert_eq!(tree.root.version_no, Some(5));
        let child_document = tree.root.children[0]
            .document
            .as_ref()
            .expect("document child should hydrate");
        assert_eq!(child_document.id, document_id);
        assert_eq!(child_document.chunk_count, 12);
        assert_eq!(
            child_document.lifecycle,
            contracts::DocumentLifecycleView::Indexed
        );
    }

    #[test]
    fn invalid_memory_directory_manifest_keeps_raw_manifest_without_tree() {
        let directory = MemoryDirectory {
            id: MemoryDirectoryId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            execution_id: WorkflowExecutionId::new(),
            owner_user_id: None,
            source_document_ids: Vec::new(),
            version_no: 2,
            directory_nodes: 0,
            refreshed_chunks: 0,
            directory_manifest: json!({
                "schema_version": "0.1.0",
                "generator": "memory-worker",
                "dataset_id": "not-a-uuid",
                "include_directory": true,
                "root": {"kind": "dataset", "title": "Broken"}
            }),
            created_at: fixed_time(),
        };

        let view = to_memory_directory_view(directory);

        assert_eq!(view.directory_manifest["dataset_id"], json!("not-a-uuid"));
        assert!(view.directory_tree.is_none());
    }
}
