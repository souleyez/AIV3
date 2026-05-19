use document_vlm_runtime::{
    build_enriched_image_text, probe_minimax_media_capabilities_from_env,
    run_document_image_vlm_from_env, DocumentImageVlmConfig, DocumentImageVlmPayload,
    MiniMaxMediaCapabilityMatrix,
};
use domain_model::{DatasetId, DocumentId};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    collections::hash_map::DefaultHasher,
    fs,
    fs::File,
    hash::{Hash, Hasher},
    io::{Read, Write},
    net::IpAddr,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use zip::ZipArchive;

#[derive(Clone, Debug)]
pub struct IngestJob {
    pub dataset_id: DatasetId,
    pub document_id: DocumentId,
    pub title: String,
    pub object_key: String,
    pub content_type: String,
}

#[derive(Clone, Debug)]
pub struct IngestOutcome {
    pub chunks: Vec<String>,
    pub inferred_title: Option<String>,
    pub parse_method: String,
    pub extracted_chars: usize,
    pub used_placeholder: bool,
    pub metadata: Value,
}

impl IngestOutcome {
    pub fn chunk_count(&self) -> u32 {
        self.chunks.len().try_into().unwrap_or(u32::MAX)
    }

    pub fn parse_status(&self) -> String {
        if let Some(status) = value_string_at_path(&self.metadata, &["media", "parse_status"]) {
            return status;
        }

        if let Some(status) = self.parse_quality_status() {
            return match status.as_str() {
                "usable_text" => "parsed".to_string(),
                "vlm_fallback_used" => "parsed_with_vlm_fallback".to_string(),
                "low_text_coverage" | "low_text_coverage_fallback_unavailable" => {
                    "parse_degraded".to_string()
                }
                _ => status,
            };
        }

        if self.used_placeholder {
            return "placeholder".to_string();
        }
        if self.parse_method.contains("vlm") {
            return "parsed_with_vlm".to_string();
        }
        if self.parse_method.ends_with("-empty") || self.parse_method.contains("+low-quality") {
            return "parse_degraded".to_string();
        }
        "parsed".to_string()
    }

    pub fn parse_quality_status(&self) -> Option<String> {
        value_string_at_path(&self.metadata, &["parse_quality", "status"])
    }

    pub fn cloud_structured_provider(&self) -> &'static str {
        if self.parse_method.contains("vlm") {
            "minimax"
        } else {
            ""
        }
    }
}

fn value_string_at_path(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub trait IngestProcessor {
    fn process(&self, job: &IngestJob) -> IngestOutcome;
}

#[derive(Clone, Debug, Default)]
pub struct LocalIngestProcessor;

impl IngestProcessor for LocalIngestProcessor {
    fn process(&self, job: &IngestJob) -> IngestOutcome {
        match extract_document_text(&job.object_key, &job.content_type) {
            Ok(extracted) if !extracted.text.trim().is_empty() => {
                let chunks = split_text_chunks(&extracted.text, 1_800);
                IngestOutcome {
                    extracted_chars: extracted.text.chars().count(),
                    chunks,
                    inferred_title: None,
                    parse_method: extracted.method,
                    used_placeholder: false,
                    metadata: extracted.metadata,
                }
            }
            _ => build_placeholder_outcome(job),
        }
    }
}

pub type PlaceholderIngestProcessor = LocalIngestProcessor;

#[derive(Clone, Debug)]
pub struct ExtractedDocumentText {
    pub text: String,
    pub method: String,
    pub metadata: Value,
}

pub fn extract_document_text(
    object_key: &str,
    content_type: &str,
) -> std::io::Result<ExtractedDocumentText> {
    let path = resolve_ingest_object_path(object_key, content_type)?;
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| format!(".{}", value.to_ascii_lowercase()))
        .unwrap_or_else(|| infer_extension_from_content_type(content_type));

    if is_direct_text_extension(&extension) {
        let text = fs::read_to_string(&path).or_else(|_| {
            fs::read(&path).map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        })?;
        let text = if matches!(extension.as_str(), ".html" | ".htm" | ".xml") {
            strip_markup_tags(&text)
        } else {
            text
        };
        return Ok(extracted_text(text, format!("local-text{}", extension)));
    }

    if extension == ".pdf" {
        if let Some(extracted) = extract_pdf_text(&path) {
            return Ok(extracted);
        }
    }

    if is_image_extension(&extension) {
        return Ok(extract_image_document_text(&path));
    }

    if is_audio_extension(&extension) || is_video_extension(&extension) {
        return Ok(extract_media_document_text(
            &path,
            if is_video_extension(&extension) {
                "video"
            } else {
                "audio"
            },
        ));
    }

    if extension == ".docx" {
        if let Some(text) = extract_docx_text(&path) {
            return Ok(extracted_text(text, "docx-ooxml"));
        }
    }

    if matches!(extension.as_str(), ".xlsx" | ".xlsm") {
        if let Some(text) = extract_xlsx_text(&path) {
            return Ok(extracted_text(text, "xlsx-ooxml"));
        }
    }

    if matches!(extension.as_str(), ".pptx" | ".pptm") {
        let base_text = extract_pptx_text(&path);
        if let Some(vlm_text) = extract_presentation_with_vlm_render(&path) {
            let text = [base_text.unwrap_or_default(), vlm_text]
                .into_iter()
                .filter(|part| !part.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n\n");
            return Ok(ExtractedDocumentText {
                text,
                method: "pptx-ooxml+presentation-vlm".to_string(),
                metadata: json!({
                    "vlm": {
                        "provider": "minimax",
                        "source": "presentation-rendered-pages"
                    }
                }),
            });
        }
        if let Some(text) = base_text {
            return Ok(extracted_text(text, "pptx-ooxml"));
        }
    }

    if let Some(markdown) = extract_with_markitdown(&path) {
        return Ok(extracted_text(markdown, "markitdown"));
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "no local parser produced text",
    ))
}

fn extracted_text(text: impl Into<String>, method: impl Into<String>) -> ExtractedDocumentText {
    ExtractedDocumentText {
        text: text.into(),
        method: method.into(),
        metadata: json!({}),
    }
}

pub fn split_text_chunks(text: &str, max_chars: usize) -> Vec<String> {
    let max_chars = max_chars.max(1);
    let paragraphs = split_text_paragraphs(text);
    if paragraphs.is_empty() {
        return Vec::new();
    }

    let mut chunks = Vec::new();
    let mut current = String::new();
    for paragraph in paragraphs {
        let paragraph = paragraph.trim();
        if paragraph.is_empty() {
            continue;
        }
        if current.chars().count() + paragraph.chars().count() + 2 > max_chars
            && !current.is_empty()
        {
            chunks.push(current.trim().to_string());
            current.clear();
        }
        if paragraph.chars().count() > max_chars {
            if !current.is_empty() {
                chunks.push(current.trim().to_string());
                current.clear();
            }
            chunks.extend(split_long_text(paragraph, max_chars));
            continue;
        }
        if !current.is_empty() {
            current.push_str("\n\n");
        }
        current.push_str(paragraph);
    }
    if !current.trim().is_empty() {
        chunks.push(current.trim().to_string());
    }

    chunks
}

pub fn split_text_paragraphs(text: &str) -> Vec<String> {
    let sanitized = text.replace('\0', "");
    let mut paragraphs = Vec::new();
    let mut current_lines = Vec::new();

    for raw_line in sanitized.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            flush_text_paragraph(&mut paragraphs, &mut current_lines);
            continue;
        }
        current_lines.push(line.to_string());
    }
    flush_text_paragraph(&mut paragraphs, &mut current_lines);
    paragraphs
}

fn flush_text_paragraph(paragraphs: &mut Vec<String>, current_lines: &mut Vec<String>) {
    if current_lines.is_empty() {
        return;
    }
    let paragraph = current_lines.join("\n").trim().to_string();
    if !paragraph.is_empty() {
        paragraphs.push(paragraph);
    }
    current_lines.clear();
}

fn build_placeholder_outcome(job: &IngestJob) -> IngestOutcome {
    let chunk_count = if job.content_type.contains("pdf") {
        12
    } else {
        4
    };
    let chunks = (0..chunk_count)
        .map(|index| {
            format!(
                "Placeholder extracted chunk {} for {} ({})",
                index + 1,
                job.title,
                job.content_type
            )
        })
        .collect::<Vec<_>>();

    IngestOutcome {
        chunks,
        inferred_title: None,
        parse_method: "placeholder".to_string(),
        extracted_chars: 0,
        used_placeholder: true,
        metadata: json!({}),
    }
}

fn resolve_ingest_object_path(object_key: &str, content_type: &str) -> std::io::Result<PathBuf> {
    if is_remote_media_object_key(object_key, content_type) {
        return resolve_remote_media_object_path(object_key, content_type);
    }

    resolve_local_object_path(object_key).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "local object file not found")
    })
}

fn resolve_local_object_path(object_key: &str) -> Option<PathBuf> {
    let raw = object_key.trim().trim_start_matches("file://");
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

fn is_remote_media_object_key(object_key: &str, content_type: &str) -> bool {
    let raw = object_key.trim();
    if !(raw.starts_with("http://") || raw.starts_with("https://")) {
        return false;
    }
    let extension = extension_from_remote_url(raw)
        .unwrap_or_else(|| infer_extension_from_content_type(content_type));
    is_audio_extension(&extension) || is_video_extension(&extension)
}

fn resolve_remote_media_object_path(
    object_key: &str,
    content_type: &str,
) -> std::io::Result<PathBuf> {
    if !env_flag("INGEST_REMOTE_MEDIA_ENABLED", false) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "remote media ingest is disabled",
        ));
    }

    let url = reqwest::Url::parse(object_key.trim()).map_err(|error| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid remote media url: {error}"),
        )
    })?;
    validate_remote_media_url(&url, content_type)?;

    let cache_dir = remote_media_cache_dir();
    fs::create_dir_all(&cache_dir)?;
    let extension = extension_from_remote_url(url.as_str())
        .unwrap_or_else(|| infer_extension_from_content_type(content_type));
    let target_path = cache_dir.join(format!(
        "remote-media-{}{}",
        remote_media_cache_key(url.as_str()),
        extension
    ));
    if target_path.is_file() {
        return Ok(target_path);
    }

    let max_bytes = env_u64("INGEST_REMOTE_MEDIA_MAX_BYTES", 200 * 1024 * 1024);
    let timeout_secs = env_u64("INGEST_REMOTE_MEDIA_TIMEOUT_SECS", 60).max(1);
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()
        .map_err(remote_media_io_error)?;
    let mut response = client
        .get(url.clone())
        .send()
        .map_err(remote_media_io_error)?;
    if !response.status().is_success() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("remote media returned HTTP {}", response.status()),
        ));
    }
    if response
        .content_length()
        .is_some_and(|content_length| content_length > max_bytes)
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "remote media exceeds max allowed bytes",
        ));
    }
    validate_remote_media_response_type(&response, content_type, &extension)?;

    let temp_path = cache_dir.join(format!(
        "remote-media-{}-{}.part{}",
        remote_media_cache_key(url.as_str()),
        uuid::Uuid::new_v4(),
        extension
    ));
    let mut file = File::create(&temp_path)?;
    let mut downloaded = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = response.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        downloaded += read as u64;
        if downloaded > max_bytes {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "remote media exceeds max allowed bytes",
            ));
        }
        file.write_all(&buffer[..read])?;
    }
    file.flush()?;
    fs::rename(&temp_path, &target_path)?;
    Ok(target_path)
}

fn validate_remote_media_url(url: &reqwest::Url, content_type: &str) -> std::io::Result<()> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "remote media url must use http or https",
        ));
    }
    let host = url.host_str().unwrap_or_default();
    if !remote_media_host_allowed(host) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "remote media host is not allowed",
        ));
    }
    let lower = url.as_str().to_ascii_lowercase();
    if lower.contains("weixin.qq.com/sph/") || lower.contains("channels.weixin.qq.com/sph/") {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "login-gated media sources are not supported",
        ));
    }
    let extension = extension_from_remote_url(url.as_str())
        .unwrap_or_else(|| infer_extension_from_content_type(content_type));
    if !(is_audio_extension(&extension) || is_video_extension(&extension)) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "remote media url must point to an audio or video file",
        ));
    }
    Ok(())
}

fn remote_media_host_allowed(host: &str) -> bool {
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

fn validate_remote_media_response_type(
    response: &reqwest::blocking::Response,
    _content_type: &str,
    _extension: &str,
) -> std::io::Result<()> {
    let response_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !remote_media_response_type_allowed(&response_type) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "remote response is not a media content type",
        ));
    }
    Ok(())
}

fn remote_media_response_type_allowed(response_type: &str) -> bool {
    let media_type = response_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    media_type.is_empty()
        || media_type.starts_with("audio/")
        || media_type.starts_with("video/")
        || media_type == "application/octet-stream"
}

fn extension_from_remote_url(url: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(url).ok()?;
    let path = parsed.path().to_ascii_lowercase();
    [
        ".mp4", ".mov", ".m4v", ".webm", ".mkv", ".avi", ".mpeg", ".mpg", ".mp3", ".wav", ".m4a",
        ".aac", ".flac", ".ogg", ".opus",
    ]
    .into_iter()
    .find(|extension| path.ends_with(extension))
    .map(str::to_string)
}

fn remote_media_cache_dir() -> PathBuf {
    if let Ok(raw) = std::env::var("INGEST_REMOTE_MEDIA_CACHE_DIR") {
        let path = PathBuf::from(raw.trim());
        if !path.as_os_str().is_empty() {
            return path;
        }
    }
    std::env::temp_dir().join("aidp-v3-remote-media-cache")
}

fn remote_media_cache_key(value: &str) -> String {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn remote_media_io_error(error: reqwest::Error) -> std::io::Error {
    let kind = if error.is_timeout() {
        std::io::ErrorKind::TimedOut
    } else {
        std::io::ErrorKind::Other
    };
    std::io::Error::new(kind, error.to_string())
}

fn env_flag(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on" | "enabled"
            )
        })
        .unwrap_or(default)
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(default)
}

fn infer_extension_from_content_type(content_type: &str) -> String {
    let lower = content_type.to_ascii_lowercase();
    if lower.contains("markdown") {
        ".md".to_string()
    } else if lower.contains("csv") {
        ".csv".to_string()
    } else if lower.contains("json") {
        ".json".to_string()
    } else if lower.contains("html") || lower.contains("xml") {
        ".html".to_string()
    } else if lower.contains("pdf") {
        ".pdf".to_string()
    } else if lower.contains("jpeg") || lower.contains("jpg") {
        ".jpg".to_string()
    } else if lower.contains("png") {
        ".png".to_string()
    } else if lower.contains("webp") {
        ".webp".to_string()
    } else if lower.contains("mp3") || lower.contains("mpeg") {
        ".mp3".to_string()
    } else if lower.contains("wav") {
        ".wav".to_string()
    } else if lower.contains("mp4") {
        ".mp4".to_string()
    } else if lower.contains("quicktime") {
        ".mov".to_string()
    } else if lower.starts_with("audio/") {
        ".mp3".to_string()
    } else if lower.starts_with("video/") {
        ".mp4".to_string()
    } else if lower.starts_with("image/") {
        ".png".to_string()
    } else if lower.starts_with("text/") {
        ".txt".to_string()
    } else {
        String::new()
    }
}

fn is_direct_text_extension(extension: &str) -> bool {
    matches!(
        extension,
        ".txt" | ".md" | ".csv" | ".json" | ".html" | ".htm" | ".xml"
    )
}

fn is_image_extension(extension: &str) -> bool {
    matches!(
        extension,
        ".png" | ".jpg" | ".jpeg" | ".webp" | ".bmp" | ".tif" | ".tiff" | ".gif"
    )
}

fn is_audio_extension(extension: &str) -> bool {
    matches!(
        extension,
        ".mp3" | ".wav" | ".m4a" | ".aac" | ".flac" | ".ogg" | ".opus"
    )
}

fn is_video_extension(extension: &str) -> bool {
    matches!(
        extension,
        ".mp4" | ".mov" | ".mkv" | ".webm" | ".avi" | ".mpeg" | ".mpg"
    )
}

fn extract_pdf_text(path: &Path) -> Option<ExtractedDocumentText> {
    let mut usable_candidates = Vec::new();
    let mut low_quality_candidates = Vec::new();
    if let Some(extracted) = extract_pdf_with_paddleocr(path) {
        classify_pdf_candidate(
            extracted,
            &mut usable_candidates,
            &mut low_quality_candidates,
        );
    }

    for extracted in [
        extract_pdf_with_pdftotext(path).map(|text| extracted_text(text, "pdf-pdftotext")),
        extract_pdf_with_python(path).map(|text| extracted_text(text, "pdf-python")),
    ]
    .into_iter()
    .flatten()
    {
        classify_pdf_candidate(
            extracted,
            &mut usable_candidates,
            &mut low_quality_candidates,
        );
    }

    if let Some(selected) = select_pdf_usable_candidate(&mut usable_candidates) {
        return Some(rescue_weak_pdf_candidate_with_vlm(path, selected));
    }

    for extracted in [
        extract_pdf_with_ocrmypdf(path).map(|text| extracted_text(text, "pdf-ocrmypdf")),
        extract_pdf_with_tesseract_render(path)
            .map(|text| extracted_text(text, "pdf-tesseract-render")),
    ]
    .into_iter()
    .flatten()
    {
        classify_pdf_candidate(
            extracted,
            &mut usable_candidates,
            &mut low_quality_candidates,
        );
        if let Some(selected) = select_pdf_usable_candidate(&mut usable_candidates) {
            return Some(rescue_weak_pdf_candidate_with_vlm(path, selected));
        }
    }

    let low_quality_candidate = select_pdf_low_quality_candidate(low_quality_candidates);
    let low_quality_ocr = low_quality_candidate
        .as_ref()
        .map(|candidate| candidate.text.as_str());
    if let Some(vlm) = extract_pdf_with_vlm_render(path, low_quality_ocr) {
        let text_chars = pdf_quality_text_chars(&vlm.text);
        return Some(with_pdf_parse_quality_metadata(
            vlm,
            "vlm_fallback_used",
            text_chars,
            low_quality_candidate.as_ref(),
        ));
    }

    if let Some(literal) =
        extract_pdf_literal_text(path).map(|text| extracted_text(text, "pdf-literal"))
    {
        match pdf_parse_quality(&literal.text) {
            PdfParseQuality::Usable { text_chars } => {
                return Some(with_pdf_parse_quality_metadata(
                    literal,
                    "usable_text",
                    text_chars,
                    low_quality_candidate.as_ref(),
                ));
            }
            PdfParseQuality::LowTextCoverage { text_chars } => {
                let literal =
                    with_pdf_parse_quality_metadata(literal, "low_text_coverage", text_chars, None);
                let low_quality_candidate = match low_quality_candidate {
                    Some(candidate) => {
                        Some(select_better_pdf_low_quality_candidate(candidate, literal))
                    }
                    None => Some(literal),
                };
                return low_quality_candidate.map(pdf_low_quality_diagnostic_text);
            }
        }
    }

    low_quality_candidate.map(pdf_low_quality_diagnostic_text)
}

fn rescue_weak_pdf_candidate_with_vlm(
    path: &Path,
    selected: ExtractedDocumentText,
) -> ExtractedDocumentText {
    if !pdf_candidate_should_try_vlm_rescue(&selected) {
        return selected;
    }

    let Some(vlm) = extract_pdf_with_vlm_render(path, Some(&selected.text)) else {
        return selected;
    };
    let text_chars = pdf_quality_text_chars(&vlm.text);
    let vlm =
        with_pdf_parse_quality_metadata(vlm, "vlm_fallback_used", text_chars, Some(&selected));
    select_pdf_vlm_rescue_candidate(selected, vlm)
}

fn classify_pdf_candidate(
    extracted: ExtractedDocumentText,
    usable_candidates: &mut Vec<ExtractedDocumentText>,
    low_quality_candidates: &mut Vec<ExtractedDocumentText>,
) {
    match pdf_parse_quality(&extracted.text) {
        PdfParseQuality::Usable { text_chars } => {
            usable_candidates.push(with_pdf_parse_quality_metadata(
                extracted,
                "usable_text",
                text_chars,
                None,
            ));
        }
        PdfParseQuality::LowTextCoverage { text_chars } => {
            low_quality_candidates.push(with_pdf_parse_quality_metadata(
                extracted,
                "low_text_coverage",
                text_chars,
                None,
            ));
        }
    }
}

fn select_pdf_usable_candidate(
    candidates: &mut Vec<ExtractedDocumentText>,
) -> Option<ExtractedDocumentText> {
    if candidates.is_empty() {
        return None;
    }
    let reports = candidates
        .iter()
        .map(pdf_candidate_quality_report)
        .collect::<Vec<_>>();
    let (selected_index, _) = candidates
        .iter()
        .enumerate()
        .max_by_key(|(_, candidate)| pdf_candidate_quality_score(candidate))?;
    let mut selected = candidates.remove(selected_index);
    let selected_report = pdf_candidate_quality_report(&selected);
    merge_parse_quality_field(
        &mut selected.metadata,
        "candidate_selection",
        json!({
            "policy": "score_text_structure_and_layout",
            "selected_method": selected.method.clone(),
            "selected": selected_report,
            "candidates": reports,
        }),
    );
    Some(selected)
}

fn select_pdf_low_quality_candidate(
    candidates: Vec<ExtractedDocumentText>,
) -> Option<ExtractedDocumentText> {
    candidates
        .into_iter()
        .max_by_key(pdf_low_quality_candidate_score)
}

fn select_better_pdf_low_quality_candidate(
    left: ExtractedDocumentText,
    right: ExtractedDocumentText,
) -> ExtractedDocumentText {
    if pdf_low_quality_candidate_score(&right) > pdf_low_quality_candidate_score(&left) {
        right
    } else {
        left
    }
}

fn pdf_candidate_should_try_vlm_rescue(candidate: &ExtractedDocumentText) -> bool {
    if !document_pdf_vlm_fallback_on_weak_text() {
        return false;
    }
    let text_chars = pdf_quality_text_chars(&candidate.text);
    text_chars <= document_pdf_vlm_weak_text_max_chars()
        && pdf_candidate_structure_block_count(candidate) == 0
        && pdf_candidate_heading_count(&candidate.text) == 0
        && pdf_candidate_table_signal_count(&candidate.text) == 0
}

fn document_pdf_vlm_fallback_on_weak_text() -> bool {
    env_flag_value("DOCUMENT_PDF_VLM_FALLBACK_ON_WEAK_TEXT").unwrap_or(true)
}

fn document_pdf_vlm_weak_text_max_chars() -> usize {
    std::env::var("DOCUMENT_PDF_VLM_WEAK_TEXT_MAX_CHARS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(256)
        .max(pdf_min_usable_text_chars())
}

fn select_pdf_vlm_rescue_candidate(
    mut existing: ExtractedDocumentText,
    mut vlm: ExtractedDocumentText,
) -> ExtractedDocumentText {
    let existing_report = pdf_candidate_quality_report(&existing);
    let vlm_report = pdf_candidate_quality_report(&vlm);
    let existing_score = pdf_candidate_quality_score(&existing);
    let vlm_score = pdf_candidate_quality_score(&vlm);
    let selected = if vlm_score > existing_score {
        "vlm"
    } else {
        "existing"
    };
    let rescue_report = json!({
        "policy": "try_minimax_for_weak_unstructured_pdf_text",
        "selected": selected,
        "existing": existing_report,
        "vlm": vlm_report,
    });

    if selected == "vlm" {
        merge_parse_quality_field(&mut vlm.metadata, "vlm_rescue", rescue_report);
        vlm
    } else {
        merge_parse_quality_field(&mut existing.metadata, "vlm_rescue", rescue_report);
        existing
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PdfParseQuality {
    Usable { text_chars: usize },
    LowTextCoverage { text_chars: usize },
}

fn pdf_parse_quality(text: &str) -> PdfParseQuality {
    let text_chars = pdf_quality_text_chars(text);
    if text_chars < pdf_min_usable_text_chars() {
        PdfParseQuality::LowTextCoverage { text_chars }
    } else {
        PdfParseQuality::Usable { text_chars }
    }
}

fn pdf_quality_text_chars(text: &str) -> usize {
    text.chars()
        .filter(|ch| !ch.is_whitespace() && !ch.is_control())
        .count()
}

fn pdf_min_usable_text_chars() -> usize {
    std::env::var("DOCUMENT_PDF_MIN_USABLE_TEXT_CHARS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(32)
        .max(1)
}

fn with_pdf_parse_quality_metadata(
    mut extracted: ExtractedDocumentText,
    status: &str,
    text_chars: usize,
    fallback_from: Option<&ExtractedDocumentText>,
) -> ExtractedDocumentText {
    let fallback_from = fallback_from.map(|candidate| {
        json!({
            "method": candidate.method.clone(),
            "text_chars": pdf_quality_text_chars(&candidate.text),
        })
    });
    merge_object_value(
        &mut extracted.metadata,
        "parse_quality",
        json!({
            "kind": "pdf_text_extraction",
            "status": status,
            "text_chars": text_chars,
            "min_usable_text_chars": pdf_min_usable_text_chars(),
            "fallback_from": fallback_from,
        }),
    );
    extracted
}

fn merge_parse_quality_field(target: &mut Value, key: &str, value: Value) {
    if let Some(parse_quality) = target
        .get_mut("parse_quality")
        .and_then(Value::as_object_mut)
    {
        parse_quality.insert(key.to_string(), value);
    }
}

fn pdf_candidate_quality_report(extracted: &ExtractedDocumentText) -> Value {
    let text_chars = pdf_quality_text_chars(&extracted.text);
    let structure_block_count = pdf_candidate_structure_block_count(extracted);
    let heading_count = pdf_candidate_heading_count(&extracted.text);
    let table_signal_count = pdf_candidate_table_signal_count(&extracted.text);
    json!({
        "method": extracted.method.clone(),
        "text_chars": text_chars,
        "structure_block_count": structure_block_count,
        "heading_count": heading_count,
        "table_signal_count": table_signal_count,
        "quality_score": pdf_candidate_quality_score(extracted),
    })
}

fn pdf_candidate_quality_score(extracted: &ExtractedDocumentText) -> usize {
    let text_chars = pdf_quality_text_chars(&extracted.text).min(50_000);
    let structure_block_count = pdf_candidate_structure_block_count(extracted).min(200);
    let heading_count = pdf_candidate_heading_count(&extracted.text).min(50);
    let table_signal_count = pdf_candidate_table_signal_count(&extracted.text).min(50);
    let method_bonus = if extracted.method.contains("paddleocr") {
        80
    } else if extracted.method.contains("pdftotext") {
        20
    } else if extracted.method.contains("python") {
        15
    } else {
        0
    };

    text_chars
        + structure_block_count * 8
        + heading_count * 40
        + table_signal_count * 20
        + method_bonus
}

fn pdf_low_quality_candidate_score(extracted: &ExtractedDocumentText) -> usize {
    pdf_quality_text_chars(&extracted.text)
        + pdf_candidate_structure_block_count(extracted).min(50) * 2
        + pdf_candidate_heading_count(&extracted.text).min(10) * 5
        + pdf_candidate_table_signal_count(&extracted.text).min(10) * 5
}

fn pdf_candidate_structure_block_count(extracted: &ExtractedDocumentText) -> usize {
    extracted
        .metadata
        .get("document_structure")
        .and_then(|value| value.get("block_count"))
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(0)
}

fn pdf_candidate_heading_count(text: &str) -> usize {
    text.lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.starts_with('#')
                || trimmed.ends_with(':')
                || trimmed.ends_with('：')
                || looks_like_numbered_heading(trimmed)
        })
        .count()
}

fn looks_like_numbered_heading(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_digit() {
        return false;
    }
    let prefix = chars.take(5).collect::<String>();
    prefix.contains('.') || prefix.contains('、') || prefix.contains(')')
}

fn pdf_candidate_table_signal_count(text: &str) -> usize {
    text.lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.matches('|').count() >= 2 || trimmed.contains('\t')
        })
        .count()
}

fn pdf_low_quality_diagnostic_text(mut extracted: ExtractedDocumentText) -> ExtractedDocumentText {
    let text_chars = pdf_quality_text_chars(&extracted.text);
    let snippet = extracted
        .text
        .chars()
        .take(200)
        .collect::<String>()
        .trim()
        .to_string();
    extracted.text = format!(
        "PDF parse quality warning: text extraction produced only {text_chars} non-whitespace characters, below the minimum usable threshold. OCR/VLM fallback did not produce displayable text; do not treat the low-quality extract as document content.\n\nLow-quality extract:\n{snippet}"
    );
    extracted.method = format!("{}+low-quality", extracted.method);
    merge_object_value(
        &mut extracted.metadata,
        "parse_quality",
        json!({
            "kind": "pdf_text_extraction",
            "status": "low_text_coverage_fallback_unavailable",
            "text_chars": text_chars,
            "min_usable_text_chars": pdf_min_usable_text_chars(),
            "fallback_status": "unavailable",
        }),
    );
    extracted
}

fn merge_object_value(target: &mut Value, key: &str, value: Value) {
    if !target.is_object() {
        *target = json!({});
    }
    if let Some(object) = target.as_object_mut() {
        object.insert(key.to_string(), value);
    }
}

fn extract_pdf_with_paddleocr(path: &Path) -> Option<ExtractedDocumentText> {
    if !document_paddleocr_enabled() {
        return None;
    }

    let temp_dir = create_temp_dir("aidp-paddleocr").ok()?;
    let extracted = run_paddleocr_sidecar(path, &temp_dir);
    let _ = fs::remove_dir_all(&temp_dir);
    extracted
}

fn document_paddleocr_enabled() -> bool {
    if let Some(enabled) = env_flag_value("DOCUMENT_PADDLEOCR_ENABLED") {
        return enabled;
    }

    if let Ok(engine) = std::env::var("DOCUMENT_PDF_PARSE_ENGINE") {
        let engine = engine.trim().to_ascii_lowercase();
        if matches!(engine.as_str(), "off" | "disabled" | "native_first") {
            return false;
        }
        if matches!(engine.as_str(), "paddleocr" | "paddleocr_first") {
            return true;
        }
    }

    std::env::var("DOCUMENT_PADDLEOCR_PYTHON_BIN")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

fn document_paddleocr_timeout() -> Duration {
    let millis = std::env::var("DOCUMENT_PADDLEOCR_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(300_000)
        .max(1_000);
    Duration::from_millis(millis)
}

fn document_paddleocr_max_pages() -> usize {
    std::env::var("DOCUMENT_PADDLEOCR_MAX_PAGES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(8)
        .max(1)
}

fn env_flag_value(name: &str) -> Option<bool> {
    std::env::var(name).ok().and_then(|value| {
        let value = value.trim().to_ascii_lowercase();
        match value.as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" | "disabled" => Some(false),
            _ => None,
        }
    })
}

fn paddleocr_python_command_candidates() -> Vec<String> {
    let mut candidates = direct_command_candidates("DOCUMENT_PADDLEOCR_PYTHON_BIN", "");
    candidates.extend(python_command_candidates());
    candidates.dedup();
    candidates
}

fn run_paddleocr_sidecar(path: &Path, output_dir: &Path) -> Option<ExtractedDocumentText> {
    let output_path = output_dir.join("result.json");
    let path_arg = path.to_string_lossy().to_string();
    let output_arg = output_path.to_string_lossy().to_string();
    let max_pages = document_paddleocr_max_pages();
    let max_pages_arg = max_pages.to_string();
    let script = r##"
import json
import os
import sys

os.environ.setdefault("PADDLE_PDX_DISABLE_MODEL_SOURCE_CHECK", "True")

pdf_path = sys.argv[1]
output_path = sys.argv[2]
max_pages = max(1, int(sys.argv[3]))
os.makedirs(os.path.dirname(output_path), exist_ok=True)

def env_bool(name, default):
    value = os.environ.get(name)
    if value is None or not str(value).strip():
        return default
    return str(value).strip().lower() in ("1", "true", "yes", "on")

def emit(payload):
    with open(output_path, "w", encoding="utf-8") as fh:
        json.dump(payload, fh, ensure_ascii=False)

def truncate_text(value, limit=1200):
    text = str(value or "").strip()
    return text if len(text) <= limit else text[:limit] + "..."

def plain(value):
    if value is None or isinstance(value, (str, int, float, bool)):
        return value
    if isinstance(value, dict):
        return {str(k): plain(v) for k, v in value.items()}
    if isinstance(value, (list, tuple)):
        return [plain(item) for item in value]
    for attr in ("json",):
        try:
            attr_value = getattr(value, attr)
            if callable(attr_value):
                attr_value = attr_value()
            return plain(attr_value)
        except Exception:
            pass
    try:
        return json.loads(json.dumps(value, ensure_ascii=False, default=str))
    except Exception:
        return str(value)

def markdown_text(value):
    value = plain(value)
    if isinstance(value, str):
        return value.strip()
    if isinstance(value, dict):
        for key in ("markdown_texts", "markdown_text", "markdown", "text", "content"):
            item = value.get(key)
            if isinstance(item, str) and item.strip():
                return item.strip()
            if isinstance(item, list):
                joined = "\n\n".join(str(part).strip() for part in item if str(part).strip())
                if joined.strip():
                    return joined.strip()
    if isinstance(value, list):
        joined = "\n\n".join(markdown_text(item) for item in value)
        return joined.strip()
    return ""

def first_text_value(node):
    if not isinstance(node, dict):
        return ""
    for key in ("text", "content", "rec_text", "transcription", "markdown", "html"):
        value = node.get(key)
        if isinstance(value, str) and value.strip():
            return value.strip()
    texts = node.get("rec_texts")
    if isinstance(texts, list):
        joined = " ".join(str(item).strip() for item in texts if str(item).strip())
        if joined:
            return joined
    return ""

def collect_blocks(node, page_number, blocks):
    if len(blocks) >= 200:
        return
    if isinstance(node, dict):
        text = first_text_value(node)
        if text:
            block = {
                "page_number": page_number,
                "text": truncate_text(text),
            }
            for out_key, keys in (
                ("type", ("block_label", "type", "label", "category")),
                ("bbox", ("bbox", "box", "poly", "coordinate")),
                ("confidence", ("confidence", "score", "rec_score")),
            ):
                for key in keys:
                    if key in node and node[key] not in (None, ""):
                        block[out_key] = plain(node[key])
                        break
            blocks.append(block)
        for value in node.values():
            collect_blocks(value, page_number, blocks)
    elif isinstance(node, list):
        for item in node:
            collect_blocks(item, page_number, blocks)

try:
    from paddleocr import PPStructureV3
except Exception as exc:
    emit({"ok": False, "error": f"paddleocr_import_failed: {exc}"})
    sys.exit(2)

errors = []
blocks = []
raw_markdowns = []
page_markdowns = []
page_count = 0

try:
    paddleocr_config = {
        "use_table_recognition": env_bool("DOCUMENT_PADDLEOCR_USE_TABLE_RECOGNITION", True),
        "use_formula_recognition": env_bool("DOCUMENT_PADDLEOCR_USE_FORMULA_RECOGNITION", False),
        "use_chart_recognition": env_bool("DOCUMENT_PADDLEOCR_USE_CHART_RECOGNITION", False),
        "use_seal_recognition": env_bool("DOCUMENT_PADDLEOCR_USE_SEAL_RECOGNITION", False),
    }
    pipeline = PPStructureV3(**paddleocr_config)
    output = pipeline.predict(input=pdf_path)
    for page_index, result in enumerate(output):
        if page_index >= max_pages:
            break
        page_number = page_index + 1
        page_count += 1
        try:
            result.save_to_json(save_path=os.path.dirname(output_path))
        except Exception as exc:
            errors.append(f"save_to_json_page_{page_number}: {exc}")
        try:
            result.save_to_markdown(save_path=os.path.dirname(output_path))
        except Exception as exc:
            errors.append(f"save_to_markdown_page_{page_number}: {exc}")
        markdown_obj = getattr(result, "markdown", None)
        if markdown_obj is not None:
            raw_markdowns.append(markdown_obj)
        markdown = markdown_text(markdown_obj) or markdown_text(result)
        if markdown:
            page_markdowns.append(f"# Page {page_number}\n\n{markdown}")
        collect_blocks(plain(result), page_number, blocks)

    combined_markdown = ""
    if raw_markdowns and hasattr(pipeline, "concatenate_markdown_pages"):
        try:
            combined_markdown = markdown_text(pipeline.concatenate_markdown_pages(raw_markdowns))
        except Exception as exc:
            errors.append(f"concatenate_markdown_pages: {exc}")
    if not combined_markdown:
        combined_markdown = "\n\n".join(page_markdowns)

    emit({
        "ok": True,
        "markdown": combined_markdown,
        "page_count": page_count,
        "block_count": len(blocks),
        "blocks": blocks,
        "errors": errors,
        "config": paddleocr_config,
    })
    sys.exit(0 if combined_markdown.strip() else 3)
except Exception as exc:
    emit({
        "ok": False,
        "error": f"paddleocr_predict_failed: {exc}",
        "page_count": page_count,
        "block_count": len(blocks),
        "blocks": blocks,
        "errors": errors,
        "config": globals().get("paddleocr_config", {}),
    })
    sys.exit(4)
"##;

    for command in paddleocr_python_command_candidates() {
        let _ = fs::remove_file(&output_path);
        if !run_status_command_with_timeout(
            &command,
            &[
                "-c",
                script,
                path_arg.as_str(),
                output_arg.as_str(),
                max_pages_arg.as_str(),
            ],
            document_paddleocr_timeout(),
        ) {
            continue;
        }
        let Some(payload) = fs::read_to_string(&output_path)
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        else {
            continue;
        };
        if let Some(extracted) = paddleocr_payload_to_extracted_text(payload, max_pages) {
            return Some(extracted);
        }
    }
    None
}

fn paddleocr_payload_to_extracted_text(
    payload: Value,
    max_pages: usize,
) -> Option<ExtractedDocumentText> {
    if payload
        .get("ok")
        .and_then(Value::as_bool)
        .is_some_and(|ok| !ok)
    {
        return None;
    }
    let markdown = payload
        .get("markdown")
        .and_then(Value::as_str)
        .unwrap_or("")
        .replace('\x0c', "\n");
    let text = normalize_extracted_text(&markdown)?;
    let blocks = payload.get("blocks").cloned().unwrap_or_else(|| json!([]));
    let block_count = payload
        .get("block_count")
        .and_then(Value::as_u64)
        .or_else(|| blocks.as_array().map(|items| items.len() as u64))
        .unwrap_or(0);
    Some(ExtractedDocumentText {
        text,
        method: "pdf-paddleocr".to_string(),
        metadata: json!({
            "document_structure": {
                "source": "paddleocr_pp_structure_v3",
                "format": "structured_markdown",
                "page_count": payload.get("page_count").cloned().unwrap_or(Value::Null),
                "block_count": block_count,
                "blocks": blocks,
            },
            "paddleocr": {
                "parser": "PP-StructureV3",
                "max_pages": max_pages,
                "config": payload.get("config").cloned().unwrap_or_else(|| json!({})),
                "errors": payload.get("errors").cloned().unwrap_or_else(|| json!([])),
            }
        }),
    })
}

fn extract_pdf_with_pdftotext(path: &Path) -> Option<String> {
    let path_arg = path.to_string_lossy().to_string();
    let candidates = [
        std::env::var("PDFTOTEXT_BIN").ok(),
        Some("pdftotext".to_string()),
    ];
    for command in candidates.into_iter().flatten() {
        if let Some(output) = run_text_command(&command, &[path_arg.as_str(), "-"]) {
            if let Some(text) = normalize_extracted_text(&output.replace('\x0c', "\n")) {
                return Some(text);
            }
        }
    }
    None
}

fn extract_pdf_with_python(path: &Path) -> Option<String> {
    let path_arg = path.to_string_lossy().to_string();
    let script = r#"
import sys
try:
    from pypdf import PdfReader
except Exception:
    from PyPDF2 import PdfReader
reader = PdfReader(sys.argv[1])
for page in reader.pages:
    text = page.extract_text() or ""
    if text:
        print(text)
"#;
    for command in python_command_candidates() {
        if let Some(output) = run_text_command(&command, &["-c", script, path_arg.as_str()]) {
            if let Some(text) = normalize_extracted_text(&output.replace('\x0c', "\n")) {
                return Some(text);
            }
        }
    }
    None
}

fn extract_pdf_with_ocrmypdf(path: &Path) -> Option<String> {
    let temp_dir = create_temp_dir("aidp-ocrmypdf").ok()?;
    let sidecar_path = temp_dir.join("sidecar.txt");
    let output_pdf_path = temp_dir.join("ocr-output.pdf");
    let path_arg = path.to_string_lossy().to_string();
    let sidecar_arg = sidecar_path.to_string_lossy().to_string();
    let output_arg = output_pdf_path.to_string_lossy().to_string();
    let args = [
        "--force-ocr",
        "--skip-big",
        "50",
        "--sidecar",
        sidecar_arg.as_str(),
        path_arg.as_str(),
        output_arg.as_str(),
    ];

    let mut extracted = None;
    for command in direct_command_candidates("OCRMYPDF_BIN", "ocrmypdf") {
        if run_status_command(&command, &args) {
            extracted = fs::read_to_string(&sidecar_path)
                .ok()
                .and_then(|text| normalize_extracted_text(&text.replace('\x0c', "\n")));
            if extracted.is_some() {
                break;
            }
        }
    }
    let _ = fs::remove_dir_all(&temp_dir);
    extracted
}

fn extract_pdf_with_tesseract_render(path: &Path) -> Option<String> {
    let path_arg = path.to_string_lossy().to_string();
    let max_pages = std::env::var("DOCUMENT_PDF_OCR_MAX_PAGES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(4)
        .max(1)
        .to_string();
    let script = r#"
import os, shutil, subprocess, sys, tempfile
try:
    from pdf2image import convert_from_path
except Exception:
    sys.exit(2)
work = tempfile.mkdtemp(prefix="aidp-pdf-render-")
try:
    max_pages = max(1, int(sys.argv[2]))
    pages = convert_from_path(sys.argv[1], first_page=1, last_page=max_pages)
    tesseract = os.environ.get("TESSERACT_BIN", "tesseract")
    texts = []
    for index, image in enumerate(pages):
        image_path = os.path.join(work, f"page-{index + 1}.png")
        image.save(image_path)
        result = subprocess.run([tesseract, image_path, "stdout", "--psm", "3"], capture_output=True, text=True, encoding="utf-8", errors="ignore")
        if result.returncode == 0 and result.stdout.strip():
            texts.append(result.stdout.strip())
    print("\f".join(texts))
finally:
    shutil.rmtree(work, ignore_errors=True)
"#;
    for command in python_command_candidates() {
        if let Some(output) = run_text_command(
            &command,
            &["-c", script, path_arg.as_str(), max_pages.as_str()],
        ) {
            if let Some(text) = normalize_extracted_text(&output.replace('\x0c', "\n")) {
                return Some(text);
            }
        }
    }
    None
}

fn extract_pdf_with_vlm_render(
    path: &Path,
    existing_ocr_text: Option<&str>,
) -> Option<ExtractedDocumentText> {
    if !DocumentImageVlmConfig::from_env().available() {
        return None;
    }

    let temp_dir = create_temp_dir("aidp-pdf-vlm-render").ok()?;
    let rendered = render_pdf_pages_to_images(path, &temp_dir, "DOCUMENT_PDF_VLM_MAX_PAGES", 4);
    let mut blocks = Vec::new();
    let mut pages = Vec::new();
    for (index, image_path) in rendered.iter().enumerate() {
        let page_title = format!(
            "{} - Page {}",
            path.file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("PDF"),
            index + 1
        );
        if let Some(response) = run_document_image_vlm_from_env(
            &page_title,
            image_path,
            existing_ocr_text.unwrap_or(""),
        ) {
            pages.push(json!({
                "page_number": index + 1,
                "model": response.model,
                "payload": response.payload,
            }));
            blocks.push(format!(
                "# Page {}\n\n{}",
                index + 1,
                build_enriched_image_text(
                    image_path
                        .file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or("page.png"),
                    None,
                    &response.payload,
                )
            ));
        }
    }
    let _ = fs::remove_dir_all(&temp_dir);

    if blocks.is_empty() {
        return None;
    }
    let text = normalize_extracted_text(&format!(
        "[PDF VLM understanding]\n\n{}",
        blocks.join("\n\n")
    ))?;
    Some(ExtractedDocumentText {
        text,
        method: "pdf-vlm".to_string(),
        metadata: json!({
            "vlm": {
                "provider": "minimax",
                "source": "pdf-rendered-pages",
                "pages": pages,
            }
        }),
    })
}

fn extract_presentation_with_vlm_render(path: &Path) -> Option<String> {
    if !DocumentImageVlmConfig::from_env().available() {
        return None;
    }

    let temp_dir = create_temp_dir("aidp-presentation-vlm-render").ok()?;
    let pdf_path = convert_presentation_to_pdf(path, &temp_dir);
    let rendered = pdf_path
        .as_deref()
        .map(|pdf_path| {
            render_pdf_pages_to_images(
                pdf_path,
                &temp_dir,
                "DOCUMENT_PRESENTATION_VLM_MAX_SLIDES",
                4,
            )
        })
        .unwrap_or_default();
    let mut blocks = Vec::new();
    for (index, image_path) in rendered.iter().enumerate() {
        let slide_title = format!(
            "{} - Slide {}",
            path.file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("Presentation"),
            index + 1
        );
        if let Some(response) = run_document_image_vlm_from_env(&slide_title, image_path, "") {
            blocks.push(format!(
                "# Slide {}\n\n{}",
                index + 1,
                build_enriched_image_text(
                    image_path
                        .file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or("slide.png"),
                    None,
                    &response.payload,
                )
            ));
        }
    }
    let _ = fs::remove_dir_all(&temp_dir);

    if blocks.is_empty() {
        return None;
    }
    normalize_extracted_text(&format!(
        "[Presentation VLM understanding]\n\n{}",
        blocks.join("\n\n")
    ))
}

fn convert_presentation_to_pdf(path: &Path, output_dir: &Path) -> Option<PathBuf> {
    let path_arg = path.to_string_lossy().to_string();
    let output_arg = output_dir.to_string_lossy().to_string();
    let args = [
        "--headless",
        "--convert-to",
        "pdf",
        "--outdir",
        output_arg.as_str(),
        path_arg.as_str(),
    ];
    for command in presentation_converter_candidates() {
        if !run_status_command(&command, &args) {
            continue;
        }
        let expected = output_dir.join(format!(
            "{}.pdf",
            path.file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("output")
        ));
        if expected.is_file() {
            return Some(expected);
        }
        if let Some(found) = fs::read_dir(output_dir).ok().and_then(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .find(|path| {
                    path.extension()
                        .and_then(|value| value.to_str())
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"))
                })
        }) {
            return Some(found);
        }
    }
    None
}

fn render_pdf_pages_to_images(
    path: &Path,
    output_dir: &Path,
    max_pages_env: &str,
    default_max_pages: usize,
) -> Vec<PathBuf> {
    let path_arg = path.to_string_lossy().to_string();
    let output_arg = output_dir.to_string_lossy().to_string();
    let max_pages = std::env::var(max_pages_env)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(default_max_pages)
        .max(1)
        .to_string();
    let script = r#"
import os, sys
try:
    from pdf2image import convert_from_path
except Exception:
    sys.exit(2)
out = sys.argv[2]
os.makedirs(out, exist_ok=True)
max_pages = max(1, int(sys.argv[3]))
pages = convert_from_path(sys.argv[1], first_page=1, last_page=max_pages)
for index, image in enumerate(pages):
    image_path = os.path.join(out, f"page-{index + 1}.png")
    image.save(image_path)
    print(image_path)
"#;
    for command in python_command_candidates() {
        if let Some(output) = run_text_command(
            &command,
            &[
                "-c",
                script,
                path_arg.as_str(),
                output_arg.as_str(),
                max_pages.as_str(),
            ],
        ) {
            let paths = output
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(PathBuf::from)
                .filter(|path| path.is_file())
                .collect::<Vec<_>>();
            if !paths.is_empty() {
                return paths;
            }
        }
    }
    Vec::new()
}

fn extract_pdf_literal_text(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    let mut cursor = 0;
    let mut literals = Vec::new();
    while cursor < bytes.len() {
        if bytes[cursor] == b'(' {
            if let Some((literal, next_cursor)) = decode_pdf_literal_at(&bytes, cursor) {
                let tail_end = (next_cursor + 48).min(bytes.len());
                let tail = String::from_utf8_lossy(&bytes[next_cursor..tail_end]);
                if tail.contains("Tj")
                    || tail.contains("TJ")
                    || is_probably_pdf_text_literal(&literal)
                {
                    literals.push(literal);
                }
                cursor = next_cursor;
                continue;
            }
        }
        cursor += 1;
    }

    normalize_extracted_text(&literals.join("\n"))
}

fn decode_pdf_literal_at(bytes: &[u8], start: usize) -> Option<(String, usize)> {
    let mut cursor = start + 1;
    let mut depth = 1;
    let mut output = Vec::new();

    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\\' => {
                cursor += 1;
                if cursor >= bytes.len() {
                    break;
                }
                match bytes[cursor] {
                    b'n' => output.push(b'\n'),
                    b'r' => output.push(b'\n'),
                    b't' => output.push(b'\t'),
                    b'b' => output.push(8),
                    b'f' => output.push(12),
                    b'(' => output.push(b'('),
                    b')' => output.push(b')'),
                    b'\\' => output.push(b'\\'),
                    b'\r' => {
                        if bytes.get(cursor + 1) == Some(&b'\n') {
                            cursor += 1;
                        }
                    }
                    b'\n' => {}
                    b'0'..=b'7' => {
                        let mut value: u16 = 0;
                        let mut count = 0;
                        while count < 3 && cursor < bytes.len() && bytes[cursor].is_ascii_digit() {
                            let digit = bytes[cursor] - b'0';
                            if digit > 7 {
                                break;
                            }
                            value = value.saturating_mul(8).saturating_add(digit.into());
                            cursor += 1;
                            count += 1;
                        }
                        output.push(value.min(u8::MAX.into()) as u8);
                        continue;
                    }
                    other => output.push(other),
                }
            }
            b'(' => {
                depth += 1;
                output.push(b'(');
            }
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some((decode_pdf_string_bytes(&output), cursor + 1));
                }
                output.push(b')');
            }
            other => output.push(other),
        }
        cursor += 1;
    }

    None
}

fn decode_pdf_string_bytes(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xFE, 0xFF]) && bytes.len() > 2 {
        let utf16 = bytes[2..]
            .chunks_exact(2)
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        if let Ok(text) = String::from_utf16(&utf16) {
            return text;
        }
    }
    String::from_utf8_lossy(bytes).into_owned()
}

fn is_probably_pdf_text_literal(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.chars().count() < 4 {
        return false;
    }
    let meaningful = trimmed
        .chars()
        .filter(|ch| ch.is_alphanumeric() || ch.is_ascii_punctuation())
        .count();
    meaningful >= 4 && !trimmed.starts_with('/')
}

fn extract_image_document_text(path: &Path) -> ExtractedDocumentText {
    let image_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("image");
    let ocr_text = extract_image_text_with_tesseract(path);

    if let Some(response) =
        run_document_image_vlm_from_env(image_name, path, ocr_text.as_deref().unwrap_or(""))
    {
        return ExtractedDocumentText {
            text: build_enriched_image_text(image_name, ocr_text.as_deref(), &response.payload),
            method: if ocr_text.is_some() {
                "image-ocr+vlm".to_string()
            } else {
                "image-vlm".to_string()
            },
            metadata: build_image_vlm_metadata(response.model, response.payload),
        };
    }

    if let Some(ocr_text) = ocr_text {
        return ExtractedDocumentText {
            text: format!("Image file: {image_name}\n\nOCR text:\n{ocr_text}"),
            method: "image-ocr".to_string(),
            metadata: json!({}),
        };
    }

    ExtractedDocumentText {
        text: format!("Image file: {image_name}\n\nOCR text was not extracted from this image."),
        method: "image-ocr-empty".to_string(),
        metadata: json!({}),
    }
}

fn build_image_vlm_metadata(model: String, payload: DocumentImageVlmPayload) -> Value {
    json!({
        "vlm": {
            "provider": "minimax",
            "model": model,
            "source": "image-document",
            "payload": payload,
        }
    })
}

#[derive(Clone, Debug)]
struct MediaTranscript {
    text: String,
    source: String,
    segments: Vec<MediaTranscriptSegment>,
}

#[derive(Clone, Debug, Serialize)]
struct MediaTranscriptSegment {
    start_seconds: Option<f64>,
    end_seconds: Option<f64>,
    text: String,
    source: String,
    language: Option<String>,
    confidence: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
struct MediaScene {
    start_seconds: Option<f64>,
    end_seconds: Option<f64>,
    representative_seconds: Option<f64>,
    summary: String,
    source: String,
}

#[derive(Clone, Debug, Serialize)]
struct MediaOcrSnippet {
    timestamp_seconds: Option<f64>,
    text: String,
    source: String,
}

fn extract_media_document_text(path: &Path, media_kind: &str) -> ExtractedDocumentText {
    let media_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("media");
    let probe = probe_media_metadata(path);
    let transcript = extract_media_transcript(path);
    let transcript_text = transcript.as_ref().map(|item| item.text.as_str());
    let transcript_segments = transcript
        .as_ref()
        .map(|item| item.segments.clone())
        .unwrap_or_default();
    let scenes = if media_kind == "video" {
        extract_media_scenes(path)
    } else {
        Vec::new()
    };
    let keyframe_ocr_snippets = if media_kind == "video" {
        extract_keyframe_ocr_snippets(path)
    } else {
        Vec::new()
    };
    let minimax_capabilities = probe_minimax_media_capabilities_from_env();
    let metadata_summary = summarize_media_probe(probe.as_ref());
    let parse_status = if transcript_text.is_some_and(|text| !text.trim().is_empty()) {
        "transcribed"
    } else if !scenes.is_empty() || !keyframe_ocr_snippets.is_empty() {
        "enriched_partial"
    } else {
        "partial"
    };
    let text = [
        format!("Media file: {media_name}"),
        format!("Media type: {media_kind}"),
        (!metadata_summary.is_empty())
            .then(|| format!("Media metadata:\n{metadata_summary}"))
            .unwrap_or_default(),
        transcript_text
            .filter(|text| !text.trim().is_empty())
            .map(|text| {
                if transcript_segments.is_empty() {
                    format!("Transcript:\n{text}")
                } else {
                    format!(
                        "Transcript segments:\n{}",
                        format_transcript_segments_for_text(&transcript_segments)
                    )
                }
            })
            .unwrap_or_else(|| {
                "Transcript was not extracted because no configured media transcription command produced text.".to_string()
            }),
        (!scenes.is_empty())
            .then(|| format!("Video scenes:\n{}", format_scenes_for_text(&scenes)))
            .unwrap_or_default(),
        (!keyframe_ocr_snippets.is_empty())
            .then(|| {
                format!(
                    "Keyframe OCR snippets:\n{}",
                    format_keyframe_ocr_for_text(&keyframe_ocr_snippets)
                )
            })
            .unwrap_or_default(),
        format!(
            "MiniMax media capability status:\n{}",
            summarize_minimax_media_capabilities(&minimax_capabilities)
        ),
    ]
    .into_iter()
    .filter(|part| !part.trim().is_empty())
    .collect::<Vec<_>>()
    .join("\n\n");

    ExtractedDocumentText {
        text,
        method: if parse_status == "transcribed" {
            "media-transcript".to_string()
        } else {
            "media-partial".to_string()
        },
        metadata: json!({
            "media": {
                "kind": media_kind,
                "parse_status": parse_status,
                "probe": probe,
                "transcript_extracted": parse_status == "transcribed",
                "transcript_source": transcript.as_ref().map(|item| item.source.clone()),
                "transcript_segments": transcript_segments,
                "scenes": scenes,
                "keyframe_ocr_snippets": keyframe_ocr_snippets,
                "provider_capabilities": minimax_capabilities,
                "provider_evidence": build_minimax_provider_evidence(&minimax_capabilities),
            }
        }),
    }
}

fn probe_media_metadata(path: &Path) -> Option<Value> {
    let path_arg = path.to_string_lossy().to_string();
    for command in direct_command_candidates("FFPROBE_BIN", "ffprobe") {
        if let Some(output) = run_text_command(
            &command,
            &[
                "-v",
                "error",
                "-print_format",
                "json",
                "-show_format",
                "-show_streams",
                path_arg.as_str(),
            ],
        ) {
            if let Ok(value) = serde_json::from_str::<Value>(&output) {
                return Some(value);
            }
        }
    }
    None
}

fn summarize_media_probe(probe: Option<&Value>) -> String {
    let Some(probe) = probe else {
        return String::new();
    };
    let mut lines = Vec::new();
    if let Some(format) = probe.get("format").and_then(Value::as_object) {
        if let Some(format_name) = format.get("format_name").and_then(Value::as_str) {
            lines.push(format!("Format: {format_name}"));
        }
        if let Some(duration) = format.get("duration").and_then(Value::as_str) {
            lines.push(format!("Duration seconds: {duration}"));
        }
        if let Some(size) = format.get("size").and_then(Value::as_str) {
            lines.push(format!("Size bytes: {size}"));
        }
    }
    if let Some(streams) = probe.get("streams").and_then(Value::as_array) {
        lines.extend(streams.iter().enumerate().filter_map(|(index, stream)| {
            let kind = stream.get("codec_type").and_then(Value::as_str)?;
            let codec = stream
                .get("codec_name")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            Some(format!("Stream {}: {} {}", index + 1, kind, codec))
        }));
    }
    lines.join("\n")
}

fn extract_media_transcript(path: &Path) -> Option<MediaTranscript> {
    let path_arg = path.to_string_lossy().to_string();
    if let Ok(command) = std::env::var("MEDIA_TRANSCRIBE_BIN") {
        let command = command.trim();
        if !command.is_empty() {
            if let Some(output) = run_text_command(command, &[path_arg.as_str()]) {
                if let Some(transcript) =
                    parse_media_transcript_output(&output, "MEDIA_TRANSCRIBE_BIN")
                {
                    return Some(transcript);
                }
            }
        }
    }
    None
}

fn extract_media_scenes(path: &Path) -> Vec<MediaScene> {
    let path_arg = path.to_string_lossy().to_string();
    if let Ok(command) = std::env::var("MEDIA_SCENE_BIN") {
        let command = command.trim();
        if !command.is_empty() {
            if let Some(output) = run_text_command(command, &[path_arg.as_str()]) {
                return parse_media_scenes_output(&output, "MEDIA_SCENE_BIN");
            }
        }
    }
    Vec::new()
}

fn extract_keyframe_ocr_snippets(path: &Path) -> Vec<MediaOcrSnippet> {
    let path_arg = path.to_string_lossy().to_string();
    if let Ok(command) = std::env::var("MEDIA_KEYFRAME_OCR_BIN") {
        let command = command.trim();
        if !command.is_empty() {
            if let Some(output) = run_text_command(command, &[path_arg.as_str()]) {
                return parse_keyframe_ocr_output(&output, "MEDIA_KEYFRAME_OCR_BIN");
            }
        }
    }
    Vec::new()
}

fn parse_media_transcript_output(output: &str, source: &str) -> Option<MediaTranscript> {
    if let Ok(value) = serde_json::from_str::<Value>(output) {
        let segments = value
            .get("segments")
            .or_else(|| value.get("transcript_segments"))
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| media_transcript_segment_from_value(item, source))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let text = value
            .get("text")
            .or_else(|| value.get("transcript"))
            .and_then(Value::as_str)
            .and_then(normalize_extracted_text)
            .or_else(|| join_segment_text(&segments));
        if let Some(text) = text {
            return Some(MediaTranscript {
                text,
                source: source.to_string(),
                segments: ensure_transcript_segments(segments, source),
            });
        }
    }

    let srt_segments = parse_srt_or_vtt_segments(output, source);
    if let Some(text) = join_segment_text(&srt_segments) {
        return Some(MediaTranscript {
            text,
            source: source.to_string(),
            segments: srt_segments,
        });
    }

    normalize_extracted_text(output).map(|text| MediaTranscript {
        segments: vec![MediaTranscriptSegment {
            start_seconds: None,
            end_seconds: None,
            text: text.clone(),
            source: source.to_string(),
            language: None,
            confidence: None,
        }],
        text,
        source: source.to_string(),
    })
}

fn media_transcript_segment_from_value(
    value: &Value,
    default_source: &str,
) -> Option<MediaTranscriptSegment> {
    let text = value
        .get("text")
        .or_else(|| value.get("content"))
        .and_then(Value::as_str)
        .and_then(normalize_extracted_text)?;
    Some(MediaTranscriptSegment {
        start_seconds: numeric_field(value, &["start_seconds", "start", "from"]),
        end_seconds: numeric_field(value, &["end_seconds", "end", "to"]),
        text,
        source: value
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or(default_source)
            .to_string(),
        language: value
            .get("language")
            .and_then(Value::as_str)
            .map(str::to_string),
        confidence: numeric_field(value, &["confidence", "score"]),
    })
}

fn ensure_transcript_segments(
    segments: Vec<MediaTranscriptSegment>,
    source: &str,
) -> Vec<MediaTranscriptSegment> {
    if !segments.is_empty() {
        return segments;
    }
    Vec::from([MediaTranscriptSegment {
        start_seconds: None,
        end_seconds: None,
        text: String::new(),
        source: source.to_string(),
        language: None,
        confidence: None,
    }])
    .into_iter()
    .filter(|segment| !segment.text.trim().is_empty())
    .collect()
}

fn join_segment_text(segments: &[MediaTranscriptSegment]) -> Option<String> {
    let text = segments
        .iter()
        .map(|segment| segment.text.trim())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    (!text.trim().is_empty()).then_some(text)
}

fn parse_srt_or_vtt_segments(output: &str, source: &str) -> Vec<MediaTranscriptSegment> {
    let normalized = output.replace("\r\n", "\n").replace('\r', "\n");
    let mut segments = Vec::new();
    let mut lines = normalized.lines().peekable();
    while let Some(line) = lines.next() {
        if !line.contains("-->") {
            continue;
        }
        let (start_seconds, end_seconds) = parse_timestamp_range(line);
        let mut text_lines = Vec::new();
        while let Some(next) = lines.peek().copied() {
            if next.trim().is_empty() {
                lines.next();
                break;
            }
            if next.contains("-->") {
                break;
            }
            text_lines.push(next.trim());
            lines.next();
        }
        let text = text_lines.join(" ");
        if let Some(text) = normalize_extracted_text(&text) {
            segments.push(MediaTranscriptSegment {
                start_seconds,
                end_seconds,
                text,
                source: source.to_string(),
                language: None,
                confidence: None,
            });
        }
    }
    segments
}

fn parse_timestamp_range(line: &str) -> (Option<f64>, Option<f64>) {
    let mut parts = line.split("-->");
    let start = parts.next().and_then(parse_media_timestamp);
    let end = parts
        .next()
        .map(|value| value.split_whitespace().next().unwrap_or(value))
        .and_then(parse_media_timestamp);
    (start, end)
}

fn parse_media_timestamp(value: &str) -> Option<f64> {
    let normalized = value.trim().replace(',', ".");
    let parts = normalized.split(':').collect::<Vec<_>>();
    match parts.as_slice() {
        [hours, minutes, seconds] => Some(
            hours.parse::<f64>().ok()? * 3600.0
                + minutes.parse::<f64>().ok()? * 60.0
                + seconds.parse::<f64>().ok()?,
        ),
        [minutes, seconds] => {
            Some(minutes.parse::<f64>().ok()? * 60.0 + seconds.parse::<f64>().ok()?)
        }
        [seconds] => seconds.parse::<f64>().ok(),
        _ => None,
    }
}

fn parse_media_scenes_output(output: &str, source: &str) -> Vec<MediaScene> {
    let Ok(value) = serde_json::from_str::<Value>(output) else {
        return Vec::new();
    };
    value
        .get("scenes")
        .or_else(|| value.get("scene_windows"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let summary = item
                        .get("summary")
                        .or_else(|| item.get("label"))
                        .and_then(Value::as_str)
                        .and_then(normalize_extracted_text)
                        .unwrap_or_else(|| "Scene".to_string());
                    Some(MediaScene {
                        start_seconds: numeric_field(item, &["start_seconds", "start"]),
                        end_seconds: numeric_field(item, &["end_seconds", "end"]),
                        representative_seconds: numeric_field(
                            item,
                            &["representative_seconds", "timestamp_seconds", "timestamp"],
                        ),
                        summary,
                        source: item
                            .get("source")
                            .and_then(Value::as_str)
                            .unwrap_or(source)
                            .to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_keyframe_ocr_output(output: &str, source: &str) -> Vec<MediaOcrSnippet> {
    let Ok(value) = serde_json::from_str::<Value>(output) else {
        return normalize_extracted_text(output)
            .map(|text| {
                vec![MediaOcrSnippet {
                    timestamp_seconds: None,
                    text,
                    source: source.to_string(),
                }]
            })
            .unwrap_or_default();
    };
    value
        .get("snippets")
        .or_else(|| value.get("ocr_snippets"))
        .or_else(|| value.get("keyframe_ocr_snippets"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let text = item
                        .get("text")
                        .or_else(|| item.get("content"))
                        .and_then(Value::as_str)
                        .and_then(normalize_extracted_text)?;
                    Some(MediaOcrSnippet {
                        timestamp_seconds: numeric_field(
                            item,
                            &["timestamp_seconds", "timestamp", "time"],
                        ),
                        text,
                        source: item
                            .get("source")
                            .and_then(Value::as_str)
                            .unwrap_or(source)
                            .to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn numeric_field(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|key| {
        value.get(*key).and_then(|item| {
            item.as_f64()
                .or_else(|| item.as_str().and_then(|text| text.parse::<f64>().ok()))
        })
    })
}

fn format_transcript_segments_for_text(segments: &[MediaTranscriptSegment]) -> String {
    segments
        .iter()
        .filter(|segment| !segment.text.trim().is_empty())
        .map(|segment| {
            format!(
                "- {} {}",
                format_media_time_window(segment.start_seconds, segment.end_seconds),
                segment.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_scenes_for_text(scenes: &[MediaScene]) -> String {
    scenes
        .iter()
        .map(|scene| {
            format!(
                "- {} {}",
                format_media_time_window(scene.start_seconds, scene.end_seconds),
                scene.summary
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_keyframe_ocr_for_text(snippets: &[MediaOcrSnippet]) -> String {
    snippets
        .iter()
        .map(|snippet| {
            format!(
                "- {} {}",
                snippet
                    .timestamp_seconds
                    .map(|seconds| format!("[{}]", format_media_timestamp(seconds)))
                    .unwrap_or_else(|| "[time unknown]".to_string()),
                snippet.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_media_time_window(start_seconds: Option<f64>, end_seconds: Option<f64>) -> String {
    match (start_seconds, end_seconds) {
        (Some(start), Some(end)) => {
            format!(
                "[{} - {}]",
                format_media_timestamp(start),
                format_media_timestamp(end)
            )
        }
        (Some(start), None) => format!("[{}]", format_media_timestamp(start)),
        _ => "[time unknown]".to_string(),
    }
}

fn format_media_timestamp(seconds: f64) -> String {
    let safe_seconds = seconds.max(0.0);
    let total = safe_seconds.floor() as u64;
    let millis = ((safe_seconds - total as f64) * 1000.0).round() as u64;
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let secs = total % 60;
    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{secs:02}.{millis:03}")
    } else {
        format!("{minutes:02}:{secs:02}.{millis:03}")
    }
}

fn summarize_minimax_media_capabilities(matrix: &MiniMaxMediaCapabilityMatrix) -> String {
    [
        &matrix.audio_transcript,
        &matrix.native_video_understanding,
        &matrix.keyframe_image_vlm,
    ]
    .into_iter()
    .map(|capability| {
        format!(
            "- {}: status={}, supported={}",
            capability.capability, capability.status, capability.supported
        )
    })
    .collect::<Vec<_>>()
    .join("\n")
}

fn build_minimax_provider_evidence(matrix: &MiniMaxMediaCapabilityMatrix) -> Value {
    json!([
        matrix.audio_transcript,
        matrix.native_video_understanding,
        matrix.keyframe_image_vlm,
    ])
}

fn extract_image_text_with_tesseract(path: &Path) -> Option<String> {
    let commands = direct_command_candidates("TESSERACT_BIN", "tesseract");
    let languages = tesseract_language_candidates();
    let psm_values = ["6", "3"];
    extract_image_text_with_tesseract_candidates(path, &commands, &languages, &psm_values)
}

fn extract_image_text_with_tesseract_candidates(
    path: &Path,
    commands: &[String],
    languages: &[String],
    psm_values: &[&str],
) -> Option<String> {
    let path_arg = path.to_string_lossy().to_string();
    let mut best_text = String::new();
    for command in commands {
        for language in languages {
            for psm in psm_values {
                let args = [
                    path_arg.as_str(),
                    "stdout",
                    "-l",
                    language.as_str(),
                    "--psm",
                    psm,
                ];
                if let Some(text) = run_text_command(command, &args) {
                    if text.chars().count() > best_text.chars().count() {
                        best_text = text;
                    }
                }
            }
        }
    }
    normalize_extracted_text(&best_text)
}

fn extract_docx_text(path: &Path) -> Option<String> {
    let xml = read_zip_entry_to_string(path, "word/document.xml")?;
    normalize_extracted_text(&decode_xml_entities(&strip_markup_tags(
        &xml.replace("</w:p>", "\n").replace("</w:tc>", "\t"),
    )))
}

fn extract_pptx_text(path: &Path) -> Option<String> {
    let mut archive = open_zip_archive(path)?;
    let mut slide_entries = Vec::new();
    for index in 0..archive.len() {
        let name = {
            let file = archive.by_index(index).ok()?;
            file.name().to_string()
        };
        if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
            slide_entries.push(name);
        }
    }
    slide_entries.sort();

    let mut slides = Vec::new();
    for name in slide_entries {
        let xml = read_zip_entry_from_archive(&mut archive, &name)?;
        let text = decode_xml_entities(&strip_markup_tags(
            &xml.replace("</a:p>", "\n").replace("</p:sp>", "\n"),
        ));
        if let Some(normalized) = normalize_extracted_text(&text) {
            slides.push(normalized);
        }
    }

    normalize_extracted_text(&slides.join("\n\n"))
}

fn extract_xlsx_text(path: &Path) -> Option<String> {
    let mut archive = open_zip_archive(path)?;
    let shared_strings = read_zip_entry_from_archive(&mut archive, "xl/sharedStrings.xml")
        .map(|xml| extract_shared_strings(&xml))
        .unwrap_or_default();

    let mut sheet_entries = Vec::new();
    for index in 0..archive.len() {
        let name = {
            let file = archive.by_index(index).ok()?;
            file.name().to_string()
        };
        if name.starts_with("xl/worksheets/sheet") && name.ends_with(".xml") {
            sheet_entries.push(name);
        }
    }
    sheet_entries.sort();

    let mut sheets = Vec::new();
    for (sheet_index, name) in sheet_entries.into_iter().enumerate() {
        let xml = read_zip_entry_from_archive(&mut archive, &name)?;
        if let Some(text) = extract_sheet_text(&xml, &shared_strings) {
            sheets.push(format!("# Sheet {}\n{}", sheet_index + 1, text));
        }
    }

    normalize_extracted_text(&sheets.join("\n\n"))
}

fn open_zip_archive(path: &Path) -> Option<ZipArchive<File>> {
    let file = File::open(path).ok()?;
    ZipArchive::new(file).ok()
}

fn read_zip_entry_to_string(path: &Path, entry_name: &str) -> Option<String> {
    let mut archive = open_zip_archive(path)?;
    read_zip_entry_from_archive(&mut archive, entry_name)
}

fn read_zip_entry_from_archive(archive: &mut ZipArchive<File>, entry_name: &str) -> Option<String> {
    let mut file = archive.by_name(entry_name).ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn extract_shared_strings(xml: &str) -> Vec<String> {
    split_xml_segments(xml, "si")
        .into_iter()
        .filter_map(|segment| {
            normalize_extracted_text(&decode_xml_entities(&strip_markup_tags(
                &segment.replace("</t>", " "),
            )))
        })
        .collect()
}

fn extract_sheet_text(xml: &str, shared_strings: &[String]) -> Option<String> {
    let rows = split_xml_segments(xml, "row")
        .into_iter()
        .filter_map(|row| {
            let cells = split_xml_elements(&row, "c")
                .into_iter()
                .filter_map(|cell| extract_cell_text(&cell, shared_strings))
                .collect::<Vec<_>>();
            (!cells.is_empty()).then(|| cells.join("\t"))
        })
        .collect::<Vec<_>>();

    normalize_extracted_text(&rows.join("\n"))
}

fn extract_cell_text(cell_xml: &str, shared_strings: &[String]) -> Option<String> {
    let cell_type_shared = cell_xml.contains("t=\"s\"") || cell_xml.contains("t='s'");
    let inline_text = first_xml_tag_text(cell_xml, "t")
        .map(|text| decode_xml_entities(&text))
        .and_then(|text| normalize_extracted_text(&text));
    if !cell_type_shared {
        return inline_text.or_else(|| {
            first_xml_tag_text(cell_xml, "v")
                .map(|text| decode_xml_entities(&text))
                .and_then(|text| normalize_extracted_text(&text))
        });
    }

    let index = first_xml_tag_text(cell_xml, "v")?
        .trim()
        .parse::<usize>()
        .ok()?;
    shared_strings.get(index).cloned()
}

fn split_xml_segments(xml: &str, tag: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut cursor = 0;
    let open_prefix = format!("<{tag}");
    let close_tag = format!("</{tag}>");
    while let Some(open_offset) = xml[cursor..].find(&open_prefix) {
        let open = cursor + open_offset;
        let Some(open_end_offset) = xml[open..].find('>') else {
            break;
        };
        let content_start = open + open_end_offset + 1;
        let Some(close_offset) = xml[content_start..].find(&close_tag) else {
            break;
        };
        let close = content_start + close_offset;
        segments.push(xml[content_start..close].to_string());
        cursor = close + close_tag.len();
    }
    segments
}

fn split_xml_elements(xml: &str, tag: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut cursor = 0;
    let open_prefix = format!("<{tag}");
    let close_tag = format!("</{tag}>");
    while let Some(open_offset) = xml[cursor..].find(&open_prefix) {
        let open = cursor + open_offset;
        let Some(open_end_offset) = xml[open..].find('>') else {
            break;
        };
        let content_start = open + open_end_offset + 1;
        let Some(close_offset) = xml[content_start..].find(&close_tag) else {
            break;
        };
        let close_end = content_start + close_offset + close_tag.len();
        segments.push(xml[open..close_end].to_string());
        cursor = close_end;
    }
    segments
}

fn first_xml_tag_text(xml: &str, tag: &str) -> Option<String> {
    split_xml_segments(xml, tag).into_iter().next()
}

fn extract_with_markitdown(path: &Path) -> Option<String> {
    let path_arg = path.to_string_lossy().to_string();
    for command in direct_command_candidates("MARKITDOWN_BIN", "markitdown") {
        if let Some(output) = run_text_command(&command, &[path_arg.as_str()]) {
            return Some(output);
        }
    }

    for command in python_command_candidates() {
        if let Some(output) = run_text_command(&command, &["-m", "markitdown", path_arg.as_str()]) {
            return Some(output);
        }
    }

    None
}

fn direct_command_candidates(env_name: &str, fallback: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Ok(configured) = std::env::var(env_name) {
        let configured = configured.trim();
        if !configured.is_empty() {
            candidates.push(configured.to_string());
        }
    }
    if !fallback.trim().is_empty() {
        candidates.push(fallback.to_string());
    }
    candidates.dedup();
    candidates
}

fn presentation_converter_candidates() -> Vec<String> {
    let mut candidates = direct_command_candidates("LIBREOFFICE_BIN", "soffice");
    candidates.push("libreoffice".to_string());
    candidates.dedup();
    candidates
}

fn python_command_candidates() -> Vec<String> {
    let mut candidates = Vec::new();
    if let Ok(configured) = std::env::var("PYTHON_BIN") {
        let configured = configured.trim();
        if !configured.is_empty() {
            candidates.push(configured.to_string());
        }
    }
    candidates.push("python3".to_string());
    candidates.push("python".to_string());
    candidates.dedup();
    candidates
}

fn tesseract_language_candidates() -> Vec<String> {
    let configured = [
        std::env::var("TESSERACT_LANGS").ok(),
        std::env::var("TESSERACT_LANG").ok(),
    ]
    .into_iter()
    .flatten()
    .flat_map(|value| {
        value
            .split(|ch: char| ch == ',' || ch.is_whitespace())
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>()
    });
    let mut candidates = configured.collect::<Vec<_>>();
    candidates.extend(
        ["chi_sim+eng", "chi_sim", "eng"]
            .into_iter()
            .map(str::to_string),
    );
    candidates.dedup();
    candidates
}

fn run_text_command(command: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(command).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout)
        .replace('\0', "")
        .trim()
        .to_string();
    (!text.is_empty()).then_some(text)
}

fn run_status_command(command: &str, args: &[&str]) -> bool {
    Command::new(command)
        .args(args)
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn run_status_command_with_timeout(command: &str, args: &[&str], timeout: Duration) -> bool {
    let mut child = match Command::new(command)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return false,
    };
    let start = Instant::now();
    loop {
        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return false;
        }
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

fn create_temp_dir(prefix: &str) -> std::io::Result<PathBuf> {
    let base = std::env::temp_dir();
    for attempt in 0..10 {
        let unique = format!(
            "{}-{}-{}-{}",
            prefix,
            std::process::id(),
            unix_timestamp_nanos(),
            attempt
        );
        let path = base.join(unique);
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "temporary directory collision",
    ))
}

fn unix_timestamp_nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

fn strip_markup_tags(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut inside_tag = false;
    for ch in text.chars() {
        match ch {
            '<' => inside_tag = true,
            '>' => {
                inside_tag = false;
                output.push(' ');
            }
            _ if !inside_tag => output.push(ch),
            _ => {}
        }
    }
    output
}

fn decode_xml_entities(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

fn normalize_extracted_text(text: &str) -> Option<String> {
    let normalized = text
        .replace('\0', "")
        .lines()
        .map(collapse_line_spaces_preserving_tabs)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    (!normalized.is_empty()).then_some(normalized)
}

fn collapse_line_spaces_preserving_tabs(line: &str) -> String {
    line.split('\t')
        .map(|part| part.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join("\t")
        .trim()
        .to_string()
}

fn split_long_text(text: &str, max_chars: usize) -> Vec<String> {
    let sentence_units = split_sentence_units(text);
    if sentence_units.len() > 1 {
        let mut chunks = Vec::new();
        let mut current = String::new();
        for unit in sentence_units {
            if unit.chars().count() > max_chars {
                if !current.trim().is_empty() {
                    chunks.push(current.trim().to_string());
                    current.clear();
                }
                chunks.extend(split_long_text_by_char(&unit, max_chars));
                continue;
            }
            if current.chars().count() + unit.chars().count() > max_chars
                && !current.trim().is_empty()
            {
                chunks.push(current.trim().to_string());
                current.clear();
            }
            current.push_str(&unit);
        }
        if !current.trim().is_empty() {
            chunks.push(current.trim().to_string());
        }
        return chunks;
    }
    split_long_text_by_char(text, max_chars)
}

fn split_sentence_units(text: &str) -> Vec<String> {
    let mut units = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        current.push(ch);
        if matches!(ch, '。' | '！' | '？' | '；' | '!' | '?' | ';' | '\n') {
            let unit = current.trim().to_string();
            if !unit.is_empty() {
                units.push(unit);
            }
            current.clear();
        }
    }
    let tail = current.trim().to_string();
    if !tail.is_empty() {
        units.push(tail);
    }
    units
}

fn split_long_text_by_char(text: &str, max_chars: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if current.chars().count() >= max_chars {
            chunks.push(current);
            current = String::new();
        }
        current.push(ch);
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        sync::Mutex,
        time::{SystemTime, UNIX_EPOCH},
    };
    use zip::{write::SimpleFileOptions, ZipWriter};

    static PADDLEOCR_ENV_LOCK: Mutex<()> = Mutex::new(());
    static PDF_PARSE_ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn local_ingest_processor_returns_stable_placeholder_chunks_without_file() {
        let processor = LocalIngestProcessor;
        let pdf_job = IngestJob {
            dataset_id: DatasetId::new(),
            document_id: DocumentId::new(),
            title: "Q1 PDF".to_string(),
            object_key: "missing.pdf".to_string(),
            content_type: "application/pdf".to_string(),
        };
        let markdown_job = IngestJob {
            dataset_id: DatasetId::new(),
            document_id: DocumentId::new(),
            title: "Notes".to_string(),
            object_key: "missing.md".to_string(),
            content_type: "text/markdown".to_string(),
        };

        assert_eq!(processor.process(&pdf_job).chunk_count(), 12);
        assert_eq!(processor.process(&markdown_job).chunk_count(), 4);
    }

    #[test]
    fn remote_media_ingest_is_disabled_by_default() {
        with_env_var("INGEST_REMOTE_MEDIA_ENABLED", None, || {
            let processor = LocalIngestProcessor;
            let outcome = processor.process(&IngestJob {
                dataset_id: DatasetId::new(),
                document_id: DocumentId::new(),
                title: "remote-course.mp4".to_string(),
                object_key: "https://cdn.example.com/course/lesson-01.mp4".to_string(),
                content_type: "video/mp4".to_string(),
            });

            assert!(outcome.used_placeholder);
            assert_eq!(outcome.parse_method, "placeholder");
        });
    }

    #[test]
    fn remote_media_url_guards_reject_private_and_login_gated_sources() {
        let private_url =
            reqwest::Url::parse("https://127.0.0.1/video.mp4").expect("private URL should parse");
        let wechat_url = reqwest::Url::parse("https://weixin.qq.com/sph/ActLMg4yTD.mp4")
            .expect("wechat URL should parse");
        let public_url = reqwest::Url::parse("https://cdn.example.com/course/lesson-01.mp4")
            .expect("public URL should parse");

        assert!(validate_remote_media_url(&private_url, "video/mp4").is_err());
        assert!(validate_remote_media_url(&wechat_url, "video/mp4").is_err());
        assert!(validate_remote_media_url(&public_url, "video/mp4").is_ok());
        assert!(is_remote_media_object_key(
            "https://cdn.example.com/course/lesson-01.mp4?token=redacted",
            "video/mp4"
        ));
    }

    #[test]
    fn remote_media_response_type_rejects_html_even_for_video_extension() {
        assert!(remote_media_response_type_allowed("video/mp4"));
        assert!(remote_media_response_type_allowed(
            "application/octet-stream"
        ));
        assert!(remote_media_response_type_allowed(""));
        assert!(!remote_media_response_type_allowed(
            "text/html; charset=utf-8"
        ));
        assert!(!remote_media_response_type_allowed("application/json"));
    }

    #[test]
    fn local_ingest_processor_extracts_real_markdown_text_from_object_key() {
        let file_path = write_temp_file(
            "real-upload.md",
            "# Title\n\nThis is a real uploaded document.",
        );
        let processor = LocalIngestProcessor;
        let outcome = processor.process(&IngestJob {
            dataset_id: DatasetId::new(),
            document_id: DocumentId::new(),
            title: "real-upload.md".to_string(),
            object_key: file_path.to_string_lossy().to_string(),
            content_type: "text/markdown".to_string(),
        });

        assert!(!outcome.used_placeholder);
        assert_eq!(outcome.parse_method, "local-text.md");
        assert_eq!(outcome.chunk_count(), 1);
        assert!(outcome.chunks[0].contains("real uploaded document"));
        let _ = fs::remove_file(file_path);
    }

    #[test]
    fn local_ingest_processor_extracts_simple_pdf_literal_text() {
        let file_path = write_temp_file(
            "simple.pdf",
            r#"%PDF-1.4
1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj
2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj
3 0 obj << /Type /Page /Parent 2 0 R /Contents 4 0 R >> endobj
4 0 obj << /Length 76 >> stream
BT /F1 12 Tf 72 720 Td (Quarterly Report) Tj T* (Orders grew 20%) Tj ET
endstream endobj
trailer << /Root 1 0 R >>
%%EOF"#,
        );

        let outcome = LocalIngestProcessor.process(&IngestJob {
            dataset_id: DatasetId::new(),
            document_id: DocumentId::new(),
            title: "simple.pdf".to_string(),
            object_key: file_path.to_string_lossy().to_string(),
            content_type: "application/pdf".to_string(),
        });

        let body = outcome.chunks.join("\n");
        assert!(!outcome.used_placeholder);
        assert!(outcome.parse_method.starts_with("pdf-"));
        assert!(body.contains("Quarterly Report"));
        assert!(body.contains("Orders grew 20%"));
        let _ = fs::remove_file(file_path);
    }

    #[test]
    fn pdf_parse_quality_marks_one_character_extract_as_low_coverage() {
        with_env_var("DOCUMENT_PDF_MIN_USABLE_TEXT_CHARS", Some("32"), || {
            assert_eq!(
                pdf_parse_quality("字"),
                PdfParseQuality::LowTextCoverage { text_chars: 1 }
            );
            assert_eq!(
                pdf_parse_quality(&"正".repeat(32)),
                PdfParseQuality::Usable { text_chars: 32 }
            );
        });
    }

    #[test]
    fn pdf_low_quality_diagnostic_does_not_treat_single_character_as_content() {
        with_env_var("DOCUMENT_PDF_MIN_USABLE_TEXT_CHARS", Some("32"), || {
            let extracted = with_pdf_parse_quality_metadata(
                extracted_text("字", "pdf-python"),
                "low_text_coverage",
                1,
                None,
            );

            let diagnostic = pdf_low_quality_diagnostic_text(extracted);

            assert_eq!(diagnostic.method, "pdf-python+low-quality");
            assert!(diagnostic.text.contains("PDF parse quality warning"));
            assert!(diagnostic
                .text
                .contains("do not treat the low-quality extract"));
            assert_eq!(
                diagnostic.metadata["parse_quality"]["status"],
                json!("low_text_coverage_fallback_unavailable")
            );
            assert_eq!(diagnostic.metadata["parse_quality"]["text_chars"], json!(1));
        });
    }

    #[test]
    fn pdf_candidate_selection_prefers_structured_paddleocr_when_text_is_similar() {
        with_env_var("DOCUMENT_PDF_MIN_USABLE_TEXT_CHARS", Some("32"), || {
            let native_text = "Plain resume text ".repeat(12);
            let paddle_text =
                "# 工作经历\n\n广东高明中港城商业管理有限公司\n\n| 时间 | 公司 |\n| --- | --- |\n";
            let mut usable = Vec::new();
            let mut low_quality = Vec::new();
            classify_pdf_candidate(
                extracted_text(native_text, "pdf-pdftotext"),
                &mut usable,
                &mut low_quality,
            );
            classify_pdf_candidate(
                ExtractedDocumentText {
                    text: paddle_text.to_string(),
                    method: "pdf-paddleocr".to_string(),
                    metadata: json!({
                        "document_structure": {
                            "source": "paddleocr_pp_structure_v3",
                            "block_count": 8,
                        }
                    }),
                },
                &mut usable,
                &mut low_quality,
            );

            let selected =
                select_pdf_usable_candidate(&mut usable).expect("usable candidate should select");

            assert_eq!(selected.method, "pdf-paddleocr");
            assert_eq!(
                selected.metadata["parse_quality"]["candidate_selection"]["selected_method"],
                json!("pdf-paddleocr")
            );
            assert_eq!(
                selected.metadata["parse_quality"]["candidate_selection"]["candidates"]
                    .as_array()
                    .map(Vec::len),
                Some(2)
            );
        });
    }

    #[test]
    fn pdf_candidate_selection_prefers_much_longer_native_text() {
        with_env_var("DOCUMENT_PDF_MIN_USABLE_TEXT_CHARS", Some("32"), || {
            let mut usable = Vec::new();
            let mut low_quality = Vec::new();
            classify_pdf_candidate(
                ExtractedDocumentText {
                    text: format!("# 简历\n\n{}", "短文本".repeat(12)),
                    method: "pdf-paddleocr".to_string(),
                    metadata: json!({
                        "document_structure": {
                            "source": "paddleocr_pp_structure_v3",
                            "block_count": 5,
                        }
                    }),
                },
                &mut usable,
                &mut low_quality,
            );
            classify_pdf_candidate(
                extracted_text(
                    "Full native text with work history and company names. ".repeat(30),
                    "pdf-pdftotext",
                ),
                &mut usable,
                &mut low_quality,
            );

            let selected =
                select_pdf_usable_candidate(&mut usable).expect("usable candidate should select");

            assert_eq!(selected.method, "pdf-pdftotext");
            assert_eq!(
                selected.metadata["parse_quality"]["candidate_selection"]["selected_method"],
                json!("pdf-pdftotext")
            );
        });
    }

    #[test]
    fn pdf_low_quality_candidate_selection_keeps_best_diagnostic_source() {
        with_env_var("DOCUMENT_PDF_MIN_USABLE_TEXT_CHARS", Some("32"), || {
            let first = with_pdf_parse_quality_metadata(
                extracted_text("字", "pdf-paddleocr"),
                "low_text_coverage",
                1,
                None,
            );
            let second = with_pdf_parse_quality_metadata(
                extracted_text("短文本但比单字更多", "pdf-python"),
                "low_text_coverage",
                8,
                None,
            );

            let selected = select_pdf_low_quality_candidate(vec![first, second])
                .expect("low quality candidate should select");

            assert_eq!(selected.method, "pdf-python");
        });
    }

    #[test]
    fn pdf_vlm_rescue_targets_only_weak_unstructured_text() {
        with_pdf_parse_env(
            &[
                ("DOCUMENT_PDF_VLM_WEAK_TEXT_MAX_CHARS", Some("128")),
                ("DOCUMENT_PDF_VLM_FALLBACK_ON_WEAK_TEXT", None),
            ],
            || {
                let weak = extracted_text("plain resume text ".repeat(5), "pdf-python");
                assert!(pdf_candidate_should_try_vlm_rescue(&weak));

                let structured = ExtractedDocumentText {
                    text: "plain resume text ".repeat(5),
                    method: "pdf-paddleocr".to_string(),
                    metadata: json!({
                        "document_structure": {
                            "block_count": 3,
                        }
                    }),
                };
                assert!(!pdf_candidate_should_try_vlm_rescue(&structured));

                let long = extracted_text("complete resume text ".repeat(40), "pdf-pdftotext");
                assert!(!pdf_candidate_should_try_vlm_rescue(&long));
            },
        );
    }

    #[test]
    fn pdf_vlm_rescue_can_be_disabled() {
        with_pdf_parse_env(
            &[("DOCUMENT_PDF_VLM_FALLBACK_ON_WEAK_TEXT", Some("false"))],
            || {
                let weak = extracted_text("plain resume text ".repeat(5), "pdf-python");
                assert!(!pdf_candidate_should_try_vlm_rescue(&weak));
            },
        );
    }

    #[test]
    fn pdf_vlm_rescue_selection_prefers_better_vlm_output() {
        let existing = with_pdf_parse_quality_metadata(
            extracted_text("plain resume text ".repeat(5), "pdf-python"),
            "usable_text",
            80,
            None,
        );
        let vlm = with_pdf_parse_quality_metadata(
            extracted_text(
                "# Page 1\n\n广东高明中港城商业管理有限公司\n\n工作经历和岗位职责。".repeat(8),
                "pdf-vlm",
            ),
            "vlm_fallback_used",
            240,
            Some(&existing),
        );

        let selected = select_pdf_vlm_rescue_candidate(existing, vlm);

        assert_eq!(selected.method, "pdf-vlm");
        assert_eq!(
            selected.metadata["parse_quality"]["vlm_rescue"]["selected"],
            json!("vlm")
        );
    }

    #[test]
    fn pdf_vlm_rescue_selection_keeps_existing_when_vlm_is_weaker() {
        let existing = with_pdf_parse_quality_metadata(
            extracted_text(
                "plain resume text with useful extracted content ".repeat(20),
                "pdf-python",
            ),
            "usable_text",
            900,
            None,
        );
        let vlm = with_pdf_parse_quality_metadata(
            extracted_text("短", "pdf-vlm"),
            "vlm_fallback_used",
            1,
            Some(&existing),
        );

        let selected = select_pdf_vlm_rescue_candidate(existing, vlm);

        assert_eq!(selected.method, "pdf-python");
        assert_eq!(
            selected.metadata["parse_quality"]["vlm_rescue"]["selected"],
            json!("existing")
        );
    }

    #[test]
    fn ingest_outcome_derives_model_visible_parse_status() {
        let degraded = IngestOutcome {
            chunks: vec!["PDF parse quality warning".to_string()],
            inferred_title: None,
            parse_method: "pdf-python+low-quality".to_string(),
            extracted_chars: 25,
            used_placeholder: false,
            metadata: json!({
                "parse_quality": {
                    "status": "low_text_coverage_fallback_unavailable"
                }
            }),
        };
        assert_eq!(degraded.parse_status(), "parse_degraded");
        assert_eq!(
            degraded.parse_quality_status().as_deref(),
            Some("low_text_coverage_fallback_unavailable")
        );

        let vlm = IngestOutcome {
            chunks: vec!["VLM extracted page".to_string()],
            inferred_title: None,
            parse_method: "pdf-vlm".to_string(),
            extracted_chars: 18,
            used_placeholder: false,
            metadata: json!({
                "parse_quality": {
                    "status": "vlm_fallback_used"
                }
            }),
        };
        assert_eq!(vlm.parse_status(), "parsed_with_vlm_fallback");
        assert_eq!(vlm.cloud_structured_provider(), "minimax");

        let media = IngestOutcome {
            chunks: vec!["Transcript was not extracted".to_string()],
            inferred_title: None,
            parse_method: "media-partial".to_string(),
            extracted_chars: 32,
            used_placeholder: false,
            metadata: json!({
                "media": {
                    "parse_status": "partial"
                }
            }),
        };
        assert_eq!(media.parse_status(), "partial");
    }

    #[test]
    fn paddleocr_disabled_by_default() {
        with_paddleocr_env(
            &[
                ("DOCUMENT_PADDLEOCR_ENABLED", None),
                ("DOCUMENT_PDF_PARSE_ENGINE", None),
                ("DOCUMENT_PADDLEOCR_PYTHON_BIN", None),
            ],
            || {
                assert!(extract_pdf_with_paddleocr(Path::new("missing.pdf")).is_none());
            },
        );
    }

    #[test]
    fn paddleocr_enabled_by_dedicated_runtime_config() {
        with_paddleocr_env(
            &[
                ("DOCUMENT_PADDLEOCR_ENABLED", None),
                ("DOCUMENT_PDF_PARSE_ENGINE", None),
                (
                    "DOCUMENT_PADDLEOCR_PYTHON_BIN",
                    Some("/opt/paddle/bin/python"),
                ),
            ],
            || {
                assert!(document_paddleocr_enabled());
            },
        );
    }

    #[test]
    fn paddleocr_explicit_false_overrides_dedicated_runtime_config() {
        with_paddleocr_env(
            &[
                ("DOCUMENT_PADDLEOCR_ENABLED", Some("false")),
                ("DOCUMENT_PDF_PARSE_ENGINE", Some("paddleocr_first")),
                (
                    "DOCUMENT_PADDLEOCR_PYTHON_BIN",
                    Some("/opt/paddle/bin/python"),
                ),
            ],
            || {
                assert!(!document_paddleocr_enabled());
            },
        );
    }

    #[test]
    fn paddleocr_native_first_engine_disables_auto_runtime_config() {
        with_paddleocr_env(
            &[
                ("DOCUMENT_PADDLEOCR_ENABLED", None),
                ("DOCUMENT_PDF_PARSE_ENGINE", Some("native_first")),
                (
                    "DOCUMENT_PADDLEOCR_PYTHON_BIN",
                    Some("/opt/paddle/bin/python"),
                ),
            ],
            || {
                assert!(!document_paddleocr_enabled());
            },
        );
    }

    #[test]
    fn paddleocr_python_candidates_prefer_dedicated_bin() {
        with_paddleocr_env(
            &[(
                "DOCUMENT_PADDLEOCR_PYTHON_BIN",
                Some("/opt/paddle/bin/python"),
            )],
            || {
                let candidates = paddleocr_python_command_candidates();

                assert_eq!(
                    candidates.first().map(String::as_str),
                    Some("/opt/paddle/bin/python")
                );
                assert!(candidates.iter().any(|candidate| candidate == "python3"));
            },
        );
    }

    #[test]
    fn paddleocr_payload_keeps_structured_markdown_and_blocks() {
        let extracted = paddleocr_payload_to_extracted_text(
            json!({
                "ok": true,
                "markdown": "# Page 1\n\n## 工作经历\n\n广东高明中港城商业管理有限公司\n\n- 运营经理",
                "page_count": 1,
                "block_count": 2,
                "blocks": [
                    {
                        "page_number": 1,
                        "type": "paragraph_title",
                        "text": "工作经历",
                        "bbox": [12, 24, 160, 48],
                        "confidence": 0.97
                    },
                    {
                        "page_number": 1,
                        "type": "text",
                        "text": "广东高明中港城商业管理有限公司"
                    }
                ],
                "errors": []
            }),
            8,
        )
        .expect("structured PaddleOCR payload should become extracted text");

        assert_eq!(extracted.method, "pdf-paddleocr");
        assert!(extracted.text.contains("广东高明中港城商业管理有限公司"));
        assert_eq!(
            extracted.metadata["document_structure"]["source"],
            json!("paddleocr_pp_structure_v3")
        );
        assert_eq!(
            extracted.metadata["document_structure"]["blocks"][0]["type"],
            json!("paragraph_title")
        );
        assert_eq!(extracted.metadata["paddleocr"]["max_pages"], json!(8));
        assert_eq!(extracted.metadata["paddleocr"]["config"], json!({}));
    }

    #[test]
    fn paddleocr_payload_with_one_character_stays_low_quality() {
        with_env_var("DOCUMENT_PDF_MIN_USABLE_TEXT_CHARS", Some("32"), || {
            let extracted = paddleocr_payload_to_extracted_text(
                json!({
                    "ok": true,
                    "markdown": "字",
                    "page_count": 1,
                    "block_count": 1,
                    "blocks": [{"page_number": 1, "text": "字"}],
                }),
                8,
            )
            .expect("single character payload is still parseable before quality gate");

            assert_eq!(
                pdf_parse_quality(&extracted.text),
                PdfParseQuality::LowTextCoverage { text_chars: 1 }
            );
        });
    }

    #[test]
    fn local_ingest_processor_marks_image_ocr_empty_without_placeholder() {
        with_env_var("DOCUMENT_IMAGE_PARSE_MODE", Some("ocr-only"), || {
            let file_path = write_temp_file("uploaded-screenshot", "not a real image");

            let outcome = LocalIngestProcessor.process(&IngestJob {
                dataset_id: DatasetId::new(),
                document_id: DocumentId::new(),
                title: "uploaded-screenshot".to_string(),
                object_key: file_path.to_string_lossy().to_string(),
                content_type: "image/png".to_string(),
            });

            let body = outcome.chunks.join("\n");
            assert!(!outcome.used_placeholder);
            assert_eq!(outcome.parse_method, "image-ocr-empty");
            assert!(body.contains("Image file:"));
            assert!(body.contains("OCR text was not extracted"));
            let _ = fs::remove_file(file_path);
        });
    }

    #[test]
    fn local_ingest_processor_does_not_call_vlm_when_disabled() {
        with_env_var("DOCUMENT_IMAGE_PARSE_MODE", Some("ocr-only"), || {
            let file_path = write_temp_file("uploaded-screenshot", "not a real image");

            let outcome = LocalIngestProcessor.process(&IngestJob {
                dataset_id: DatasetId::new(),
                document_id: DocumentId::new(),
                title: "uploaded-screenshot".to_string(),
                object_key: file_path.to_string_lossy().to_string(),
                content_type: "image/png".to_string(),
            });

            assert_eq!(outcome.parse_method, "image-ocr-empty");
            let _ = fs::remove_file(file_path);
        });
    }

    #[test]
    fn image_vlm_metadata_preserves_structured_payload() {
        let metadata = build_image_vlm_metadata(
            "MiniMax-M2.5-highspeed".to_string(),
            DocumentImageVlmPayload {
                summary: "高明中港城客流截图".to_string(),
                visual_summary: "图片展示商场分区客流驾驶舱。".to_string(),
                transcribed_text: "A区 2180 人次".to_string(),
                ..Default::default()
            },
        );

        assert_eq!(metadata["vlm"]["provider"], json!("minimax"));
        assert_eq!(metadata["vlm"]["model"], json!("MiniMax-M2.5-highspeed"));
        assert_eq!(
            metadata["vlm"]["payload"]["summary"],
            json!("高明中港城客流截图")
        );
        assert_eq!(
            metadata["vlm"]["payload"]["transcribedText"],
            json!("A区 2180 人次")
        );
    }

    #[test]
    fn local_ingest_processor_marks_audio_without_fake_transcript() {
        with_env_var("MEDIA_TRANSCRIBE_BIN", None, || {
            let file_path = write_temp_file("call-recording.mp3", "not a real audio file");

            let outcome = LocalIngestProcessor.process(&IngestJob {
                dataset_id: DatasetId::new(),
                document_id: DocumentId::new(),
                title: "call-recording.mp3".to_string(),
                object_key: file_path.to_string_lossy().to_string(),
                content_type: "audio/mpeg".to_string(),
            });

            let body = outcome.chunks.join("\n");
            assert!(!outcome.used_placeholder);
            assert_eq!(outcome.parse_method, "media-partial");
            assert!(body.contains("Media type: audio"));
            assert!(body.contains("Transcript was not extracted"));
            assert_eq!(outcome.metadata["media"]["kind"], json!("audio"));
            assert_eq!(outcome.metadata["media"]["parse_status"], json!("partial"));
            assert_eq!(
                outcome.metadata["media"]["transcript_extracted"],
                json!(false)
            );
            let _ = fs::remove_file(file_path);
        });
    }

    #[test]
    fn media_transcript_parser_keeps_json_segments_with_timestamps() {
        let transcript = parse_media_transcript_output(
            r#"{
                "segments": [
                    {"start": 1.5, "end": 3.25, "text": "客户询问订单状态", "language": "zh", "confidence": 0.91},
                    {"start": 4, "end": 5.5, "text": "客服承诺当天反馈"}
                ]
            }"#,
            "fake-transcriber",
        )
        .expect("json transcript should parse");

        assert_eq!(transcript.segments.len(), 2);
        assert_eq!(transcript.segments[0].start_seconds, Some(1.5));
        assert_eq!(transcript.segments[0].end_seconds, Some(3.25));
        assert!(transcript.text.contains("客户询问订单状态"));
        assert!(format_transcript_segments_for_text(&transcript.segments).contains("00:01.500"));
    }

    #[test]
    fn media_transcript_parser_accepts_srt_windows() {
        let transcript = parse_media_transcript_output(
            "1\n00:00:01,000 --> 00:00:02,500\n第一句话\n\n2\n00:00:03,000 --> 00:00:04,000\n第二句话",
            "fake-srt",
        )
        .expect("srt transcript should parse");

        assert_eq!(transcript.segments.len(), 2);
        assert_eq!(transcript.segments[0].start_seconds, Some(1.0));
        assert_eq!(transcript.segments[0].end_seconds, Some(2.5));
        assert!(transcript.text.contains("第二句话"));
    }

    #[test]
    fn media_scene_and_keyframe_ocr_parsers_keep_timestamp_metadata() {
        let scenes = parse_media_scenes_output(
            r#"{"scenes":[{"start_seconds":0,"end_seconds":12.4,"representative_seconds":6,"summary":"门店入口画面"}]}"#,
            "fake-scenes",
        );
        let snippets = parse_keyframe_ocr_output(
            r#"{"snippets":[{"timestamp_seconds":6,"text":"今日客流 2180","source":"fake-ocr"}]}"#,
            "fake-ocr",
        );

        assert_eq!(scenes.len(), 1);
        assert_eq!(scenes[0].representative_seconds, Some(6.0));
        assert_eq!(snippets.len(), 1);
        assert_eq!(snippets[0].timestamp_seconds, Some(6.0));
        assert_eq!(snippets[0].text, "今日客流 2180");
    }

    #[test]
    fn split_text_chunks_splits_long_unicode_text_on_char_boundaries() {
        let chunks = split_text_chunks("订单分析很好。客服反馈也很好。", 8);

        assert!(chunks.len() > 1);
        assert!(chunks.iter().all(|chunk| chunk.chars().count() <= 8));
    }

    #[test]
    fn split_text_chunks_preserves_blank_line_paragraph_boundaries() {
        let chunks = split_text_chunks(
            "第一段介绍订单延期风险。\n\n第二段介绍客服满意度。\n\n第三段介绍库存周转。",
            16,
        );

        assert_eq!(
            chunks,
            vec![
                "第一段介绍订单延期风险。",
                "第二段介绍客服满意度。",
                "第三段介绍库存周转。"
            ]
        );
    }

    #[test]
    fn split_text_paragraphs_keeps_wrapped_lines_in_same_paragraph() {
        let paragraphs = split_text_paragraphs(
            "采购制度第一行\n延续同一个段落\n\n审批流程第一行\n审批流程第二行",
        );

        assert_eq!(
            paragraphs,
            vec![
                "采购制度第一行\n延续同一个段落",
                "审批流程第一行\n审批流程第二行"
            ]
        );
    }

    #[test]
    fn local_ingest_processor_extracts_docx_xml_text() {
        let file_path = write_temp_zip(
            "sample.docx",
            &[(
                "word/document.xml",
                r#"<w:document><w:body><w:p><w:r><w:t>客户说明</w:t></w:r></w:p><w:p><w:r><w:t>订单增长 20%</w:t></w:r></w:p></w:body></w:document>"#,
            )],
        );

        let outcome = LocalIngestProcessor.process(&IngestJob {
            dataset_id: DatasetId::new(),
            document_id: DocumentId::new(),
            title: "sample.docx".to_string(),
            object_key: file_path.to_string_lossy().to_string(),
            content_type: "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                .to_string(),
        });

        assert!(!outcome.used_placeholder);
        assert_eq!(outcome.parse_method, "docx-ooxml");
        assert!(outcome.chunks.join("\n").contains("客户说明"));
        assert!(outcome.chunks.join("\n").contains("订单增长 20%"));
        let _ = fs::remove_file(file_path);
    }

    #[test]
    fn local_ingest_processor_extracts_xlsx_shared_strings_and_values() {
        let file_path = write_temp_zip(
            "orders.xlsx",
            &[
                (
                    "xl/sharedStrings.xml",
                    r#"<sst><si><t>订单号</t></si><si><t>金额</t></si><si><t>A001</t></si></sst>"#,
                ),
                (
                    "xl/worksheets/sheet1.xml",
                    r#"<worksheet><sheetData><row r="1"><c t="s"><v>0</v></c><c t="s"><v>1</v></c></row><row r="2"><c t="s"><v>2</v></c><c><v>1280</v></c></row></sheetData></worksheet>"#,
                ),
            ],
        );

        let outcome = LocalIngestProcessor.process(&IngestJob {
            dataset_id: DatasetId::new(),
            document_id: DocumentId::new(),
            title: "orders.xlsx".to_string(),
            object_key: file_path.to_string_lossy().to_string(),
            content_type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
                .to_string(),
        });

        let body = outcome.chunks.join("\n");
        assert!(!outcome.used_placeholder);
        assert_eq!(outcome.parse_method, "xlsx-ooxml");
        assert!(body.contains("订单号\t金额"));
        assert!(body.contains("A001\t1280"));
        let _ = fs::remove_file(file_path);
    }

    #[test]
    fn local_ingest_processor_extracts_pptx_slide_text() {
        let file_path = write_temp_zip(
            "deck.pptx",
            &[(
                "ppt/slides/slide1.xml",
                r#"<p:sld><p:cSld><p:spTree><p:sp><p:txBody><a:p><a:r><a:t>经营看板</a:t></a:r></a:p><a:p><a:r><a:t>客服风险下降</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>"#,
            )],
        );

        let outcome = LocalIngestProcessor.process(&IngestJob {
            dataset_id: DatasetId::new(),
            document_id: DocumentId::new(),
            title: "deck.pptx".to_string(),
            object_key: file_path.to_string_lossy().to_string(),
            content_type:
                "application/vnd.openxmlformats-officedocument.presentationml.presentation"
                    .to_string(),
        });

        let body = outcome.chunks.join("\n");
        assert!(!outcome.used_placeholder);
        assert_eq!(outcome.parse_method, "pptx-ooxml");
        assert!(body.contains("经营看板"));
        assert!(body.contains("客服风险下降"));
        let _ = fs::remove_file(file_path);
    }

    fn write_temp_file(name: &str, content: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be available")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{suffix}-{name}"));
        fs::write(&path, content).expect("temp file should be written");
        path
    }

    fn write_temp_zip(name: &str, entries: &[(&str, &str)]) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be available")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{suffix}-{name}"));
        let file = File::create(&path).expect("zip file should be created");
        let mut writer = ZipWriter::new(file);
        for (entry_name, content) in entries {
            writer
                .start_file(*entry_name, SimpleFileOptions::default())
                .expect("zip entry should start");
            writer
                .write_all(content.as_bytes())
                .expect("zip entry should be written");
        }
        writer.finish().expect("zip should finish");
        path
    }

    fn with_env_var<T>(name: &str, value: Option<&str>, run: impl FnOnce() -> T) -> T {
        let previous = std::env::var(name).ok();
        match value {
            Some(value) => std::env::set_var(name, value),
            None => std::env::remove_var(name),
        }
        let result = run();
        match previous {
            Some(previous) => std::env::set_var(name, previous),
            None => std::env::remove_var(name),
        }
        result
    }

    struct EnvVarRestore(Vec<(String, Option<String>)>);

    impl Drop for EnvVarRestore {
        fn drop(&mut self) {
            for (name, value) in self.0.drain(..) {
                match value {
                    Some(value) => std::env::set_var(name, value),
                    None => std::env::remove_var(name),
                }
            }
        }
    }

    fn with_pdf_parse_env<T>(updates: &[(&str, Option<&str>)], run: impl FnOnce() -> T) -> T {
        let _guard = PDF_PARSE_ENV_LOCK
            .lock()
            .expect("pdf parse env lock should not be poisoned");
        let previous = updates
            .iter()
            .map(|(name, _)| ((*name).to_string(), std::env::var(name).ok()))
            .collect::<Vec<_>>();

        for (name, value) in updates {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }

        let _restore = EnvVarRestore(previous);
        run()
    }

    fn with_paddleocr_env<T>(updates: &[(&str, Option<&str>)], run: impl FnOnce() -> T) -> T {
        let _guard = PADDLEOCR_ENV_LOCK
            .lock()
            .expect("paddleocr env lock should not be poisoned");
        let previous = updates
            .iter()
            .map(|(name, _)| ((*name).to_string(), std::env::var(name).ok()))
            .collect::<Vec<_>>();

        for (name, value) in updates {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }

        let _restore = EnvVarRestore(previous);
        run()
    }
}
