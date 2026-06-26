use chrono::{DateTime, SecondsFormat, Utc};
use hmac::{Hmac, KeyInit, Mac};
use serde_json::Value;
use sha2::Sha256;

use crate::{bytes_to_lower_hex, external_channel_support::ExternalActionDispatchAuth, sha256_hex};

pub(crate) fn external_action_dispatch_headers(
    connection_id: &str,
    url: &reqwest::Url,
    body: &[u8],
    now: DateTime<Utc>,
    nonce: &str,
    auth: &ExternalActionDispatchAuth,
) -> std::result::Result<reqwest::header::HeaderMap, String> {
    let timestamp = now.to_rfc3339_opts(SecondsFormat::Secs, true);
    let body_sha256 = sha256_hex([body]);
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::CONTENT_TYPE,
        reqwest::header::HeaderValue::from_static("application/json"),
    );
    insert_dispatch_header(&mut headers, "x-v3-connection-id", connection_id)?;
    insert_dispatch_header(&mut headers, "x-v3-timestamp", &timestamp)?;
    insert_dispatch_header(&mut headers, "x-v3-nonce", nonce)?;
    insert_dispatch_header(&mut headers, "x-v3-content-sha256", &body_sha256)?;
    if let Some(token) = auth.bearer_token.as_deref() {
        let header_value = reqwest::header::HeaderValue::from_str(&format!("Bearer {token}"))
            .map_err(|_| "invalid_header_value:authorization".to_string())?;
        headers.insert(reqwest::header::AUTHORIZATION, header_value);
    }
    if let Some(secret) = auth.signing_secret.as_deref() {
        let signature_path = external_action_dispatch_signature_path(url);
        let canonical = external_action_dispatch_signature_payload(
            "POST",
            &signature_path,
            &timestamp,
            nonce,
            &body_sha256,
        );
        let signature = external_action_dispatch_signature_hex(secret, &canonical);
        insert_dispatch_header(
            &mut headers,
            "x-v3-signature",
            &format!("sha256={signature}"),
        )?;
    }
    Ok(headers)
}

pub(crate) fn external_action_dispatch_signature_path(url: &reqwest::Url) -> String {
    match url.query() {
        Some(query) => format!("{}?{query}", url.path()),
        None => url.path().to_string(),
    }
}

pub(crate) fn external_action_dispatch_signature_payload(
    method: &str,
    path: &str,
    timestamp: &str,
    nonce: &str,
    body_sha256: &str,
) -> String {
    format!("{method}\n{path}\n{timestamp}\n{nonce}\n{body_sha256}")
}

pub(crate) fn external_action_dispatch_signature_hex(secret: &str, payload: &str) -> String {
    type HmacSha256 = Hmac<Sha256>;
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(payload.as_bytes());
    bytes_to_lower_hex(mac.finalize().into_bytes().as_slice())
}

pub(crate) fn external_action_reqwest_error_kind(error: &reqwest::Error) -> &'static str {
    if error.is_timeout() {
        "timeout"
    } else if error.is_connect() {
        "connect"
    } else if error.is_request() {
        "request"
    } else if error.is_body() {
        "body"
    } else if error.is_decode() {
        "decode"
    } else {
        "unknown"
    }
}

pub(crate) fn external_action_response_request_id(response: &Value) -> Option<String> {
    response
        .get("external_request_id")
        .or_else(|| response.get("externalRequestId"))
        .or_else(|| response.get("request_id"))
        .or_else(|| response.get("requestId"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn insert_dispatch_header(
    headers: &mut reqwest::header::HeaderMap,
    name: &'static str,
    value: &str,
) -> std::result::Result<(), String> {
    let header_value = reqwest::header::HeaderValue::from_str(value)
        .map_err(|_| format!("invalid_header_value:{name}"))?;
    headers.insert(reqwest::header::HeaderName::from_static(name), header_value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn dispatch_headers_include_bearer_signature_and_canonical_body_hash() {
        let auth = ExternalActionDispatchAuth {
            bearer_token: Some("dispatch-token".to_string()),
            signing_secret: Some("dispatch-secret".to_string()),
        };
        let url = reqwest::Url::parse("https://api.example.com/actions/dispatch?tenant=t1")
            .expect("url should parse");
        let body = br#"{"action_id":"act-001"}"#;
        let now = DateTime::parse_from_rfc3339("2026-05-14T09:30:00Z")
            .expect("timestamp should parse")
            .with_timezone(&Utc);
        let headers = external_action_dispatch_headers(
            "generic-chat-main",
            &url,
            body,
            now,
            "nonce-001",
            &auth,
        )
        .expect("headers should build");

        let body_hash = sha256_hex([body.as_slice()]);
        let canonical = external_action_dispatch_signature_payload(
            "POST",
            "/actions/dispatch?tenant=t1",
            "2026-05-14T09:30:00Z",
            "nonce-001",
            &body_hash,
        );
        let expected_signature =
            external_action_dispatch_signature_hex("dispatch-secret", &canonical);

        assert_eq!(
            headers
                .get("authorization")
                .and_then(|value| value.to_str().ok()),
            Some("Bearer dispatch-token")
        );
        assert_eq!(
            headers
                .get("x-v3-content-sha256")
                .and_then(|value| value.to_str().ok()),
            Some(body_hash.as_str())
        );
        assert_eq!(
            headers
                .get("x-v3-signature")
                .and_then(|value| value.to_str().ok()),
            Some(format!("sha256={expected_signature}").as_str())
        );
    }

    #[test]
    fn dispatch_headers_allow_unsigned_or_unbearered_modes() {
        let url = reqwest::Url::parse("https://api.example.com/actions").expect("url");
        let now = DateTime::parse_from_rfc3339("2026-05-14T09:30:00Z")
            .expect("timestamp should parse")
            .with_timezone(&Utc);
        let headers = external_action_dispatch_headers(
            "generic-chat-main",
            &url,
            b"{}",
            now,
            "nonce-001",
            &ExternalActionDispatchAuth {
                bearer_token: None,
                signing_secret: None,
            },
        )
        .expect("base headers should build");

        assert!(headers.get("authorization").is_none());
        assert!(headers.get("x-v3-signature").is_none());
        assert_eq!(
            headers
                .get("content-type")
                .and_then(|value| value.to_str().ok()),
            Some("application/json")
        );
    }

    #[test]
    fn signature_path_preserves_query_and_omits_fragment() {
        let with_query =
            reqwest::Url::parse("https://api.example.com/actions/dispatch?tenant=t1#ignored")
                .expect("url");
        assert_eq!(
            external_action_dispatch_signature_path(&with_query),
            "/actions/dispatch?tenant=t1"
        );

        let no_query = reqwest::Url::parse("https://api.example.com/actions#ignored").expect("url");
        assert_eq!(
            external_action_dispatch_signature_path(&no_query),
            "/actions"
        );
    }

    #[test]
    fn response_request_id_accepts_known_aliases_and_ignores_empty_values() {
        assert_eq!(
            external_action_response_request_id(&json!({"external_request_id": " ext-1 "})),
            Some("ext-1".to_string())
        );
        assert_eq!(
            external_action_response_request_id(&json!({"externalRequestId": "ext-2"})),
            Some("ext-2".to_string())
        );
        assert_eq!(
            external_action_response_request_id(&json!({"request_id": "ext-3"})),
            Some("ext-3".to_string())
        );
        assert_eq!(
            external_action_response_request_id(&json!({"requestId": "ext-4"})),
            Some("ext-4".to_string())
        );
        assert_eq!(
            external_action_response_request_id(&json!({"requestId": "  "})),
            None
        );
    }
}
