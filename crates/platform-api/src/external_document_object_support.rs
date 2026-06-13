use std::{fs, net::IpAddr, path::PathBuf};

use crate::ApiError;

pub(crate) fn validate_external_document_content_url(
    url: &reqwest::Url,
    allow_http_loopback: bool,
) -> std::result::Result<(), ApiError> {
    match url.scheme() {
        "https" => {}
        "http" if allow_http_loopback && is_loopback_url_host(url.host_str()) => {}
        _ => {
            return Err(ApiError::bad_request(
                "external_document_content_url_insecure",
                "content_url must use HTTPS; HTTP is only allowed for loopback smoke tests"
                    .to_string(),
            ))
        }
    }
    if let Some(host) = url.host_str() {
        if let Ok(ip) = host.parse::<IpAddr>() {
            if ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_multicast()
                || match ip {
                    IpAddr::V4(value) => {
                        value.is_private() || value.is_link_local() || value.is_broadcast()
                    }
                    IpAddr::V6(value) => value.is_unique_local() || value.is_unicast_link_local(),
                }
            {
                if !(allow_http_loopback && ip.is_loopback()) {
                    return Err(ApiError::bad_request(
                        "external_document_content_url_private_host",
                        "content_url host must not be a private or local IP".to_string(),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn is_loopback_url_host(host: Option<&str>) -> bool {
    matches!(host, Some("127.0.0.1" | "localhost" | "::1"))
}

pub(crate) fn external_document_object_root() -> std::result::Result<PathBuf, ApiError> {
    let root = std::env::var("PLATFORM_LOCAL_OBJECT_ROOT")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("ai-data-platform-v3-objects"));
    fs::create_dir_all(&root).map_err(|error| {
        ApiError::internal(
            "external_document_store_failed",
            format!("failed to create local object root: {error}"),
        )
    })?;
    Ok(root)
}

pub(crate) fn external_document_file_extension(
    url: &reqwest::Url,
    content_type: Option<&str>,
) -> Option<String> {
    url.path_segments()
        .and_then(|mut segments| segments.next_back())
        .and_then(|filename| filename.rsplit_once('.').map(|(_, ext)| ext))
        .map(|ext| format!(".{}", safe_external_path_segment(ext)))
        .filter(|ext| ext.len() > 1 && ext.len() <= 12)
        .or_else(|| content_type.and_then(external_document_extension_from_content_type))
}

pub(crate) fn external_document_extension_from_content_type(content_type: &str) -> Option<String> {
    let normalized = content_type
        .split(';')
        .next()
        .unwrap_or(content_type)
        .trim()
        .to_ascii_lowercase();
    let extension = match normalized.as_str() {
        "text/plain" => ".txt",
        "text/markdown" => ".md",
        "text/html" => ".html",
        "application/pdf" => ".pdf",
        "application/json" => ".json",
        "application/zip" | "application/x-zip-compressed" => ".zip",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => ".docx",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => ".xlsx",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => ".pptx",
        _ => return None,
    };
    Some(extension.to_string())
}

pub(crate) fn safe_external_path_segment(value: &str) -> String {
    let mut output = String::new();
    for ch in value.trim().chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            output.push(ch);
        } else {
            output.push('_');
        }
    }
    if output.is_empty() {
        "unknown".to_string()
    } else {
        output.chars().take(120).collect()
    }
}

pub(crate) fn external_document_redact_url(url: &reqwest::Url) -> String {
    format!(
        "{}://{}{}{}",
        url.scheme(),
        url.host_str().unwrap_or("[unknown]"),
        url.port()
            .map(|port| format!(":{port}"))
            .unwrap_or_default(),
        url.path()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{http::StatusCode, response::IntoResponse};

    fn error_status(result: std::result::Result<(), ApiError>) -> Option<StatusCode> {
        let error = result.err()?;
        Some(error.into_response().status())
    }

    #[test]
    fn validate_external_document_content_url_keeps_existing_https_and_loopback_rules() {
        let public_https = reqwest::Url::parse("https://files.example.com/docs/a.pdf?token=secret")
            .expect("url should parse");
        assert!(validate_external_document_content_url(&public_https, false).is_ok());

        let loopback_http =
            reqwest::Url::parse("http://127.0.0.1:3900/doc.md").expect("url should parse");
        assert_eq!(
            error_status(validate_external_document_content_url(
                &loopback_http,
                false
            )),
            Some(StatusCode::BAD_REQUEST)
        );
        assert!(validate_external_document_content_url(&loopback_http, true).is_ok());

        let private_https =
            reqwest::Url::parse("https://192.168.1.10/private.docx").expect("url should parse");
        assert_eq!(
            error_status(validate_external_document_content_url(
                &private_https,
                false
            )),
            Some(StatusCode::BAD_REQUEST)
        );
    }

    #[test]
    fn external_document_file_extension_prefers_short_url_extension_then_content_type() {
        let docx_url =
            reqwest::Url::parse("https://files.example.com/folder/report.final.docx?token=secret")
                .expect("url should parse");
        assert_eq!(
            external_document_file_extension(&docx_url, Some("application/pdf")).as_deref(),
            Some(".docx")
        );

        let long_ext_url = reqwest::Url::parse(
            "https://files.example.com/folder/report.verylongextension?token=secret",
        )
        .expect("url should parse");
        assert_eq!(
            external_document_file_extension(
                &long_ext_url,
                Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
            )
            .as_deref(),
            Some(".xlsx")
        );
    }

    #[test]
    fn path_segment_and_redacted_url_preserve_existing_public_shapes() {
        assert_eq!(
            safe_external_path_segment(" 资料 1/合同:rev "),
            "___1____rev"
        );
        assert_eq!(safe_external_path_segment("   "), "unknown");

        let long = "a".repeat(130);
        assert_eq!(safe_external_path_segment(&long).len(), 120);

        let url =
            reqwest::Url::parse("https://user:pass@files.example.com:8443/a/b.docx?token=secret")
                .expect("url should parse");
        assert_eq!(
            external_document_redact_url(&url),
            "https://files.example.com:8443/a/b.docx"
        );
    }
}
