use chrono::Utc;
use domain_model::{Document, DocumentChunk, WorkflowTask};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

pub const DEFAULT_FRAME_EXTRACTION_INTERVAL_SECONDS: f64 = 0.15;
pub const DEFAULT_RAW_FRAMES_DIR_NAME: &str = "raw_frames";
pub const DEFAULT_RAW_FRAME_FILE_PATTERN: &str = "frame_%06d.jpg";
pub const DEFAULT_FRAME_MANIFEST_FILE_NAME: &str = "frame_manifest.json";

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
    extract_video_ppt_output_with_frame_extraction(
        document,
        chunks,
        video_frame_extraction_plan(document),
    )
}

pub fn extract_video_ppt_output_with_frame_extraction(
    document: &Document,
    chunks: &[DocumentChunk],
    frame_extraction: Value,
) -> Value {
    let evidence = video_evidence_summary_from_chunks(chunks);
    let artifacts = video_extraction_artifact_refs(document, &evidence, &frame_extraction);
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
        "frame_extraction": frame_extraction,
        "artifacts": artifacts,
        "html_artifacts": [],
        "no_host_composed_answer": true,
        "note": "media-worker summarizes persisted media evidence and records raw_frames extraction state; durable PPTX/Markdown artifacts remain later stages.",
    })
}

#[derive(Clone, Debug, PartialEq)]
pub struct FrameExtractionConfig {
    pub enabled: bool,
    pub ffmpeg_bin: String,
    pub output_root: PathBuf,
    pub interval_seconds: f64,
}

impl Default for FrameExtractionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            ffmpeg_bin: "ffmpeg".to_string(),
            output_root: std::env::temp_dir().join("aidp-v3-video-extraction"),
            interval_seconds: DEFAULT_FRAME_EXTRACTION_INTERVAL_SECONDS,
        }
    }
}

pub fn run_video_frame_extraction_if_enabled(
    document: &Document,
    config: &FrameExtractionConfig,
) -> Value {
    if !config.enabled {
        return video_frame_extraction_plan(document);
    }

    let Some(input_path) = resolve_local_media_input_path(document) else {
        let mut plan = video_frame_extraction_plan(document);
        if let Some(object) = plan.as_object_mut() {
            object.insert("status".to_string(), Value::String("skipped".to_string()));
            object.insert("enabled".to_string(), Value::Bool(true));
            object.insert(
                "reason".to_string(),
                Value::String("local_media_path_not_available".to_string()),
            );
        }
        return plan;
    };

    match run_video_frame_extraction(document, &input_path, config) {
        Ok(manifest) => manifest,
        Err(error) => json!({
            "status": "failed",
            "source": "ffmpeg_external_process",
            "enabled": true,
            "reason": error,
            "input_path": input_path.display().to_string(),
            "sop": "wechat-video-ppt-extract/raw_frames",
        }),
    }
}

pub fn run_video_frame_extraction(
    document: &Document,
    input_path: &Path,
    config: &FrameExtractionConfig,
) -> Result<Value, String> {
    if !input_path.is_file() {
        return Err("local_media_path_not_found".to_string());
    }
    if config.interval_seconds <= 0.0 {
        return Err("invalid_frame_interval".to_string());
    }

    let session_dir = config
        .output_root
        .join(format!("video-extraction-{}", document.id));
    let raw_frames_dir = session_dir.join(DEFAULT_RAW_FRAMES_DIR_NAME);
    fs::create_dir_all(&raw_frames_dir).map_err(|error| error.to_string())?;
    let output_pattern = raw_frames_dir.join(DEFAULT_RAW_FRAME_FILE_PATTERN);
    let fps_filter = format!("fps=1/{}", config.interval_seconds);
    let output = Command::new(&config.ffmpeg_bin)
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-i")
        .arg(input_path)
        .arg("-vf")
        .arg(&fps_filter)
        .arg(output_pattern.as_os_str())
        .output()
        .map_err(|error| error.to_string())?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr)
            .chars()
            .take(1_000)
            .collect::<String>();
        return Err(if stderr.trim().is_empty() {
            format!("ffmpeg exited with {}", output.status)
        } else {
            stderr
        });
    }

    let frame_count = fs::read_dir(&raw_frames_dir)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .count();

    let manifest_path = session_dir.join(DEFAULT_FRAME_MANIFEST_FILE_NAME);
    let manifest = json!({
        "status": "completed",
        "source": "ffmpeg_external_process",
        "enabled": true,
        "input_path": input_path.display().to_string(),
        "session_dir": session_dir.display().to_string(),
        "raw_frames_dir": raw_frames_dir.display().to_string(),
        "manifest_path": manifest_path.display().to_string(),
        "manifest_file_name": DEFAULT_FRAME_MANIFEST_FILE_NAME,
        "frame_file_pattern": DEFAULT_RAW_FRAME_FILE_PATTERN,
        "interval_seconds": config.interval_seconds,
        "save_all": true,
        "contact_sheet_source": DEFAULT_RAW_FRAMES_DIR_NAME,
        "frame_count": frame_count,
        "sop": "wechat-video-ppt-extract/raw_frames",
    });
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?;
    fs::write(&manifest_path, manifest_bytes).map_err(|error| error.to_string())?;

    Ok(manifest)
}

pub fn video_frame_extraction_plan(document: &Document) -> Value {
    json!({
        "status": "planned",
        "source": "ffmpeg_external_process",
        "enabled": false,
        "input_required": "local_media_path",
        "session_dir": format!("video-extraction-{}", document.id),
        "raw_frames_dir": format!("video-extraction-{}/{}", document.id, DEFAULT_RAW_FRAMES_DIR_NAME),
        "manifest_file_name": DEFAULT_FRAME_MANIFEST_FILE_NAME,
        "frame_file_pattern": DEFAULT_RAW_FRAME_FILE_PATTERN,
        "interval_seconds": DEFAULT_FRAME_EXTRACTION_INTERVAL_SECONDS,
        "save_all": true,
        "contact_sheet_source": DEFAULT_RAW_FRAMES_DIR_NAME,
        "sop": "wechat-video-ppt-extract/raw_frames",
    })
}

pub fn video_extraction_html_artifact_from_output(
    assistant_run_id: &str,
    local_thread_id: Option<&str>,
    output: &Value,
) -> Option<Value> {
    let document_id = output.get("document_id").and_then(Value::as_str)?;
    let title = output
        .get("title")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("视频 PPT 提取");
    let status = output
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("partial");
    let evidence_summary = output
        .get("evidence_summary")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let frame_extraction = output
        .get("frame_extraction")
        .cloned()
        .unwrap_or_else(|| json!({ "status": "planned" }));
    let frame_status = frame_extraction
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("planned");
    let frame_count = frame_extraction
        .get("frame_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let evidence_status = if status == "completed" || frame_count > 0 {
        "available"
    } else {
        "missing"
    };
    let missing = video_extraction_missing_items(&evidence_summary, &frame_extraction);
    let local_thread_id = local_thread_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    Some(json!({
        "kind": "html_artifact",
        "version": 1,
        "id": format!("html-artifact-video-extraction-{assistant_run_id}-{document_id}"),
        "title": format!("{title} - 视频提取摘要"),
        "source_type": "video_extraction",
        "template_id": "video_extraction_summary",
        "owner_scope": {
            "type": "assistant_run",
            "id": assistant_run_id,
        },
        "data_refs": [
            {
                "kind": "document",
                "id": document_id,
                "label": title,
            }
        ],
        "provenance": {
            "producer": "media-worker",
            "reason": "video_extraction_workflow_completed",
            "source_run_id": assistant_run_id,
        },
        "interaction_mode": "read_only",
        "created_at": Utc::now(),
        "payload": {
            "document": {
                "id": document_id,
                "dataset_id": output.get("dataset_id").cloned().unwrap_or(Value::Null),
                "title": title,
                "content_type": output.get("content_type").cloned().unwrap_or(Value::Null),
            },
            "media_kind": "video",
            "parse_status": status,
            "evidence_status": evidence_status,
            "summary": evidence_summary,
            "missing": missing,
            "provider_evidence": [
                {
                    "provider": "media-worker",
                    "capability": "ffmpeg_raw_frames",
                    "status": frame_status,
                    "supported": frame_status == "completed",
                    "detail": format!("raw_frames={frame_count}; manifest={}", frame_extraction.get("manifest_file_name").and_then(Value::as_str).unwrap_or(DEFAULT_FRAME_MANIFEST_FILE_NAME)),
                }
            ],
            "artifacts": output.get("artifacts").cloned().unwrap_or_else(|| json!([])),
            "local_thread_id": local_thread_id,
            "note": "后台视频抽取阶段已完成；该摘要只展示已实际产生或已明确缺失的证据，后续生成 PPT/Markdown 时继续沿用这些证据。"
        }
    }))
}

fn video_extraction_missing_items(
    evidence_summary: &Value,
    frame_extraction: &Value,
) -> Vec<&'static str> {
    let transcript_count = evidence_summary
        .get("transcript_segment_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let scene_count = evidence_summary
        .get("scene_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let ocr_count = evidence_summary
        .get("keyframe_ocr_snippet_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let frame_count = frame_extraction
        .get("frame_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let mut missing = Vec::new();
    if transcript_count == 0 {
        missing.push("transcript_text");
    }
    if scene_count == 0 {
        missing.push("scene_windows");
    }
    if ocr_count == 0 {
        missing.push("keyframe_ocr");
    }
    if frame_count == 0 {
        missing.push("raw_frames");
    }
    missing
}

pub fn frame_extraction_config_from_env() -> FrameExtractionConfig {
    let mut config = FrameExtractionConfig::default();
    config.enabled = env_flag("MEDIA_FRAME_EXTRACTION_ENABLED", false);
    if let Some(ffmpeg_bin) = optional_env("MEDIA_FFMPEG_BIN") {
        config.ffmpeg_bin = ffmpeg_bin;
    }
    if let Some(output_root) = optional_env("MEDIA_FRAME_OUTPUT_ROOT") {
        config.output_root = PathBuf::from(output_root);
    }
    if let Some(interval) = optional_env("MEDIA_FRAME_INTERVAL_SECONDS")
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| *value > 0.0)
    {
        config.interval_seconds = interval;
    }
    config
}

pub fn resolve_local_media_input_path(document: &Document) -> Option<PathBuf> {
    let raw = document.object_key.trim();
    if raw.starts_with("http://") || raw.starts_with("https://") {
        return None;
    }

    let raw = raw.trim_start_matches("file://");
    if raw.is_empty() {
        return None;
    }

    let direct = PathBuf::from(raw);
    if direct.is_file() {
        return Some(direct);
    }

    let root = std::env::var("PLATFORM_LOCAL_OBJECT_ROOT").ok()?;
    let rooted = Path::new(&root).join(raw);
    rooted.is_file().then_some(rooted)
}

fn env_flag(key: &str, default: bool) -> bool {
    std::env::var(key)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}

fn optional_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
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
    frame_extraction: &Value,
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
    if frame_extraction
        .get("manifest_path")
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty())
    {
        artifacts.push(video_artifact_ref(
            document,
            "frame_manifest",
            "application/json",
        ));
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
        assert_eq!(partial["frame_extraction"]["status"], json!("planned"));
        assert_eq!(
            partial["frame_extraction"]["raw_frames_dir"],
            json!(format!("video-extraction-{}/raw_frames", document.id))
        );
        assert_eq!(
            partial["frame_extraction"]["interval_seconds"],
            json!(DEFAULT_FRAME_EXTRACTION_INTERVAL_SECONDS)
        );
        assert_eq!(
            partial["frame_extraction"]["manifest_file_name"],
            json!(DEFAULT_FRAME_MANIFEST_FILE_NAME)
        );
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

    #[test]
    fn extract_output_includes_frame_manifest_artifact_when_manifest_exists() {
        let document = test_document();
        let frame_extraction = json!({
            "status": "completed",
            "manifest_path": "C:/tmp/video-extraction/frame_manifest.json",
            "raw_frames_dir": "C:/tmp/video-extraction/raw_frames",
            "frame_count": 12
        });

        let output =
            extract_video_ppt_output_with_frame_extraction(&document, &[], frame_extraction);

        let artifacts = output["artifacts"].as_array().expect("artifacts");
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0]["artifact_kind"], json!("frame_manifest"));
        assert_eq!(artifacts[0]["format"], json!("application/json"));
    }

    #[test]
    fn video_extraction_html_artifact_summarizes_background_output() {
        let document = test_document();
        let frame_extraction = json!({
            "status": "completed",
            "manifest_path": "C:/tmp/video-extraction/frame_manifest.json",
            "manifest_file_name": DEFAULT_FRAME_MANIFEST_FILE_NAME,
            "frame_count": 12
        });
        let output =
            extract_video_ppt_output_with_frame_extraction(&document, &[], frame_extraction);

        let artifact = video_extraction_html_artifact_from_output(
            "00000000-0000-0000-0000-000000000001",
            Some("local-thread-1"),
            &output,
        )
        .expect("artifact");

        assert_eq!(artifact["kind"], json!("html_artifact"));
        assert_eq!(artifact["source_type"], json!("video_extraction"));
        assert_eq!(artifact["template_id"], json!("video_extraction_summary"));
        assert_eq!(artifact["interaction_mode"], json!("read_only"));
        assert_eq!(
            artifact["payload"]["provider_evidence"][0]["capability"],
            json!("ffmpeg_raw_frames")
        );
        assert_eq!(
            artifact["payload"]["provider_evidence"][0]["supported"],
            json!(true)
        );
        assert_eq!(artifact["payload"]["missing"][0], json!("transcript_text"));
    }

    #[test]
    fn frame_extraction_is_planned_by_default_and_skips_remote_sources_when_enabled() {
        let mut document = test_document();
        let disabled = run_video_frame_extraction_if_enabled(
            &document,
            &FrameExtractionConfig {
                enabled: false,
                ..FrameExtractionConfig::default()
            },
        );

        assert_eq!(disabled["status"], json!("planned"));
        assert_eq!(disabled["enabled"], json!(false));

        document.object_key = "https://cdn.example.com/video.mp4".to_string();
        let skipped = run_video_frame_extraction_if_enabled(
            &document,
            &FrameExtractionConfig {
                enabled: true,
                ..FrameExtractionConfig::default()
            },
        );

        assert_eq!(skipped["status"], json!("skipped"));
        assert_eq!(skipped["enabled"], json!(true));
        assert_eq!(skipped["reason"], json!("local_media_path_not_available"));
    }

    #[test]
    fn frame_extraction_rejects_missing_local_input_without_invoking_ffmpeg() {
        let document = test_document();
        let config = FrameExtractionConfig {
            enabled: true,
            ..FrameExtractionConfig::default()
        };
        let missing =
            std::env::temp_dir().join(format!("aidp-v3-missing-video-{}.mp4", DocumentId::new()));

        let error = run_video_frame_extraction(&document, &missing, &config)
            .expect_err("missing input should be rejected before ffmpeg");

        assert_eq!(error, "local_media_path_not_found");
    }
}
