use anyhow::{anyhow, Result};
use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{fs, path::Path, time::Duration};

const DEFAULT_MINIMAX_BASE_URL: &str = "https://api.minimaxi.com/v1";
const DEFAULT_MINIMAX_MODEL: &str = "MiniMax-M2.5-highspeed";
const DEFAULT_MINIMAX_MEDIA_MODEL: &str = "MiniMax-M2.5-highspeed";
const DEFAULT_MAX_IMAGE_BYTES: u64 = 20_000_000;
const DEFAULT_TIMEOUT_SECS: u64 = 60;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocumentImageParseMode {
    OcrOnly,
    OcrPlusVlm,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocumentImageVlmProviderMode {
    Disabled,
    MiniMax,
}

#[derive(Clone, Debug)]
pub struct DocumentImageVlmConfig {
    pub parse_mode: DocumentImageParseMode,
    pub provider_mode: DocumentImageVlmProviderMode,
    pub api_key: Option<String>,
    pub base_url: String,
    pub model: String,
    pub max_image_bytes: u64,
    pub timeout_secs: u64,
}

impl DocumentImageVlmConfig {
    pub fn from_env() -> Self {
        Self {
            parse_mode: resolve_parse_mode(env("DOCUMENT_IMAGE_PARSE_MODE").as_deref()),
            provider_mode: resolve_provider_mode(env("DOCUMENT_IMAGE_VLM_PROVIDER").as_deref()),
            api_key: env("MINIMAX_API_KEY").or_else(|| env("MINIMAX_CN_API_KEY")),
            base_url: env("MINIMAX_BASE_URL")
                .or_else(|| env("MINIMAX_CN_BASE_URL"))
                .unwrap_or_else(|| DEFAULT_MINIMAX_BASE_URL.to_string()),
            model: env("DOCUMENT_IMAGE_VLM_MODEL")
                .or_else(|| env("MINIMAX_IMAGE_MODEL"))
                .unwrap_or_else(|| DEFAULT_MINIMAX_MODEL.to_string()),
            max_image_bytes: env("DOCUMENT_IMAGE_VLM_MAX_IMAGE_BYTES")
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(DEFAULT_MAX_IMAGE_BYTES)
                .max(1024 * 1024),
            timeout_secs: env("DOCUMENT_IMAGE_VLM_TIMEOUT_SECS")
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(DEFAULT_TIMEOUT_SECS)
                .max(1),
        }
    }

    pub fn available(&self) -> bool {
        self.parse_mode == DocumentImageParseMode::OcrPlusVlm
            && self.provider_mode == DocumentImageVlmProviderMode::MiniMax
            && self
                .api_key
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MiniMaxMediaCapabilityConfig {
    pub provider: String,
    pub prefer_minimax: bool,
    pub api_key: Option<String>,
    pub base_url: String,
    pub media_model: String,
    pub audio_transcript_endpoint: Option<String>,
    pub video_understanding_endpoint: Option<String>,
    pub audio_probe_verified: bool,
    pub video_probe_verified: bool,
    pub image_vlm_available: bool,
}

impl MiniMaxMediaCapabilityConfig {
    pub fn from_env() -> Self {
        let image_vlm_config = DocumentImageVlmConfig::from_env();
        Self {
            provider: env("MEDIA_PARSE_PROVIDER").unwrap_or_else(|| "local".to_string()),
            prefer_minimax: parse_bool_env("MEDIA_PARSE_PREFER_MINIMAX"),
            api_key: env("MINIMAX_API_KEY").or_else(|| env("MINIMAX_CN_API_KEY")),
            base_url: env("MINIMAX_BASE_URL")
                .or_else(|| env("MINIMAX_CN_BASE_URL"))
                .unwrap_or_else(|| DEFAULT_MINIMAX_BASE_URL.to_string()),
            media_model: env("MINIMAX_MEDIA_MODEL")
                .or_else(|| env("MINIMAX_AUDIO_MODEL"))
                .or_else(|| env("MINIMAX_VIDEO_MODEL"))
                .unwrap_or_else(|| DEFAULT_MINIMAX_MEDIA_MODEL.to_string()),
            audio_transcript_endpoint: env("MINIMAX_MEDIA_TRANSCRIBE_ENDPOINT")
                .or_else(|| env("MINIMAX_AUDIO_TRANSCRIBE_ENDPOINT")),
            video_understanding_endpoint: env("MINIMAX_MEDIA_VIDEO_ENDPOINT")
                .or_else(|| env("MINIMAX_VIDEO_UNDERSTANDING_ENDPOINT")),
            audio_probe_verified: parse_bool_env("MINIMAX_MEDIA_AUDIO_PROBE_VERIFIED")
                || parse_bool_env("MINIMAX_AUDIO_TRANSCRIBE_PROBE_VERIFIED"),
            video_probe_verified: parse_bool_env("MINIMAX_MEDIA_VIDEO_PROBE_VERIFIED")
                || parse_bool_env("MINIMAX_VIDEO_UNDERSTANDING_PROBE_VERIFIED"),
            image_vlm_available: image_vlm_config.available(),
        }
    }

    fn minimax_requested(&self) -> bool {
        self.prefer_minimax || self.provider.trim().eq_ignore_ascii_case("minimax")
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct MiniMaxMediaCapability {
    pub capability: String,
    pub provider: String,
    pub model: String,
    pub status: String,
    pub supported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct MiniMaxMediaCapabilityMatrix {
    pub provider: String,
    pub audio_transcript: MiniMaxMediaCapability,
    pub native_video_understanding: MiniMaxMediaCapability,
    pub keyframe_image_vlm: MiniMaxMediaCapability,
}

pub fn probe_minimax_media_capabilities_from_env() -> MiniMaxMediaCapabilityMatrix {
    probe_minimax_media_capabilities(&MiniMaxMediaCapabilityConfig::from_env())
}

pub fn probe_minimax_media_capabilities(
    config: &MiniMaxMediaCapabilityConfig,
) -> MiniMaxMediaCapabilityMatrix {
    MiniMaxMediaCapabilityMatrix {
        provider: "minimax".to_string(),
        audio_transcript: media_endpoint_capability(
            "audio_transcript",
            config,
            config.audio_transcript_endpoint.clone(),
            config.audio_probe_verified,
        ),
        native_video_understanding: media_endpoint_capability(
            "native_video_understanding",
            config,
            config.video_understanding_endpoint.clone(),
            config.video_probe_verified,
        ),
        keyframe_image_vlm: if config.image_vlm_available {
            MiniMaxMediaCapability {
                capability: "keyframe_image_vlm".to_string(),
                provider: "minimax".to_string(),
                model: config.media_model.clone(),
                status: "available_via_document_image_vlm".to_string(),
                supported: true,
                endpoint: Some(format!(
                    "{}/chat/completions",
                    config.base_url.trim_end_matches('/')
                )),
                detail: "Document image VLM is configured; video keyframes may reuse it after local frame extraction.".to_string(),
            }
        } else {
            MiniMaxMediaCapability {
                capability: "keyframe_image_vlm".to_string(),
                provider: "minimax".to_string(),
                model: config.media_model.clone(),
                status: "not_configured".to_string(),
                supported: false,
                endpoint: None,
                detail: "Document image VLM is not configured, so keyframe VLM is unavailable."
                    .to_string(),
            }
        },
    }
}

fn media_endpoint_capability(
    capability: &str,
    config: &MiniMaxMediaCapabilityConfig,
    endpoint: Option<String>,
    probe_verified: bool,
) -> MiniMaxMediaCapability {
    if !config.minimax_requested() {
        return MiniMaxMediaCapability {
            capability: capability.to_string(),
            provider: "minimax".to_string(),
            model: config.media_model.clone(),
            status: "disabled".to_string(),
            supported: false,
            endpoint,
            detail: "MiniMax media provider was not requested for media parsing.".to_string(),
        };
    }
    if config
        .api_key
        .as_deref()
        .is_none_or(|value| value.trim().is_empty())
    {
        return MiniMaxMediaCapability {
            capability: capability.to_string(),
            provider: "minimax".to_string(),
            model: config.media_model.clone(),
            status: "missing_api_key".to_string(),
            supported: false,
            endpoint,
            detail: "MiniMax media capability cannot be used without an API key.".to_string(),
        };
    }
    let Some(endpoint) = endpoint.filter(|value| !value.trim().is_empty()) else {
        return MiniMaxMediaCapability {
            capability: capability.to_string(),
            provider: "minimax".to_string(),
            model: config.media_model.clone(),
            status: "not_configured".to_string(),
            supported: false,
            endpoint: None,
            detail: "No exact MiniMax media endpoint was configured for this capability."
                .to_string(),
        };
    };
    if !probe_verified {
        return MiniMaxMediaCapability {
            capability: capability.to_string(),
            provider: "minimax".to_string(),
            model: config.media_model.clone(),
            status: "configured_unverified".to_string(),
            supported: false,
            endpoint: Some(endpoint),
            detail: "Endpoint is configured, but no successful capability probe has been recorded; do not use it for production parsing.".to_string(),
        };
    }
    MiniMaxMediaCapability {
        capability: capability.to_string(),
        provider: "minimax".to_string(),
        model: config.media_model.clone(),
        status: "verified".to_string(),
        supported: true,
        endpoint: Some(endpoint),
        detail: "Capability probe is marked verified for this MiniMax media endpoint.".to_string(),
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentImageVlmPayload {
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub document_kind: String,
    #[serde(default)]
    pub layout_type: String,
    #[serde(default)]
    pub topic_tags: Vec<String>,
    #[serde(default)]
    pub risk_level: String,
    #[serde(default)]
    pub visual_summary: String,
    #[serde(default)]
    pub evidence_blocks: Vec<DocumentImageVlmEvidenceBlock>,
    #[serde(default)]
    pub field_candidates: Vec<DocumentImageVlmFieldCandidate>,
    #[serde(default)]
    pub entities: Vec<DocumentImageVlmEntity>,
    #[serde(default)]
    pub claims: Vec<DocumentImageVlmClaim>,
    #[serde(default)]
    pub chart_or_table_detected: bool,
    #[serde(default)]
    pub table_like_signals: Vec<String>,
    #[serde(default)]
    pub transcribed_text: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct DocumentImageVlmEvidenceBlock {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub text: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentImageVlmFieldCandidate {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub value: Value,
    #[serde(default)]
    pub confidence: Option<f64>,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub evidence_text: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentImageVlmEntity {
    #[serde(default)]
    pub text: String,
    #[serde(default, rename = "type")]
    pub entity_type: String,
    #[serde(default)]
    pub confidence: Option<f64>,
    #[serde(default)]
    pub evidence_text: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentImageVlmClaim {
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub predicate: String,
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub confidence: Option<f64>,
    #[serde(default)]
    pub evidence_text: String,
}

#[derive(Clone, Debug)]
pub struct DocumentImageVlmRequest {
    pub title: String,
    pub image_path: String,
    pub existing_text: String,
}

#[derive(Clone, Debug)]
pub struct DocumentImageVlmResponse {
    pub raw_content: String,
    pub model: String,
    pub payload: DocumentImageVlmPayload,
}

pub trait DocumentImageVlmProvider {
    fn parse_image(
        &self,
        request: &DocumentImageVlmRequest,
    ) -> Result<Option<DocumentImageVlmResponse>>;
}

#[derive(Clone, Debug)]
pub struct MiniMaxDocumentImageVlmProvider {
    config: DocumentImageVlmConfig,
}

impl MiniMaxDocumentImageVlmProvider {
    pub fn new(config: DocumentImageVlmConfig) -> Self {
        Self { config }
    }
}

impl DocumentImageVlmProvider for MiniMaxDocumentImageVlmProvider {
    fn parse_image(
        &self,
        request: &DocumentImageVlmRequest,
    ) -> Result<Option<DocumentImageVlmResponse>> {
        if !self.config.available() {
            return Ok(None);
        }

        let image_path = Path::new(&request.image_path);
        let metadata = match fs::metadata(image_path) {
            Ok(metadata) if metadata.is_file() && metadata.len() > 0 => metadata,
            _ => return Ok(None),
        };
        if metadata.len() > self.config.max_image_bytes {
            return Ok(None);
        }

        let bytes = fs::read(image_path)?;
        let data_url = format!(
            "data:{};base64,{}",
            image_content_type(image_path),
            general_purpose::STANDARD.encode(bytes)
        );
        let api_key = self
            .config
            .api_key
            .as_deref()
            .ok_or_else(|| anyhow!("MINIMAX_API_KEY is missing"))?;
        let url = format!(
            "{}/chat/completions",
            self.config.base_url.trim_end_matches('/')
        );
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(self.config.timeout_secs))
            .build()?;
        let body = json!({
            "model": self.config.model,
            "temperature": 0.1,
            "response_format": { "type": "json_object" },
            "messages": [
                { "role": "system", "content": build_document_image_vlm_system_prompt() },
                {
                    "role": "user",
                    "content": [
                        { "type": "text", "text": build_document_image_vlm_prompt(request) },
                        { "type": "image_url", "image_url": { "url": data_url } }
                    ]
                }
            ]
        });
        let response: Value = client
            .post(url)
            .bearer_auth(api_key)
            .json(&body)
            .send()?
            .error_for_status()?
            .json()?;
        let content = response["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_string();
        let Some(payload) = parse_document_image_vlm_payload(&content) else {
            return Ok(None);
        };
        Ok(Some(DocumentImageVlmResponse {
            raw_content: content,
            model: self.config.model.clone(),
            payload,
        }))
    }
}

pub fn run_document_image_vlm_from_env(
    title: &str,
    image_path: &Path,
    existing_text: &str,
) -> Option<DocumentImageVlmResponse> {
    let config = DocumentImageVlmConfig::from_env();
    let provider = MiniMaxDocumentImageVlmProvider::new(config);
    provider
        .parse_image(&DocumentImageVlmRequest {
            title: title.to_string(),
            image_path: image_path.to_string_lossy().to_string(),
            existing_text: existing_text.to_string(),
        })
        .ok()
        .flatten()
}

pub fn build_document_image_vlm_system_prompt() -> String {
    [
        "You are an image-document structuring assistant for a private enterprise knowledge base.",
        "Inspect only the provided image.",
        "Return strict JSON only. No markdown. No explanation.",
        "Do not invent facts. Only extract content visible in the image.",
        "If a value is not visible, omit it or leave it empty.",
        "Use this schema:",
        r#"{"summary":"","documentKind":"","layoutType":"","topicTags":[],"riskLevel":"low|medium|high","visualSummary":"","evidenceBlocks":[{"title":"","text":""}],"fieldCandidates":[{"key":"","value":"","confidence":0.8,"source":"vlm","evidenceText":""}],"entities":[{"text":"","type":"","confidence":0.8,"evidenceText":""}],"claims":[{"subject":"","predicate":"","object":"","confidence":0.8,"evidenceText":""}],"chartOrTableDetected":false,"tableLikeSignals":[],"transcribedText":""}"#,
    ]
    .join(" ")
}

pub fn build_document_image_vlm_prompt(request: &DocumentImageVlmRequest) -> String {
    let mut parts = vec![
        format!("Document title: {}", sanitize_text(&request.title, 240)),
        format!("Image file: {}", sanitize_text(&request.image_path, 500)),
    ];
    let existing = sanitize_text(&request.existing_text, 5000);
    if !existing.is_empty() && !existing.contains("OCR text was not extracted") {
        parts.push(format!("Existing OCR text or parse text:\n{existing}"));
    }
    parts.push("Focus on layout semantics, visible sections, business labels, screenshot widgets, form-like fields, and chart/table cues.".to_string());
    parts.push("Return only the JSON object.".to_string());
    parts.join("\n\n")
}

pub fn parse_document_image_vlm_payload(raw: &str) -> Option<DocumentImageVlmPayload> {
    let text = raw.trim();
    if text.is_empty() {
        return None;
    }
    let candidate = extract_fenced_json(text).unwrap_or(text);
    let start = candidate.find('{')?;
    let end = candidate.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str(&candidate[start..=end]).ok()
}

pub fn build_enriched_image_text(
    image_name: &str,
    existing_ocr_text: Option<&str>,
    payload: &DocumentImageVlmPayload,
) -> String {
    let visual_summary = sanitize_text(
        first_non_empty(&[&payload.visual_summary, &payload.summary]),
        1200,
    );
    let transcribed = sanitize_text(&payload.transcribed_text, 8000);
    let evidence = payload
        .evidence_blocks
        .iter()
        .filter_map(|block| {
            let title = sanitize_text(&block.title, 160);
            let text = sanitize_text(&block.text, 1000);
            (!title.is_empty() || !text.is_empty()).then(|| {
                if title.is_empty() {
                    format!("- {text}")
                } else if text.is_empty() {
                    format!("- {title}")
                } else {
                    format!("- {title}: {text}")
                }
            })
        })
        .collect::<Vec<_>>()
        .join("\n");

    [
        format!("Image file: {image_name}"),
        existing_ocr_text
            .map(|text| sanitize_text(text, 8000))
            .filter(|text| !text.is_empty())
            .map(|text| format!("OCR text:\n{text}"))
            .unwrap_or_default(),
        (!visual_summary.is_empty())
            .then(|| format!("Visual summary:\n{visual_summary}"))
            .unwrap_or_default(),
        (!transcribed.is_empty())
            .then(|| format!("Visual transcription:\n{transcribed}"))
            .unwrap_or_default(),
        (!evidence.is_empty())
            .then(|| format!("Visual evidence:\n{evidence}"))
            .unwrap_or_default(),
    ]
    .into_iter()
    .filter(|part| !part.trim().is_empty())
    .collect::<Vec<_>>()
    .join("\n\n")
}

fn resolve_parse_mode(value: Option<&str>) -> DocumentImageParseMode {
    match value
        .unwrap_or("ocr-plus-vlm")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "ocr-only" | "disabled" => DocumentImageParseMode::OcrOnly,
        _ => DocumentImageParseMode::OcrPlusVlm,
    }
}

fn resolve_provider_mode(value: Option<&str>) -> DocumentImageVlmProviderMode {
    match value
        .unwrap_or("minimax")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "disabled" | "none" => DocumentImageVlmProviderMode::Disabled,
        _ => DocumentImageVlmProviderMode::MiniMax,
    }
}

fn env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_bool_env(name: &str) -> bool {
    env(name)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on" | "verified"
            )
        })
        .unwrap_or(false)
}

fn extract_fenced_json(text: &str) -> Option<&str> {
    let start_marker = text.find("```")?;
    let after_start = &text[start_marker + 3..];
    let after_lang = after_start
        .strip_prefix("json")
        .unwrap_or(after_start)
        .trim_start();
    let end_marker = after_lang.find("```")?;
    Some(after_lang[..end_marker].trim())
}

fn image_content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "tif" | "tiff" => "image/tiff",
        _ => "image/png",
    }
}

fn sanitize_text(value: &str, max_chars: usize) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_chars {
        return normalized;
    }
    let mut output = normalized
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>();
    output.push_str("...");
    output
}

fn first_non_empty<'a>(values: &[&'a str]) -> &'a str {
    values
        .iter()
        .find(|value| !value.trim().is_empty())
        .copied()
        .unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_strict_json_payload_from_fenced_response() {
        let payload = parse_document_image_vlm_payload(
            r#"```json
{"summary":"制度截图","visualSummary":"展示制度入口","topicTags":["制度"],"evidenceBlocks":[{"title":"入口","text":"制度管理"}],"entities":[{"text":"制度管理","type":"module"}],"transcribedText":"制度管理"}
```"#,
        )
        .expect("payload should parse");

        assert_eq!(payload.summary, "制度截图");
        assert_eq!(payload.visual_summary, "展示制度入口");
        assert_eq!(payload.topic_tags, vec!["制度"]);
        assert_eq!(payload.evidence_blocks[0].title, "入口");
        assert_eq!(payload.entities[0].entity_type, "module");
    }

    #[test]
    fn builds_enriched_text_without_inventing_missing_sections() {
        let payload = DocumentImageVlmPayload {
            summary: "客流截图".to_string(),
            transcribed_text: "A区 2180 人次".to_string(),
            evidence_blocks: vec![DocumentImageVlmEvidenceBlock {
                title: "分区".to_string(),
                text: "A区 2180 人次".to_string(),
            }],
            ..Default::default()
        };

        let text = build_enriched_image_text("footfall.png", Some("A区"), &payload);

        assert!(text.contains("Image file: footfall.png"));
        assert!(text.contains("OCR text:"));
        assert!(text.contains("Visual transcription:"));
        assert!(text.contains("- 分区: A区 2180 人次"));
    }

    #[test]
    fn disabled_or_missing_key_config_is_unavailable() {
        let config = DocumentImageVlmConfig {
            parse_mode: DocumentImageParseMode::OcrPlusVlm,
            provider_mode: DocumentImageVlmProviderMode::MiniMax,
            api_key: None,
            base_url: DEFAULT_MINIMAX_BASE_URL.to_string(),
            model: DEFAULT_MINIMAX_MODEL.to_string(),
            max_image_bytes: DEFAULT_MAX_IMAGE_BYTES,
            timeout_secs: DEFAULT_TIMEOUT_SECS,
        };
        assert!(!config.available());
    }

    #[test]
    fn minimax_media_capability_does_not_mark_key_only_audio_as_supported() {
        let config = MiniMaxMediaCapabilityConfig {
            provider: "minimax".to_string(),
            prefer_minimax: true,
            api_key: Some("secret".to_string()),
            base_url: DEFAULT_MINIMAX_BASE_URL.to_string(),
            media_model: DEFAULT_MINIMAX_MEDIA_MODEL.to_string(),
            audio_transcript_endpoint: Some("/v1/audio/transcriptions".to_string()),
            video_understanding_endpoint: None,
            audio_probe_verified: false,
            video_probe_verified: false,
            image_vlm_available: false,
        };

        let matrix = probe_minimax_media_capabilities(&config);

        assert_eq!(matrix.audio_transcript.status, "configured_unverified");
        assert!(!matrix.audio_transcript.supported);
        assert_eq!(matrix.native_video_understanding.status, "not_configured");
    }

    #[test]
    fn minimax_media_capability_marks_only_verified_endpoint_as_supported() {
        let config = MiniMaxMediaCapabilityConfig {
            provider: "minimax".to_string(),
            prefer_minimax: true,
            api_key: Some("secret".to_string()),
            base_url: DEFAULT_MINIMAX_BASE_URL.to_string(),
            media_model: DEFAULT_MINIMAX_MEDIA_MODEL.to_string(),
            audio_transcript_endpoint: Some("/v1/audio/transcriptions".to_string()),
            video_understanding_endpoint: Some("/v1/video/understanding".to_string()),
            audio_probe_verified: true,
            video_probe_verified: false,
            image_vlm_available: true,
        };

        let matrix = probe_minimax_media_capabilities(&config);

        assert!(matrix.audio_transcript.supported);
        assert_eq!(matrix.audio_transcript.status, "verified");
        assert!(!matrix.native_video_understanding.supported);
        assert_eq!(
            matrix.keyframe_image_vlm.status,
            "available_via_document_image_vlm"
        );
        assert!(matrix.keyframe_image_vlm.supported);
    }
}
