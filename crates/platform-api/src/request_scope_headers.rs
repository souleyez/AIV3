use crate::{ApiError, ACTIVE_SECRET_BINDING_IDS_HEADER, LOCAL_THREAD_ID_HEADER};
use axum::http::HeaderMap;
use domain_model::SecretBindingId;
use uuid::Uuid;

pub(crate) fn active_secret_binding_ids_from_headers(
    headers: &HeaderMap,
) -> std::result::Result<Vec<SecretBindingId>, ApiError> {
    let Some(value) = headers.get(ACTIVE_SECRET_BINDING_IDS_HEADER) else {
        return Ok(Vec::new());
    };
    let raw = value.to_str().map_err(|_| {
        ApiError::bad_request(
            "invalid_secret_binding_ids_header",
            format!("{ACTIVE_SECRET_BINDING_IDS_HEADER} must be valid utf-8"),
        )
    })?;
    raw.split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            Uuid::parse_str(entry)
                .map(SecretBindingId)
                .map_err(|error| {
                    ApiError::bad_request(
                        "invalid_secret_binding_id",
                        format!("invalid secret binding id {entry}: {error}"),
                    )
                })
        })
        .collect()
}

pub(crate) fn local_thread_id_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get(LOCAL_THREAD_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

pub(crate) fn merge_secret_binding_ids(
    left: &[SecretBindingId],
    right: &[SecretBindingId],
) -> Vec<SecretBindingId> {
    let mut merged = Vec::with_capacity(left.len() + right.len());
    for id in left.iter().chain(right.iter()) {
        if !merged.iter().any(|existing| existing == id) {
            merged.push(*id);
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderName, HeaderValue, StatusCode};

    #[test]
    fn active_secret_binding_ids_header_parses_trimmed_comma_list() {
        let first = SecretBindingId::new();
        let second = SecretBindingId::new();
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static(ACTIVE_SECRET_BINDING_IDS_HEADER),
            HeaderValue::from_str(&format!(" {first},, {second} ")).expect("valid header"),
        );

        assert_eq!(
            active_secret_binding_ids_from_headers(&headers).expect("header parses"),
            vec![first, second]
        );
    }

    #[test]
    fn active_secret_binding_ids_header_rejects_invalid_uuid_with_existing_error() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static(ACTIVE_SECRET_BINDING_IDS_HEADER),
            HeaderValue::from_static("not-a-uuid"),
        );

        let error =
            active_secret_binding_ids_from_headers(&headers).expect_err("invalid uuid should fail");
        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.payload.code, "invalid_secret_binding_id");
        assert!(error
            .payload
            .message
            .contains("invalid secret binding id not-a-uuid"));
    }

    #[test]
    fn active_secret_binding_ids_header_defaults_to_empty_without_header() {
        assert_eq!(
            active_secret_binding_ids_from_headers(&HeaderMap::new()).expect("missing header ok"),
            Vec::<SecretBindingId>::new()
        );
    }

    #[test]
    fn local_thread_id_header_trims_and_discards_empty_values() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static(LOCAL_THREAD_ID_HEADER),
            HeaderValue::from_static(" thread-1 "),
        );
        assert_eq!(
            local_thread_id_from_headers(&headers),
            Some("thread-1".to_string())
        );

        headers.insert(
            HeaderName::from_static(LOCAL_THREAD_ID_HEADER),
            HeaderValue::from_static("   "),
        );
        assert_eq!(local_thread_id_from_headers(&headers), None);
    }

    #[test]
    fn merge_secret_binding_ids_preserves_first_seen_order_and_deduplicates() {
        let first = SecretBindingId::new();
        let second = SecretBindingId::new();
        let third = SecretBindingId::new();

        assert_eq!(
            merge_secret_binding_ids(&[first, second], &[second, third, first]),
            vec![first, second, third]
        );
    }
}
