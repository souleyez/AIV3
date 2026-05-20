use aes::Aes256;
use base64::{engine::general_purpose, Engine as _};
use cbc::cipher::{block_padding::NoPadding, BlockDecryptMut, KeyIvInit};
use chrono::{DateTime, Utc};
use contracts::{
    ExternalAttachmentRefView, ExternalBotMessageView, ExternalBotReplyTypeView,
    ExternalBotReplyView, ExternalChannelPlatformView, ExternalMessageTypeView,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, error::Error, fmt};

const DEFAULT_MAX_CLOCK_SKEW_SECONDS: i64 = 300;

#[derive(Clone, Debug)]
pub struct FeishuAdapterConfig {
    pub platform: ExternalChannelPlatformView,
    pub tenant_external_id: String,
    pub bot_external_id: String,
    pub verification_token: String,
    pub encrypt_key: String,
    pub max_clock_skew_seconds: i64,
}

impl FeishuAdapterConfig {
    pub fn new(
        platform: ExternalChannelPlatformView,
        tenant_external_id: impl Into<String>,
        bot_external_id: impl Into<String>,
        verification_token: impl Into<String>,
        encrypt_key: impl Into<String>,
    ) -> Self {
        Self {
            platform,
            tenant_external_id: tenant_external_id.into(),
            bot_external_id: bot_external_id.into(),
            verification_token: verification_token.into(),
            encrypt_key: encrypt_key.into(),
            max_clock_skew_seconds: DEFAULT_MAX_CLOCK_SKEW_SECONDS,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeishuCallbackHeaders {
    pub timestamp: String,
    pub nonce: String,
    pub signature: String,
}

impl FeishuCallbackHeaders {
    pub fn new(
        timestamp: impl Into<String>,
        nonce: impl Into<String>,
        signature: impl Into<String>,
    ) -> Self {
        Self {
            timestamp: timestamp.into(),
            nonce: nonce.into(),
            signature: signature.into(),
        }
    }
}

#[derive(Debug, Default)]
pub struct FeishuReplayGuard {
    seen: BTreeSet<String>,
}

impl FeishuReplayGuard {
    pub fn check_and_record(
        &mut self,
        timestamp: &str,
        nonce: &str,
    ) -> Result<(), FeishuAdapterError> {
        let key = format!("{timestamp}:{nonce}");
        if !self.seen.insert(key) {
            return Err(FeishuAdapterError::new(
                "feishu_replay_detected",
                "duplicate Feishu callback timestamp/nonce pair",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FeishuCallbackOutcome {
    Challenge { challenge: String },
    Message(ExternalBotMessageView),
    Ignored { event_type: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeishuAdapterError {
    pub code: &'static str,
    pub message: String,
}

impl FeishuAdapterError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for FeishuAdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl Error for FeishuAdapterError {}

pub fn feishu_callback_signature(
    timestamp: &str,
    nonce: &str,
    encrypt_key: &str,
    raw_body: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(timestamp.as_bytes());
    hasher.update(nonce.as_bytes());
    hasher.update(encrypt_key.as_bytes());
    hasher.update(raw_body.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn validate_feishu_callback(
    headers: &FeishuCallbackHeaders,
    raw_body: &str,
    encrypt_key: &str,
    now: DateTime<Utc>,
    max_clock_skew_seconds: i64,
    replay_guard: Option<&mut FeishuReplayGuard>,
) -> Result<(), FeishuAdapterError> {
    let timestamp = headers.timestamp.parse::<i64>().map_err(|_| {
        FeishuAdapterError::new(
            "feishu_timestamp_invalid",
            "Feishu callback timestamp must be a unix timestamp in seconds",
        )
    })?;
    let drift = (now.timestamp() - timestamp).abs();
    if max_clock_skew_seconds >= 0 && drift > max_clock_skew_seconds {
        return Err(FeishuAdapterError::new(
            "feishu_timestamp_out_of_window",
            format!("Feishu callback timestamp drift {drift}s exceeds the configured window"),
        ));
    }

    let expected =
        feishu_callback_signature(&headers.timestamp, &headers.nonce, encrypt_key, raw_body);
    if !expected.eq_ignore_ascii_case(&headers.signature) {
        return Err(FeishuAdapterError::new(
            "feishu_signature_mismatch",
            "Feishu callback signature did not match the raw request body",
        ));
    }

    if let Some(guard) = replay_guard {
        guard.check_and_record(&headers.timestamp, &headers.nonce)?;
    }
    Ok(())
}

pub fn normalize_feishu_callback(
    config: &FeishuAdapterConfig,
    headers: Option<&FeishuCallbackHeaders>,
    raw_body: &str,
    now: DateTime<Utc>,
    replay_guard: Option<&mut FeishuReplayGuard>,
) -> Result<FeishuCallbackOutcome, FeishuAdapterError> {
    let outer_payload: Value = serde_json::from_str(raw_body).map_err(|error| {
        FeishuAdapterError::new(
            "feishu_payload_invalid",
            format!("Feishu callback payload is not valid JSON: {error}"),
        )
    })?;

    if let Some(challenge) = feishu_challenge(&outer_payload) {
        verify_feishu_token(config, &outer_payload)?;
        return Ok(FeishuCallbackOutcome::Challenge { challenge });
    }

    let headers = headers.ok_or_else(|| {
        FeishuAdapterError::new(
            "feishu_signature_headers_missing",
            "Feishu event callback requires timestamp, nonce, and signature headers",
        )
    })?;
    validate_feishu_callback(
        headers,
        raw_body,
        &config.encrypt_key,
        now,
        config.max_clock_skew_seconds,
        replay_guard,
    )?;

    let payload = decrypt_feishu_payload_if_needed(&outer_payload, &config.encrypt_key)?;
    verify_feishu_token(config, &payload)?;

    let event_type = feishu_event_type(&payload).unwrap_or_else(|| "unknown".to_string());
    if event_type != "im.message.receive_v1" {
        return Ok(FeishuCallbackOutcome::Ignored { event_type });
    }

    Ok(FeishuCallbackOutcome::Message(feishu_message_from_payload(
        config, &payload, now,
    )?))
}

pub fn decrypt_feishu_encrypt(
    encrypt: &str,
    encrypt_key: &str,
) -> Result<String, FeishuAdapterError> {
    let decoded = general_purpose::STANDARD.decode(encrypt).map_err(|error| {
        FeishuAdapterError::new(
            "feishu_encrypt_base64_invalid",
            format!("Feishu encrypted payload is not valid base64: {error}"),
        )
    })?;
    if decoded.len() < 32 {
        return Err(FeishuAdapterError::new(
            "feishu_ciphertext_invalid",
            "Feishu encrypted payload is too short",
        ));
    }
    let (iv, ciphertext) = decoded.split_at(16);
    if ciphertext.len() % 16 != 0 {
        return Err(FeishuAdapterError::new(
            "feishu_ciphertext_invalid",
            "Feishu ciphertext length must be a multiple of AES block size",
        ));
    }

    let key = Sha256::digest(encrypt_key.as_bytes());
    let mut ciphertext = ciphertext.to_vec();
    let plaintext = cbc::Decryptor::<Aes256>::new_from_slices(&key, iv)
        .map_err(|error| {
            FeishuAdapterError::new(
                "feishu_cipher_init_failed",
                format!("Feishu AES decryptor could not be initialized: {error}"),
            )
        })?
        .decrypt_padded_mut::<NoPadding>(&mut ciphertext)
        .map_err(|error| {
            FeishuAdapterError::new(
                "feishu_decrypt_failed",
                format!("Feishu encrypted payload could not be decrypted: {error}"),
            )
        })?;
    let start = plaintext
        .iter()
        .position(|byte| *byte == b'{')
        .ok_or_else(|| {
            FeishuAdapterError::new(
                "feishu_plaintext_invalid",
                "Feishu decrypted payload does not contain a JSON object",
            )
        })?;
    let end = plaintext
        .iter()
        .rposition(|byte| *byte == b'}')
        .filter(|end| *end >= start)
        .ok_or_else(|| {
            FeishuAdapterError::new(
                "feishu_plaintext_invalid",
                "Feishu decrypted payload does not contain a complete JSON object",
            )
        })?;
    std::str::from_utf8(&plaintext[start..=end])
        .map(str::to_string)
        .map_err(|error| {
            FeishuAdapterError::new(
                "feishu_plaintext_invalid",
                format!("Feishu decrypted payload is not UTF-8: {error}"),
            )
        })
}

pub fn render_feishu_reply_body(reply: &ExternalBotReplyView) -> Value {
    let (msg_type, content) = match reply.reply_type {
        ExternalBotReplyTypeView::Card => (
            "interactive",
            reply
                .card
                .clone()
                .unwrap_or_else(|| json!({ "config": {}, "elements": [] })),
        ),
        _ => ("text", json!({ "text": feishu_reply_text(reply) })),
    };

    json!({
        "receive_id_type": "chat_id",
        "receive_id": reply.target_conversation_external_id,
        "msg_type": msg_type,
        "content": content.to_string(),
    })
}

fn feishu_challenge(payload: &Value) -> Option<String> {
    let callback_type = value_string(payload, &["type"]);
    if callback_type.as_deref() == Some("url_verification") {
        return value_string(payload, &["challenge"]);
    }
    if value_string(payload, &["schema"]).as_deref() == Some("2.0") {
        return None;
    }
    value_string(payload, &["challenge"])
}

fn decrypt_feishu_payload_if_needed(
    payload: &Value,
    encrypt_key: &str,
) -> Result<Value, FeishuAdapterError> {
    let Some(encrypt) = value_string(payload, &["encrypt"]) else {
        return Ok(payload.clone());
    };
    let decrypted = decrypt_feishu_encrypt(&encrypt, encrypt_key)?;
    serde_json::from_str(&decrypted).map_err(|error| {
        FeishuAdapterError::new(
            "feishu_decrypted_payload_invalid",
            format!("Feishu decrypted payload is not valid JSON: {error}"),
        )
    })
}

fn verify_feishu_token(
    config: &FeishuAdapterConfig,
    payload: &Value,
) -> Result<(), FeishuAdapterError> {
    if config.verification_token.trim().is_empty() {
        return Ok(());
    }
    let token = value_string(payload, &["header", "token"])
        .or_else(|| value_string(payload, &["token"]))
        .ok_or_else(|| {
            FeishuAdapterError::new(
                "feishu_token_missing",
                "Feishu callback did not include a verification token",
            )
        })?;
    if token != config.verification_token {
        return Err(FeishuAdapterError::new(
            "feishu_token_mismatch",
            "Feishu callback verification token did not match the configured connection",
        ));
    }
    Ok(())
}

fn feishu_message_from_payload(
    config: &FeishuAdapterConfig,
    payload: &Value,
    now: DateTime<Utc>,
) -> Result<ExternalBotMessageView, FeishuAdapterError> {
    let event = payload.get("event").ok_or_else(|| {
        FeishuAdapterError::new(
            "feishu_event_missing",
            "Feishu message event is missing event",
        )
    })?;
    let message = event.get("message").ok_or_else(|| {
        FeishuAdapterError::new(
            "feishu_message_missing",
            "Feishu message event is missing event.message",
        )
    })?;
    let sender = event.get("sender").unwrap_or(&Value::Null);
    let sender_id = sender.get("sender_id").unwrap_or(&Value::Null);

    let tenant_external_id = value_string(payload, &["header", "tenant_key"])
        .or_else(|| value_string(event, &["tenant_key"]))
        .unwrap_or_else(|| config.tenant_external_id.clone());
    let bot_external_id = value_string(payload, &["header", "app_id"])
        .unwrap_or_else(|| config.bot_external_id.clone());
    let conversation_external_id = string_field(message, "chat_id").ok_or_else(|| {
        FeishuAdapterError::new(
            "feishu_chat_id_missing",
            "Feishu message event is missing message.chat_id",
        )
    })?;
    let sender_external_id = first_string_field(sender_id, &["union_id", "user_id", "open_id"])
        .or_else(|| string_field(sender, "open_id"))
        .ok_or_else(|| {
            FeishuAdapterError::new(
                "feishu_sender_missing",
                "Feishu message event is missing sender id",
            )
        })?;
    let message_external_id = string_field(message, "message_id").ok_or_else(|| {
        FeishuAdapterError::new(
            "feishu_message_id_missing",
            "Feishu message event is missing message.message_id",
        )
    })?;
    let message_type = string_field(message, "message_type")
        .as_deref()
        .map(feishu_message_type)
        .unwrap_or(ExternalMessageTypeView::Unknown);
    let content = parse_feishu_content(message);
    let text = feishu_text(message_type.clone(), &content);
    let event_id = value_string(payload, &["header", "event_id"]);

    Ok(ExternalBotMessageView {
        platform: config.platform.clone(),
        tenant_external_id: tenant_external_id.clone(),
        bot_external_id: bot_external_id.clone(),
        conversation_external_id,
        thread_external_id: first_string_field(message, &["thread_id", "root_id"]),
        sender_external_id,
        message_external_id: message_external_id.clone(),
        message_type,
        text,
        default_prompt: None,
        output_format: None,
        render_mode: None,
        mention_external_user_ids: feishu_mentions(message, &content),
        attachment_refs: feishu_attachment_refs(&content),
        available_document_external_ids: Vec::new(),
        available_document_source_id: None,
        requested_skills: Vec::new(),
        idempotency_key: format!(
            "feishu:{tenant_external_id}:{bot_external_id}:{}",
            event_id.as_deref().unwrap_or(&message_external_id)
        ),
        received_at: value_string(payload, &["header", "create_time"])
            .and_then(|value| parse_feishu_timestamp(&value))
            .unwrap_or(now),
    })
}

fn feishu_event_type(payload: &Value) -> Option<String> {
    value_string(payload, &["header", "event_type"])
        .or_else(|| value_string(payload, &["event", "type"]))
        .or_else(|| value_string(payload, &["type"]))
}

fn parse_feishu_content(message: &Value) -> Value {
    string_field(message, "content")
        .and_then(|content| serde_json::from_str::<Value>(&content).ok())
        .unwrap_or(Value::Null)
}

fn feishu_message_type(value: &str) -> ExternalMessageTypeView {
    match value {
        "text" | "post" => ExternalMessageTypeView::Text,
        "image" => ExternalMessageTypeView::Image,
        "file" => ExternalMessageTypeView::File,
        "audio" => ExternalMessageTypeView::Audio,
        "media" | "video" => ExternalMessageTypeView::Video,
        "interactive" | "card" => ExternalMessageTypeView::Card,
        _ => ExternalMessageTypeView::Unknown,
    }
}

fn feishu_text(message_type: ExternalMessageTypeView, content: &Value) -> Option<String> {
    match message_type {
        ExternalMessageTypeView::Text => string_field(content, "text")
            .or_else(|| string_field(content, "title"))
            .filter(|text| !text.trim().is_empty()),
        ExternalMessageTypeView::Card => string_field(content, "title"),
        _ => None,
    }
}

fn feishu_mentions(message: &Value, content: &Value) -> Vec<String> {
    let mut values = Vec::new();
    collect_feishu_mentions(message.get("mentions"), &mut values);
    collect_feishu_mentions(content.get("mentions"), &mut values);
    values.sort();
    values.dedup();
    values
}

fn collect_feishu_mentions(value: Option<&Value>, output: &mut Vec<String>) {
    let Some(Value::Array(mentions)) = value else {
        return;
    };
    for mention in mentions {
        if let Some(id) = mention
            .get("id")
            .and_then(|id| first_string_field(id, &["union_id", "user_id", "open_id"]))
            .or_else(|| first_string_field(mention, &["user_id", "open_id", "union_id"]))
        {
            output.push(id);
        }
    }
}

fn feishu_attachment_refs(content: &Value) -> Vec<ExternalAttachmentRefView> {
    let mut refs = Vec::new();
    for key in ["file_key", "image_key", "media_key"] {
        if let Some(attachment_external_id) = string_field(content, key) {
            refs.push(ExternalAttachmentRefView {
                attachment_external_id,
                filename: string_field(content, "file_name")
                    .or_else(|| string_field(content, "name")),
                content_type: string_field(content, "mime_type"),
                size_bytes: content.get("size").and_then(Value::as_u64),
                download_url_redacted: None,
            });
        }
    }
    refs
}

fn feishu_reply_text(reply: &ExternalBotReplyView) -> String {
    let mut text = reply
        .text
        .clone()
        .or_else(|| reply.task_status.clone())
        .unwrap_or_else(|| "V3 task accepted.".to_string());
    if !reply.artifact_links.is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&reply.artifact_links.join("\n"));
    }
    text
}

fn parse_feishu_timestamp(value: &str) -> Option<DateTime<Utc>> {
    let parsed = value.parse::<i64>().ok()?;
    if value.len() >= 13 {
        DateTime::<Utc>::from_timestamp_millis(parsed)
    } else {
        DateTime::<Utc>::from_timestamp(parsed, 0)
    }
}

fn value_string(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_str().map(ToString::to_string)
}

fn first_string_field(value: &Value, fields: &[&str]) -> Option<String> {
    fields.iter().find_map(|field| string_field(value, field))
}

fn string_field(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cbc::cipher::BlockEncryptMut;
    use chrono::TimeZone;

    fn sample_config() -> FeishuAdapterConfig {
        FeishuAdapterConfig {
            platform: ExternalChannelPlatformView::Feishu,
            tenant_external_id: "tenant-fallback".to_string(),
            bot_external_id: "bot-fallback".to_string(),
            verification_token: "verification-token".to_string(),
            encrypt_key: "encrypt-key".to_string(),
            max_clock_skew_seconds: 300,
        }
    }

    fn encrypt_feishu_test_payload(payload: &Value, encrypt_key: &str) -> String {
        let key = Sha256::digest(encrypt_key.as_bytes());
        let iv = [0x24u8; 16];
        let mut plaintext = payload.to_string().into_bytes();
        let padding = 16 - (plaintext.len() % 16);
        let padding = if padding == 0 { 16 } else { padding };
        plaintext.extend(std::iter::repeat_n(padding as u8, padding));
        let plaintext_len = plaintext.len();
        let encrypted = cbc::Encryptor::<Aes256>::new_from_slices(&key, &iv)
            .expect("test AES encryptor should initialize")
            .encrypt_padded_mut::<NoPadding>(&mut plaintext, plaintext_len)
            .expect("test plaintext is block aligned");
        let mut envelope = iv.to_vec();
        envelope.extend_from_slice(encrypted);
        general_purpose::STANDARD.encode(envelope)
    }

    #[test]
    fn feishu_signature_matches_sanitized_fixture_and_rejects_replay() {
        let raw_body = r#"{"schema":"2.0"}"#;
        let signature =
            feishu_callback_signature("1700000000", "nonce-001", "encrypt-key", raw_body);
        assert_eq!(
            signature,
            "095eaf170b330bb22ef5f5d34e5f411e586b7a1af20aabf375342a97203d9c00"
        );
        let headers = FeishuCallbackHeaders::new("1700000000", "nonce-001", signature);
        let now = Utc.timestamp_opt(1_700_000_000, 0).single().unwrap();
        let mut replay_guard = FeishuReplayGuard::default();

        validate_feishu_callback(
            &headers,
            raw_body,
            "encrypt-key",
            now,
            300,
            Some(&mut replay_guard),
        )
        .expect("first callback should validate");
        let replay = validate_feishu_callback(
            &headers,
            raw_body,
            "encrypt-key",
            now,
            300,
            Some(&mut replay_guard),
        )
        .expect_err("duplicate callback should be rejected");
        assert_eq!(replay.code, "feishu_replay_detected");
    }

    #[test]
    fn feishu_encrypt_decrypts_to_json_payload() {
        let payload = json!({
            "schema": "2.0",
            "header": {
                "event_type": "im.message.receive_v1",
                "token": "verification-token"
            },
            "event": {
                "message": {
                    "message_id": "om_encrypted",
                    "chat_id": "oc_room",
                    "message_type": "text",
                    "content": "{\"text\":\"encrypted hello\"}"
                }
            }
        });
        let encrypt = encrypt_feishu_test_payload(&payload, "encrypt-key");

        let decrypted =
            decrypt_feishu_encrypt(&encrypt, "encrypt-key").expect("payload should decrypt");
        let decoded: Value = serde_json::from_str(&decrypted).expect("payload should be JSON");
        assert_eq!(
            decoded["header"]["event_type"],
            json!("im.message.receive_v1")
        );
        assert_eq!(
            decoded["event"]["message"]["message_id"],
            json!("om_encrypted")
        );
    }

    #[test]
    fn feishu_event_normalizes_message_event_to_external_bot_message() {
        let config = sample_config();
        let raw_body = json!({
            "schema": "2.0",
            "header": {
                "event_id": "evt-001",
                "event_type": "im.message.receive_v1",
                "app_id": "cli_v3",
                "tenant_key": "tenant-feishu",
                "create_time": "1700000000123",
                "token": "verification-token"
            },
            "event": {
                "sender": {
                    "sender_id": {
                        "open_id": "ou_sender",
                        "user_id": "user_sender"
                    }
                },
                "message": {
                    "message_id": "om_001",
                    "chat_id": "oc_room",
                    "chat_type": "group",
                    "message_type": "text",
                    "content": "{\"text\":\"hello V3\"}",
                    "mentions": [
                        {"id": {"open_id": "ou_mentioned"}}
                    ]
                }
            }
        })
        .to_string();
        let signature =
            feishu_callback_signature("1700000000", "nonce-002", "encrypt-key", &raw_body);
        let headers = FeishuCallbackHeaders::new("1700000000", "nonce-002", signature);
        let now = Utc.timestamp_opt(1_700_000_000, 0).single().unwrap();
        let mut replay_guard = FeishuReplayGuard::default();

        let outcome = normalize_feishu_callback(
            &config,
            Some(&headers),
            &raw_body,
            now,
            Some(&mut replay_guard),
        )
        .expect("message should normalize");
        let FeishuCallbackOutcome::Message(message) = outcome else {
            panic!("expected message outcome");
        };
        assert_eq!(message.platform, ExternalChannelPlatformView::Feishu);
        assert_eq!(message.tenant_external_id, "tenant-feishu");
        assert_eq!(message.bot_external_id, "cli_v3");
        assert_eq!(message.conversation_external_id, "oc_room");
        assert_eq!(message.sender_external_id, "user_sender");
        assert_eq!(message.message_external_id, "om_001");
        assert_eq!(message.message_type, ExternalMessageTypeView::Text);
        assert_eq!(message.text.as_deref(), Some("hello V3"));
        assert_eq!(message.mention_external_user_ids, vec!["ou_mentioned"]);
        assert_eq!(
            message.idempotency_key,
            "feishu:tenant-feishu:cli_v3:evt-001"
        );
    }

    #[test]
    fn feishu_event_normalizes_encrypted_message_event() {
        let config = sample_config();
        let decrypted_payload = json!({
            "schema": "2.0",
            "header": {
                "event_id": "evt-encrypted-001",
                "event_type": "im.message.receive_v1",
                "app_id": "cli_v3",
                "tenant_key": "tenant-feishu",
                "create_time": "1700000000123",
                "token": "verification-token"
            },
            "event": {
                "sender": {
                    "sender_id": {
                        "user_id": "user_sender"
                    }
                },
                "message": {
                    "message_id": "om_encrypted_001",
                    "chat_id": "oc_room",
                    "message_type": "text",
                    "content": "{\"text\":\"encrypted V3\"}"
                }
            }
        });
        let raw_body = json!({
            "encrypt": encrypt_feishu_test_payload(&decrypted_payload, "encrypt-key")
        })
        .to_string();
        let signature =
            feishu_callback_signature("1700000000", "nonce-encrypted", "encrypt-key", &raw_body);
        let headers = FeishuCallbackHeaders::new("1700000000", "nonce-encrypted", signature);
        let now = Utc.timestamp_opt(1_700_000_000, 0).single().unwrap();

        let outcome = normalize_feishu_callback(&config, Some(&headers), &raw_body, now, None)
            .expect("encrypted message should normalize");
        let FeishuCallbackOutcome::Message(message) = outcome else {
            panic!("expected encrypted message outcome");
        };
        assert_eq!(message.tenant_external_id, "tenant-feishu");
        assert_eq!(message.sender_external_id, "user_sender");
        assert_eq!(message.message_external_id, "om_encrypted_001");
        assert_eq!(message.text.as_deref(), Some("encrypted V3"));
        assert_eq!(
            message.idempotency_key,
            "feishu:tenant-feishu:cli_v3:evt-encrypted-001"
        );
    }

    #[test]
    fn feishu_reply_renders_channel_safe_text_payload() {
        let reply = ExternalBotReplyView {
            target_conversation_external_id: "oc_room".to_string(),
            reply_type: ExternalBotReplyTypeView::Text,
            text: Some("done".to_string()),
            card: None,
            artifact_links: vec!["https://example.invalid/report".to_string()],
            task_status: None,
            requires_confirmation: false,
            action_id: None,
            confirmation_id: None,
        };

        let payload = render_feishu_reply_body(&reply);
        assert_eq!(payload["receive_id_type"], json!("chat_id"));
        assert_eq!(payload["receive_id"], json!("oc_room"));
        assert_eq!(payload["msg_type"], json!("text"));
        assert!(payload["content"]
            .as_str()
            .expect("content string")
            .contains("https://example.invalid/report"));
    }
}
