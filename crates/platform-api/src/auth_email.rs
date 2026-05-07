use anyhow::{anyhow, Result};
use chrono::{DateTime, Duration, Utc};
use domain_model::{AuthChallengePurpose, EmailVerificationChallenge};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::env;
use std::time::Duration as StdDuration;
use storage::NewEmailVerificationChallenge;
use uuid::Uuid;

const DEFAULT_CODE_BYTES_HEX_LEN: usize = 8;
const DEFAULT_TTL_MINUTES: i64 = 10;
const DEFAULT_MAX_ATTEMPTS: i32 = 5;
const CLOUDFLARE_EMAIL_API_BASE: &str = "https://api.cloudflare.com/client/v4";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerificationEmailMessage {
    pub to: String,
    pub purpose: AuthChallengePurpose,
    pub code: String,
}

pub trait EmailSender {
    fn send_verification_code(&self, message: &VerificationEmailMessage) -> Result<()>;
}

#[derive(Clone, Debug, Default)]
pub struct LoggingEmailSender;

impl EmailSender for LoggingEmailSender {
    fn send_verification_code(&self, message: &VerificationEmailMessage) -> Result<()> {
        tracing::info!(
            email = %message.to,
            purpose = message.purpose.as_str(),
            "queued verification code email through logging sender"
        );
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct CloudflareEmailConfig {
    pub account_id: String,
    pub api_token: String,
    pub sender: String,
    pub sender_name: Option<String>,
    pub reply_to: Option<String>,
    pub api_base_url: String,
}

impl CloudflareEmailConfig {
    pub fn from_env() -> Result<Option<Self>> {
        let account_id = read_optional_env("CLOUDFLARE_EMAIL_ACCOUNT_ID");
        let api_token = read_optional_env("CLOUDFLARE_EMAIL_API_TOKEN");
        let sender = read_optional_env("CLOUDFLARE_EMAIL_SENDER");
        if account_id.is_none() && api_token.is_none() && sender.is_none() {
            return Ok(None);
        }
        let account_id = account_id.ok_or_else(|| {
            anyhow!("CLOUDFLARE_EMAIL_ACCOUNT_ID is required when Cloudflare email is enabled")
        })?;
        let api_token = api_token.ok_or_else(|| {
            anyhow!("CLOUDFLARE_EMAIL_API_TOKEN is required when Cloudflare email is enabled")
        })?;
        let sender = sender.ok_or_else(|| {
            anyhow!("CLOUDFLARE_EMAIL_SENDER is required when Cloudflare email is enabled")
        })?;
        Ok(Some(Self {
            account_id,
            api_token,
            sender,
            sender_name: read_optional_env("CLOUDFLARE_EMAIL_SENDER_NAME"),
            reply_to: read_optional_env("CLOUDFLARE_EMAIL_REPLY_TO"),
            api_base_url: read_optional_env("CLOUDFLARE_EMAIL_API_BASE_URL")
                .unwrap_or_else(|| CLOUDFLARE_EMAIL_API_BASE.to_string()),
        }))
    }

    fn send_endpoint(&self) -> String {
        format!(
            "{}/accounts/{}/email/sending/send",
            self.api_base_url.trim_end_matches('/'),
            self.account_id
        )
    }
}

#[derive(Clone, Debug)]
pub struct CloudflareEmailSender {
    config: CloudflareEmailConfig,
    client: Client,
}

impl CloudflareEmailSender {
    pub fn new(config: CloudflareEmailConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(StdDuration::from_secs(10))
            .build()?;
        Ok(Self { config, client })
    }
}

impl EmailSender for CloudflareEmailSender {
    fn send_verification_code(&self, message: &VerificationEmailMessage) -> Result<()> {
        let payload = build_cloudflare_email_payload(&self.config, message);
        let response = self
            .client
            .post(self.config.send_endpoint())
            .bearer_auth(&self.config.api_token)
            .json(&payload)
            .send()?;
        let status = response.status();
        let api_response = response.json::<CloudflareEmailApiResponse>()?;
        if status.is_success() && api_response.success {
            return Ok(());
        }
        let reason = api_response
            .errors
            .first()
            .map(|error| format!("{}: {}", error.code, error.message))
            .unwrap_or_else(|| format!("Cloudflare email API returned {status}"));
        Err(anyhow!(
            "Cloudflare verification email send failed: {reason}"
        ))
    }
}

pub fn send_verification_email(message: &VerificationEmailMessage) -> Result<()> {
    if let Some(config) = CloudflareEmailConfig::from_env()? {
        return CloudflareEmailSender::new(config)?.send_verification_code(message);
    }
    LoggingEmailSender.send_verification_code(message)
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct CloudflareEmailPayload {
    to: String,
    from: CloudflareEmailFrom,
    subject: String,
    text: String,
    html: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reply_to: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct CloudflareEmailFrom {
    address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct CloudflareEmailApiResponse {
    #[serde(default)]
    success: bool,
    #[serde(default)]
    errors: Vec<CloudflareEmailApiError>,
}

#[derive(Clone, Debug, Deserialize)]
struct CloudflareEmailApiError {
    code: i64,
    message: String,
}

fn build_cloudflare_email_payload(
    config: &CloudflareEmailConfig,
    message: &VerificationEmailMessage,
) -> CloudflareEmailPayload {
    let purpose = verification_purpose_label(&message.purpose);
    let subject = format!("AI Data Platform 验证码：{}", message.code);
    let text = format!(
        "你正在进行 {purpose}。\n\n验证码：{}\n\n验证码 10 分钟内有效。请不要把验证码告诉其他人；我们不会通过邮件索要你的本地密钥。",
        message.code
    );
    let html = format!(
        "<p>你正在进行 <strong>{purpose}</strong>。</p><p>验证码：</p><p style=\"font-size:24px;font-weight:700;letter-spacing:4px\">{}</p><p>验证码 10 分钟内有效。请不要把验证码告诉其他人；我们不会通过邮件索要你的本地密钥。</p>",
        message.code
    );
    CloudflareEmailPayload {
        to: message.to.clone(),
        from: CloudflareEmailFrom {
            address: config.sender.clone(),
            name: config.sender_name.clone(),
        },
        subject,
        text,
        html,
        reply_to: config.reply_to.clone(),
    }
}

fn verification_purpose_label(purpose: &AuthChallengePurpose) -> &'static str {
    match purpose {
        AuthChallengePurpose::AccountCreate => "创建账号",
        AuthChallengePurpose::Login => "登录账号",
        AuthChallengePurpose::RecoverKey => "恢复账号访问",
        AuthChallengePurpose::BindEmail => "绑定邮箱",
        AuthChallengePurpose::RotateKey => "更换本地密钥",
    }
}

fn read_optional_env(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[derive(Clone, Debug)]
pub struct EmailOtpService {
    pepper: String,
    ttl: Duration,
    max_attempts: i32,
}

#[derive(Clone, Debug)]
pub struct PreparedEmailChallenge {
    pub challenge: NewEmailVerificationChallenge,
    pub email: VerificationEmailMessage,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OtpVerificationStatus {
    Valid,
    Invalid,
    Expired,
    Consumed,
    AttemptsExceeded,
}

impl EmailOtpService {
    pub fn new(pepper: impl Into<String>) -> Result<Self> {
        let pepper = pepper.into();
        if pepper.trim().is_empty() {
            return Err(anyhow!("email OTP pepper must not be empty"));
        }
        Ok(Self {
            pepper,
            ttl: Duration::minutes(DEFAULT_TTL_MINUTES),
            max_attempts: DEFAULT_MAX_ATTEMPTS,
        })
    }

    pub fn with_policy(
        pepper: impl Into<String>,
        ttl: Duration,
        max_attempts: i32,
    ) -> Result<Self> {
        let pepper = pepper.into();
        if pepper.trim().is_empty() {
            return Err(anyhow!("email OTP pepper must not be empty"));
        }
        if ttl <= Duration::zero() {
            return Err(anyhow!("email OTP ttl must be positive"));
        }
        if max_attempts <= 0 {
            return Err(anyhow!("email OTP max attempts must be positive"));
        }
        Ok(Self {
            pepper,
            ttl,
            max_attempts,
        })
    }

    pub fn prepare_challenge(
        &self,
        email: &str,
        purpose: AuthChallengePurpose,
        now: DateTime<Utc>,
    ) -> PreparedEmailChallenge {
        let email_normalized = normalize_email(email);
        let code = generate_otp_code();
        let code_hash = self.hash_code(&email_normalized, &purpose, &code);
        PreparedEmailChallenge {
            challenge: NewEmailVerificationChallenge {
                email_normalized: email_normalized.clone(),
                purpose: purpose.clone(),
                code_hash,
                max_attempts: self.max_attempts,
                expires_at: now + self.ttl,
                metadata: Value::Null,
                created_at: now,
            },
            email: VerificationEmailMessage {
                to: email_normalized,
                purpose,
                code,
            },
        }
    }

    pub fn verify_challenge(
        &self,
        challenge: &EmailVerificationChallenge,
        code: &str,
        now: DateTime<Utc>,
    ) -> OtpVerificationStatus {
        if challenge.consumed_at.is_some() {
            return OtpVerificationStatus::Consumed;
        }
        if now > challenge.expires_at {
            return OtpVerificationStatus::Expired;
        }
        if challenge.attempt_count >= challenge.max_attempts {
            return OtpVerificationStatus::AttemptsExceeded;
        }
        let expected = self.hash_code(&challenge.email_normalized, &challenge.purpose, code);
        if expected == challenge.code_hash {
            OtpVerificationStatus::Valid
        } else {
            OtpVerificationStatus::Invalid
        }
    }

    fn hash_code(
        &self,
        email_normalized: &str,
        purpose: &AuthChallengePurpose,
        code: &str,
    ) -> String {
        hash_otp_code(&self.pepper, email_normalized, purpose, code)
    }
}

pub fn normalize_email(email: &str) -> String {
    email.trim().to_ascii_lowercase()
}

fn normalize_code(code: &str) -> String {
    code.trim().to_ascii_uppercase()
}

fn generate_otp_code() -> String {
    Uuid::new_v4()
        .simple()
        .to_string()
        .chars()
        .take(DEFAULT_CODE_BYTES_HEX_LEN)
        .collect::<String>()
        .to_ascii_uppercase()
}

fn hash_otp_code(
    pepper: &str,
    email_normalized: &str,
    purpose: &AuthChallengePurpose,
    code: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(pepper.as_bytes());
    hasher.update(b":");
    hasher.update(email_normalized.as_bytes());
    hasher.update(b":");
    hasher.update(purpose.as_str().as_bytes());
    hasher.update(b":");
    hasher.update(normalize_code(code).as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain_model::{EmailVerificationChallengeId, TenantId};

    fn test_challenge(
        service: &EmailOtpService,
        code: &str,
        now: DateTime<Utc>,
        patch: impl FnOnce(&mut EmailVerificationChallenge),
    ) -> EmailVerificationChallenge {
        let mut challenge = EmailVerificationChallenge {
            id: EmailVerificationChallengeId::new(),
            tenant_id: TenantId::new(),
            email_normalized: "user@example.com".to_string(),
            purpose: AuthChallengePurpose::Login,
            code_hash: service.hash_code("user@example.com", &AuthChallengePurpose::Login, code),
            attempt_count: 0,
            max_attempts: 5,
            expires_at: now + Duration::minutes(10),
            consumed_at: None,
            metadata: Value::Null,
            created_at: now,
        };
        patch(&mut challenge);
        challenge
    }

    #[test]
    fn otp_hash_does_not_store_plain_code() {
        let service = EmailOtpService::new("server-pepper").expect("service");
        let prepared = service.prepare_challenge(
            " User@Example.COM ",
            AuthChallengePurpose::RecoverKey,
            Utc::now(),
        );

        assert_eq!(prepared.challenge.email_normalized, "user@example.com");
        assert_eq!(prepared.email.to, "user@example.com");
        assert_eq!(prepared.email.code.len(), DEFAULT_CODE_BYTES_HEX_LEN);
        assert!(!prepared.challenge.code_hash.contains(&prepared.email.code));
    }

    #[test]
    fn expired_code_is_rejected() {
        let service = EmailOtpService::new("server-pepper").expect("service");
        let now = Utc::now();
        let challenge = test_challenge(&service, "ABC12345", now, |challenge| {
            challenge.expires_at = now - Duration::seconds(1);
        });

        assert_eq!(
            service.verify_challenge(&challenge, "ABC12345", now),
            OtpVerificationStatus::Expired
        );
    }

    #[test]
    fn wrong_code_is_rejected_before_attempt_increment() {
        let service = EmailOtpService::new("server-pepper").expect("service");
        let now = Utc::now();
        let challenge = test_challenge(&service, "ABC12345", now, |_| {});

        assert_eq!(
            service.verify_challenge(&challenge, "WRONG", now),
            OtpVerificationStatus::Invalid
        );
    }

    #[test]
    fn consumed_code_cannot_be_reused() {
        let service = EmailOtpService::new("server-pepper").expect("service");
        let now = Utc::now();
        let challenge = test_challenge(&service, "ABC12345", now, |challenge| {
            challenge.consumed_at = Some(now);
        });

        assert_eq!(
            service.verify_challenge(&challenge, "ABC12345", now),
            OtpVerificationStatus::Consumed
        );
    }

    #[test]
    fn attempts_exceeded_is_rejected_before_hash_check() {
        let service = EmailOtpService::new("server-pepper").expect("service");
        let now = Utc::now();
        let challenge = test_challenge(&service, "ABC12345", now, |challenge| {
            challenge.attempt_count = challenge.max_attempts;
        });

        assert_eq!(
            service.verify_challenge(&challenge, "ABC12345", now),
            OtpVerificationStatus::AttemptsExceeded
        );
    }

    #[test]
    fn cloudflare_payload_uses_sender_and_never_mentions_raw_key_recovery() {
        let config = CloudflareEmailConfig {
            account_id: "account".to_string(),
            api_token: "token".to_string(),
            sender: "verify@example.com".to_string(),
            sender_name: Some("AI Data Platform".to_string()),
            reply_to: Some("support@example.com".to_string()),
            api_base_url: CLOUDFLARE_EMAIL_API_BASE.to_string(),
        };
        let payload = build_cloudflare_email_payload(
            &config,
            &VerificationEmailMessage {
                to: "user@example.com".to_string(),
                purpose: AuthChallengePurpose::RecoverKey,
                code: "ABC12345".to_string(),
            },
        );

        assert_eq!(payload.to, "user@example.com");
        assert_eq!(payload.from.address, "verify@example.com");
        assert_eq!(payload.from.name.as_deref(), Some("AI Data Platform"));
        assert_eq!(payload.reply_to.as_deref(), Some("support@example.com"));
        assert!(payload.text.contains("ABC12345"));
        assert!(payload.text.contains("恢复账号访问"));
        assert!(payload.text.contains("不会通过邮件索要你的本地密钥"));
        assert!(!payload.text.contains("找回原密钥"));
    }
}
