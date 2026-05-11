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
pub const DEFAULT_GENERATED_ARTIFACTS_DIR_NAME: &str = "generated_artifacts";
pub const DEFAULT_TRANSCRIPT_ARTIFACT_FILE_NAME: &str = "transcript.txt";
pub const DEFAULT_PPT_OUTLINE_ARTIFACT_FILE_NAME: &str = "ppt_outline.md";
pub const DEFAULT_TIMESTAMP_MAP_ARTIFACT_FILE_NAME: &str = "timestamp_map.json";
pub const DEFAULT_EXTRACTION_ARTIFACTS_MANIFEST_FILE_NAME: &str =
    "extraction_artifacts_manifest.json";
pub const DEFAULT_SLIDE_CANDIDATES_FILE_NAME: &str = "slide_candidates_manifest.json";
pub const DEFAULT_CONTACT_SHEET_PLAN_FILE_NAME: &str = "contact_sheet_plan.json";

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
    extract_video_ppt_output_with_artifacts(
        document,
        chunks,
        frame_extraction,
        video_generated_artifacts_plan(document),
    )
}

pub fn extract_video_ppt_output_with_artifacts(
    document: &Document,
    chunks: &[DocumentChunk],
    frame_extraction: Value,
    generated_artifacts: Value,
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
        "generated_artifacts": generated_artifacts,
        "artifacts": artifacts,
        "html_artifacts": [],
        "no_host_composed_answer": true,
        "note": "media-worker summarizes persisted media evidence, records raw_frames extraction state, and writes deterministic text artifacts when evidence exists; durable PPTX artifacts remain later stages.",
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

pub fn write_video_extraction_text_artifacts_if_available(
    document: &Document,
    chunks: &[DocumentChunk],
    frame_extraction: &Value,
    output_root: &Path,
) -> Value {
    match write_video_extraction_text_artifacts(document, chunks, frame_extraction, output_root) {
        Ok(manifest) => manifest,
        Err(error) => json!({
            "status": "failed",
            "source": "media_worker_text_artifact_writer",
            "reason": error,
            "session_dir": output_root.join(format!("video-extraction-{}", document.id)).display().to_string(),
            "artifacts_dir_name": DEFAULT_GENERATED_ARTIFACTS_DIR_NAME,
        }),
    }
}

pub fn write_video_extraction_text_artifacts(
    document: &Document,
    chunks: &[DocumentChunk],
    frame_extraction: &Value,
    output_root: &Path,
) -> Result<Value, String> {
    let evidence = video_media_evidence_items_from_chunks(chunks);
    let frame_count = frame_extraction
        .get("frame_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if !evidence.has_any() && frame_count == 0 {
        return Ok(video_generated_artifacts_plan(document));
    }

    let session_dir = output_root.join(format!("video-extraction-{}", document.id));
    let artifacts_dir = session_dir.join(DEFAULT_GENERATED_ARTIFACTS_DIR_NAME);
    fs::create_dir_all(&artifacts_dir).map_err(|error| error.to_string())?;

    let mut files = Vec::<Value>::new();
    if !evidence.transcript_segments.is_empty() {
        let transcript_path = artifacts_dir.join(DEFAULT_TRANSCRIPT_ARTIFACT_FILE_NAME);
        fs::write(
            &transcript_path,
            render_video_transcript_text(&evidence.transcript_segments),
        )
        .map_err(|error| error.to_string())?;
        files.push(video_generated_artifact_file(
            document,
            "transcript_text",
            "text/plain",
            &transcript_path,
        ));
    }

    if !evidence.scenes.is_empty() || !evidence.keyframe_ocr_snippets.is_empty() || frame_count > 0
    {
        let outline_path = artifacts_dir.join(DEFAULT_PPT_OUTLINE_ARTIFACT_FILE_NAME);
        fs::write(
            &outline_path,
            render_video_ppt_outline_markdown(document, &evidence, frame_extraction),
        )
        .map_err(|error| error.to_string())?;
        files.push(video_generated_artifact_file(
            document,
            "ppt_outline",
            "text/markdown",
            &outline_path,
        ));
    }

    if let Some(candidate_files) =
        write_video_slide_candidate_review_files(document, frame_extraction, &artifacts_dir)?
    {
        files.extend(candidate_files);
    }

    let timestamp_map_path = artifacts_dir.join(DEFAULT_TIMESTAMP_MAP_ARTIFACT_FILE_NAME);
    let timestamp_map = json!({
        "document_id": document.id.to_string(),
        "dataset_id": document.dataset_id.to_string(),
        "title": document.title,
        "transcript_segments": evidence.transcript_segments.clone(),
        "scenes": evidence.scenes.clone(),
        "keyframe_ocr_snippets": evidence.keyframe_ocr_snippets.clone(),
        "frame_extraction": frame_extraction,
    });
    let timestamp_map_bytes =
        serde_json::to_vec_pretty(&timestamp_map).map_err(|error| error.to_string())?;
    fs::write(&timestamp_map_path, timestamp_map_bytes).map_err(|error| error.to_string())?;
    files.push(video_generated_artifact_file(
        document,
        "timestamp_map",
        "application/json",
        &timestamp_map_path,
    ));

    let manifest_path = artifacts_dir.join(DEFAULT_EXTRACTION_ARTIFACTS_MANIFEST_FILE_NAME);
    let manifest = json!({
        "status": "completed",
        "source": "media_worker_text_artifact_writer",
        "session_dir": session_dir.display().to_string(),
        "artifacts_dir": artifacts_dir.display().to_string(),
        "manifest_path": manifest_path.display().to_string(),
        "manifest_file_name": DEFAULT_EXTRACTION_ARTIFACTS_MANIFEST_FILE_NAME,
        "evidence_counts": {
            "transcript_segment_count": evidence.transcript_segments.len(),
            "scene_count": evidence.scenes.len(),
            "keyframe_ocr_snippet_count": evidence.keyframe_ocr_snippets.len(),
            "frame_count": frame_count,
        },
        "files": files,
    });
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?;
    fs::write(&manifest_path, manifest_bytes).map_err(|error| error.to_string())?;

    Ok(manifest)
}

fn write_video_slide_candidate_review_files(
    document: &Document,
    frame_extraction: &Value,
    artifacts_dir: &Path,
) -> Result<Option<Vec<Value>>, String> {
    let raw_frames_dir = frame_extraction
        .get("raw_frames_dir")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    let Some(raw_frames_dir) = raw_frames_dir else {
        return Ok(None);
    };
    if !raw_frames_dir.is_dir() {
        return Ok(None);
    }

    let frames = sorted_raw_frame_files(&raw_frames_dir)?;
    if frames.is_empty() {
        return Ok(None);
    }

    let candidates = frames
        .iter()
        .enumerate()
        .map(|(index, path)| {
            json!({
                "candidate_index": index + 1,
                "source": "raw_frames",
                "file_name": path.file_name().and_then(|value| value.to_str()).unwrap_or("frame"),
                "frame_path": path.display().to_string(),
                "selection_status": "review_required",
                "notes": "Candidate frame retained conservatively; rectangle extraction/dedupe/manual keep-list are later stages.",
            })
        })
        .collect::<Vec<_>>();
    let candidate_manifest_path = artifacts_dir.join(DEFAULT_SLIDE_CANDIDATES_FILE_NAME);
    let candidate_manifest = json!({
        "status": "review_required",
        "source": "raw_frames",
        "document_id": document.id.to_string(),
        "dataset_id": document.dataset_id.to_string(),
        "title": document.title,
        "raw_frames_dir": raw_frames_dir.display().to_string(),
        "candidate_count": candidates.len(),
        "dedupe_policy": "conservative_keep_all_until_review",
        "candidates": candidates,
    });
    fs::write(
        &candidate_manifest_path,
        serde_json::to_vec_pretty(&candidate_manifest).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;

    let contact_sheet_plan_path = artifacts_dir.join(DEFAULT_CONTACT_SHEET_PLAN_FILE_NAME);
    let contact_sheet_plan = json!({
        "status": "planned",
        "source": "raw_frames",
        "raw_frames_dir": raw_frames_dir.display().to_string(),
        "candidate_manifest": candidate_manifest_path.display().to_string(),
        "recommended_output": artifacts_dir.join("raw_contact_sheet.jpg").display().to_string(),
        "review_rule": "build a numbered contact sheet before rectangle extraction; keep user/model selected slide numbers only",
        "skill_reference": "wechat-video-ppt-extract/contact-sheet --source raw_frames",
    });
    fs::write(
        &contact_sheet_plan_path,
        serde_json::to_vec_pretty(&contact_sheet_plan).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;

    Ok(Some(vec![
        video_generated_artifact_file(
            document,
            "slide_image_candidates",
            "application/json",
            &candidate_manifest_path,
        ),
        video_generated_artifact_file(
            document,
            "contact_sheet_plan",
            "application/json",
            &contact_sheet_plan_path,
        ),
    ]))
}

fn sorted_raw_frame_files(raw_frames_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut frames = fs::read_dir(raw_frames_dir)
        .map_err(|error| error.to_string())?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| {
            path.extension()
                .and_then(|value| value.to_str())
                .map(|extension| {
                    matches!(
                        extension.to_ascii_lowercase().as_str(),
                        "jpg" | "jpeg" | "png" | "webp"
                    )
                })
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    frames.sort_by(|left, right| {
        left.file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .cmp(
                right
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or_default(),
            )
    });
    Ok(frames)
}

pub fn video_generated_artifacts_plan(document: &Document) -> Value {
    json!({
        "status": "planned",
        "source": "media_worker_text_artifact_writer",
        "session_dir": format!("video-extraction-{}", document.id),
        "artifacts_dir": format!("video-extraction-{}/{}", document.id, DEFAULT_GENERATED_ARTIFACTS_DIR_NAME),
        "manifest_file_name": DEFAULT_EXTRACTION_ARTIFACTS_MANIFEST_FILE_NAME,
        "files": [],
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
            "generated_artifacts": output.get("generated_artifacts").cloned().unwrap_or_else(|| json!({})),
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

#[derive(Clone, Debug, Default)]
struct VideoMediaEvidenceItems {
    transcript_segments: Vec<Value>,
    scenes: Vec<Value>,
    keyframe_ocr_snippets: Vec<Value>,
}

impl VideoMediaEvidenceItems {
    fn has_any(&self) -> bool {
        !self.transcript_segments.is_empty()
            || !self.scenes.is_empty()
            || !self.keyframe_ocr_snippets.is_empty()
    }
}

fn video_media_evidence_items_from_chunks(chunks: &[DocumentChunk]) -> VideoMediaEvidenceItems {
    let mut evidence = VideoMediaEvidenceItems::default();
    for chunk in chunks {
        let Some(media) = media_value_from_chunk(chunk) else {
            continue;
        };
        evidence
            .transcript_segments
            .extend(cloned_array_items(media, "transcript_segments"));
        evidence.scenes.extend(cloned_array_items(media, "scenes"));
        evidence
            .keyframe_ocr_snippets
            .extend(cloned_array_items(media, "keyframe_ocr_snippets"));
    }
    evidence
}

fn media_value_from_chunk(chunk: &DocumentChunk) -> Option<&Value> {
    chunk
        .metadata
        .get("parse_metadata")
        .and_then(|value| value.get("media"))
        .or_else(|| chunk.metadata.get("media"))
}

fn cloned_array_items(value: &Value, key: &str) -> Vec<Value> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| items.iter().cloned().collect())
        .unwrap_or_default()
}

fn render_video_transcript_text(segments: &[Value]) -> String {
    let mut output = String::new();
    for (index, segment) in segments.iter().enumerate() {
        let text = video_item_text(segment, &["text", "content", "summary"]).unwrap_or_default();
        let range = video_time_range_label(segment);
        if !range.is_empty() {
            output.push_str(&format!("[{range}] {text}\n"));
        } else {
            output.push_str(&format!("{}: {text}\n", index + 1));
        }
    }
    output
}

fn render_video_ppt_outline_markdown(
    document: &Document,
    evidence: &VideoMediaEvidenceItems,
    frame_extraction: &Value,
) -> String {
    let mut output = format!("# {}\n\n", document.title);
    let frame_count = frame_extraction
        .get("frame_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    output.push_str("## Extraction Summary\n\n");
    output.push_str(&format!(
        "- Transcript segments: {}\n- Scenes: {}\n- Keyframe OCR snippets: {}\n- Raw frames: {}\n\n",
        evidence.transcript_segments.len(),
        evidence.scenes.len(),
        evidence.keyframe_ocr_snippets.len(),
        frame_count
    ));

    output.push_str("## Slide / Scene Candidates\n\n");
    if evidence.scenes.is_empty() && evidence.keyframe_ocr_snippets.is_empty() {
        output.push_str("- No scene or OCR candidates yet. Use raw frames/contact sheet before final PPTX generation.\n\n");
    } else {
        for (index, scene) in evidence.scenes.iter().enumerate() {
            let range = video_time_range_label(scene);
            let summary = video_item_text(scene, &["summary", "text", "description"])
                .unwrap_or_else(|| "Untitled scene".to_string());
            output.push_str(&format!(
                "- Scene {}{}: {}\n",
                index + 1,
                optional_time_suffix(&range),
                summary
            ));
        }
        for (index, snippet) in evidence.keyframe_ocr_snippets.iter().enumerate() {
            let timestamp = video_timestamp_label(snippet);
            let text = video_item_text(snippet, &["text", "ocr_text", "summary"])
                .unwrap_or_else(|| "No OCR text".to_string());
            output.push_str(&format!(
                "- OCR {}{}: {}\n",
                index + 1,
                optional_time_suffix(&timestamp),
                text
            ));
        }
        output.push('\n');
    }

    output.push_str("## Transcript\n\n");
    if evidence.transcript_segments.is_empty() {
        output.push_str("No transcript segments yet.\n");
    } else {
        for segment in &evidence.transcript_segments {
            let text =
                video_item_text(segment, &["text", "content", "summary"]).unwrap_or_default();
            let range = video_time_range_label(segment);
            output.push_str(&format!("- {}{}\n", optional_time_prefix(&range), text));
        }
    }
    output
}

fn video_generated_artifact_file(
    document: &Document,
    artifact_kind: &str,
    format: &str,
    path: &Path,
) -> Value {
    let artifact_id = format!("video-{}-{artifact_kind}", document.id);
    json!({
        "artifact_kind": artifact_kind,
        "artifact_id": artifact_id,
        "title": format!("{} - {}", document.title, artifact_kind),
        "format": format,
        "path": path.display().to_string(),
        "uri": format!("artifact://{artifact_id}"),
    })
}

fn video_item_text(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn video_time_range_label(value: &Value) -> String {
    let start = video_number_field(value, &["start_seconds", "startSeconds", "start"]);
    let end = video_number_field(value, &["end_seconds", "endSeconds", "end"]);
    match (start, end) {
        (Some(start), Some(end)) => format!("{}-{}", format_seconds(start), format_seconds(end)),
        (Some(start), None) => format_seconds(start),
        (None, Some(end)) => format_seconds(end),
        (None, None) => String::new(),
    }
}

fn video_timestamp_label(value: &Value) -> String {
    video_number_field(
        value,
        &[
            "timestamp_seconds",
            "timestampSeconds",
            "time_seconds",
            "timeSeconds",
        ],
    )
    .map(format_seconds)
    .unwrap_or_default()
}

fn video_number_field(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|key| value.get(*key)?.as_f64())
}

fn format_seconds(value: f64) -> String {
    let safe_value = value.max(0.0);
    let total_seconds = safe_value.round() as u64;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{minutes}:{seconds:02}")
}

fn optional_time_suffix(value: &str) -> String {
    if value.is_empty() {
        String::new()
    } else {
        format!(" ({value})")
    }
}

fn optional_time_prefix(value: &str) -> String {
    if value.is_empty() {
        String::new()
    } else {
        format!("[{value}] ")
    }
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
        assert_eq!(partial["generated_artifacts"]["status"], json!("planned"));
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
    fn writes_video_text_artifacts_from_media_evidence() {
        let document = test_document();
        let chunk = test_chunk(json!({
            "media": {
                "transcript_segments": [{
                    "start_seconds": 0.0,
                    "end_seconds": 2.4,
                    "text": "第一页讲产品定位"
                }],
                "scenes": [{
                    "start_seconds": 0.0,
                    "end_seconds": 2.4,
                    "summary": "标题页"
                }],
                "keyframe_ocr_snippets": [{
                    "timestamp_seconds": 1.2,
                    "text": "AI Data Platform"
                }]
            }
        }));
        let output_root = std::env::temp_dir().join(format!(
            "aidp-v3-video-artifacts-test-{}",
            DocumentId::new()
        ));
        let frame_extraction = json!({
            "status": "completed",
            "frame_count": 8,
            "manifest_file_name": DEFAULT_FRAME_MANIFEST_FILE_NAME
        });

        let manifest = write_video_extraction_text_artifacts(
            &document,
            &[chunk],
            &frame_extraction,
            &output_root,
        )
        .expect("text artifacts");

        assert_eq!(manifest["status"], json!("completed"));
        assert_eq!(
            manifest["evidence_counts"]["transcript_segment_count"],
            json!(1)
        );
        assert_eq!(manifest["evidence_counts"]["frame_count"], json!(8));
        assert_eq!(manifest["files"].as_array().expect("files").len(), 3);

        let transcript_path = manifest["files"]
            .as_array()
            .expect("files")
            .iter()
            .find(|file| file["artifact_kind"] == json!("transcript_text"))
            .and_then(|file| file["path"].as_str())
            .expect("transcript path");
        let transcript = fs::read_to_string(transcript_path).expect("transcript file");
        assert!(transcript.contains("第一页讲产品定位"));
    }

    #[test]
    fn writes_slide_candidate_review_files_from_raw_frames() {
        let document = test_document();
        let output_root = std::env::temp_dir().join(format!(
            "aidp-v3-video-candidates-test-{}",
            DocumentId::new()
        ));
        let raw_frames_dir = output_root
            .join(format!("video-extraction-{}", document.id))
            .join(DEFAULT_RAW_FRAMES_DIR_NAME);
        fs::create_dir_all(&raw_frames_dir).expect("raw frames dir");
        fs::write(raw_frames_dir.join("frame_000002.jpg"), b"fake").expect("frame 2");
        fs::write(raw_frames_dir.join("frame_000001.jpg"), b"fake").expect("frame 1");
        let frame_extraction = json!({
            "status": "completed",
            "raw_frames_dir": raw_frames_dir.display().to_string(),
            "frame_count": 2,
            "manifest_file_name": DEFAULT_FRAME_MANIFEST_FILE_NAME
        });

        let manifest =
            write_video_extraction_text_artifacts(&document, &[], &frame_extraction, &output_root)
                .expect("candidate artifacts");

        let files = manifest["files"].as_array().expect("files");
        assert!(files
            .iter()
            .any(|file| file["artifact_kind"] == json!("slide_image_candidates")));
        assert!(files
            .iter()
            .any(|file| file["artifact_kind"] == json!("contact_sheet_plan")));
        let candidates_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("slide_image_candidates"))
            .and_then(|file| file["path"].as_str())
            .expect("candidate manifest path");
        let candidates = fs::read_to_string(candidates_path).expect("candidate manifest");
        assert!(candidates.contains("frame_000001.jpg"));
        assert!(candidates.contains("review_required"));
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
