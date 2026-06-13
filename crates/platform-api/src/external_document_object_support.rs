use std::{collections::BTreeMap, fs, net::IpAddr, path::PathBuf};

use domain_model::{Dataset, DatasetLifecycle};
use serde_json::Value;

use crate::sha256_hex;
use crate::text_normalization::non_empty_trimmed_string;
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

pub(crate) fn effective_external_document_parse_dataset_external_id(
    value: Option<&str>,
) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub(crate) fn external_document_parse_dataset_key(
    source_id: &str,
    dataset_external_id: Option<&str>,
) -> String {
    let source_slug = external_document_parse_dataset_key_component(source_id);
    let raw_key = if let Some(dataset_external_id) = dataset_external_id {
        format!(
            "external-source-{}-dataset-{}",
            source_slug,
            external_document_parse_dataset_key_component(dataset_external_id)
        )
    } else {
        format!("external-source-{source_slug}")
    };
    compact_external_document_parse_dataset_key(raw_key)
}

pub(crate) fn external_document_parse_dataset_key_component(value: &str) -> String {
    let mut slug = String::new();
    let mut last_was_separator = false;
    for character in value.trim().chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
            last_was_separator = false;
        } else if !slug.is_empty() && !last_was_separator {
            slug.push('-');
            last_was_separator = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        return format!("ref-{}", &sha256_hex([value.as_bytes()])[..12]);
    }
    if slug.len() > 64 {
        let hash = sha256_hex([value.as_bytes()]);
        slug.truncate(48);
        while slug.ends_with('-') {
            slug.pop();
        }
        slug.push('-');
        slug.push_str(&hash[..12]);
    }
    slug
}

pub(crate) fn compact_external_document_parse_dataset_key(mut key: String) -> String {
    if key.len() <= 128 {
        return key;
    }
    let hash = sha256_hex([key.as_bytes()]);
    key.truncate(112);
    while key.ends_with('-') {
        key.pop();
    }
    key.push('-');
    key.push_str(&hash[..12]);
    key
}

pub(crate) fn external_dataset_matches_external_document_parse_dataset(
    dataset: &Dataset,
    source_id: &str,
    dataset_external_id: &str,
) -> bool {
    if dataset.lifecycle == DatasetLifecycle::Archived {
        return false;
    }
    let expected_key = external_document_parse_dataset_key(source_id, Some(dataset_external_id));
    if dataset.key == expected_key {
        return true;
    }
    external_document_source_id_from_metadata(&dataset.metadata, None).as_deref() == Some(source_id)
        && external_document_dataset_external_ids_from_metadata(&dataset.metadata)
            .iter()
            .any(|value| value == dataset_external_id)
}

pub(crate) fn external_document_source_id_from_metadata(
    metadata: &BTreeMap<String, Value>,
    revision_external_id: Option<&str>,
) -> Option<String> {
    let object = metadata
        .get("external_source")
        .or_else(|| metadata.get("externalSource"))
        .and_then(Value::as_object)?;
    let source_id = object
        .get("source_id")
        .or_else(|| object.get("sourceId"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    if revision_external_id.is_some_and(|revision| {
        object
            .get("revision_external_id")
            .or_else(|| object.get("revisionExternalId"))
            .and_then(Value::as_str)
            != Some(revision)
    }) {
        return None;
    }
    Some(source_id.to_string())
}

pub(crate) fn external_document_external_id_from_metadata(
    metadata: &BTreeMap<String, Value>,
) -> Option<String> {
    metadata
        .get("external_source")
        .or_else(|| metadata.get("externalSource"))
        .and_then(Value::as_object)
        .and_then(|object| {
            object
                .get("document_external_id")
                .or_else(|| object.get("documentExternalId"))
                .and_then(Value::as_str)
        })
        .and_then(non_empty_trimmed_string)
}

pub(crate) fn external_document_dataset_external_ids_from_metadata(
    metadata: &BTreeMap<String, Value>,
) -> Vec<String> {
    let Some(object) = metadata
        .get("external_source")
        .or_else(|| metadata.get("externalSource"))
        .and_then(Value::as_object)
    else {
        return Vec::new();
    };
    let mut values = Vec::new();
    for key in [
        "dataset_external_id",
        "datasetExternalId",
        "requested_dataset_external_id",
        "requestedDatasetExternalId",
    ] {
        if let Some(value) = object
            .get(key)
            .and_then(Value::as_str)
            .and_then(non_empty_trimmed_string)
        {
            if !values.contains(&value) {
                values.push(value);
            }
        }
    }
    values
}

pub(crate) fn external_document_metadata_matches(
    metadata: &BTreeMap<String, Value>,
    source_id: &str,
    document_external_id: &str,
    revision_external_id: Option<&str>,
) -> bool {
    external_document_source_matches(metadata, source_id, revision_external_id)
        && external_document_metadata_document_id_matches(
            metadata,
            document_external_id,
            revision_external_id,
        )
}

pub(crate) fn external_document_metadata_document_id_matches(
    metadata: &BTreeMap<String, Value>,
    document_external_id: &str,
    revision_external_id: Option<&str>,
) -> bool {
    let Some(object) = metadata
        .get("external_source")
        .or_else(|| metadata.get("externalSource"))
        .and_then(Value::as_object)
    else {
        return false;
    };
    object
        .get("document_external_id")
        .or_else(|| object.get("documentExternalId"))
        .and_then(Value::as_str)
        == Some(document_external_id)
        && revision_external_id.is_none_or(|revision| {
            object
                .get("revision_external_id")
                .or_else(|| object.get("revisionExternalId"))
                .and_then(Value::as_str)
                == Some(revision)
        })
}

pub(crate) fn external_document_source_matches(
    metadata: &BTreeMap<String, Value>,
    source_id: &str,
    revision_external_id: Option<&str>,
) -> bool {
    let Some(object) = metadata
        .get("external_source")
        .or_else(|| metadata.get("externalSource"))
        .and_then(Value::as_object)
    else {
        return false;
    };
    object.get("source_id").and_then(Value::as_str) == Some(source_id)
        && revision_external_id.is_none_or(|revision| {
            object
                .get("revision_external_id")
                .or_else(|| object.get("revisionExternalId"))
                .and_then(Value::as_str)
                == Some(revision)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{http::StatusCode, response::IntoResponse};
    use domain_model::{DatasetId, DatasetVisibility, TenantId, UserId};
    use serde_json::json;

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

    fn test_dataset() -> Dataset {
        Dataset {
            id: DatasetId::new(),
            tenant_id: TenantId::new(),
            owner_user_id: Some(UserId::new()),
            key: "dataset-key".to_string(),
            title: "Dataset".to_string(),
            description: None,
            visibility: DatasetVisibility::Public,
            lifecycle: DatasetLifecycle::Active,
            default_secret_binding_ids: Vec::new(),
            metadata: BTreeMap::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn document_parse_dataset_key_keeps_stable_group_ids() {
        assert_eq!(
            effective_external_document_parse_dataset_external_id(Some(
                "345214b9-13cb-4f1d-a3c1-cb000d1e4d81"
            )),
            Some("345214b9-13cb-4f1d-a3c1-cb000d1e4d81".to_string())
        );
        assert_eq!(
            external_document_parse_dataset_key(
                "third-party-source-main",
                Some("345214b9-13cb-4f1d-a3c1-cb000d1e4d81")
            ),
            "external-source-third-party-source-main-dataset-345214b9-13cb-4f1d-a3c1-cb000d1e4d81"
        );

        let long_component = "A".repeat(100);
        assert!(external_document_parse_dataset_key_component(&long_component).len() <= 64);
        assert!(
            compact_external_document_parse_dataset_key(format!(
                "external-source-{}",
                "a".repeat(180)
            ))
            .len()
                <= 128
        );
    }

    #[test]
    fn document_metadata_helpers_accept_snake_and_camel_external_source() {
        let metadata = json!({
            "externalSource": {
                "source_id": "third-party-source-main",
                "documentExternalId": "doc-main",
                "revisionExternalId": "v1",
                "datasetExternalId": "dataset-effective",
                "requestedDatasetExternalId": "dataset-requested"
            }
        })
        .as_object()
        .expect("object")
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();

        assert_eq!(
            external_document_source_id_from_metadata(&metadata, Some("v1")).as_deref(),
            Some("third-party-source-main")
        );
        assert_eq!(
            external_document_source_id_from_metadata(&metadata, Some("v2")),
            None
        );
        assert_eq!(
            external_document_external_id_from_metadata(&metadata).as_deref(),
            Some("doc-main")
        );
        assert_eq!(
            external_document_dataset_external_ids_from_metadata(&metadata),
            vec![
                "dataset-effective".to_string(),
                "dataset-requested".to_string()
            ]
        );
        assert!(external_document_metadata_matches(
            &metadata,
            "third-party-source-main",
            "doc-main",
            Some("v1")
        ));
        assert!(!external_document_metadata_document_id_matches(
            &metadata,
            "other-doc",
            Some("v1")
        ));
    }

    #[test]
    fn dataset_match_accepts_key_or_external_source_metadata_and_rejects_archived() {
        let mut dataset = test_dataset();
        dataset.key = external_document_parse_dataset_key(
            "third-party-source-main",
            Some("345214b9-13cb-4f1d-a3c1-cb000d1e4d81"),
        );
        assert!(external_dataset_matches_external_document_parse_dataset(
            &dataset,
            "third-party-source-main",
            "345214b9-13cb-4f1d-a3c1-cb000d1e4d81"
        ));

        dataset.key = "external-source-third-party-source-main-dataset-current".to_string();
        dataset.metadata.insert(
            "external_source".to_string(),
            json!({
                "source_id": "third-party-source-main",
                "dataset_external_id": "external-source-third-party-source-main-dataset-current",
                "requested_dataset_external_id": "345214b9-13cb-4f1d-a3c1-cb000d1e4d81"
            }),
        );
        assert!(external_dataset_matches_external_document_parse_dataset(
            &dataset,
            "third-party-source-main",
            "external-source-third-party-source-main-dataset-current"
        ));
        assert!(external_dataset_matches_external_document_parse_dataset(
            &dataset,
            "third-party-source-main",
            "345214b9-13cb-4f1d-a3c1-cb000d1e4d81"
        ));

        dataset.lifecycle = DatasetLifecycle::Archived;
        assert!(!external_dataset_matches_external_document_parse_dataset(
            &dataset,
            "third-party-source-main",
            "345214b9-13cb-4f1d-a3c1-cb000d1e4d81"
        ));
    }
}
