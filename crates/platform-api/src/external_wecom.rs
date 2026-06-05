use aes::Aes256;
use base64::{engine::general_purpose, Engine as _};
use cbc::cipher::{block_padding::NoPadding, BlockDecryptMut, KeyIvInit};
use chrono::{DateTime, Utc};
use contracts::{
    ExternalAttachmentRefView, ExternalBotMessageView, ExternalBotReplyTypeView,
    ExternalBotReplyView, ExternalChannelPlatformView, ExternalMessageTypeView,
};
use quick_xml::{events::Event, Reader};
use serde_json::{json, Value};
use sha1::{Digest, Sha1};
use std::{collections::BTreeSet, error::Error, fmt};

const DEFAULT_MAX_CLOCK_SKEW_SECONDS: i64 = 300;

#[derive(Clone, Debug)]
pub struct WeComAdapterConfig {
    pub tenant_external_id: String,
    pub bot_external_id: String,
    pub token: String,
    pub encoding_aes_key: Option<String>,
    pub receive_id: Option<String>,
    pub max_clock_skew_seconds: i64,
}

impl WeComAdapterConfig {
    pub fn new(
        tenant_external_id: impl Into<String>,
        bot_external_id: impl Into<String>,
        token: impl Into<String>,
    ) -> Self {
        Self {
            tenant_external_id: tenant_external_id.into(),
            bot_external_id: bot_external_id.into(),
            token: token.into(),
            encoding_aes_key: None,
            receive_id: None,
            max_clock_skew_seconds: DEFAULT_MAX_CLOCK_SKEW_SECONDS,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeComCallbackParams {
    pub msg_signature: String,
    pub timestamp: String,
    pub nonce: String,
}

impl WeComCallbackParams {
    pub fn new(
        msg_signature: impl Into<String>,
        timestamp: impl Into<String>,
        nonce: impl Into<String>,
    ) -> Self {
        Self {
            msg_signature: msg_signature.into(),
            timestamp: timestamp.into(),
            nonce: nonce.into(),
        }
    }
}

#[derive(Debug, Default)]
pub struct WeComReplayGuard {
    seen: BTreeSet<String>,
}

impl WeComReplayGuard {
    pub fn check_and_record(
        &mut self,
        timestamp: &str,
        nonce: &str,
    ) -> Result<(), WeComAdapterError> {
        let key = format!("{timestamp}:{nonce}");
        if !self.seen.insert(key) {
            return Err(WeComAdapterError::new(
                "wecom_replay_detected",
                "duplicate WeCom callback timestamp/nonce pair",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WeComCallbackOutcome {
    Message(ExternalBotMessageView),
    Ignored { message_type: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeComAdapterError {
    pub code: &'static str,
    pub message: String,
}

impl WeComAdapterError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for WeComAdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl Error for WeComAdapterError {}

pub fn wecom_callback_signature(
    token: &str,
    timestamp: &str,
    nonce: &str,
    encrypt: &str,
) -> String {
    let mut parts = [token, timestamp, nonce, encrypt];
    parts.sort();
    let mut hasher = Sha1::new();
    hasher.update(parts.join("").as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn validate_wecom_callback(
    params: &WeComCallbackParams,
    token: &str,
    encrypt: &str,
    now: DateTime<Utc>,
    max_clock_skew_seconds: i64,
    replay_guard: Option<&mut WeComReplayGuard>,
) -> Result<(), WeComAdapterError> {
    let timestamp = params.timestamp.parse::<i64>().map_err(|_| {
        WeComAdapterError::new(
            "wecom_timestamp_invalid",
            "WeCom callback timestamp must be a unix timestamp in seconds",
        )
    })?;
    let drift = (now.timestamp() - timestamp).abs();
    if max_clock_skew_seconds >= 0 && drift > max_clock_skew_seconds {
        return Err(WeComAdapterError::new(
            "wecom_timestamp_out_of_window",
            format!("WeCom callback timestamp drift {drift}s exceeds the configured window"),
        ));
    }

    let expected = wecom_callback_signature(token, &params.timestamp, &params.nonce, encrypt);
    if !expected.eq_ignore_ascii_case(&params.msg_signature) {
        return Err(WeComAdapterError::new(
            "wecom_signature_mismatch",
            "WeCom callback signature did not match the encrypted message body",
        ));
    }

    if let Some(guard) = replay_guard {
        guard.check_and_record(&params.timestamp, &params.nonce)?;
    }
    Ok(())
}

pub fn decrypt_wecom_echostr(
    echostr: &str,
    encoding_aes_key: &str,
    expected_receive_id: Option<&str>,
) -> Result<String, WeComAdapterError> {
    decrypt_wecom_encrypt(echostr, encoding_aes_key, expected_receive_id)
}

pub fn decrypt_wecom_envelope(
    encrypted_xml: &str,
    encoding_aes_key: &str,
    expected_receive_id: Option<&str>,
) -> Result<String, WeComAdapterError> {
    let fields = parse_wecom_xml(encrypted_xml)?;
    let encrypt = fields.get("Encrypt").ok_or_else(|| {
        WeComAdapterError::new(
            "wecom_encrypt_missing",
            "WeCom callback envelope is missing the Encrypt field",
        )
    })?;
    decrypt_wecom_encrypt(encrypt, encoding_aes_key, expected_receive_id)
}

pub fn normalize_wecom_callback(
    config: &WeComAdapterConfig,
    params: &WeComCallbackParams,
    encrypted_xml: &str,
    decrypted_xml: Option<&str>,
    now: DateTime<Utc>,
    replay_guard: Option<&mut WeComReplayGuard>,
) -> Result<WeComCallbackOutcome, WeComAdapterError> {
    let encrypted_fields = parse_wecom_xml(encrypted_xml)?;
    let encrypt = encrypted_fields.get("Encrypt").ok_or_else(|| {
        WeComAdapterError::new(
            "wecom_encrypt_missing",
            "WeCom callback envelope is missing the Encrypt field",
        )
    })?;
    validate_wecom_callback(
        params,
        &config.token,
        encrypt,
        now,
        config.max_clock_skew_seconds,
        replay_guard,
    )?;

    let message_fields = parse_wecom_xml(decrypted_xml.unwrap_or(encrypted_xml))?;
    let message_type = message_fields
        .get("MsgType")
        .or_else(|| message_fields.get("Event"))
        .cloned()
        .ok_or_else(|| {
            WeComAdapterError::new(
                "wecom_decryption_required",
                "WeCom callback signature is valid, but decrypted XML is required to normalize the event",
            )
        })?;
    if message_type == "event" {
        return Ok(WeComCallbackOutcome::Ignored { message_type });
    }

    Ok(WeComCallbackOutcome::Message(wecom_message_from_fields(
        config,
        &message_fields,
        now,
    )?))
}

fn decrypt_wecom_encrypt(
    encrypt: &str,
    encoding_aes_key: &str,
    expected_receive_id: Option<&str>,
) -> Result<String, WeComAdapterError> {
    let aes_key = decode_wecom_aes_key(encoding_aes_key)?;
    let mut ciphertext = general_purpose::STANDARD.decode(encrypt).map_err(|error| {
        WeComAdapterError::new(
            "wecom_encrypt_base64_invalid",
            format!("WeCom encrypted payload is not valid base64: {error}"),
        )
    })?;
    if ciphertext.len() % 16 != 0 {
        return Err(WeComAdapterError::new(
            "wecom_ciphertext_invalid",
            "WeCom encrypted payload length must be a multiple of AES block size",
        ));
    }

    let iv = &aes_key[..16];
    let decrypted = cbc::Decryptor::<Aes256>::new_from_slices(&aes_key, iv)
        .map_err(|error| {
            WeComAdapterError::new(
                "wecom_cipher_init_failed",
                format!("WeCom AES decryptor could not be initialized: {error}"),
            )
        })?
        .decrypt_padded_mut::<NoPadding>(&mut ciphertext)
        .map_err(|error| {
            WeComAdapterError::new(
                "wecom_decrypt_failed",
                format!("WeCom encrypted payload could not be decrypted: {error}"),
            )
        })?;
    let unpadded_len = strip_wecom_pkcs7_padding(decrypted)?;
    let unpadded = &decrypted[..unpadded_len];
    if unpadded.len() < 20 {
        return Err(WeComAdapterError::new(
            "wecom_plaintext_invalid",
            "WeCom decrypted payload is too short",
        ));
    }
    let msg_len =
        u32::from_be_bytes([unpadded[16], unpadded[17], unpadded[18], unpadded[19]]) as usize;
    let msg_start = 20;
    let msg_end = msg_start + msg_len;
    if msg_end > unpadded.len() {
        return Err(WeComAdapterError::new(
            "wecom_plaintext_invalid",
            "WeCom decrypted payload message length exceeds payload size",
        ));
    }
    let msg = std::str::from_utf8(&unpadded[msg_start..msg_end]).map_err(|error| {
        WeComAdapterError::new(
            "wecom_plaintext_invalid",
            format!("WeCom decrypted XML is not UTF-8: {error}"),
        )
    })?;
    let receive_id = std::str::from_utf8(&unpadded[msg_end..]).map_err(|error| {
        WeComAdapterError::new(
            "wecom_receive_id_invalid",
            format!("WeCom decrypted receive id is not UTF-8: {error}"),
        )
    })?;
    if let Some(expected) = expected_receive_id.filter(|value| !value.trim().is_empty()) {
        if receive_id != expected {
            return Err(WeComAdapterError::new(
                "wecom_receive_id_mismatch",
                "WeCom decrypted receive id did not match the configured connection",
            ));
        }
    }
    Ok(msg.to_string())
}

fn decode_wecom_aes_key(encoding_aes_key: &str) -> Result<[u8; 32], WeComAdapterError> {
    if encoding_aes_key.len() != 43 {
        return Err(WeComAdapterError::new(
            "wecom_encoding_aes_key_invalid",
            "WeCom EncodingAESKey must be 43 characters",
        ));
    }
    let decoded = general_purpose::STANDARD
        .decode(format!("{encoding_aes_key}="))
        .map_err(|error| {
            WeComAdapterError::new(
                "wecom_encoding_aes_key_invalid",
                format!("WeCom EncodingAESKey is not valid base64: {error}"),
            )
        })?;
    decoded.try_into().map_err(|_| {
        WeComAdapterError::new(
            "wecom_encoding_aes_key_invalid",
            "WeCom EncodingAESKey must decode to 32 bytes",
        )
    })
}

fn strip_wecom_pkcs7_padding(decrypted: &[u8]) -> Result<usize, WeComAdapterError> {
    let padding = *decrypted.last().ok_or_else(|| {
        WeComAdapterError::new(
            "wecom_plaintext_invalid",
            "WeCom decrypted payload is empty",
        )
    })? as usize;
    if !(1..=32).contains(&padding) || padding > decrypted.len() {
        return Err(WeComAdapterError::new(
            "wecom_padding_invalid",
            "WeCom decrypted payload has invalid PKCS#7 padding",
        ));
    }
    if !decrypted[decrypted.len() - padding..]
        .iter()
        .all(|byte| *byte as usize == padding)
    {
        return Err(WeComAdapterError::new(
            "wecom_padding_invalid",
            "WeCom decrypted payload has inconsistent PKCS#7 padding",
        ));
    }
    Ok(decrypted.len() - padding)
}

pub fn render_wecom_reply_body(reply: &ExternalBotReplyView, agent_id: impl Into<String>) -> Value {
    let text = wecom_reply_text(reply);
    json!({
        "touser": reply.target_conversation_external_id,
        "agentid": agent_id.into(),
        "msgtype": "text",
        "text": {
            "content": text,
        },
    })
}

fn wecom_message_from_fields(
    config: &WeComAdapterConfig,
    fields: &std::collections::BTreeMap<String, String>,
    now: DateTime<Utc>,
) -> Result<ExternalBotMessageView, WeComAdapterError> {
    let sender_external_id =
        first_field(fields, &["FromUserName", "ExternalUserID"]).ok_or_else(|| {
            WeComAdapterError::new(
                "wecom_sender_missing",
                "WeCom decrypted message is missing FromUserName",
            )
        })?;
    let message_external_id = first_field(fields, &["MsgId", "MsgID"]).unwrap_or_else(|| {
        format!(
            "{}:{}",
            fields.get("CreateTime").cloned().unwrap_or_default(),
            sender_external_id
        )
    });
    let message_type = fields
        .get("MsgType")
        .map(|value| wecom_message_type(value))
        .unwrap_or(ExternalMessageTypeView::Unknown);
    let conversation_external_id =
        first_field(fields, &["ChatId", "OpenKfId"]).unwrap_or_else(|| sender_external_id.clone());

    Ok(ExternalBotMessageView {
        platform: ExternalChannelPlatformView::WeCom,
        tenant_external_id: config.tenant_external_id.clone(),
        bot_external_id: config.bot_external_id.clone(),
        conversation_external_id,
        thread_external_id: first_field(fields, &["ThreadId", "RootId"]),
        sender_external_id,
        message_external_id: message_external_id.clone(),
        message_type,
        text: first_field(fields, &["Content", "EventKey", "Event"]),
        default_prompt: None,
        output_format: None,
        render_mode: None,
        artifact_type: None,
        template: None,
        mention_external_user_ids: Vec::new(),
        attachment_refs: wecom_attachment_refs(fields),
        business_datasource_ids: Vec::new(),
        available_document_external_ids: Vec::new(),
        available_document_source_id: None,
        dataset_external_id: None,
        dataset_external_ids: Vec::new(),
        requested_skills: Vec::new(),
        idempotency_key: format!(
            "we_com:{}:{}:{}",
            config.tenant_external_id, config.bot_external_id, message_external_id
        ),
        received_at: fields
            .get("CreateTime")
            .and_then(|value| value.parse::<i64>().ok())
            .and_then(|seconds| DateTime::<Utc>::from_timestamp(seconds, 0))
            .unwrap_or(now),
    })
}

fn parse_wecom_xml(
    xml: &str,
) -> Result<std::collections::BTreeMap<String, String>, WeComAdapterError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut current_tag: Option<String> = None;
    let mut fields = std::collections::BTreeMap::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                current_tag = Some(String::from_utf8_lossy(event.name().as_ref()).to_string());
            }
            Ok(Event::End(_)) => {
                current_tag = None;
            }
            Ok(Event::Text(event)) => {
                if let Some(tag) = current_tag.as_ref() {
                    let value = event.decode().map_err(|error| {
                        WeComAdapterError::new(
                            "wecom_xml_invalid",
                            format!("WeCom XML text decode failed: {error}"),
                        )
                    })?;
                    fields.insert(tag.clone(), value.into_owned());
                }
            }
            Ok(Event::CData(event)) => {
                if let Some(tag) = current_tag.as_ref() {
                    fields.insert(
                        tag.clone(),
                        String::from_utf8_lossy(event.as_ref()).to_string(),
                    );
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => {
                return Err(WeComAdapterError::new(
                    "wecom_xml_invalid",
                    format!("WeCom callback XML could not be parsed: {error}"),
                ));
            }
            _ => {}
        }
    }
    Ok(fields)
}

fn wecom_message_type(value: &str) -> ExternalMessageTypeView {
    match value {
        "text" => ExternalMessageTypeView::Text,
        "image" => ExternalMessageTypeView::Image,
        "file" => ExternalMessageTypeView::File,
        "voice" => ExternalMessageTypeView::Audio,
        "video" => ExternalMessageTypeView::Video,
        "event" => ExternalMessageTypeView::Event,
        _ => ExternalMessageTypeView::Unknown,
    }
}

fn wecom_attachment_refs(
    fields: &std::collections::BTreeMap<String, String>,
) -> Vec<ExternalAttachmentRefView> {
    let mut refs = Vec::new();
    for key in ["MediaId", "PicUrl"] {
        if let Some(attachment_external_id) = fields.get(key).filter(|value| !value.is_empty()) {
            refs.push(ExternalAttachmentRefView {
                attachment_external_id: attachment_external_id.clone(),
                filename: fields.get("FileName").cloned(),
                content_type: None,
                size_bytes: None,
                download_url_redacted: if key == "PicUrl" {
                    Some("redacted:wecom-image-url".to_string())
                } else {
                    None
                },
            });
        }
    }
    refs
}

fn wecom_reply_text(reply: &ExternalBotReplyView) -> String {
    let mut text = reply
        .text
        .clone()
        .or_else(|| reply.task_status.clone())
        .unwrap_or_else(|| "DataMax task accepted.".to_string());
    if matches!(
        reply.reply_type,
        ExternalBotReplyTypeView::RequiresConfirmation
    ) && !reply.requires_confirmation
    {
        text.push_str("\nConfirmation state unavailable.");
    }
    if !reply.artifact_links.is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&reply.artifact_links.join("\n"));
    }
    text
}

fn first_field(
    fields: &std::collections::BTreeMap<String, String>,
    names: &[&str],
) -> Option<String> {
    names
        .iter()
        .find_map(|name| fields.get(*name))
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sample_config() -> WeComAdapterConfig {
        WeComAdapterConfig {
            tenant_external_id: "corp_001".to_string(),
            bot_external_id: "1000002".to_string(),
            token: "token".to_string(),
            encoding_aes_key: None,
            receive_id: None,
            max_clock_skew_seconds: 300,
        }
    }

    #[test]
    fn wecom_signature_matches_sanitized_fixture_and_rejects_replay() {
        let signature = wecom_callback_signature("token", "1700000000", "nonce-001", "ENCRYPT_STR");
        assert_eq!(signature, "4097c34e24a3da10060604ac5a5d5c2ada0f8dbe");
        let params = WeComCallbackParams::new(signature, "1700000000", "nonce-001");
        let now = Utc.timestamp_opt(1_700_000_000, 0).single().unwrap();
        let mut replay_guard = WeComReplayGuard::default();

        validate_wecom_callback(
            &params,
            "token",
            "ENCRYPT_STR",
            now,
            300,
            Some(&mut replay_guard),
        )
        .expect("first callback should validate");
        let replay = validate_wecom_callback(
            &params,
            "token",
            "ENCRYPT_STR",
            now,
            300,
            Some(&mut replay_guard),
        )
        .expect_err("duplicate callback should be rejected");
        assert_eq!(replay.code, "wecom_replay_detected");
    }

    #[test]
    fn wecom_event_normalizes_decrypted_text_xml_to_external_bot_message() {
        let config = sample_config();
        let encrypted_xml = "<xml><ToUserName><![CDATA[corp_001]]></ToUserName><AgentID><![CDATA[1000002]]></AgentID><Encrypt><![CDATA[ENCRYPT_STR]]></Encrypt></xml>";
        let decrypted_xml = "<xml><ToUserName><![CDATA[corp_001]]></ToUserName><FromUserName><![CDATA[user_alpha]]></FromUserName><CreateTime>1700000000</CreateTime><MsgType><![CDATA[text]]></MsgType><Content><![CDATA[hello V3]]></Content><MsgId>msg-001</MsgId><AgentID>1000002</AgentID></xml>";
        let signature = wecom_callback_signature("token", "1700000000", "nonce-002", "ENCRYPT_STR");
        let params = WeComCallbackParams::new(signature, "1700000000", "nonce-002");
        let now = Utc.timestamp_opt(1_700_000_000, 0).single().unwrap();
        let mut replay_guard = WeComReplayGuard::default();

        let outcome = normalize_wecom_callback(
            &config,
            &params,
            encrypted_xml,
            Some(decrypted_xml),
            now,
            Some(&mut replay_guard),
        )
        .expect("message should normalize");
        let WeComCallbackOutcome::Message(message) = outcome else {
            panic!("expected message outcome");
        };
        assert_eq!(message.platform, ExternalChannelPlatformView::WeCom);
        assert_eq!(message.tenant_external_id, "corp_001");
        assert_eq!(message.bot_external_id, "1000002");
        assert_eq!(message.conversation_external_id, "user_alpha");
        assert_eq!(message.sender_external_id, "user_alpha");
        assert_eq!(message.message_external_id, "msg-001");
        assert_eq!(message.message_type, ExternalMessageTypeView::Text);
        assert_eq!(message.text.as_deref(), Some("hello V3"));
        assert_eq!(message.idempotency_key, "we_com:corp_001:1000002:msg-001");
    }

    #[test]
    fn wecom_reply_renders_text_send_body() {
        let reply = ExternalBotReplyView {
            target_conversation_external_id: "user_alpha".to_string(),
            reply_type: ExternalBotReplyTypeView::TaskStatus,
            text: None,
            card: None,
            artifact_links: Vec::new(),
            task_status: Some("accepted".to_string()),
            requires_confirmation: false,
            action_id: None,
            confirmation_id: None,
        };

        let payload = render_wecom_reply_body(&reply, "1000002");
        assert_eq!(payload["touser"], json!("user_alpha"));
        assert_eq!(payload["agentid"], json!("1000002"));
        assert_eq!(payload["msgtype"], json!("text"));
        assert_eq!(payload["text"]["content"], json!("accepted"));
    }
}
