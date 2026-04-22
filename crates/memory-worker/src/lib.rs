use domain_model::{DatasetId, DocumentId};
use serde_json::{json, Value};

#[derive(Clone, Debug)]
pub struct MemorySourceDocument {
    pub document_id: DocumentId,
    pub title: String,
    pub lifecycle: String,
    pub chunk_count: usize,
}

#[derive(Clone, Debug)]
pub struct MemoryRefreshJob {
    pub dataset_id: DatasetId,
    pub include_directory: bool,
    pub documents: Vec<MemorySourceDocument>,
}

#[derive(Clone, Debug)]
pub struct MemoryRefreshOutcome {
    pub directory_nodes: usize,
    pub refreshed_chunks: usize,
    pub directory_manifest: Value,
}

pub trait MemoryIndexer {
    fn refresh(&self, job: &MemoryRefreshJob) -> MemoryRefreshOutcome;
}

#[derive(Clone, Debug)]
pub struct PlaceholderMemoryIndexer;

impl MemoryIndexer for PlaceholderMemoryIndexer {
    fn refresh(&self, job: &MemoryRefreshJob) -> MemoryRefreshOutcome {
        let refreshed_chunks = job
            .documents
            .iter()
            .map(|document| document.chunk_count)
            .sum();
        let root_children: Vec<_> = job
            .documents
            .iter()
            .map(|document| {
                json!({
                    "kind": "document",
                    "document_id": document.document_id,
                    "title": document.title,
                    "lifecycle": document.lifecycle,
                    "chunk_count": document.chunk_count,
                })
            })
            .collect();
        let directory_nodes = if job.include_directory {
            root_children.len() + 1
        } else {
            root_children.len()
        };

        MemoryRefreshOutcome {
            directory_nodes,
            refreshed_chunks,
            directory_manifest: json!({
                "schema_version": "0.1.0",
                "generator": "memory-worker",
                "dataset_id": job.dataset_id,
                "include_directory": job.include_directory,
                "root": {
                    "kind": "dataset",
                    "scope": "dataset",
                    "title": "Dataset Memory Directory",
                    "children": root_children,
                }
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_memory_indexer_builds_directory_manifest() {
        let dataset_id = DatasetId::new();
        let indexer = PlaceholderMemoryIndexer;
        let outcome = indexer.refresh(&MemoryRefreshJob {
            dataset_id,
            include_directory: true,
            documents: vec![
                MemorySourceDocument {
                    document_id: DocumentId::new(),
                    title: "Alpha".to_string(),
                    lifecycle: "indexed".to_string(),
                    chunk_count: 4,
                },
                MemorySourceDocument {
                    document_id: DocumentId::new(),
                    title: "Beta".to_string(),
                    lifecycle: "indexed".to_string(),
                    chunk_count: 6,
                },
            ],
        });

        assert_eq!(outcome.directory_nodes, 3);
        assert_eq!(outcome.refreshed_chunks, 10);
        assert_eq!(
            outcome.directory_manifest["root"]["scope"],
            json!("dataset")
        );
        assert_eq!(
            outcome.directory_manifest["root"]["children"][0]["title"],
            json!("Alpha")
        );
    }
}
