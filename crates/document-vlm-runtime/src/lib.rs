use anyhow::{anyhow, Result};
use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{fs, path::Path, time::Duration};

const DEFAULT_MINIMAX_BASE_URL: &str = "https://api.minimaxi.com/v1";
const DEFAULT_MINIMAX_MODEL: &str = "MiniMax-M2.5-highspeed";
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
}
