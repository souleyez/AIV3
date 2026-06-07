use chrono::Utc;
use domain_model::{Document, DocumentChunk, WorkflowTask};
use image::{GenericImageView, ImageReader, Pixel};
use serde_json::{json, Value};
use std::{
    collections::{hash_map::DefaultHasher, BTreeSet},
    fs::{self, File},
    hash::{Hash, Hasher},
    io::{Read, Write},
    net::IpAddr,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
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
pub const DEFAULT_PUBLISHED_DELIVERABLE_MANIFEST_FILE_NAME: &str =
    "published_deliverable_manifest.json";
pub const DEFAULT_PUBLISHED_VERSION_HISTORY_FILE_NAME: &str = "published_version_history.json";
pub const DEFAULT_SLIDE_CANDIDATES_FILE_NAME: &str = "slide_candidates_manifest.json";
pub const DEFAULT_CONTACT_SHEET_PLAN_FILE_NAME: &str = "contact_sheet_plan.json";
pub const DEFAULT_CONTACT_SHEET_HTML_FILE_NAME: &str = "raw_contact_sheet.html";
pub const DEFAULT_PPT_KEEP_LIST_TEMPLATE_FILE_NAME: &str = "ppt_keep_list_template.json";
pub const DEFAULT_SELECTED_SLIDES_MANIFEST_FILE_NAME: &str = "selected_slides_manifest.json";
pub const DEFAULT_SLIDE_RECTANGLES_MANIFEST_FILE_NAME: &str = "slide_rectangles_manifest.json";
pub const DEFAULT_SUBTITLE_PAGE_MAP_FILE_NAME: &str = "subtitle_page_map.json";
pub const DEFAULT_SLIDE_QUALITY_REPORT_FILE_NAME: &str = "slide_quality_report.json";
pub const DEFAULT_SLIDE_NOTES_ARTIFACT_FILE_NAME: &str = "slide_notes.md";
pub const DEFAULT_VIDEO_SLIDES_MARKDOWN_FILE_NAME: &str = "video_slides.md";
pub const DEFAULT_PPTX_BUILD_PLAN_FILE_NAME: &str = "pptx_build_plan.json";
pub const DEFAULT_VIDEO_SLIDES_PPTX_FILE_NAME: &str = "video_slides_screenshot_based.pptx";
const LOW_CONFIDENCE_VIDEO_EVIDENCE_THRESHOLD: f64 = 0.65;
const VIDEO_VISUAL_SIGNATURE_GRID_SIZE: u32 = 32;
const VIDEO_VISUAL_NEAR_DUPLICATE_MAX_AVG_DIFF: f64 = 3.0;
const VIDEO_VISUAL_SHAPE_DUPLICATE_MIN_JACCARD: f64 = 0.86;
const VIDEO_VISUAL_SHAPE_DUPLICATE_MIN_CONTAINMENT: f64 = 0.96;
const VIDEO_VISUAL_SHAPE_DUPLICATE_MIN_SIGNAL_BALANCE: f64 = 0.80;
const VIDEO_VISUAL_SHAPE_DUPLICATE_MIN_SIGNAL_SAMPLES: usize = 6;
const VIDEO_VISUAL_SHAPE_DUPLICATE_MAX_AVG_LUMA: f64 = 140.0;
const VIDEO_AUTO_SLIDE_SEGMENT_MAX_AVG_DIFF: f64 = 2.0;
const VIDEO_AUTO_SLIDE_SEGMENT_CHANGED_SAMPLE_MIN_DIFF: u8 = 24;
const VIDEO_AUTO_SLIDE_SEGMENT_MAX_CHANGED_SAMPLE_RATIO: f64 = 0.012;
const VIDEO_AUTO_SLIDE_MIN_STABLE_FRAMES: usize = 2;
const VIDEO_AUTO_SLIDE_MAX_SELECTED_PAGES: usize = 96;
const VIDEO_AUTO_SLIDE_DARK_LOW_INFO_MAX_AVG_LUMA: f64 = 24.0;
const VIDEO_AUTO_SLIDE_DARK_LOW_INFO_MAX_LUMA_RANGE: u8 = 12;
const VIDEO_AUTO_SLIDE_BRIGHT_LOW_INFO_MIN_AVG_LUMA: f64 = 238.0;
const VIDEO_AUTO_SLIDE_BRIGHT_LOW_INFO_MAX_LUMA_RANGE: u8 = 8;
const VIDEO_AUTO_SLIDE_FLAT_LOW_INFO_MAX_LUMA_RANGE: u8 = 3;
const VIDEO_SLIDE_SHARPNESS_TARGET_SAMPLES: f64 = 40_000.0;
const VIDEO_SLIDE_SHARPNESS_LOW_RISK_MIN_SCORE: i64 = 70;
const VIDEO_SLIDE_SHARPNESS_MEDIUM_RISK_MIN_SCORE: i64 = 40;
const VIDEO_REMOTE_INPUT_DEFAULT_MAX_BYTES: u64 = 200 * 1024 * 1024;
const VIDEO_REMOTE_INPUT_DEFAULT_TIMEOUT_SECS: u64 = 60;
const VIDEO_DELIVERABLE_PACKAGE_REQUIRED_KINDS: &[&str] = &[
    "pptx",
    "final_deliverables_manifest",
    "extraction_artifacts_manifest",
    "slide_rectangles_manifest",
    "slide_notes",
    "video_slides_markdown",
];
const VIDEO_PUBLISHED_DELIVERABLE_REQUIRED_KINDS: &[&str] = &[
    "pptx",
    "final_deliverables_manifest",
    "published_deliverable_manifest",
    "published_version_history",
    "extraction_artifacts_manifest",
    "slide_rectangles_manifest",
    "slide_notes",
    "video_slides_markdown",
];
const VIDEO_CONDITIONAL_DELIVERABLE_KINDS: &[&str] = &["subtitle_page_map"];

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
    let evidence_items = video_media_evidence_items_from_chunks(chunks);
    let artifacts = merged_video_extraction_artifact_refs(
        document,
        &evidence,
        &frame_extraction,
        &generated_artifacts,
    );
    let deliverable_status = video_deliverable_status_with_evidence_and_frame_extraction(
        &generated_artifacts,
        &frame_extraction,
        &evidence_items,
    );
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
        "source_summary": video_source_summary_from_document(document),
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

#[derive(Clone, Debug, PartialEq, Eq)]
enum VideoFrameExtractionInput {
    LocalPath(PathBuf),
    RemoteUrl(String),
}

impl VideoFrameExtractionInput {
    fn manifest_input_kind(&self) -> &'static str {
        match self {
            Self::LocalPath(_) => "local_media_path",
            Self::RemoteUrl(_) => "remote_video_url",
        }
    }

    fn manifest_input_value(&self) -> String {
        match self {
            Self::LocalPath(path) => path.display().to_string(),
            Self::RemoteUrl(_) => "[redacted]".to_string(),
        }
    }

    fn input_url_redacted(&self) -> bool {
        matches!(self, Self::RemoteUrl(_))
    }

    fn sanitize_process_output(&self, value: String) -> String {
        match self {
            Self::LocalPath(_) => value,
            Self::RemoteUrl(url) => value.replace(url, "[redacted]"),
        }
    }
}

#[derive(Clone, Debug)]
struct PreparedVideoFrameExtractionInput {
    ffmpeg_arg: String,
    cached_input_path: Option<PathBuf>,
    downloaded_bytes: Option<u64>,
}

pub fn run_video_frame_extraction_if_enabled(
    document: &Document,
    config: &FrameExtractionConfig,
) -> Value {
    if !config.enabled {
        return video_frame_extraction_plan(document);
    }

    let Some(input) = resolve_media_frame_extraction_input(document) else {
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

    match run_video_frame_extraction_with_input(document, &input, config) {
        Ok(manifest) => manifest,
        Err(error) => json!({
            "status": "failed",
            "source": "ffmpeg_external_process",
            "enabled": true,
            "reason": error,
            "input_kind": input.manifest_input_kind(),
            "input_path": input.manifest_input_value(),
            "input_url_redacted": input.input_url_redacted(),
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
    run_video_frame_extraction_with_input(
        document,
        &VideoFrameExtractionInput::LocalPath(input_path.to_path_buf()),
        config,
    )
}

fn run_video_frame_extraction_with_input(
    document: &Document,
    input: &VideoFrameExtractionInput,
    config: &FrameExtractionConfig,
) -> Result<Value, String> {
    if config.interval_seconds <= 0.0 {
        return Err("invalid_frame_interval".to_string());
    }

    let session_dir = config
        .output_root
        .join(format!("video-extraction-{}", document.id));
    let prepared_input = prepare_video_frame_extraction_input(input, &session_dir)
        .map_err(|error| input.sanitize_process_output(error))?;
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
        .arg(&prepared_input.ffmpeg_arg)
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
            input.sanitize_process_output(stderr)
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
        "input_kind": input.manifest_input_kind(),
        "input_path": input.manifest_input_value(),
        "input_url_redacted": input.input_url_redacted(),
        "input_downloaded": prepared_input.cached_input_path.is_some(),
        "input_download_bytes": prepared_input.downloaded_bytes,
        "input_cache_path": prepared_input.cached_input_path.as_ref().map(|path| path.display().to_string()),
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

fn prepare_video_frame_extraction_input(
    input: &VideoFrameExtractionInput,
    session_dir: &Path,
) -> Result<PreparedVideoFrameExtractionInput, String> {
    match input {
        VideoFrameExtractionInput::LocalPath(path) => Ok(PreparedVideoFrameExtractionInput {
            ffmpeg_arg: path.display().to_string(),
            cached_input_path: None,
            downloaded_bytes: None,
        }),
        VideoFrameExtractionInput::RemoteUrl(url) => {
            let (path, bytes) = download_remote_video_frame_extraction_input(url, session_dir)?;
            Ok(PreparedVideoFrameExtractionInput {
                ffmpeg_arg: path.display().to_string(),
                cached_input_path: Some(path),
                downloaded_bytes: Some(bytes),
            })
        }
    }
}

fn download_remote_video_frame_extraction_input(
    raw_url: &str,
    session_dir: &Path,
) -> Result<(PathBuf, u64), String> {
    let url = reqwest::Url::parse(raw_url.trim()).map_err(|_| "remote_media_invalid_url")?;
    if !video_remote_media_url_allowed(url.as_str()) {
        return Err("remote_media_url_not_allowed".to_string());
    }
    let max_bytes = env_u64(
        "MEDIA_REMOTE_INPUT_MAX_BYTES",
        VIDEO_REMOTE_INPUT_DEFAULT_MAX_BYTES,
    );
    let timeout_secs = env_u64(
        "MEDIA_REMOTE_INPUT_TIMEOUT_SECS",
        VIDEO_REMOTE_INPUT_DEFAULT_TIMEOUT_SECS,
    )
    .max(1);
    let cache_dir = session_dir.join("remote_input");
    fs::create_dir_all(&cache_dir).map_err(|error| error.to_string())?;
    let extension = video_remote_media_url_extension(url.as_str()).unwrap_or(".mp4");
    let cache_key = video_remote_media_cache_key(url.as_str());
    let target_path = cache_dir.join(format!("remote-video-{cache_key}{extension}"));
    if target_path.is_file() {
        let bytes = fs::metadata(&target_path)
            .map_err(|error| error.to_string())?
            .len();
        return Ok((target_path, bytes));
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "remote_media_client_build_failed")?;
    let mut response = client
        .get(url)
        .send()
        .map_err(|_| "remote_media_fetch_failed")?;
    if response.status().is_redirection() {
        return Err("remote_media_redirect_not_followed".to_string());
    }
    if !response.status().is_success() {
        return Err(format!("remote_media_http_{}", response.status().as_u16()));
    }
    if response
        .content_length()
        .is_some_and(|content_length| content_length > max_bytes)
    {
        return Err("remote_media_exceeds_max_bytes".to_string());
    }
    if !video_remote_media_response_type_allowed(
        response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default(),
    ) {
        return Err("remote_media_response_type_not_allowed".to_string());
    }

    let temp_path = cache_dir.join(format!(
        "remote-video-{cache_key}-{}.part{extension}",
        std::process::id()
    ));
    let mut file = File::create(&temp_path).map_err(|error| error.to_string())?;
    let mut downloaded = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = response
            .read(&mut buffer)
            .map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        downloaded += read as u64;
        if downloaded > max_bytes {
            let _ = fs::remove_file(&temp_path);
            return Err("remote_media_exceeds_max_bytes".to_string());
        }
        file.write_all(&buffer[..read])
            .map_err(|error| error.to_string())?;
    }
    file.flush().map_err(|error| error.to_string())?;
    fs::rename(&temp_path, &target_path).map_err(|error| error.to_string())?;
    Ok((target_path, downloaded))
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
    let timestamp_map = video_public_timestamp_map(document, &evidence, frame_extraction);
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
    let published_deliverable_manifest_path =
        artifacts_dir.join(DEFAULT_PUBLISHED_DELIVERABLE_MANIFEST_FILE_NAME);
    let published_version_history_path =
        artifacts_dir.join(DEFAULT_PUBLISHED_VERSION_HISTORY_FILE_NAME);
    let manifest_path = artifacts_dir.join(DEFAULT_EXTRACTION_ARTIFACTS_MANIFEST_FILE_NAME);
    files.push(video_generated_artifact_file(
        document,
        "final_deliverables_manifest",
        "application/json",
        &final_deliverables_manifest_path,
    ));
    files.push(video_generated_artifact_file(
        document,
        "published_deliverable_manifest",
        "application/json",
        &published_deliverable_manifest_path,
    ));
    files.push(video_generated_artifact_file(
        document,
        "published_version_history",
        "application/json",
        &published_version_history_path,
    ));
    files.push(video_generated_artifact_file(
        document,
        "extraction_artifacts_manifest",
        "application/json",
        &manifest_path,
    ));

    let final_deliverables_manifest =
        video_final_deliverables_manifest(document, &files, frame_count);
    let final_deliverables_manifest_bytes = serde_json::to_vec_pretty(&final_deliverables_manifest)
        .map_err(|error| error.to_string())?;
    fs::write(
        &final_deliverables_manifest_path,
        final_deliverables_manifest_bytes,
    )
    .map_err(|error| error.to_string())?;

    let published_deliverable_manifest =
        video_published_deliverable_manifest(document, &files, frame_count);
    let published_deliverable_manifest_bytes =
        serde_json::to_vec_pretty(&published_deliverable_manifest)
            .map_err(|error| error.to_string())?;
    fs::write(
        &published_deliverable_manifest_path,
        published_deliverable_manifest_bytes,
    )
    .map_err(|error| error.to_string())?;

    let published_version_history =
        video_published_version_history_manifest(document, &files, frame_count);
    let published_version_history_bytes =
        serde_json::to_vec_pretty(&published_version_history).map_err(|error| error.to_string())?;
    fs::write(
        &published_version_history_path,
        published_version_history_bytes,
    )
    .map_err(|error| error.to_string())?;

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
    let public_manifest = video_public_extraction_artifacts_manifest(&manifest);
    let manifest_bytes =
        serde_json::to_vec_pretty(&public_manifest).map_err(|error| error.to_string())?;
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

    let keep_list_template_path = artifacts_dir.join(DEFAULT_PPT_KEEP_LIST_TEMPLATE_FILE_NAME);
    let manual_selected_candidate_indices =
        read_selected_candidate_indices_from_keep_list(&keep_list_template_path, frames.len());
    let auto_selection = if manual_selected_candidate_indices.is_empty() {
        video_auto_slide_selection_from_frames(&frames, frame_extraction)
    } else {
        VideoAutoSlideSelection::manual_override_skipped()
    };
    let auto_selection_value = auto_selection.to_value();
    let selected_candidate_indices = if manual_selected_candidate_indices.is_empty() {
        auto_selection.selected_candidate_indices.clone()
    } else {
        manual_selected_candidate_indices
    };
    let selection_source =
        if !selected_candidate_indices.is_empty() && auto_selection.status == "auto_selected" {
            "auto_unique_slide_keyframes"
        } else if !selected_candidate_indices.is_empty() {
            "ppt_keep_list_template"
        } else {
            "none"
        };
    let selected_candidate_index_set = selected_candidate_indices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();

    let candidates = frames
        .iter()
        .enumerate()
        .map(|(index, path)| {
            let candidate_index = index + 1;
            let timestamp_seconds = video_candidate_timestamp_seconds(index + 1, frame_extraction);
            let evidence_refs =
                video_candidate_nearby_evidence_refs(timestamp_seconds, evidence, frame_extraction);
            let evidence_ref_count = video_evidence_ref_count(&evidence_refs);
            let selection_status = if selected_candidate_index_set.contains(&candidate_index) {
                selection_source
            } else {
                "review_required"
            };
            json!({
                "candidate_index": candidate_index,
                "source": "raw_frames",
                "file_name": path.file_name().and_then(|value| value.to_str()).unwrap_or("frame"),
                "frame_path": path.display().to_string(),
                "timestamp_seconds": timestamp_seconds,
                "timestamp_label": format_seconds(timestamp_seconds),
                "evidence_reference_status": if evidence_ref_count == 0 { "raw_frame_only" } else { "matched_nearby_evidence" },
                "nearby_evidence_refs": evidence_refs,
                "selection_status": selection_status,
                "notes": if selection_status == "auto_unique_slide_keyframes" {
                    "Candidate frame selected automatically as the representative frame for a stable PPT page segment; review is still required before customer delivery."
                } else {
                    "Candidate frame retained conservatively; automatic slide-page extraction or manual keep-list review may select it later."
                },
            })
        })
        .collect::<Vec<_>>();
    let candidate_evidence_ref_count = candidates
        .iter()
        .map(|candidate| {
            candidate
                .get("nearby_evidence_refs")
                .map(video_evidence_ref_count)
                .unwrap_or(0)
        })
        .sum::<usize>();
    let candidate_manifest_path = artifacts_dir.join(DEFAULT_SLIDE_CANDIDATES_FILE_NAME);
    let candidate_manifest = json!({
        "status": if selection_source == "auto_unique_slide_keyframes" { "auto_selected" } else { "review_required" },
        "source": "raw_frames",
        "document_id": document.id.to_string(),
        "dataset_id": document.dataset_id.to_string(),
        "title": document.title,
        "raw_frames_dir": raw_frames_dir.display().to_string(),
        "candidate_count": candidates.len(),
        "candidate_evidence_ref_count": candidate_evidence_ref_count,
        "candidate_evidence_policy": {
            "status": if candidate_evidence_ref_count == 0 { "raw_frames_only" } else { "timestamp_nearby_refs" },
            "match_rule": "scene overlap by candidate timestamp; OCR and transcript refs use timestamp proximity with a conservative closest fallback",
            "redaction": "source/provider/locator fields are sanitized before writing candidate evidence refs",
        },
        "rectangle_extraction_status": "not_promoted",
        "selection_source": selection_source,
        "selected_candidate_indices": selected_candidate_indices.clone(),
        "auto_selection": auto_selection_value.clone(),
        "dedupe_policy": if selection_source == "auto_unique_slide_keyframes" {
            "auto-selected stable PPT page segments first; selected representatives are then de-duplicated by exact frame bytes and conservative visual similarity before PPTX generation"
        } else {
            "conservative_keep_all_until_review"
        },
        "candidates": candidates,
    });
    fs::write(
        &candidate_manifest_path,
        video_public_json_bytes(&candidate_manifest)?,
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
        "status": if selection_source == "auto_unique_slide_keyframes" { "auto_selected" } else { "planned" },
        "source": "raw_frames",
        "raw_frames_dir": raw_frames_dir.display().to_string(),
        "candidate_manifest": candidate_manifest_path.display().to_string(),
        "preview_html": contact_sheet_html_path.display().to_string(),
        "recommended_output": contact_sheet_html_path.display().to_string(),
        "review_rule": if selection_source == "auto_unique_slide_keyframes" {
            "auto-selected stable PPT page representatives are available; use the numbered contact sheet to verify or override selected_candidate_indices"
        } else {
            "build a numbered contact sheet before rectangle extraction; keep user/model selected slide numbers only"
        },
        "skill_reference": "wechat-video-ppt-extract/contact-sheet --source raw_frames",
    });
    fs::write(
        &contact_sheet_plan_path,
        video_public_json_bytes(&contact_sheet_plan)?,
    )
    .map_err(|error| error.to_string())?;

    let keep_list_template = json!({
        "status": if selection_source == "auto_unique_slide_keyframes" { "auto_selected" } else { "waiting_for_selection" },
        "source": "slide_candidates_manifest",
        "document_id": document.id.to_string(),
        "dataset_id": document.dataset_id.to_string(),
        "title": document.title,
        "candidate_manifest": candidate_manifest_path.display().to_string(),
        "contact_sheet_plan": contact_sheet_plan_path.display().to_string(),
        "selected_candidate_indices": selected_candidate_indices.clone(),
        "selection_source": selection_source,
        "auto_selection": auto_selection_value.clone(),
        "rejected_candidate_indices": [],
        "selection_notes": [],
        "review_policy": {
            "requires_numbered_contact_sheet": selection_source != "auto_unique_slide_keyframes",
            "manual_override_allowed": true,
            "dedupe_policy": "exact frame bytes, conservative visual near-duplicates, and high-confidence contrast-normalized shape duplicates are removed before final PPTX generation",
            "do_not_auto_select_all_frames": true,
        },
    });
    if !keep_list_template_path.is_file() {
        fs::write(
            &keep_list_template_path,
            video_public_json_bytes(&keep_list_template)?,
        )
        .map_err(|error| error.to_string())?;
    }

    let selected_slides_manifest_path =
        artifacts_dir.join(DEFAULT_SELECTED_SLIDES_MANIFEST_FILE_NAME);
    let slide_rectangles_manifest_path =
        artifacts_dir.join(DEFAULT_SLIDE_RECTANGLES_MANIFEST_FILE_NAME);
    let selected_slides_manifest = selected_slides_manifest_from_keep_list(
        document,
        &frames,
        &selected_candidate_indices,
        evidence,
        frame_extraction,
        &candidate_manifest_path,
        &contact_sheet_html_path,
        &keep_list_template_path,
        selection_source,
        &auto_selection_value,
    );
    fs::write(
        &selected_slides_manifest_path,
        video_public_json_bytes(&selected_slides_manifest)?,
    )
    .map_err(|error| error.to_string())?;

    let slide_rectangles_manifest = video_slide_rectangles_manifest_from_selected_slides(
        document,
        &selected_slides_manifest,
        &candidate_manifest_path,
        &contact_sheet_html_path,
        &keep_list_template_path,
    );
    fs::write(
        &slide_rectangles_manifest_path,
        video_public_json_bytes(&slide_rectangles_manifest)?,
    )
    .map_err(|error| error.to_string())?;

    let subtitle_page_map = video_subtitle_page_map_from_selected_slides(&selected_slides_manifest);
    let subtitle_page_map_path = artifacts_dir.join(DEFAULT_SUBTITLE_PAGE_MAP_FILE_NAME);
    let has_subtitle_page_map =
        subtitle_page_map.get("status").and_then(Value::as_str) == Some("mapped");
    if has_subtitle_page_map {
        fs::write(
            &subtitle_page_map_path,
            video_public_json_bytes(&subtitle_page_map)?,
        )
        .map_err(|error| error.to_string())?;
    }

    let slide_quality_report_path = artifacts_dir.join(DEFAULT_SLIDE_QUALITY_REPORT_FILE_NAME);
    let slide_quality_report = video_slide_quality_report_from_manifests(
        document,
        &selected_slides_manifest,
        &slide_rectangles_manifest,
        &subtitle_page_map,
    );
    fs::write(
        &slide_quality_report_path,
        video_public_json_bytes(&slide_quality_report)?,
    )
    .map_err(|error| error.to_string())?;

    let slide_notes_path = artifacts_dir.join(DEFAULT_SLIDE_NOTES_ARTIFACT_FILE_NAME);
    fs::write(
        &slide_notes_path,
        render_selected_slide_notes_markdown(document, &selected_slides_manifest),
    )
    .map_err(|error| error.to_string())?;
    let video_slides_markdown_path = artifacts_dir.join(DEFAULT_VIDEO_SLIDES_MARKDOWN_FILE_NAME);
    fs::write(
        &video_slides_markdown_path,
        render_video_slides_markdown(document, &selected_slides_manifest),
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
        "candidate_manifest_file_name": DEFAULT_SLIDE_CANDIDATES_FILE_NAME,
        "contact_sheet_plan": contact_sheet_plan_path.display().to_string(),
        "contact_sheet_plan_file_name": DEFAULT_CONTACT_SHEET_PLAN_FILE_NAME,
        "contact_sheet_html": contact_sheet_html_path.display().to_string(),
        "contact_sheet_html_file_name": DEFAULT_CONTACT_SHEET_HTML_FILE_NAME,
        "keep_list_template": keep_list_template_path.display().to_string(),
        "keep_list_template_file_name": DEFAULT_PPT_KEEP_LIST_TEMPLATE_FILE_NAME,
            "selected_slides_manifest": selected_slides_manifest_path.display().to_string(),
            "selected_slides_manifest_file_name": DEFAULT_SELECTED_SLIDES_MANIFEST_FILE_NAME,
            "slide_rectangles_manifest": slide_rectangles_manifest_path.display().to_string(),
            "slide_rectangles_manifest_file_name": DEFAULT_SLIDE_RECTANGLES_MANIFEST_FILE_NAME,
            "slide_quality_report": slide_quality_report_path.display().to_string(),
            "slide_quality_report_file_name": DEFAULT_SLIDE_QUALITY_REPORT_FILE_NAME,
            "recommended_output": pptx_output_path.display().to_string(),
            "recommended_output_file_name": DEFAULT_VIDEO_SLIDES_PPTX_FILE_NAME,
        "selection": {
            "mode": if selection_source == "auto_unique_slide_keyframes" { "auto_unique_slide_keyframes" } else { "manual_or_model_review_required" },
            "selected_candidate_indices": selected_candidate_indices.clone(),
            "auto_selection": auto_selection_value.clone(),
            "rule": if selection_source == "auto_unique_slide_keyframes" {
                "Build a screenshot PPTX from stable PPT page representatives; review the contact sheet and selected_slides_manifest before customer delivery."
            } else {
                "Do not build a final PPTX until ppt_keep_list_template.json is filled and confirmed from the numbered contact sheet."
            },
        },
        "speaker_notes": {
            "include_source_frame": true,
            "include_candidate_index": true,
            "include_timestamp_when_available": true,
            "include_transcript_pre_page_map": has_subtitle_page_map,
        },
        "native_metadata": {
            "picture_alt_text": true,
            "include_source_frame": true,
            "include_timestamp": true,
            "include_crop_mode": true,
            "redact_internal_paths": true,
        },
        "build_policy": if selection_source == "auto_unique_slide_keyframes" { "one_raster_image_per_auto_detected_ppt_page" } else { "one_raster_image_per_slide_after_keep_list" },
        "skill_reference": "wechat-video-ppt-extract/build-selected",
    });
    fs::write(
        &pptx_build_plan_path,
        video_public_json_bytes(&pptx_build_plan)?,
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
        object.insert(
            "dedupe_status".to_string(),
            selected_slides_manifest
                .get("dedupe_status")
                .cloned()
                .unwrap_or_else(|| json!("waiting_for_selection")),
        );
        object.insert(
            "deduped_candidate_count".to_string(),
            selected_slides_manifest
                .get("deduped_candidate_count")
                .cloned()
                .unwrap_or_else(|| json!(0)),
        );
        object.insert(
            "exact_duplicate_count".to_string(),
            selected_slides_manifest
                .get("exact_duplicate_count")
                .cloned()
                .unwrap_or_else(|| json!(0)),
        );
        object.insert(
            "visual_duplicate_count".to_string(),
            selected_slides_manifest
                .get("visual_duplicate_count")
                .cloned()
                .unwrap_or_else(|| json!(0)),
        );
    }

    let mut slide_rectangles_artifact = video_generated_artifact_file(
        document,
        "slide_rectangles_manifest",
        "application/json",
        &slide_rectangles_manifest_path,
    );
    if let Some(object) = slide_rectangles_artifact.as_object_mut() {
        object.insert(
            "rectangle_extraction_status".to_string(),
            slide_rectangles_manifest
                .get("rectangle_extraction_status")
                .cloned()
                .unwrap_or_else(|| json!("waiting_for_selection")),
        );
        object.insert(
            "rectangle_extraction_mode".to_string(),
            slide_rectangles_manifest
                .get("rectangle_extraction_mode")
                .cloned()
                .unwrap_or_else(|| json!("full_frame_fallback")),
        );
        object.insert(
            "promoted_rectangle_count".to_string(),
            slide_rectangles_manifest
                .get("promoted_rectangle_count")
                .cloned()
                .unwrap_or_else(|| json!(0)),
        );
        object.insert(
            "dedupe_status".to_string(),
            slide_rectangles_manifest
                .get("dedupe_status")
                .cloned()
                .unwrap_or_else(|| json!("waiting_for_selection")),
        );
        object.insert(
            "deduped_candidate_count".to_string(),
            slide_rectangles_manifest
                .get("deduped_candidate_count")
                .cloned()
                .unwrap_or_else(|| json!(0)),
        );
        object.insert(
            "exact_duplicate_count".to_string(),
            slide_rectangles_manifest
                .get("exact_duplicate_count")
                .cloned()
                .unwrap_or_else(|| json!(0)),
        );
        object.insert(
            "visual_duplicate_count".to_string(),
            slide_rectangles_manifest
                .get("visual_duplicate_count")
                .cloned()
                .unwrap_or_else(|| json!(0)),
        );
    }

    let mut slide_quality_report_artifact = video_generated_artifact_file(
        document,
        "slide_quality_report",
        "application/json",
        &slide_quality_report_path,
    );
    if let Some(object) = slide_quality_report_artifact.as_object_mut() {
        object.insert(
            "status".to_string(),
            slide_quality_report
                .get("status")
                .cloned()
                .unwrap_or_else(|| json!("review_required")),
        );
        object.insert(
            "quality_score".to_string(),
            slide_quality_report
                .get("quality_score")
                .cloned()
                .unwrap_or_else(|| json!(0)),
        );
        object.insert(
            "risk_count".to_string(),
            slide_quality_report
                .get("risk_count")
                .cloned()
                .unwrap_or_else(|| json!(0)),
        );
        object.insert(
            "slide_count".to_string(),
            slide_quality_report
                .get("slide_count")
                .cloned()
                .unwrap_or_else(|| json!(0)),
        );
    }

    let mut slide_candidates_artifact = video_generated_artifact_file(
        document,
        "slide_image_candidates",
        "application/json",
        &candidate_manifest_path,
    );
    if let Some(object) = slide_candidates_artifact.as_object_mut() {
        object.insert(
            "candidate_count".to_string(),
            candidate_manifest
                .get("candidate_count")
                .cloned()
                .unwrap_or_else(|| json!(0)),
        );
        object.insert(
            "rectangle_extraction_status".to_string(),
            candidate_manifest
                .get("rectangle_extraction_status")
                .cloned()
                .unwrap_or_else(|| json!("not_promoted")),
        );
        object.insert("selection_status".to_string(), json!("review_required"));
    }

    let mut artifact_files = vec![
        slide_candidates_artifact,
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
        slide_rectangles_artifact,
        slide_quality_report_artifact,
        video_generated_artifact_file(document, "slide_notes", "text/markdown", &slide_notes_path),
        video_generated_artifact_file(
            document,
            "video_slides_markdown",
            "text/markdown",
            &video_slides_markdown_path,
        ),
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

fn video_frame_content_fingerprint(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    Some(format!("fnv1a64:{hash:016x}"))
}

#[derive(Clone, Debug)]
struct VideoFrameVisualSignature {
    fingerprint: String,
    samples: Vec<u8>,
}

#[derive(Clone, Debug)]
struct AcceptedVideoFrameVisualSignature {
    candidate_index: usize,
    file_name: String,
    signature: VideoFrameVisualSignature,
}

#[derive(Clone, Debug)]
struct VideoAutoSlideFrame {
    candidate_index: usize,
    file_name: String,
    signature: VideoFrameVisualSignature,
    sharpness_score: Option<i64>,
}

#[derive(Clone, Debug)]
struct VideoAutoSlideCluster {
    frames: Vec<VideoAutoSlideFrame>,
}

#[derive(Clone, Debug)]
struct VideoAutoSlideSelection {
    status: &'static str,
    selected_candidate_indices: Vec<usize>,
    selected_clusters: Vec<Value>,
    rejected_clusters: Vec<Value>,
    decodable_frame_count: usize,
    undecodable_frame_count: usize,
}

impl VideoAutoSlideSelection {
    fn manual_override_skipped() -> Self {
        Self {
            status: "manual_keep_list_present",
            selected_candidate_indices: Vec::new(),
            selected_clusters: Vec::new(),
            rejected_clusters: Vec::new(),
            decodable_frame_count: 0,
            undecodable_frame_count: 0,
        }
    }

    fn to_value(&self) -> Value {
        json!({
            "status": self.status,
            "selected_candidate_indices": self.selected_candidate_indices,
            "selected_page_count": self.selected_candidate_indices.len(),
            "selected_clusters": self.selected_clusters,
            "rejected_clusters": self.rejected_clusters,
            "decodable_frame_count": self.decodable_frame_count,
            "undecodable_frame_count": self.undecodable_frame_count,
            "cluster_policy": {
                "mode": "stable_ppt_page_segment_best_sharpness_middle_tiebreak",
                "visual_signature": format!("luma_{}x{}", VIDEO_VISUAL_SIGNATURE_GRID_SIZE, VIDEO_VISUAL_SIGNATURE_GRID_SIZE),
                "same_segment_max_avg_luma_diff": VIDEO_AUTO_SLIDE_SEGMENT_MAX_AVG_DIFF,
                "same_segment_changed_sample_min_diff": VIDEO_AUTO_SLIDE_SEGMENT_CHANGED_SAMPLE_MIN_DIFF,
                "same_segment_max_changed_sample_ratio": VIDEO_AUTO_SLIDE_SEGMENT_MAX_CHANGED_SAMPLE_RATIO,
                "min_stable_frames": VIDEO_AUTO_SLIDE_MIN_STABLE_FRAMES,
                "max_selected_pages": VIDEO_AUTO_SLIDE_MAX_SELECTED_PAGES,
                "ordinary_video_guard": "short unstable visual changes and dark, bright, or flat low-information stable segments are rejected instead of auto-selecting every frame"
            }
        })
    }
}

fn video_frame_visual_signature(path: &Path) -> Option<VideoFrameVisualSignature> {
    let image = ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?;
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return None;
    }

    let mut samples = Vec::with_capacity(
        (VIDEO_VISUAL_SIGNATURE_GRID_SIZE * VIDEO_VISUAL_SIGNATURE_GRID_SIZE) as usize,
    );
    for grid_y in 0..VIDEO_VISUAL_SIGNATURE_GRID_SIZE {
        for grid_x in 0..VIDEO_VISUAL_SIGNATURE_GRID_SIZE {
            let x = ((u64::from(grid_x) * u64::from(width))
                / u64::from(VIDEO_VISUAL_SIGNATURE_GRID_SIZE))
            .min(u64::from(width.saturating_sub(1))) as u32;
            let y = ((u64::from(grid_y) * u64::from(height))
                / u64::from(VIDEO_VISUAL_SIGNATURE_GRID_SIZE))
            .min(u64::from(height.saturating_sub(1))) as u32;
            let pixel = image.get_pixel(x, y).to_rgb();
            samples.push(video_luma_from_rgb(&pixel.0));
        }
    }

    Some(VideoFrameVisualSignature {
        fingerprint: video_visual_fingerprint_from_samples(&samples),
        samples,
    })
}

fn video_luma_from_rgb(pixel: &[u8; 3]) -> u8 {
    ((u32::from(pixel[0]) * 299 + u32::from(pixel[1]) * 587 + u32::from(pixel[2]) * 114) / 1000)
        as u8
}

fn video_visual_fingerprint_from_samples(samples: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for sample in samples {
        hash ^= u64::from(*sample / 4);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!(
        "luma{}x{}-fnv1a64:{hash:016x}",
        VIDEO_VISUAL_SIGNATURE_GRID_SIZE, VIDEO_VISUAL_SIGNATURE_GRID_SIZE
    )
}

fn video_visual_signature_avg_diff(
    left: &VideoFrameVisualSignature,
    right: &VideoFrameVisualSignature,
) -> Option<f64> {
    if left.samples.len() != right.samples.len() || left.samples.is_empty() {
        return None;
    }
    let total: u64 = left
        .samples
        .iter()
        .zip(right.samples.iter())
        .map(|(left, right)| u64::from(left.abs_diff(*right)))
        .sum();
    Some(total as f64 / left.samples.len() as f64)
}

fn video_visual_signature_changed_sample_ratio(
    left: &VideoFrameVisualSignature,
    right: &VideoFrameVisualSignature,
) -> Option<f64> {
    if left.samples.len() != right.samples.len() || left.samples.is_empty() {
        return None;
    }
    let changed_count = left
        .samples
        .iter()
        .zip(right.samples.iter())
        .filter(|(left, right)| {
            (**left).abs_diff(**right) >= VIDEO_AUTO_SLIDE_SEGMENT_CHANGED_SAMPLE_MIN_DIFF
        })
        .count();
    Some(changed_count as f64 / left.samples.len() as f64)
}

fn video_visual_signatures_same_auto_slide_segment(
    left: &VideoFrameVisualSignature,
    right: &VideoFrameVisualSignature,
) -> Option<bool> {
    let avg_diff = video_visual_signature_avg_diff(left, right)?;
    let changed_sample_ratio = video_visual_signature_changed_sample_ratio(left, right)?;
    Some(
        avg_diff <= VIDEO_AUTO_SLIDE_SEGMENT_MAX_AVG_DIFF
            && changed_sample_ratio <= VIDEO_AUTO_SLIDE_SEGMENT_MAX_CHANGED_SAMPLE_RATIO,
    )
}

fn video_visual_signature_luma_summary(
    signature: &VideoFrameVisualSignature,
) -> Option<(f64, u8, u8)> {
    let min_luma = *signature.samples.iter().min()?;
    let max_luma = *signature.samples.iter().max()?;
    let total: u64 = signature
        .samples
        .iter()
        .map(|sample| u64::from(*sample))
        .sum();
    let avg_luma = total as f64 / signature.samples.len() as f64;
    Some((video_round_similarity_score(avg_luma), min_luma, max_luma))
}

fn video_visual_signature_low_information_rejection(
    signature: &VideoFrameVisualSignature,
) -> Option<(&'static str, f64, u8)> {
    let (avg_luma, min_luma, max_luma) = video_visual_signature_luma_summary(signature)?;
    let luma_range = max_luma.saturating_sub(min_luma);
    if avg_luma <= VIDEO_AUTO_SLIDE_DARK_LOW_INFO_MAX_AVG_LUMA
        && luma_range <= VIDEO_AUTO_SLIDE_DARK_LOW_INFO_MAX_LUMA_RANGE
    {
        return Some(("dark_low_information_stable_segment", avg_luma, luma_range));
    }
    if avg_luma >= VIDEO_AUTO_SLIDE_BRIGHT_LOW_INFO_MIN_AVG_LUMA
        && luma_range <= VIDEO_AUTO_SLIDE_BRIGHT_LOW_INFO_MAX_LUMA_RANGE
    {
        return Some((
            "bright_low_information_stable_segment",
            avg_luma,
            luma_range,
        ));
    }
    if luma_range <= VIDEO_AUTO_SLIDE_FLAT_LOW_INFO_MAX_LUMA_RANGE {
        return Some(("flat_low_information_stable_segment", avg_luma, luma_range));
    }
    None
}

fn video_visual_near_duplicate_match<'a>(
    signature: &VideoFrameVisualSignature,
    accepted: &'a [AcceptedVideoFrameVisualSignature],
) -> Option<(&'a AcceptedVideoFrameVisualSignature, f64)> {
    accepted
        .iter()
        .filter_map(|candidate| {
            video_visual_signature_avg_diff(signature, &candidate.signature)
                .map(|score| (candidate, score))
        })
        .filter(|(_, score)| *score <= VIDEO_VISUAL_NEAR_DUPLICATE_MAX_AVG_DIFF)
        .min_by(|left, right| {
            left.1
                .partial_cmp(&right.1)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn video_visual_signature_shape_mask(signature: &VideoFrameVisualSignature) -> Option<Vec<bool>> {
    let (avg_luma, min_luma, max_luma) = video_visual_signature_luma_summary(signature)?;
    if avg_luma > VIDEO_VISUAL_SHAPE_DUPLICATE_MAX_AVG_LUMA {
        return None;
    }
    let luma_range = max_luma.saturating_sub(min_luma);
    if luma_range < 12 {
        return None;
    }
    let threshold = (avg_luma + f64::from(luma_range) * 0.2)
        .round()
        .clamp(0.0, 255.0) as u8;
    let mask = signature
        .samples
        .iter()
        .map(|sample| *sample >= threshold)
        .collect::<Vec<_>>();
    let signal_count = mask.iter().filter(|value| **value).count();
    if signal_count < VIDEO_VISUAL_SHAPE_DUPLICATE_MIN_SIGNAL_SAMPLES {
        return None;
    }
    Some(mask)
}

fn video_visual_signature_shape_similarity_score(
    left: &VideoFrameVisualSignature,
    right: &VideoFrameVisualSignature,
) -> Option<f64> {
    let left_mask = video_visual_signature_shape_mask(left)?;
    let right_mask = video_visual_signature_shape_mask(right)?;
    if left_mask.len() != right_mask.len() || left_mask.is_empty() {
        return None;
    }
    let mut intersection_count = 0_usize;
    let mut union_count = 0_usize;
    let mut left_signal_count = 0_usize;
    let mut right_signal_count = 0_usize;
    for (left, right) in left_mask.iter().zip(right_mask.iter()) {
        if *left {
            left_signal_count += 1;
        }
        if *right {
            right_signal_count += 1;
        }
        if *left || *right {
            union_count += 1;
        }
        if *left && *right {
            intersection_count += 1;
        }
    }
    if union_count < VIDEO_VISUAL_SHAPE_DUPLICATE_MIN_SIGNAL_SAMPLES {
        return None;
    }
    let jaccard = intersection_count as f64 / union_count as f64;
    if jaccard >= VIDEO_VISUAL_SHAPE_DUPLICATE_MIN_JACCARD {
        return Some(jaccard);
    }
    let min_signal_count = left_signal_count.min(right_signal_count);
    let max_signal_count = left_signal_count.max(right_signal_count);
    if min_signal_count < VIDEO_VISUAL_SHAPE_DUPLICATE_MIN_SIGNAL_SAMPLES || max_signal_count == 0 {
        return None;
    }
    let containment = intersection_count as f64 / min_signal_count as f64;
    let signal_balance = min_signal_count as f64 / max_signal_count as f64;
    if containment >= VIDEO_VISUAL_SHAPE_DUPLICATE_MIN_CONTAINMENT
        && signal_balance >= VIDEO_VISUAL_SHAPE_DUPLICATE_MIN_SIGNAL_BALANCE
    {
        return Some(containment);
    }
    Some(jaccard)
}

fn video_visual_shape_duplicate_match<'a>(
    signature: &VideoFrameVisualSignature,
    accepted: &'a [AcceptedVideoFrameVisualSignature],
) -> Option<(&'a AcceptedVideoFrameVisualSignature, f64)> {
    accepted
        .iter()
        .filter_map(|candidate| {
            video_visual_signature_shape_similarity_score(signature, &candidate.signature)
                .map(|score| (candidate, score))
        })
        .filter(|(_, score)| *score >= VIDEO_VISUAL_SHAPE_DUPLICATE_MIN_JACCARD)
        .max_by(|left, right| {
            left.1
                .partial_cmp(&right.1)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn video_auto_slide_selection_from_frames(
    frames: &[PathBuf],
    frame_extraction: &Value,
) -> VideoAutoSlideSelection {
    let mut selected_candidate_indices = Vec::<usize>::new();
    let mut selected_clusters = Vec::<Value>::new();
    let mut rejected_clusters = Vec::<Value>::new();
    let mut decodable_frame_count = 0_usize;
    let mut undecodable_frame_count = 0_usize;
    let mut current_cluster: Option<VideoAutoSlideCluster> = None;
    let interval_seconds = video_frame_interval_seconds(frame_extraction);

    for (index, frame) in frames.iter().enumerate() {
        let candidate_index = index + 1;
        let file_name = frame
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("frame")
            .to_string();
        let Some(signature) = video_frame_visual_signature(frame) else {
            undecodable_frame_count += 1;
            continue;
        };
        decodable_frame_count += 1;
        let next_frame = VideoAutoSlideFrame {
            candidate_index,
            file_name,
            signature,
            sharpness_score: video_slide_sharpness_score(frame),
        };

        let belongs_to_current = current_cluster
            .as_ref()
            .and_then(|cluster| cluster.frames.first())
            .and_then(|first| {
                video_visual_signatures_same_auto_slide_segment(
                    &next_frame.signature,
                    &first.signature,
                )
            })
            .unwrap_or(false);

        if belongs_to_current {
            if let Some(cluster) = current_cluster.as_mut() {
                cluster.frames.push(next_frame);
            }
            continue;
        }

        if let Some(cluster) = current_cluster.take() {
            video_finalize_auto_slide_cluster(
                cluster,
                interval_seconds,
                &mut selected_candidate_indices,
                &mut selected_clusters,
                &mut rejected_clusters,
            );
        }
        current_cluster = Some(VideoAutoSlideCluster {
            frames: vec![next_frame],
        });
    }

    if let Some(cluster) = current_cluster.take() {
        video_finalize_auto_slide_cluster(
            cluster,
            interval_seconds,
            &mut selected_candidate_indices,
            &mut selected_clusters,
            &mut rejected_clusters,
        );
    }

    if selected_candidate_indices.len() > VIDEO_AUTO_SLIDE_MAX_SELECTED_PAGES {
        let overflow = selected_candidate_indices.split_off(VIDEO_AUTO_SLIDE_MAX_SELECTED_PAGES);
        rejected_clusters.push(json!({
            "reason": "auto_selected_page_limit_exceeded",
            "candidate_indices": overflow,
            "max_selected_pages": VIDEO_AUTO_SLIDE_MAX_SELECTED_PAGES,
        }));
        selected_clusters.truncate(VIDEO_AUTO_SLIDE_MAX_SELECTED_PAGES);
    }

    let status = if !selected_candidate_indices.is_empty() {
        "auto_selected"
    } else if decodable_frame_count == 0 {
        "not_applicable_no_decodable_frames"
    } else {
        "review_required_no_stable_ppt_page_segments"
    };

    VideoAutoSlideSelection {
        status,
        selected_candidate_indices,
        selected_clusters,
        rejected_clusters,
        decodable_frame_count,
        undecodable_frame_count,
    }
}

fn video_finalize_auto_slide_cluster(
    cluster: VideoAutoSlideCluster,
    interval_seconds: f64,
    selected_candidate_indices: &mut Vec<usize>,
    selected_clusters: &mut Vec<Value>,
    rejected_clusters: &mut Vec<Value>,
) {
    if cluster.frames.is_empty() {
        return;
    }
    let first = cluster.frames.first().expect("cluster first frame");
    let last = cluster.frames.last().expect("cluster last frame");
    let frame_count = cluster.frames.len();
    if frame_count < VIDEO_AUTO_SLIDE_MIN_STABLE_FRAMES {
        rejected_clusters.push(json!({
            "reason": "unstable_short_segment",
            "candidate_count": frame_count,
            "first_candidate_index": first.candidate_index,
            "last_candidate_index": last.candidate_index,
            "first_file_name": first.file_name,
            "last_file_name": last.file_name,
            "min_stable_frames": VIDEO_AUTO_SLIDE_MIN_STABLE_FRAMES,
        }));
        return;
    }

    let (selected_frame, selection_rule) = video_select_auto_slide_cluster_frame(&cluster);
    if let Some((reason, avg_luma, luma_range)) =
        video_visual_signature_low_information_rejection(&selected_frame.signature)
    {
        rejected_clusters.push(json!({
            "reason": reason,
            "candidate_count": frame_count,
            "first_candidate_index": first.candidate_index,
            "last_candidate_index": last.candidate_index,
            "selected_candidate_index": selected_frame.candidate_index,
            "selected_file_name": selected_frame.file_name,
            "selected_sharpness_score": selected_frame.sharpness_score,
            "avg_luma": avg_luma,
            "luma_range": luma_range,
            "dark_max_avg_luma": VIDEO_AUTO_SLIDE_DARK_LOW_INFO_MAX_AVG_LUMA,
            "dark_max_luma_range": VIDEO_AUTO_SLIDE_DARK_LOW_INFO_MAX_LUMA_RANGE,
            "bright_min_avg_luma": VIDEO_AUTO_SLIDE_BRIGHT_LOW_INFO_MIN_AVG_LUMA,
            "bright_max_luma_range": VIDEO_AUTO_SLIDE_BRIGHT_LOW_INFO_MAX_LUMA_RANGE,
            "flat_max_luma_range": VIDEO_AUTO_SLIDE_FLAT_LOW_INFO_MAX_LUMA_RANGE,
        }));
        return;
    }
    selected_candidate_indices.push(selected_frame.candidate_index);
    selected_clusters.push(json!({
        "status": "stable_ppt_page_segment",
        "candidate_count": frame_count,
        "first_candidate_index": first.candidate_index,
        "last_candidate_index": last.candidate_index,
        "selected_candidate_index": selected_frame.candidate_index,
        "selected_file_name": selected_frame.file_name,
        "selected_sharpness_score": selected_frame.sharpness_score,
        "duration_seconds_estimate": video_round_similarity_score(frame_count as f64 * interval_seconds),
        "selection_rule": selection_rule,
    }));
}

fn video_select_auto_slide_cluster_frame(
    cluster: &VideoAutoSlideCluster,
) -> (&VideoAutoSlideFrame, &'static str) {
    let midpoint = cluster.frames.len() / 2;
    let selected = cluster
        .frames
        .iter()
        .enumerate()
        .min_by_key(|(index, frame)| {
            (
                std::cmp::Reverse(frame.sharpness_score.unwrap_or(-1)),
                index.abs_diff(midpoint),
                frame.candidate_index,
            )
        })
        .map(|(_, frame)| frame)
        .unwrap_or_else(|| {
            cluster
                .frames
                .get(midpoint)
                .expect("cluster selected frame")
        });
    let midpoint_frame = cluster
        .frames
        .get(midpoint)
        .expect("cluster midpoint frame");
    let selection_rule = if selected.candidate_index == midpoint_frame.candidate_index {
        "middle_frame_of_stable_visual_segment"
    } else {
        "highest_sharpness_frame_of_stable_visual_segment"
    };
    (selected, selection_rule)
}

fn video_round_similarity_score(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn video_candidate_nearby_evidence_refs(
    timestamp_seconds: f64,
    evidence: &VideoMediaEvidenceItems,
    frame_extraction: &Value,
) -> Value {
    let window_seconds = video_frame_interval_seconds(frame_extraction).max(1.0);
    json!({
        "policy": "timestamp_nearby_with_closest_fallback",
        "window_seconds": window_seconds,
        "scene_refs": video_candidate_scene_refs(timestamp_seconds, &evidence.scenes, window_seconds),
        "transcript_refs": video_candidate_transcript_refs(timestamp_seconds, &evidence.transcript_segments, window_seconds),
        "ocr_refs": video_candidate_ocr_refs(timestamp_seconds, &evidence.keyframe_ocr_snippets, window_seconds),
    })
}

fn video_evidence_ref_count(value: &Value) -> usize {
    ["scene_refs", "transcript_refs", "ocr_refs"]
        .iter()
        .map(|key| {
            value
                .get(*key)
                .and_then(Value::as_array)
                .map(Vec::len)
                .unwrap_or(0)
        })
        .sum()
}

fn video_candidate_scene_refs(
    timestamp_seconds: f64,
    scenes: &[Value],
    window_seconds: f64,
) -> Vec<Value> {
    let mut ranked = scenes
        .iter()
        .enumerate()
        .filter_map(|(index, scene)| {
            video_range_distance_seconds(scene, timestamp_seconds)
                .map(|distance| (distance, index, scene))
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| video_ranked_distance_order(left.0, left.1, right.0, right.1));

    let mut refs = ranked
        .iter()
        .filter(|(distance, _, scene)| {
            *distance <= window_seconds || video_range_contains_timestamp(scene, timestamp_seconds)
        })
        .take(3)
        .map(|(distance, index, scene)| {
            let match_rule = if video_range_contains_timestamp(scene, timestamp_seconds) {
                "overlaps_timestamp"
            } else {
                "nearby_timestamp"
            };
            video_candidate_evidence_ref(
                scene,
                "scene",
                *index + 1,
                Some(*distance),
                match_rule,
                &["summary", "text", "description"],
            )
        })
        .collect::<Vec<_>>();

    if refs.is_empty() {
        if let Some((distance, index, scene)) = ranked.first() {
            refs.push(video_candidate_evidence_ref(
                scene,
                "scene",
                *index + 1,
                Some(*distance),
                "closest_available",
                &["summary", "text", "description"],
            ));
        } else if let Some((index, scene)) = scenes.iter().enumerate().next() {
            refs.push(video_candidate_evidence_ref(
                scene,
                "scene",
                index + 1,
                None,
                "untimed_available",
                &["summary", "text", "description"],
            ));
        }
    }

    refs
}

fn video_candidate_transcript_refs(
    timestamp_seconds: f64,
    transcript_segments: &[Value],
    window_seconds: f64,
) -> Vec<Value> {
    let mut ranked = transcript_segments
        .iter()
        .enumerate()
        .filter_map(|(index, segment)| {
            video_range_distance_seconds(segment, timestamp_seconds)
                .map(|distance| (distance, index, segment))
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| video_ranked_distance_order(left.0, left.1, right.0, right.1));

    let mut refs = ranked
        .iter()
        .filter(|(distance, _, segment)| {
            *distance <= window_seconds
                || video_range_contains_timestamp(segment, timestamp_seconds)
        })
        .take(3)
        .map(|(distance, index, segment)| {
            let match_rule = if video_range_contains_timestamp(segment, timestamp_seconds) {
                "overlaps_timestamp"
            } else {
                "nearby_timestamp"
            };
            video_candidate_evidence_ref(
                segment,
                "transcript",
                *index + 1,
                Some(*distance),
                match_rule,
                &["text", "content", "summary"],
            )
        })
        .collect::<Vec<_>>();

    if refs.is_empty() {
        if let Some((distance, index, segment)) = ranked.first() {
            refs.push(video_candidate_evidence_ref(
                segment,
                "transcript",
                *index + 1,
                Some(*distance),
                "closest_available",
                &["text", "content", "summary"],
            ));
        } else if let Some((index, segment)) = transcript_segments.iter().enumerate().next() {
            refs.push(video_candidate_evidence_ref(
                segment,
                "transcript",
                index + 1,
                None,
                "untimed_available",
                &["text", "content", "summary"],
            ));
        }
    }

    refs
}

fn video_candidate_ocr_refs(
    timestamp_seconds: f64,
    ocr_snippets: &[Value],
    window_seconds: f64,
) -> Vec<Value> {
    let mut ranked = ocr_snippets
        .iter()
        .enumerate()
        .filter_map(|(index, snippet)| {
            video_timestamp_distance_seconds(snippet, timestamp_seconds)
                .map(|distance| (distance, index, snippet))
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| video_ranked_distance_order(left.0, left.1, right.0, right.1));

    let mut refs = ranked
        .iter()
        .filter(|(distance, _, _)| *distance <= window_seconds)
        .take(3)
        .map(|(distance, index, snippet)| {
            video_candidate_evidence_ref(
                snippet,
                "ocr",
                *index + 1,
                Some(*distance),
                "nearby_timestamp",
                &["text", "ocr_text", "summary"],
            )
        })
        .collect::<Vec<_>>();

    if refs.is_empty() {
        if let Some((distance, index, snippet)) = ranked.first() {
            refs.push(video_candidate_evidence_ref(
                snippet,
                "ocr",
                *index + 1,
                Some(*distance),
                "closest_available",
                &["text", "ocr_text", "summary"],
            ));
        } else if let Some((index, snippet)) = ocr_snippets.iter().enumerate().next() {
            refs.push(video_candidate_evidence_ref(
                snippet,
                "ocr",
                index + 1,
                None,
                "untimed_available",
                &["text", "ocr_text", "summary"],
            ));
        }
    }

    refs
}

fn video_candidate_evidence_ref(
    value: &Value,
    kind: &str,
    index: usize,
    distance_seconds: Option<f64>,
    match_rule: &str,
    text_keys: &[&str],
) -> Value {
    let text = video_item_text(value, text_keys)
        .map(|text| video_safe_evidence_text(&text))
        .unwrap_or_else(|| "unknown".to_string());
    let time_label = if kind == "ocr" {
        video_timestamp_label(value)
    } else {
        video_time_range_label(value)
    };
    json!({
        "kind": kind,
        "index": index,
        "reference": video_evidence_reference_label(value, kind, index),
        "time_label": time_label,
        "match": match_rule,
        "distance_seconds": distance_seconds.map(video_rounded_distance_seconds),
        "text": text,
    })
}

fn video_range_distance_seconds(value: &Value, timestamp_seconds: f64) -> Option<f64> {
    let start = video_number_field(value, &["start_seconds", "startSeconds", "start"]);
    let end = video_number_field(value, &["end_seconds", "endSeconds", "end"]);
    match (start, end) {
        (Some(start), Some(end)) if timestamp_seconds >= start && timestamp_seconds <= end => {
            Some(0.0)
        }
        (Some(start), Some(_end)) if timestamp_seconds < start => Some(start - timestamp_seconds),
        (Some(_), Some(end)) => Some(timestamp_seconds - end),
        (Some(start), None) => Some((timestamp_seconds - start).abs()),
        (None, Some(end)) => Some((timestamp_seconds - end).abs()),
        (None, None) => None,
    }
}

fn video_range_contains_timestamp(value: &Value, timestamp_seconds: f64) -> bool {
    let start = video_number_field(value, &["start_seconds", "startSeconds", "start"]);
    let end = video_number_field(value, &["end_seconds", "endSeconds", "end"]);
    matches!(
        (start, end),
        (Some(start), Some(end)) if timestamp_seconds >= start && timestamp_seconds <= end
    )
}

fn video_timestamp_distance_seconds(value: &Value, timestamp_seconds: f64) -> Option<f64> {
    video_number_field(
        value,
        &[
            "timestamp_seconds",
            "timestampSeconds",
            "time_seconds",
            "timeSeconds",
        ],
    )
    .map(|timestamp| (timestamp_seconds - timestamp).abs())
}

fn video_ranked_distance_order(
    left_distance: f64,
    left_index: usize,
    right_distance: f64,
    right_index: usize,
) -> std::cmp::Ordering {
    left_distance
        .partial_cmp(&right_distance)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| left_index.cmp(&right_index))
}

fn video_rounded_distance_seconds(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
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
    selection_source: &str,
    auto_selection: &Value,
) -> Value {
    let mut previous_timestamp_seconds = 0.0_f64;
    let mut selected_candidates = Vec::<Value>::new();
    let mut final_selected_candidate_indices = Vec::<usize>::new();
    let mut rejected_duplicate_candidates = Vec::<Value>::new();
    let mut seen_content_fingerprints = BTreeSet::<String>::new();
    let mut seen_visual_signatures = Vec::<AcceptedVideoFrameVisualSignature>::new();
    for candidate_index in selected_candidate_indices {
        let Some(frame) = frames.get(candidate_index.saturating_sub(1)) else {
            continue;
        };
        let file_name = frame
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("frame");
        let timestamp_seconds =
            video_candidate_timestamp_seconds(*candidate_index, frame_extraction);
        let content_fingerprint = video_frame_content_fingerprint(frame);
        if let Some(fingerprint) = content_fingerprint.as_ref() {
            if seen_content_fingerprints.contains(fingerprint) {
                rejected_duplicate_candidates.push(json!({
                    "candidate_index": candidate_index,
                    "file_name": file_name,
                    "timestamp_seconds": timestamp_seconds,
                    "timestamp_label": format_seconds(timestamp_seconds),
                    "dedupe_reason": "exact_frame_content_duplicate",
                    "content_fingerprint": fingerprint,
                }));
                continue;
            }
        }
        let visual_signature = video_frame_visual_signature(frame);
        if let Some(signature) = visual_signature.as_ref() {
            if let Some((matched, score)) =
                video_visual_near_duplicate_match(signature, &seen_visual_signatures)
            {
                rejected_duplicate_candidates.push(json!({
                    "candidate_index": candidate_index,
                    "file_name": file_name,
                    "timestamp_seconds": timestamp_seconds,
                    "timestamp_label": format_seconds(timestamp_seconds),
                    "dedupe_reason": "visual_near_duplicate",
                    "visual_fingerprint": signature.fingerprint,
                    "matched_candidate_index": matched.candidate_index,
                    "matched_file_name": matched.file_name,
                    "matched_visual_fingerprint": matched.signature.fingerprint,
                    "visual_similarity_score": video_round_similarity_score(score),
                    "visual_similarity_threshold": VIDEO_VISUAL_NEAR_DUPLICATE_MAX_AVG_DIFF,
                }));
                continue;
            }
            if let Some((matched, score)) =
                video_visual_shape_duplicate_match(signature, &seen_visual_signatures)
            {
                rejected_duplicate_candidates.push(json!({
                    "candidate_index": candidate_index,
                    "file_name": file_name,
                    "timestamp_seconds": timestamp_seconds,
                    "timestamp_label": format_seconds(timestamp_seconds),
                    "dedupe_reason": "visual_shape_duplicate",
                    "visual_fingerprint": signature.fingerprint,
                    "matched_candidate_index": matched.candidate_index,
                    "matched_file_name": matched.file_name,
                    "matched_visual_fingerprint": matched.signature.fingerprint,
                    "visual_shape_similarity_score": video_round_similarity_score(score),
                    "visual_shape_similarity_threshold": VIDEO_VISUAL_SHAPE_DUPLICATE_MIN_JACCARD,
                }));
                continue;
            }
        }
        let slide_index = selected_candidates.len();
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
        let ocr_snippets = video_ocr_snippets_for_window(
            &evidence.keyframe_ocr_snippets,
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
        let ocr_alignment_status = if evidence.keyframe_ocr_snippets.is_empty() {
            "missing_ocr"
        } else if ocr_snippets.is_empty() {
            "unmatched"
        } else {
            "window_mapped"
        };
        let slide_rectangle = video_slide_rectangle_from_frame(
            slide_index + 1,
            *candidate_index,
            file_name,
            timestamp_seconds,
            frame,
        );
        let dedupe_fingerprint_status = if content_fingerprint.is_some() {
            "available"
        } else {
            "unavailable"
        };
        if let Some(fingerprint) = content_fingerprint.as_ref() {
            seen_content_fingerprints.insert(fingerprint.clone());
        }
        let visual_fingerprint_status = if visual_signature.is_some() {
            "available"
        } else {
            "unavailable"
        };
        if let Some(signature) = visual_signature.as_ref() {
            seen_visual_signatures.push(AcceptedVideoFrameVisualSignature {
                candidate_index: *candidate_index,
                file_name: file_name.to_string(),
                signature: signature.clone(),
            });
        }
        final_selected_candidate_indices.push(*candidate_index);
        selected_candidates.push(json!({
            "candidate_index": candidate_index,
            "file_name": file_name,
            "frame_path": frame.display().to_string(),
            "timestamp_seconds": timestamp_seconds,
            "timestamp_label": format_seconds(timestamp_seconds),
            "content_fingerprint": content_fingerprint,
            "dedupe_fingerprint_status": dedupe_fingerprint_status,
            "visual_fingerprint": visual_signature.as_ref().map(|signature| signature.fingerprint.clone()),
            "visual_fingerprint_status": visual_fingerprint_status,
            "transcript_window": {
                "start_seconds": window_start_seconds,
                "end_seconds": window_end_seconds,
                "assignment_rule": "pre_page_previous_to_current",
            },
            "subtitle_alignment_status": subtitle_alignment_status,
            "transcript_segments": transcript_segments,
            "ocr_alignment_status": ocr_alignment_status,
            "ocr_snippets": ocr_snippets,
            "rectangle_extraction_status": "promoted_full_frame_fallback",
            "slide_rectangle": slide_rectangle,
            "contact_sheet_anchor": format!("candidate-{candidate_index}"),
            "selection_status": "selected",
        }));
    }
    let rectangle_extraction_status = video_slide_rectangle_aggregate_status(&selected_candidates);
    let rectangle_extraction_mode = video_slide_rectangle_aggregate_mode(&selected_candidates);
    let exact_duplicate_count = rejected_duplicate_candidates
        .iter()
        .filter(|candidate| {
            candidate.get("dedupe_reason").and_then(Value::as_str)
                == Some("exact_frame_content_duplicate")
        })
        .count();
    let visual_duplicate_count = rejected_duplicate_candidates
        .iter()
        .filter(|candidate| {
            matches!(
                candidate.get("dedupe_reason").and_then(Value::as_str),
                Some("visual_near_duplicate") | Some("visual_shape_duplicate")
            )
        })
        .count();
    let visual_shape_duplicate_count = rejected_duplicate_candidates
        .iter()
        .filter(|candidate| {
            candidate.get("dedupe_reason").and_then(Value::as_str) == Some("visual_shape_duplicate")
        })
        .count();
    let dedupe_status = if selected_candidates.is_empty() {
        "waiting_for_selection"
    } else if visual_duplicate_count > 0 && exact_duplicate_count > 0 {
        "exact_and_visual_similarity_deduped"
    } else if visual_duplicate_count > 0 {
        "visual_similarity_deduped"
    } else if exact_duplicate_count > 0 {
        "exact_frame_content_deduped"
    } else {
        "selected_keep_list_order_deduped"
    };

    json!({
        "status": if selected_candidates.is_empty() { "waiting_for_selection" } else { "ready_for_pptx_writer" },
        "source": selection_source,
        "document_id": document.id.to_string(),
        "dataset_id": document.dataset_id.to_string(),
        "title": document.title,
        "candidate_manifest": candidate_manifest_path.display().to_string(),
        "contact_sheet_html": contact_sheet_html_path.display().to_string(),
        "keep_list_template": keep_list_template_path.display().to_string(),
        "selection_source": selection_source,
        "auto_selection": auto_selection,
        "selected_candidate_indices": final_selected_candidate_indices,
        "requested_selected_candidate_indices": selected_candidate_indices,
        "requested_selected_count": selected_candidate_indices.len(),
        "selected_count": selected_candidates.len(),
        "deduped_candidate_count": rejected_duplicate_candidates.len(),
        "exact_duplicate_count": exact_duplicate_count,
        "visual_duplicate_count": visual_duplicate_count,
        "visual_shape_duplicate_count": visual_shape_duplicate_count,
        "rectangle_extraction_status": rectangle_extraction_status,
        "rectangle_extraction_mode": rectangle_extraction_mode,
        "dedupe_status": dedupe_status,
        "dedupe_policy": "selected candidate indices are range-checked, order-preserved, de-duplicated by index; exact duplicate frame bytes, conservative visual near-duplicates, and high-confidence contrast-normalized shape duplicates are removed before rectangle promotion",
        "rectangle_policy": "promote selected raw frames with conservative visual detectors when possible; otherwise use full-frame relative rectangles that require review",
        "selected_candidates": selected_candidates,
        "rejected_duplicate_candidates": rejected_duplicate_candidates,
        "next_step": if selected_candidates.is_empty() {
            "fill ppt_keep_list_template.json from the numbered contact sheet"
        } else if selection_source == "auto_unique_slide_keyframes" {
            "review auto-selected PPT page frames, slide_rectangles_manifest.json, and generated screenshot PPTX before customer delivery"
        } else {
            "review slide_rectangles_manifest.json and build screenshot-based PPTX from selected_candidates only"
        },
    })
}

fn video_slide_quality_report_from_manifests(
    document: &Document,
    selected_slides_manifest: &Value,
    slide_rectangles_manifest: &Value,
    subtitle_page_map: &Value,
) -> Value {
    let selected_candidates = selected_slides_manifest
        .get("selected_candidates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let rectangles = slide_rectangles_manifest
        .get("rectangles")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let subtitle_pages = subtitle_page_map
        .get("pages")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let subtitle_map_status = subtitle_page_map
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("not_mapped");
    let deduped_candidate_count = selected_slides_manifest
        .get("deduped_candidate_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let exact_duplicate_count = selected_slides_manifest
        .get("exact_duplicate_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let visual_duplicate_count = selected_slides_manifest
        .get("visual_duplicate_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let visual_shape_duplicate_count = selected_slides_manifest
        .get("visual_shape_duplicate_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);

    let mut full_frame_fallback_count = 0_u64;
    let mut detector_crop_count = 0_u64;
    let mut review_required_count = 0_u64;
    let mut subtitle_mapped_count = 0_u64;
    let mut subtitle_missing_count = 0_u64;
    let mut ocr_mapped_count = 0_u64;
    let mut ocr_missing_count = 0_u64;
    let mut sharpness_low_count = 0_u64;
    let mut sharpness_medium_count = 0_u64;
    let mut sharpness_high_count = 0_u64;
    let mut sharpness_unknown_count = 0_u64;
    let mut slide_scores = Vec::<i64>::new();
    let slides = selected_candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            let slide_number = index + 1;
            let candidate_index = candidate
                .get("candidate_index")
                .and_then(Value::as_u64)
                .unwrap_or(slide_number as u64);
            let rectangle = rectangles.iter().find(|rectangle| {
                rectangle
                    .get("slide_number")
                    .and_then(Value::as_u64)
                    .is_some_and(|value| value == slide_number as u64)
                    || rectangle
                        .get("candidate_index")
                        .and_then(Value::as_u64)
                        .is_some_and(|value| value == candidate_index)
            });
            let rectangle_status = rectangle
                .and_then(|rectangle| rectangle.get("rectangle_extraction_status"))
                .and_then(Value::as_str)
                .or_else(|| {
                    candidate
                        .get("rectangle_extraction_status")
                        .and_then(Value::as_str)
                })
                .unwrap_or("missing_rectangle");
            let rectangle_mode = rectangle
                .and_then(|rectangle| rectangle.get("rectangle_extraction_mode"))
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let review_required = rectangle
                .and_then(|rectangle| rectangle.get("review_required"))
                .and_then(Value::as_bool)
                .unwrap_or(true);
            if rectangle_status == "promoted_full_frame_fallback" {
                full_frame_fallback_count += 1;
            }
            if rectangle_status == "promoted_detector_crop" {
                detector_crop_count += 1;
            }
            if review_required {
                review_required_count += 1;
            }
            let transcript_segment_count = candidate
                .get("transcript_segments")
                .and_then(Value::as_array)
                .map(|segments| segments.len())
                .unwrap_or(0);
            let subtitle_alignment_status = candidate
                .get("subtitle_alignment_status")
                .and_then(Value::as_str)
                .unwrap_or(if subtitle_map_status == "mapped" {
                    "mapped"
                } else {
                    "missing_transcript"
                });
            let page_has_subtitle_segments = subtitle_pages.iter().any(|page| {
                page.get("slide_number")
                    .and_then(Value::as_u64)
                    .is_some_and(|value| value == slide_number as u64)
                    && page
                        .get("transcript_segments")
                        .and_then(Value::as_array)
                        .is_some_and(|segments| !segments.is_empty())
            });
            let transcript_ready = transcript_segment_count > 0 || page_has_subtitle_segments;
            if transcript_ready {
                subtitle_mapped_count += 1;
            } else {
                subtitle_missing_count += 1;
            }
            let ocr_snippet_count = candidate
                .get("ocr_snippets")
                .and_then(Value::as_array)
                .map(|snippets| snippets.len())
                .unwrap_or(0);
            let ocr_alignment_status = candidate
                .get("ocr_alignment_status")
                .and_then(Value::as_str)
                .unwrap_or("missing_ocr");
            if ocr_snippet_count > 0 {
                ocr_mapped_count += 1;
            } else {
                ocr_missing_count += 1;
            }
            let crop_risk = if rectangle_status == "promoted_full_frame_fallback" {
                "high"
            } else if review_required {
                "medium"
            } else {
                "low"
            };
            let transcript_risk = if transcript_ready {
                "low"
            } else if subtitle_alignment_status == "unmatched" {
                "medium"
            } else {
                "high"
            };
            let ocr_risk = if ocr_snippet_count > 0 {
                "low"
            } else if ocr_alignment_status == "unmatched" {
                "medium"
            } else {
                "high"
            };
            let sharpness = video_slide_sharpness_assessment_from_candidate(candidate);
            match sharpness.risk {
                "low" => sharpness_low_count += 1,
                "medium" => sharpness_medium_count += 1,
                "high" => sharpness_high_count += 1,
                _ => sharpness_unknown_count += 1,
            }
            let mut score = 100_i64;
            if crop_risk == "high" {
                score -= 25;
            } else if crop_risk == "medium" {
                score -= 10;
            }
            if transcript_risk == "high" {
                score -= 15;
            } else if transcript_risk == "medium" {
                score -= 8;
            }
            if ocr_risk == "medium" {
                score -= 3;
            }
            if sharpness.risk == "high" {
                score -= 12;
            } else if sharpness.risk == "medium" {
                score -= 5;
            } else if sharpness.risk == "unknown" {
                score -= 4;
            }
            if review_required {
                score -= 5;
            }
            let score = score.max(0);
            slide_scores.push(score);
            json!({
                "slide_number": slide_number,
                "candidate_index": candidate_index,
                "source_frame": candidate.get("file_name").cloned().unwrap_or_else(|| json!("frame")),
                "timestamp_label": candidate.get("timestamp_label").cloned().unwrap_or(Value::Null),
                "rectangle_extraction_status": rectangle_status,
                "rectangle_extraction_mode": rectangle_mode,
                "crop_risk": crop_risk,
                "subtitle_alignment_status": subtitle_alignment_status,
                "transcript_segment_count": transcript_segment_count,
                "transcript_risk": transcript_risk,
                "ocr_alignment_status": ocr_alignment_status,
                "ocr_snippet_count": ocr_snippet_count,
                "ocr_risk": ocr_risk,
                "sharpness_status": sharpness.status,
                "sharpness_score": sharpness.score,
                "sharpness_risk": sharpness.risk,
                "review_required": review_required,
                "quality_score": score,
            })
        })
        .collect::<Vec<_>>();

    let slide_count = slides.len();
    let quality_score = if slide_scores.is_empty() {
        0
    } else {
        (slide_scores.iter().sum::<i64>() as f64 / slide_scores.len() as f64).round() as i64
    };
    let mut risk_flags = Vec::<Value>::new();
    if full_frame_fallback_count > 0 {
        risk_flags.push(json!({
            "code": "full_frame_rectangle_fallback",
            "severity": "medium",
            "count": full_frame_fallback_count,
            "review_action": "review_or_replace_full_frame_crops",
        }));
    }
    if subtitle_missing_count > 0 {
        risk_flags.push(json!({
            "code": "missing_transcript_alignment",
            "severity": "medium",
            "count": subtitle_missing_count,
            "review_action": "attach_or_parse_transcript_evidence",
        }));
    }
    if ocr_missing_count > 0 {
        risk_flags.push(json!({
            "code": "missing_ocr_evidence",
            "severity": "low",
            "count": ocr_missing_count,
            "review_action": "run_or_review_ocr_evidence_before_customer_delivery",
        }));
    }
    if deduped_candidate_count > 0 {
        risk_flags.push(json!({
            "code": "selected_slide_duplicates_removed",
            "severity": "low",
            "count": deduped_candidate_count,
            "exact_duplicate_count": exact_duplicate_count,
            "visual_duplicate_count": visual_duplicate_count,
            "review_action": "review_slide_dedupe_manifest",
        }));
    }
    if sharpness_high_count > 0 || sharpness_unknown_count > 0 {
        risk_flags.push(json!({
            "code": "frame_sharpness_review_required",
            "severity": "medium",
            "count": sharpness_high_count + sharpness_unknown_count,
            "high_count": sharpness_high_count,
            "unknown_count": sharpness_unknown_count,
            "review_action": "review_blurry_or_unmeasured_slide_frames",
        }));
    }
    if slide_count == 1 {
        risk_flags.push(json!({
            "code": "single_slide_output_review_required",
            "severity": "medium",
            "count": 1,
            "review_action": "confirm_video_contains_only_one_ppt_or_reprocess_with_more_coverage",
        }));
    }
    if review_required_count > 0 {
        risk_flags.push(json!({
            "code": "manual_review_required",
            "severity": "medium",
            "count": review_required_count,
            "review_action": "review_slide_quality_report",
        }));
    }

    json!({
        "schema": "v3.video_ppt_slide_quality_report.v1",
        "status": if slide_count == 0 { "waiting_for_selection" } else { "review_required" },
        "source": "selected_slides_and_slide_rectangles",
        "document_id": document.id.to_string(),
        "dataset_id": document.dataset_id.to_string(),
        "title": document.title,
        "quality_score": quality_score,
        "slide_count": slide_count,
        "risk_count": risk_flags.len(),
        "summary": {
            "full_frame_fallback_count": full_frame_fallback_count,
            "detector_crop_count": detector_crop_count,
            "review_required_count": review_required_count,
            "subtitle_mapped_count": subtitle_mapped_count,
            "subtitle_missing_count": subtitle_missing_count,
            "ocr_mapped_count": ocr_mapped_count,
            "ocr_missing_count": ocr_missing_count,
            "sharpness_low_count": sharpness_low_count,
            "sharpness_medium_count": sharpness_medium_count,
            "sharpness_high_count": sharpness_high_count,
            "sharpness_unknown_count": sharpness_unknown_count,
            "deduped_candidate_count": deduped_candidate_count,
            "exact_duplicate_count": exact_duplicate_count,
            "visual_duplicate_count": visual_duplicate_count,
            "visual_shape_duplicate_count": visual_shape_duplicate_count,
            "single_slide_output": slide_count == 1,
        },
        "risk_flags": risk_flags,
        "slides": slides,
        "redaction": {
            "status": "applied",
            "policy": "source frames are file names only; internal paths and source URLs are redacted"
        },
        "next_action": if slide_count == 0 {
            "fill_ppt_keep_list_template"
        } else {
            "review_slide_quality_report_before_customer_delivery"
        },
    })
}

#[derive(Clone, Debug)]
struct VideoSlideSharpnessAssessment {
    status: &'static str,
    score: Option<i64>,
    risk: &'static str,
}

impl VideoSlideSharpnessAssessment {
    fn unavailable() -> Self {
        Self {
            status: "unavailable",
            score: None,
            risk: "unknown",
        }
    }
}

fn video_slide_sharpness_assessment_from_candidate(
    candidate: &Value,
) -> VideoSlideSharpnessAssessment {
    let Some(frame_path) = candidate.get("frame_path").and_then(Value::as_str) else {
        return VideoSlideSharpnessAssessment::unavailable();
    };
    video_slide_sharpness_assessment(Path::new(frame_path))
}

fn video_slide_sharpness_assessment(frame: &Path) -> VideoSlideSharpnessAssessment {
    let Some(score) = video_slide_sharpness_score(frame) else {
        return VideoSlideSharpnessAssessment::unavailable();
    };
    let risk = if score >= VIDEO_SLIDE_SHARPNESS_LOW_RISK_MIN_SCORE {
        "low"
    } else if score >= VIDEO_SLIDE_SHARPNESS_MEDIUM_RISK_MIN_SCORE {
        "medium"
    } else {
        "high"
    };
    VideoSlideSharpnessAssessment {
        status: "measured",
        score: Some(score),
        risk,
    }
}

fn video_slide_sharpness_score(frame: &Path) -> Option<i64> {
    let image = ImageReader::open(frame)
        .ok()?
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?
        .to_luma8();
    let (width, height) = image.dimensions();
    if width < 2 || height < 2 {
        return None;
    }
    let pixel_count = width as f64 * height as f64;
    let sample_step = ((pixel_count / VIDEO_SLIDE_SHARPNESS_TARGET_SAMPLES)
        .sqrt()
        .ceil() as u32)
        .max(1);
    let mut gradients = Vec::<u16>::new();
    let mut min_luma = u8::MAX;
    let mut max_luma = u8::MIN;
    for y in (0..height).step_by(sample_step as usize) {
        for x in (0..width).step_by(sample_step as usize) {
            let luma = image.get_pixel(x, y).0[0];
            min_luma = min_luma.min(luma);
            max_luma = max_luma.max(luma);
            if x + 1 < width {
                let right = image.get_pixel(x + 1, y).0[0];
                gradients.push(luma.abs_diff(right) as u16);
            }
            if y + 1 < height {
                let below = image.get_pixel(x, y + 1).0[0];
                gradients.push(luma.abs_diff(below) as u16);
            }
            if sample_step > 1 && x + sample_step < width {
                let right = image.get_pixel(x + sample_step, y).0[0];
                gradients.push(luma.abs_diff(right) as u16);
            }
            if sample_step > 1 && y + sample_step < height {
                let below = image.get_pixel(x, y + sample_step).0[0];
                gradients.push(luma.abs_diff(below) as u16);
            }
        }
    }
    if gradients.len() < 16 {
        return None;
    }
    let luma_range = max_luma.saturating_sub(min_luma);
    if luma_range <= 3 {
        return Some(0);
    }
    gradients.sort_unstable();
    let top_count = (gradients.len() / 10).max(1);
    let top_sum: u64 = gradients
        .iter()
        .rev()
        .take(top_count)
        .map(|value| *value as u64)
        .sum();
    let top_avg_gradient = top_sum as f64 / top_count as f64;
    let normalized = ((top_avg_gradient / 70.0) * 100.0).round();
    let score = normalized.clamp(0.0, 100.0) as i64;
    if luma_range < 12 {
        Some(score.min(25))
    } else {
        Some(score)
    }
}

fn video_slide_rectangle_from_frame(
    slide_number: usize,
    candidate_index: usize,
    file_name: &str,
    timestamp_seconds: f64,
    frame: &Path,
) -> Value {
    video_detect_slide_rectangle(frame)
        .map(|detection| {
            json!({
                "slide_number": slide_number,
                "candidate_index": candidate_index,
                "source_frame": file_name,
                "timestamp_seconds": timestamp_seconds,
                "rectangle_source": detection.rectangle_source,
                "rectangle_extraction_status": "promoted_detector_crop",
                "rectangle_extraction_mode": detection.rectangle_extraction_mode,
                "crop_box": {
                    "unit": "relative",
                    "x": detection.x,
                    "y": detection.y,
                    "width": detection.width,
                    "height": detection.height,
                },
                "detector": {
                    "name": detection.detector_name,
                    "image_width": detection.image_width,
                    "image_height": detection.image_height,
                    "foreground_pixel_count": detection.signal_pixel_count,
                    "background_sample_count": detection.sample_count,
                    "background_source": detection.signal_source,
                    "signal_pixel_count": detection.signal_pixel_count,
                    "sample_count": detection.sample_count,
                    "signal_source": detection.signal_source,
                    "threshold": detection.threshold,
                },
                "confidence_label": "detector_candidate_requires_review",
                "review_required": true,
            })
        })
        .unwrap_or_else(|| {
            video_full_frame_slide_rectangle(
                slide_number,
                candidate_index,
                file_name,
                timestamp_seconds,
            )
        })
}

fn video_full_frame_slide_rectangle(
    slide_number: usize,
    candidate_index: usize,
    file_name: &str,
    timestamp_seconds: f64,
) -> Value {
    json!({
        "slide_number": slide_number,
        "candidate_index": candidate_index,
        "source_frame": file_name,
        "timestamp_seconds": timestamp_seconds,
        "rectangle_source": "raw_frame_full_frame_fallback",
        "rectangle_extraction_status": "promoted_full_frame_fallback",
        "rectangle_extraction_mode": "full_frame_fallback",
        "crop_box": {
            "unit": "relative",
            "x": 0.0,
            "y": 0.0,
            "width": 1.0,
            "height": 1.0,
        },
        "confidence_label": "fallback_requires_review",
        "review_required": true,
    })
}

struct VideoSlideRectangleDetection {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    image_width: u32,
    image_height: u32,
    signal_pixel_count: u64,
    sample_count: u64,
    signal_source: &'static str,
    detector_name: &'static str,
    rectangle_source: &'static str,
    rectangle_extraction_mode: &'static str,
    threshold: u8,
}

#[derive(Clone, Copy)]
struct VideoForegroundComponent {
    min_x: u32,
    max_x: u32,
    min_y: u32,
    max_y: u32,
    pixel_count: u64,
}

fn video_detect_slide_rectangle(frame: &Path) -> Option<VideoSlideRectangleDetection> {
    let image = ImageReader::open(frame)
        .ok()?
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?;
    let (width, height) = image.dimensions();
    if width < 16 || height < 16 {
        return None;
    }

    video_detect_slide_rectangle_by_background_contrast(&image, width, height)
        .or_else(|| video_detect_slide_rectangle_by_edge_projection(&image, width, height))
        .or_else(|| video_detect_slide_rectangle_by_bright_canvas(&image, width, height))
}

fn video_detect_slide_rectangle_by_background_contrast(
    image: &image::DynamicImage,
    width: u32,
    height: u32,
) -> Option<VideoSlideRectangleDetection> {
    let (background, background_sample_count) =
        video_border_median_background_rgb(image, width, height)?;
    let threshold = 36_u8;
    let mut column_foreground_counts = vec![0_u32; width as usize];
    let mut row_foreground_counts = vec![0_u32; height as usize];
    let mut foreground_pixel_count = 0_u64;

    for y in 0..height {
        for x in 0..width {
            let pixel = image.get_pixel(x, y).to_rgb();
            if video_pixel_distance_exceeds_threshold(&pixel.0, &background, threshold) {
                column_foreground_counts[x as usize] += 1;
                row_foreground_counts[y as usize] += 1;
                foreground_pixel_count += 1;
            }
        }
    }

    let image_area = u64::from(width) * u64::from(height);
    if foreground_pixel_count < image_area / 100 {
        return None;
    }
    let column_threshold = (height / 10).max(4);
    let row_threshold = (width / 10).max(4);
    let min_x = column_foreground_counts
        .iter()
        .position(|count| *count >= column_threshold)? as u32;
    let max_x = column_foreground_counts
        .iter()
        .rposition(|count| *count >= column_threshold)? as u32;
    let min_y = row_foreground_counts
        .iter()
        .position(|count| *count >= row_threshold)? as u32;
    let max_y = row_foreground_counts
        .iter()
        .rposition(|count| *count >= row_threshold)? as u32;

    let box_width = max_x - min_x + 1;
    let box_height = max_y - min_y + 1;
    if box_width < width / 4 || box_height < height / 4 {
        return None;
    }
    if box_width >= width.saturating_sub(2) && box_height >= height.saturating_sub(2) {
        return None;
    }

    if let Some(component_detection) = video_refine_slide_rectangle_by_foreground_component(
        image,
        width,
        height,
        &background,
        threshold,
        min_x,
        max_x,
        min_y,
        max_y,
        foreground_pixel_count,
        background_sample_count,
    ) {
        return Some(component_detection);
    }

    Some(VideoSlideRectangleDetection {
        x: video_relative_coord(min_x, width),
        y: video_relative_coord(min_y, height),
        width: video_relative_coord(box_width, width),
        height: video_relative_coord(box_height, height),
        image_width: width,
        image_height: height,
        signal_pixel_count: foreground_pixel_count,
        sample_count: background_sample_count,
        signal_source: "border_median_rgb",
        detector_name: "border_background_contrast_v2",
        rectangle_source: "raw_frame_border_background_contrast",
        rectangle_extraction_mode: "border_background_contrast_v2",
        threshold,
    })
}

fn video_refine_slide_rectangle_by_foreground_component(
    image: &image::DynamicImage,
    width: u32,
    height: u32,
    background: &[u8; 3],
    threshold: u8,
    aggregate_min_x: u32,
    aggregate_max_x: u32,
    aggregate_min_y: u32,
    aggregate_max_y: u32,
    foreground_pixel_count: u64,
    background_sample_count: u64,
) -> Option<VideoSlideRectangleDetection> {
    let pixel_len = usize::try_from(u64::from(width) * u64::from(height)).ok()?;
    let mut visited = vec![false; pixel_len];
    let mut stack = Vec::new();
    let mut meaningful_component_count = 0_u32;
    let mut largest_component: Option<VideoForegroundComponent> = None;

    for y in 0..height {
        for x in 0..width {
            let index = video_image_index(x, y, width)?;
            if visited[index] {
                continue;
            }
            visited[index] = true;

            let pixel = image.get_pixel(x, y).to_rgb();
            if !video_pixel_distance_exceeds_threshold(&pixel.0, background, threshold) {
                continue;
            }

            let mut component = VideoForegroundComponent {
                min_x: x,
                max_x: x,
                min_y: y,
                max_y: y,
                pixel_count: 0,
            };
            stack.push((x, y));

            while let Some((current_x, current_y)) = stack.pop() {
                component.min_x = component.min_x.min(current_x);
                component.max_x = component.max_x.max(current_x);
                component.min_y = component.min_y.min(current_y);
                component.max_y = component.max_y.max(current_y);
                component.pixel_count += 1;

                if current_x > 0 {
                    video_push_foreground_neighbor(
                        image,
                        width,
                        background,
                        threshold,
                        current_x - 1,
                        current_y,
                        &mut visited,
                        &mut stack,
                    )?;
                }
                if current_x + 1 < width {
                    video_push_foreground_neighbor(
                        image,
                        width,
                        background,
                        threshold,
                        current_x + 1,
                        current_y,
                        &mut visited,
                        &mut stack,
                    )?;
                }
                if current_y > 0 {
                    video_push_foreground_neighbor(
                        image,
                        width,
                        background,
                        threshold,
                        current_x,
                        current_y - 1,
                        &mut visited,
                        &mut stack,
                    )?;
                }
                if current_y + 1 < height {
                    video_push_foreground_neighbor(
                        image,
                        width,
                        background,
                        threshold,
                        current_x,
                        current_y + 1,
                        &mut visited,
                        &mut stack,
                    )?;
                }
            }

            if component.pixel_count >= 4 {
                meaningful_component_count += 1;
            }
            if largest_component
                .map(|largest| component.pixel_count > largest.pixel_count)
                .unwrap_or(true)
            {
                largest_component = Some(component);
            }
        }
    }

    if meaningful_component_count <= 1 {
        return None;
    }

    let largest = largest_component?;
    let box_width = largest.max_x - largest.min_x + 1;
    let box_height = largest.max_y - largest.min_y + 1;
    if box_width < width / 4 || box_height < height / 4 {
        return None;
    }
    if box_width >= width.saturating_sub(2) && box_height >= height.saturating_sub(2) {
        return None;
    }
    let aspect_ratio = f64::from(box_width) / f64::from(box_height);
    if !(0.9..=2.4).contains(&aspect_ratio) {
        return None;
    }
    if largest.pixel_count.saturating_mul(100) < foreground_pixel_count.saturating_mul(70) {
        return None;
    }

    let aggregate_width = aggregate_max_x - aggregate_min_x + 1;
    let aggregate_height = aggregate_max_y - aggregate_min_y + 1;
    let aggregate_area = u64::from(aggregate_width) * u64::from(aggregate_height);
    let largest_area = u64::from(box_width) * u64::from(box_height);
    if aggregate_area.saturating_mul(100) <= largest_area.saturating_mul(110) {
        return None;
    }

    Some(VideoSlideRectangleDetection {
        x: video_relative_coord(largest.min_x, width),
        y: video_relative_coord(largest.min_y, height),
        width: video_relative_coord(box_width, width),
        height: video_relative_coord(box_height, height),
        image_width: width,
        image_height: height,
        signal_pixel_count: largest.pixel_count,
        sample_count: background_sample_count,
        signal_source: "border_median_component",
        detector_name: "foreground_component_v1",
        rectangle_source: "raw_frame_foreground_component",
        rectangle_extraction_mode: "foreground_component_v1",
        threshold,
    })
}

fn video_push_foreground_neighbor(
    image: &image::DynamicImage,
    width: u32,
    background: &[u8; 3],
    threshold: u8,
    x: u32,
    y: u32,
    visited: &mut [bool],
    stack: &mut Vec<(u32, u32)>,
) -> Option<()> {
    let index = video_image_index(x, y, width)?;
    if visited[index] {
        return Some(());
    }
    visited[index] = true;
    let pixel = image.get_pixel(x, y).to_rgb();
    if video_pixel_distance_exceeds_threshold(&pixel.0, background, threshold) {
        stack.push((x, y));
    }
    Some(())
}

fn video_detect_slide_rectangle_by_bright_canvas(
    image: &image::DynamicImage,
    width: u32,
    height: u32,
) -> Option<VideoSlideRectangleDetection> {
    if width < 32 || height < 32 {
        return None;
    }

    let threshold = 212_u8;
    let mut column_bright_counts = vec![0_u32; width as usize];
    let mut row_bright_counts = vec![0_u32; height as usize];
    let mut bright_pixel_count = 0_u64;

    for y in 0..height {
        for x in 0..width {
            let pixel = image.get_pixel(x, y).to_rgb();
            if video_luma_from_rgb(&pixel.0) >= threshold {
                column_bright_counts[x as usize] += 1;
                row_bright_counts[y as usize] += 1;
                bright_pixel_count += 1;
            }
        }
    }

    let image_area = u64::from(width) * u64::from(height);
    if bright_pixel_count < image_area / 12 {
        return None;
    }

    let column_threshold = (height / 6).max(6);
    let row_threshold = (width / 6).max(6);
    let min_x = column_bright_counts
        .iter()
        .position(|count| *count >= column_threshold)? as u32;
    let max_x = column_bright_counts
        .iter()
        .rposition(|count| *count >= column_threshold)? as u32;
    let min_y = row_bright_counts
        .iter()
        .position(|count| *count >= row_threshold)? as u32;
    let max_y = row_bright_counts
        .iter()
        .rposition(|count| *count >= row_threshold)? as u32;

    if min_x <= 1
        || min_y <= 1
        || max_x >= width.saturating_sub(2)
        || max_y >= height.saturating_sub(2)
    {
        return None;
    }

    let box_width = max_x - min_x + 1;
    let box_height = max_y - min_y + 1;
    if box_width < width / 4 || box_height < height / 4 {
        return None;
    }
    if box_width >= width.saturating_mul(19) / 20 && box_height >= height.saturating_mul(19) / 20 {
        return None;
    }

    let aspect_ratio = f64::from(box_width) / f64::from(box_height);
    if !(0.8..=2.7).contains(&aspect_ratio) {
        return None;
    }

    let box_area = u64::from(box_width) * u64::from(box_height);
    if bright_pixel_count.saturating_mul(100) < box_area.saturating_mul(55) {
        return None;
    }

    Some(VideoSlideRectangleDetection {
        x: video_relative_coord(min_x, width),
        y: video_relative_coord(min_y, height),
        width: video_relative_coord(box_width, width),
        height: video_relative_coord(box_height, height),
        image_width: width,
        image_height: height,
        signal_pixel_count: bright_pixel_count,
        sample_count: image_area,
        signal_source: "bright_canvas_luma",
        detector_name: "bright_canvas_v1",
        rectangle_source: "raw_frame_bright_canvas",
        rectangle_extraction_mode: "bright_canvas_v1",
        threshold,
    })
}

fn video_image_index(x: u32, y: u32, width: u32) -> Option<usize> {
    usize::try_from(u64::from(y) * u64::from(width) + u64::from(x)).ok()
}

fn video_detect_slide_rectangle_by_edge_projection(
    image: &image::DynamicImage,
    width: u32,
    height: u32,
) -> Option<VideoSlideRectangleDetection> {
    if width < 32 || height < 32 {
        return None;
    }

    let threshold = 54_u8;
    let mut column_edge_counts = vec![0_u32; width as usize];
    let mut row_edge_counts = vec![0_u32; height as usize];
    let mut edge_pixel_count = 0_u64;

    for y in 0..height {
        for x in 1..width {
            let left = image.get_pixel(x - 1, y).to_rgb();
            let right = image.get_pixel(x, y).to_rgb();
            if video_pixel_distance_exceeds_threshold(&left.0, &right.0, threshold) {
                column_edge_counts[x as usize] += 1;
                edge_pixel_count += 1;
            }
        }
    }
    for y in 1..height {
        for x in 0..width {
            let top = image.get_pixel(x, y - 1).to_rgb();
            let bottom = image.get_pixel(x, y).to_rgb();
            if video_pixel_distance_exceeds_threshold(&top.0, &bottom.0, threshold) {
                row_edge_counts[y as usize] += 1;
                edge_pixel_count += 1;
            }
        }
    }

    let column_threshold = (height.saturating_mul(2) / 5).max(10);
    let row_threshold = (width.saturating_mul(2) / 5).max(10);
    let columns = video_projection_line_centers(&column_edge_counts, column_threshold);
    let rows = video_projection_line_centers(&row_edge_counts, row_threshold);
    let (Some(min_x), Some(max_x), Some(min_y), Some(max_y)) = (
        columns.first().copied(),
        columns.last().copied(),
        rows.first().copied(),
        rows.last().copied(),
    ) else {
        return None;
    };
    if min_x >= max_x || min_y >= max_y {
        return None;
    }

    let box_width = max_x - min_x + 1;
    let box_height = max_y - min_y + 1;
    if box_width < width / 4 || box_height < height / 4 {
        return None;
    }
    if min_x <= 1
        || min_y <= 1
        || max_x >= width.saturating_sub(2)
        || max_y >= height.saturating_sub(2)
    {
        return None;
    }
    if box_width >= width.saturating_mul(19) / 20 && box_height >= height.saturating_mul(19) / 20 {
        return None;
    }

    Some(VideoSlideRectangleDetection {
        x: video_relative_coord(min_x, width),
        y: video_relative_coord(min_y, height),
        width: video_relative_coord(box_width, width),
        height: video_relative_coord(box_height, height),
        image_width: width,
        image_height: height,
        signal_pixel_count: edge_pixel_count,
        sample_count: (columns.len() + rows.len()) as u64,
        signal_source: "projection_edge_lines",
        detector_name: "edge_projection_v1",
        rectangle_source: "raw_frame_edge_projection",
        rectangle_extraction_mode: "edge_projection_v1",
        threshold,
    })
}

fn video_projection_line_centers(counts: &[u32], threshold: u32) -> Vec<u32> {
    let mut centers = Vec::new();
    let mut index = 0_usize;
    while index < counts.len() {
        if counts[index] < threshold {
            index += 1;
            continue;
        }
        let start = index;
        let mut best_index = index;
        let mut best_count = counts[index];
        while index + 1 < counts.len() && counts[index + 1] >= threshold {
            index += 1;
            if counts[index] > best_count {
                best_count = counts[index];
                best_index = index;
            }
        }
        let end = index;
        centers.push(((start + best_index + end) / 3) as u32);
        index += 1;
    }
    centers
}

fn video_border_median_background_rgb(
    image: &image::DynamicImage,
    width: u32,
    height: u32,
) -> Option<([u8; 3], u64)> {
    let sample_capacity = (u64::from(width) * 2 + u64::from(height) * 2) as usize;
    let mut red = Vec::with_capacity(sample_capacity);
    let mut green = Vec::with_capacity(sample_capacity);
    let mut blue = Vec::with_capacity(sample_capacity);
    for x in 0..width {
        video_push_background_sample(image, x, 0, &mut red, &mut green, &mut blue);
        if height > 1 {
            video_push_background_sample(image, x, height - 1, &mut red, &mut green, &mut blue);
        }
    }
    for y in 1..height.saturating_sub(1) {
        video_push_background_sample(image, 0, y, &mut red, &mut green, &mut blue);
        if width > 1 {
            video_push_background_sample(image, width - 1, y, &mut red, &mut green, &mut blue);
        }
    }
    if red.is_empty() {
        return None;
    }
    red.sort_unstable();
    green.sort_unstable();
    blue.sort_unstable();
    let median_index = red.len() / 2;
    Some((
        [red[median_index], green[median_index], blue[median_index]],
        red.len() as u64,
    ))
}

fn video_push_background_sample(
    image: &image::DynamicImage,
    x: u32,
    y: u32,
    red: &mut Vec<u8>,
    green: &mut Vec<u8>,
    blue: &mut Vec<u8>,
) {
    let pixel = image.get_pixel(x, y).to_rgb();
    red.push(pixel.0[0]);
    green.push(pixel.0[1]);
    blue.push(pixel.0[2]);
}

fn video_pixel_distance_exceeds_threshold(
    pixel: &[u8; 3],
    background: &[u8; 3],
    threshold: u8,
) -> bool {
    pixel
        .iter()
        .zip(background.iter())
        .any(|(left, right)| left.abs_diff(*right) > threshold)
}

fn video_relative_coord(value: u32, total: u32) -> f64 {
    ((f64::from(value) / f64::from(total)) * 10_000.0).round() / 10_000.0
}

fn video_slide_rectangle_aggregate_status(selected_candidates: &[Value]) -> &'static str {
    if selected_candidates.is_empty() {
        return "waiting_for_selection";
    }
    let detector_count = selected_candidates
        .iter()
        .filter(|candidate| {
            candidate
                .get("slide_rectangle")
                .and_then(|rectangle| rectangle.get("rectangle_extraction_status"))
                .and_then(Value::as_str)
                == Some("promoted_detector_crop")
        })
        .count();
    if detector_count == selected_candidates.len() {
        "promoted_detector_crop"
    } else if detector_count > 0 {
        "mixed_detector_and_full_frame_fallback"
    } else {
        "promoted_full_frame_fallback"
    }
}

fn video_slide_rectangle_aggregate_mode(selected_candidates: &[Value]) -> &'static str {
    match video_slide_rectangle_aggregate_status(selected_candidates) {
        "waiting_for_selection" => "waiting_for_selection",
        "promoted_detector_crop" => {
            let detector_modes = selected_candidates
                .iter()
                .filter_map(|candidate| {
                    candidate
                        .get("slide_rectangle")
                        .and_then(|rectangle| rectangle.get("rectangle_extraction_mode"))
                        .and_then(Value::as_str)
                })
                .collect::<BTreeSet<_>>();
            if detector_modes.len() == 1 {
                if detector_modes.contains("foreground_component_v1") {
                    "foreground_component_v1"
                } else if detector_modes.contains("bright_canvas_v1") {
                    "bright_canvas_v1"
                } else if detector_modes.contains("edge_projection_v1") {
                    "edge_projection_v1"
                } else if detector_modes.contains("border_background_contrast_v2") {
                    "border_background_contrast_v2"
                } else {
                    "simple_background_contrast_v1"
                }
            } else {
                "mixed_detector_modes"
            }
        }
        "mixed_detector_and_full_frame_fallback" => "mixed_detector_and_full_frame_fallback",
        _ => "full_frame_fallback",
    }
}

fn video_slide_rectangles_manifest_from_selected_slides(
    document: &Document,
    selected_slides_manifest: &Value,
    candidate_manifest_path: &Path,
    contact_sheet_html_path: &Path,
    keep_list_template_path: &Path,
) -> Value {
    let selected_candidates = selected_slides_manifest
        .get("selected_candidates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let rectangles = selected_candidates
        .iter()
        .filter_map(|candidate| candidate.get("slide_rectangle").cloned())
        .collect::<Vec<_>>();
    let promoted_count = rectangles.len();
    let rectangle_extraction_status = selected_slides_manifest
        .get("rectangle_extraction_status")
        .and_then(Value::as_str)
        .unwrap_or(if promoted_count == 0 {
            "waiting_for_selection"
        } else {
            "promoted_full_frame_fallback"
        });
    let rectangle_extraction_mode = selected_slides_manifest
        .get("rectangle_extraction_mode")
        .and_then(Value::as_str)
        .unwrap_or(if promoted_count == 0 {
            "waiting_for_selection"
        } else {
            "full_frame_fallback"
        });
    let dedupe_status = selected_slides_manifest
        .get("dedupe_status")
        .and_then(Value::as_str)
        .unwrap_or(if promoted_count == 0 {
            "waiting_for_selection"
        } else {
            "selected_keep_list_order_deduped"
        });
    let deduped_candidate_count = selected_slides_manifest
        .get("deduped_candidate_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let exact_duplicate_count = selected_slides_manifest
        .get("exact_duplicate_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let visual_duplicate_count = selected_slides_manifest
        .get("visual_duplicate_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let visual_shape_duplicate_count = selected_slides_manifest
        .get("visual_shape_duplicate_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let rejected_duplicate_candidates = selected_slides_manifest
        .get("rejected_duplicate_candidates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    json!({
        "status": rectangle_extraction_status,
        "source": "selected_slides_manifest",
        "document_id": document.id.to_string(),
        "dataset_id": document.dataset_id.to_string(),
        "title": document.title,
        "candidate_manifest": candidate_manifest_path.display().to_string(),
        "contact_sheet_html": contact_sheet_html_path.display().to_string(),
        "keep_list_template": keep_list_template_path.display().to_string(),
        "rectangle_extraction_status": rectangle_extraction_status,
        "rectangle_extraction_mode": rectangle_extraction_mode,
        "promoted_rectangle_count": promoted_count,
        "dedupe_status": dedupe_status,
        "deduped_candidate_count": deduped_candidate_count,
        "exact_duplicate_count": exact_duplicate_count,
        "visual_duplicate_count": visual_duplicate_count,
        "visual_shape_duplicate_count": visual_shape_duplicate_count,
        "dedupe_policy": {
            "source": "ppt_keep_list_template.selected_candidate_indices",
            "rule": "range-check candidate numbers, preserve selected order, keep the first occurrence of each candidate number, remove exact duplicate frame bytes, conservative visual near-duplicates, and high-confidence contrast-normalized shape duplicates",
            "visual_signature_grid": format!("{}x{}", VIDEO_VISUAL_SIGNATURE_GRID_SIZE, VIDEO_VISUAL_SIGNATURE_GRID_SIZE),
            "visual_near_duplicate_max_avg_diff": VIDEO_VISUAL_NEAR_DUPLICATE_MAX_AVG_DIFF,
            "visual_shape_duplicate_min_jaccard": VIDEO_VISUAL_SHAPE_DUPLICATE_MIN_JACCARD,
            "visual_shape_duplicate_min_containment": VIDEO_VISUAL_SHAPE_DUPLICATE_MIN_CONTAINMENT,
            "visual_shape_duplicate_min_signal_balance": VIDEO_VISUAL_SHAPE_DUPLICATE_MIN_SIGNAL_BALANCE,
            "visual_shape_duplicate_min_signal_samples": VIDEO_VISUAL_SHAPE_DUPLICATE_MIN_SIGNAL_SAMPLES,
            "visual_shape_duplicate_max_avg_luma": VIDEO_VISUAL_SHAPE_DUPLICATE_MAX_AVG_LUMA,
        },
        "crop_policy": {
            "mode": rectangle_extraction_mode,
            "unit": "relative",
            "reason": if rectangle_extraction_mode == "full_frame_fallback" {
                "no conservative visual rectangle detector was confident enough; selected raw frames are explicitly marked review_required"
            } else {
                "conservative visual detector crops are promoted but still require review"
            },
        },
        "rectangles": rectangles,
        "rejected_duplicate_candidates": rejected_duplicate_candidates,
        "next_step": if promoted_count == 0 {
            "fill ppt_keep_list_template.json from the numbered contact sheet"
        } else {
            "replace full-frame fallback rectangles with detector crops when rectangle extraction is available"
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

fn video_ocr_snippets_for_window(
    ocr_snippets: &[Value],
    start_seconds: f64,
    end_seconds: f64,
) -> Vec<Value> {
    ocr_snippets
        .iter()
        .filter_map(|snippet| {
            let timestamp_seconds = video_ocr_snippet_timestamp_seconds(snippet)?;
            if timestamp_seconds < start_seconds || timestamp_seconds > end_seconds {
                return None;
            }
            Some(json!({
                "timestamp_seconds": timestamp_seconds,
                "timestamp_label": format_seconds(timestamp_seconds),
                "text": video_item_text(snippet, &["text", "ocr_text", "summary"]).unwrap_or_default(),
                "confidence": video_number_field(snippet, &["confidence", "ocr_confidence", "text_confidence"]),
            }))
        })
        .collect()
}

fn video_ocr_snippet_timestamp_seconds(snippet: &Value) -> Option<f64> {
    video_number_field(
        snippet,
        &[
            "timestamp_seconds",
            "timestampSeconds",
            "time_seconds",
            "timeSeconds",
        ],
    )
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
    output.push_str("This file is deterministic evidence for the screenshot-based PPTX. It contains source frame, selection, rectangle, transcript/OCR window, and review metadata without exposing local paths.\n\n");
    output.push_str("## Deck Summary\n\n");
    let requested_selected_count = selected_slides_manifest
        .get("requested_selected_count")
        .and_then(Value::as_u64)
        .unwrap_or(selected_candidates.len() as u64);
    let selected_count = selected_slides_manifest
        .get("selected_count")
        .and_then(Value::as_u64)
        .unwrap_or(selected_candidates.len() as u64);
    let deduped_candidate_count = selected_slides_manifest
        .get("deduped_candidate_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let dedupe_status = selected_slides_manifest
        .get("dedupe_status")
        .and_then(Value::as_str)
        .unwrap_or("waiting_for_selection");
    let rectangle_status = selected_slides_manifest
        .get("rectangle_extraction_status")
        .and_then(Value::as_str)
        .unwrap_or("waiting_for_selection");
    let rectangle_mode = selected_slides_manifest
        .get("rectangle_extraction_mode")
        .and_then(Value::as_str)
        .unwrap_or("waiting_for_selection");
    output.push_str(&format!(
        "- Requested keep-list entries: {requested_selected_count}\n- Selected slides: {selected_count}\n- Removed duplicate candidates: {deduped_candidate_count}\n- Dedupe status: {dedupe_status}\n- Rectangle status: {rectangle_status}\n- Rectangle mode: {rectangle_mode}\n\n"
    ));
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
        let timestamp = candidate
            .get("timestamp_label")
            .and_then(Value::as_str)
            .unwrap_or("");
        let contact_sheet_anchor = candidate
            .get("contact_sheet_anchor")
            .and_then(Value::as_str)
            .unwrap_or("");
        output.push_str(&format!(
            "### Slide {slide_number}\n\n- Candidate: {candidate_index}\n- Source frame: {file_name}\n- Frame timestamp: {}\n- Internal frame path: [redacted]\n",
            if timestamp.is_empty() { "unknown" } else { timestamp }
        ));
        if !contact_sheet_anchor.is_empty() {
            output.push_str(&format!(
                "- Contact sheet anchor: `{contact_sheet_anchor}`\n"
            ));
        }
        if let Some(window) = candidate.get("transcript_window") {
            let start = window.get("start_seconds").and_then(Value::as_f64);
            let end = window.get("end_seconds").and_then(Value::as_f64);
            let assignment_rule = window
                .get("assignment_rule")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            output.push_str(&format!(
                "- Transcript window: {} -> {} ({assignment_rule})\n",
                start
                    .map(format_seconds)
                    .unwrap_or_else(|| "unknown".to_string()),
                end.map(format_seconds)
                    .unwrap_or_else(|| "unknown".to_string())
            ));
        }
        if let Some(rectangle) = candidate.get("slide_rectangle") {
            let status = rectangle
                .get("rectangle_extraction_status")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let mode = rectangle
                .get("rectangle_extraction_mode")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let source = rectangle
                .get("rectangle_source")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            output.push_str(&format!("- Rectangle: {status} / {mode} ({source})\n"));
            if let Some(crop_box) = rectangle.get("crop_box") {
                let unit = crop_box
                    .get("unit")
                    .and_then(Value::as_str)
                    .unwrap_or("relative");
                output.push_str(&format!(
                    "- Crop box: x={}, y={}, width={}, height={} ({unit})\n",
                    video_crop_box_value_label(crop_box, "x"),
                    video_crop_box_value_label(crop_box, "y"),
                    video_crop_box_value_label(crop_box, "width"),
                    video_crop_box_value_label(crop_box, "height")
                ));
            }
            let review_required = rectangle
                .get("review_required")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            output.push_str(&format!(
                "- Review required: {}\n",
                if review_required { "yes" } else { "no" }
            ));
        }
        let transcript_segments = candidate
            .get("transcript_segments")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let ocr_snippets = candidate
            .get("ocr_snippets")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if transcript_segments.is_empty() && ocr_snippets.is_empty() {
            output.push_str(&format!("- Speaker notes: Source frame metadata only for candidate {candidate_index}. Human/model review should align transcript/subtitles before customer delivery.\n\n"));
        } else {
            if transcript_segments.is_empty() {
                output.push_str("- Speaker notes: no aligned transcript segment is available for this slide yet.\n");
            } else {
                output.push_str("- Speaker notes: pre-page transcript assignment is available and still requires review.\n");
                output.push_str("- Aligned transcript:\n");
                for segment in transcript_segments {
                    let text = video_item_text(&segment, &["text", "content", "summary"])
                        .map(|text| video_safe_evidence_text(&text))
                        .unwrap_or_default();
                    let range = video_time_range_label(&segment);
                    output.push_str(&format!("  - {}{}\n", optional_time_prefix(&range), text));
                }
            }
            if !ocr_snippets.is_empty() {
                output.push_str("- Aligned OCR snippets:\n");
                for snippet in ocr_snippets {
                    let text = video_item_text(&snippet, &["text", "ocr_text", "summary"])
                        .map(|text| video_safe_evidence_text(&text))
                        .unwrap_or_default();
                    let timestamp = snippet
                        .get("timestamp_label")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown");
                    output.push_str(&format!("  - [{timestamp}] {text}\n"));
                }
            }
            output.push('\n');
        }
    }
    output
}

fn render_video_slides_markdown(document: &Document, selected_slides_manifest: &Value) -> String {
    let selected_candidates = selected_slides_manifest
        .get("selected_candidates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut output = format!("# Video Slides: {}\n\n", document.title);
    output.push_str("This Markdown deck mirrors the screenshot-based PPTX using only DataMax-observed evidence. It is safe to share for review because local frame paths, source URLs, tokens, and provider secrets are not included.\n\n");
    output.push_str("## Package Summary\n\n");
    let selected_count = selected_slides_manifest
        .get("selected_count")
        .and_then(Value::as_u64)
        .unwrap_or(selected_candidates.len() as u64);
    let rectangle_mode = selected_slides_manifest
        .get("rectangle_extraction_mode")
        .and_then(Value::as_str)
        .unwrap_or("waiting_for_selection");
    let dedupe_status = selected_slides_manifest
        .get("dedupe_status")
        .and_then(Value::as_str)
        .unwrap_or("waiting_for_selection");
    output.push_str(&format!(
        "- Selected slides: {selected_count}\n- Markdown source: selected slide manifest\n- PPTX companion: `{}`\n- Rectangle mode: {rectangle_mode}\n- Dedupe status: {dedupe_status}\n- Review policy: every generated slide still requires human or model-assisted review before customer delivery.\n\n",
        DEFAULT_VIDEO_SLIDES_PPTX_FILE_NAME
    ));

    output.push_str("## Slides\n\n");
    if selected_candidates.is_empty() {
        output.push_str("- No selected slides yet. Fill `ppt_keep_list_template.json` from the numbered contact sheet before generating the final Markdown/PPTX deck.\n");
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
            .map(video_safe_evidence_text)
            .unwrap_or_else(|| "frame".to_string());
        let timestamp = candidate
            .get("timestamp_label")
            .and_then(Value::as_str)
            .map(video_safe_evidence_text)
            .unwrap_or_else(|| "unknown".to_string());
        output.push_str(&format!(
            "### Slide {slide_number}: candidate {candidate_index}\n\n"
        ));
        output.push_str(&format!(
            "- Source frame: `{file_name}`\n- Frame timestamp: {timestamp}\n"
        ));
        if let Some(anchor) = candidate
            .get("contact_sheet_anchor")
            .and_then(Value::as_str)
            .map(video_safe_evidence_text)
        {
            output.push_str(&format!("- Contact sheet anchor: `{anchor}`\n"));
        }
        if let Some(rectangle) = candidate.get("slide_rectangle") {
            let status = rectangle
                .get("rectangle_extraction_status")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let mode = rectangle
                .get("rectangle_extraction_mode")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            output.push_str(&format!("- Crop status: {status} / {mode}\n"));
            if let Some(crop_box) = rectangle.get("crop_box") {
                output.push_str(&format!(
                    "- Crop box: x={}, y={}, width={}, height={} (relative)\n",
                    video_crop_box_value_label(crop_box, "x"),
                    video_crop_box_value_label(crop_box, "y"),
                    video_crop_box_value_label(crop_box, "width"),
                    video_crop_box_value_label(crop_box, "height")
                ));
            }
        }

        let transcript_segments = candidate
            .get("transcript_segments")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let ocr_snippets = candidate
            .get("ocr_snippets")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if transcript_segments.is_empty() {
            output.push_str(
                "- Narration: no aligned transcript segment is available for this slide yet.\n\n",
            );
        } else {
            output.push_str("\nNarration evidence:\n\n");
            for segment in transcript_segments {
                let text = video_item_text(&segment, &["text", "content", "summary"])
                    .map(|text| video_safe_evidence_text(&text))
                    .unwrap_or_else(|| "unknown".to_string());
                let range = video_time_range_label(&segment);
                output.push_str(&format!("- {}{}\n", optional_time_prefix(&range), text));
            }
            output.push('\n');
        }
        if !ocr_snippets.is_empty() {
            output.push_str("OCR evidence:\n\n");
            for snippet in ocr_snippets {
                let text = video_item_text(&snippet, &["text", "ocr_text", "summary"])
                    .map(|text| video_safe_evidence_text(&text))
                    .unwrap_or_else(|| "unknown".to_string());
                let timestamp = snippet
                    .get("timestamp_label")
                    .and_then(Value::as_str)
                    .map(video_safe_evidence_text)
                    .unwrap_or_else(|| "unknown".to_string());
                output.push_str(&format!("- [{timestamp}] {text}\n"));
            }
            output.push('\n');
        }
    }

    output
}

fn video_crop_box_value_label(crop_box: &Value, key: &str) -> String {
    crop_box
        .get(key)
        .and_then(Value::as_f64)
        .map(|value| {
            let rounded = (value * 10_000.0).round() / 10_000.0;
            if (rounded.fract()).abs() < f64::EPSILON {
                format!("{rounded:.0}")
            } else {
                format!("{rounded:.4}")
                    .trim_end_matches('0')
                    .trim_end_matches('.')
                    .to_string()
            }
        })
        .unwrap_or_else(|| "unknown".to_string())
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
    let deduped_candidate_count = selected_slides_manifest
        .get("deduped_candidate_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if deduped_candidate_count > 0 {
        let exact_duplicate_count = selected_slides_manifest
            .get("exact_duplicate_count")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let visual_duplicate_count = selected_slides_manifest
            .get("visual_duplicate_count")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        warnings.push(format!(
            "Selected slide dedupe removed {deduped_candidate_count} duplicate candidate(s) before PPTX generation ({exact_duplicate_count} exact, {visual_duplicate_count} visual duplicate)."
        ));
    }
    let rectangle_status = selected_slides_manifest
        .get("rectangle_extraction_status")
        .and_then(Value::as_str)
        .unwrap_or("waiting_for_selection");
    if rectangle_status == "promoted_detector_crop" {
        warnings.push(
            "Slide rectangles are detector-cropped and still require visual review.".to_string(),
        );
    } else if rectangle_status == "mixed_detector_and_full_frame_fallback" {
        warnings.push(
            "Some slide rectangles are detector-cropped and some still use full-frame fallback; review required."
                .to_string(),
        );
    } else if rectangle_status == "promoted_full_frame_fallback" {
        warnings.push(
            "Slide rectangles are promoted with a full-frame fallback crop and still require visual review."
                .to_string(),
        );
    } else {
        warnings.push(
            "Slide images are raw frame screenshots until rectangle extraction/dedupe is promoted into the worker."
                .to_string(),
        );
    }
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
<Application>DataMax</Application>
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
<dc:creator>DataMax</dc:creator>
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
    let source_rect = render_pptx_source_rect(candidate);
    let file_name_attr = html_escape_attr(file_name);
    let picture_alt_text = html_escape_attr(&render_pptx_picture_alt_text(slide_number, candidate));
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
<p:cSld>
<p:spTree>
<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
<p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>
<p:pic>
<p:nvPicPr><p:cNvPr id="2" name="Candidate {candidate_index}: {file_name_attr}" descr="{picture_alt_text}"/><p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr><p:nvPr/></p:nvPicPr>
<p:blipFill><a:blip r:embed="rId1"/>{source_rect}<a:stretch><a:fillRect/></a:stretch></p:blipFill>
<p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="12192000" cy="6858000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr>
</p:pic>
</p:spTree>
</p:cSld>
<p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>
</p:sld>
"#
    )
}

fn render_pptx_picture_alt_text(slide_number: usize, candidate: &Value) -> String {
    let candidate_index = candidate
        .get("candidate_index")
        .and_then(Value::as_u64)
        .unwrap_or(slide_number as u64);
    let file_name = candidate
        .get("file_name")
        .and_then(Value::as_str)
        .map(video_safe_evidence_text)
        .unwrap_or_else(|| "frame".to_string());
    let timestamp = candidate
        .get("timestamp_label")
        .and_then(Value::as_str)
        .map(video_safe_evidence_text)
        .unwrap_or_else(|| "unknown".to_string());
    let crop_status = candidate
        .get("slide_rectangle")
        .and_then(|rectangle| rectangle.get("rectangle_extraction_status"))
        .and_then(Value::as_str)
        .map(video_safe_evidence_text)
        .unwrap_or_else(|| "unknown".to_string());
    let crop_mode = candidate
        .get("slide_rectangle")
        .and_then(|rectangle| rectangle.get("rectangle_extraction_mode"))
        .and_then(Value::as_str)
        .map(video_safe_evidence_text)
        .unwrap_or_else(|| "unknown".to_string());
    let transcript_segment_count = candidate
        .get("transcript_segments")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);

    format!(
        "Slide {slide_number}; candidate {candidate_index}; source frame {file_name}; timestamp {timestamp}; crop {crop_status}/{crop_mode}; transcript segments {transcript_segment_count}; paths redacted"
    )
}

fn render_pptx_source_rect(candidate: &Value) -> String {
    let Some(crop_box) = candidate
        .get("slide_rectangle")
        .and_then(|rectangle| rectangle.get("crop_box"))
    else {
        return String::new();
    };
    if crop_box.get("unit").and_then(Value::as_str) != Some("relative") {
        return String::new();
    }
    let Some(x) = crop_box.get("x").and_then(Value::as_f64) else {
        return String::new();
    };
    let Some(y) = crop_box.get("y").and_then(Value::as_f64) else {
        return String::new();
    };
    let Some(width) = crop_box.get("width").and_then(Value::as_f64) else {
        return String::new();
    };
    let Some(height) = crop_box.get("height").and_then(Value::as_f64) else {
        return String::new();
    };
    if !video_relative_crop_box_is_valid(x, y, width, height) {
        return String::new();
    }
    let right = 1.0 - x - width;
    let bottom = 1.0 - y - height;
    if x <= 0.0 && y <= 0.0 && right <= 0.0 && bottom <= 0.0 {
        return String::new();
    }
    format!(
        r#"<a:srcRect l="{}" r="{}" t="{}" b="{}"/>"#,
        pptx_crop_percent(x),
        pptx_crop_percent(right),
        pptx_crop_percent(y),
        pptx_crop_percent(bottom)
    )
}

fn video_relative_crop_box_is_valid(x: f64, y: f64, width: f64, height: f64) -> bool {
    [x, y, width, height].iter().all(|value| value.is_finite())
        && x >= 0.0
        && y >= 0.0
        && width > 0.0
        && height > 0.0
        && x + width <= 1.0001
        && y + height <= 1.0001
}

fn pptx_crop_percent(value: f64) -> i64 {
    (value.clamp(0.0, 1.0) * 100_000.0).round() as i64
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
        .map(video_safe_evidence_text)
        .unwrap_or_else(|| "frame".to_string());
    let timestamp = candidate
        .get("timestamp_label")
        .and_then(Value::as_str)
        .map(video_safe_evidence_text)
        .unwrap_or_else(|| "unknown".to_string());
    let transcript_note = candidate
        .get("transcript_segments")
        .and_then(Value::as_array)
        .filter(|segments| !segments.is_empty())
        .map(|segments| {
            let text = segments
                .iter()
                .filter_map(|segment| video_item_text(segment, &["text", "content", "summary"]))
                .map(|text| video_safe_evidence_text(&text))
                .collect::<Vec<_>>()
                .join(" ");
            format!(" Pre-page transcript: {text}.")
        })
        .unwrap_or_else(|| " Transcript/subtitle alignment is not yet verified.".to_string());
    let ocr_note = candidate
        .get("ocr_snippets")
        .and_then(Value::as_array)
        .filter(|snippets| !snippets.is_empty())
        .map(|snippets| {
            let text = snippets
                .iter()
                .filter_map(|snippet| video_item_text(snippet, &["text", "ocr_text", "summary"]))
                .map(|text| video_safe_evidence_text(&text))
                .collect::<Vec<_>>()
                .join(" ");
            format!(" OCR evidence: {text}.")
        })
        .unwrap_or_default();
    let note = format!(
        "Source frame: {file_name}; candidate: {candidate_index}; timestamp: {timestamp}; internal path: [redacted].{transcript_note}{ocr_note}"
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
    let files = generated_artifacts
        .get("files")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
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
    let html_artifact_ids = vec![artifact_id.clone()];
    let completion_follow_up = video_extraction_completion_follow_up_from_output(
        output,
        std::slice::from_ref(&artifact_summary),
    );
    let deliverable_package = video_deliverable_package_summary(
        assistant_run_id,
        document_id,
        title,
        output,
        &files,
        &deliverable_status,
        &html_artifact_ids,
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
            "deliverable_package": deliverable_package,
            "completion_follow_up": completion_follow_up,
            "completion_audit": video_extraction_completion_audit_from_output(output),
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
    let published_deliverable_manifest = files
        .iter()
        .find(|file| {
            file.get("artifact_kind").and_then(Value::as_str)
                == Some("published_deliverable_manifest")
        })
        .cloned()
        .unwrap_or(Value::Null);
    let published_version_history = files
        .iter()
        .find(|file| {
            file.get("artifact_kind").and_then(Value::as_str) == Some("published_version_history")
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
    let deliverable_package = video_deliverable_package_summary(
        assistant_run_id,
        document_id,
        title,
        output,
        &files,
        &deliverable_status,
        &html_artifact_ids,
    );
    let completion_follow_up =
        video_extraction_completion_follow_up_from_output(output, html_artifacts);
    let model_completion_turn_request = completion_follow_up
        .as_ref()
        .and_then(|follow_up| follow_up.get("model_follow_up"))
        .cloned()
        .unwrap_or(Value::Null);
    let model_completion_turn_dispatch_request =
        video_extraction_model_completion_dispatch_request(
            assistant_run_id,
            None,
            None,
            output,
            completion_follow_up.as_ref(),
        )
        .unwrap_or(Value::Null);

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
                "published_deliverable_manifest",
                "published_version_history",
                "extraction_artifacts_manifest",
                "ppt_outline",
                "slide_notes",
                "video_slides_markdown",
                "subtitle_page_map",
                "transcript_text",
            ],
        ),
        "manifest_outputs": video_artifact_files_by_kinds(
            &files,
            &[
                "final_deliverables_manifest",
                "published_deliverable_manifest",
                "published_version_history",
                "extraction_artifacts_manifest",
            ],
        ),
        "final_outputs": video_artifact_files_by_kinds(&files, &["pptx", "video_slides_markdown"]),
        "review_outputs": video_artifact_files_by_kinds(
            &files,
            &[
                "slide_image_candidates",
                "contact_sheet_plan",
                "contact_sheet_html",
                "ppt_keep_list_template",
                "selected_slides_manifest",
                "slide_rectangles_manifest",
                "slide_quality_report",
                "slide_notes",
                "pptx_build_plan",
            ],
        ),
        "evidence_outputs": video_artifact_files_by_kinds(
            &files,
            &[
                "frame_manifest",
                "transcript_text",
                "source_text",
                "ppt_outline",
                "timestamp_map",
                "subtitle_page_map",
            ],
        ),
        "final_deliverables_manifest": final_deliverables_manifest,
        "published_deliverable_manifest": published_deliverable_manifest,
        "published_version_history": published_version_history,
        "deliverable_package": deliverable_package,
        "html_artifacts": html_artifact_summaries,
        "html_artifact_ids": html_artifact_ids,
        "completion_follow_up": completion_follow_up,
        "model_completion_turn_request": model_completion_turn_request,
        "model_completion_turn_dispatch_request": model_completion_turn_dispatch_request,
        "completion_audit": video_extraction_completion_audit_from_output(output),
    }))
}

fn video_required_deliverable_package_kinds(
    files: &[Value],
    deliverable_status: &Value,
) -> Vec<&'static str> {
    video_required_file_kinds_for(
        VIDEO_DELIVERABLE_PACKAGE_REQUIRED_KINDS,
        files,
        deliverable_status,
    )
}

fn video_required_published_deliverable_kinds(
    files: &[Value],
    deliverable_status: &Value,
) -> Vec<&'static str> {
    video_required_file_kinds_for(
        VIDEO_PUBLISHED_DELIVERABLE_REQUIRED_KINDS,
        files,
        deliverable_status,
    )
}

fn video_required_file_kinds_for(
    base_kinds: &'static [&'static str],
    files: &[Value],
    deliverable_status: &Value,
) -> Vec<&'static str> {
    let mut kinds = base_kinds.to_vec();
    for &kind in VIDEO_CONDITIONAL_DELIVERABLE_KINDS {
        if video_conditional_deliverable_kind_required(kind, files, deliverable_status) {
            kinds.push(kind);
        }
    }
    kinds
}

fn video_conditional_deliverable_kind_required(
    kind: &str,
    files: &[Value],
    deliverable_status: &Value,
) -> bool {
    files
        .iter()
        .any(|file| file.get("artifact_kind").and_then(Value::as_str) == Some(kind))
        || match kind {
            "subtitle_page_map" => {
                deliverable_status
                    .get("has_subtitle_page_map")
                    .and_then(Value::as_bool)
                    == Some(true)
            }
            _ => false,
        }
}

fn video_deliverable_package_summary(
    assistant_run_id: &str,
    document_id: &str,
    title: &str,
    output: &Value,
    files: &[Value],
    deliverable_status: &Value,
    html_artifact_ids: &[String],
) -> Value {
    let ready_file_kinds = files
        .iter()
        .filter_map(|file| file.get("artifact_kind").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    let required_file_kind_refs =
        video_required_deliverable_package_kinds(files, deliverable_status);
    let required_file_kinds = required_file_kind_refs
        .iter()
        .copied()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let missing_required_file_kinds = required_file_kind_refs
        .iter()
        .copied()
        .filter(|kind| !ready_file_kinds.contains(kind))
        .map(str::to_string)
        .collect::<Vec<_>>();
    let published_manifest = files
        .iter()
        .find(|file| {
            file.get("artifact_kind").and_then(Value::as_str)
                == Some("published_deliverable_manifest")
        })
        .cloned()
        .unwrap_or(Value::Null);
    let has_published_manifest = !published_manifest.is_null();
    let published_version_history = files
        .iter()
        .find(|file| {
            file.get("artifact_kind").and_then(Value::as_str) == Some("published_version_history")
        })
        .cloned()
        .unwrap_or(Value::Null);
    let has_published_version_history = !published_version_history.is_null();
    let required_files = required_file_kind_refs
        .iter()
        .filter_map(|kind| {
            files
                .iter()
                .find(|file| file.get("artifact_kind").and_then(Value::as_str) == Some(*kind))
                .map(|file| {
                    json!({
                        "artifact_kind": kind,
                        "file_name": video_artifact_file_name(file, kind),
                    })
                })
        })
        .collect::<Vec<_>>();
    let state = deliverable_status
        .get("state")
        .and_then(Value::as_str)
        .or_else(|| output.get("status").and_then(Value::as_str))
        .unwrap_or("partial");
    let base_publishable = state == "final_pptx_ready" && missing_required_file_kinds.is_empty();
    let immutable_version =
        base_publishable && has_published_manifest && has_published_version_history;
    let lifecycle_state = if immutable_version {
        "published_version_ready"
    } else if base_publishable {
        "downloadable_not_published"
    } else {
        "not_ready"
    };
    let next_action = if immutable_version {
        "review_published_deliverable_manifest"
    } else if base_publishable {
        "persist_video_published_version"
    } else {
        "complete_required_deliverables"
    };

    json!({
        "kind": "video_extraction_deliverable_package",
        "version": 1,
        "package_id": format!("video-deliverable-package-{assistant_run_id}-{document_id}"),
        "lifecycle_state": lifecycle_state,
        "publishable": base_publishable,
        "published": immutable_version,
        "immutable_version": immutable_version,
        "version_no": if immutable_version { json!(1) } else { Value::Null },
        "published_manifest": published_manifest,
        "published_version_history": published_version_history,
        "next_action": next_action,
        "source_run_id": assistant_run_id,
        "document_id": document_id,
        "dataset_id": output.get("dataset_id").cloned().unwrap_or(Value::Null),
        "title": title,
        "required_file_kinds": required_file_kinds.clone(),
        "missing_required_file_kinds": missing_required_file_kinds,
        "ready_required_file_count": required_files.len(),
        "required_file_count": required_file_kind_refs.len(),
        "required_files": required_files,
        "artifact_group_counts": video_artifact_group_counts(files),
        "html_artifact_ids": html_artifact_ids,
        "no_host_composed_answer": true,
    })
}

fn video_artifact_file_name(file: &Value, fallback_kind: &str) -> String {
    file.get("file_name")
        .or_else(|| file.get("fileName"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .or_else(|| {
            file.get("path")
                .or_else(|| file.get("uri"))
                .and_then(Value::as_str)
                .and_then(|value| {
                    Path::new(value)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .map(str::to_string)
                })
        })
        .unwrap_or_else(|| fallback_kind.to_string())
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
    let next_actions = video_extraction_completion_next_actions(state, &files, &deliverable_status);
    let user_notification = video_extraction_completion_user_notification(
        state,
        &ready_file_kinds,
        &html_artifact_ids,
        &deliverable_status,
        &next_actions,
    );
    let source_run_id = html_artifacts
        .iter()
        .find_map(|artifact| {
            artifact
                .get("provenance")
                .and_then(|provenance| provenance.get("source_run_id"))
                .and_then(Value::as_str)
                .or_else(|| {
                    artifact
                        .get("owner_scope")
                        .and_then(|scope| scope.get("id"))
                        .and_then(Value::as_str)
                })
        })
        .unwrap_or("");
    let deliverable_package = video_deliverable_package_summary(
        source_run_id,
        document_id,
        title,
        output,
        &files,
        &deliverable_status,
        &html_artifact_ids,
    );
    let model_follow_up = video_extraction_model_completion_follow_up(
        title,
        state,
        &ready_file_kinds,
        &html_artifact_ids,
        &deliverable_status,
        &deliverable_package,
        &next_actions,
    );

    Some(json!({
        "kind": "video_extraction_completion_follow_up",
        "document_id": document_id,
        "dataset_id": output.get("dataset_id").cloned().unwrap_or(Value::Null),
        "title": title,
        "status": state,
        "deliverable_status": deliverable_status,
        "deliverable_package": deliverable_package,
        "ready_file_kinds": ready_file_kinds,
        "html_artifact_ids": html_artifact_ids,
        "next_actions": next_actions,
        "model_follow_up": model_follow_up,
        "user_notification": user_notification,
        "no_host_composed_answer": true,
    }))
}

pub fn video_extraction_model_completion_dispatch_request(
    assistant_run_id: &str,
    workflow_execution_id: Option<&str>,
    workflow_kind: Option<&str>,
    output: &Value,
    completion_follow_up: Option<&Value>,
) -> Option<Value> {
    let assistant_run_id = assistant_run_id.trim();
    if assistant_run_id.is_empty() {
        return None;
    }
    let document_id = output
        .get("document_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let completion_follow_up = completion_follow_up?;
    let model_request = completion_follow_up.get("model_follow_up")?;
    if model_request.get("kind").and_then(Value::as_str)
        != Some("video_extraction_model_completion_turn_request")
    {
        return None;
    }
    if model_request.get("required").and_then(Value::as_bool) != Some(true) {
        return None;
    }

    let workflow_execution_id = workflow_execution_id
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let workflow_kind = workflow_kind
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("video_extraction_workflow");
    let idempotency_key = workflow_execution_id
        .map(|workflow_execution_id| {
            format!(
                "video-completion-turn:{assistant_run_id}:{workflow_execution_id}:{document_id}:v1"
            )
        })
        .unwrap_or_else(|| format!("video-completion-turn:{assistant_run_id}:{document_id}:v1"));
    let status = completion_follow_up
        .get("status")
        .and_then(Value::as_str)
        .or_else(|| output.get("status").and_then(Value::as_str))
        .unwrap_or("partial");
    let title = completion_follow_up
        .get("title")
        .and_then(Value::as_str)
        .or_else(|| output.get("title").and_then(Value::as_str))
        .unwrap_or("视频 PPT 提取");

    Some(json!({
        "kind": "assistant_run_model_completion_turn_dispatch_request",
        "version": 1,
        "status": "queued",
        "turn_owner": "model",
        "dispatch_target": "continue_assistant_run",
        "entrypoint": "assistant_run_background_completion",
        "idempotency_key": idempotency_key,
        "assistant_run_id": assistant_run_id,
        "workflow_execution_id": workflow_execution_id,
        "workflow_kind": workflow_kind,
        "source_event": "video_extraction.workflow_completed",
        "document_id": document_id,
        "dataset_id": output.get("dataset_id").cloned().unwrap_or(Value::Null),
        "title": title,
        "completion_status": status,
        "continue_request": {
            "prompt": "后台视频/PPT提取已完成，请基于待模型接手请求和已完成 observation，用模型自己的口吻输出下一条结果说明。",
            "max_steps": 1,
            "current_artifact": Value::Null,
        },
        "model_completion_turn_request": model_request.clone(),
        "completion_context": model_request
            .get("completion_context")
            .cloned()
            .unwrap_or_else(|| json!({})),
        "answer_contract": model_request
            .get("answer_contract")
            .cloned()
            .unwrap_or_else(|| json!({})),
        "privacy_contract": {
            "no_host_composed_answer": true,
            "must_write_in_model_voice": true,
            "must_reference_observation_only": true,
            "must_not_claim_missing_files": true,
            "must_not_include_private_paths_or_urls": true,
            "must_not_request_login_cookie_or_recording_bypass": true,
        },
        "no_host_composed_answer": true,
    }))
}

fn video_extraction_model_completion_follow_up(
    title: &str,
    state: &str,
    ready_file_kinds: &[String],
    html_artifact_ids: &[String],
    deliverable_status: &Value,
    deliverable_package: &Value,
    next_actions: &[Value],
) -> Value {
    let warnings = deliverable_status
        .get("warnings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let warning_count = deliverable_status
        .get("warning_count")
        .and_then(Value::as_u64)
        .unwrap_or(warnings.len() as u64);
    let warning_codes = warnings
        .iter()
        .filter_map(|warning| warning.get("code").and_then(Value::as_str))
        .map(str::to_string)
        .collect::<Vec<_>>();
    let missing_required_file_kinds = deliverable_package
        .get("missing_required_file_kinds")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let primary_next_action = next_actions.first().cloned().unwrap_or(Value::Null);

    json!({
        "kind": "video_extraction_model_completion_turn_request",
        "version": 1,
        "required": true,
        "turn_owner": "model",
        "source_event": "video_extraction.workflow_completed",
        "instruction": "Use this structured completion status to write the next assistant message in the model's own words. Mention only ready files and explicit missing/review items from the observation; do not claim missing files are available.",
        "completion_context": {
            "title": title,
            "status": state,
            "ready_file_kinds": ready_file_kinds,
            "html_artifact_ids": html_artifact_ids,
            "warning_count": warning_count,
            "warning_codes": warning_codes,
            "primary_next_action": primary_next_action,
            "missing_required_file_kinds": missing_required_file_kinds,
            "has_pptx": deliverable_status
                .get("has_pptx")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            "has_video_slides_markdown": deliverable_status
                .get("has_video_slides_markdown")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            "has_subtitle_page_map": deliverable_status
                .get("has_subtitle_page_map")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        },
        "answer_contract": {
            "must_write_in_model_voice": true,
            "must_reference_observation_only": true,
            "must_not_claim_missing_files": true,
            "must_not_include_private_paths_or_urls": true,
            "must_not_request_login_cookie_or_recording_bypass": true,
            "must_keep_missing_items_explicit": true,
            "no_host_composed_answer": true,
        },
    })
}

pub fn video_extraction_completion_audit_from_output(output: &Value) -> Value {
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
    let frame_extraction = output
        .get("frame_extraction")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let warning_codes = deliverable_warning_codes(&deliverable_status)
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let provider_failure_count = deliverable_status
        .get("warnings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|warning| warning.get("code").and_then(Value::as_str) == Some("provider_failure"))
        .count();
    let provider_failures = video_provider_failure_audit_summaries(&deliverable_status);
    let source_resolution = video_completion_source_resolution_audit(output);
    let artifact_kinds = files
        .iter()
        .filter_map(|file| file.get("artifact_kind").and_then(Value::as_str))
        .take(32)
        .map(str::to_string)
        .collect::<Vec<_>>();
    let artifact_group_counts = video_artifact_group_counts(&files);
    let output_status = output
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("partial");
    let deliverable_state = deliverable_status
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or(output_status);
    let warning_count = deliverable_status
        .get("warning_count")
        .and_then(Value::as_u64)
        .unwrap_or(warning_codes.len() as u64);

    json!({
        "kind": "video_extraction_completion_audit",
        "version": 1,
        "stateTransition": {
            "workflowTask": "extract_video_ppt",
            "outputStatus": output_status,
            "deliverableState": deliverable_state,
        },
        "state_transition": {
            "workflow_task": "extract_video_ppt",
            "output_status": output_status,
            "deliverable_state": deliverable_state,
        },
        "documentId": output.get("document_id").cloned().unwrap_or(Value::Null),
        "document_id": output.get("document_id").cloned().unwrap_or(Value::Null),
        "datasetId": output.get("dataset_id").cloned().unwrap_or(Value::Null),
        "dataset_id": output.get("dataset_id").cloned().unwrap_or(Value::Null),
        "frameExtractionStatus": frame_extraction
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown"),
        "frame_extraction_status": frame_extraction
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown"),
        "generatedArtifactsStatus": generated_artifacts
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown"),
        "generated_artifacts_status": generated_artifacts
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown"),
        "artifactGroupCounts": artifact_group_counts.clone(),
        "artifact_group_counts": artifact_group_counts,
        "artifactKinds": artifact_kinds.clone(),
        "artifact_kinds": artifact_kinds,
        "warningCodes": warning_codes.clone(),
        "warning_codes": warning_codes,
        "warning_count": warning_count,
        "provider_failure_count": provider_failure_count,
        "sourceResolution": source_resolution.clone(),
        "source_resolution": source_resolution,
        "providerFailures": provider_failures.clone(),
        "provider_failures": provider_failures,
        "redaction": {
            "raw_urls_included": false,
            "private_paths_included": false,
            "cookies_included": false,
            "provider_keys_included": false,
            "raw_provider_payloads_included": false,
        },
    })
}

fn video_source_summary_from_document(document: &Document) -> Value {
    let remote_media = document.metadata.get("remote_media");
    let source_type = remote_media
        .and_then(|value| value.get("source_type"))
        .and_then(Value::as_str)
        .unwrap_or_else(|| {
            if video_string_is_http_url(&document.object_key) {
                "direct_video_url"
            } else {
                "uploaded_or_local_video"
            }
        });
    let asset_state = remote_media
        .and_then(|value| value.get("asset_state"))
        .and_then(Value::as_str)
        .unwrap_or_else(|| {
            if video_string_is_http_url(&document.object_key) {
                "remote_registered"
            } else {
                "local_or_uploaded"
            }
        });
    let source_url_present = remote_media
        .and_then(|value| value.get("source_url"))
        .and_then(Value::as_str)
        .map(|value| !value.trim().is_empty())
        .unwrap_or_else(|| video_string_is_http_url(&document.object_key));
    let source_page_url_present = remote_media
        .and_then(|value| value.get("source_page_url"))
        .and_then(Value::as_str)
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    let ingest_requires_env = remote_media
        .and_then(|value| value.get("ingest_requires_env"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string);

    json!({
        "kind": "video_source_summary",
        "source_type": source_type,
        "asset_state": asset_state,
        "content_type": document.content_type,
        "source_url_present": source_url_present,
        "source_page_url_present": source_page_url_present,
        "source_url_redacted": source_url_present,
        "source_page_url_redacted": source_page_url_present,
        "object_key_redacted": video_string_is_http_url(&document.object_key),
        "ingest_requires_env": ingest_requires_env,
    })
}

fn video_completion_source_resolution_audit(output: &Value) -> Value {
    let source_summary = output
        .get("source_summary")
        .or_else(|| output.get("sourceSummary"));
    let source_type = video_value_string(source_summary, &["source_type", "sourceType"], "unknown");
    let asset_state = video_value_string(source_summary, &["asset_state", "assetState"], "unknown");
    let content_type =
        video_value_string(source_summary, &["content_type", "contentType"], "unknown");
    let source_url_present = video_value_bool(
        source_summary,
        &["source_url_present", "sourceUrlPresent"],
        false,
    );
    let source_page_url_present = video_value_bool(
        source_summary,
        &["source_page_url_present", "sourcePageUrlPresent"],
        false,
    );
    let ingest_requires_env = source_summary
        .and_then(|value| {
            value
                .get("ingest_requires_env")
                .or_else(|| value.get("ingestRequiresEnv"))
        })
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string);

    json!({
        "source_type": source_type,
        "asset_state": asset_state,
        "content_type": content_type,
        "source_url_present": source_url_present,
        "source_page_url_present": source_page_url_present,
        "source_url_redacted": source_url_present,
        "source_page_url_redacted": source_page_url_present,
        "ingest_requires_env": ingest_requires_env,
    })
}

fn video_provider_failure_audit_summaries(deliverable_status: &Value) -> Vec<Value> {
    deliverable_status
        .get("warnings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|warning| warning.get("code").and_then(Value::as_str) == Some("provider_failure"))
        .flat_map(|warning| {
            warning
                .get("providers")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_else(|| vec![warning.clone()])
        })
        .take(16)
        .map(|provider| {
            json!({
                "provider": provider
                    .get("provider")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown"),
                "capability": provider
                    .get("capability")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown"),
                "status": provider
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("failed"),
                "supported": provider
                    .get("supported")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            })
        })
        .collect()
}

fn video_value_string(source: Option<&Value>, keys: &[&str], fallback: &str) -> String {
    source
        .and_then(|value| {
            keys.iter()
                .find_map(|key| value.get(*key).and_then(Value::as_str))
        })
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn video_value_bool(source: Option<&Value>, keys: &[&str], fallback: bool) -> bool {
    source
        .and_then(|value| keys.iter().find_map(|key| value.get(*key)))
        .and_then(Value::as_bool)
        .unwrap_or(fallback)
}

fn video_string_is_http_url(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.starts_with("http://") || trimmed.starts_with("https://")
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

pub fn video_extraction_durable_published_version_manifest(
    assistant_run_id: &str,
    workflow_execution_id: &str,
    output: &Value,
) -> Option<Value> {
    let document_id = output.get("document_id").and_then(Value::as_str)?;
    let dataset_id = output.get("dataset_id").and_then(Value::as_str)?;
    let title = output
        .get("title")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("视频 PPT 提取");
    let generated_artifacts = output
        .get("generated_artifacts")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let files = generated_artifacts
        .get("files")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let deliverable_status = output
        .get("deliverable_status")
        .cloned()
        .unwrap_or_else(|| video_deliverable_status(&generated_artifacts));
    let ready_file_kinds = files
        .iter()
        .filter_map(|file| file.get("artifact_kind").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    let required_file_kinds =
        video_required_published_deliverable_kinds(&files, &deliverable_status);
    let missing_required_file_kinds = required_file_kinds
        .iter()
        .copied()
        .filter(|kind| !ready_file_kinds.contains(kind))
        .map(str::to_string)
        .collect::<Vec<_>>();
    let source_ready = deliverable_status.get("state").and_then(Value::as_str)
        == Some("final_pptx_ready")
        && missing_required_file_kinds.is_empty();
    if !source_ready {
        return None;
    }

    let package_key = format!("video-ppt-{assistant_run_id}-{document_id}");
    let published_files = video_public_artifact_files_by_kinds(&files, &required_file_kinds);
    let version_fingerprint =
        video_durable_published_version_fingerprint(document_id, dataset_id, &published_files);
    Some(json!({
        "manifest_type": "v3.video_ppt_durable_published_version.v1",
        "source": "media_worker_durable_published_version",
        "assistant_run_id": assistant_run_id,
        "workflow_execution_id": workflow_execution_id,
        "document_id": document_id,
        "dataset_id": dataset_id,
        "package_key": package_key,
        "version_fingerprint": version_fingerprint,
        "title": video_safe_evidence_text(title),
        "lifecycle_state": "published_version_ready",
        "generated_history_scope": "generated_artifact_workspace",
        "durable_history_status": "promoted_to_storage",
        "source_history_file_name": DEFAULT_PUBLISHED_VERSION_HISTORY_FILE_NAME,
        "published_manifest_file_name": DEFAULT_PUBLISHED_DELIVERABLE_MANIFEST_FILE_NAME,
        "ready_file_kinds": video_ready_file_kinds(&files),
        "required_file_kinds": required_file_kinds.clone(),
        "missing_required_file_kinds": missing_required_file_kinds,
        "deliverable_status": deliverable_status,
        "published_files": published_files,
        "publish_policy": {
            "path_policy": "public_manifest_entries_redact_local_paths",
            "mutation_policy": "append_new_durable_version_for_customer_visible_changes",
            "storage_policy": "database_history_keeps_redacted_artifact_pointer_metadata_without_copying_private_source_media",
        },
        "no_host_composed_answer": true,
    }))
}

pub fn merge_video_extraction_durable_published_version_ref(
    output_artifacts: &Value,
    assistant_run_id: &str,
    document_id: &str,
    durable_ref: &Value,
) -> Value {
    if durable_ref.is_null() {
        return output_artifacts
            .as_array()
            .cloned()
            .map(Value::Array)
            .unwrap_or_else(|| json!([]));
    }
    let target_id = format!("video-extraction-{assistant_run_id}-{document_id}");
    let mut artifacts = output_artifacts.as_array().cloned().unwrap_or_default();
    for artifact in &mut artifacts {
        if artifact.get("id").and_then(Value::as_str) != Some(target_id.as_str()) {
            continue;
        }
        let Some(object) = artifact.as_object_mut() else {
            continue;
        };
        object.insert("durable_published_version".to_string(), durable_ref.clone());
        if let Some(package) = object
            .get_mut("deliverable_package")
            .and_then(Value::as_object_mut)
        {
            package.insert(
                "durable_history_status".to_string(),
                json!("promoted_to_storage"),
            );
            package.insert("durable_published_version".to_string(), durable_ref.clone());
        }
    }
    Value::Array(artifacts)
}

fn video_durable_published_version_fingerprint(
    document_id: &str,
    dataset_id: &str,
    published_files: &[Value],
) -> String {
    let mut parts = published_files
        .iter()
        .filter_map(|file| {
            let kind = file.get("artifact_kind").and_then(Value::as_str)?;
            let file_name = file.get("file_name").and_then(Value::as_str).unwrap_or("");
            let uri = file.get("uri").and_then(Value::as_str).unwrap_or("");
            Some(format!("{kind}:{file_name}:{uri}"))
        })
        .collect::<Vec<_>>();
    parts.sort();
    format!(
        "video-ppt:v1:{document_id}:{dataset_id}:{}",
        parts.join("|")
    )
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
                    | "published_deliverable_manifest"
                    | "published_version_history"
                    | "extraction_artifacts_manifest"
                    | "slide_rectangles_manifest"
                    | "ppt_outline"
                    | "slide_notes"
                    | "video_slides_markdown"
                    | "subtitle_page_map"
                    | "transcript_text"
                    | "source_text"
                    | "timestamp_map"
            )
        })
        .map(str::to_string)
        .collect()
}

fn video_artifact_group_counts(files: &[Value]) -> Value {
    json!({
        "manifest_outputs": video_artifact_files_by_kinds(files, &[
            "final_deliverables_manifest",
            "published_deliverable_manifest",
            "published_version_history",
            "extraction_artifacts_manifest",
        ]).len(),
        "final_outputs": video_artifact_files_by_kinds(files, &["pptx", "video_slides_markdown"]).len(),
        "review_outputs": video_artifact_files_by_kinds(files, &[
            "slide_image_candidates",
            "contact_sheet_plan",
            "contact_sheet_html",
            "ppt_keep_list_template",
            "selected_slides_manifest",
            "slide_rectangles_manifest",
            "slide_quality_report",
            "slide_notes",
            "pptx_build_plan",
        ]).len(),
        "evidence_outputs": video_artifact_files_by_kinds(files, &[
            "frame_manifest",
            "transcript_text",
            "source_text",
            "ppt_outline",
            "timestamp_map",
            "subtitle_page_map",
        ]).len(),
    })
}

fn video_extraction_completion_next_actions(
    state: &str,
    files: &[Value],
    deliverable_status: &Value,
) -> Vec<Value> {
    let file_kinds = files
        .iter()
        .filter_map(|file| file.get("artifact_kind").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    let mut actions = vec![json!("open_video_extraction_summary")];
    let warning_codes = deliverable_warning_codes(deliverable_status);
    if warning_codes.contains("frame_extraction_failed") {
        actions.push(json!("retry_frame_extraction"));
    }
    if warning_codes.contains("frame_extraction_skipped") {
        actions.push(json!("provide_local_media_file_or_parsed_frames"));
    }
    if warning_codes.contains("generated_artifacts_failed") {
        actions.push(json!("retry_generated_artifact_writer"));
    }
    if warning_codes.contains("missing_transcript_alignment") {
        actions.push(json!("attach_or_parse_transcript_evidence"));
    }
    if warning_codes.contains("missing_contact_sheet") {
        actions.push(json!("generate_contact_sheet_from_raw_frames"));
    }
    if warning_codes.contains("no_slide_rectangle_found") {
        actions.push(json!(
            "review_contact_sheet_or_promote_rectangle_extraction"
        ));
    }
    if warning_codes.contains("full_frame_rectangle_fallback") {
        actions.push(json!("review_slide_rectangles_manifest"));
    }
    if warning_codes.contains("selected_slide_duplicates_removed") {
        actions.push(json!("review_slide_dedupe_manifest"));
    }
    if warning_codes.contains("published_version_missing") {
        actions.push(json!("persist_video_published_version"));
    }
    if warning_codes.contains("keep_list_not_confirmed") {
        actions.push(json!("fill_ppt_keep_list_template"));
    }
    if warning_codes.contains("low_confidence_transcript") {
        actions.push(json!("review_low_confidence_transcript"));
    }
    if warning_codes.contains("subtitle_ocr_low_confidence") {
        actions.push(json!("rerun_or_review_subtitle_ocr"));
    }
    if warning_codes.contains("parse_partial") {
        actions.push(json!("rerun_or_refresh_video_parse"));
    }
    if warning_codes.contains("provider_failure") {
        actions.push(json!("check_video_provider_configuration"));
    }
    if file_kinds.contains("pptx") {
        actions.push(json!("download_pptx"));
    }
    if file_kinds.contains("final_deliverables_manifest") {
        actions.push(json!("review_final_deliverables_manifest"));
    }
    if file_kinds.contains("published_deliverable_manifest") {
        actions.push(json!("review_published_deliverable_manifest"));
    }
    if file_kinds.contains("published_version_history") {
        actions.push(json!("review_published_version_history"));
    }
    if file_kinds.contains("extraction_artifacts_manifest") {
        actions.push(json!("review_extraction_artifacts_manifest"));
    }
    if file_kinds.contains("slide_rectangles_manifest") {
        actions.push(json!("review_slide_rectangles_manifest"));
    }
    if file_kinds.contains("slide_notes") {
        actions.push(json!("review_slide_notes"));
    }
    if file_kinds.contains("video_slides_markdown") {
        actions.push(json!("review_video_slides_markdown"));
    }
    if file_kinds.contains("subtitle_page_map") {
        actions.push(json!("review_subtitle_page_map"));
    }
    if state != "final_pptx_ready" {
        actions.push(json!("complete_keep_list_or_review_missing_inputs"));
    }
    actions
}

fn video_extraction_completion_user_notification(
    state: &str,
    ready_file_kinds: &[String],
    html_artifact_ids: &[String],
    deliverable_status: &Value,
    next_actions: &[Value],
) -> Value {
    let warning_count = deliverable_status
        .get("warning_count")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| {
            deliverable_status
                .get("warnings")
                .and_then(Value::as_array)
                .map(|warnings| warnings.len() as u64)
                .unwrap_or(0)
        });
    let has_pptx = deliverable_status
        .get("has_pptx")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let ready_file_count = ready_file_kinds.len();
    let primary_next_action = next_actions.first().cloned().unwrap_or(Value::Null);
    let (severity, title, message) = if has_pptx || state == "final_pptx_ready" {
        (
            "success",
            "视频/PPT 提取已完成",
            "后台任务已生成可下载 PPTX，并已更新右侧视频提取摘要。",
        )
    } else if warning_count > 0 {
        (
            "warning",
            "视频/PPT 提取需要复核",
            "后台任务已更新视频提取摘要，但仍有缺失证据或待复核交付项。",
        )
    } else if ready_file_count > 0 {
        (
            "info",
            "视频/PPT 产物已更新",
            "后台任务已生成部分可复核文件，并已更新右侧视频提取摘要。",
        )
    } else {
        (
            "warning",
            "视频/PPT 提取未产生交付文件",
            "后台任务已结束，但当前没有可下载交付文件；请查看摘要中的下一步。",
        )
    };

    json!({
        "kind": "video_extraction_status_notification",
        "channel": "assistant_run_artifact_status",
        "scope": "status_only",
        "user_visible": true,
        "severity": severity,
        "title": title,
        "message": message,
        "status": state,
        "ready_file_count": ready_file_count,
        "warning_count": warning_count,
        "html_artifact_ids": html_artifact_ids,
        "primary_next_action": primary_next_action,
        "no_host_composed_answer": true,
    })
}

fn deliverable_warning_codes(deliverable_status: &Value) -> BTreeSet<&str> {
    deliverable_status
        .get("warnings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|warning| warning.get("code").and_then(Value::as_str))
        .collect()
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

fn resolve_media_frame_extraction_input(document: &Document) -> Option<VideoFrameExtractionInput> {
    if let Some(path) = resolve_local_media_input_path(document) {
        return Some(VideoFrameExtractionInput::LocalPath(path));
    }

    let remote_url = document
        .metadata
        .get("remote_media")
        .and_then(|value| value.get("source_url"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| document.object_key.trim());
    if video_remote_media_url_allowed(remote_url) {
        Some(VideoFrameExtractionInput::RemoteUrl(remote_url.to_string()))
    } else {
        None
    }
}

fn video_remote_media_url_allowed(raw: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(raw.trim()) else {
        return false;
    };
    if !matches!(url.scheme(), "http" | "https") {
        return false;
    }
    let host = url.host_str().unwrap_or_default();
    if !video_remote_media_host_allowed(host) {
        return false;
    }
    let lower = url.as_str().to_ascii_lowercase();
    if lower.contains("weixin.qq.com/sph/") || lower.contains("channels.weixin.qq.com/sph/") {
        return false;
    }
    video_remote_media_url_has_video_extension(url.as_str())
}

fn video_remote_media_host_allowed(host: &str) -> bool {
    let lower = host.trim().trim_matches(['[', ']']).to_ascii_lowercase();
    if lower.is_empty()
        || lower == "localhost"
        || lower.ends_with(".localhost")
        || lower.ends_with(".local")
    {
        return false;
    }
    if let Ok(ip) = lower.parse::<IpAddr>() {
        return match ip {
            IpAddr::V4(ip) => {
                !(ip.is_private()
                    || ip.is_loopback()
                    || ip.is_link_local()
                    || ip.is_broadcast()
                    || ip.is_documentation()
                    || ip.octets()[0] == 0)
            }
            IpAddr::V6(ip) => !(ip.is_loopback() || ip.is_unspecified() || ip.is_unique_local()),
        };
    }
    true
}

fn video_remote_media_url_has_video_extension(raw: &str) -> bool {
    video_remote_media_url_extension(raw).is_some()
}

fn video_remote_media_url_extension(raw: &str) -> Option<&'static str> {
    let Ok(url) = reqwest::Url::parse(raw) else {
        return None;
    };
    let path = url.path().to_ascii_lowercase();
    [
        ".mp4", ".mov", ".m4v", ".webm", ".mkv", ".avi", ".mpeg", ".mpg",
    ]
    .into_iter()
    .find(|extension| path.ends_with(extension))
}

fn video_remote_media_response_type_allowed(response_type: &str) -> bool {
    let media_type = response_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    media_type.is_empty()
        || media_type.starts_with("video/")
        || media_type == "application/octet-stream"
}

fn video_remote_media_cache_key(value: &str) -> String {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
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

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
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
    let has_published_deliverable_manifest =
        artifact_kinds.contains("published_deliverable_manifest");
    let has_published_version_history = artifact_kinds.contains("published_version_history");
    let has_extraction_artifacts_manifest =
        artifact_kinds.contains("extraction_artifacts_manifest");
    let has_slide_notes = artifact_kinds.contains("slide_notes");
    let has_video_slides_markdown = artifact_kinds.contains("video_slides_markdown");
    let has_slide_rectangles_manifest = artifact_kinds.contains("slide_rectangles_manifest");
    let has_slide_quality_report = artifact_kinds.contains("slide_quality_report");
    let has_subtitle_page_map = artifact_kinds.contains("subtitle_page_map");
    let mut warnings = video_generated_artifact_quality_warnings(
        &files,
        has_pptx,
        has_selected_slides,
        has_contact_sheet,
        has_transcript,
        has_subtitle_page_map,
        has_published_deliverable_manifest,
        has_published_version_history,
    );
    let generated_artifacts_status = generated_artifacts
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("planned");
    let generated_artifacts_reason = generated_artifacts
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if generated_artifacts_status == "failed" {
        let mut warning = json!({
            "code": "generated_artifacts_failed",
            "severity": "high",
            "message": "Generated video/PPT artifact writing failed; final files may be missing until the artifact writer is retried successfully."
        });
        if !generated_artifacts_reason.is_empty() {
            warning["reason"] = json!(generated_artifacts_reason);
        }
        warnings.push(warning);
    }
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
        "has_published_deliverable_manifest": has_published_deliverable_manifest,
        "has_published_version_history": has_published_version_history,
        "has_extraction_artifacts_manifest": has_extraction_artifacts_manifest,
        "has_slide_rectangles_manifest": has_slide_rectangles_manifest,
        "has_slide_quality_report": has_slide_quality_report,
        "has_slide_notes": has_slide_notes,
        "has_video_slides_markdown": has_video_slides_markdown,
        "has_subtitle_page_map": has_subtitle_page_map,
        "has_pptx": has_pptx,
        "generated_artifacts_status": generated_artifacts_status,
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
            append_deliverable_warning(object, warning);
        }
    }
    status
}

fn video_deliverable_status_with_evidence_and_frame_extraction(
    generated_artifacts: &Value,
    frame_extraction: &Value,
    evidence: &VideoMediaEvidenceItems,
) -> Value {
    let mut status =
        video_deliverable_status_with_frame_extraction(generated_artifacts, frame_extraction);
    if let Some(object) = status.as_object_mut() {
        if let Some(warning) = video_partial_evidence_warning(evidence) {
            append_deliverable_warning(object, warning);
        }
        if let Some(warning) = video_provider_failure_warning(evidence) {
            append_deliverable_warning(object, warning);
        }
        for warning in video_low_confidence_evidence_warnings(evidence) {
            append_deliverable_warning(object, warning);
        }
    }
    status
}

fn append_deliverable_warning(status: &mut serde_json::Map<String, Value>, warning: Value) {
    let warnings = status
        .entry("warnings".to_string())
        .or_insert_with(|| json!([]));
    let mut next_warning_count = None;
    if let Some(warnings) = warnings.as_array_mut() {
        warnings.push(warning);
        next_warning_count = Some(warnings.len());
    }
    if let Some(next_warning_count) = next_warning_count {
        status.insert("warning_count".to_string(), json!(next_warning_count));
    }
}

fn video_generated_artifact_quality_warnings(
    files: &[Value],
    has_pptx: bool,
    has_selected_slides: bool,
    has_contact_sheet: bool,
    has_transcript: bool,
    has_subtitle_page_map: bool,
    has_published_deliverable_manifest: bool,
    has_published_version_history: bool,
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
    if video_has_raw_frame_candidates(files) && !video_has_promoted_slide_rectangles(files) {
        warnings.push(json!({
            "code": "no_slide_rectangle_found",
            "severity": "medium",
            "message": "No promoted slide-rectangle extraction is available yet; current candidates are raw frames/contact-sheet entries and require keep-list review before final delivery."
        }));
    } else if video_has_full_frame_rectangle_fallback(files) {
        warnings.push(json!({
            "code": "full_frame_rectangle_fallback",
            "severity": "low",
            "message": "Slide rectangles are promoted as full-frame fallback crops; review or replace them when visual rectangle detection is available."
        }));
    }
    if let Some(warning) = video_selected_slide_dedupe_warning(files) {
        warnings.push(warning);
    }
    if has_pptx {
        warnings.push(json!({
            "code": "screenshot_based_pptx",
            "severity": "low",
            "message": "PPTX uses raster screenshots; editable native slide reconstruction is not included in this slice."
        }));
        if !has_published_deliverable_manifest || !has_published_version_history {
            warnings.push(json!({
                "code": "published_version_missing",
                "severity": "medium",
                "message": "Final PPTX files are downloadable, but immutable published-version metadata is not complete yet."
            }));
        }
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

fn video_selected_slide_dedupe_warning(files: &[Value]) -> Option<Value> {
    let dedupe_manifest = files
        .iter()
        .find(|file| {
            file.get("artifact_kind").and_then(Value::as_str) == Some("slide_rectangles_manifest")
                && file
                    .get("deduped_candidate_count")
                    .and_then(Value::as_u64)
                    .is_some_and(|count| count > 0)
        })
        .or_else(|| {
            files.iter().find(|file| {
                file.get("artifact_kind").and_then(Value::as_str)
                    == Some("selected_slides_manifest")
                    && file
                        .get("deduped_candidate_count")
                        .and_then(Value::as_u64)
                        .is_some_and(|count| count > 0)
            })
        })?;
    let deduped_candidate_count = dedupe_manifest
        .get("deduped_candidate_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if deduped_candidate_count == 0 {
        return None;
    }
    let exact_duplicate_count = dedupe_manifest
        .get("exact_duplicate_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let visual_duplicate_count = dedupe_manifest
        .get("visual_duplicate_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let dedupe_status = dedupe_manifest
        .get("dedupe_status")
        .and_then(Value::as_str)
        .unwrap_or("selected_keep_list_order_deduped");
    let message = if visual_duplicate_count > 0 && exact_duplicate_count > 0 {
        "Selected slide candidates included exact duplicate frames and conservative visual duplicates; DataMax removed them before rectangle promotion and PPTX generation."
    } else if visual_duplicate_count > 0 {
        "Selected slide candidates included conservative visual duplicates; DataMax removed them before rectangle promotion and PPTX generation."
    } else if exact_duplicate_count > 0 {
        "Selected slide candidates included exact duplicate frame bytes; DataMax removed them before rectangle promotion and PPTX generation."
    } else {
        "Selected slide candidates included duplicates; DataMax removed them before rectangle promotion and PPTX generation."
    };
    Some(json!({
        "code": "selected_slide_duplicates_removed",
        "severity": "low",
        "message": message,
        "dedupe_status": dedupe_status,
        "deduped_candidate_count": deduped_candidate_count,
        "exact_duplicate_count": exact_duplicate_count,
        "visual_duplicate_count": visual_duplicate_count,
    }))
}

fn video_has_raw_frame_candidates(files: &[Value]) -> bool {
    files.iter().any(|file| {
        file.get("artifact_kind").and_then(Value::as_str) == Some("slide_image_candidates")
            && file
                .get("candidate_count")
                .and_then(Value::as_u64)
                .is_some_and(|candidate_count| candidate_count > 0)
    })
}

fn video_has_promoted_slide_rectangles(files: &[Value]) -> bool {
    files.iter().any(|file| {
        file.get("rectangle_extraction_status")
            .and_then(Value::as_str)
            .is_some_and(|status| {
                matches!(
                    status,
                    "completed"
                        | "promoted"
                        | "available"
                        | "promoted_full_frame_fallback"
                        | "promoted_detector_crop"
                        | "mixed_detector_and_full_frame_fallback"
                )
            })
    })
}

fn video_has_full_frame_rectangle_fallback(files: &[Value]) -> bool {
    files.iter().any(|file| {
        file.get("artifact_kind").and_then(Value::as_str) == Some("slide_rectangles_manifest")
            && file
                .get("rectangle_extraction_mode")
                .and_then(Value::as_str)
                .is_some_and(|mode| {
                    matches!(
                        mode,
                        "full_frame_fallback" | "mixed_detector_and_full_frame_fallback"
                    )
                })
            && file
                .get("promoted_rectangle_count")
                .and_then(Value::as_u64)
                .is_some_and(|count| count > 0)
    })
}

fn video_partial_evidence_warning(evidence: &VideoMediaEvidenceItems) -> Option<Value> {
    let mut missing = Vec::new();
    if evidence.transcript_segments.is_empty() {
        missing.push("transcript_segments");
    }
    if evidence.scenes.is_empty() {
        missing.push("scene_windows");
    }
    if evidence.keyframe_ocr_snippets.is_empty() {
        missing.push("keyframe_ocr_snippets");
    }
    if missing.is_empty() {
        return None;
    }

    let has_any = evidence.has_any();
    Some(json!({
        "code": "parse_partial",
        "severity": if has_any { "medium" } else { "high" },
        "message": if has_any {
            "Video parsing produced only partial evidence; rerun parsing or attach missing transcript/scene/OCR evidence before final delivery."
        } else {
            "Video parsing has not produced transcript, scene, or keyframe OCR evidence yet; final PPT/source-text delivery is blocked."
        },
        "missing_evidence": missing,
        "observed_evidence": {
            "transcript_segment_count": evidence.transcript_segments.len(),
            "scene_count": evidence.scenes.len(),
            "keyframe_ocr_snippet_count": evidence.keyframe_ocr_snippets.len(),
        }
    }))
}

fn video_low_confidence_evidence_warnings(evidence: &VideoMediaEvidenceItems) -> Vec<Value> {
    let low_confidence_transcript_count = evidence
        .transcript_segments
        .iter()
        .filter(|segment| {
            video_evidence_confidence_below_threshold(
                segment,
                &["confidence", "transcript_confidence"],
            )
        })
        .count();
    let low_confidence_ocr_count = evidence
        .keyframe_ocr_snippets
        .iter()
        .filter(|snippet| {
            video_evidence_confidence_below_threshold(
                snippet,
                &["confidence", "ocr_confidence", "text_confidence"],
            )
        })
        .count();

    let mut warnings = Vec::new();
    if low_confidence_transcript_count > 0 {
        warnings.push(json!({
            "code": "low_confidence_transcript",
            "severity": "medium",
            "message": "Some transcript segments have low confidence; review or replace them before using narration as final source text.",
            "count": low_confidence_transcript_count,
            "threshold": LOW_CONFIDENCE_VIDEO_EVIDENCE_THRESHOLD,
        }));
    }
    if low_confidence_ocr_count > 0 {
        warnings.push(json!({
            "code": "subtitle_ocr_low_confidence",
            "severity": "medium",
            "message": "Some keyframe/subtitle OCR snippets have low confidence; rerun OCR or manually verify text before final PPT delivery.",
            "count": low_confidence_ocr_count,
            "threshold": LOW_CONFIDENCE_VIDEO_EVIDENCE_THRESHOLD,
        }));
    }
    warnings
}

fn video_provider_failure_warning(evidence: &VideoMediaEvidenceItems) -> Option<Value> {
    let failed_providers = evidence
        .provider_evidence
        .iter()
        .filter(|item| video_provider_evidence_is_failure(item))
        .map(|item| {
            json!({
                "provider": video_item_text(item, &["provider"]).unwrap_or_else(|| "unknown".to_string()),
                "capability": video_item_text(item, &["capability"]).unwrap_or_else(|| "unknown".to_string()),
                "status": video_item_text(item, &["status"]).unwrap_or_else(|| "unknown".to_string()),
            })
        })
        .collect::<Vec<_>>();
    if failed_providers.is_empty() {
        return None;
    }

    let all_providers_failed = failed_providers.len() == evidence.provider_evidence.len();
    Some(json!({
        "code": "provider_failure",
        "severity": if all_providers_failed { "high" } else { "medium" },
        "message": if all_providers_failed {
            "All recorded video parsing provider capabilities are unavailable or unsupported; refresh provider configuration before relying on extracted media evidence."
        } else {
            "At least one recorded video parsing provider capability is unavailable or unsupported; review provider configuration before final delivery."
        },
        "failed_provider_count": failed_providers.len(),
        "providers": failed_providers,
    }))
}

fn video_provider_evidence_is_failure(value: &Value) -> bool {
    if value
        .get("supported")
        .and_then(Value::as_bool)
        .is_some_and(|supported| !supported)
    {
        return true;
    }

    let status = value
        .get("status")
        .and_then(Value::as_str)
        .map(|status| status.trim().to_ascii_lowercase())
        .unwrap_or_default();
    matches!(
        status.as_str(),
        "failed"
            | "error"
            | "unavailable"
            | "unsupported"
            | "not_supported"
            | "configured_unverified"
            | "probe_missing"
    )
}

fn video_evidence_confidence_below_threshold(value: &Value, keys: &[&str]) -> bool {
    keys.iter().any(|key| {
        value
            .get(*key)
            .and_then(Value::as_f64)
            .is_some_and(|confidence| {
                confidence >= 0.0 && confidence < LOW_CONFIDENCE_VIDEO_EVIDENCE_THRESHOLD
            })
    })
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
        "manifest_outputs": video_public_artifact_files_by_kinds(files, &[
            "final_deliverables_manifest",
            "published_deliverable_manifest",
            "published_version_history",
            "extraction_artifacts_manifest",
        ]),
        "final_outputs": video_public_artifact_files_by_kinds(files, &["pptx", "video_slides_markdown"]),
        "review_outputs": video_public_artifact_files_by_kinds(files, &[
            "slide_image_candidates",
            "contact_sheet_plan",
            "contact_sheet_html",
            "ppt_keep_list_template",
            "selected_slides_manifest",
            "slide_rectangles_manifest",
            "slide_quality_report",
            "slide_notes",
            "pptx_build_plan",
        ]),
        "evidence_outputs": video_public_artifact_files_by_kinds(files, &[
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

fn video_published_deliverable_manifest(
    document: &Document,
    files: &[Value],
    frame_count: u64,
) -> Value {
    let generated_artifacts = json!({
        "status": "completed",
        "files": files,
    });
    let deliverable_status = video_deliverable_status(&generated_artifacts);
    let ready_file_kinds = files
        .iter()
        .filter_map(|file| file.get("artifact_kind").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    let required_file_kinds =
        video_required_published_deliverable_kinds(files, &deliverable_status);
    let missing_required_file_kinds = required_file_kinds
        .iter()
        .copied()
        .filter(|kind| !ready_file_kinds.contains(kind))
        .map(str::to_string)
        .collect::<Vec<_>>();
    let source_ready = deliverable_status.get("state").and_then(Value::as_str)
        == Some("final_pptx_ready")
        && missing_required_file_kinds.is_empty();
    let lifecycle_state = if source_ready {
        "published_version_ready"
    } else {
        "not_ready"
    };

    json!({
        "manifest_type": "v3.video_ppt_published_deliverable.v1",
        "status": lifecycle_state,
        "source": "media_worker_published_deliverable_manifest",
        "document_id": document.id.to_string(),
        "dataset_id": document.dataset_id.to_string(),
        "title": document.title,
        "frame_count": frame_count,
        "lifecycle_state": lifecycle_state,
        "published": source_ready,
        "immutable_version": source_ready,
        "version_no": if source_ready { json!(1) } else { Value::Null },
        "version_label": if source_ready { "v1" } else { "draft" },
        "deliverable_status": deliverable_status,
        "required_file_kinds": required_file_kinds.clone(),
        "missing_required_file_kinds": missing_required_file_kinds,
        "published_files": video_public_artifact_files_by_kinds(files, &required_file_kinds),
        "manifest_outputs": video_public_artifact_files_by_kinds(files, &[
            "final_deliverables_manifest",
            "published_deliverable_manifest",
            "published_version_history",
            "extraction_artifacts_manifest",
        ]),
        "final_outputs": video_public_artifact_files_by_kinds(files, &["pptx", "video_slides_markdown"]),
        "review_outputs": video_public_artifact_files_by_kinds(files, &[
            "slide_image_candidates",
            "contact_sheet_plan",
            "contact_sheet_html",
            "ppt_keep_list_template",
            "selected_slides_manifest",
            "slide_rectangles_manifest",
            "slide_quality_report",
            "slide_notes",
            "pptx_build_plan",
        ]),
        "evidence_outputs": video_public_artifact_files_by_kinds(files, &[
            "frame_manifest",
            "transcript_text",
            "source_text",
            "ppt_outline",
            "timestamp_map",
            "subtitle_page_map",
        ]),
        "publish_policy": {
            "path_policy": "public_manifest_entries_redact_local_paths",
            "mutation_policy": "create_a_new_version_for_customer_visible_changes",
            "content_policy": "published_manifest_records_artifact_pointer_metadata_without_copying_private_source_media",
            "history_policy": "package_level_history_records_latest_published_version_and_is_ready_for_later_durable_storage_promotion",
        },
        "no_host_composed_answer": true,
    })
}

fn video_published_version_history_manifest(
    document: &Document,
    files: &[Value],
    frame_count: u64,
) -> Value {
    let generated_artifacts = json!({
        "status": "completed",
        "files": files,
    });
    let deliverable_status = video_deliverable_status(&generated_artifacts);
    let ready_file_kinds = files
        .iter()
        .filter_map(|file| file.get("artifact_kind").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    let required_file_kinds =
        video_required_published_deliverable_kinds(files, &deliverable_status);
    let missing_required_file_kinds = required_file_kinds
        .iter()
        .copied()
        .filter(|kind| !ready_file_kinds.contains(kind))
        .map(str::to_string)
        .collect::<Vec<_>>();
    let source_ready = deliverable_status.get("state").and_then(Value::as_str)
        == Some("final_pptx_ready")
        && missing_required_file_kinds.is_empty();
    let version_entry = if source_ready {
        json!({
            "version_no": 1,
            "version_label": "v1",
            "lifecycle_state": "published_version_ready",
            "published": true,
            "immutable_version": true,
            "frame_count": frame_count,
            "published_manifest_file_name": DEFAULT_PUBLISHED_DELIVERABLE_MANIFEST_FILE_NAME,
            "file_count": required_file_kinds.len(),
            "artifact_kinds": required_file_kinds.clone(),
            "published_files": video_public_artifact_files_by_kinds(files, &required_file_kinds),
        })
    } else {
        Value::Null
    };
    let versions = if version_entry.is_null() {
        Vec::new()
    } else {
        vec![version_entry]
    };

    json!({
        "manifest_type": "v3.video_ppt_published_version_history.v1",
        "status": if source_ready { "history_ready" } else { "not_ready" },
        "source": "media_worker_published_version_history",
        "document_id": document.id.to_string(),
        "dataset_id": document.dataset_id.to_string(),
        "title": document.title,
        "history_scope": "generated_artifact_workspace",
        "durable_history_status": "pending_storage_promotion",
        "frame_count": frame_count,
        "latest_version_no": if source_ready { json!(1) } else { Value::Null },
        "latest_version_label": if source_ready { json!("v1") } else { Value::Null },
        "version_count": versions.len(),
        "required_file_kinds": required_file_kinds,
        "missing_required_file_kinds": missing_required_file_kinds,
        "deliverable_status": deliverable_status,
        "manifest_outputs": video_public_artifact_files_by_kinds(files, &[
            "final_deliverables_manifest",
            "published_deliverable_manifest",
            "published_version_history",
            "extraction_artifacts_manifest",
        ]),
        "versions": versions,
        "publish_policy": {
            "path_policy": "public_manifest_entries_redact_local_paths",
            "mutation_policy": "append_new_version_for_customer_visible_changes",
            "durable_history_policy": "promote_this_package_level_history_to_storage_when_the_published_artifact_store_is_available",
        },
        "no_host_composed_answer": true,
    })
}

fn video_public_timestamp_map(
    document: &Document,
    evidence: &VideoMediaEvidenceItems,
    frame_extraction: &Value,
) -> Value {
    json!({
        "document_id": document.id.to_string(),
        "dataset_id": document.dataset_id.to_string(),
        "title": document.title,
        "redaction": {
            "status": "applied",
            "policy": "paths_urls_tokens_and_provider_secrets_are_redacted",
        },
        "transcript_segments": video_public_evidence_array(&evidence.transcript_segments),
        "scenes": video_public_evidence_array(&evidence.scenes),
        "keyframe_ocr_snippets": video_public_evidence_array(&evidence.keyframe_ocr_snippets),
        "frame_extraction": video_public_evidence_value(frame_extraction),
    })
}

fn video_public_json_bytes(value: &Value) -> Result<Vec<u8>, String> {
    serde_json::to_vec_pretty(&video_public_evidence_value(value))
        .map_err(|error| error.to_string())
}

fn video_public_evidence_array(items: &[Value]) -> Vec<Value> {
    items.iter().map(video_public_evidence_value).collect()
}

fn video_public_evidence_value(value: &Value) -> Value {
    match value {
        Value::Array(items) => json!(items
            .iter()
            .map(video_public_evidence_value)
            .collect::<Vec<_>>()),
        Value::Object(object) => {
            let sanitized = object
                .iter()
                .map(|(key, value)| (key.clone(), video_public_evidence_field(key, value)))
                .collect::<serde_json::Map<_, _>>();
            Value::Object(sanitized)
        }
        Value::String(value) => json!(video_safe_evidence_text(value)),
        _ => value.clone(),
    }
}

fn video_public_evidence_field(key: &str, value: &Value) -> Value {
    let lower_key = key.to_ascii_lowercase();
    let key_requires_redaction = lower_key.contains("path")
        || lower_key.contains("dir")
        || lower_key.contains("url")
        || lower_key.contains("cookie")
        || lower_key.contains("authorization")
        || lower_key.contains("provider_key")
        || lower_key == "uri"
        || lower_key.ends_with("_uri")
        || lower_key.contains("token")
        || lower_key.contains("secret")
        || lower_key == "object_key";
    match value {
        Value::String(text) if key_requires_redaction => {
            if lower_key.contains("file_name") || lower_key == "filename" {
                json!(video_safe_evidence_text(text))
            } else {
                json!("[redacted]")
            }
        }
        _ => video_public_evidence_value(value),
    }
}

fn video_public_extraction_artifacts_manifest(manifest: &Value) -> Value {
    let mut public_manifest = manifest.clone();
    let Some(object) = public_manifest.as_object_mut() else {
        return public_manifest;
    };

    for key in ["session_dir", "artifacts_dir"] {
        if object
            .get(key)
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty())
        {
            object.insert(key.to_string(), json!("[redacted]"));
            object.insert(format!("{key}_redacted"), json!(true));
        }
    }
    if let Some(manifest_path) = object.get("manifest_path").and_then(Value::as_str) {
        let manifest_file_name = Path::new(manifest_path)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or(DEFAULT_EXTRACTION_ARTIFACTS_MANIFEST_FILE_NAME)
            .to_string();
        object.insert("manifest_path".to_string(), json!("[redacted]"));
        object.insert("manifest_file_name".to_string(), json!(manifest_file_name));
        object.insert("manifest_path_redacted".to_string(), json!(true));
    }
    let public_files = object.get("files").and_then(Value::as_array).map(|files| {
        files
            .iter()
            .map(video_public_artifact_file)
            .collect::<Vec<_>>()
    });
    if let Some(public_files) = public_files {
        object.insert("files".to_string(), json!(public_files));
    }

    public_manifest
}

fn video_public_artifact_files_by_kinds(files: &[Value], kinds: &[&str]) -> Vec<Value> {
    files
        .iter()
        .filter(|file| {
            file.get("artifact_kind")
                .and_then(Value::as_str)
                .is_some_and(|kind| kinds.contains(&kind))
        })
        .map(video_public_artifact_file)
        .collect()
}

fn video_public_artifact_file(file: &Value) -> Value {
    let mut public_file = video_public_evidence_value(file);
    let Some(object) = public_file.as_object_mut() else {
        return public_file;
    };
    let file_name = file
        .get("path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .and_then(|path| Path::new(path).file_name().and_then(|value| value.to_str()))
        .or_else(|| file.get("file_name").and_then(Value::as_str))
        .unwrap_or("artifact");
    object.insert("file_name".to_string(), json!(file_name));
    if file
        .get("path")
        .and_then(Value::as_str)
        .is_some_and(|path| !path.trim().is_empty())
    {
        object.insert("path".to_string(), json!("[redacted]"));
        object.insert("path_redacted".to_string(), json!(true));
    }
    if file
        .get("uri")
        .and_then(Value::as_str)
        .is_some_and(|uri| !uri.trim().is_empty())
    {
        object.insert("uri".to_string(), json!("[redacted]"));
        object.insert("uri_redacted".to_string(), json!(true));
    }
    public_file
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
        "final_pptx_ready" => {
            "review the immutable published deliverable manifest and expose the final PPTX"
        }
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
    provider_evidence: Vec<Value>,
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
        evidence
            .provider_evidence
            .extend(cloned_array_items(media, "provider_evidence"));
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
    output.push_str("## Evidence References\n\n");
    output.push_str(&render_video_evidence_reference_lines(evidence));
    output.push('\n');

    output.push_str("## Quality Notes\n\n");
    output.push_str(&render_video_quality_note_lines(evidence, frame_extraction));
    output.push('\n');

    output.push_str("## Slide / Scene Candidates\n\n");
    if evidence.scenes.is_empty() && evidence.keyframe_ocr_snippets.is_empty() {
        output.push_str("- No scene or OCR candidates yet. Use raw frames/contact sheet before final PPTX generation.\n\n");
    } else {
        for (index, scene) in evidence.scenes.iter().enumerate() {
            let range = video_time_range_label(scene);
            let summary = video_item_text(scene, &["summary", "text", "description"])
                .unwrap_or_else(|| "Untitled scene".to_string());
            let reference = video_evidence_reference_label(scene, "scene", index + 1);
            output.push_str(&format!(
                "- Scene {}{}: {} ({})\n",
                index + 1,
                optional_time_suffix(&range),
                summary,
                reference
            ));
        }
        for (index, snippet) in evidence.keyframe_ocr_snippets.iter().enumerate() {
            let timestamp = video_timestamp_label(snippet);
            let text = video_item_text(snippet, &["text", "ocr_text", "summary"])
                .unwrap_or_else(|| "No OCR text".to_string());
            let reference = video_evidence_reference_label(snippet, "ocr", index + 1);
            output.push_str(&format!(
                "- OCR {}{}: {} ({})\n",
                index + 1,
                optional_time_suffix(&timestamp),
                text,
                reference
            ));
        }
        output.push('\n');
    }

    output.push_str("## Transcript\n\n");
    if evidence.transcript_segments.is_empty() {
        output.push_str("No transcript segments yet.\n");
    } else {
        for (index, segment) in evidence.transcript_segments.iter().enumerate() {
            let text =
                video_item_text(segment, &["text", "content", "summary"]).unwrap_or_default();
            let range = video_time_range_label(segment);
            let reference = video_evidence_reference_label(segment, "transcript", index + 1);
            output.push_str(&format!(
                "- {}{} ({})\n",
                optional_time_prefix(&range),
                text,
                reference
            ));
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
    output.push_str("This file contains only evidence extracted or observed by DataMax. Missing evidence is left explicit rather than fabricated.\n\n");
    output.push_str("## Counts\n\n");
    output.push_str(&format!(
        "- Transcript segments: {}\n- Scenes: {}\n- Keyframe OCR snippets: {}\n- Raw frames: {}\n\n",
        evidence.transcript_segments.len(),
        evidence.scenes.len(),
        evidence.keyframe_ocr_snippets.len(),
        frame_count
    ));
    output.push_str("## Evidence References\n\n");
    output.push_str(&render_video_evidence_reference_lines(evidence));
    output.push('\n');

    output.push_str("## Quality Notes\n\n");
    output.push_str(&render_video_quality_note_lines(evidence, frame_extraction));
    output.push('\n');

    output.push_str("## Provider Evidence\n\n");
    output.push_str(&render_video_provider_evidence_lines(evidence));
    output.push('\n');

    output.push_str("## Transcript Evidence\n\n");
    if evidence.transcript_segments.is_empty() {
        output.push_str("- Missing transcript evidence.\n\n");
    } else {
        for (index, segment) in evidence.transcript_segments.iter().enumerate() {
            let text =
                video_item_text(segment, &["text", "content", "summary"]).unwrap_or_default();
            let range = video_time_range_label(segment);
            let reference = video_evidence_reference_label(segment, "transcript", index + 1);
            output.push_str(&format!(
                "- {}{} ({})\n",
                optional_time_prefix(&range),
                text,
                reference
            ));
        }
        output.push('\n');
    }

    output.push_str("## Scene Evidence\n\n");
    if evidence.scenes.is_empty() {
        output.push_str("- Missing scene evidence.\n\n");
    } else {
        for (index, scene) in evidence.scenes.iter().enumerate() {
            let summary = video_item_text(scene, &["summary", "text", "description"])
                .unwrap_or_else(|| "Untitled scene".to_string());
            let range = video_time_range_label(scene);
            let reference = video_evidence_reference_label(scene, "scene", index + 1);
            output.push_str(&format!(
                "- {}{} ({})\n",
                optional_time_prefix(&range),
                summary,
                reference
            ));
        }
        output.push('\n');
    }

    output.push_str("## Keyframe OCR Evidence\n\n");
    if evidence.keyframe_ocr_snippets.is_empty() {
        output.push_str("- Missing keyframe OCR evidence.\n\n");
    } else {
        for (index, snippet) in evidence.keyframe_ocr_snippets.iter().enumerate() {
            let text = video_item_text(snippet, &["text", "ocr_text", "summary"])
                .unwrap_or_else(|| "No OCR text".to_string());
            let timestamp = video_timestamp_label(snippet);
            let reference = video_evidence_reference_label(snippet, "ocr", index + 1);
            output.push_str(&format!(
                "- {}{} ({})\n",
                optional_time_prefix(&timestamp),
                text,
                reference
            ));
        }
        output.push('\n');
    }

    output.push_str("## Raw Frame Evidence\n\n");
    if frame_count == 0 {
        output.push_str("- Missing raw frame evidence.\n");
    } else {
        output.push_str(&format!("- Raw frames captured: {frame_count}\n"));
        if frame_extraction
            .get("raw_frames_dir")
            .and_then(Value::as_str)
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
        {
            output.push_str("- Raw frames directory: [redacted]\n");
        }
        if let Some(manifest_path) = frame_extraction
            .get("manifest_path")
            .and_then(Value::as_str)
        {
            let manifest_file_name = Path::new(manifest_path)
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or(DEFAULT_FRAME_MANIFEST_FILE_NAME);
            output.push_str(&format!(
                "- Frame manifest: {manifest_file_name} (path redacted)\n"
            ));
        }
    }

    output
}

fn render_video_quality_note_lines(
    evidence: &VideoMediaEvidenceItems,
    frame_extraction: &Value,
) -> String {
    let mut lines = Vec::new();
    if evidence.transcript_segments.is_empty() {
        lines.push("- Missing transcript evidence; source text and speaker notes cannot be treated as final narration.".to_string());
    }
    if evidence.scenes.is_empty() {
        lines.push("- Missing scene evidence; slide grouping should stay review-only.".to_string());
    }
    if evidence.keyframe_ocr_snippets.is_empty() {
        lines.push("- Missing keyframe OCR evidence; slide text should be verified from frames or source files.".to_string());
    }

    for warning in video_low_confidence_evidence_warnings(evidence) {
        let code = warning
            .get("code")
            .and_then(Value::as_str)
            .unwrap_or("low_confidence_evidence");
        let count = warning.get("count").and_then(Value::as_u64).unwrap_or(0);
        lines.push(format!(
            "- {code}: {count} low-confidence evidence item(s) require review."
        ));
    }

    if let Some(warning) = video_provider_failure_warning(evidence) {
        let count = warning
            .get("failed_provider_count")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        lines.push(format!(
            "- provider_failure: {count} provider capability check(s) failed or are unverified."
        ));
        for provider in warning
            .get("providers")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .take(3)
        {
            let provider_name = provider
                .get("provider")
                .and_then(Value::as_str)
                .map(video_safe_evidence_text)
                .unwrap_or_else(|| "unknown".to_string());
            let capability = provider
                .get("capability")
                .and_then(Value::as_str)
                .map(video_safe_evidence_text)
                .unwrap_or_else(|| "unknown".to_string());
            let status = provider
                .get("status")
                .and_then(Value::as_str)
                .map(video_safe_evidence_text)
                .unwrap_or_else(|| "unknown".to_string());
            lines.push(format!(
                "  - provider={provider_name}; capability={capability}; status={status}"
            ));
        }
    }

    let frame_status = frame_extraction
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if matches!(frame_status, "failed" | "skipped") {
        let reason = frame_extraction
            .get("reason")
            .and_then(Value::as_str)
            .map(video_safe_evidence_text)
            .unwrap_or_else(|| "unknown".to_string());
        lines.push(format!(
            "- frame_extraction_{frame_status}: raw frames/contact sheet are not complete; reason={reason}."
        ));
    }

    if lines.is_empty() {
        "- No blocking quality notes recorded for the current evidence set.\n".to_string()
    } else {
        format!("{}\n", lines.join("\n"))
    }
}

fn render_video_evidence_reference_lines(evidence: &VideoMediaEvidenceItems) -> String {
    let mut output = String::new();
    if evidence.transcript_segments.is_empty()
        && evidence.scenes.is_empty()
        && evidence.keyframe_ocr_snippets.is_empty()
    {
        output.push_str("- No evidence references available yet.\n");
        return output;
    }

    for (index, segment) in evidence.transcript_segments.iter().enumerate().take(12) {
        output.push_str(&format!(
            "- Transcript {}: {}\n",
            index + 1,
            video_evidence_reference_label(segment, "transcript", index + 1)
        ));
    }
    for (index, scene) in evidence.scenes.iter().enumerate().take(12) {
        output.push_str(&format!(
            "- Scene {}: {}\n",
            index + 1,
            video_evidence_reference_label(scene, "scene", index + 1)
        ));
    }
    for (index, snippet) in evidence.keyframe_ocr_snippets.iter().enumerate().take(12) {
        output.push_str(&format!(
            "- OCR {}: {}\n",
            index + 1,
            video_evidence_reference_label(snippet, "ocr", index + 1)
        ));
    }
    output
}

fn render_video_provider_evidence_lines(evidence: &VideoMediaEvidenceItems) -> String {
    if evidence.provider_evidence.is_empty() {
        return "- No provider evidence recorded.\n".to_string();
    }

    let mut output = String::new();
    for provider in evidence.provider_evidence.iter().take(12) {
        let provider_name = video_safe_evidence_text(
            &video_item_text(provider, &["provider"]).unwrap_or_else(|| "unknown".to_string()),
        );
        let capability = video_safe_evidence_text(
            &video_item_text(provider, &["capability"]).unwrap_or_else(|| "unknown".to_string()),
        );
        let status = video_safe_evidence_text(
            &video_item_text(provider, &["status"]).unwrap_or_else(|| "unknown".to_string()),
        );
        let supported = provider
            .get("supported")
            .and_then(Value::as_bool)
            .map(|value| if value { "supported" } else { "not_supported" })
            .unwrap_or("unknown_support");
        output.push_str(&format!(
            "- {provider_name} / {capability}: {status}; {supported}\n"
        ));
    }
    output
}

fn video_evidence_reference_label(value: &Value, kind: &str, index: usize) -> String {
    let mut parts = vec![format!("ref={kind}#{index}")];
    let time = if kind == "ocr" {
        video_timestamp_label(value)
    } else {
        video_time_range_label(value)
    };
    if !time.is_empty() {
        parts.push(format!("time={time}"));
    }
    if let Some(source) = video_item_text(value, &["source", "provider", "origin"]) {
        parts.push(format!("source={}", video_safe_evidence_text(&source)));
    }
    if let Some(locator) = video_item_text(
        value,
        &[
            "evidence_ref",
            "evidenceRef",
            "source_ref",
            "sourceRef",
            "source_locator",
            "sourceLocator",
            "chunk_id",
            "chunkId",
        ],
    ) {
        parts.push(format!("locator={}", video_safe_evidence_text(&locator)));
    }
    if let Some(confidence) = video_number_field(value, &["confidence", "ocr_confidence"]) {
        parts.push(format!("confidence={confidence:.2}"));
    }
    parts.join("; ")
}

fn video_safe_evidence_text(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "unknown".to_string();
    }
    let lower = trimmed.to_ascii_lowercase();
    let looks_secret = lower.contains("token")
        || lower.contains("cookie")
        || lower.contains("authorization")
        || lower.contains("bearer ")
        || lower.contains("provider_key")
        || lower.contains("secret");
    let looks_url = lower.starts_with("http://") || lower.starts_with("https://");
    let looks_path = trimmed.contains("\\") || trimmed.contains(":/") || trimmed.starts_with('/');
    if looks_secret || looks_url || looks_path {
        "[redacted]".to_string()
    } else {
        trimmed.chars().take(120).collect()
    }
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

    fn write_test_slide_rectangle_png(path: &Path) {
        let mut image = image::RgbImage::from_pixel(100, 80, image::Rgb([8, 8, 8]));
        for y in 10..60 {
            for x in 20..80 {
                image.put_pixel(x, y, image::Rgb([240, 240, 240]));
            }
        }
        image.save(path).expect("test slide rectangle png");
    }

    fn write_test_noisy_top_left_slide_rectangle_png(path: &Path) {
        let mut image = image::RgbImage::from_pixel(100, 80, image::Rgb([8, 8, 8]));
        for y in 0..6 {
            for x in 0..6 {
                image.put_pixel(x, y, image::Rgb([160, 20, 20]));
            }
        }
        for y in 10..60 {
            for x in 20..80 {
                image.put_pixel(x, y, image::Rgb([240, 240, 240]));
            }
        }
        image.save(path).expect("test noisy slide rectangle png");
    }

    fn write_test_slide_with_external_foreground_png(path: &Path) {
        let mut image = image::RgbImage::from_pixel(100, 80, image::Rgb([8, 8, 8]));
        for y in 10..60 {
            for x in 20..80 {
                image.put_pixel(x, y, image::Rgb([240, 240, 240]));
            }
        }
        for y in 5..75 {
            for x in 90..96 {
                image.put_pixel(x, y, image::Rgb([230, 230, 230]));
            }
        }
        image.save(path).expect("test external foreground png");
    }

    fn write_test_gradient_edge_slide_rectangle_png(path: &Path) {
        let mut image = image::RgbImage::new(100, 80);
        for y in 0..80 {
            for x in 0..100 {
                let tone = 70 + ((x + y) % 120) as u8;
                image.put_pixel(x, y, image::Rgb([tone, tone, tone]));
            }
        }
        for y in 10..60 {
            for x in 20..80 {
                image.put_pixel(x, y, image::Rgb([244, 244, 244]));
            }
        }
        for y in 10..60 {
            image.put_pixel(20, y, image::Rgb([12, 12, 12]));
            image.put_pixel(79, y, image::Rgb([12, 12, 12]));
        }
        for x in 20..80 {
            image.put_pixel(x, 10, image::Rgb([12, 12, 12]));
            image.put_pixel(x, 59, image::Rgb([12, 12, 12]));
        }
        image
            .save(path)
            .expect("test gradient edge slide rectangle png");
    }

    fn write_test_low_contrast_bright_canvas_slide_png(path: &Path) {
        let mut image = image::RgbImage::new(120, 90);
        for y in 0..90 {
            for x in 0..120 {
                let tone = 205 + ((x * 17 + y * 31) % 6) as u8;
                image.put_pixel(x, y, image::Rgb([tone, tone, tone]));
            }
        }
        for y in 18..72 {
            for x in 26..104 {
                image.put_pixel(x, y, image::Rgb([244, 244, 244]));
            }
        }
        for x in 34..94 {
            image.put_pixel(x, 30, image::Rgb([30, 60, 140]));
            image.put_pixel(x, 50, image::Rgb([40, 40, 40]));
        }
        for y in 38..64 {
            image.put_pixel(44, y, image::Rgb([210, 80, 80]));
            image.put_pixel(82, y, image::Rgb([70, 120, 210]));
        }
        image
            .save(path)
            .expect("test low contrast bright canvas png");
    }

    fn write_test_visual_slide_png(
        path: &Path,
        background: [u8; 3],
        foreground: [u8; 3],
        rect_x: std::ops::Range<u32>,
        rect_y: std::ops::Range<u32>,
    ) {
        let mut image = image::RgbImage::from_pixel(100, 80, image::Rgb(background));
        for y in rect_y {
            for x in rect_x.clone() {
                image.put_pixel(x, y, image::Rgb(foreground));
            }
        }
        image.save(path).expect("test visual slide png");
    }

    fn write_test_sparse_text_build_slide_png(path: &Path, build_step: u8) {
        let mut image = image::RgbImage::from_pixel(160, 90, image::Rgb([14, 18, 17]));
        let lines = [
            (19_u32, 30_u32, 132_u32),
            (30_u32, 42_u32, 118_u32),
            (47_u32, 24_u32, 136_u32),
            (61_u32, 30_u32, 124_u32),
        ];
        let visible_lines = match build_step {
            0 => 2,
            1 => 3,
            _ => 4,
        };
        for (index, (y, start_x, end_x)) in lines.iter().enumerate() {
            if index >= visible_lines {
                continue;
            }
            for dy in 0..2 {
                for x in *start_x..*end_x {
                    image.put_pixel(x, y + dy, image::Rgb([235, 238, 236]));
                }
            }
        }
        for y in 72..76 {
            for x in 10..24 {
                image.put_pixel(x, y, image::Rgb([240, 118, 78]));
            }
        }
        image.save(path).expect("test sparse text build slide png");
    }

    fn write_test_shape_duplicate_slide_png(path: &Path, foreground: [u8; 3], variant: u8) {
        let mut image = image::RgbImage::from_pixel(160, 90, image::Rgb([14, 18, 17]));
        let lines = if variant == 0 {
            [(19_u32, 28_u32, 132_u32), (32_u32, 42_u32, 118_u32)]
        } else {
            [(46_u32, 20_u32, 140_u32), (60_u32, 36_u32, 126_u32)]
        };
        for (y, start_x, end_x) in lines {
            for dy in 0..2 {
                for x in start_x..end_x {
                    image.put_pixel(x, y + dy, image::Rgb(foreground));
                }
            }
        }
        for y in 72..76 {
            for x in 10..24 {
                image.put_pixel(x, y, image::Rgb([240, 118, 78]));
            }
        }
        image.save(path).expect("test shape duplicate slide png");
    }

    fn write_test_bright_template_build_slide_png(path: &Path, bullet_count: u8) {
        let mut image = image::RgbImage::from_pixel(160, 90, image::Rgb([246, 246, 244]));
        for y in 0..26 {
            for x in 0..160 {
                image.put_pixel(x, y, image::Rgb([18, 96, 190]));
            }
        }
        for y in 8..15 {
            for x in 18..86 {
                image.put_pixel(x, y, image::Rgb([248, 248, 248]));
            }
        }
        for bullet_index in 0..bullet_count {
            let top = 38 + u32::from(bullet_index) * 14;
            for y in top..top + 7 {
                for x in 30..126 {
                    image.put_pixel(x, y, image::Rgb([84, 84, 84]));
                }
            }
        }
        image
            .save(path)
            .expect("test bright template build slide png");
    }

    fn write_test_solid_frame_png(path: &Path, tone: [u8; 3]) {
        let image = image::RgbImage::from_pixel(100, 80, image::Rgb(tone));
        image.save(path).expect("test solid frame png");
    }

    fn write_test_sharp_text_slide_png(path: &Path) {
        let mut image = image::RgbImage::from_pixel(160, 90, image::Rgb([245, 245, 245]));
        for y in 10..80 {
            for x in 16..144 {
                image.put_pixel(x, y, image::Rgb([252, 252, 252]));
            }
        }
        for x in 28..132 {
            image.put_pixel(x, 22, image::Rgb([20, 20, 20]));
            image.put_pixel(x, 23, image::Rgb([20, 20, 20]));
            image.put_pixel(x, 52, image::Rgb([30, 30, 30]));
        }
        for y in 34..70 {
            image.put_pixel(36, y, image::Rgb([25, 25, 25]));
            image.put_pixel(76, y, image::Rgb([25, 25, 25]));
            image.put_pixel(118, y, image::Rgb([25, 25, 25]));
        }
        for y in 38..66 {
            for x in [48, 52, 56, 90, 94, 98, 102] {
                image.put_pixel(x, y, image::Rgb([15, 15, 15]));
            }
        }
        image.save(path).expect("test sharp text slide png");
    }

    fn write_test_low_contrast_text_slide_png(path: &Path) {
        let mut image = image::RgbImage::from_pixel(160, 90, image::Rgb([236, 236, 236]));
        for x in 28..132 {
            image.put_pixel(x, 22, image::Rgb([228, 228, 228]));
            image.put_pixel(x, 23, image::Rgb([228, 228, 228]));
            image.put_pixel(x, 52, image::Rgb([229, 229, 229]));
        }
        for y in 34..70 {
            image.put_pixel(36, y, image::Rgb([230, 230, 230]));
            image.put_pixel(76, y, image::Rgb([230, 230, 230]));
            image.put_pixel(118, y, image::Rgb([230, 230, 230]));
        }
        for y in 38..66 {
            for x in [48, 52, 56, 90, 94, 98, 102] {
                image.put_pixel(x, y, image::Rgb([228, 228, 228]));
            }
        }
        image.save(path).expect("test low contrast text slide png");
    }

    fn assert_public_manifest_file_entry(files: &[Value], kind: &str, file_name: &str) {
        let file = files
            .iter()
            .find(|file| file["artifact_kind"] == json!(kind))
            .unwrap_or_else(|| panic!("missing public manifest file entry {}", kind));

        assert_eq!(file["file_name"], json!(file_name));
        assert_eq!(file["path"], json!("[redacted]"));
        assert_eq!(file["path_redacted"], json!(true));
        if file.get("uri").is_some() {
            assert_eq!(file["uri"], json!("[redacted]"));
            assert_eq!(file["uri_redacted"], json!(true));
        }
    }

    #[test]
    fn measures_slide_frame_sharpness_for_quality_report() {
        let output_root = std::env::temp_dir().join(format!(
            "aidp-v3-video-sharpness-test-{}",
            DocumentId::new()
        ));
        fs::create_dir_all(&output_root).expect("sharpness test dir");
        let sharp_path = output_root.join("sharp-slide.png");
        let solid_path = output_root.join("solid-slide.png");
        let missing_path = output_root.join("missing-slide.png");
        write_test_sharp_text_slide_png(&sharp_path);
        write_test_solid_frame_png(&solid_path, [244, 244, 244]);

        let sharp = video_slide_sharpness_assessment(&sharp_path);
        assert_eq!(sharp.status, "measured");
        assert_eq!(sharp.risk, "low");
        assert!(sharp.score.unwrap_or_default() >= VIDEO_SLIDE_SHARPNESS_LOW_RISK_MIN_SCORE);

        let solid = video_slide_sharpness_assessment(&solid_path);
        assert_eq!(solid.status, "measured");
        assert_eq!(solid.risk, "high");
        assert!(solid.score.unwrap_or(100) < VIDEO_SLIDE_SHARPNESS_MEDIUM_RISK_MIN_SCORE);

        let missing = video_slide_sharpness_assessment(&missing_path);
        assert_eq!(missing.status, "unavailable");
        assert_eq!(missing.risk, "unknown");
        assert_eq!(missing.score, None);
    }

    #[test]
    fn flags_low_contrast_slide_text_in_quality_report() {
        let document = test_document();
        let output_root = std::env::temp_dir().join(format!(
            "aidp-v3-video-low-contrast-quality-test-{}",
            DocumentId::new()
        ));
        let session_dir = output_root.join(format!("video-extraction-{}", document.id));
        let artifacts_dir = session_dir.join(DEFAULT_GENERATED_ARTIFACTS_DIR_NAME);
        let raw_frames_dir = session_dir.join(DEFAULT_RAW_FRAMES_DIR_NAME);
        fs::create_dir_all(&raw_frames_dir).expect("raw frames dir");
        fs::create_dir_all(&artifacts_dir).expect("artifacts dir");
        write_test_low_contrast_text_slide_png(&raw_frames_dir.join("frame_000001.png"));
        fs::write(
            artifacts_dir.join(DEFAULT_PPT_KEEP_LIST_TEMPLATE_FILE_NAME),
            serde_json::to_vec_pretty(&json!({
                "status": "selected",
                "selected_candidate_indices": [1]
            }))
            .expect("keep list bytes"),
        )
        .expect("keep list");
        let frame_extraction = json!({
            "status": "completed",
            "raw_frames_dir": raw_frames_dir.display().to_string(),
            "frame_count": 1,
            "manifest_file_name": DEFAULT_FRAME_MANIFEST_FILE_NAME
        });

        let manifest =
            write_video_extraction_text_artifacts(&document, &[], &frame_extraction, &output_root)
                .expect("quality report artifacts");
        let files = manifest["files"].as_array().expect("files");
        let slide_quality_report_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("slide_quality_report"))
            .and_then(|file| file["path"].as_str())
            .expect("slide quality report path");
        let slide_quality_report =
            fs::read_to_string(slide_quality_report_path).expect("slide quality report");
        assert!(!slide_quality_report.contains(&raw_frames_dir.display().to_string()));
        let slide_quality_report_json: Value =
            serde_json::from_str(&slide_quality_report).expect("quality report json");

        assert_eq!(slide_quality_report_json["slide_count"], json!(1));
        assert_eq!(
            slide_quality_report_json["summary"]["sharpness_high_count"],
            json!(1)
        );
        assert_eq!(
            slide_quality_report_json["summary"]["sharpness_unknown_count"],
            json!(0)
        );
        assert_eq!(
            slide_quality_report_json["slides"][0]["sharpness_status"],
            json!("measured")
        );
        assert_eq!(
            slide_quality_report_json["slides"][0]["sharpness_risk"],
            json!("high")
        );
        assert!(
            slide_quality_report_json["slides"][0]["sharpness_score"]
                .as_i64()
                .expect("sharpness score")
                < VIDEO_SLIDE_SHARPNESS_MEDIUM_RISK_MIN_SCORE
        );
        let sharpness_risk = slide_quality_report_json["risk_flags"]
            .as_array()
            .expect("risk flags")
            .iter()
            .find(|risk| risk["code"] == json!("frame_sharpness_review_required"))
            .expect("sharpness risk flag");
        assert_eq!(sharpness_risk["high_count"], json!(1));
        assert_eq!(sharpness_risk["unknown_count"], json!(0));
        assert_eq!(
            sharpness_risk["review_action"],
            json!("review_blurry_or_unmeasured_slide_frames")
        );
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
        let partial_warnings = partial["deliverable_status"]["warnings"]
            .as_array()
            .expect("partial warnings");
        assert!(partial_warnings.iter().any(|warning| {
            warning["code"] == json!("parse_partial")
                && warning["severity"] == json!("high")
                && warning["missing_evidence"]
                    .as_array()
                    .expect("missing")
                    .len()
                    == 3
        }));
        let partial_follow_up =
            video_extraction_completion_follow_up_from_output(&partial, &[]).expect("follow up");
        assert!(partial_follow_up["next_actions"]
            .as_array()
            .expect("next actions")
            .contains(&json!("rerun_or_refresh_video_parse")));

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
        let completed_warnings = completed["deliverable_status"]["warnings"]
            .as_array()
            .expect("completed warnings");
        assert!(completed_warnings.iter().any(|warning| {
            if warning["code"] != json!("parse_partial") || warning["severity"] != json!("medium") {
                return false;
            }
            let missing = warning["missing_evidence"].as_array().expect("missing");
            missing.len() == 1 && missing[0] == json!("scene_windows")
        }));
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
    fn extract_output_surfaces_skipped_frame_extraction_warning() {
        let document = test_document();
        let frame_extraction = json!({
            "status": "skipped",
            "source": "ffmpeg_external_process",
            "reason": "local_media_path_not_available"
        });

        let output =
            extract_video_ppt_output_with_frame_extraction(&document, &[], frame_extraction);

        assert_eq!(
            output["deliverable_status"]["frame_extraction_status"],
            json!("skipped")
        );
        assert_eq!(
            output["deliverable_status"]["frame_extraction_reason"],
            json!("local_media_path_not_available")
        );
        let warnings = output["deliverable_status"]["warnings"]
            .as_array()
            .expect("warnings");
        assert!(warnings.iter().any(|warning| {
            warning["code"] == json!("frame_extraction_skipped")
                && warning["severity"] == json!("medium")
                && warning["reason"] == json!("local_media_path_not_available")
        }));
    }

    #[test]
    fn extract_output_surfaces_generated_artifact_writer_failure() {
        let document = test_document();
        let generated_artifacts = json!({
            "status": "failed",
            "source": "media_worker_text_artifact_writer",
            "reason": "disk full",
            "files": []
        });

        let output = extract_video_ppt_output_with_artifacts(
            &document,
            &[],
            video_frame_extraction_plan(&document),
            generated_artifacts,
        );

        assert_eq!(
            output["deliverable_status"]["generated_artifacts_status"],
            json!("failed")
        );
        let warnings = output["deliverable_status"]["warnings"]
            .as_array()
            .expect("warnings");
        assert!(warnings.iter().any(|warning| {
            warning["code"] == json!("generated_artifacts_failed")
                && warning["severity"] == json!("high")
                && warning["reason"] == json!("disk full")
        }));
    }

    #[test]
    fn extract_output_surfaces_low_confidence_evidence_warnings() {
        let document = test_document();
        let chunk = test_chunk(json!({
            "media": {
                "transcript_segments": [{
                    "text": "page one",
                    "confidence": 0.42
                }],
                "scenes": [],
                "keyframe_ocr_snippets": [{
                    "text": "slide title",
                    "ocr_confidence": 0.5
                }]
            }
        }));

        let output = extract_video_ppt_output_with_frame_extraction(
            &document,
            &[chunk],
            video_frame_extraction_plan(&document),
        );

        let warnings = output["deliverable_status"]["warnings"]
            .as_array()
            .expect("warnings");
        assert!(warnings.iter().any(|warning| {
            warning["code"] == json!("low_confidence_transcript")
                && warning["severity"] == json!("medium")
                && warning["count"] == json!(1)
        }));
        assert!(warnings.iter().any(|warning| {
            warning["code"] == json!("subtitle_ocr_low_confidence")
                && warning["severity"] == json!("medium")
                && warning["count"] == json!(1)
        }));

        let follow_up =
            video_extraction_completion_follow_up_from_output(&output, &[]).expect("follow up");
        let next_actions = follow_up["next_actions"].as_array().expect("next actions");
        assert!(next_actions.contains(&json!("review_low_confidence_transcript")));
        assert!(next_actions.contains(&json!("rerun_or_review_subtitle_ocr")));
    }

    #[test]
    fn extract_output_surfaces_provider_failure_warning() {
        let document = test_document();
        let chunk = test_chunk(json!({
            "media": {
                "transcript_segments": [{"text": "page one"}],
                "scenes": [{"summary": "title page"}],
                "keyframe_ocr_snippets": [{"text": "slide title"}],
                "provider_evidence": [{
                    "provider": "minimax",
                    "capability": "native_video_understanding",
                    "status": "configured_unverified",
                    "supported": false,
                    "detail": "probe missing",
                    "endpoint": "/video",
                    "model": "MiniMax-M2.5-highspeed"
                }]
            }
        }));

        let output = extract_video_ppt_output_with_frame_extraction(
            &document,
            &[chunk],
            video_frame_extraction_plan(&document),
        );

        let warnings = output["deliverable_status"]["warnings"]
            .as_array()
            .expect("warnings");
        assert!(warnings.iter().any(|warning| {
            warning["code"] == json!("provider_failure")
                && warning["severity"] == json!("high")
                && warning["failed_provider_count"] == json!(1)
                && warning["providers"][0]["provider"] == json!("minimax")
                && warning["providers"][0]["capability"] == json!("native_video_understanding")
        }));

        let follow_up =
            video_extraction_completion_follow_up_from_output(&output, &[]).expect("follow up");
        assert!(follow_up["next_actions"]
            .as_array()
            .expect("next actions")
            .contains(&json!("check_video_provider_configuration")));
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
    fn extract_output_records_redacted_video_source_summary() {
        let mut document = test_document();
        document.object_key =
            "https://private.example.test/course/lesson.mp4?token=secret-token".to_string();
        document.metadata = BTreeMap::from_iter([(
            "remote_media".to_string(),
            json!({
                "source_url": "https://private.example.test/course/lesson.mp4?token=secret-token",
                "source_page_url": "https://private.example.test/lesson?cookie=secret-cookie",
                "source_type": "public_page_resolvable_video",
                "asset_state": "remote_registered",
                "ingest_requires_env": "INGEST_REMOTE_MEDIA_ENABLED"
            }),
        )]);

        let output = extract_video_ppt_output_with_artifacts(
            &document,
            &[],
            video_frame_extraction_plan(&document),
            json!({"status": "planned", "files": []}),
        );
        let serialized = serde_json::to_string(&output).expect("output serializes");

        assert_eq!(
            output["source_summary"]["source_type"],
            json!("public_page_resolvable_video")
        );
        assert_eq!(
            output["source_summary"]["asset_state"],
            json!("remote_registered")
        );
        assert_eq!(output["source_summary"]["source_url_present"], json!(true));
        assert_eq!(output["source_summary"]["source_url_redacted"], json!(true));
        assert_eq!(
            output["source_summary"]["source_page_url_present"],
            json!(true)
        );
        assert_eq!(
            output["source_summary"]["ingest_requires_env"],
            json!("INGEST_REMOTE_MEDIA_ENABLED")
        );
        assert!(!serialized.contains("private.example.test"));
        assert!(!serialized.contains("secret-token"));
        assert!(!serialized.contains("secret-cookie"));
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
        assert_eq!(
            artifact["payload"]["completion_audit"]["kind"],
            json!("video_extraction_completion_audit")
        );
        assert_eq!(
            artifact["payload"]["deliverable_package"]["kind"],
            json!("video_extraction_deliverable_package")
        );
        assert_eq!(
            artifact["payload"]["deliverable_package"]["lifecycle_state"],
            json!("not_ready")
        );
        assert_eq!(
            artifact["payload"]["deliverable_package"]["publishable"],
            json!(false)
        );
        assert!(
            artifact["payload"]["deliverable_package"]["missing_required_file_kinds"]
                .as_array()
                .expect("missing required file kinds")
                .contains(&json!("pptx"))
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
                "artifact_kind": "published_deliverable_manifest",
                "artifact_id": format!("video-{}-published-deliverable", document.id),
                "title": "published deliverable manifest",
                "format": "application/json",
                "path": "generated_artifacts/published_deliverable_manifest.json",
                "uri": format!("artifact://video-{}-published-deliverable", document.id)
            }, {
                "artifact_kind": "published_version_history",
                "artifact_id": format!("video-{}-published-version-history", document.id),
                "title": "published version history",
                "format": "application/json",
                "path": "generated_artifacts/published_version_history.json",
                "uri": format!("artifact://video-{}-published-version-history", document.id)
            }, {
                "artifact_kind": "extraction_artifacts_manifest",
                "artifact_id": format!("video-{}-extraction-manifest", document.id),
                "title": "extraction artifacts manifest",
                "format": "application/json",
                "path": "generated_artifacts/extraction_artifacts_manifest.json",
                "uri": format!("artifact://video-{}-extraction-manifest", document.id)
            }, {
                "artifact_kind": "slide_rectangles_manifest",
                "artifact_id": format!("video-{}-slide-rectangles", document.id),
                "title": "slide rectangles manifest",
                "format": "application/json",
                "path": "generated_artifacts/slide_rectangles_manifest.json",
                "uri": format!("artifact://video-{}-slide-rectangles", document.id),
                "rectangle_extraction_status": "promoted_full_frame_fallback",
                "rectangle_extraction_mode": "full_frame_fallback",
                "promoted_rectangle_count": 1
            }, {
                "artifact_kind": "slide_notes",
                "artifact_id": format!("video-{}-slide-notes", document.id),
                "title": "slide notes",
                "format": "text/markdown",
                "path": "generated_artifacts/slide_notes.md",
                "uri": format!("artifact://video-{}-slide-notes", document.id)
            }, {
                "artifact_kind": "video_slides_markdown",
                "artifact_id": format!("video-{}-video-slides-markdown", document.id),
                "title": "video slides markdown",
                "format": "text/markdown",
                "path": "generated_artifacts/video_slides.md",
                "uri": format!("artifact://video-{}-video-slides-markdown", document.id)
            }, {
                "artifact_kind": "subtitle_page_map",
                "artifact_id": format!("video-{}-subtitle-page-map", document.id),
                "title": "subtitle page map",
                "format": "application/json",
                "path": "generated_artifacts/subtitle_page_map.json",
                "uri": format!("artifact://video-{}-subtitle-page-map", document.id)
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
            output_artifact["deliverable_status"]["has_published_deliverable_manifest"],
            json!(true)
        );
        assert_eq!(
            output_artifact["deliverable_status"]["has_published_version_history"],
            json!(true)
        );
        assert_eq!(
            output_artifact["deliverable_status"]["has_extraction_artifacts_manifest"],
            json!(true)
        );
        assert_eq!(
            output_artifact["deliverable_status"]["has_slide_rectangles_manifest"],
            json!(true)
        );
        assert_eq!(
            output_artifact["deliverable_status"]["has_slide_notes"],
            json!(true)
        );
        assert_eq!(
            output_artifact["deliverable_status"]["has_video_slides_markdown"],
            json!(true)
        );
        assert_eq!(
            output_artifact["deliverable_status"]["has_subtitle_page_map"],
            json!(true)
        );
        assert_eq!(
            output_artifact["final_deliverables_manifest"]["path"],
            json!("generated_artifacts/final_deliverables_manifest.json")
        );
        assert_eq!(
            output_artifact["published_deliverable_manifest"]["path"],
            json!("generated_artifacts/published_deliverable_manifest.json")
        );
        assert_eq!(
            output_artifact["published_version_history"]["path"],
            json!("generated_artifacts/published_version_history.json")
        );
        assert_eq!(
            output_artifact["completion_follow_up"]["kind"],
            json!("video_extraction_completion_follow_up")
        );
        assert_eq!(
            output_artifact["completion_follow_up"]["no_host_composed_answer"],
            json!(true)
        );
        assert_eq!(
            output_artifact["model_completion_turn_request"]["kind"],
            json!("video_extraction_model_completion_turn_request")
        );
        assert_eq!(
            output_artifact["model_completion_turn_request"]["turn_owner"],
            json!("model")
        );
        assert_eq!(
            output_artifact["model_completion_turn_request"]["answer_contract"]
                ["no_host_composed_answer"],
            json!(true)
        );
        assert_eq!(
            output_artifact["model_completion_turn_dispatch_request"]["kind"],
            json!("assistant_run_model_completion_turn_dispatch_request")
        );
        assert_eq!(
            output_artifact["model_completion_turn_dispatch_request"]["dispatch_target"],
            json!("continue_assistant_run")
        );
        assert_eq!(
            output_artifact["model_completion_turn_dispatch_request"]["continue_request"]
                ["max_steps"],
            json!(1)
        );
        assert_eq!(
            output_artifact["model_completion_turn_dispatch_request"]["idempotency_key"],
            json!(format!("video-completion-turn:{run_id}:{}:v1", document.id))
        );
        assert_eq!(
            output_artifact["model_completion_turn_dispatch_request"]["privacy_contract"]
                ["must_not_include_private_paths_or_urls"],
            json!(true)
        );
        assert_eq!(
            output_artifact["completion_audit"]["kind"],
            json!("video_extraction_completion_audit")
        );
        assert_eq!(
            html_artifact["payload"]["deliverable_package"]["lifecycle_state"],
            json!("published_version_ready")
        );
        assert_eq!(
            html_artifact["payload"]["deliverable_package"]["next_action"],
            json!("review_published_deliverable_manifest")
        );
        assert_eq!(
            output_artifact["deliverable_package"]["kind"],
            json!("video_extraction_deliverable_package")
        );
        assert_eq!(
            output_artifact["deliverable_package"]["publishable"],
            json!(true)
        );
        assert_eq!(
            output_artifact["deliverable_package"]["immutable_version"],
            json!(true)
        );
        assert_eq!(
            output_artifact["deliverable_package"]["published"],
            json!(true)
        );
        assert_eq!(
            output_artifact["deliverable_package"]["missing_required_file_kinds"],
            json!([])
        );
        assert_eq!(
            output_artifact["deliverable_package"]["required_file_count"],
            json!(7)
        );
        assert_eq!(
            output_artifact["deliverable_package"]["ready_required_file_count"],
            json!(7)
        );
        assert!(output_artifact["deliverable_package"]["required_files"]
            .as_array()
            .expect("required files")
            .iter()
            .any(|file| {
                file["artifact_kind"] == json!("pptx")
                    && file["file_name"] == json!("video_slides_screenshot_based.pptx")
            }));
        assert!(output_artifact["primary_files"]
            .as_array()
            .expect("primary files")
            .iter()
            .any(|file| file["artifact_kind"] == json!("final_deliverables_manifest")));
        assert!(output_artifact["manifest_outputs"]
            .as_array()
            .expect("manifest outputs")
            .iter()
            .any(|file| file["artifact_kind"] == json!("published_deliverable_manifest")));
        assert!(output_artifact["manifest_outputs"]
            .as_array()
            .expect("manifest outputs")
            .iter()
            .any(|file| file["artifact_kind"] == json!("published_version_history")));
        assert!(output_artifact["manifest_outputs"]
            .as_array()
            .expect("manifest outputs")
            .iter()
            .any(|file| file["artifact_kind"] == json!("extraction_artifacts_manifest")));
        assert!(output_artifact["final_outputs"]
            .as_array()
            .expect("final outputs")
            .iter()
            .any(|file| file["artifact_kind"] == json!("pptx")));
        assert!(output_artifact["final_outputs"]
            .as_array()
            .expect("final outputs")
            .iter()
            .any(|file| file["artifact_kind"] == json!("video_slides_markdown")));
        assert!(output_artifact["final_outputs"]
            .as_array()
            .expect("final outputs")
            .iter()
            .any(|file| file["artifact_kind"] == json!("video_slides_markdown")));
        assert!(output_artifact["review_outputs"]
            .as_array()
            .expect("review outputs")
            .iter()
            .any(|file| file["artifact_kind"] == json!("slide_rectangles_manifest")));
        assert!(output_artifact["review_outputs"]
            .as_array()
            .expect("review outputs")
            .iter()
            .any(|file| file["artifact_kind"] == json!("slide_notes")));
        assert!(output_artifact["evidence_outputs"]
            .as_array()
            .expect("evidence outputs")
            .iter()
            .any(|file| file["artifact_kind"] == json!("subtitle_page_map")));
        assert_eq!(output_artifact["html_artifact_ids"][0], html_artifact["id"]);
    }

    #[test]
    fn final_video_deliverables_do_not_require_subtitle_page_map_without_transcript_alignment() {
        let document = test_document();
        let run_id = "00000000-0000-0000-0000-000000000001";
        let files = [
            ("pptx", DEFAULT_VIDEO_SLIDES_PPTX_FILE_NAME),
            (
                "final_deliverables_manifest",
                DEFAULT_FINAL_DELIVERABLES_MANIFEST_FILE_NAME,
            ),
            (
                "published_deliverable_manifest",
                DEFAULT_PUBLISHED_DELIVERABLE_MANIFEST_FILE_NAME,
            ),
            (
                "published_version_history",
                DEFAULT_PUBLISHED_VERSION_HISTORY_FILE_NAME,
            ),
            (
                "extraction_artifacts_manifest",
                DEFAULT_EXTRACTION_ARTIFACTS_MANIFEST_FILE_NAME,
            ),
            (
                "slide_rectangles_manifest",
                DEFAULT_SLIDE_RECTANGLES_MANIFEST_FILE_NAME,
            ),
            ("slide_notes", DEFAULT_SLIDE_NOTES_ARTIFACT_FILE_NAME),
            (
                "video_slides_markdown",
                DEFAULT_VIDEO_SLIDES_MARKDOWN_FILE_NAME,
            ),
        ]
        .into_iter()
        .map(|(kind, file_name)| {
            json!({
                "artifact_kind": kind,
                "artifact_id": format!("video-{}-{kind}", document.id),
                "title": format!("artifact {kind}"),
                "format": "application/json",
                "path": format!("generated_artifacts/{file_name}"),
                "uri": format!("artifact://video-{}-{kind}", document.id),
            })
        })
        .collect::<Vec<_>>();
        let generated_artifacts = json!({
            "status": "completed",
            "files": files.clone(),
        });
        let deliverable_status = video_deliverable_status(&generated_artifacts);
        let output = json!({
            "status": "completed",
            "document_id": document.id.to_string(),
            "dataset_id": document.dataset_id.to_string(),
            "title": "No subtitle sample",
            "generated_artifacts": generated_artifacts,
            "deliverable_status": deliverable_status.clone(),
        });

        assert_eq!(deliverable_status["state"], json!("final_pptx_ready"));
        assert_eq!(deliverable_status["has_subtitle_page_map"], json!(false));
        let warning_codes = deliverable_warning_codes(&deliverable_status);
        assert!(warning_codes.contains("missing_transcript_alignment"));
        assert!(warning_codes.contains("speaker_notes_metadata_only"));

        let package = video_deliverable_package_summary(
            run_id,
            &document.id.to_string(),
            "No subtitle sample",
            &output,
            &files,
            &deliverable_status,
            &[],
        );
        assert_eq!(package["publishable"], json!(true));
        assert_eq!(package["missing_required_file_kinds"], json!([]));
        assert_eq!(package["required_file_count"], json!(6));
        assert!(!package["required_file_kinds"]
            .as_array()
            .expect("required file kinds")
            .contains(&json!("subtitle_page_map")));

        let published_manifest = video_published_deliverable_manifest(&document, &files, 2);
        assert_eq!(
            published_manifest["lifecycle_state"],
            json!("published_version_ready")
        );
        assert_eq!(published_manifest["missing_required_file_kinds"], json!([]));
        assert!(!published_manifest["required_file_kinds"]
            .as_array()
            .expect("required file kinds")
            .contains(&json!("subtitle_page_map")));
        assert!(!published_manifest["published_files"]
            .as_array()
            .expect("published files")
            .iter()
            .any(|file| file["artifact_kind"] == json!("subtitle_page_map")));

        let version_history = video_published_version_history_manifest(&document, &files, 2);
        assert_eq!(version_history["status"], json!("history_ready"));
        assert_eq!(version_history["versions"][0]["file_count"], json!(8));
        assert!(!version_history["versions"][0]["artifact_kinds"]
            .as_array()
            .expect("artifact kinds")
            .contains(&json!("subtitle_page_map")));

        let durable_manifest =
            video_extraction_durable_published_version_manifest(run_id, "workflow-1", &output)
                .expect("durable published manifest without subtitle map");
        assert_eq!(
            durable_manifest["lifecycle_state"],
            json!("published_version_ready")
        );
        assert!(!durable_manifest["required_file_kinds"]
            .as_array()
            .expect("required file kinds")
            .contains(&json!("subtitle_page_map")));
    }

    #[test]
    fn durable_published_version_manifest_redacts_artifact_paths() {
        let document = test_document();
        let run_id = "00000000-0000-0000-0000-000000000001";
        let files = VIDEO_PUBLISHED_DELIVERABLE_REQUIRED_KINDS
            .iter()
            .map(|kind| {
                let file_name = match *kind {
                    "pptx" => DEFAULT_VIDEO_SLIDES_PPTX_FILE_NAME,
                    "published_deliverable_manifest" => {
                        DEFAULT_PUBLISHED_DELIVERABLE_MANIFEST_FILE_NAME
                    }
                    "published_version_history" => DEFAULT_PUBLISHED_VERSION_HISTORY_FILE_NAME,
                    "final_deliverables_manifest" => DEFAULT_FINAL_DELIVERABLES_MANIFEST_FILE_NAME,
                    "extraction_artifacts_manifest" => {
                        DEFAULT_EXTRACTION_ARTIFACTS_MANIFEST_FILE_NAME
                    }
                    "slide_rectangles_manifest" => DEFAULT_SLIDE_RECTANGLES_MANIFEST_FILE_NAME,
                    "slide_notes" => DEFAULT_SLIDE_NOTES_ARTIFACT_FILE_NAME,
                    "video_slides_markdown" => DEFAULT_VIDEO_SLIDES_MARKDOWN_FILE_NAME,
                    "subtitle_page_map" => DEFAULT_SUBTITLE_PAGE_MAP_FILE_NAME,
                    _ => "artifact.json",
                };
                json!({
                    "artifact_kind": kind,
                    "artifact_id": format!("video-{}-{kind}", document.id),
                    "title": format!("artifact {kind}"),
                    "format": "application/json",
                    "path": format!("/private/video-extraction/{file_name}"),
                    "uri": format!("artifact://video-{}-{kind}", document.id),
                })
            })
            .collect::<Vec<_>>();
        let output = json!({
            "status": "completed",
            "document_id": document.id.to_string(),
            "dataset_id": document.dataset_id.to_string(),
            "title": "Published lesson",
            "generated_artifacts": {
                "status": "completed",
                "files": files,
            }
        });

        let manifest =
            video_extraction_durable_published_version_manifest(run_id, "workflow-1", &output)
                .expect("durable version manifest");
        let serialized = serde_json::to_string(&manifest).expect("manifest serializes");

        assert_eq!(
            manifest["manifest_type"],
            json!("v3.video_ppt_durable_published_version.v1")
        );
        assert_eq!(
            manifest["lifecycle_state"],
            json!("published_version_ready")
        );
        assert_eq!(
            manifest["durable_history_status"],
            json!("promoted_to_storage")
        );
        assert_eq!(
            manifest["package_key"],
            json!(format!("video-ppt-{run_id}-{}", document.id))
        );
        assert!(manifest["version_fingerprint"]
            .as_str()
            .expect("version fingerprint")
            .starts_with("video-ppt:v1:"));
        assert!(manifest["published_files"]
            .as_array()
            .expect("published files")
            .iter()
            .all(|file| file["path"] == json!("[redacted]")));
        assert!(!serialized.contains("/private/video-extraction"));
        assert!(!serialized.contains("token"));

        let incomplete = json!({
            "document_id": document.id.to_string(),
            "dataset_id": document.dataset_id.to_string(),
            "generated_artifacts": {
                "status": "completed",
                "files": [{
                    "artifact_kind": "pptx",
                    "path": "/private/video-extraction/video_slides_screenshot_based.pptx"
                }]
            }
        });
        assert!(video_extraction_durable_published_version_manifest(
            run_id,
            "workflow-1",
            &incomplete
        )
        .is_none());
    }

    #[test]
    fn merge_durable_published_version_ref_updates_video_output_artifact() {
        let run_id = "00000000-0000-0000-0000-000000000001";
        let document_id = "11111111-1111-1111-1111-111111111111";
        let output_artifacts = json!([{
            "id": format!("video-extraction-{run_id}-{document_id}"),
            "type": "video_extraction_artifacts",
            "deliverable_package": {
                "kind": "video_extraction_deliverable_package",
                "lifecycle_state": "published_version_ready"
            }
        }]);
        let durable_ref = json!({
            "type": "video_ppt_published_version",
            "package_id": "package-1",
            "version_id": "version-1",
            "version_no": 2,
            "lifecycle_state": "published_version_ready"
        });

        let merged = merge_video_extraction_durable_published_version_ref(
            &output_artifacts,
            run_id,
            document_id,
            &durable_ref,
        );

        assert_eq!(merged[0]["durable_published_version"], durable_ref);
        assert_eq!(
            merged[0]["deliverable_package"]["durable_history_status"],
            json!("promoted_to_storage")
        );
        assert_eq!(
            merged[0]["deliverable_package"]["durable_published_version"]["version_no"],
            json!(2)
        );
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
            }, {
                "artifact_kind": "extraction_artifacts_manifest",
                "artifact_id": format!("video-{}-extraction-artifacts", document.id),
                "title": "extraction artifacts manifest",
                "format": "application/json",
                "path": "generated_artifacts/extraction_artifacts_manifest.json"
            }, {
                "artifact_kind": "slide_rectangles_manifest",
                "artifact_id": format!("video-{}-slide-rectangles", document.id),
                "title": "slide rectangles manifest",
                "format": "application/json",
                "path": "generated_artifacts/slide_rectangles_manifest.json",
                "rectangle_extraction_status": "promoted_full_frame_fallback",
                "rectangle_extraction_mode": "full_frame_fallback",
                "promoted_rectangle_count": 1
            }, {
                "artifact_kind": "slide_notes",
                "artifact_id": format!("video-{}-slide-notes", document.id),
                "title": "slide notes",
                "format": "text/markdown",
                "path": "generated_artifacts/slide_notes.md"
            }, {
                "artifact_kind": "video_slides_markdown",
                "artifact_id": format!("video-{}-video-slides-markdown", document.id),
                "title": "video slides markdown",
                "format": "text/markdown",
                "path": "generated_artifacts/video_slides.md"
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
        assert!(follow_up["ready_file_kinds"]
            .as_array()
            .expect("ready file kinds")
            .contains(&json!("extraction_artifacts_manifest")));
        assert!(follow_up["ready_file_kinds"]
            .as_array()
            .expect("ready file kinds")
            .contains(&json!("slide_rectangles_manifest")));
        assert_eq!(
            follow_up["deliverable_status"]["has_slide_notes"],
            json!(true)
        );
        assert_eq!(
            follow_up["deliverable_status"]["has_video_slides_markdown"],
            json!(true)
        );
        assert_eq!(
            follow_up["deliverable_package"]["lifecycle_state"],
            json!("downloadable_not_published")
        );
        assert_eq!(follow_up["deliverable_package"]["publishable"], json!(true));
        assert_eq!(
            follow_up["deliverable_package"]["next_action"],
            json!("persist_video_published_version")
        );
        assert_eq!(
            follow_up["deliverable_package"]["source_run_id"],
            json!(run_id)
        );
        assert_eq!(
            follow_up["deliverable_package"]["missing_required_file_kinds"],
            json!([])
        );
        assert!(follow_up["next_actions"]
            .as_array()
            .expect("next actions")
            .contains(&json!("download_pptx")));
        assert!(follow_up["next_actions"]
            .as_array()
            .expect("next actions")
            .contains(&json!("review_extraction_artifacts_manifest")));
        assert!(follow_up["next_actions"]
            .as_array()
            .expect("next actions")
            .contains(&json!("review_slide_notes")));
        assert!(follow_up["next_actions"]
            .as_array()
            .expect("next actions")
            .contains(&json!("review_video_slides_markdown")));
        assert!(follow_up["next_actions"]
            .as_array()
            .expect("next actions")
            .contains(&json!("review_subtitle_page_map")));
        assert_eq!(follow_up["model_follow_up"]["required"], json!(true));
        assert_eq!(
            follow_up["model_follow_up"]["kind"],
            json!("video_extraction_model_completion_turn_request")
        );
        assert_eq!(
            follow_up["model_follow_up"]["completion_context"]["status"],
            json!("final_pptx_ready")
        );
        assert_eq!(
            follow_up["model_follow_up"]["completion_context"]["has_video_slides_markdown"],
            json!(true)
        );
        assert_eq!(
            follow_up["model_follow_up"]["answer_contract"]["must_not_claim_missing_files"],
            json!(true)
        );
        assert_eq!(
            follow_up["model_follow_up"]["answer_contract"]
                ["must_not_include_private_paths_or_urls"],
            json!(true)
        );
        assert_eq!(
            follow_up["user_notification"]["kind"],
            json!("video_extraction_status_notification")
        );
        assert_eq!(follow_up["user_notification"]["severity"], json!("success"));
        assert_eq!(
            follow_up["user_notification"]["no_host_composed_answer"],
            json!(true)
        );
        assert_eq!(
            follow_up["user_notification"]["html_artifact_ids"][0],
            html_artifact["id"]
        );
    }

    #[test]
    fn video_extraction_model_completion_dispatch_request_is_redacted_and_idempotent() {
        let output = json!({
            "document_id": "doc-1",
            "dataset_id": "dataset-1",
            "title": "客户课程视频",
            "status": "completed",
            "generated_artifacts": {
                "files": [{
                    "artifact_kind": "pptx",
                    "path": "C:\\Users\\soulzyn\\secret\\video_slides.pptx",
                    "uri": "https://internal.example.local/private/video_slides.pptx"
                }]
            }
        });
        let html_artifact = json!({"id": "html-artifact-video"});
        let follow_up =
            video_extraction_completion_follow_up_from_output(&output, &[html_artifact])
                .expect("follow up");

        let dispatch = video_extraction_model_completion_dispatch_request(
            "run-1",
            Some("workflow-1"),
            Some("video_extraction_workflow"),
            &output,
            Some(&follow_up),
        )
        .expect("dispatch request");

        assert_eq!(
            dispatch["kind"],
            json!("assistant_run_model_completion_turn_dispatch_request")
        );
        assert_eq!(dispatch["status"], json!("queued"));
        assert_eq!(dispatch["dispatch_target"], json!("continue_assistant_run"));
        assert_eq!(
            dispatch["idempotency_key"],
            json!("video-completion-turn:run-1:workflow-1:doc-1:v1")
        );
        assert_eq!(dispatch["continue_request"]["max_steps"], json!(1));
        assert_eq!(
            dispatch["model_completion_turn_request"]["kind"],
            json!("video_extraction_model_completion_turn_request")
        );
        assert_eq!(
            dispatch["privacy_contract"]["no_host_composed_answer"],
            json!(true)
        );
        assert_eq!(
            dispatch["privacy_contract"]["must_write_in_model_voice"],
            json!(true)
        );
        let serialized = dispatch.to_string();
        assert!(!serialized.contains("soulzyn"));
        assert!(!serialized.contains("secret"));
        assert!(!serialized.contains("video_slides.pptx"));
        assert!(!serialized.contains("internal.example.local"));
    }

    #[test]
    fn video_extraction_completion_follow_up_uses_warning_specific_next_actions() {
        let document = test_document();
        let frame_extraction = json!({
            "status": "failed",
            "source": "ffmpeg_external_process",
            "reason": "ffmpeg exited with status 1"
        });
        let generated_artifacts = json!({
            "status": "failed",
            "source": "media_worker_text_artifact_writer",
            "reason": "disk full",
            "files": []
        });
        let output = extract_video_ppt_output_with_artifacts(
            &document,
            &[],
            frame_extraction,
            generated_artifacts,
        );

        let follow_up =
            video_extraction_completion_follow_up_from_output(&output, &[]).expect("follow up");
        let next_actions = follow_up["next_actions"].as_array().expect("next actions");

        assert_eq!(
            follow_up["deliverable_package"]["lifecycle_state"],
            json!("not_ready")
        );
        assert_eq!(
            follow_up["deliverable_package"]["publishable"],
            json!(false)
        );
        assert_eq!(
            follow_up["deliverable_package"]["next_action"],
            json!("complete_required_deliverables")
        );
        assert!(next_actions.contains(&json!("retry_frame_extraction")));
        assert!(next_actions.contains(&json!("retry_generated_artifact_writer")));
        assert!(next_actions.contains(&json!("attach_or_parse_transcript_evidence")));
        assert!(next_actions.contains(&json!("generate_contact_sheet_from_raw_frames")));
        assert!(next_actions.contains(&json!("fill_ppt_keep_list_template")));
        assert!(next_actions.contains(&json!("complete_keep_list_or_review_missing_inputs")));
        assert_eq!(follow_up["user_notification"]["severity"], json!("warning"));
        assert_eq!(
            follow_up["user_notification"]["scope"],
            json!("status_only")
        );
        assert_eq!(
            follow_up["user_notification"]["no_host_composed_answer"],
            json!(true)
        );

        let skipped_output = extract_video_ppt_output_with_frame_extraction(
            &document,
            &[],
            json!({
                "status": "skipped",
                "source": "ffmpeg_external_process",
                "reason": "local_media_path_not_available"
            }),
        );
        let skipped_follow_up =
            video_extraction_completion_follow_up_from_output(&skipped_output, &[])
                .expect("skipped follow up");
        assert!(skipped_follow_up["next_actions"]
            .as_array()
            .expect("skipped next actions")
            .contains(&json!("provide_local_media_file_or_parsed_frames")));
    }

    #[test]
    fn video_extraction_completion_audit_redacts_source_and_provider_material() {
        let document = test_document();
        let output = json!({
            "status": "completed",
            "document_id": document.id.to_string(),
            "dataset_id": document.dataset_id.to_string(),
            "source_url": "https://private.example.test/video.mp4?token=secret-token",
            "cookie": "SESSION=secret-cookie",
            "provider_key": "sk-provider-secret",
            "provider_payload": {
                "raw_url": "https://private.example.test/raw",
                "authorization": "Bearer secret-token"
            },
            "source_summary": {
                "source_type": "direct_video_url",
                "asset_state": "remote_registered",
                "content_type": "video/mp4",
                "source_url": "https://private.example.test/video.mp4?token=secret-token",
                "source_url_present": true,
                "source_page_url": "https://private.example.test/page?cookie=secret-cookie",
                "source_page_url_present": true,
                "ingest_requires_env": "INGEST_REMOTE_MEDIA_ENABLED"
            },
            "frame_extraction": {
                "status": "failed",
                "reason": "C:/private/source/video.mp4 could not be read"
            },
            "generated_artifacts": {
                "status": "completed",
                "files": [{
                    "artifact_kind": "pptx",
                    "path": "generated_artifacts/video_slides_screenshot_based.pptx"
                }, {
                    "artifact_kind": "final_deliverables_manifest",
                    "path": "generated_artifacts/final_deliverables_manifest.json"
                }, {
                    "artifact_kind": "slide_notes",
                    "path": "generated_artifacts/slide_notes.md"
                }, {
                    "artifact_kind": "subtitle_page_map",
                    "path": "generated_artifacts/subtitle_page_map.json"
                }]
            },
            "deliverable_status": {
                "state": "evidence_artifacts_ready",
                "warning_count": 1,
                "warnings": [{
                    "code": "provider_failure",
                    "provider": "minimax",
                    "reason": "provider rejected sk-provider-secret",
                    "providers": [{
                        "provider": "minimax",
                        "capability": "native_video_understanding",
                        "status": "unsupported",
                        "supported": false,
                        "detail": "failed with sk-provider-secret"
                    }]
                }]
            }
        });

        let audit = video_extraction_completion_audit_from_output(&output);
        let serialized = serde_json::to_string(&audit).expect("audit serializes");

        assert_eq!(audit["kind"], json!("video_extraction_completion_audit"));
        assert_eq!(
            audit["state_transition"]["deliverable_state"],
            json!("evidence_artifacts_ready")
        );
        assert_eq!(audit["frame_extraction_status"], json!("failed"));
        assert_eq!(audit["generated_artifacts_status"], json!("completed"));
        assert_eq!(audit["provider_failure_count"], json!(1));
        assert_eq!(audit["artifact_group_counts"]["manifest_outputs"], json!(1));
        assert_eq!(audit["artifact_group_counts"]["final_outputs"], json!(1));
        assert_eq!(audit["artifact_group_counts"]["review_outputs"], json!(1));
        assert_eq!(audit["artifact_group_counts"]["evidence_outputs"], json!(1));
        assert_eq!(
            audit["source_resolution"]["source_type"],
            json!("direct_video_url")
        );
        assert_eq!(
            audit["source_resolution"]["source_url_redacted"],
            json!(true)
        );
        assert_eq!(
            audit["source_resolution"]["source_page_url_redacted"],
            json!(true)
        );
        assert_eq!(audit["provider_failures"][0]["provider"], json!("minimax"));
        assert_eq!(
            audit["provider_failures"][0]["capability"],
            json!("native_video_understanding")
        );
        assert!(audit["warning_codes"]
            .as_array()
            .expect("warning codes")
            .contains(&json!("provider_failure")));
        assert!(!serialized.contains("private.example.test"));
        assert!(!serialized.contains("secret-token"));
        assert!(!serialized.contains("secret-cookie"));
        assert!(!serialized.contains("sk-provider-secret"));
        assert!(!serialized.contains("C:/private/source"));
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
                    "text": "第一页讲产品定位",
                    "source": "MEDIA_TRANSCRIBE_BIN",
                    "evidence_ref": "chunk-1",
                    "confidence": 0.91
                }],
                "scenes": [{
                    "start_seconds": 0.0,
                    "end_seconds": 2.4,
                    "summary": "标题页",
                    "source": "MEDIA_SCENE_BIN"
                }],
                "keyframe_ocr_snippets": [{
                    "timestamp_seconds": 1.2,
                    "text": "AI Data Platform",
                    "source": "C:/private/video/frame_000001.jpg",
                    "ocr_confidence": 0.88
                }],
                "provider_evidence": [{
                    "provider": "minimax",
                    "capability": "native_video_understanding",
                    "status": "configured_unverified",
                    "supported": false,
                    "detail": "failed with secret-token"
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
            "raw_frames_dir": "C:/private/video-extraction/raw_frames",
            "manifest_path": "C:/private/video-extraction/frame_manifest.json",
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
        assert_eq!(manifest["files"].as_array().expect("files").len(), 8);
        assert!(manifest["files"]
            .as_array()
            .expect("files")
            .iter()
            .any(|file| file["artifact_kind"] == json!("extraction_artifacts_manifest")));

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
        assert!(source_text.contains("Evidence References"));
        assert!(source_text.contains("Quality Notes"));
        assert!(source_text.contains("provider_failure"));
        assert!(source_text.contains("Provider Evidence"));
        assert!(source_text.contains("ref=transcript#1"));
        assert!(source_text.contains("source=MEDIA_TRANSCRIBE_BIN"));
        assert!(source_text.contains("confidence=0.91"));
        assert!(source_text.contains("native_video_understanding"));
        assert!(source_text.contains("not_supported"));
        assert!(source_text.contains("AI Data Platform"));
        assert!(source_text.contains("Raw frames directory: [redacted]"));
        assert!(source_text.contains("Frame manifest: frame_manifest.json (path redacted)"));
        assert!(!source_text.contains("C:/private/video"));
        assert!(!source_text.contains("C:/private/video-extraction"));
        assert!(!source_text.contains("secret-token"));
        let outline_path = manifest["files"]
            .as_array()
            .expect("files")
            .iter()
            .find(|file| file["artifact_kind"] == json!("ppt_outline"))
            .and_then(|file| file["path"].as_str())
            .expect("outline path");
        let outline = fs::read_to_string(outline_path).expect("outline file");
        assert!(outline.contains("Evidence References"));
        assert!(outline.contains("Quality Notes"));
        assert!(outline.contains("provider_failure"));
        assert!(outline.contains("ref=scene#1"));
        assert!(outline.contains("source=[redacted]"));
        let timestamp_map_path = manifest["files"]
            .as_array()
            .expect("files")
            .iter()
            .find(|file| file["artifact_kind"] == json!("timestamp_map"))
            .and_then(|file| file["path"].as_str())
            .expect("timestamp map path");
        let timestamp_map = fs::read_to_string(timestamp_map_path).expect("timestamp map");
        assert!(timestamp_map.contains("paths_urls_tokens_and_provider_secrets_are_redacted"));
        assert!(timestamp_map.contains("\"source\": \"MEDIA_TRANSCRIBE_BIN\""));
        assert!(timestamp_map.contains("\"raw_frames_dir\": \"[redacted]\""));
        assert!(timestamp_map.contains("\"manifest_path\": \"[redacted]\""));
        assert!(!timestamp_map.contains("C:/private/video"));
        assert!(!timestamp_map.contains("C:/private/video-extraction"));
        assert!(!timestamp_map.contains("secret-token"));
        let final_manifest_path = manifest["files"]
            .as_array()
            .expect("files")
            .iter()
            .find(|file| file["artifact_kind"] == json!("final_deliverables_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("final deliverables manifest path");
        let final_manifest =
            fs::read_to_string(final_manifest_path).expect("final deliverables manifest");
        let final_manifest_json: Value =
            serde_json::from_str(&final_manifest).expect("final manifest json");
        assert!(final_manifest.contains("evidence_artifacts_ready"));
        assert!(final_manifest.contains("manifest_outputs"));
        assert!(final_manifest.contains("evidence_outputs"));
        assert!(final_manifest.contains("final_deliverables_manifest"));
        assert!(final_manifest.contains("published_deliverable_manifest"));
        assert!(final_manifest.contains("published_version_history"));
        assert!(final_manifest.contains("extraction_artifacts_manifest"));
        assert!(final_manifest.contains("\"path\": \"[redacted]\""));
        assert!(final_manifest.contains("\"file_name\""));
        assert!(!final_manifest.contains("aidp-v3-video-artifacts-test"));
        assert!(final_manifest_json["manifest_outputs"]
            .as_array()
            .expect("manifest outputs")
            .iter()
            .any(|file| file["artifact_kind"] == json!("published_deliverable_manifest")));
        assert!(final_manifest_json["manifest_outputs"]
            .as_array()
            .expect("manifest outputs")
            .iter()
            .any(|file| file["artifact_kind"] == json!("published_version_history")));
        assert!(final_manifest_json["manifest_outputs"]
            .as_array()
            .expect("manifest outputs")
            .iter()
            .any(|file| file["artifact_kind"] == json!("extraction_artifacts_manifest")));
        assert_eq!(
            final_manifest_json["deliverable_status"]["has_final_deliverables_manifest"],
            json!(true)
        );
        assert_eq!(
            final_manifest_json["deliverable_status"]["has_published_deliverable_manifest"],
            json!(true)
        );
        assert_eq!(
            final_manifest_json["deliverable_status"]["has_published_version_history"],
            json!(true)
        );
        assert_eq!(
            final_manifest_json["deliverable_status"]["has_extraction_artifacts_manifest"],
            json!(true)
        );
        let published_manifest_path = manifest["files"]
            .as_array()
            .expect("files")
            .iter()
            .find(|file| file["artifact_kind"] == json!("published_deliverable_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("published deliverable manifest path");
        let published_manifest =
            fs::read_to_string(published_manifest_path).expect("published deliverable manifest");
        assert!(published_manifest.contains("v3.video_ppt_published_deliverable.v1"));
        assert!(published_manifest.contains("not_ready"));
        assert!(published_manifest.contains("\"path\": \"[redacted]\""));
        assert!(!published_manifest.contains("aidp-v3-video-artifacts-test"));
        let published_history_path = manifest["files"]
            .as_array()
            .expect("files")
            .iter()
            .find(|file| file["artifact_kind"] == json!("published_version_history"))
            .and_then(|file| file["path"].as_str())
            .expect("published version history path");
        let published_history =
            fs::read_to_string(published_history_path).expect("published version history");
        assert!(published_history.contains("v3.video_ppt_published_version_history.v1"));
        assert!(published_history.contains("generated_artifact_workspace"));
        assert!(published_history.contains("\"path\": \"[redacted]\""));
        assert!(!published_history.contains("aidp-v3-video-artifacts-test"));
        let extraction_manifest_path = manifest["manifest_path"]
            .as_str()
            .expect("extraction manifest path");
        let extraction_manifest =
            fs::read_to_string(extraction_manifest_path).expect("extraction manifest");
        assert!(extraction_manifest.contains("extraction_artifacts_manifest"));
        assert!(extraction_manifest.contains("\"path\": \"[redacted]\""));
        assert!(extraction_manifest.contains("\"session_dir\": \"[redacted]\""));
        assert!(extraction_manifest.contains("\"manifest_path\": \"[redacted]\""));
        assert!(!extraction_manifest.contains("aidp-v3-video-artifacts-test"));
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
            "interval_seconds": 0.15,
            "manifest_file_name": DEFAULT_FRAME_MANIFEST_FILE_NAME
        });
        let chunk = test_chunk(json!({
            "media": {
                "transcript_segments": [{
                    "start_seconds": 0.0,
                    "end_seconds": 0.2,
                    "text": "Opening narration",
                    "source": "MEDIA_TRANSCRIBE_BIN"
                }],
                "scenes": [{
                    "start_seconds": 0.0,
                    "end_seconds": 0.3,
                    "summary": "Opening slide",
                    "source": "MEDIA_SCENE_BIN"
                }],
                "keyframe_ocr_snippets": [{
                    "timestamp_seconds": 0.15,
                    "text": "Opening Title",
                    "source": "C:/private/frame.jpg",
                    "ocr_confidence": 0.9
                }]
            }
        }));

        let manifest = write_video_extraction_text_artifacts(
            &document,
            &[chunk],
            &frame_extraction,
            &output_root,
        )
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
            .any(|file| file["artifact_kind"] == json!("slide_rectangles_manifest")));
        assert!(files
            .iter()
            .any(|file| file["artifact_kind"] == json!("pptx_build_plan")));
        assert!(files
            .iter()
            .any(|file| file["artifact_kind"] == json!("final_deliverables_manifest")));
        let slide_candidates_ref = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("slide_image_candidates"))
            .expect("slide candidates artifact ref");
        assert_eq!(slide_candidates_ref["candidate_count"], json!(2));
        assert_eq!(
            slide_candidates_ref["rectangle_extraction_status"],
            json!("not_promoted")
        );
        assert_eq!(
            slide_candidates_ref["selection_status"],
            json!("review_required")
        );
        let candidates_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("slide_image_candidates"))
            .and_then(|file| file["path"].as_str())
            .expect("candidate manifest path");
        let candidates = fs::read_to_string(candidates_path).expect("candidate manifest");
        assert!(candidates.contains("frame_000001.jpg"));
        assert!(candidates.contains("review_required"));
        assert!(candidates.contains("rectangle_extraction_status"));
        assert!(candidates.contains("candidate_evidence_policy"));
        assert!(candidates.contains("matched_nearby_evidence"));
        assert!(candidates.contains("\"raw_frames_dir\": \"[redacted]\""));
        assert!(candidates.contains("\"frame_path\": \"[redacted]\""));
        assert!(candidates.contains("Opening narration"));
        assert!(candidates.contains("Opening slide"));
        assert!(candidates.contains("Opening Title"));
        assert!(candidates.contains("source=[redacted]"));
        assert!(!candidates.contains("C:/private/frame.jpg"));
        assert!(!candidates.contains(&output_root.display().to_string()));
        let candidate_manifest: Value =
            serde_json::from_str(&candidates).expect("candidate manifest json");
        assert!(
            candidate_manifest["candidate_evidence_ref_count"]
                .as_u64()
                .expect("candidate evidence ref count")
                >= 3
        );
        let first_candidate = candidate_manifest["candidates"]
            .as_array()
            .expect("candidate array")
            .first()
            .expect("first candidate");
        assert_eq!(
            first_candidate["evidence_reference_status"],
            json!("matched_nearby_evidence")
        );
        assert!(!first_candidate["nearby_evidence_refs"]["scene_refs"]
            .as_array()
            .expect("scene refs")
            .is_empty());
        assert!(!first_candidate["nearby_evidence_refs"]["transcript_refs"]
            .as_array()
            .expect("transcript refs")
            .is_empty());
        assert!(!first_candidate["nearby_evidence_refs"]["ocr_refs"]
            .as_array()
            .expect("ocr refs")
            .is_empty());
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
        let contact_sheet_plan_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("contact_sheet_plan"))
            .and_then(|file| file["path"].as_str())
            .expect("contact sheet plan path");
        let contact_sheet_plan =
            fs::read_to_string(contact_sheet_plan_path).expect("contact sheet plan");
        assert!(contact_sheet_plan.contains("\"raw_frames_dir\": \"[redacted]\""));
        assert!(contact_sheet_plan.contains("\"candidate_manifest\": \"[redacted]\""));
        assert!(!contact_sheet_plan.contains(&output_root.display().to_string()));
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
        assert!(selected_slides.contains("\"candidate_manifest\": \"[redacted]\""));
        assert!(!selected_slides.contains(&output_root.display().to_string()));
        let selected_slides_ref = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("selected_slides_manifest"))
            .expect("selected slides artifact ref");
        assert_eq!(selected_slides_ref["selected_count"], json!(0));
        let slide_rectangles_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("slide_rectangles_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("slide rectangles manifest path");
        let slide_rectangles =
            fs::read_to_string(slide_rectangles_path).expect("slide rectangles manifest");
        assert!(slide_rectangles.contains("waiting_for_selection"));
        assert!(slide_rectangles.contains("\"promoted_rectangle_count\": 0"));
        assert!(slide_rectangles.contains("\"candidate_manifest\": \"[redacted]\""));
        assert!(!slide_rectangles.contains(&output_root.display().to_string()));
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
        assert!(pptx_plan.contains("\"keep_list_template\": \"[redacted]\""));
        assert!(pptx_plan.contains("\"recommended_output\": \"[redacted]\""));
        assert!(pptx_plan.contains("selected_candidate_indices"));
        assert!(pptx_plan.contains(DEFAULT_VIDEO_SLIDES_PPTX_FILE_NAME));
        assert!(!pptx_plan.contains(&output_root.display().to_string()));
        let final_manifest_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("final_deliverables_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("final deliverables manifest path");
        let final_manifest =
            fs::read_to_string(final_manifest_path).expect("final deliverables manifest");
        assert!(final_manifest.contains("review_ready"));
        assert!(final_manifest.contains("review_outputs"));
        assert!(final_manifest.contains("slide_notes"));
        assert!(final_manifest.contains("video_slides_markdown"));
        let generated_artifacts = json!({
            "status": "completed",
            "files": files.clone()
        });
        let deliverable_status = video_deliverable_status(&generated_artifacts);
        let warnings = deliverable_status["warnings"]
            .as_array()
            .expect("deliverable warnings");
        assert!(warnings
            .iter()
            .any(|warning| warning["code"] == json!("no_slide_rectangle_found")));
        let next_actions = video_extraction_completion_next_actions(
            deliverable_status["state"]
                .as_str()
                .expect("deliverable state"),
            files,
            &deliverable_status,
        );
        assert!(
            next_actions
                .iter()
                .any(|action| action
                    == &json!("review_contact_sheet_or_promote_rectangle_extraction"))
        );
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
        fs::write(raw_frames_dir.join("frame_000001.jpg"), b"fake-1").expect("frame 1");
        fs::write(raw_frames_dir.join("frame_000002.jpg"), b"fake-2").expect("frame 2");
        fs::write(raw_frames_dir.join("frame_000003.jpg"), b"fake-3").expect("frame 3");
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
        assert!(selected_slides.contains("promoted_full_frame_fallback"));
        assert!(selected_slides.contains("selected_keep_list_order_deduped"));
        let selected_slides_json: Value =
            serde_json::from_str(&selected_slides).expect("selected slides json");
        assert_eq!(
            selected_slides_json["selected_candidate_indices"],
            json!([2, 1])
        );
        assert_eq!(
            selected_slides_json["requested_selected_candidate_indices"],
            json!([2, 1])
        );
        assert_eq!(selected_slides_json["selected_count"], json!(2));
        assert_eq!(selected_slides_json["requested_selected_count"], json!(2));
        let selected_slides_ref = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("selected_slides_manifest"))
            .expect("selected slides artifact ref");
        assert_eq!(selected_slides_ref["selected_count"], json!(2));
        let slide_rectangles_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("slide_rectangles_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("slide rectangles manifest path");
        let slide_rectangles =
            fs::read_to_string(slide_rectangles_path).expect("slide rectangles manifest");
        assert!(slide_rectangles.contains("promoted_full_frame_fallback"));
        assert!(slide_rectangles.contains("\"promoted_rectangle_count\": 2"));
        assert!(slide_rectangles.contains("\"unit\": \"relative\""));
        assert!(slide_rectangles.contains("\"review_required\": true"));
        assert!(!slide_rectangles.contains(&raw_frames_dir.display().to_string()));
        let slide_quality_report_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("slide_quality_report"))
            .and_then(|file| file["path"].as_str())
            .expect("slide quality report path");
        let slide_quality_report =
            fs::read_to_string(slide_quality_report_path).expect("slide quality report");
        assert!(slide_quality_report.contains("v3.video_ppt_slide_quality_report.v1"));
        assert!(slide_quality_report.contains("full_frame_rectangle_fallback"));
        assert!(slide_quality_report.contains("manual_review_required"));
        assert!(!slide_quality_report.contains(&raw_frames_dir.display().to_string()));
        let slide_quality_report_json: Value =
            serde_json::from_str(&slide_quality_report).expect("quality report json");
        assert_eq!(slide_quality_report_json["slide_count"], json!(2));
        assert_eq!(
            slide_quality_report_json["summary"]["full_frame_fallback_count"],
            json!(2)
        );
        assert_eq!(
            slide_quality_report_json["summary"]["sharpness_unknown_count"],
            json!(2)
        );
        assert_eq!(
            slide_quality_report_json["summary"]["ocr_missing_count"],
            json!(2)
        );
        let missing_ocr_risk = slide_quality_report_json["risk_flags"]
            .as_array()
            .expect("risk flags")
            .iter()
            .find(|risk| risk["code"] == json!("missing_ocr_evidence"))
            .expect("missing ocr evidence risk flag");
        assert_eq!(missing_ocr_risk["severity"], json!("low"));
        assert_eq!(missing_ocr_risk["count"], json!(2));
        assert_eq!(
            missing_ocr_risk["review_action"],
            json!("run_or_review_ocr_evidence_before_customer_delivery")
        );
        assert_eq!(
            slide_quality_report_json["slides"][0]["sharpness_status"],
            json!("unavailable")
        );
        assert_eq!(
            slide_quality_report_json["slides"][0]["sharpness_score"],
            Value::Null
        );
        assert_eq!(
            slide_quality_report_json["slides"][0]["sharpness_risk"],
            json!("unknown")
        );
        assert_eq!(
            slide_quality_report_json["slides"][0]["crop_risk"],
            json!("high")
        );
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
        assert!(pptx_plan.contains("slide_rectangles_manifest.json"));
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
        let mut slide_xml = String::new();
        archive
            .by_name("ppt/slides/slide1.xml")
            .expect("slide xml")
            .read_to_string(&mut slide_xml)
            .expect("slide xml text");
        assert!(!slide_xml.contains("srcRect"));
        let mut notes = String::new();
        archive
            .by_name("ppt/notesSlides/notesSlide1.xml")
            .expect("notes slide")
            .read_to_string(&mut notes)
            .expect("notes slide text");
        assert!(notes.contains("Source frame"));
        assert!(notes.contains("[redacted]"));
        assert!(!notes.contains(&raw_frames_dir.display().to_string()));
        let final_manifest_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("final_deliverables_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("final deliverables manifest path");
        let final_manifest =
            fs::read_to_string(final_manifest_path).expect("final deliverables manifest");
        assert!(final_manifest.contains("final_pptx_ready"));
        assert!(final_manifest.contains("manifest_outputs"));
        assert!(final_manifest.contains("final_outputs"));
        assert!(final_manifest.contains("slide_notes"));
        assert!(final_manifest.contains("slide_quality_report"));
        assert!(final_manifest.contains("video_slides_markdown"));
        assert!(final_manifest.contains(DEFAULT_VIDEO_SLIDES_MARKDOWN_FILE_NAME));
        assert!(final_manifest.contains(DEFAULT_VIDEO_SLIDES_PPTX_FILE_NAME));
        assert!(final_manifest.contains("speaker_notes_metadata_only"));
        assert!(final_manifest.contains("full_frame_rectangle_fallback"));
        let slide_notes_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("slide_notes"))
            .and_then(|file| file["path"].as_str())
            .expect("slide notes path");
        let slide_notes = fs::read_to_string(slide_notes_path).expect("slide notes");
        assert!(slide_notes.contains("Deck Summary"));
        assert!(slide_notes.contains("Selected slides: 2"));
        assert!(slide_notes.contains("Dedupe status: selected_keep_list_order_deduped"));
        assert!(slide_notes.contains("Rectangle mode: full_frame_fallback"));
        assert!(slide_notes.contains("Slide 1"));
        assert!(slide_notes.contains("candidate 2"));
        assert!(slide_notes.contains("Contact sheet anchor: `candidate-2`"));
        assert!(
            slide_notes.contains("Transcript window: 0:00 -> 0:00 (pre_page_previous_to_current)")
        );
        assert!(
            slide_notes.contains("Rectangle: promoted_full_frame_fallback / full_frame_fallback")
        );
        assert!(slide_notes.contains("Crop box: x=0, y=0, width=1, height=1 (relative)"));
        assert!(slide_notes.contains("Review required: yes"));
        assert!(slide_notes.contains("Internal frame path: [redacted]"));
        assert!(!slide_notes.contains(&raw_frames_dir.display().to_string()));
        let video_slides_markdown_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("video_slides_markdown"))
            .and_then(|file| file["path"].as_str())
            .expect("video slides markdown path");
        let video_slides_markdown =
            fs::read_to_string(video_slides_markdown_path).expect("video slides markdown");
        assert!(video_slides_markdown.contains("Video Slides"));
        assert!(video_slides_markdown.contains("PPTX companion"));
        assert!(video_slides_markdown.contains("candidate 2"));
        assert!(video_slides_markdown.contains("Source frame: `frame_000002.jpg`"));
        assert!(video_slides_markdown.contains("Crop status: promoted_full_frame_fallback"));
        assert!(!video_slides_markdown.contains(&raw_frames_dir.display().to_string()));
    }

    #[test]
    fn flags_single_slide_output_for_quality_review_without_blocking_delivery() {
        let document = test_document();
        let output_root = std::env::temp_dir().join(format!(
            "aidp-v3-video-single-slide-review-test-{}",
            DocumentId::new()
        ));
        let session_dir = output_root.join(format!("video-extraction-{}", document.id));
        let artifacts_dir = session_dir.join(DEFAULT_GENERATED_ARTIFACTS_DIR_NAME);
        let raw_frames_dir = session_dir.join(DEFAULT_RAW_FRAMES_DIR_NAME);
        fs::create_dir_all(&raw_frames_dir).expect("raw frames dir");
        fs::create_dir_all(&artifacts_dir).expect("artifacts dir");
        fs::write(raw_frames_dir.join("frame_000001.jpg"), b"fake-1").expect("frame 1");
        fs::write(
            artifacts_dir.join(DEFAULT_PPT_KEEP_LIST_TEMPLATE_FILE_NAME),
            serde_json::to_vec_pretty(&json!({
                "status": "selected",
                "selected_candidate_indices": [1]
            }))
            .expect("keep list bytes"),
        )
        .expect("keep list");
        let frame_extraction = json!({
            "status": "completed",
            "raw_frames_dir": raw_frames_dir.display().to_string(),
            "frame_count": 1,
            "manifest_file_name": DEFAULT_FRAME_MANIFEST_FILE_NAME
        });

        let manifest =
            write_video_extraction_text_artifacts(&document, &[], &frame_extraction, &output_root)
                .expect("candidate artifacts");

        let files = manifest["files"].as_array().expect("files");
        let generated_artifacts = json!({
            "status": "completed",
            "files": files.clone()
        });
        let deliverable_status = video_deliverable_status(&generated_artifacts);
        assert_eq!(
            deliverable_status["state"],
            json!("final_pptx_ready"),
            "single-slide review risk must not block screenshot PPTX delivery"
        );

        let selected_slides_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("selected_slides_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("selected slides manifest path");
        let selected_slides: Value = serde_json::from_str(
            &fs::read_to_string(selected_slides_path).expect("selected slides manifest"),
        )
        .expect("selected slides manifest json");
        assert_eq!(selected_slides["selected_count"], json!(1));

        let slide_quality_report_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("slide_quality_report"))
            .and_then(|file| file["path"].as_str())
            .expect("slide quality report path");
        let slide_quality_report =
            fs::read_to_string(slide_quality_report_path).expect("slide quality report");
        assert!(!slide_quality_report.contains(&raw_frames_dir.display().to_string()));
        let slide_quality_report_json: Value =
            serde_json::from_str(&slide_quality_report).expect("quality report json");
        assert_eq!(slide_quality_report_json["slide_count"], json!(1));
        assert_eq!(
            slide_quality_report_json["summary"]["single_slide_output"],
            json!(true)
        );
        let single_slide_risk = slide_quality_report_json["risk_flags"]
            .as_array()
            .expect("risk flags")
            .iter()
            .find(|risk| risk["code"] == json!("single_slide_output_review_required"))
            .expect("single slide risk flag");
        assert_eq!(single_slide_risk["severity"], json!("medium"));
        assert_eq!(
            single_slide_risk["review_action"],
            json!("confirm_video_contains_only_one_ppt_or_reprocess_with_more_coverage")
        );
    }

    #[test]
    fn dedupes_selected_slide_manifest_by_exact_frame_content() {
        let document = test_document();
        let output_root = std::env::temp_dir().join(format!(
            "aidp-v3-video-selected-dedupe-test-{}",
            DocumentId::new()
        ));
        let session_dir = output_root.join(format!("video-extraction-{}", document.id));
        let artifacts_dir = session_dir.join(DEFAULT_GENERATED_ARTIFACTS_DIR_NAME);
        let raw_frames_dir = session_dir.join(DEFAULT_RAW_FRAMES_DIR_NAME);
        fs::create_dir_all(&raw_frames_dir).expect("raw frames dir");
        fs::create_dir_all(&artifacts_dir).expect("artifacts dir");
        fs::write(raw_frames_dir.join("frame_000001.jpg"), b"duplicate-frame").expect("frame 1");
        fs::write(raw_frames_dir.join("frame_000002.jpg"), b"duplicate-frame").expect("frame 2");
        fs::write(raw_frames_dir.join("frame_000003.jpg"), b"unique-frame").expect("frame 3");
        fs::write(
            artifacts_dir.join(DEFAULT_PPT_KEEP_LIST_TEMPLATE_FILE_NAME),
            serde_json::to_vec_pretty(&json!({
                "status": "selected",
                "selected_candidate_indices": [1, 2, 3]
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
        let selected_slides_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("selected_slides_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("selected slides manifest path");
        let selected_slides: Value = serde_json::from_str(
            &fs::read_to_string(selected_slides_path).expect("selected slides manifest"),
        )
        .expect("selected slides manifest json");
        assert_eq!(selected_slides["requested_selected_count"], json!(3));
        assert_eq!(selected_slides["selected_count"], json!(2));
        assert_eq!(
            selected_slides["requested_selected_candidate_indices"],
            json!([1, 2, 3])
        );
        assert_eq!(selected_slides["selected_candidate_indices"], json!([1, 3]));
        assert_eq!(selected_slides["deduped_candidate_count"], json!(1));
        assert_eq!(
            selected_slides["dedupe_status"],
            json!("exact_frame_content_deduped")
        );
        assert_eq!(
            selected_slides["rejected_duplicate_candidates"][0]["candidate_index"],
            json!(2)
        );
        assert_eq!(
            selected_slides["selected_candidates"][1]["candidate_index"],
            json!(3)
        );
        let slide_rectangles_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("slide_rectangles_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("slide rectangles manifest path");
        let slide_rectangles: Value = serde_json::from_str(
            &fs::read_to_string(slide_rectangles_path).expect("slide rectangles manifest"),
        )
        .expect("slide rectangles manifest json");
        assert_eq!(slide_rectangles["promoted_rectangle_count"], json!(2));
        assert_eq!(slide_rectangles["deduped_candidate_count"], json!(1));
        assert_eq!(
            slide_rectangles["dedupe_status"],
            json!("exact_frame_content_deduped")
        );
        assert_eq!(
            slide_rectangles["rejected_duplicate_candidates"][0]["dedupe_reason"],
            json!("exact_frame_content_duplicate")
        );
        let slide_rectangles_ref = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("slide_rectangles_manifest"))
            .expect("slide rectangles artifact ref");
        assert_eq!(slide_rectangles_ref["deduped_candidate_count"], json!(1));

        let pptx_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("pptx"))
            .and_then(|file| file["path"].as_str())
            .expect("pptx path");
        let mut archive =
            ZipArchive::new(File::open(pptx_path).expect("pptx file")).expect("pptx zip");
        assert!(archive.by_name("ppt/slides/slide1.xml").is_ok());
        assert!(archive.by_name("ppt/slides/slide2.xml").is_ok());
        assert!(archive.by_name("ppt/slides/slide3.xml").is_err());
    }

    #[test]
    fn dedupes_selected_slide_manifest_by_visual_similarity() {
        let document = test_document();
        let output_root = std::env::temp_dir().join(format!(
            "aidp-v3-video-selected-visual-dedupe-test-{}",
            DocumentId::new()
        ));
        let session_dir = output_root.join(format!("video-extraction-{}", document.id));
        let artifacts_dir = session_dir.join(DEFAULT_GENERATED_ARTIFACTS_DIR_NAME);
        let raw_frames_dir = session_dir.join(DEFAULT_RAW_FRAMES_DIR_NAME);
        fs::create_dir_all(&raw_frames_dir).expect("raw frames dir");
        fs::create_dir_all(&artifacts_dir).expect("artifacts dir");
        write_test_visual_slide_png(
            &raw_frames_dir.join("frame_000001.png"),
            [8, 8, 8],
            [240, 240, 240],
            20..80,
            10..60,
        );
        write_test_visual_slide_png(
            &raw_frames_dir.join("frame_000002.png"),
            [9, 9, 9],
            [242, 242, 242],
            20..80,
            10..60,
        );
        write_test_visual_slide_png(
            &raw_frames_dir.join("frame_000003.png"),
            [8, 8, 8],
            [120, 180, 240],
            5..45,
            10..60,
        );
        fs::write(
            artifacts_dir.join(DEFAULT_PPT_KEEP_LIST_TEMPLATE_FILE_NAME),
            serde_json::to_vec_pretty(&json!({
                "status": "selected",
                "selected_candidate_indices": [1, 2, 3]
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
        let selected_slides_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("selected_slides_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("selected slides manifest path");
        let selected_slides: Value = serde_json::from_str(
            &fs::read_to_string(selected_slides_path).expect("selected slides manifest"),
        )
        .expect("selected slides manifest json");
        assert_eq!(selected_slides["requested_selected_count"], json!(3));
        assert_eq!(selected_slides["selected_count"], json!(2));
        assert_eq!(
            selected_slides["requested_selected_candidate_indices"],
            json!([1, 2, 3])
        );
        assert_eq!(selected_slides["selected_candidate_indices"], json!([1, 3]));
        assert_eq!(selected_slides["deduped_candidate_count"], json!(1));
        assert_eq!(selected_slides["exact_duplicate_count"], json!(0));
        assert_eq!(selected_slides["visual_duplicate_count"], json!(1));
        assert_eq!(
            selected_slides["dedupe_status"],
            json!("visual_similarity_deduped")
        );
        assert_eq!(
            selected_slides["rejected_duplicate_candidates"][0]["dedupe_reason"],
            json!("visual_near_duplicate")
        );
        assert_eq!(
            selected_slides["rejected_duplicate_candidates"][0]["matched_candidate_index"],
            json!(1)
        );
        assert_eq!(
            selected_slides["selected_candidates"][1]["candidate_index"],
            json!(3)
        );
        assert_eq!(
            selected_slides["selected_candidates"][0]["visual_fingerprint_status"],
            json!("available")
        );

        let slide_rectangles_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("slide_rectangles_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("slide rectangles manifest path");
        let slide_rectangles: Value = serde_json::from_str(
            &fs::read_to_string(slide_rectangles_path).expect("slide rectangles manifest"),
        )
        .expect("slide rectangles manifest json");
        assert_eq!(slide_rectangles["promoted_rectangle_count"], json!(2));
        assert_eq!(slide_rectangles["deduped_candidate_count"], json!(1));
        assert_eq!(slide_rectangles["visual_duplicate_count"], json!(1));
        assert_eq!(
            slide_rectangles["dedupe_status"],
            json!("visual_similarity_deduped")
        );
        let slide_rectangles_ref = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("slide_rectangles_manifest"))
            .expect("slide rectangles artifact ref");
        assert_eq!(slide_rectangles_ref["deduped_candidate_count"], json!(1));
        assert_eq!(slide_rectangles_ref["exact_duplicate_count"], json!(0));
        assert_eq!(slide_rectangles_ref["visual_duplicate_count"], json!(1));

        let generated_artifacts = json!({
            "status": "completed",
            "files": files.clone(),
        });
        let deliverable_status = video_deliverable_status(&generated_artifacts);
        let warning_codes = deliverable_warning_codes(&deliverable_status);
        assert!(warning_codes.contains("selected_slide_duplicates_removed"));
        let dedupe_warning = deliverable_status["warnings"]
            .as_array()
            .expect("warnings")
            .iter()
            .find(|warning| warning["code"] == json!("selected_slide_duplicates_removed"))
            .expect("dedupe warning");
        assert_eq!(dedupe_warning["deduped_candidate_count"], json!(1));
        assert_eq!(dedupe_warning["visual_duplicate_count"], json!(1));
        let next_actions = video_extraction_completion_next_actions(
            deliverable_status["state"]
                .as_str()
                .expect("deliverable state"),
            files,
            &deliverable_status,
        );
        assert!(next_actions
            .iter()
            .any(|action| action.as_str() == Some("review_slide_dedupe_manifest")));

        let slide_notes_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("slide_notes"))
            .and_then(|file| file["path"].as_str())
            .expect("slide notes path");
        let slide_notes = fs::read_to_string(slide_notes_path).expect("slide notes");
        assert!(slide_notes.contains("Selected slide dedupe removed 1 duplicate"));

        let pptx_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("pptx"))
            .and_then(|file| file["path"].as_str())
            .expect("pptx path");
        let mut archive =
            ZipArchive::new(File::open(pptx_path).expect("pptx file")).expect("pptx zip");
        assert!(archive.by_name("ppt/slides/slide1.xml").is_ok());
        assert!(archive.by_name("ppt/slides/slide2.xml").is_ok());
        assert!(archive.by_name("ppt/slides/slide3.xml").is_err());
    }

    #[test]
    fn dedupes_selected_slide_manifest_by_visual_shape_similarity() {
        let document = test_document();
        let output_root = std::env::temp_dir().join(format!(
            "aidp-v3-video-selected-shape-dedupe-test-{}",
            DocumentId::new()
        ));
        let session_dir = output_root.join(format!("video-extraction-{}", document.id));
        let artifacts_dir = session_dir.join(DEFAULT_GENERATED_ARTIFACTS_DIR_NAME);
        let raw_frames_dir = session_dir.join(DEFAULT_RAW_FRAMES_DIR_NAME);
        fs::create_dir_all(&raw_frames_dir).expect("raw frames dir");
        fs::create_dir_all(&artifacts_dir).expect("artifacts dir");
        write_test_shape_duplicate_slide_png(
            &raw_frames_dir.join("frame_000001.png"),
            [236, 238, 236],
            0,
        );
        write_test_shape_duplicate_slide_png(
            &raw_frames_dir.join("frame_000002.png"),
            [92, 96, 94],
            0,
        );
        write_test_shape_duplicate_slide_png(
            &raw_frames_dir.join("frame_000003.png"),
            [236, 238, 236],
            1,
        );
        fs::write(
            artifacts_dir.join(DEFAULT_PPT_KEEP_LIST_TEMPLATE_FILE_NAME),
            serde_json::to_vec_pretty(&json!({
                "status": "selected",
                "selected_candidate_indices": [1, 2, 3]
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
        let selected_slides_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("selected_slides_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("selected slides manifest path");
        let selected_slides: Value = serde_json::from_str(
            &fs::read_to_string(selected_slides_path).expect("selected slides manifest"),
        )
        .expect("selected slides manifest json");
        assert_eq!(selected_slides["requested_selected_count"], json!(3));
        assert_eq!(selected_slides["selected_count"], json!(2));
        assert_eq!(
            selected_slides["requested_selected_candidate_indices"],
            json!([1, 2, 3])
        );
        assert_eq!(selected_slides["selected_candidate_indices"], json!([1, 3]));
        assert_eq!(selected_slides["deduped_candidate_count"], json!(1));
        assert_eq!(selected_slides["visual_duplicate_count"], json!(1));
        assert_eq!(selected_slides["visual_shape_duplicate_count"], json!(1));
        assert_eq!(
            selected_slides["dedupe_status"],
            json!("visual_similarity_deduped")
        );
        assert_eq!(
            selected_slides["rejected_duplicate_candidates"][0]["dedupe_reason"],
            json!("visual_shape_duplicate")
        );
        assert_eq!(
            selected_slides["rejected_duplicate_candidates"][0]["matched_candidate_index"],
            json!(1)
        );

        let slide_rectangles_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("slide_rectangles_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("slide rectangles manifest path");
        let slide_rectangles: Value = serde_json::from_str(
            &fs::read_to_string(slide_rectangles_path).expect("slide rectangles manifest"),
        )
        .expect("slide rectangles manifest json");
        assert_eq!(slide_rectangles["promoted_rectangle_count"], json!(2));
        assert_eq!(slide_rectangles["visual_duplicate_count"], json!(1));
        assert_eq!(slide_rectangles["visual_shape_duplicate_count"], json!(1));

        let slide_quality_report_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("slide_quality_report"))
            .and_then(|file| file["path"].as_str())
            .expect("slide quality report path");
        let slide_quality_report: Value = serde_json::from_str(
            &fs::read_to_string(slide_quality_report_path).expect("slide quality report"),
        )
        .expect("slide quality report json");
        assert_eq!(
            slide_quality_report["summary"]["visual_duplicate_count"],
            json!(1)
        );
        assert_eq!(
            slide_quality_report["summary"]["visual_shape_duplicate_count"],
            json!(1)
        );
    }

    #[test]
    fn keeps_bright_template_build_states_out_of_visual_shape_dedupe() {
        let document = test_document();
        let output_root = std::env::temp_dir().join(format!(
            "aidp-v3-video-bright-template-build-dedupe-test-{}",
            DocumentId::new()
        ));
        let session_dir = output_root.join(format!("video-extraction-{}", document.id));
        let artifacts_dir = session_dir.join(DEFAULT_GENERATED_ARTIFACTS_DIR_NAME);
        let raw_frames_dir = session_dir.join(DEFAULT_RAW_FRAMES_DIR_NAME);
        fs::create_dir_all(&raw_frames_dir).expect("raw frames dir");
        fs::create_dir_all(&artifacts_dir).expect("artifacts dir");
        write_test_bright_template_build_slide_png(&raw_frames_dir.join("frame_000001.png"), 0);
        write_test_bright_template_build_slide_png(&raw_frames_dir.join("frame_000002.png"), 1);
        write_test_bright_template_build_slide_png(&raw_frames_dir.join("frame_000003.png"), 2);
        fs::write(
            artifacts_dir.join(DEFAULT_PPT_KEEP_LIST_TEMPLATE_FILE_NAME),
            serde_json::to_vec_pretty(&json!({
                "status": "selected",
                "selected_candidate_indices": [1, 2, 3]
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
        let selected_slides_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("selected_slides_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("selected slides manifest path");
        let selected_slides: Value = serde_json::from_str(
            &fs::read_to_string(selected_slides_path).expect("selected slides manifest"),
        )
        .expect("selected slides manifest json");
        assert_eq!(selected_slides["requested_selected_count"], json!(3));
        assert_eq!(selected_slides["selected_count"], json!(3));
        assert_eq!(
            selected_slides["selected_candidate_indices"],
            json!([1, 2, 3])
        );
        assert_eq!(selected_slides["deduped_candidate_count"], json!(0));
        assert_eq!(selected_slides["visual_duplicate_count"], json!(0));
        assert_eq!(selected_slides["visual_shape_duplicate_count"], json!(0));
    }

    #[test]
    fn auto_selects_stable_ppt_pages_from_decodable_raw_frames_without_keep_list() {
        let document = test_document();
        let output_root = std::env::temp_dir().join(format!(
            "aidp-v3-video-auto-selected-slides-test-{}",
            DocumentId::new()
        ));
        let session_dir = output_root.join(format!("video-extraction-{}", document.id));
        let artifacts_dir = session_dir.join(DEFAULT_GENERATED_ARTIFACTS_DIR_NAME);
        let raw_frames_dir = session_dir.join(DEFAULT_RAW_FRAMES_DIR_NAME);
        fs::create_dir_all(&raw_frames_dir).expect("raw frames dir");
        fs::create_dir_all(&artifacts_dir).expect("artifacts dir");

        for frame_index in 1..=3 {
            write_test_visual_slide_png(
                &raw_frames_dir.join(format!("frame_{frame_index:06}.png")),
                [8, 8, 8],
                [240, 240, 240],
                20..80,
                10..60,
            );
        }
        for frame_index in 4..=6 {
            write_test_visual_slide_png(
                &raw_frames_dir.join(format!("frame_{frame_index:06}.png")),
                [24, 24, 24],
                [230, 230, 230],
                12..88,
                16..68,
            );
        }
        for frame_index in 7..=9 {
            write_test_visual_slide_png(
                &raw_frames_dir.join(format!("frame_{frame_index:06}.png")),
                [42, 42, 42],
                [248, 248, 248],
                28..74,
                6..74,
            );
        }

        let frame_extraction = json!({
            "status": "completed",
            "raw_frames_dir": raw_frames_dir.display().to_string(),
            "frame_count": 9,
            "interval_seconds": 0.15,
            "manifest_file_name": DEFAULT_FRAME_MANIFEST_FILE_NAME
        });

        let manifest =
            write_video_extraction_text_artifacts(&document, &[], &frame_extraction, &output_root)
                .expect("candidate artifacts");
        let files = manifest["files"].as_array().expect("files");

        let candidate_manifest_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("slide_image_candidates"))
            .and_then(|file| file["path"].as_str())
            .expect("candidate manifest path");
        let candidate_manifest: Value = serde_json::from_str(
            &fs::read_to_string(candidate_manifest_path).expect("candidate manifest"),
        )
        .expect("candidate manifest json");
        assert_eq!(candidate_manifest["status"], json!("auto_selected"));
        assert_eq!(
            candidate_manifest["selection_source"],
            json!("auto_unique_slide_keyframes")
        );
        assert_eq!(
            candidate_manifest["selected_candidate_indices"],
            json!([2, 5, 8])
        );
        assert_eq!(
            candidate_manifest["auto_selection"]["selected_page_count"],
            json!(3)
        );
        assert!(!candidate_manifest
            .to_string()
            .contains(&raw_frames_dir.display().to_string()));

        let selected_slides_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("selected_slides_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("selected slides manifest path");
        let selected_slides: Value = serde_json::from_str(
            &fs::read_to_string(selected_slides_path).expect("selected slides manifest"),
        )
        .expect("selected slides manifest json");
        assert_eq!(selected_slides["status"], json!("ready_for_pptx_writer"));
        assert_eq!(
            selected_slides["source"],
            json!("auto_unique_slide_keyframes")
        );
        assert_eq!(selected_slides["selected_count"], json!(3));
        assert_eq!(
            selected_slides["selected_candidate_indices"],
            json!([2, 5, 8])
        );

        let pptx_plan_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("pptx_build_plan"))
            .and_then(|file| file["path"].as_str())
            .expect("pptx build plan path");
        let pptx_plan = fs::read_to_string(pptx_plan_path).expect("pptx build plan");
        assert!(pptx_plan.contains("one_raster_image_per_auto_detected_ppt_page"));
        assert!(pptx_plan.contains("auto_unique_slide_keyframes"));

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
        assert!(archive.by_name("ppt/slides/slide3.xml").is_ok());
        assert!(archive.by_name("ppt/slides/slide4.xml").is_err());
        assert!(archive.by_name("ppt/media/image1.png").is_ok());
        assert!(archive.by_name("ppt/media/image2.png").is_ok());
        assert!(archive.by_name("ppt/media/image3.png").is_ok());
    }

    #[test]
    fn selects_highest_sharpness_frame_from_stable_auto_slide_cluster() {
        let signature = VideoFrameVisualSignature {
            fingerprint: "test-signature".to_string(),
            samples: vec![12, 64, 128, 220],
        };
        let cluster = VideoAutoSlideCluster {
            frames: vec![
                VideoAutoSlideFrame {
                    candidate_index: 10,
                    file_name: "frame_000010.png".to_string(),
                    signature: signature.clone(),
                    sharpness_score: Some(80),
                },
                VideoAutoSlideFrame {
                    candidate_index: 11,
                    file_name: "frame_000011.png".to_string(),
                    signature: signature.clone(),
                    sharpness_score: Some(20),
                },
                VideoAutoSlideFrame {
                    candidate_index: 12,
                    file_name: "frame_000012.png".to_string(),
                    signature: signature.clone(),
                    sharpness_score: Some(40),
                },
            ],
        };
        let (selected, selection_rule) = video_select_auto_slide_cluster_frame(&cluster);
        assert_eq!(selected.candidate_index, 10);
        assert_eq!(
            selection_rule,
            "highest_sharpness_frame_of_stable_visual_segment"
        );

        let midpoint_cluster = VideoAutoSlideCluster {
            frames: vec![
                VideoAutoSlideFrame {
                    candidate_index: 20,
                    file_name: "frame_000020.png".to_string(),
                    signature: signature.clone(),
                    sharpness_score: Some(60),
                },
                VideoAutoSlideFrame {
                    candidate_index: 21,
                    file_name: "frame_000021.png".to_string(),
                    signature: signature.clone(),
                    sharpness_score: Some(60),
                },
                VideoAutoSlideFrame {
                    candidate_index: 22,
                    file_name: "frame_000022.png".to_string(),
                    signature,
                    sharpness_score: Some(60),
                },
            ],
        };
        let (selected, selection_rule) = video_select_auto_slide_cluster_frame(&midpoint_cluster);
        assert_eq!(selected.candidate_index, 21);
        assert_eq!(selection_rule, "middle_frame_of_stable_visual_segment");
    }

    #[test]
    fn auto_selects_sparse_text_build_states_in_public_course_style_slides() {
        let document = test_document();
        let output_root = std::env::temp_dir().join(format!(
            "aidp-v3-video-auto-sparse-text-build-test-{}",
            DocumentId::new()
        ));
        let session_dir = output_root.join(format!("video-extraction-{}", document.id));
        let artifacts_dir = session_dir.join(DEFAULT_GENERATED_ARTIFACTS_DIR_NAME);
        let raw_frames_dir = session_dir.join(DEFAULT_RAW_FRAMES_DIR_NAME);
        fs::create_dir_all(&raw_frames_dir).expect("raw frames dir");
        fs::create_dir_all(&artifacts_dir).expect("artifacts dir");

        for frame_index in 1..=3 {
            write_test_sparse_text_build_slide_png(
                &raw_frames_dir.join(format!("frame_{frame_index:06}.png")),
                0,
            );
        }
        for frame_index in 4..=6 {
            write_test_sparse_text_build_slide_png(
                &raw_frames_dir.join(format!("frame_{frame_index:06}.png")),
                1,
            );
        }
        for frame_index in 7..=9 {
            write_test_sparse_text_build_slide_png(
                &raw_frames_dir.join(format!("frame_{frame_index:06}.png")),
                2,
            );
        }

        let frame_extraction = json!({
            "status": "completed",
            "raw_frames_dir": raw_frames_dir.display().to_string(),
            "frame_count": 9,
            "interval_seconds": 0.15,
            "manifest_file_name": DEFAULT_FRAME_MANIFEST_FILE_NAME
        });

        let manifest =
            write_video_extraction_text_artifacts(&document, &[], &frame_extraction, &output_root)
                .expect("candidate artifacts");
        let files = manifest["files"].as_array().expect("files");

        let candidate_manifest_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("slide_image_candidates"))
            .and_then(|file| file["path"].as_str())
            .expect("candidate manifest path");
        let candidate_manifest: Value = serde_json::from_str(
            &fs::read_to_string(candidate_manifest_path).expect("candidate manifest"),
        )
        .expect("candidate manifest json");
        assert_eq!(candidate_manifest["status"], json!("auto_selected"));
        assert_eq!(
            candidate_manifest["auto_selection"]["cluster_policy"]["visual_signature"],
            json!("luma_32x32")
        );
        assert_eq!(
            candidate_manifest["selected_candidate_indices"],
            json!([2, 5, 8])
        );

        let selected_slides_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("selected_slides_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("selected slides manifest path");
        let selected_slides: Value = serde_json::from_str(
            &fs::read_to_string(selected_slides_path).expect("selected slides manifest"),
        )
        .expect("selected slides manifest json");
        assert_eq!(selected_slides["selected_count"], json!(3));
        assert_eq!(
            selected_slides["selected_candidate_indices"],
            json!([2, 5, 8])
        );
        assert_eq!(
            selected_slides["requested_selected_candidate_indices"],
            json!([2, 5, 8])
        );
    }

    #[test]
    fn auto_selects_slides_without_dark_stable_transition_segments() {
        let document = test_document();
        let output_root = std::env::temp_dir().join(format!(
            "aidp-v3-video-auto-dark-transition-guard-test-{}",
            DocumentId::new()
        ));
        let session_dir = output_root.join(format!("video-extraction-{}", document.id));
        let artifacts_dir = session_dir.join(DEFAULT_GENERATED_ARTIFACTS_DIR_NAME);
        let raw_frames_dir = session_dir.join(DEFAULT_RAW_FRAMES_DIR_NAME);
        fs::create_dir_all(&raw_frames_dir).expect("raw frames dir");
        fs::create_dir_all(&artifacts_dir).expect("artifacts dir");

        for frame_index in 1..=3 {
            write_test_solid_frame_png(
                &raw_frames_dir.join(format!("frame_{frame_index:06}.png")),
                [6, 6, 6],
            );
        }
        for frame_index in 4..=6 {
            write_test_visual_slide_png(
                &raw_frames_dir.join(format!("frame_{frame_index:06}.png")),
                [16, 16, 16],
                [242, 242, 242],
                20..80,
                10..60,
            );
        }

        let frame_extraction = json!({
            "status": "completed",
            "raw_frames_dir": raw_frames_dir.display().to_string(),
            "frame_count": 6,
            "interval_seconds": 0.2,
            "manifest_file_name": DEFAULT_FRAME_MANIFEST_FILE_NAME
        });

        let manifest =
            write_video_extraction_text_artifacts(&document, &[], &frame_extraction, &output_root)
                .expect("candidate artifacts");
        let files = manifest["files"].as_array().expect("files");

        let candidate_manifest_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("slide_image_candidates"))
            .and_then(|file| file["path"].as_str())
            .expect("candidate manifest path");
        let candidate_manifest: Value = serde_json::from_str(
            &fs::read_to_string(candidate_manifest_path).expect("candidate manifest"),
        )
        .expect("candidate manifest json");
        assert_eq!(candidate_manifest["selected_candidate_indices"], json!([5]));
        assert_eq!(
            candidate_manifest["auto_selection"]["selected_clusters"][0]
                ["selected_candidate_index"],
            json!(5)
        );
        assert_eq!(
            candidate_manifest["auto_selection"]["rejected_clusters"][0]["reason"],
            json!("dark_low_information_stable_segment")
        );
        assert_eq!(
            candidate_manifest["auto_selection"]["rejected_clusters"][0]
                ["selected_candidate_index"],
            json!(2)
        );

        let selected_slides_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("selected_slides_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("selected slides manifest path");
        let selected_slides: Value = serde_json::from_str(
            &fs::read_to_string(selected_slides_path).expect("selected slides manifest"),
        )
        .expect("selected slides manifest json");
        assert_eq!(selected_slides["selected_count"], json!(1));
        assert_eq!(selected_slides["selected_candidate_indices"], json!([5]));
    }

    #[test]
    fn auto_selects_slides_without_bright_stable_transition_segments() {
        let document = test_document();
        let output_root = std::env::temp_dir().join(format!(
            "aidp-v3-video-auto-bright-transition-guard-test-{}",
            DocumentId::new()
        ));
        let session_dir = output_root.join(format!("video-extraction-{}", document.id));
        let artifacts_dir = session_dir.join(DEFAULT_GENERATED_ARTIFACTS_DIR_NAME);
        let raw_frames_dir = session_dir.join(DEFAULT_RAW_FRAMES_DIR_NAME);
        fs::create_dir_all(&raw_frames_dir).expect("raw frames dir");
        fs::create_dir_all(&artifacts_dir).expect("artifacts dir");

        for frame_index in 1..=3 {
            write_test_solid_frame_png(
                &raw_frames_dir.join(format!("frame_{frame_index:06}.png")),
                [246, 246, 246],
            );
        }
        for frame_index in 4..=6 {
            write_test_visual_slide_png(
                &raw_frames_dir.join(format!("frame_{frame_index:06}.png")),
                [24, 24, 24],
                [232, 232, 232],
                18..84,
                12..66,
            );
        }

        let frame_extraction = json!({
            "status": "completed",
            "raw_frames_dir": raw_frames_dir.display().to_string(),
            "frame_count": 6,
            "interval_seconds": 0.2,
            "manifest_file_name": DEFAULT_FRAME_MANIFEST_FILE_NAME
        });

        let manifest =
            write_video_extraction_text_artifacts(&document, &[], &frame_extraction, &output_root)
                .expect("candidate artifacts");
        let files = manifest["files"].as_array().expect("files");

        let candidate_manifest_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("slide_image_candidates"))
            .and_then(|file| file["path"].as_str())
            .expect("candidate manifest path");
        let candidate_manifest: Value = serde_json::from_str(
            &fs::read_to_string(candidate_manifest_path).expect("candidate manifest"),
        )
        .expect("candidate manifest json");
        assert_eq!(candidate_manifest["selected_candidate_indices"], json!([5]));
        assert_eq!(
            candidate_manifest["auto_selection"]["selected_clusters"][0]
                ["selected_candidate_index"],
            json!(5)
        );
        assert_eq!(
            candidate_manifest["auto_selection"]["rejected_clusters"][0]["reason"],
            json!("bright_low_information_stable_segment")
        );
        assert_eq!(
            candidate_manifest["auto_selection"]["rejected_clusters"][0]
                ["selected_candidate_index"],
            json!(2)
        );

        let selected_slides_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("selected_slides_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("selected slides manifest path");
        let selected_slides: Value = serde_json::from_str(
            &fs::read_to_string(selected_slides_path).expect("selected slides manifest"),
        )
        .expect("selected slides manifest json");
        assert_eq!(selected_slides["selected_count"], json!(1));
        assert_eq!(selected_slides["selected_candidate_indices"], json!([5]));
    }

    #[test]
    fn auto_selects_slides_without_flat_stable_loading_segments() {
        let document = test_document();
        let output_root = std::env::temp_dir().join(format!(
            "aidp-v3-video-auto-flat-transition-guard-test-{}",
            DocumentId::new()
        ));
        let session_dir = output_root.join(format!("video-extraction-{}", document.id));
        let artifacts_dir = session_dir.join(DEFAULT_GENERATED_ARTIFACTS_DIR_NAME);
        let raw_frames_dir = session_dir.join(DEFAULT_RAW_FRAMES_DIR_NAME);
        fs::create_dir_all(&raw_frames_dir).expect("raw frames dir");
        fs::create_dir_all(&artifacts_dir).expect("artifacts dir");

        for frame_index in 1..=3 {
            write_test_solid_frame_png(
                &raw_frames_dir.join(format!("frame_{frame_index:06}.png")),
                [126, 126, 126],
            );
        }
        for frame_index in 4..=6 {
            write_test_visual_slide_png(
                &raw_frames_dir.join(format!("frame_{frame_index:06}.png")),
                [18, 18, 18],
                [238, 238, 238],
                22..86,
                14..64,
            );
        }

        let frame_extraction = json!({
            "status": "completed",
            "raw_frames_dir": raw_frames_dir.display().to_string(),
            "frame_count": 6,
            "interval_seconds": 0.2,
            "manifest_file_name": DEFAULT_FRAME_MANIFEST_FILE_NAME
        });

        let manifest =
            write_video_extraction_text_artifacts(&document, &[], &frame_extraction, &output_root)
                .expect("candidate artifacts");
        let files = manifest["files"].as_array().expect("files");

        let candidate_manifest_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("slide_image_candidates"))
            .and_then(|file| file["path"].as_str())
            .expect("candidate manifest path");
        let candidate_manifest: Value = serde_json::from_str(
            &fs::read_to_string(candidate_manifest_path).expect("candidate manifest"),
        )
        .expect("candidate manifest json");
        assert_eq!(candidate_manifest["selected_candidate_indices"], json!([5]));
        assert_eq!(
            candidate_manifest["auto_selection"]["selected_clusters"][0]
                ["selected_candidate_index"],
            json!(5)
        );
        assert_eq!(
            candidate_manifest["auto_selection"]["rejected_clusters"][0]["reason"],
            json!("flat_low_information_stable_segment")
        );
        assert_eq!(
            candidate_manifest["auto_selection"]["rejected_clusters"][0]
                ["selected_candidate_index"],
            json!(2)
        );

        let selected_slides_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("selected_slides_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("selected slides manifest path");
        let selected_slides: Value = serde_json::from_str(
            &fs::read_to_string(selected_slides_path).expect("selected slides manifest"),
        )
        .expect("selected slides manifest json");
        assert_eq!(selected_slides["selected_count"], json!(1));
        assert_eq!(selected_slides["selected_candidate_indices"], json!([5]));
    }

    #[test]
    fn auto_selects_contentful_dark_theme_slides_without_low_information_rejection() {
        let document = test_document();
        let output_root = std::env::temp_dir().join(format!(
            "aidp-v3-video-auto-dark-theme-slide-test-{}",
            DocumentId::new()
        ));
        let session_dir = output_root.join(format!("video-extraction-{}", document.id));
        let artifacts_dir = session_dir.join(DEFAULT_GENERATED_ARTIFACTS_DIR_NAME);
        let raw_frames_dir = session_dir.join(DEFAULT_RAW_FRAMES_DIR_NAME);
        fs::create_dir_all(&raw_frames_dir).expect("raw frames dir");
        fs::create_dir_all(&artifacts_dir).expect("artifacts dir");

        for frame_index in 1..=3 {
            write_test_visual_slide_png(
                &raw_frames_dir.join(format!("frame_{frame_index:06}.png")),
                [10, 12, 18],
                [92, 150, 230],
                16..88,
                12..68,
            );
        }

        let frame_extraction = json!({
            "status": "completed",
            "raw_frames_dir": raw_frames_dir.display().to_string(),
            "frame_count": 3,
            "interval_seconds": 0.2,
            "manifest_file_name": DEFAULT_FRAME_MANIFEST_FILE_NAME
        });

        let manifest =
            write_video_extraction_text_artifacts(&document, &[], &frame_extraction, &output_root)
                .expect("candidate artifacts");
        let files = manifest["files"].as_array().expect("files");

        let candidate_manifest_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("slide_image_candidates"))
            .and_then(|file| file["path"].as_str())
            .expect("candidate manifest path");
        let candidate_manifest: Value = serde_json::from_str(
            &fs::read_to_string(candidate_manifest_path).expect("candidate manifest"),
        )
        .expect("candidate manifest json");
        assert_eq!(candidate_manifest["status"], json!("auto_selected"));
        assert_eq!(candidate_manifest["selected_candidate_indices"], json!([2]));
        assert_eq!(
            candidate_manifest["auto_selection"]["selected_clusters"][0]["status"],
            json!("stable_ppt_page_segment")
        );
        assert_eq!(
            candidate_manifest["auto_selection"]["selected_clusters"][0]
                ["selected_candidate_index"],
            json!(2)
        );
        assert_eq!(
            candidate_manifest["auto_selection"]["rejected_clusters"],
            json!([])
        );

        let selected_slides_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("selected_slides_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("selected slides manifest path");
        let selected_slides: Value = serde_json::from_str(
            &fs::read_to_string(selected_slides_path).expect("selected slides manifest"),
        )
        .expect("selected slides manifest json");
        assert_eq!(selected_slides["selected_count"], json!(1));
        assert_eq!(selected_slides["selected_candidate_indices"], json!([2]));
    }

    #[test]
    fn auto_selects_stable_slides_without_short_animation_transition_frames() {
        let document = test_document();
        let output_root = std::env::temp_dir().join(format!(
            "aidp-v3-video-auto-animation-transition-test-{}",
            DocumentId::new()
        ));
        let session_dir = output_root.join(format!("video-extraction-{}", document.id));
        let artifacts_dir = session_dir.join(DEFAULT_GENERATED_ARTIFACTS_DIR_NAME);
        let raw_frames_dir = session_dir.join(DEFAULT_RAW_FRAMES_DIR_NAME);
        fs::create_dir_all(&raw_frames_dir).expect("raw frames dir");
        fs::create_dir_all(&artifacts_dir).expect("artifacts dir");

        for frame_index in 1..=3 {
            write_test_visual_slide_png(
                &raw_frames_dir.join(format!("frame_{frame_index:06}.png")),
                [8, 8, 8],
                [240, 240, 240],
                20..80,
                10..60,
            );
        }
        write_test_visual_slide_png(
            &raw_frames_dir.join("frame_000004.png"),
            [64, 74, 96],
            [205, 120, 70],
            8..56,
            18..34,
        );
        write_test_visual_slide_png(
            &raw_frames_dir.join("frame_000005.png"),
            [180, 190, 208],
            [45, 72, 120],
            48..96,
            36..62,
        );
        for frame_index in 6..=8 {
            write_test_visual_slide_png(
                &raw_frames_dir.join(format!("frame_{frame_index:06}.png")),
                [28, 28, 28],
                [225, 225, 225],
                12..88,
                16..68,
            );
        }

        let frame_extraction = json!({
            "status": "completed",
            "raw_frames_dir": raw_frames_dir.display().to_string(),
            "frame_count": 8,
            "interval_seconds": 0.2,
            "manifest_file_name": DEFAULT_FRAME_MANIFEST_FILE_NAME
        });

        let manifest =
            write_video_extraction_text_artifacts(&document, &[], &frame_extraction, &output_root)
                .expect("candidate artifacts");
        let files = manifest["files"].as_array().expect("files");

        let candidate_manifest_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("slide_image_candidates"))
            .and_then(|file| file["path"].as_str())
            .expect("candidate manifest path");
        let candidate_manifest: Value = serde_json::from_str(
            &fs::read_to_string(candidate_manifest_path).expect("candidate manifest"),
        )
        .expect("candidate manifest json");
        assert_eq!(
            candidate_manifest["selected_candidate_indices"],
            json!([2, 7])
        );
        assert_eq!(
            candidate_manifest["auto_selection"]["selected_clusters"][0]
                ["selected_candidate_index"],
            json!(2)
        );
        assert_eq!(
            candidate_manifest["auto_selection"]["selected_clusters"][1]
                ["selected_candidate_index"],
            json!(7)
        );
        let rejected_clusters = candidate_manifest["auto_selection"]["rejected_clusters"]
            .as_array()
            .expect("rejected clusters");
        assert!(rejected_clusters.iter().any(|cluster| {
            cluster["reason"] == json!("unstable_short_segment")
                && cluster["first_candidate_index"] == json!(4)
                && cluster["last_candidate_index"] == json!(4)
        }));
        assert!(rejected_clusters.iter().any(|cluster| {
            cluster["reason"] == json!("unstable_short_segment")
                && cluster["first_candidate_index"] == json!(5)
                && cluster["last_candidate_index"] == json!(5)
        }));

        let selected_slides_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("selected_slides_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("selected slides manifest path");
        let selected_slides: Value = serde_json::from_str(
            &fs::read_to_string(selected_slides_path).expect("selected slides manifest"),
        )
        .expect("selected slides manifest json");
        assert_eq!(selected_slides["selected_count"], json!(2));
        assert_eq!(selected_slides["selected_candidate_indices"], json!([2, 7]));
    }

    #[test]
    fn detects_obvious_slide_rectangle_crop_from_png_frame() {
        let document = test_document();
        let output_root = std::env::temp_dir().join(format!(
            "aidp-v3-video-detected-rectangle-test-{}",
            DocumentId::new()
        ));
        let session_dir = output_root.join(format!("video-extraction-{}", document.id));
        let artifacts_dir = session_dir.join(DEFAULT_GENERATED_ARTIFACTS_DIR_NAME);
        let raw_frames_dir = session_dir.join(DEFAULT_RAW_FRAMES_DIR_NAME);
        fs::create_dir_all(&raw_frames_dir).expect("raw frames dir");
        fs::create_dir_all(&artifacts_dir).expect("artifacts dir");
        write_test_slide_rectangle_png(&raw_frames_dir.join("frame_000001.png"));
        fs::write(
            artifacts_dir.join(DEFAULT_PPT_KEEP_LIST_TEMPLATE_FILE_NAME),
            serde_json::to_vec_pretty(&json!({
                "status": "selected",
                "selected_candidate_indices": [1]
            }))
            .expect("keep list bytes"),
        )
        .expect("keep list");
        let frame_extraction = json!({
            "status": "completed",
            "raw_frames_dir": raw_frames_dir.display().to_string(),
            "frame_count": 1,
            "manifest_file_name": DEFAULT_FRAME_MANIFEST_FILE_NAME
        });

        let manifest =
            write_video_extraction_text_artifacts(&document, &[], &frame_extraction, &output_root)
                .expect("candidate artifacts");

        let files = manifest["files"].as_array().expect("files");
        let slide_rectangles_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("slide_rectangles_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("slide rectangles manifest path");
        let slide_rectangles: Value = serde_json::from_str(
            &fs::read_to_string(slide_rectangles_path).expect("slide rectangles manifest"),
        )
        .expect("slide rectangles manifest json");
        assert_eq!(
            slide_rectangles["rectangle_extraction_status"],
            json!("promoted_detector_crop")
        );
        assert_eq!(
            slide_rectangles["rectangle_extraction_mode"],
            json!("border_background_contrast_v2")
        );
        let crop_box = &slide_rectangles["rectangles"][0]["crop_box"];
        assert_eq!(crop_box["unit"], json!("relative"));
        assert_eq!(crop_box["x"], json!(0.2));
        assert_eq!(crop_box["y"], json!(0.125));
        assert_eq!(crop_box["width"], json!(0.6));
        assert_eq!(crop_box["height"], json!(0.625));
        assert_eq!(
            slide_rectangles["rectangles"][0]["rectangle_source"],
            json!("raw_frame_border_background_contrast")
        );
        assert_eq!(
            slide_rectangles["rectangles"][0]["detector"]["background_source"],
            json!("border_median_rgb")
        );
        assert_eq!(
            slide_rectangles["rectangles"][0]["review_required"],
            json!(true)
        );
        let selected_slides_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("selected_slides_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("selected slides manifest path");
        let selected_slides: Value = serde_json::from_str(
            &fs::read_to_string(selected_slides_path).expect("selected slides manifest"),
        )
        .expect("selected slides manifest json");
        assert_eq!(
            selected_slides["rectangle_extraction_status"],
            json!("promoted_detector_crop")
        );
        let pptx_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("pptx"))
            .and_then(|file| file["path"].as_str())
            .expect("pptx path");
        let mut archive =
            ZipArchive::new(File::open(pptx_path).expect("pptx file")).expect("pptx zip");
        let mut slide_xml = String::new();
        archive
            .by_name("ppt/slides/slide1.xml")
            .expect("slide xml")
            .read_to_string(&mut slide_xml)
            .expect("slide xml text");
        assert!(slide_xml.contains(r#"<a:srcRect l="20000" r="20000" t="12500" b="25000"/>"#));
        assert!(slide_xml.contains(r#"descr="Slide 1; candidate 1; source frame frame_000001.png; timestamp 0:00; crop promoted_detector_crop/border_background_contrast_v2; transcript segments 0; paths redacted""#));
        assert!(!slide_xml.contains(&raw_frames_dir.display().to_string()));
    }

    #[test]
    fn detects_slide_rectangle_when_top_left_background_is_noisy() {
        let output_root = std::env::temp_dir().join(format!(
            "aidp-v3-video-noisy-background-test-{}",
            DocumentId::new()
        ));
        fs::create_dir_all(&output_root).expect("output root");
        let frame_path = output_root.join("frame_000001.png");
        write_test_noisy_top_left_slide_rectangle_png(&frame_path);

        let detection =
            video_detect_slide_rectangle(&frame_path).expect("border-median detector crop");

        assert_eq!(detection.x, 0.2);
        assert_eq!(detection.y, 0.125);
        assert_eq!(detection.width, 0.6);
        assert_eq!(detection.height, 0.625);
        assert_eq!(detection.signal_source, "border_median_rgb");
        assert!(detection.sample_count > 0);
    }

    #[test]
    fn detects_slide_rectangle_ignoring_external_foreground_component() {
        let output_root = std::env::temp_dir().join(format!(
            "aidp-v3-video-foreground-component-test-{}",
            DocumentId::new()
        ));
        fs::create_dir_all(&output_root).expect("output root");
        let frame_path = output_root.join("frame_000001.png");
        write_test_slide_with_external_foreground_png(&frame_path);

        let detection =
            video_detect_slide_rectangle(&frame_path).expect("foreground component detector crop");

        assert_eq!(detection.detector_name, "foreground_component_v1");
        assert_eq!(detection.rectangle_source, "raw_frame_foreground_component");
        assert_eq!(
            detection.rectangle_extraction_mode,
            "foreground_component_v1"
        );
        assert_eq!(detection.signal_source, "border_median_component");
        assert_eq!(detection.x, 0.2);
        assert_eq!(detection.y, 0.125);
        assert_eq!(detection.width, 0.6);
        assert_eq!(detection.height, 0.625);

        let rectangle =
            video_slide_rectangle_from_frame(1, 1, "frame_000001.png", 0.0, &frame_path);
        assert_eq!(
            rectangle["rectangle_source"],
            json!("raw_frame_foreground_component")
        );
        assert_eq!(
            rectangle["rectangle_extraction_mode"],
            json!("foreground_component_v1")
        );
        assert_eq!(
            rectangle["detector"]["name"],
            json!("foreground_component_v1")
        );
    }

    #[test]
    fn writes_foreground_component_crop_for_speaker_window_obstruction() {
        let document = test_document();
        let output_root = std::env::temp_dir().join(format!(
            "aidp-v3-video-speaker-window-crop-test-{}",
            DocumentId::new()
        ));
        let session_dir = output_root.join(format!("video-extraction-{}", document.id));
        let artifacts_dir = session_dir.join(DEFAULT_GENERATED_ARTIFACTS_DIR_NAME);
        let raw_frames_dir = session_dir.join(DEFAULT_RAW_FRAMES_DIR_NAME);
        fs::create_dir_all(&raw_frames_dir).expect("raw frames dir");
        fs::create_dir_all(&artifacts_dir).expect("artifacts dir");
        write_test_slide_with_external_foreground_png(&raw_frames_dir.join("frame_000001.png"));
        fs::write(
            artifacts_dir.join(DEFAULT_PPT_KEEP_LIST_TEMPLATE_FILE_NAME),
            serde_json::to_vec_pretty(&json!({
                "status": "selected",
                "selected_candidate_indices": [1]
            }))
            .expect("keep list bytes"),
        )
        .expect("keep list");
        let frame_extraction = json!({
            "status": "completed",
            "raw_frames_dir": raw_frames_dir.display().to_string(),
            "frame_count": 1,
            "manifest_file_name": DEFAULT_FRAME_MANIFEST_FILE_NAME
        });

        let manifest =
            write_video_extraction_text_artifacts(&document, &[], &frame_extraction, &output_root)
                .expect("candidate artifacts");
        let files = manifest["files"].as_array().expect("files");

        let slide_rectangles_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("slide_rectangles_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("slide rectangles manifest path");
        let slide_rectangles: Value = serde_json::from_str(
            &fs::read_to_string(slide_rectangles_path).expect("slide rectangles manifest"),
        )
        .expect("slide rectangles manifest json");
        assert_eq!(
            slide_rectangles["rectangle_extraction_status"],
            json!("promoted_detector_crop")
        );
        assert_eq!(
            slide_rectangles["rectangle_extraction_mode"],
            json!("foreground_component_v1")
        );
        assert_eq!(slide_rectangles["promoted_rectangle_count"], json!(1));
        assert_eq!(
            slide_rectangles["rectangles"][0]["rectangle_source"],
            json!("raw_frame_foreground_component")
        );
        assert_eq!(
            slide_rectangles["rectangles"][0]["detector"]["name"],
            json!("foreground_component_v1")
        );
        let crop_box = &slide_rectangles["rectangles"][0]["crop_box"];
        assert_eq!(crop_box["x"], json!(0.2));
        assert_eq!(crop_box["y"], json!(0.125));
        assert_eq!(crop_box["width"], json!(0.6));
        assert_eq!(crop_box["height"], json!(0.625));

        let selected_slides_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("selected_slides_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("selected slides manifest path");
        let selected_slides: Value = serde_json::from_str(
            &fs::read_to_string(selected_slides_path).expect("selected slides manifest"),
        )
        .expect("selected slides manifest json");
        assert_eq!(
            selected_slides["rectangle_extraction_mode"],
            json!("foreground_component_v1")
        );
        assert_eq!(selected_slides["selected_count"], json!(1));

        let slide_quality_report_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("slide_quality_report"))
            .and_then(|file| file["path"].as_str())
            .expect("slide quality report path");
        let slide_quality_report =
            fs::read_to_string(slide_quality_report_path).expect("slide quality report");
        assert!(!slide_quality_report.contains(&raw_frames_dir.display().to_string()));
        let slide_quality_report_json: Value =
            serde_json::from_str(&slide_quality_report).expect("quality report json");
        assert_eq!(
            slide_quality_report_json["summary"]["detector_crop_count"],
            json!(1)
        );
        assert_eq!(
            slide_quality_report_json["summary"]["full_frame_fallback_count"],
            json!(0)
        );
        assert_eq!(
            slide_quality_report_json["slides"][0]["rectangle_extraction_mode"],
            json!("foreground_component_v1")
        );
        assert_eq!(
            slide_quality_report_json["slides"][0]["crop_risk"],
            json!("medium")
        );

        let pptx_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("pptx"))
            .and_then(|file| file["path"].as_str())
            .expect("pptx path");
        let mut archive =
            ZipArchive::new(File::open(pptx_path).expect("pptx file")).expect("pptx zip");
        let mut slide_xml = String::new();
        archive
            .by_name("ppt/slides/slide1.xml")
            .expect("slide xml")
            .read_to_string(&mut slide_xml)
            .expect("slide xml text");
        assert!(slide_xml.contains(r#"<a:srcRect l="20000" r="20000" t="12500" b="25000"/>"#));
        assert!(slide_xml.contains("crop promoted_detector_crop/foreground_component_v1"));
        assert!(!slide_xml.contains(&raw_frames_dir.display().to_string()));
    }

    #[test]
    fn detects_slide_rectangle_from_edges_when_background_is_not_uniform() {
        let output_root = std::env::temp_dir().join(format!(
            "aidp-v3-video-edge-projection-test-{}",
            DocumentId::new()
        ));
        fs::create_dir_all(&output_root).expect("output root");
        let frame_path = output_root.join("frame_000001.png");
        write_test_gradient_edge_slide_rectangle_png(&frame_path);

        let detection =
            video_detect_slide_rectangle(&frame_path).expect("edge projection detector crop");

        assert_eq!(detection.detector_name, "edge_projection_v1");
        assert_eq!(detection.rectangle_source, "raw_frame_edge_projection");
        assert_eq!(detection.rectangle_extraction_mode, "edge_projection_v1");
        assert!(detection.x >= 0.19 && detection.x <= 0.21);
        assert!(detection.y >= 0.12 && detection.y <= 0.14);
        assert!(detection.width >= 0.59 && detection.width <= 0.62);
        assert!(detection.height >= 0.62 && detection.height <= 0.65);
        assert!(detection.signal_pixel_count > 0);
        assert!(detection.sample_count >= 4);

        let rectangle =
            video_slide_rectangle_from_frame(1, 1, "frame_000001.png", 0.0, &frame_path);
        assert_eq!(
            rectangle["rectangle_source"],
            json!("raw_frame_edge_projection")
        );
        assert_eq!(
            rectangle["rectangle_extraction_mode"],
            json!("edge_projection_v1")
        );
        assert_eq!(rectangle["detector"]["name"], json!("edge_projection_v1"));
    }

    #[test]
    fn detects_slide_rectangle_from_low_contrast_bright_canvas() {
        let output_root = std::env::temp_dir().join(format!(
            "aidp-v3-video-bright-canvas-test-{}",
            DocumentId::new()
        ));
        fs::create_dir_all(&output_root).expect("output root");
        let frame_path = output_root.join("frame_000001.png");
        write_test_low_contrast_bright_canvas_slide_png(&frame_path);

        let detection = video_detect_slide_rectangle(&frame_path).expect("bright canvas crop");

        assert_eq!(detection.detector_name, "bright_canvas_v1");
        assert_eq!(detection.rectangle_source, "raw_frame_bright_canvas");
        assert_eq!(detection.rectangle_extraction_mode, "bright_canvas_v1");
        assert_eq!(detection.signal_source, "bright_canvas_luma");
        assert!(detection.x >= 0.21 && detection.x <= 0.22);
        assert_eq!(detection.y, 0.2);
        assert_eq!(detection.width, 0.65);
        assert_eq!(detection.height, 0.6);

        let rectangle =
            video_slide_rectangle_from_frame(1, 1, "frame_000001.png", 0.0, &frame_path);
        assert_eq!(
            rectangle["rectangle_source"],
            json!("raw_frame_bright_canvas")
        );
        assert_eq!(
            rectangle["rectangle_extraction_mode"],
            json!("bright_canvas_v1")
        );
        assert_eq!(rectangle["detector"]["name"], json!("bright_canvas_v1"));
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
                "keyframe_ocr_snippets": [{
                    "timestamp_seconds": 0.8,
                    "text": "OCR title for selected slide",
                    "ocr_confidence": 0.91
                }]
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
        let selected_slides_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("selected_slides_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("selected slides manifest path");
        let selected_slides = fs::read_to_string(selected_slides_path).expect("selected slides");
        assert!(selected_slides.contains("window_mapped"));
        assert!(selected_slides.contains("OCR title for selected slide"));
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
        assert!(slide_notes.contains("Aligned OCR snippets"));
        assert!(slide_notes.contains("OCR title for selected slide"));
        let slide_quality_report_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("slide_quality_report"))
            .and_then(|file| file["path"].as_str())
            .expect("slide quality report path");
        let slide_quality_report =
            fs::read_to_string(slide_quality_report_path).expect("slide quality report");
        assert!(slide_quality_report.contains("\"ocr_snippet_count\": 1"));
        assert!(slide_quality_report.contains("\"ocr_risk\": \"low\""));
        let video_slides_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("video_slides_markdown"))
            .and_then(|file| file["path"].as_str())
            .expect("video slides path");
        let video_slides = fs::read_to_string(video_slides_path).expect("video slides");
        assert!(video_slides.contains("OCR evidence"));
        assert!(video_slides.contains("OCR title for selected slide"));
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
    fn writes_ocr_notes_without_subtitle_page_map_when_transcript_missing() {
        let document = test_document();
        let output_root = std::env::temp_dir().join(format!(
            "aidp-v3-video-ocr-only-notes-test-{}",
            DocumentId::new()
        ));
        let session_dir = output_root.join(format!("video-extraction-{}", document.id));
        let artifacts_dir = session_dir.join(DEFAULT_GENERATED_ARTIFACTS_DIR_NAME);
        let raw_frames_dir = session_dir.join(DEFAULT_RAW_FRAMES_DIR_NAME);
        fs::create_dir_all(&raw_frames_dir).expect("raw frames dir");
        fs::create_dir_all(&artifacts_dir).expect("artifacts dir");
        fs::write(raw_frames_dir.join("frame_000001.jpg"), b"fake").expect("frame 1");
        fs::write(raw_frames_dir.join("frame_000002.jpg"), b"fake").expect("frame 2");
        fs::write(
            artifacts_dir.join(DEFAULT_PPT_KEEP_LIST_TEMPLATE_FILE_NAME),
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
                "transcript_segments": [],
                "scenes": [],
                "keyframe_ocr_snippets": [{
                    "timestamp_seconds": 0.8,
                    "text": "OCR-only title for selected slide",
                    "ocr_confidence": 0.93
                }, {
                    "timestamp_seconds": 0.9,
                    "text": "https://private.example/video?token=secret",
                    "ocr_confidence": 0.88
                }]
            }
        }));

        let manifest = write_video_extraction_text_artifacts(
            &document,
            &[chunk],
            &frame_extraction,
            &output_root,
        )
        .expect("ocr-only notes artifacts");

        let files = manifest["files"].as_array().expect("files");
        assert!(!files
            .iter()
            .any(|file| file["artifact_kind"] == json!("subtitle_page_map")));
        let selected_slides_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("selected_slides_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("selected slides manifest path");
        let selected_slides = fs::read_to_string(selected_slides_path).expect("selected slides");
        assert!(selected_slides.contains("missing_transcript"));
        assert!(selected_slides.contains("window_mapped"));
        assert!(selected_slides.contains("OCR-only title for selected slide"));
        assert!(selected_slides.contains("[redacted]"));
        assert!(!selected_slides.contains("token=secret"));
        let slide_notes_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("slide_notes"))
            .and_then(|file| file["path"].as_str())
            .expect("slide notes path");
        let slide_notes = fs::read_to_string(slide_notes_path).expect("slide notes");
        assert!(slide_notes.contains("no aligned transcript segment"));
        assert!(slide_notes.contains("Aligned OCR snippets"));
        assert!(slide_notes.contains("OCR-only title for selected slide"));
        assert!(slide_notes.contains("[redacted]"));
        assert!(!slide_notes.contains("token=secret"));
        let video_slides_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("video_slides_markdown"))
            .and_then(|file| file["path"].as_str())
            .expect("video slides path");
        let video_slides = fs::read_to_string(video_slides_path).expect("video slides");
        assert!(video_slides.contains("no aligned transcript segment"));
        assert!(video_slides.contains("OCR evidence"));
        assert!(video_slides.contains("OCR-only title for selected slide"));
        assert!(video_slides.contains("[redacted]"));
        assert!(!video_slides.contains("token=secret"));
        let slide_quality_report_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("slide_quality_report"))
            .and_then(|file| file["path"].as_str())
            .expect("slide quality report path");
        let slide_quality_report =
            fs::read_to_string(slide_quality_report_path).expect("slide quality report");
        assert!(slide_quality_report.contains("\"subtitle_missing_count\": 1"));
        assert!(slide_quality_report.contains("\"ocr_mapped_count\": 1"));
        assert!(slide_quality_report.contains("\"ocr_snippet_count\": 2"));
        assert!(slide_quality_report.contains("\"ocr_risk\": \"low\""));
        assert!(slide_quality_report.contains("missing_transcript_alignment"));
        assert!(!slide_quality_report.contains("missing_ocr_evidence"));
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
        assert!(notes.contains("Transcript/subtitle alignment is not yet verified"));
        assert!(notes.contains("OCR evidence"));
        assert!(notes.contains("OCR-only title for selected slide"));
        assert!(notes.contains("[redacted]"));
        assert!(!notes.contains("token=secret"));
        let final_manifest_path = files
            .iter()
            .find(|file| file["artifact_kind"] == json!("final_deliverables_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("final deliverables manifest path");
        let final_manifest: Value = serde_json::from_slice(
            &fs::read(final_manifest_path).expect("final deliverables manifest"),
        )
        .expect("final deliverables manifest json");
        assert_eq!(
            final_manifest["deliverable_status"]["has_subtitle_page_map"],
            json!(false)
        );
        assert!(!final_manifest["evidence_outputs"]
            .as_array()
            .expect("evidence outputs")
            .iter()
            .any(|file| file["artifact_kind"] == json!("subtitle_page_map")));
        assert!(final_manifest["deliverable_status"]["warnings"]
            .as_array()
            .expect("warnings")
            .iter()
            .any(|warning| warning["code"] == json!("speaker_notes_metadata_only")));
    }

    #[test]
    fn controlled_video_sample_deliverable_contract_is_complete() {
        let document = test_document();
        let output_root =
            std::env::temp_dir().join(format!("aidp-v3-video-contract-test-{}", DocumentId::new()));
        let session_dir = output_root.join(format!("video-extraction-{}", document.id));
        let artifacts_dir = session_dir.join(DEFAULT_GENERATED_ARTIFACTS_DIR_NAME);
        let raw_frames_dir = session_dir.join(DEFAULT_RAW_FRAMES_DIR_NAME);
        fs::create_dir_all(&raw_frames_dir).expect("raw frames dir");
        fs::create_dir_all(&artifacts_dir).expect("artifacts dir");
        fs::write(raw_frames_dir.join("frame_000001.jpg"), b"fake").expect("frame 1");
        fs::write(raw_frames_dir.join("frame_000002.jpg"), b"fake").expect("frame 2");
        fs::write(
            artifacts_dir.join(DEFAULT_PPT_KEEP_LIST_TEMPLATE_FILE_NAME),
            serde_json::to_vec_pretty(&json!({
                "status": "selected",
                "selected_candidate_indices": [2],
                "selection_notes": ["controlled contract sample"]
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
                    "text": "Narration for the selected contract slide"
                }],
                "scenes": [{
                    "start_seconds": 0.0,
                    "end_seconds": 1.0,
                    "summary": "Title slide scene"
                }],
                "keyframe_ocr_snippets": [{
                    "timestamp_seconds": 1.0,
                    "text": "Contract slide title"
                }]
            }
        }));

        let generated_artifacts = write_video_extraction_text_artifacts(
            &document,
            std::slice::from_ref(&chunk),
            &frame_extraction,
            &output_root,
        )
        .expect("controlled deliverables");
        let output = extract_video_ppt_output_with_artifacts(
            &document,
            &[chunk],
            frame_extraction,
            generated_artifacts,
        );
        let run_id = "00000000-0000-0000-0000-000000000001";
        let html_artifact =
            video_extraction_html_artifact_from_output(run_id, Some("local-thread-1"), &output)
                .expect("html artifact");
        let output_artifact = video_extraction_output_artifact_from_output(
            run_id,
            Some("local-thread-1"),
            &output,
            std::slice::from_ref(&html_artifact),
        )
        .expect("output artifact");

        assert_eq!(
            output_artifact["deliverable_status"]["state"],
            json!("final_pptx_ready")
        );
        assert!(output_artifact["manifest_outputs"]
            .as_array()
            .expect("manifest outputs")
            .iter()
            .any(|file| file["artifact_kind"] == json!("final_deliverables_manifest")));
        assert!(output_artifact["manifest_outputs"]
            .as_array()
            .expect("manifest outputs")
            .iter()
            .any(|file| file["artifact_kind"] == json!("published_deliverable_manifest")));
        assert!(output_artifact["manifest_outputs"]
            .as_array()
            .expect("manifest outputs")
            .iter()
            .any(|file| file["artifact_kind"] == json!("published_version_history")));
        assert!(output_artifact["manifest_outputs"]
            .as_array()
            .expect("manifest outputs")
            .iter()
            .any(|file| file["artifact_kind"] == json!("extraction_artifacts_manifest")));
        assert!(output_artifact["final_outputs"]
            .as_array()
            .expect("final outputs")
            .iter()
            .any(|file| file["artifact_kind"] == json!("pptx")));
        assert!(output_artifact["review_outputs"]
            .as_array()
            .expect("review outputs")
            .iter()
            .any(|file| file["artifact_kind"] == json!("slide_notes")));
        assert!(output_artifact["evidence_outputs"]
            .as_array()
            .expect("evidence outputs")
            .iter()
            .any(|file| file["artifact_kind"] == json!("subtitle_page_map")));
        assert!(output_artifact["completion_follow_up"]["next_actions"]
            .as_array()
            .expect("next actions")
            .contains(&json!("download_pptx")));
        assert!(output_artifact["completion_follow_up"]["next_actions"]
            .as_array()
            .expect("next actions")
            .contains(&json!("review_slide_notes")));
        assert!(output_artifact["completion_follow_up"]["next_actions"]
            .as_array()
            .expect("next actions")
            .contains(&json!("review_slide_rectangles_manifest")));
        assert_eq!(
            html_artifact["payload"]["completion_follow_up"]["html_artifact_ids"][0],
            html_artifact["id"]
        );
        let audit_counts = &output_artifact["completion_audit"]["artifact_group_counts"];
        assert_eq!(audit_counts["manifest_outputs"], json!(4));
        assert_eq!(audit_counts["final_outputs"], json!(2));
        assert!(audit_counts["review_outputs"].as_u64().unwrap_or(0) >= 1);
        assert!(audit_counts["evidence_outputs"].as_u64().unwrap_or(0) >= 1);
        assert_eq!(
            output_artifact["completion_audit"]["redaction"]["provider_keys_included"],
            json!(false)
        );

        let generated_files = output["generated_artifacts"]["files"]
            .as_array()
            .expect("generated files");
        let final_manifest_path = generated_files
            .iter()
            .find(|file| file["artifact_kind"] == json!("final_deliverables_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("final deliverables manifest path");
        let final_manifest_json: Value = serde_json::from_str(
            &fs::read_to_string(final_manifest_path).expect("final deliverables manifest"),
        )
        .expect("final deliverables manifest json");
        assert_public_manifest_file_entry(
            final_manifest_json["manifest_outputs"]
                .as_array()
                .expect("final manifest outputs"),
            "final_deliverables_manifest",
            DEFAULT_FINAL_DELIVERABLES_MANIFEST_FILE_NAME,
        );
        assert_public_manifest_file_entry(
            final_manifest_json["manifest_outputs"]
                .as_array()
                .expect("final manifest outputs"),
            "published_deliverable_manifest",
            DEFAULT_PUBLISHED_DELIVERABLE_MANIFEST_FILE_NAME,
        );
        assert_public_manifest_file_entry(
            final_manifest_json["manifest_outputs"]
                .as_array()
                .expect("final manifest outputs"),
            "published_version_history",
            DEFAULT_PUBLISHED_VERSION_HISTORY_FILE_NAME,
        );
        assert_public_manifest_file_entry(
            final_manifest_json["manifest_outputs"]
                .as_array()
                .expect("final manifest outputs"),
            "extraction_artifacts_manifest",
            DEFAULT_EXTRACTION_ARTIFACTS_MANIFEST_FILE_NAME,
        );
        assert_public_manifest_file_entry(
            final_manifest_json["final_outputs"]
                .as_array()
                .expect("final outputs"),
            "pptx",
            DEFAULT_VIDEO_SLIDES_PPTX_FILE_NAME,
        );
        assert_public_manifest_file_entry(
            final_manifest_json["final_outputs"]
                .as_array()
                .expect("final outputs"),
            "video_slides_markdown",
            DEFAULT_VIDEO_SLIDES_MARKDOWN_FILE_NAME,
        );
        assert_public_manifest_file_entry(
            final_manifest_json["review_outputs"]
                .as_array()
                .expect("review outputs"),
            "slide_rectangles_manifest",
            DEFAULT_SLIDE_RECTANGLES_MANIFEST_FILE_NAME,
        );
        assert_public_manifest_file_entry(
            final_manifest_json["review_outputs"]
                .as_array()
                .expect("review outputs"),
            "slide_notes",
            DEFAULT_SLIDE_NOTES_ARTIFACT_FILE_NAME,
        );
        assert_public_manifest_file_entry(
            final_manifest_json["evidence_outputs"]
                .as_array()
                .expect("evidence outputs"),
            "subtitle_page_map",
            DEFAULT_SUBTITLE_PAGE_MAP_FILE_NAME,
        );
        let published_manifest_path = generated_files
            .iter()
            .find(|file| file["artifact_kind"] == json!("published_deliverable_manifest"))
            .and_then(|file| file["path"].as_str())
            .expect("published deliverable manifest path");
        let published_manifest_json: Value = serde_json::from_str(
            &fs::read_to_string(published_manifest_path).expect("published deliverable manifest"),
        )
        .expect("published deliverable manifest json");
        assert_eq!(
            published_manifest_json["manifest_type"],
            json!("v3.video_ppt_published_deliverable.v1")
        );
        assert_eq!(
            published_manifest_json["lifecycle_state"],
            json!("published_version_ready")
        );
        assert_eq!(published_manifest_json["immutable_version"], json!(true));
        assert_eq!(published_manifest_json["version_no"], json!(1));
        assert_public_manifest_file_entry(
            published_manifest_json["published_files"]
                .as_array()
                .expect("published files"),
            "published_version_history",
            DEFAULT_PUBLISHED_VERSION_HISTORY_FILE_NAME,
        );
        assert_public_manifest_file_entry(
            published_manifest_json["published_files"]
                .as_array()
                .expect("published files"),
            "slide_rectangles_manifest",
            DEFAULT_SLIDE_RECTANGLES_MANIFEST_FILE_NAME,
        );
        assert_public_manifest_file_entry(
            published_manifest_json["published_files"]
                .as_array()
                .expect("published files"),
            "video_slides_markdown",
            DEFAULT_VIDEO_SLIDES_MARKDOWN_FILE_NAME,
        );
        assert_public_manifest_file_entry(
            published_manifest_json["published_files"]
                .as_array()
                .expect("published files"),
            "published_deliverable_manifest",
            DEFAULT_PUBLISHED_DELIVERABLE_MANIFEST_FILE_NAME,
        );
        let published_history_path = generated_files
            .iter()
            .find(|file| file["artifact_kind"] == json!("published_version_history"))
            .and_then(|file| file["path"].as_str())
            .expect("published version history path");
        let published_history_json: Value = serde_json::from_str(
            &fs::read_to_string(published_history_path).expect("published version history"),
        )
        .expect("published version history json");
        assert_eq!(
            published_history_json["manifest_type"],
            json!("v3.video_ppt_published_version_history.v1")
        );
        assert_eq!(published_history_json["status"], json!("history_ready"));
        assert_eq!(published_history_json["latest_version_no"], json!(1));
        assert_eq!(published_history_json["version_count"], json!(1));
        assert_eq!(
            published_history_json["versions"][0]["published_manifest_file_name"],
            json!(DEFAULT_PUBLISHED_DELIVERABLE_MANIFEST_FILE_NAME)
        );

        let extraction_manifest_path = output["generated_artifacts"]["manifest_path"]
            .as_str()
            .expect("extraction manifest path");
        let extraction_manifest_json: Value = serde_json::from_str(
            &fs::read_to_string(extraction_manifest_path).expect("extraction manifest"),
        )
        .expect("extraction manifest json");
        let extraction_files = extraction_manifest_json["files"]
            .as_array()
            .expect("extraction files");
        assert_public_manifest_file_entry(
            extraction_files,
            "final_deliverables_manifest",
            DEFAULT_FINAL_DELIVERABLES_MANIFEST_FILE_NAME,
        );
        assert_public_manifest_file_entry(
            extraction_files,
            "published_deliverable_manifest",
            DEFAULT_PUBLISHED_DELIVERABLE_MANIFEST_FILE_NAME,
        );
        assert_public_manifest_file_entry(
            extraction_files,
            "published_version_history",
            DEFAULT_PUBLISHED_VERSION_HISTORY_FILE_NAME,
        );
        assert_public_manifest_file_entry(
            extraction_files,
            "extraction_artifacts_manifest",
            DEFAULT_EXTRACTION_ARTIFACTS_MANIFEST_FILE_NAME,
        );
        assert_public_manifest_file_entry(
            extraction_files,
            "pptx",
            DEFAULT_VIDEO_SLIDES_PPTX_FILE_NAME,
        );
        assert_public_manifest_file_entry(
            extraction_files,
            "slide_rectangles_manifest",
            DEFAULT_SLIDE_RECTANGLES_MANIFEST_FILE_NAME,
        );
        assert_public_manifest_file_entry(
            extraction_files,
            "slide_notes",
            DEFAULT_SLIDE_NOTES_ARTIFACT_FILE_NAME,
        );
        assert_public_manifest_file_entry(
            extraction_files,
            "video_slides_markdown",
            DEFAULT_VIDEO_SLIDES_MARKDOWN_FILE_NAME,
        );
        assert_public_manifest_file_entry(
            extraction_files,
            "subtitle_page_map",
            DEFAULT_SUBTITLE_PAGE_MAP_FILE_NAME,
        );
    }

    #[test]
    fn frame_extraction_is_planned_by_default() {
        let document = test_document();
        let disabled = run_video_frame_extraction_if_enabled(
            &document,
            &FrameExtractionConfig {
                enabled: false,
                ..FrameExtractionConfig::default()
            },
        );

        assert_eq!(disabled["status"], json!("planned"));
        assert_eq!(disabled["enabled"], json!(false));
    }

    #[test]
    fn frame_extraction_accepts_public_remote_video_url_input() {
        let mut document = test_document();
        document.object_key =
            "https://cdn.example.com/course/lesson-01.mp4?token=redacted".to_string();
        document.metadata.insert(
            "remote_media".to_string(),
            json!({
                "source_url": "https://cdn.example.com/course/lesson-01.mp4?token=redacted",
                "source_type": "public_page_resolvable_video"
            }),
        );

        let input = resolve_media_frame_extraction_input(&document)
            .expect("public remote video should be accepted as ffmpeg input");

        assert_eq!(
            input,
            VideoFrameExtractionInput::RemoteUrl(
                "https://cdn.example.com/course/lesson-01.mp4?token=redacted".to_string()
            )
        );
        assert_eq!(input.manifest_input_kind(), "remote_video_url");
        assert_eq!(input.manifest_input_value(), "[redacted]");
        assert!(input.input_url_redacted());
        let sanitized = input.sanitize_process_output(
            "https://cdn.example.com/course/lesson-01.mp4?token=redacted: HTTP 403".to_string(),
        );
        assert_eq!(sanitized, "[redacted]: HTTP 403");
        assert!(video_remote_media_url_allowed(
            "https://cdn.example.com/course/lesson-01.mp4?token=redacted"
        ));
        assert_eq!(
            video_remote_media_url_extension(
                "https://cdn.example.com/course/lesson-01.mp4?token=redacted"
            ),
            Some(".mp4")
        );
        assert!(video_remote_media_response_type_allowed("video/mp4"));
        assert!(video_remote_media_response_type_allowed(
            "application/octet-stream"
        ));
        assert!(!video_remote_media_response_type_allowed("text/html"));
    }

    #[test]
    fn frame_extraction_rejects_private_or_login_gated_remote_video_url_input() {
        let mut private_document = test_document();
        private_document.object_key = "https://127.0.0.1/private/course.mp4".to_string();
        let mut wechat_document = test_document();
        wechat_document.object_key = "https://weixin.qq.com/sph/ActLMg4yTD.mp4".to_string();

        assert!(resolve_media_frame_extraction_input(&private_document).is_none());
        assert!(resolve_media_frame_extraction_input(&wechat_document).is_none());
        assert!(!video_remote_media_url_allowed(
            "https://127.0.0.1/private/course.mp4"
        ));
        assert!(!video_remote_media_url_allowed(
            "https://weixin.qq.com/sph/ActLMg4yTD.mp4"
        ));
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
