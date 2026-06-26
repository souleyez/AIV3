use axum::http::{header, HeaderMap};

pub(crate) fn external_channel_authorization_bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?.trim();
    let mut parts = value.split_whitespace();
    let scheme = parts.next()?;
    let token = parts.next()?;
    if parts.next().is_some() || !scheme.eq_ignore_ascii_case("bearer") || token.is_empty() {
        return None;
    }
    Some(token)
}

pub(crate) fn external_channel_constant_time_str_eq(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    let max_len = left.len().max(right.len());
    let mut diff = left.len() ^ right.len();
    for index in 0..max_len {
        let left_byte = left.get(index).copied().unwrap_or(0);
        let right_byte = right.get(index).copied().unwrap_or(0);
        diff |= usize::from(left_byte ^ right_byte);
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers_with_authorization(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, value.parse().unwrap());
        headers
    }

    #[test]
    fn external_channel_authorization_bearer_token_accepts_single_bearer_token() {
        let headers = headers_with_authorization(&format!("{} {}", "Bearer", "inbound-token"));
        assert_eq!(
            external_channel_authorization_bearer_token(&headers),
            Some("inbound-token")
        );

        let headers = headers_with_authorization(&format!("{} {}", "bearer", "another-token"));
        assert_eq!(
            external_channel_authorization_bearer_token(&headers),
            Some("another-token")
        );
    }

    #[test]
    fn external_channel_authorization_bearer_token_rejects_malformed_values() {
        assert!(external_channel_authorization_bearer_token(&HeaderMap::new()).is_none());

        let headers = headers_with_authorization("Basic inbound-token");
        assert!(external_channel_authorization_bearer_token(&headers).is_none());

        let headers = headers_with_authorization("Bearer");
        assert!(external_channel_authorization_bearer_token(&headers).is_none());

        let headers =
            headers_with_authorization(&format!("{} {} {}", "Bearer", "inbound-token", "extra"));
        assert!(external_channel_authorization_bearer_token(&headers).is_none());
    }

    #[test]
    fn external_channel_constant_time_str_eq_preserves_equality_semantics() {
        assert!(external_channel_constant_time_str_eq(
            "inbound-token",
            "inbound-token"
        ));
        assert!(!external_channel_constant_time_str_eq(
            "inbound-token",
            "other-token"
        ));
        assert!(!external_channel_constant_time_str_eq("short", "shorter"));
    }
}
