use domain_model::{DatasetId, DocumentId};

#[derive(Clone, Debug)]
pub struct IngestJob {
    pub dataset_id: DatasetId,
    pub document_id: DocumentId,
    pub content_type: String,
}

#[derive(Clone, Debug)]
pub struct IngestOutcome {
    pub chunk_count: u32,
    pub inferred_title: Option<String>,
}

pub trait IngestProcessor {
    fn process(&self, job: &IngestJob) -> IngestOutcome;
}

#[derive(Clone, Debug, Default)]
pub struct PlaceholderIngestProcessor;

impl IngestProcessor for PlaceholderIngestProcessor {
    fn process(&self, job: &IngestJob) -> IngestOutcome {
        let chunk_count = if job.content_type.contains("pdf") {
            12
        } else {
            4
        };

        IngestOutcome {
            chunk_count,
            inferred_title: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_ingest_processor_returns_stable_chunk_counts() {
        let processor = PlaceholderIngestProcessor;
        let pdf_job = IngestJob {
            dataset_id: DatasetId::new(),
            document_id: DocumentId::new(),
            content_type: "application/pdf".to_string(),
        };
        let markdown_job = IngestJob {
            dataset_id: DatasetId::new(),
            document_id: DocumentId::new(),
            content_type: "text/markdown".to_string(),
        };

        assert_eq!(processor.process(&pdf_job).chunk_count, 12);
        assert_eq!(processor.process(&markdown_job).chunk_count, 4);
    }
}
