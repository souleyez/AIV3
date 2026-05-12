use chrono::Utc;
use domain_model::{Document, DocumentChunk, WorkflowTask};
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};
use zip::{write::SimpleFileOptions, ZipWriter};

pub const DEFAULT_FRAME_EXTRACTION_INTERVAL_SECONDS: f64 = 0.15;
pub const DEFAULT_RAW_FRAMES_DIR_NAME: &str = "raw_frames";
pub const DEFAULT_RAW_FRAME_FILE_PATTERN: &str = "frame_%06d.jpg";
pub const DEFAULT_FRAME_MANIFEST_FILE_NAME: &str = "frame_manifest.json";
pub const DEFAULT_GENERATED_ARTIFACTS_DIR_NAME: &str = "generated_artifacts";
pub const DEFAULT_TRANSCRIPT_ARTIFACT_FILE_NAME: &str = "transcript.txt";
pub const DEFAULT_SOURCE_TEXT_ARTIFACT_FILE_NAME: &str = "source_text.md";
pub const DEFAULT_PPT_OUTLINE_ARTIFACT_FILE_NAME: &str = "ppt_outline.md";
pub const DEFAULT_TIMESTAMP_MAP_ARTIFACT_FILE_NAME: &str = "timestamp_map.json";
pub const DEFAULT_EXTRACTION_ARTIFACTS_MANIFEST_FILE_NAME: &str =
    "extraction_artifacts_manifest.json";
pub const DEFAULT_FINAL_DELIVERABLES_MANIFEST_FILE_NAME: &str = "final_deliverables_manifest.json";
pub const DEFAULT_SLIDE_CANDIDATES_FILE_NAME: &str = "slide_candidates_manifest.json";
pub const DEFAULT_CONTACT_SHEET_PLAN_FILE_NAME: &str = "contact_sheet_plan.json";
pub const DEFAULT_CONTACT_SHEET_HTML_FILE_NAME: &str = "raw_contact_sheet.html";
pub const DEFAULT_PPT_KEEP_LIST_TEMPLATE_FILE_NAME: &str = "ppt_keep_list_template.json";
pub const DEFAULT_SELECTED_SLIDES_MANIFEST_FILE_NAME: &str = "selected_slides_manifest.json";
pub const DEFAULT_SUBTITLE_PAGE_MAP_FILE_NAME: &str = "subtitle_page_map.json";
pub const DEFAULT_SLIDE_NOTES_ARTIFACT_FILE_NAME: &str = "slide_notes.md";
pub const DEFAULT_PPTX_BUILD_PLAN_FILE_NAME: &str = "pptx_build_plan.json";
pub const DEFAULT_VIDEO_SLIDES_PPTX_FILE_NAME: &str = "video_slides_screenshot_based.pptx";

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
    let artifacts = merged_video_extraction_artifact_refs(
        document,
        &evidence,
        &frame_extraction,
        &generated_artifacts,
    );
    let deliverable_status =
        video_deliverable_status_with_frame_extraction(&generated_artifacts, &frame_extraction);
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
        "deliverable_status": deliverable_status,
        "artifacts": artifacts,
        "html_artifacts": [],
        "no_host_composed_answer": true,
        "note": "media-worker summarizes persisted media evidence, records raw_frames extraction state, writes deterministic text/review artifacts, and emits a basic screenshot PPTX when a confirmed keep-list exists.",
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
    if let Some(frame_manifest_path) = frame_extraction_manifest_path(frame_extraction) {
        files.push(video_generated_artifact_file(
            document,
            "frame_manifest",
            "application/json",
            &frame_manifest_path,
        ));
    }

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

    if evidence.has_any() || frame_count > 0 {
        let source_text_path = artifacts_dir.join(DEFAULT_SOURCE_TEXT_ARTIFACT_FILE_NAME);
        fs::write(
            &source_text_path,
            render_video_source_text_markdown(document, &evidence, frame_extraction),
        )
        .map_err(|error| error.to_string())?;
        files.push(video_generated_artifact_file(
            document,
            "source_text",
            "text/markdown",
            &source_text_path,
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

    if let Some(candidate_files) = write_video_slide_candidate_review_files(
        document,
        &evidence,
        frame_extraction,
        &artifacts_dir,
    )? {
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

    let final_deliverables_manifest_path =
        artifacts_dir.join(DEFAULT_FINAL_DELIVERABLES_MANIFEST_FILE_NAME);
    let final_deliverables_manifest =
        video_final_deliverables_manifest(document, &files, frame_count);
    let final_deliverables_manifest_bytes = serde_json::to_vec_pretty(&final_deliverables_manifest)
        .map_err(|error| error.to_string())?;
    fs::write(
        &final_deliverables_manifest_path,
        final_deliverables_manifest_bytes,
    )
    .map_err(|error| error.to_string())?;
    files.push(video_generated_artifact_file(
        document,
        "final_deliverables_manifest",
        "application/json",
        &final_deliverables_manifest_path,
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
    evidence: &VideoMediaEvidenceItems,
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
            let timestamp_seconds = video_candidate_timestamp_seconds(index + 1, frame_extraction);
            json!({
                "candidate_index": index + 1,
                "source": "raw_frames",
                "file_name": path.file_name().and_then(|value| value.to_str()).unwrap_or("frame"),
                "frame_path": path.display().to_string(),
                "timestamp_seconds": timestamp_seconds,
                "timestamp_label": format_seconds(timestamp_seconds),
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

    let contact_sheet_html_path = artifacts_dir.join(DEFAULT_CONTACT_SHEET_HTML_FILE_NAME);
    fs::write(
        &contact_sheet_html_path,
        render_raw_contact_sheet_html(document, &frames, artifacts_dir),
    )
    .map_err(|error| error.to_string())?;

    let contact_sheet_plan_path = artifacts_dir.join(DEFAULT_CONTACT_SHEET_PLAN_FILE_NAME);
    let contact_sheet_plan = json!({
        "status": "planned",
        "source": "raw_frames",
        "raw_frames_dir": raw_frames_dir.display().to_string(),
        "candidate_manifest": candidate_manifest_path.display().to_string(),
        "preview_html": contact_sheet_html_path.display().to_string(),
        "recommended_output": contact_sheet_html_path.display().to_string(),
        "review_rule": "build a numbered contact sheet before rectangle extraction; keep user/model selected slide numbers only",
        "skill_reference": "wechat-video-ppt-extract/contact-sheet --source raw_frames",
    });
    fs::write(
        &contact_sheet_plan_path,
        serde_json::to_vec_pretty(&contact_sheet_plan).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;

    let keep_list_template_path = artifacts_dir.join(DEFAULT_PPT_KEEP_LIST_TEMPLATE_FILE_NAME);
    let selected_candidate_indices =
        read_selected_candidate_indices_from_keep_list(&keep_list_template_path, candidates.len());
    let keep_list_template = json!({
        "status": "waiting_for_selection",
        "source": "slide_candidates_manifest",
        "document_id": document.id.to_string(),
        "dataset_id": document.dataset_id.to_string(),
        "title": document.title,
        "candidate_manifest": candidate_manifest_path.display().to_string(),
        "contact_sheet_plan": contact_sheet_plan_path.display().to_string(),
        "selected_candidate_indices": [],
        "rejected_candidate_indices": [],
        "selection_notes": [],
        "review_policy": {
            "requires_numbered_contact_sheet": true,
            "dedupe_policy": "conservative_manual_or_model_review",
            "do_not_auto_select_all_frames": true,
        },
    });
    if !keep_list_template_path.is_file() {
        fs::write(
            &keep_list_template_path,
            serde_json::to_vec_pretty(&keep_list_template).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
    }

    let selected_slides_manifest_path =
        artifacts_dir.join(DEFAULT_SELECTED_SLIDES_MANIFEST_FILE_NAME);
    let selected_slides_manifest = selected_slides_manifest_from_keep_list(
        document,
        &frames,
        &selected_candidate_indices,
        evidence,
        frame_extraction,
        &candidate_manifest_path,
        &contact_sheet_html_path,
        &keep_list_template_path,
    );
    fs::write(
        &selected_slides_manifest_path,
        serde_json::to_vec_pretty(&selected_slides_manifest).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;

    let subtitle_page_map = video_subtitle_page_map_from_selected_slides(&selected_slides_manifest);
    let subtitle_page_map_path = artifacts_dir.join(DEFAULT_SUBTITLE_PAGE_MAP_FILE_NAME);
    let has_subtitle_page_map =
        subtitle_page_map.get("status").and_then(Value::as_str) == Some("mapped");
    if has_subtitle_page_map {
        fs::write(
            &subtitle_page_map_path,
            serde_json::to_vec_pretty(&subtitle_page_map).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
    }

    let slide_notes_path = artifacts_dir.join(DEFAULT_SLIDE_NOTES_ARTIFACT_FILE_NAME);
    fs::write(
        &slide_notes_path,
        render_selected_slide_notes_markdown(document, &selected_slides_manifest),
    )
    .map_err(|error| error.to_string())?;

    let pptx_build_plan_path = artifacts_dir.join(DEFAULT_PPTX_BUILD_PLAN_FILE_NAME);
    let pptx_output_path = artifacts_dir.join(DEFAULT_VIDEO_SLIDES_PPTX_FILE_NAME);
    let has_selected_slides = !selected_candidate_indices.is_empty();
    if has_selected_slides {
        write_screenshot_based_pptx(document, &selected_slides_manifest, &pptx_output_path)?;
    }
    let pptx_build_plan = json!({
        "status": if has_selected_slides { "completed" } else { "waiting_for_keep_list" },
        "source": "slide_candidates_manifest",
        "document_id": document.id.to_string(),
        "dataset_id": document.dataset_id.to_string(),
        "title": document.title,
        "candidate_manifest": candidate_manifest_path.display().to_string(),
        "contact_sheet_plan": contact_sheet_plan_path.display().to_string(),
        "contact_sheet_html": contact_sheet_html_path.display().to_string(),
        "keep_list_template": keep_list_template_path.display().to_string(),
        "selected_slides_manifest": selected_slides_manifest_path.display().to_string(),
        "recommended_output": pptx_output_path.display().to_string(),
        "selection": {
            "mode": "manual_or_model_review_required",
            "selected_candidate_indices": selected_candidate_indices.clone(),
            "rule": "Do not build a final PPTX until ppt_keep_list_template.json is filled and confirmed from the numbered contact sheet.",
        },
        "speaker_notes": {
            "include_source_frame": true,
            "include_candidate_index": true,
            "include_timestamp_when_available": true,
            "include_transcript_pre_page_map": has_subtitle_page_map,
        },
        "build_policy": "one_raster_image_per_slide_after_keep_list",
        "skill_reference": "wechat-video-ppt-extract/build-selected",
    });
    fs::write(
        &pptx_build_plan_path,
        serde_json::to_vec_pretty(&pptx_build_plan).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;

    let mut selected_slides_artifact = video_generated_artifact_file(
        document,
        "selected_slides_manifest",
        "application/json",
        &selected_slides_manifest_path,
    );
    if let Some(object) = selected_slides_artifact.as_object_mut() {
        object.insert(
            "status".to_string(),
            selected_slides_manifest
                .get("status")
                .cloned()
                .unwrap_or_else(|| json!("waiting_for_selection")),
        );
        object.insert(
            "selected_count".to_string(),
            selected_slides_manifest
                .get("selected_count")
                .cloned()
                .unwrap_or_else(|| json!(0)),
        );
    }

    let mut artifact_files = vec![
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
        video_generated_artifact_file(
            document,
            "contact_sheet_html",
            "text/html",
            &contact_sheet_html_path,
        ),
        video_generated_artifact_file(
            document,
            "ppt_keep_list_template",
            "application/json",
            &keep_list_template_path,
        ),
        selected_slides_artifact,
        video_generated_artifact_file(document, "slide_notes", "text/markdown", &slide_notes_path),
        video_generated_artifact_file(
            document,
            "pptx_build_plan",
            "application/json",
            &pptx_build_plan_path,
        ),
    ];
    if has_subtitle_page_map {
        artifact_files.push(video_generated_artifact_file(
            document,
            "subtitle_page_map",
            "application/json",
            &subtitle_page_map_path,
        ));
    }
    if has_selected_slides {
        artifact_files.push(video_generated_artifact_file(
            document,
            "pptx",
            "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            &pptx_output_path,
        ));
    }

    Ok(Some(artifact_files))
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

fn render_raw_contact_sheet_html(
    document: &Document,
    frames: &[PathBuf],
    artifacts_dir: &Path,
) -> String {
    let mut output = String::new();
    output.push_str("<!doctype html>\n<html lang=\"zh-CN\">\n<head>\n");
    output.push_str("<meta charset=\"utf-8\">\n");
    output.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    output.push_str(&format!(
        "<title>{} - raw contact sheet</title>\n",
        html_escape_text(&document.title)
    ));
    output.push_str("<style>\n");
    output.push_str(":root{color-scheme:dark;background:#111319;color:#e7eaf0;font-family:ui-sans-serif,system-ui,-apple-system,BlinkMacSystemFont,\"Segoe UI\",sans-serif;}\n");
    output.push_str("body{margin:0;padding:24px;background:#111319;}\n");
    output.push_str("header{margin-bottom:18px;color:#c7ccd7;}\n");
    output.push_str("h1{margin:0 0 6px;font-size:18px;color:#f3f5f8;font-weight:650;}\n");
    output.push_str("p{margin:0;font-size:12px;line-height:1.55;}\n");
    output.push_str(
        ".grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(180px,1fr));gap:12px;}\n",
    );
    output.push_str("figure{margin:0;padding:10px;border-radius:14px;background:#191d26;}\n");
    output.push_str(
        "img{width:100%;height:auto;display:block;border-radius:10px;background:#0b0d12;}\n",
    );
    output.push_str("figcaption{margin-top:8px;font-size:12px;color:#c7ccd7;display:flex;gap:8px;justify-content:space-between;align-items:center;}\n");
    output.push_str(".index{color:#ffffff;font-weight:700;}\n");
    output.push_str(".name{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;}\n");
    output.push_str("</style>\n</head>\n<body>\n");
    output.push_str("<header>\n");
    output.push_str(&format!("<h1>{}</h1>\n", html_escape_text(&document.title)));
    output.push_str(&format!(
        "<p>候选帧接触表。请用编号填写 {}，不要默认全选。</p>\n",
        DEFAULT_PPT_KEEP_LIST_TEMPLATE_FILE_NAME
    ));
    output.push_str("</header>\n<main class=\"grid\">\n");

    for (index, frame) in frames.iter().enumerate() {
        let candidate_index = index + 1;
        let file_name = frame
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("frame");
        let src = contact_sheet_frame_src(frame, artifacts_dir);
        output.push_str(&format!(
            "<figure id=\"candidate-{candidate_index}\"><img src=\"{}\" alt=\"Candidate {candidate_index}: {}\"><figcaption><span class=\"index\">#{candidate_index}</span><span class=\"name\">{}</span></figcaption></figure>\n",
            html_escape_attr(&src),
            html_escape_attr(file_name),
            html_escape_text(file_name)
        ));
    }

    output.push_str("</main>\n</body>\n</html>\n");
    output
}

fn contact_sheet_frame_src(frame: &Path, artifacts_dir: &Path) -> String {
    let source = artifacts_dir
        .parent()
        .and_then(|session_dir| frame.strip_prefix(session_dir).ok())
        .map(|relative_to_session| PathBuf::from("..").join(relative_to_session))
        .unwrap_or_else(|| frame.to_path_buf());
    source.display().to_string().replace('\\', "/")
}

fn html_escape_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn html_escape_attr(value: &str) -> String {
    html_escape_text(value).replace('"', "&quot;")
}

fn read_selected_candidate_indices_from_keep_list(
    path: &Path,
    candidate_count: usize,
) -> Vec<usize> {
    let Some(value) = fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
    else {
        return Vec::new();
    };
    selected_candidate_indices_from_value(&value, candidate_count)
}

fn selected_candidate_indices_from_value(value: &Value, candidate_count: usize) -> Vec<usize> {
    let mut seen = BTreeSet::<usize>::new();
    value
        .get("selected_candidate_indices")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_u64)
        .filter_map(|raw| usize::try_from(raw).ok())
        .filter(|candidate_index| (1..=candidate_count).contains(candidate_index))
        .filter(|candidate_index| seen.insert(*candidate_index))
        .collect()
}

fn selected_slides_manifest_from_keep_list(
    document: &Document,
    frames: &[PathBuf],
    selected_candidate_indices: &[usize],
    evidence: &VideoMediaEvidenceItems,
    frame_extraction: &Value,
    candidate_manifest_path: &Path,
    contact_sheet_html_path: &Path,
    keep_list_template_path: &Path,
) -> Value {
    let mut previous_timestamp_seconds = 0.0_f64;
    let selected_candidates = selected_candidate_indices
        .iter()
        .enumerate()
        .filter_map(|candidate_index| {
            let slide_index = candidate_index.0;
            let candidate_index = candidate_index.1;
            let frame = frames.get(candidate_index.saturating_sub(1))?;
            let file_name = frame
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("frame");
            let timestamp_seconds =
                video_candidate_timestamp_seconds(*candidate_index, frame_extraction);
            let window_start_seconds = if slide_index == 0 {
                0.0
            } else {
                previous_timestamp_seconds.min(timestamp_seconds)
            };
            let window_end_seconds = if timestamp_seconds > window_start_seconds {
                timestamp_seconds
            } else {
                window_start_seconds + video_frame_interval_seconds(frame_extraction)
            };
            previous_timestamp_seconds = timestamp_seconds;
            let transcript_segments = video_transcript_segments_for_window(
                &evidence.transcript_segments,
                window_start_seconds,
                window_end_seconds,
            );
            let subtitle_alignment_status = if evidence.transcript_segments.is_empty() {
                "missing_transcript"
            } else if transcript_segments.is_empty() {
                "unmatched"
            } else {
                "pre_page_mapped"
            };
            Some(json!({
                "candidate_index": candidate_index,
                "file_name": file_name,
                "frame_path": frame.display().to_string(),
                "timestamp_seconds": timestamp_seconds,
                "timestamp_label": format_seconds(timestamp_seconds),
                "transcript_window": {
                    "start_seconds": window_start_seconds,
                    "end_seconds": window_end_seconds,
                    "assignment_rule": "pre_page_previous_to_current",
                },
                "subtitle_alignment_status": subtitle_alignment_status,
                "transcript_segments": transcript_segments,
                "contact_sheet_anchor": format!("candidate-{candidate_index}"),
                "selection_status": "selected",
            }))
        })
        .collect::<Vec<_>>();

    json!({
        "status": if selected_candidates.is_empty() { "waiting_for_selection" } else { "ready_for_pptx_writer" },
        "source": "ppt_keep_list_template",
        "document_id": document.id.to_string(),
        "dataset_id": document.dataset_id.to_string(),
        "title": document.title,
        "candidate_manifest": candidate_manifest_path.display().to_string(),
        "contact_sheet_html": contact_sheet_html_path.display().to_string(),
        "keep_list_template": keep_list_template_path.display().to_string(),
        "selected_candidate_indices": selected_candidate_indices,
        "selected_count": selected_candidates.len(),
        "selected_candidates": selected_candidates,
        "next_step": if selected_candidates.is_empty() {
            "fill ppt_keep_list_template.json from the numbered contact sheet"
        } else {
            "build screenshot-based PPTX from selected_candidates only"
        },
    })
}

fn video_subtitle_page_map_from_selected_slides(selected_slides_manifest: &Value) -> Value {
    let selected_candidates = selected_slides_manifest
        .get("selected_candidates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let pages = selected_candidates
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            let transcript_segments = candidate
                .get("transcript_segments")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if transcript_segments.is_empty() {
                return None;
            }
            Some(json!({
                "slide_number": index + 1,
                "candidate_index": candidate.get("candidate_index").cloned().unwrap_or(Value::Null),
                "source_frame": candidate.get("file_name").cloned().unwrap_or(Value::Null),
                "timestamp_seconds": candidate.get("timestamp_seconds").cloned().unwrap_or(Value::Null),
                "transcript_window": candidate.get("transcript_window").cloned().unwrap_or_else(|| json!({})),
                "assignment_rule": "pre_page_previous_to_current",
                "transcript_segments": transcript_segments,
            }))
        })
        .collect::<Vec<_>>();

    json!({
        "status": if pages.is_empty() { "unmapped" } else { "mapped" },
        "source": "selected_slides_manifest",
        "document_id": selected_slides_manifest.get("document_id").cloned().unwrap_or(Value::Null),
        "dataset_id": selected_slides_manifest.get("dataset_id").cloned().unwrap_or(Value::Null),
        "title": selected_slides_manifest.get("title").cloned().unwrap_or(Value::Null),
        "assignment_rule": "pre_page_previous_to_current",
        "page_count": pages.len(),
        "pages": pages,
    })
}

fn video_transcript_segments_for_window(
    transcript_segments: &[Value],
    start_seconds: f64,
    end_seconds: f64,
) -> Vec<Value> {
    transcript_segments
        .iter()
        .filter(|segment| {
            let midpoint = video_transcript_segment_midpoint_seconds(segment);
            midpoint >= start_seconds && midpoint <= end_seconds
        })
        .map(|segment| {
            json!({
                "start_seconds": video_number_field(segment, &["start_seconds", "startSeconds", "start"]),
                "end_seconds": video_number_field(segment, &["end_seconds", "endSeconds", "end"]),
                "text": video_item_text(segment, &["text", "content", "summary"]).unwrap_or_default(),
            })
        })
        .collect()
}

fn video_transcript_segment_midpoint_seconds(segment: &Value) -> f64 {
    let start = video_number_field(segment, &["start_seconds", "startSeconds", "start"]);
    let end = video_number_field(segment, &["end_seconds", "endSeconds", "end"]);
    match (start, end) {
        (Some(start), Some(end)) if end >= start => (start + end) / 2.0,
        (Some(start), _) => start,
        (_, Some(end)) => end,
        _ => 0.0,
    }
}

fn video_candidate_timestamp_seconds(candidate_index: usize, frame_extraction: &Value) -> f64 {
    candidate_index
        .saturating_sub(1)
        .to_string()
        .parse::<f64>()
        .unwrap_or(0.0)
        * video_frame_interval_seconds(frame_extraction)
}

fn video_frame_interval_seconds(frame_extraction: &Value) -> f64 {
    frame_extraction
        .get("interval_seconds")
        .and_then(Value::as_f64)
        .filter(|value| *value > 0.0)
        .unwrap_or(DEFAULT_FRAME_EXTRACTION_INTERVAL_SECONDS)
}

fn render_selected_slide_notes_markdown(
    document: &Document,
    selected_slides_manifest: &Value,
) -> String {
    let selected_candidates = selected_slides_manifest
        .get("selected_candidates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut output = format!("# Slide Notes: {}\n\n", document.title);
    output.push_str("This file is deterministic evidence for the screenshot-based PPTX. It contains only source frame/candidate metadata and review warnings; narration alignment is a later enhancement.\n\n");
    output.push_str("## Quality Warnings\n\n");
    let warnings = video_selected_slide_quality_warnings(selected_slides_manifest);
    for warning in warnings {
        output.push_str(&format!("- {warning}\n"));
    }
    output.push('\n');
    output.push_str("## Slides\n\n");
    if selected_candidates.is_empty() {
        output.push_str("- No selected slides yet. Fill `ppt_keep_list_template.json` from the numbered contact sheet before building the final PPTX.\n");
        return output;
    }
    for (index, candidate) in selected_candidates.iter().enumerate() {
        let slide_number = index + 1;
        let candidate_index = candidate
            .get("candidate_index")
            .and_then(Value::as_u64)
            .unwrap_or(slide_number as u64);
        let file_name = candidate
            .get("file_name")
            .and_then(Value::as_str)
            .unwrap_or("frame");
        let frame_path = candidate
            .get("frame_path")
            .and_then(Value::as_str)
            .unwrap_or("");
        let timestamp = candidate
            .get("timestamp_label")
            .and_then(Value::as_str)
            .unwrap_or("");
        output.push_str(&format!(
            "### Slide {slide_number}\n\n- Candidate: {candidate_index}\n- Source frame: {file_name}\n- Frame timestamp: {}\n- Frame path: {frame_path}\n",
            if timestamp.is_empty() { "unknown" } else { timestamp }
        ));
        let transcript_segments = candidate
            .get("transcript_segments")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if transcript_segments.is_empty() {
            output.push_str(&format!("- Speaker notes: Source frame metadata only for candidate {candidate_index}. Human/model review should align transcript/subtitles before customer delivery.\n\n"));
        } else {
            output.push_str("- Speaker notes: pre-page transcript assignment is available and still requires review.\n");
            output.push_str("- Aligned transcript:\n");
            for segment in transcript_segments {
                let text =
                    video_item_text(&segment, &["text", "content", "summary"]).unwrap_or_default();
                let range = video_time_range_label(&segment);
                output.push_str(&format!("  - {}{}\n", optional_time_prefix(&range), text));
            }
            output.push('\n');
        }
    }
    output
}

fn video_selected_slide_quality_warnings(selected_slides_manifest: &Value) -> Vec<String> {
    let selected_count = selected_slides_manifest
        .get("selected_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let mut warnings = Vec::new();
    if selected_count == 0 {
        warnings.push(
            "No selected slides yet; PPTX generation is blocked until keep-list review completes."
                .to_string(),
        );
    }
    let has_transcript_alignment = selected_slides_manifest
        .get("selected_candidates")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|candidate| {
            candidate
                .get("transcript_segments")
                .and_then(Value::as_array)
                .is_some_and(|segments| !segments.is_empty())
        });
    if has_transcript_alignment {
        warnings.push(
            "Speaker notes include deterministic pre-page transcript assignment and still require review."
                .to_string(),
        );
    } else {
        warnings.push(
            "Speaker notes currently contain source frame metadata only; transcript/subtitle alignment is not yet verified."
                .to_string(),
        );
    }
    warnings.push(
        "Slide images are raw frame screenshots until rectangle extraction/dedupe is promoted into the worker."
            .to_string(),
    );
    warnings
}

fn write_screenshot_based_pptx(
    document: &Document,
    selected_slides_manifest: &Value,
    output_path: &Path,
) -> Result<(), String> {
    let candidates = selected_slides_manifest
        .get("selected_candidates")
        .and_then(Value::as_array)
        .ok_or_else(|| "selected_candidates_missing".to_string())?;
    if candidates.is_empty() {
        return Err("selected_candidates_empty".to_string());
    }

    let file = File::create(output_path).map_err(|error| error.to_string())?;
    let mut writer = ZipWriter::new(file);
    write_pptx_text_entry(
        &mut writer,
        "[Content_Types].xml",
        &render_pptx_content_types(candidates),
    )?;
    write_pptx_text_entry(&mut writer, "_rels/.rels", &render_pptx_root_rels())?;
    write_pptx_text_entry(
        &mut writer,
        "docProps/app.xml",
        &render_pptx_app_props(candidates.len()),
    )?;
    write_pptx_text_entry(
        &mut writer,
        "docProps/core.xml",
        &render_pptx_core_props(document),
    )?;
    write_pptx_text_entry(
        &mut writer,
        "ppt/presentation.xml",
        &render_pptx_presentation(candidates.len()),
    )?;
    write_pptx_text_entry(
        &mut writer,
        "ppt/_rels/presentation.xml.rels",
        &render_pptx_presentation_rels(candidates.len()),
    )?;
    write_pptx_text_entry(
        &mut writer,
        "ppt/slideMasters/slideMaster1.xml",
        PPTX_SLIDE_MASTER_XML,
    )?;
    write_pptx_text_entry(
        &mut writer,
        "ppt/slideMasters/_rels/slideMaster1.xml.rels",
        PPTX_SLIDE_MASTER_RELS_XML,
    )?;
    write_pptx_text_entry(
        &mut writer,
        "ppt/slideLayouts/slideLayout1.xml",
        PPTX_SLIDE_LAYOUT_XML,
    )?;
    write_pptx_text_entry(
        &mut writer,
        "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
        PPTX_SLIDE_LAYOUT_RELS_XML,
    )?;
    write_pptx_text_entry(&mut writer, "ppt/theme/theme1.xml", PPTX_THEME_XML)?;
    write_pptx_text_entry(
        &mut writer,
        "ppt/notesMasters/notesMaster1.xml",
        PPTX_NOTES_MASTER_XML,
    )?;
    write_pptx_text_entry(
        &mut writer,
        "ppt/notesMasters/_rels/notesMaster1.xml.rels",
        PPTX_NOTES_MASTER_RELS_XML,
    )?;

    for (index, candidate) in candidates.iter().enumerate() {
        let slide_number = index + 1;
        let frame_path = candidate
            .get("frame_path")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| format!("selected_candidate_{slide_number}_frame_path_missing"))?;
        let extension = pptx_media_extension(&frame_path);
        write_pptx_text_entry(
            &mut writer,
            &format!("ppt/slides/slide{slide_number}.xml"),
            &render_pptx_slide(slide_number, candidate),
        )?;
        write_pptx_text_entry(
            &mut writer,
            &format!("ppt/slides/_rels/slide{slide_number}.xml.rels"),
            &render_pptx_slide_rels(slide_number, extension),
        )?;
        write_pptx_text_entry(
            &mut writer,
            &format!("ppt/notesSlides/notesSlide{slide_number}.xml"),
            &render_pptx_notes_slide(slide_number, candidate),
        )?;
        write_pptx_text_entry(
            &mut writer,
            &format!("ppt/notesSlides/_rels/notesSlide{slide_number}.xml.rels"),
            &render_pptx_notes_slide_rels(slide_number),
        )?;
        let image_bytes = fs::read(&frame_path).map_err(|error| error.to_string())?;
        write_pptx_binary_entry(
            &mut writer,
            &format!("ppt/media/image{slide_number}.{extension}"),
            &image_bytes,
        )?;
    }

    writer.finish().map_err(|error| error.to_string())?;
    Ok(())
}

fn write_pptx_text_entry(
    writer: &mut ZipWriter<File>,
    name: &str,
    content: &str,
) -> Result<(), String> {
    writer
        .start_file(name, SimpleFileOptions::default())
        .map_err(|error| error.to_string())?;
    writer
        .write_all(content.as_bytes())
        .map_err(|error| error.to_string())
}

fn write_pptx_binary_entry(
    writer: &mut ZipWriter<File>,
    name: &str,
    bytes: &[u8],
) -> Result<(), String> {
    writer
        .start_file(name, SimpleFileOptions::default())
        .map_err(|error| error.to_string())?;
    writer.write_all(bytes).map_err(|error| error.to_string())
}

fn render_pptx_content_types(candidates: &[Value]) -> String {
    let mut media_defaults = BTreeSet::<&'static str>::new();
    for candidate in candidates {
        if let Some(frame_path) = candidate.get("frame_path").and_then(Value::as_str) {
            media_defaults.insert(pptx_media_extension(&PathBuf::from(frame_path)));
        }
    }

    let mut output = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
"#,
    );
    for extension in media_defaults {
        output.push_str(&format!(
            r#"<Default Extension="{extension}" ContentType="{}"/>"#,
            pptx_media_content_type(extension)
        ));
        output.push('\n');
    }
    output.push_str(
        r#"<Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>
<Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>
<Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
<Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/>
<Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
<Override PartName="/ppt/notesMasters/notesMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.notesMaster+xml"/>
<Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>
"#,
    );
    for index in 1..=candidates.len() {
        output.push_str(&format!(
            r#"<Override PartName="/ppt/slides/slide{index}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#
        ));
        output.push('\n');
        output.push_str(&format!(
            r#"<Override PartName="/ppt/notesSlides/notesSlide{index}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml"/>"#
        ));
        output.push('\n');
    }
    output.push_str("</Types>\n");
    output
}

fn render_pptx_root_rels() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>
<Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/>
</Relationships>
"#
    .to_string()
}

fn render_pptx_app_props(slide_count: usize) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes">
<Application>AI Data Platform V3</Application>
<PresentationFormat>On-screen Show (16:9)</PresentationFormat>
<Slides>{slide_count}</Slides>
<Company>AI Data Platform</Company>
</Properties>
"#
    )
}

fn render_pptx_core_props(document: &Document) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
<dc:title>{}</dc:title>
<dc:creator>AI Data Platform V3</dc:creator>
<cp:keywords>video,ppt,selected-slides</cp:keywords>
<dcterms:created xsi:type="dcterms:W3CDTF">{}</dcterms:created>
<dcterms:modified xsi:type="dcterms:W3CDTF">{}</dcterms:modified>
</cp:coreProperties>
"#,
        html_escape_text(&document.title),
        document.created_at.to_rfc3339(),
        document.updated_at.to_rfc3339()
    )
}

fn render_pptx_presentation(slide_count: usize) -> String {
    let mut slide_ids = String::new();
    for index in 1..=slide_count {
        slide_ids.push_str(&format!(
            r#"<p:sldId id="{}" r:id="rId{}"/>"#,
            255 + index,
            index + 1
        ));
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
<p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst>
<p:sldIdLst>{slide_ids}</p:sldIdLst>
<p:sldSz cx="12192000" cy="6858000" type="wide"/>
<p:notesSz cx="6858000" cy="9144000"/>
</p:presentation>
"#
    )
}

fn render_pptx_presentation_rels(slide_count: usize) -> String {
    let mut output = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>
"#,
    );
    for index in 1..=slide_count {
        output.push_str(&format!(
            r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{index}.xml"/>"#,
            index + 1
        ));
        output.push('\n');
    }
    output.push_str(&format!(
        r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesMaster" Target="notesMasters/notesMaster1.xml"/>"#,
        slide_count + 2
    ));
    output.push('\n');
    output.push_str("</Relationships>\n");
    output
}

fn render_pptx_slide(slide_number: usize, candidate: &Value) -> String {
    let candidate_index = candidate
        .get("candidate_index")
        .and_then(Value::as_u64)
        .unwrap_or(slide_number as u64);
    let file_name = candidate
        .get("file_name")
        .and_then(Value::as_str)
        .unwrap_or("frame");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
<p:cSld>
<p:spTree>
<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
<p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>
<p:pic>
<p:nvPicPr><p:cNvPr id="2" name="Candidate {candidate_index}: {}"/><p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr><p:nvPr/></p:nvPicPr>
<p:blipFill><a:blip r:embed="rId1"/><a:stretch><a:fillRect/></a:stretch></p:blipFill>
<p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="12192000" cy="6858000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr>
</p:pic>
</p:spTree>
</p:cSld>
<p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>
</p:sld>
"#,
        html_escape_attr(file_name)
    )
}

fn render_pptx_slide_rels(slide_number: usize, extension: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/image{slide_number}.{extension}"/>
<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
<Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide" Target="../notesSlides/notesSlide{slide_number}.xml"/>
</Relationships>
"#
    )
}

fn render_pptx_notes_slide(slide_number: usize, candidate: &Value) -> String {
    let candidate_index = candidate
        .get("candidate_index")
        .and_then(Value::as_u64)
        .unwrap_or(slide_number as u64);
    let file_name = candidate
        .get("file_name")
        .and_then(Value::as_str)
        .unwrap_or("frame");
    let frame_path = candidate
        .get("frame_path")
        .and_then(Value::as_str)
        .unwrap_or("");
    let timestamp = candidate
        .get("timestamp_label")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let transcript_note = candidate
        .get("transcript_segments")
        .and_then(Value::as_array)
        .filter(|segments| !segments.is_empty())
        .map(|segments| {
            let text = segments
                .iter()
                .filter_map(|segment| video_item_text(segment, &["text", "content", "summary"]))
                .collect::<Vec<_>>()
                .join(" ");
            format!(" Pre-page transcript: {text}.")
        })
        .unwrap_or_else(|| " Transcript/subtitle alignment is not yet verified.".to_string());
    let note = format!(
        "Source frame: {file_name}; candidate: {candidate_index}; timestamp: {timestamp}; path: {frame_path}.{transcript_note}"
    );
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:notes xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
<p:cSld>
<p:spTree>
<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
<p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>
<p:sp>
<p:nvSpPr><p:cNvPr id="2" name="Speaker Notes"/><p:cNvSpPr txBox="1"/><p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr>
<p:spPr><a:xfrm><a:off x="685800" y="914400"/><a:ext cx="5486400" cy="5486400"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr>
<p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="zh-CN" sz="1400"/><a:t>{}</a:t></a:r></a:p></p:txBody>
</p:sp>
</p:spTree>
</p:cSld>
<p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>
</p:notes>
"#,
        html_escape_text(&note)
    )
}

fn render_pptx_notes_slide_rels(slide_number: usize) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesMaster" Target="../notesMasters/notesMaster1.xml"/>
<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="../slides/slide{slide_number}.xml"/>
</Relationships>
"#
    )
}

fn pptx_media_extension(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("jpg")
        .to_ascii_lowercase()
        .as_str()
    {
        "jpeg" => "jpeg",
        "png" => "png",
        "gif" => "gif",
        "bmp" => "bmp",
        "tif" => "tif",
        "tiff" => "tiff",
        _ => "jpg",
    }
}

fn pptx_media_content_type(extension: &str) -> &'static str {
    match extension {
        "jpeg" | "jpg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "tif" | "tiff" => "image/tiff",
        _ => "image/jpeg",
    }
}

const PPTX_SLIDE_MASTER_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
<p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr></p:spTree></p:cSld>
<p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/>
<p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="rId1"/></p:sldLayoutIdLst>
<p:txStyles><p:titleStyle/><p:bodyStyle/><p:otherStyle/></p:txStyles>
</p:sldMaster>
"#;

const PPTX_SLIDE_MASTER_RELS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/>
</Relationships>
"#;

const PPTX_SLIDE_LAYOUT_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldLayout xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" type="blank" preserve="1">
<p:cSld name="Blank"><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr></p:spTree></p:cSld>
<p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>
</p:sldLayout>
"#;

const PPTX_SLIDE_LAYOUT_RELS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/>
</Relationships>
"#;

const PPTX_NOTES_MASTER_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:notesMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
<p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr></p:spTree></p:cSld>
<p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/>
<p:notesStyle/>
</p:notesMaster>
"#;

const PPTX_NOTES_MASTER_RELS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/>
</Relationships>
"#;

const PPTX_THEME_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="AI Data Platform">
<a:themeElements>
<a:clrScheme name="AIDP"><a:dk1><a:srgbClr val="111319"/></a:dk1><a:lt1><a:srgbClr val="FFFFFF"/></a:lt1><a:dk2><a:srgbClr val="1F2430"/></a:dk2><a:lt2><a:srgbClr val="E7EAF0"/></a:lt2><a:accent1><a:srgbClr val="4F8CFF"/></a:accent1><a:accent2><a:srgbClr val="62D2A2"/></a:accent2><a:accent3><a:srgbClr val="F7B955"/></a:accent3><a:accent4><a:srgbClr val="F16C7F"/></a:accent4><a:accent5><a:srgbClr val="9D7CFF"/></a:accent5><a:accent6><a:srgbClr val="7DD3FC"/></a:accent6><a:hlink><a:srgbClr val="4F8CFF"/></a:hlink><a:folHlink><a:srgbClr val="9D7CFF"/></a:folHlink></a:clrScheme>
<a:fontScheme name="AIDP"><a:majorFont><a:latin typeface="Aptos Display"/></a:majorFont><a:minorFont><a:latin typeface="Aptos"/></a:minorFont></a:fontScheme>
<a:fmtScheme name="AIDP"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst><a:lnStyleLst><a:ln w="9525"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln></a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:bgFillStyleLst></a:fmtScheme>
</a:themeElements>
<a:objectDefaults/>
<a:extraClrSchemeLst/>
</a:theme>
"#;

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
    let generated_artifacts = output
        .get("generated_artifacts")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let deliverable_status = output
        .get("deliverable_status")
        .cloned()
        .unwrap_or_else(|| video_deliverable_status(&generated_artifacts));
    let local_thread_id = local_thread_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let artifact_id = format!("html-artifact-video-extraction-{assistant_run_id}-{document_id}");
    let artifact_title = format!("{title} - 视频提取摘要");
    let artifact_summary = json!({
        "id": artifact_id,
        "type": "html_artifact",
        "title": artifact_title,
        "template_id": "video_extraction_summary",
        "source_type": "video_extraction",
    });
    let completion_follow_up = video_extraction_completion_follow_up_from_output(
        output,
        std::slice::from_ref(&artifact_summary),
    );

    Some(json!({
        "kind": "html_artifact",
        "version": 1,
        "id": artifact_summary["id"].clone(),
        "title": artifact_summary["title"].clone(),
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
            "deliverable_status": deliverable_status,
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
            "generated_artifacts": generated_artifacts,
            "completion_follow_up": completion_follow_up,
            "local_thread_id": local_thread_id,
            "note": "后台视频抽取阶段已完成；该摘要只展示已实际产生或已明确缺失的证据和交付物，不会补造缺失内容。"
        }
    }))
}

pub fn video_extraction_output_artifact_from_output(
    assistant_run_id: &str,
    local_thread_id: Option<&str>,
    output: &Value,
    html_artifacts: &[Value],
) -> Option<Value> {
    let document_id = output.get("document_id").and_then(Value::as_str)?;
    let title = output
        .get("title")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("视频 PPT 提取");
    let generated_artifacts = output
        .get("generated_artifacts")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let deliverable_status = output
        .get("deliverable_status")
        .cloned()
        .unwrap_or_else(|| video_deliverable_status(&generated_artifacts));
    let files = generated_artifacts
        .get("files")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let final_deliverables_manifest = files
        .iter()
        .find(|file| {
            file.get("artifact_kind").and_then(Value::as_str) == Some("final_deliverables_manifest")
        })
        .cloned()
        .unwrap_or(Value::Null);
    let html_artifact_summaries = html_artifacts
        .iter()
        .filter_map(video_extraction_html_artifact_summary)
        .collect::<Vec<_>>();
    let html_artifact_ids = html_artifact_summaries
        .iter()
        .filter_map(|artifact| artifact.get("id").and_then(Value::as_str))
        .map(str::to_string)
        .collect::<Vec<_>>();

    Some(json!({
        "type": "video_extraction_artifacts",
        "id": format!("video-extraction-{assistant_run_id}-{document_id}"),
        "title": format!("{title} - 视频/PPT交付物"),
        "assistant_run_id": assistant_run_id,
        "local_thread_id": local_thread_id
            .map(str::trim)
            .filter(|value| !value.is_empty()),
        "document_id": document_id,
        "dataset_id": output.get("dataset_id").cloned().unwrap_or(Value::Null),
        "status": deliverable_status
            .get("state")
            .and_then(Value::as_str)
            .or_else(|| output.get("status").and_then(Value::as_str))
            .unwrap_or("partial"),
        "deliverable_status": deliverable_status,
        "file_count": files.len(),
        "generated_artifacts": generated_artifacts,
        "artifacts": output.get("artifacts").cloned().unwrap_or_else(|| json!([])),
        "primary_files": video_artifact_files_by_kinds(
            &files,
            &[
                "pptx",
                "final_deliverables_manifest",
                "extraction_artifacts_manifest",
                "ppt_outline",
                "slide_notes",
                "subtitle_page_map",
                "transcript_text",
            ],
        ),
        "final_deliverables_manifest": final_deliverables_manifest,
        "html_artifacts": html_artifact_summaries,
        "html_artifact_ids": html_artifact_ids,
        "completion_follow_up": video_extraction_completion_follow_up_from_output(output, html_artifacts),
    }))
}

pub fn video_extraction_completion_follow_up_from_output(
    output: &Value,
    html_artifacts: &[Value],
) -> Option<Value> {
    let document_id = output.get("document_id").and_then(Value::as_str)?;
    let title = output
        .get("title")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("视频 PPT 提取");
    let generated_artifacts = output
        .get("generated_artifacts")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let deliverable_status = output
        .get("deliverable_status")
        .cloned()
        .unwrap_or_else(|| video_deliverable_status(&generated_artifacts));
    let files = generated_artifacts
        .get("files")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let ready_file_kinds = video_ready_file_kinds(&files);
    let html_artifact_ids = html_artifacts
        .iter()
        .filter_map(|artifact| artifact.get("id").and_then(Value::as_str))
        .map(str::to_string)
        .collect::<Vec<_>>();
    let state = deliverable_status
        .get("state")
        .and_then(Value::as_str)
        .or_else(|| output.get("status").and_then(Value::as_str))
        .unwrap_or("partial");

    Some(json!({
        "kind": "video_extraction_completion_follow_up",
        "document_id": document_id,
        "dataset_id": output.get("dataset_id").cloned().unwrap_or(Value::Null),
        "title": title,
        "status": state,
        "deliverable_status": deliverable_status,
        "ready_file_kinds": ready_file_kinds,
        "html_artifact_ids": html_artifact_ids,
        "next_actions": video_extraction_completion_next_actions(state, &files),
        "model_follow_up": {
            "required": true,
            "instruction": "Use this structured completion status to notify the user in the next model-authored turn; do not claim missing files are available.",
        },
        "no_host_composed_answer": true,
    }))
}

pub fn merge_video_extraction_output_artifacts(
    existing_output_artifacts: &Value,
    assistant_run_id: &str,
    local_thread_id: Option<&str>,
    output: &Value,
    html_artifacts: &[Value],
) -> Value {
    let Some(next_artifact) = video_extraction_output_artifact_from_output(
        assistant_run_id,
        local_thread_id,
        output,
        html_artifacts,
    ) else {
        return existing_output_artifacts
            .as_array()
            .cloned()
            .map(Value::Array)
            .unwrap_or_else(|| json!([]));
    };
    let next_id = next_artifact
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let mut artifacts = existing_output_artifacts
        .as_array()
        .cloned()
        .unwrap_or_default();
    artifacts.retain(|artifact| artifact.get("id").and_then(Value::as_str) != Some(&next_id));
    artifacts.push(next_artifact);
    Value::Array(artifacts)
}

fn video_extraction_html_artifact_summary(artifact: &Value) -> Option<Value> {
    let id = artifact.get("id").and_then(Value::as_str)?;
    Some(json!({
        "id": id,
        "type": "html_artifact",
        "title": artifact.get("title").and_then(Value::as_str).unwrap_or("视频提取摘要"),
        "template_id": artifact
            .get("template_id")
            .and_then(Value::as_str)
            .unwrap_or("video_extraction_summary"),
        "source_type": artifact
            .get("source_type")
            .and_then(Value::as_str)
            .unwrap_or("video_extraction"),
    }))
}

fn video_ready_file_kinds(files: &[Value]) -> Vec<String> {
    files
        .iter()
        .filter_map(|file| file.get("artifact_kind").and_then(Value::as_str))
        .filter(|kind| {
            matches!(
                *kind,
                "pptx"
                    | "final_deliverables_manifest"
                    | "extraction_artifacts_manifest"
                    | "ppt_outline"
                    | "slide_notes"
                    | "subtitle_page_map"
                    | "transcript_text"
                    | "source_text"
                    | "timestamp_map"
            )
        })
        .map(str::to_string)
        .collect()
}

fn video_extraction_completion_next_actions(state: &str, files: &[Value]) -> Vec<Value> {
    let file_kinds = files
        .iter()
        .filter_map(|file| file.get("artifact_kind").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    let mut actions = vec![json!("open_video_extraction_summary")];
    if file_kinds.contains("pptx") {
        actions.push(json!("download_pptx"));
    }
    if file_kinds.contains("final_deliverables_manifest") {
        actions.push(json!("review_final_deliverables_manifest"));
    }
    if file_kinds.contains("subtitle_page_map") {
        actions.push(json!("review_subtitle_page_map"));
    }
    if state != "final_pptx_ready" {
        actions.push(json!("complete_keep_list_or_review_missing_inputs"));
    }
    actions
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

fn merged_video_extraction_artifact_refs(
    document: &Document,
    evidence: &VideoExtractionEvidenceSummary,
    frame_extraction: &Value,
    generated_artifacts: &Value,
) -> Vec<Value> {
    let mut artifacts = Vec::new();
    let mut seen_kinds = BTreeSet::<String>::new();

    for artifact in generated_artifacts
        .get("files")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|artifact| {
            artifact
                .get("artifact_kind")
                .and_then(Value::as_str)
                .is_some()
        })
    {
        if let Some(kind) = artifact.get("artifact_kind").and_then(Value::as_str) {
            seen_kinds.insert(kind.to_string());
        }
        artifacts.push(artifact.clone());
    }

    for artifact in video_extraction_artifact_refs(document, evidence, frame_extraction) {
        let Some(kind) = artifact.get("artifact_kind").and_then(Value::as_str) else {
            artifacts.push(artifact);
            continue;
        };
        if seen_kinds.insert(kind.to_string()) {
            artifacts.push(artifact);
        }
    }

    artifacts
}

fn video_deliverable_status(generated_artifacts: &Value) -> Value {
    let files = generated_artifacts
        .get("files")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let artifact_kinds = files
        .iter()
        .filter_map(|file| file.get("artifact_kind").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    let has_pptx = artifact_kinds.contains("pptx");
    let has_selected_slides = files.iter().any(|file| {
        file.get("artifact_kind").and_then(Value::as_str) == Some("selected_slides_manifest")
            && file
                .get("selected_count")
                .and_then(Value::as_u64)
                .is_some_and(|selected_count| selected_count > 0)
    });
    let has_contact_sheet = artifact_kinds.contains("contact_sheet_html");
    let has_outline = artifact_kinds.contains("ppt_outline");
    let has_transcript = artifact_kinds.contains("transcript_text");
    let has_source_text = artifact_kinds.contains("source_text");
    let has_final_deliverables_manifest = artifact_kinds.contains("final_deliverables_manifest");
    let has_subtitle_page_map = artifact_kinds.contains("subtitle_page_map");
    let warnings = video_generated_artifact_quality_warnings(
        &files,
        has_pptx,
        has_selected_slides,
        has_contact_sheet,
        has_transcript,
        has_subtitle_page_map,
    );
    let state = if has_pptx {
        "final_pptx_ready"
    } else if has_selected_slides {
        "selected_slides_ready"
    } else if has_contact_sheet {
        "review_ready"
    } else if has_outline || has_transcript || has_source_text {
        "evidence_artifacts_ready"
    } else {
        generated_artifacts
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("planned")
    };

    json!({
        "state": state,
        "file_count": files.len(),
        "has_transcript_text": has_transcript,
        "has_source_text": has_source_text,
        "has_ppt_outline": has_outline,
        "has_contact_sheet_html": has_contact_sheet,
        "has_selected_slides_manifest": has_selected_slides,
        "has_final_deliverables_manifest": has_final_deliverables_manifest,
        "has_subtitle_page_map": has_subtitle_page_map,
        "has_pptx": has_pptx,
        "warning_count": warnings.len(),
        "warnings": warnings,
    })
}

fn video_deliverable_status_with_frame_extraction(
    generated_artifacts: &Value,
    frame_extraction: &Value,
) -> Value {
    let mut status = video_deliverable_status(generated_artifacts);
    let frame_status = frame_extraction
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if frame_status.is_empty() {
        return status;
    }

    let reason = frame_extraction
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if let Some(object) = status.as_object_mut() {
        object.insert("frame_extraction_status".to_string(), json!(frame_status));
        if !reason.is_empty() {
            object.insert("frame_extraction_reason".to_string(), json!(reason));
        }
        if matches!(frame_status, "failed" | "skipped") {
            let mut warning = json!({
                "code": if frame_status == "failed" { "frame_extraction_failed" } else { "frame_extraction_skipped" },
                "severity": if frame_status == "failed" { "high" } else { "medium" },
                "message": if frame_status == "failed" {
                    "Raw frame extraction failed; contact sheet, slide selection, and screenshot PPTX are blocked until the source is available and FFmpeg succeeds."
                } else {
                    "Raw frame extraction was skipped; contact sheet, slide selection, and screenshot PPTX require a local media file or parsed frame evidence."
                },
            });
            if !reason.is_empty() {
                warning["reason"] = json!(reason);
            }
            let warnings = object
                .entry("warnings".to_string())
                .or_insert_with(|| json!([]));
            let mut next_warning_count = None;
            if let Some(warnings) = warnings.as_array_mut() {
                warnings.push(warning);
                next_warning_count = Some(warnings.len());
            }
            if let Some(next_warning_count) = next_warning_count {
                object.insert("warning_count".to_string(), json!(next_warning_count));
            }
        }
    }
    status
}

fn video_generated_artifact_quality_warnings(
    files: &[Value],
    has_pptx: bool,
    has_selected_slides: bool,
    has_contact_sheet: bool,
    has_transcript: bool,
    has_subtitle_page_map: bool,
) -> Vec<Value> {
    let mut warnings = Vec::new();
    if !has_transcript {
        warnings.push(json!({
            "code": "missing_transcript_alignment",
            "severity": "medium",
            "message": "No transcript evidence is attached yet; speaker notes cannot be aligned to narration."
        }));
    }
    if !has_contact_sheet {
        warnings.push(json!({
            "code": "missing_contact_sheet",
            "severity": "medium",
            "message": "No numbered contact sheet is available yet; slide selection should not be trusted for customer delivery."
        }));
    }
    if !has_selected_slides {
        warnings.push(json!({
            "code": "keep_list_not_confirmed",
            "severity": "high",
            "message": "No confirmed selected slides were found; fill the keep-list before final PPTX delivery."
        }));
    }
    if has_pptx {
        warnings.push(json!({
            "code": "screenshot_based_pptx",
            "severity": "low",
            "message": "PPTX uses raster screenshots; editable native slide reconstruction is not included in this slice."
        }));
    }
    if has_subtitle_page_map {
        warnings.push(json!({
            "code": "speaker_notes_pre_page_alignment",
            "severity": "low",
            "message": "Speaker notes include deterministic pre-page transcript assignment; review alignment before delivery."
        }));
    } else if files
        .iter()
        .any(|file| file.get("artifact_kind").and_then(Value::as_str) == Some("slide_notes"))
    {
        warnings.push(json!({
            "code": "speaker_notes_metadata_only",
            "severity": "low",
            "message": "Speaker notes currently preserve source frame metadata only; subtitle/讲稿对页 remains a later enhancement."
        }));
    }
    warnings
}

fn video_final_deliverables_manifest(
    document: &Document,
    files: &[Value],
    frame_count: u64,
) -> Value {
    let generated_artifacts = json!({
        "status": "completed",
        "files": files,
    });
    let deliverable_status = video_deliverable_status(&generated_artifacts);
    let state = deliverable_status
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or("planned");

    json!({
        "status": state,
        "source": "media_worker_final_deliverables_manifest",
        "document_id": document.id.to_string(),
        "dataset_id": document.dataset_id.to_string(),
        "title": document.title,
        "frame_count": frame_count,
        "deliverable_status": deliverable_status,
        "final_outputs": video_artifact_files_by_kinds(files, &["pptx"]),
        "review_outputs": video_artifact_files_by_kinds(files, &[
            "slide_image_candidates",
            "contact_sheet_plan",
            "contact_sheet_html",
            "ppt_keep_list_template",
            "selected_slides_manifest",
            "pptx_build_plan",
        ]),
        "evidence_outputs": video_artifact_files_by_kinds(files, &[
            "frame_manifest",
            "transcript_text",
            "source_text",
            "ppt_outline",
            "timestamp_map",
            "subtitle_page_map",
        ]),
        "next_action": video_final_deliverables_next_action(state),
    })
}

fn video_artifact_files_by_kinds(files: &[Value], kinds: &[&str]) -> Vec<Value> {
    files
        .iter()
        .filter(|file| {
            file.get("artifact_kind")
                .and_then(Value::as_str)
                .is_some_and(|kind| kinds.contains(&kind))
        })
        .cloned()
        .collect()
}

fn video_final_deliverables_next_action(state: &str) -> &'static str {
    match state {
        "final_pptx_ready" => "persist or expose the final PPTX deliverable for download",
        "selected_slides_ready" => "run the screenshot PPTX writer for selected slide frames",
        "review_ready" => "review the contact sheet and fill ppt_keep_list_template.json",
        "evidence_artifacts_ready" => "extract raw frames or keyframes before slide review",
        _ => "wait for media evidence or frame extraction",
    }
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

fn render_video_source_text_markdown(
    document: &Document,
    evidence: &VideoMediaEvidenceItems,
    frame_extraction: &Value,
) -> String {
    let frame_count = frame_extraction
        .get("frame_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let mut output = format!("# Source Text: {}\n\n", document.title);
    output.push_str("This file contains only evidence extracted or observed by V3. Missing evidence is left explicit rather than fabricated.\n\n");
    output.push_str("## Counts\n\n");
    output.push_str(&format!(
        "- Transcript segments: {}\n- Scenes: {}\n- Keyframe OCR snippets: {}\n- Raw frames: {}\n\n",
        evidence.transcript_segments.len(),
        evidence.scenes.len(),
        evidence.keyframe_ocr_snippets.len(),
        frame_count
    ));

    output.push_str("## Transcript Evidence\n\n");
    if evidence.transcript_segments.is_empty() {
        output.push_str("- Missing transcript evidence.\n\n");
    } else {
        for segment in &evidence.transcript_segments {
            let text =
                video_item_text(segment, &["text", "content", "summary"]).unwrap_or_default();
            let range = video_time_range_label(segment);
            output.push_str(&format!("- {}{}\n", optional_time_prefix(&range), text));
        }
        output.push('\n');
    }

    output.push_str("## Scene Evidence\n\n");
    if evidence.scenes.is_empty() {
        output.push_str("- Missing scene evidence.\n\n");
    } else {
        for scene in &evidence.scenes {
            let summary = video_item_text(scene, &["summary", "text", "description"])
                .unwrap_or_else(|| "Untitled scene".to_string());
            let range = video_time_range_label(scene);
            output.push_str(&format!("- {}{}\n", optional_time_prefix(&range), summary));
        }
        output.push('\n');
    }

    output.push_str("## Keyframe OCR Evidence\n\n");
    if evidence.keyframe_ocr_snippets.is_empty() {
        output.push_str("- Missing keyframe OCR evidence.\n\n");
    } else {
        for snippet in &evidence.keyframe_ocr_snippets {
            let text = video_item_text(snippet, &["text", "ocr_text", "summary"])
                .unwrap_or_else(|| "No OCR text".to_string());
            let timestamp = video_timestamp_label(snippet);
            output.push_str(&format!("- {}{}\n", optional_time_prefix(&timestamp), text));
        }
        output.push('\n');
    }

    output.push_str("## Raw Frame Evidence\n\n");
    if frame_count == 0 {
        output.push_str("- Missing raw frame evidence.\n");
    } else {
        output.push_str(&format!("- Raw frames captured: {frame_count}\n"));
        if let Some(raw_frames_dir) = frame_extraction
            .get("raw_frames_dir")
            .and_then(Value::as_str)
        {
            output.push_str(&format!("- Raw frames directory: {raw_frames_dir}\n"));
        }
        if let Some(manifest_path) = frame_extraction
            .get("manifest_path")
            .and_then(Value::as_str)
        {
            output.push_str(&format!("- Frame manifest: {manifest_path}\n"));
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

fn frame_extraction_manifest_path(frame_extraction: &Value) -> Option<PathBuf> {
    frame_extraction
        .get("manifest_path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_file())
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
    use std::{collections::BTreeMap, io::Read};
    use zip::ZipArchive;

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
    fn extract_output_surfaces_failed_frame_extraction_warning() {
        let document = test_document();
        let frame_extraction = json!({
            "status": "failed",
            "source": "ffmpeg_external_process",
            "reason": "ffmpeg exited with status 1",
            "input_path": "C:/tmp/missing-video.mp4"
        });

        let output =
            extract_video_ppt_output_with_frame_extraction(&document, &[], frame_extraction);

        assert_eq!(
            output["deliverable_status"]["frame_extraction_status"],
            json!("failed")
        );
        assert_eq!(
            output["deliverable_status"]["frame_extraction_reason"],
            json!("ffmpeg exited with status 1")
        );
        let warnings = output["deliverable_status"]["warnings"]
            .as_array()
            .expect("warnings");
        assert!(warnings.iter().any(|warning| {
            warning["code"] == json!("frame_extraction_failed")
                && warning["severity"] == json!("high")
                && warning["reason"] == json!("ffmpeg exited with status 1")
        }));
    }

    #[test]
    fn extract_output_prefers_generated_file_refs_over_placeholder_refs() {
        let document = test_document();
        let chunk = test_chunk(json!({
            "media": {
                "transcript_segments": [{"text": "page one"}],
                "scenes": [],
                "keyframe_ocr_snippets": [{"text": "title"}]
            }
        }));
        let generated_artifacts = json!({
            "status": "completed",
            "files": [{
                "artifact_kind": "transcript_text",
                "artifact_id": format!("video-{}-transcript_text", document.id),
                "title": "generated transcript",
                "format": "text/plain",
                "path": "generated_artifacts/transcript.txt",
                "uri": format!("artifact://video-{}-transcript_text", document.id)
            }, {
                "artifact_kind": "ppt_outline",
                "artifact_id": format!("video-{}-ppt_outline", document.id),
                "title": "generated outline",
                "format": "text/markdown",
                "path": "generated_artifacts/ppt_outline.md",
                "uri": format!("artifact://video-{}-ppt_outline", document.id)
            }, {
                "artifact_kind": "pptx",
                "artifact_id": format!("video-{}-pptx", document.id),
                "title": "generated pptx",
                "format": "application/vnd.openxmlformats-officedocument.presentationml.presentation",
                "path": "generated_artifacts/video_slides_screenshot_based.pptx",
                "uri": format!("artifact://video-{}-pptx", document.id)
            }]
        });

        let output = extract_video_ppt_output_with_artifacts(
            &document,
            &[chunk],
            video_frame_extraction_plan(&document),
            generated_artifacts,
        );

        let artifacts = output["artifacts"].as_array().expect("artifacts");
        assert_eq!(
            artifacts
                .iter()
                .filter(|artifact| artifact["artifact_kind"] == json!("transcript_text"))
                .count(),
            1
        );
        let transcript = artifacts
            .iter()
            .find(|artifact| artifact["artifact_kind"] == json!("transcript_text"))
            .expect("transcript artifact");
        assert_eq!(
            transcript["path"],
            json!("generated_artifacts/transcript.txt")
        );
        assert!(artifacts
            .iter()
            .any(|artifact| artifact["artifact_kind"] == json!("ppt_outline")));
        assert!(artifacts
            .iter()
            .any(|artifact| artifact["artifact_kind"] == json!("pptx")));
        assert!(artifacts
            .iter()
            .any(|artifact| artifact["artifact_kind"] == json!("html_summary")));
        assert_eq!(
            output["deliverable_status"]["state"],
            json!("final_pptx_ready")
        );
        assert_eq!(output["deliverable_status"]["has_pptx"], json!(true));
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
        assert_eq!(
            artifact["payload"]["deliverable_status"]["state"],
            json!("planned")
        );
        assert_eq!(artifact["payload"]["missing"][0], json!("transcript_text"));
        assert_eq!(
            artifact["payload"]["completion_follow_up"]["kind"],
            json!("video_extraction_completion_follow_up")
        );
        assert_eq!(
            artifact["payload"]["completion_follow_up"]["html_artifact_ids"][0],
            artifact["id"]
        );
        assert_eq!(
            artifact["payload"]["completion_follow_up"]["no_host_composed_answer"],
            json!(true)
        );
    }

    #[test]
    fn video_extraction_output_artifact_tracks_final_deliverables() {
        let document = test_document();
        let run_id = "00000000-0000-0000-0000-000000000001";
        let generated_artifacts = json!({
            "status": "completed",
            "files": [{
                "artifact_kind": "pptx",
                "artifact_id": format!("video-{}-pptx", document.id),
                "title": "generated pptx",
                "format": "application/vnd.openxmlformats-officedocument.presentationml.presentation",
                "path": "generated_artifacts/video_slides_screenshot_based.pptx",
                "uri": format!("artifact://video-{}-pptx", document.id)
            }, {
                "artifact_kind": "final_deliverables_manifest",
                "artifact_id": format!("video-{}-final-deliverables", document.id),
                "title": "final deliverables manifest",
                "format": "application/json",
                "path": "generated_artifacts/final_deliverables_manifest.json",
                "uri": format!("artifact://video-{}-final-deliverables", document.id)
            }, {
                "artifact_kind": "extraction_artifacts_manifest",
                "artifact_id": format!("video-{}-extraction-manifest", document.id),
                "title": "extraction artifacts manifest",
                "format": "application/json",
                "path": "generated_artifacts/extraction_artifacts_manifest.json",
                "uri": format!("artifact://video-{}-extraction-manifest", document.id)
            }]
        });
        let output = extract_video_ppt_output_with_artifacts(
            &document,
            &[],
            video_frame_extraction_plan(&document),
            generated_artifacts,
        );
        let html_artifact =
            video_extraction_html_artifact_from_output(run_id, Some("local-thread-1"), &output)
                .expect("html artifact");

        let output_artifact = video_extraction_output_artifact_from_output(
            run_id,
            Some("local-thread-1"),
            &output,
            &[html_artifact.clone()],
        )
        .expect("output artifact");

        assert_eq!(output_artifact["type"], json!("video_extraction_artifacts"));
        assert_eq!(
            output_artifact["document_id"],
            json!(document.id.to_string())
        );
        assert_eq!(
            output_artifact["deliverable_status"]["state"],
            json!("final_pptx_ready")
        );
        assert_eq!(
            output_artifact["deliverable_status"]["has_final_deliverables_manifest"],
            json!(true)
        );
        assert_eq!(
            output_artifact["final_deliverables_manifest"]["path"],
            json!("generated_artifacts/final_deliverables_manifest.json")
        );
        assert_eq!(
            output_artifact["completion_follow_up"]["kind"],
            json!("video_extraction_completion_follow_up")
        );
        assert_eq!(
            output_artifact["completion_follow_up"]["no_host_composed_answer"],
            json!(true)
        );
        assert!(output_artifact["primary_files"]
            .as_array()
            .expect("primary files")
            .iter()
            .any(|file| file["artifact_kind"] == json!("final_deliverables_manifest")));
        assert_eq!(output_artifact["html_artifact_ids"][0], html_artifact["id"]);
    }

    #[test]
    fn video_extraction_completion_follow_up_lists_ready_files_and_next_actions() {
        let document = test_document();
        let run_id = "00000000-0000-0000-0000-000000000001";
        let generated_artifacts = json!({
            "status": "completed",
            "files": [{
                "artifact_kind": "pptx",
                "artifact_id": format!("video-{}-pptx", document.id),
                "title": "generated pptx",
                "format": "application/vnd.openxmlformats-officedocument.presentationml.presentation",
                "path": "generated_artifacts/video_slides_screenshot_based.pptx"
            }, {
                "artifact_kind": "subtitle_page_map",
                "artifact_id": format!("video-{}-subtitle_page_map", document.id),
                "title": "subtitle page map",
                "format": "application/json",
                "path": "generated_artifacts/subtitle_page_map.json"
            }, {
                "artifact_kind": "final_deliverables_manifest",
                "artifact_id": format!("video-{}-final-deliverables", document.id),
                "title": "final deliverables manifest",
                "format": "application/json",
                "path": "generated_artifacts/final_deliverables_manifest.json"
            }]
        });
        let output = extract_video_ppt_output_with_artifacts(
            &document,
            &[],
            video_frame_extraction_plan(&document),
            generated_artifacts,
        );
        let html_artifact =
            video_extraction_html_artifact_from_output(run_id, Some("local-thread-1"), &output)
                .expect("html artifact");

        let follow_up =
            video_extraction_completion_follow_up_from_output(&output, &[html_artifact.clone()])
                .expect("follow up");

        assert_eq!(
            follow_up["kind"],
            json!("video_extraction_completion_follow_up")
        );
        assert_eq!(follow_up["document_id"], json!(document.id.to_string()));
        assert_eq!(follow_up["no_host_composed_answer"], json!(true));
        assert_eq!(follow_up["html_artifact_ids"][0], html_artifact["id"]);
        assert!(follow_up["ready_file_kinds"]
            .as_array()
            .expect("ready file kinds")
            .contains(&json!("subtitle_page_map")));
        assert!(follow_up["next_actions"]
            .as_array()
            .expect("next actions")
            .contains(&json!("download_pptx")));
        assert!(follow_up["next_actions"]
            .as_array()
            .expect("next actions")
            .contains(&json!("review_subtitle_page_map")));
        assert_eq!(follow_up["model_follow_up"]["required"], json!(true));
    }

    #[test]
    fn merge_video_extraction_output_artifacts_replaces_same_document_summary() {
        let document = test_document();
        let run_id = "00000000-0000-0000-0000-000000000001";
        let first_output = extract_video_ppt_output_with_artifacts(
            &document,
            &[],
            video_frame_extraction_plan(&document),
            json!({
                "status": "completed",
                "files": [{
                    "artifact_kind": "transcript_text",
                    "artifact_id": format!("video-{}-transcript", document.id),
                    "title": "transcript",
                    "format": "text/plain",
                    "path": "generated_artifacts/transcript.txt"
                }]
            }),
        );
        let first_artifact = video_extraction_output_artifact_from_output(
            run_id,
            Some("local-thread-1"),
            &first_output,
            &[],
        )
        .expect("first artifact");
        let second_output = extract_video_ppt_output_with_artifacts(
            &document,
            &[],
            video_frame_extraction_plan(&document),
            json!({
                "status": "completed",
                "files": [{
                    "artifact_kind": "final_deliverables_manifest",
                    "artifact_id": format!("video-{}-final-deliverables", document.id),
                    "title": "final deliverables manifest",
                    "format": "application/json",
                    "path": "generated_artifacts/final_deliverables_manifest.json"
                }]
            }),
        );
        let existing = json!([
            first_artifact,
            {
                "type": "other_artifact",
                "id": "keep-me"
            }
        ]);

        let merged = merge_video_extraction_output_artifacts(
            &existing,
            run_id,
            Some("local-thread-1"),
            &second_output,
            &[],
        );

        let artifacts = merged.as_array().expect("merged artifacts");
        let video_artifacts = artifacts
            .iter()
            .filter(|artifact| artifact["type"] == json!("video_extraction_artifacts"))
            .collect::<Vec<_>>();
        assert_eq!(artifacts.len(), 2);
        assert_eq!(video_artifacts.len(), 1);
        assert!(artifacts
            .iter()
            .any(|artifact| artifact["id"] == json!("keep-me")));
        assert_eq!(
            video_artifacts[0]["final_deliverables_manifest"]["path"],
            json!("generated_artifacts/final_deliverables_manifest.json")
        );
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
        assert_eq!(manifest["files"].as_array().expect("files").len(), 5);

        let transcript_path = manifest["files"]
            .as_array()
            .expect("files")
            .iter()
            .find(|file| file["artifact_kind"] == json!("transcript_text"))
            .and_then(|file| file["path"].as_str())
            .expect("transcript path");
        let transcript = fs::read_to_string(transcript_path).expect("transcript file");
        assert!(transcript.contains("第一页讲产品定位"));
        let source_text_path = manifest["files"]
            .as_array()
            .expect("files")
            .iter()
            .find(|file| file["artifact_kind"] == json!("source_text"))
            .and_then(|file| file["path"].as_str())
            .expect("source text path");
        let source_text = fs::read_to_string(source_text_path).expect("source text file");
        assert!(source_text.contains("Transcript Evidence"));
        assert!(source_text.contains("AI Data Platform"));
        let final_manifest_path = manifest["files"]
            .as_array()
            .expect("files")
            .iter()
            .find(|file| file["artifact_kind"] == json!("final_deliverables_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("final deliverables manifest path");
        let final_manifest =
            fs::read_to_string(final_manifest_path).expect("final deliverables manifest");
        assert!(final_manifest.contains("evidence_artifacts_ready"));
        assert!(final_manifest.contains("evidence_outputs"));
    }

    #[test]
    fn writes_video_text_artifacts_promotes_frame_manifest_file_ref() {
        let document = test_document();
        let output_root = std::env::temp_dir().join(format!(
            "aidp-v3-video-frame-manifest-artifact-test-{}",
            DocumentId::new()
        ));
        let session_dir = output_root.join(format!("video-extraction-{}", document.id));
        fs::create_dir_all(&session_dir).expect("session dir");
        let frame_manifest_path = session_dir.join(DEFAULT_FRAME_MANIFEST_FILE_NAME);
        fs::write(&frame_manifest_path, br#"{"frame_count":1}"#).expect("frame manifest");
        let frame_extraction = json!({
            "status": "completed",
            "manifest_path": frame_manifest_path.display().to_string(),
            "frame_count": 1
        });

        let manifest =
            write_video_extraction_text_artifacts(&document, &[], &frame_extraction, &output_root)
                .expect("text artifacts");

        let files = manifest["files"].as_array().expect("files");
        let frame_manifest = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("frame_manifest"))
            .expect("frame manifest artifact");
        assert_eq!(frame_manifest["format"], json!("application/json"));
        assert_eq!(
            frame_manifest["path"],
            json!(frame_manifest_path.display().to_string())
        );
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
        assert!(files
            .iter()
            .any(|file| file["artifact_kind"] == json!("contact_sheet_html")));
        assert!(files
            .iter()
            .any(|file| file["artifact_kind"] == json!("ppt_keep_list_template")));
        assert!(files
            .iter()
            .any(|file| file["artifact_kind"] == json!("selected_slides_manifest")));
        assert!(files
            .iter()
            .any(|file| file["artifact_kind"] == json!("pptx_build_plan")));
        assert!(files
            .iter()
            .any(|file| file["artifact_kind"] == json!("final_deliverables_manifest")));
        let candidates_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("slide_image_candidates"))
            .and_then(|file| file["path"].as_str())
            .expect("candidate manifest path");
        let candidates = fs::read_to_string(candidates_path).expect("candidate manifest");
        assert!(candidates.contains("frame_000001.jpg"));
        assert!(candidates.contains("review_required"));
        let contact_sheet_html_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("contact_sheet_html"))
            .and_then(|file| file["path"].as_str())
            .expect("contact sheet html path");
        let contact_sheet_html =
            fs::read_to_string(contact_sheet_html_path).expect("contact sheet html");
        assert!(contact_sheet_html.contains("candidate-1"));
        assert!(contact_sheet_html.contains("../raw_frames/frame_000001.jpg"));
        assert!(contact_sheet_html.contains(DEFAULT_PPT_KEEP_LIST_TEMPLATE_FILE_NAME));
        let keep_list_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("ppt_keep_list_template"))
            .and_then(|file| file["path"].as_str())
            .expect("keep list template path");
        let keep_list = fs::read_to_string(keep_list_path).expect("keep list template");
        assert!(keep_list.contains("waiting_for_selection"));
        assert!(keep_list.contains("do_not_auto_select_all_frames"));
        let selected_slides_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("selected_slides_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("selected slides manifest path");
        let selected_slides =
            fs::read_to_string(selected_slides_path).expect("selected slides manifest");
        assert!(selected_slides.contains("waiting_for_selection"));
        assert!(selected_slides.contains("\"selected_count\": 0"));
        let selected_slides_ref = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("selected_slides_manifest"))
            .expect("selected slides artifact ref");
        assert_eq!(selected_slides_ref["selected_count"], json!(0));
        let slide_notes_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("slide_notes"))
            .and_then(|file| file["path"].as_str())
            .expect("slide notes path");
        let slide_notes = fs::read_to_string(slide_notes_path).expect("slide notes");
        assert!(slide_notes.contains("Quality Warnings"));
        assert!(slide_notes.contains("No selected slides yet"));
        let pptx_plan_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("pptx_build_plan"))
            .and_then(|file| file["path"].as_str())
            .expect("pptx build plan path");
        let pptx_plan = fs::read_to_string(pptx_plan_path).expect("pptx build plan");
        assert!(pptx_plan.contains("waiting_for_keep_list"));
        assert!(pptx_plan.contains("ppt_keep_list_template.json"));
        assert!(pptx_plan.contains("selected_candidate_indices"));
        assert!(pptx_plan.contains(DEFAULT_VIDEO_SLIDES_PPTX_FILE_NAME));
        let final_manifest_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("final_deliverables_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("final deliverables manifest path");
        let final_manifest =
            fs::read_to_string(final_manifest_path).expect("final deliverables manifest");
        assert!(final_manifest.contains("review_ready"));
        assert!(final_manifest.contains("review_outputs"));
    }

    #[test]
    fn writes_selected_slide_manifest_from_existing_keep_list() {
        let document = test_document();
        let output_root = std::env::temp_dir().join(format!(
            "aidp-v3-video-selected-slides-test-{}",
            DocumentId::new()
        ));
        let session_dir = output_root.join(format!("video-extraction-{}", document.id));
        let artifacts_dir = session_dir.join(DEFAULT_GENERATED_ARTIFACTS_DIR_NAME);
        let raw_frames_dir = session_dir.join(DEFAULT_RAW_FRAMES_DIR_NAME);
        fs::create_dir_all(&raw_frames_dir).expect("raw frames dir");
        fs::create_dir_all(&artifacts_dir).expect("artifacts dir");
        fs::write(raw_frames_dir.join("frame_000001.jpg"), b"fake").expect("frame 1");
        fs::write(raw_frames_dir.join("frame_000002.jpg"), b"fake").expect("frame 2");
        fs::write(raw_frames_dir.join("frame_000003.jpg"), b"fake").expect("frame 3");
        let keep_list_path = artifacts_dir.join(DEFAULT_PPT_KEEP_LIST_TEMPLATE_FILE_NAME);
        fs::write(
            &keep_list_path,
            serde_json::to_vec_pretty(&json!({
                "status": "selected",
                "selected_candidate_indices": [2, 1, 2, 99, 0],
                "selection_notes": ["manual pick"]
            }))
            .expect("keep list bytes"),
        )
        .expect("keep list");
        let frame_extraction = json!({
            "status": "completed",
            "raw_frames_dir": raw_frames_dir.display().to_string(),
            "frame_count": 3,
            "manifest_file_name": DEFAULT_FRAME_MANIFEST_FILE_NAME
        });

        let manifest =
            write_video_extraction_text_artifacts(&document, &[], &frame_extraction, &output_root)
                .expect("candidate artifacts");

        let files = manifest["files"].as_array().expect("files");
        assert!(files
            .iter()
            .any(|file| file["artifact_kind"] == json!("pptx")));
        assert!(files
            .iter()
            .any(|file| file["artifact_kind"] == json!("final_deliverables_manifest")));
        let selected_slides_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("selected_slides_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("selected slides manifest path");
        let selected_slides =
            fs::read_to_string(selected_slides_path).expect("selected slides manifest");
        assert!(selected_slides.contains("ready_for_pptx_writer"));
        assert!(selected_slides.contains("\"selected_candidate_indices\": [\n    2,\n    1\n  ]"));
        assert!(selected_slides.contains("frame_000002.jpg"));
        assert!(selected_slides.contains("frame_000001.jpg"));
        let selected_slides_ref = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("selected_slides_manifest"))
            .expect("selected slides artifact ref");
        assert_eq!(selected_slides_ref["selected_count"], json!(2));
        let keep_list = fs::read_to_string(keep_list_path).expect("preserved keep list");
        assert!(keep_list.contains("manual pick"));
        let pptx_plan_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("pptx_build_plan"))
            .and_then(|file| file["path"].as_str())
            .expect("pptx build plan path");
        let pptx_plan = fs::read_to_string(pptx_plan_path).expect("pptx build plan");
        assert!(pptx_plan.contains("completed"));
        assert!(pptx_plan.contains("selected_slides_manifest.json"));
        let pptx_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("pptx"))
            .and_then(|file| file["path"].as_str())
            .expect("pptx path");
        let mut archive =
            ZipArchive::new(File::open(pptx_path).expect("pptx file")).expect("pptx zip");
        assert!(archive.by_name("[Content_Types].xml").is_ok());
        assert!(archive.by_name("ppt/presentation.xml").is_ok());
        assert!(archive.by_name("ppt/slides/slide1.xml").is_ok());
        assert!(archive.by_name("ppt/slides/slide2.xml").is_ok());
        assert!(archive.by_name("ppt/notesSlides/notesSlide1.xml").is_ok());
        assert!(archive.by_name("ppt/notesSlides/notesSlide2.xml").is_ok());
        assert!(archive.by_name("ppt/notesMasters/notesMaster1.xml").is_ok());
        assert!(archive.by_name("ppt/media/image1.jpg").is_ok());
        assert!(archive.by_name("ppt/media/image2.jpg").is_ok());
        let mut presentation_rels = String::new();
        archive
            .by_name("ppt/_rels/presentation.xml.rels")
            .expect("presentation relationships")
            .read_to_string(&mut presentation_rels)
            .expect("presentation relationships text");
        assert!(presentation_rels.contains("slide1.xml"));
        assert!(presentation_rels.contains("notesMaster1.xml"));
        let mut slide_rels = String::new();
        archive
            .by_name("ppt/slides/_rels/slide1.xml.rels")
            .expect("slide relationships")
            .read_to_string(&mut slide_rels)
            .expect("slide relationships text");
        assert!(slide_rels.contains("notesSlide1.xml"));
        let mut notes = String::new();
        archive
            .by_name("ppt/notesSlides/notesSlide1.xml")
            .expect("notes slide")
            .read_to_string(&mut notes)
            .expect("notes slide text");
        assert!(notes.contains("Source frame"));
        let final_manifest_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("final_deliverables_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("final deliverables manifest path");
        let final_manifest =
            fs::read_to_string(final_manifest_path).expect("final deliverables manifest");
        assert!(final_manifest.contains("final_pptx_ready"));
        assert!(final_manifest.contains("final_outputs"));
        assert!(final_manifest.contains(DEFAULT_VIDEO_SLIDES_PPTX_FILE_NAME));
        assert!(final_manifest.contains("speaker_notes_metadata_only"));
        let slide_notes_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("slide_notes"))
            .and_then(|file| file["path"].as_str())
            .expect("slide notes path");
        let slide_notes = fs::read_to_string(slide_notes_path).expect("slide notes");
        assert!(slide_notes.contains("Slide 1"));
        assert!(slide_notes.contains("candidate 2"));
    }

    #[test]
    fn writes_subtitle_page_map_and_transcript_notes_for_selected_slides() {
        let document = test_document();
        let output_root = std::env::temp_dir().join(format!(
            "aidp-v3-video-subtitle-map-test-{}",
            DocumentId::new()
        ));
        let session_dir = output_root.join(format!("video-extraction-{}", document.id));
        let artifacts_dir = session_dir.join(DEFAULT_GENERATED_ARTIFACTS_DIR_NAME);
        let raw_frames_dir = session_dir.join(DEFAULT_RAW_FRAMES_DIR_NAME);
        fs::create_dir_all(&raw_frames_dir).expect("raw frames dir");
        fs::create_dir_all(&artifacts_dir).expect("artifacts dir");
        fs::write(raw_frames_dir.join("frame_000001.jpg"), b"fake").expect("frame 1");
        fs::write(raw_frames_dir.join("frame_000002.jpg"), b"fake").expect("frame 2");
        let keep_list_path = artifacts_dir.join(DEFAULT_PPT_KEEP_LIST_TEMPLATE_FILE_NAME);
        fs::write(
            &keep_list_path,
            serde_json::to_vec_pretty(&json!({
                "status": "selected",
                "selected_candidate_indices": [2]
            }))
            .expect("keep list bytes"),
        )
        .expect("keep list");
        let frame_extraction = json!({
            "status": "completed",
            "raw_frames_dir": raw_frames_dir.display().to_string(),
            "frame_count": 2,
            "interval_seconds": 1.0,
            "manifest_file_name": DEFAULT_FRAME_MANIFEST_FILE_NAME
        });
        let chunk = test_chunk(json!({
            "media": {
                "transcript_segments": [{
                    "start_seconds": 0.2,
                    "end_seconds": 0.6,
                    "text": "Narration for the first selected slide"
                }],
                "scenes": [],
                "keyframe_ocr_snippets": []
            }
        }));

        let manifest = write_video_extraction_text_artifacts(
            &document,
            &[chunk],
            &frame_extraction,
            &output_root,
        )
        .expect("subtitle page map artifacts");

        let files = manifest["files"].as_array().expect("files");
        let subtitle_page_map_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("subtitle_page_map"))
            .and_then(|file| file["path"].as_str())
            .expect("subtitle page map path");
        let subtitle_page_map =
            fs::read_to_string(subtitle_page_map_path).expect("subtitle page map");
        assert!(subtitle_page_map.contains("pre_page_previous_to_current"));
        assert!(subtitle_page_map.contains("Narration for the first selected slide"));
        let slide_notes_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("slide_notes"))
            .and_then(|file| file["path"].as_str())
            .expect("slide notes path");
        let slide_notes = fs::read_to_string(slide_notes_path).expect("slide notes");
        assert!(slide_notes.contains("Aligned transcript"));
        assert!(slide_notes.contains("Narration for the first selected slide"));
        let pptx_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("pptx"))
            .and_then(|file| file["path"].as_str())
            .expect("pptx path");
        let mut archive =
            ZipArchive::new(File::open(pptx_path).expect("pptx file")).expect("pptx zip");
        let mut notes = String::new();
        archive
            .by_name("ppt/notesSlides/notesSlide1.xml")
            .expect("notes slide")
            .read_to_string(&mut notes)
            .expect("notes slide text");
        assert!(notes.contains("Pre-page transcript"));
        assert!(notes.contains("Narration for the first selected slide"));
        let final_manifest_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("final_deliverables_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("final deliverables manifest path");
        let final_manifest =
            fs::read_to_string(final_manifest_path).expect("final deliverables manifest");
        assert!(final_manifest.contains("subtitle_page_map"));
        assert!(final_manifest.contains("speaker_notes_pre_page_alignment"));
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
