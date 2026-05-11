use domain_model::{Document, DocumentChunk, WorkflowTask};
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaWorkflowTaskKind {
    ResolveVideoSource,
    RegisterVideoAsset,
    ExtractVideoPpt,
}

impl MediaWorkflowTaskKind {
    pub fn from_task_key(value: &str) -> Option<Self> {
        match value {
            "resolve_video_source" => Some(Self::ResolveVideoSource),
            "register_video_asset" => Some(Self::RegisterVideoAsset),
            "extract_video_ppt" => Some(Self::ExtractVideoPpt),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::ResolveVideoSource => "resolve_video_source",
            Self::RegisterVideoAsset => "register_video_asset",
            Self::ExtractVideoPpt => "extract_video_ppt",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VideoExtractionEvidenceSummary {
    pub transcript_segment_count: usize,
    pub scene_count: usize,
    pub keyframe_ocr_snippet_count: usize,
    pub chunk_count: usize,
}

impl VideoExtractionEvidenceSummary {
    pub fn has_evidence(&self) -> bool {
        self.transcript_segment_count > 0
            || self.scene_count > 0
            || self.keyframe_ocr_snippet_count > 0
    }
}

pub fn resolve_video_source_output(task: &WorkflowTask, context: &Value) -> Value {
    json!({
        "status": "resolved",
        "task_key": task.task_key,
        "source": context.get("source").cloned().unwrap_or(Value::Null),
        "document_id": context.get("document_id").cloned().unwrap_or(Value::Null),
        "dataset_id": context.get("dataset_id").cloned().unwrap_or(Value::Null),
        "asset_state": "source_resolved",
    })
}

pub fn register_video_asset_output(document: &Document) -> Value {
    json!({
        "status": "registered",
        "document_id": document.id.to_string(),
        "dataset_id": document.dataset_id.to_string(),
        "title": document.title,
        "content_type": document.content_type,
        "asset_state": "registered",
    })
}

pub fn extract_video_ppt_output(document: &Document, chunks: &[DocumentChunk]) -> Value {
    let evidence = video_evidence_summary_from_chunks(chunks);
    let artifacts = video_extraction_artifact_refs(document, &evidence);
    let status = if evidence.has_evidence() {
        "completed"
    } else {
        "partial"
    };

    json!({
        "status": status,
        "document_id": document.id.to_string(),
        "dataset_id": document.dataset_id.to_string(),
        "title": document.title,
        "content_type": document.content_type,
        "evidence_summary": {
            "transcript_segment_count": evidence.transcript_segment_count,
            "scene_count": evidence.scene_count,
            "keyframe_ocr_snippet_count": evidence.keyframe_ocr_snippet_count,
            "chunk_count": evidence.chunk_count,
        },
        "artifacts": artifacts,
        "html_artifacts": [],
        "no_host_composed_answer": true,
        "note": "media-worker only summarizes persisted media evidence in this slice; PPTX/Markdown durable artifact writing remains a later stage.",
    })
}

pub fn video_evidence_summary_from_chunks(
    chunks: &[DocumentChunk],
) -> VideoExtractionEvidenceSummary {
    let mut summary = VideoExtractionEvidenceSummary {
        chunk_count: chunks.len(),
        ..VideoExtractionEvidenceSummary::default()
    };

    for chunk in chunks {
        let media = chunk
            .metadata
            .get("parse_metadata")
            .and_then(|value| value.get("media"))
            .or_else(|| chunk.metadata.get("media"));
        if let Some(media) = media {
            summary.transcript_segment_count += array_len(media, "transcript_segments");
            summary.scene_count += array_len(media, "scenes");
            summary.keyframe_ocr_snippet_count += array_len(media, "keyframe_ocr_snippets");
        }
    }

    summary
}

fn video_extraction_artifact_refs(
    document: &Document,
    evidence: &VideoExtractionEvidenceSummary,
) -> Vec<Value> {
    let mut artifacts = Vec::new();
    if evidence.transcript_segment_count > 0 {
        artifacts.push(video_artifact_ref(
            document,
            "transcript_text",
            "text/plain",
        ));
    }
    if evidence.keyframe_ocr_snippet_count > 0 {
        artifacts.push(video_artifact_ref(
            document,
            "slide_image_candidates",
            "application/json",
        ));
    }
    if evidence.has_evidence() {
        artifacts.push(video_artifact_ref(
            document,
            "timestamp_map",
            "application/json",
        ));
        artifacts.push(video_artifact_ref(document, "html_summary", "text/html"));
    }
    artifacts
}

fn video_artifact_ref(document: &Document, artifact_kind: &str, format: &str) -> Value {
    let artifact_id = format!("video-{}-{artifact_kind}", document.id);
    json!({
        "artifact_kind": artifact_kind,
        "artifact_id": artifact_id,
        "title": format!("{} - {}", document.title, artifact_kind),
        "format": format,
        "uri": format!("artifact://{artifact_id}"),
    })
}

fn array_len(value: &Value, key: &str) -> usize {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{
        DatasetId, DocumentChunkId, DocumentChunkState, DocumentId, DocumentLifecycle, TenantId,
    };
    use serde_json::json;
    use std::collections::BTreeMap;

    fn test_document() -> Document {
        Document {
            id: DocumentId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            owner_user_id: None,
            title: "Test Video".to_string(),
            object_key: "training.mp4".to_string(),
            content_type: "video/mp4".to_string(),
            lifecycle: DocumentLifecycle::Extracted,
            secret_binding_ids: Vec::new(),
            metadata: BTreeMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn test_chunk(metadata: Value) -> DocumentChunk {
        DocumentChunk {
            id: DocumentChunkId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            document_id: DocumentId::new(),
            chunk_index: 0,
            content: "Video parse summary".to_string(),
            token_count: 6,
            state: DocumentChunkState::Extracted,
            metadata: BTreeMap::from_iter([("parse_metadata".to_string(), metadata)]),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn task_kind_accepts_video_workflow_task_keys() {
        assert_eq!(
            MediaWorkflowTaskKind::from_task_key("resolve_video_source"),
            Some(MediaWorkflowTaskKind::ResolveVideoSource)
        );
        assert_eq!(
            MediaWorkflowTaskKind::RegisterVideoAsset.as_str(),
            "register_video_asset"
        );
        assert_eq!(MediaWorkflowTaskKind::from_task_key("unknown"), None);
    }

    #[test]
    fn evidence_summary_counts_media_metadata() {
        let chunk = test_chunk(json!({
            "media": {
                "transcript_segments": [{"text": "page one"}],
                "scenes": [{"summary": "title page"}],
                "keyframe_ocr_snippets": [{"text": "AI data assistant"}]
            }
        }));

        let summary = video_evidence_summary_from_chunks(&[chunk]);

        assert_eq!(summary.transcript_segment_count, 1);
        assert_eq!(summary.scene_count, 1);
        assert_eq!(summary.keyframe_ocr_snippet_count, 1);
        assert!(summary.has_evidence());
    }

    #[test]
    fn extract_output_is_partial_without_evidence_and_completed_with_evidence() {
        let document = test_document();
        let partial = extract_video_ppt_output(&document, &[]);
        assert_eq!(partial["status"], json!("partial"));
        assert!(partial["artifacts"].as_array().expect("array").is_empty());

        let chunk = test_chunk(json!({
            "media": {
                "transcript_segments": [{"text": "page one"}],
                "scenes": [],
                "keyframe_ocr_snippets": [{"text": "title"}]
            }
        }));
        let completed = extract_video_ppt_output(&document, &[chunk]);

        assert_eq!(completed["status"], json!("completed"));
        assert_eq!(
            completed["evidence_summary"]["transcript_segment_count"],
            json!(1)
        );
        assert_eq!(
            completed["artifacts"][0]["artifact_kind"],
            json!("transcript_text")
        );
    }
}
