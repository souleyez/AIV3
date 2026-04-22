use domain_model::{DatasetId, DocumentId};

#[derive(Clone, Debug)]
pub struct RetrievalIndexJob {
    pub dataset_id: DatasetId,
    pub document_id: DocumentId,
    pub chunk_count: usize,
}

#[derive(Clone, Debug)]
pub struct RetrievalIndexOutcome {
    pub embedded_chunks: u32,
    pub payload_filter_key: String,
    pub embedding_model: String,
}

pub trait RetrievalIndexer {
    fn index(&self, job: &RetrievalIndexJob) -> RetrievalIndexOutcome;
}

#[derive(Clone, Debug)]
pub struct PlaceholderRetrievalIndexer;

impl RetrievalIndexer for PlaceholderRetrievalIndexer {
    fn index(&self, job: &RetrievalIndexJob) -> RetrievalIndexOutcome {
        RetrievalIndexOutcome {
            embedded_chunks: job.chunk_count as u32,
            payload_filter_key: format!("dataset/{}", job.dataset_id),
            embedding_model: "placeholder-minilm".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_retrieval_indexer_uses_chunk_count() {
        let dataset_id = DatasetId::new();
        let outcome = PlaceholderRetrievalIndexer.index(&RetrievalIndexJob {
            dataset_id,
            document_id: DocumentId::new(),
            chunk_count: 7,
        });

        assert_eq!(outcome.embedded_chunks, 7);
        assert_eq!(outcome.payload_filter_key, format!("dataset/{dataset_id}"));
        assert_eq!(outcome.embedding_model, "placeholder-minilm");
    }
}
