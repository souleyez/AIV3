use crate::ApiError;
use domain_model::{DatasetLifecycle, DocumentLifecycle};

pub(crate) fn parse_dataset_lifecycle_update(
    lifecycle: Option<String>,
) -> std::result::Result<Option<DatasetLifecycle>, ApiError> {
    lifecycle
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            DatasetLifecycle::from_str(&normalized).ok_or_else(|| {
                ApiError::bad_request(
                    "validation_error",
                    format!("unsupported dataset lifecycle: {value}"),
                )
            })
        })
        .transpose()
}

pub(crate) fn parse_document_lifecycle_update(
    lifecycle: Option<String>,
) -> std::result::Result<Option<DocumentLifecycle>, ApiError> {
    lifecycle
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            DocumentLifecycle::from_str(&normalized).ok_or_else(|| {
                ApiError::bad_request(
                    "validation_error",
                    format!("unsupported document lifecycle: {value}"),
                )
            })
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    #[test]
    fn dataset_lifecycle_update_trims_and_lowercases_values() {
        assert_eq!(
            parse_dataset_lifecycle_update(None).expect("none lifecycle parses"),
            None
        );
        assert_eq!(
            parse_dataset_lifecycle_update(Some(" Archived ".to_string()))
                .expect("archived lifecycle parses"),
            Some(DatasetLifecycle::Archived)
        );
    }

    #[test]
    fn document_lifecycle_update_trims_and_lowercases_values() {
        assert_eq!(
            parse_document_lifecycle_update(None).expect("none lifecycle parses"),
            None
        );
        assert_eq!(
            parse_document_lifecycle_update(Some(" Indexed ".to_string()))
                .expect("indexed lifecycle parses"),
            Some(DocumentLifecycle::Indexed)
        );
    }

    #[test]
    fn lifecycle_update_rejects_unsupported_values_with_existing_errors() {
        let dataset_error = parse_dataset_lifecycle_update(Some("deleted".to_string()))
            .expect_err("unsupported dataset lifecycle should fail");
        assert_eq!(dataset_error.status, StatusCode::BAD_REQUEST);
        assert_eq!(dataset_error.payload.code, "validation_error");
        assert!(dataset_error
            .payload
            .message
            .contains("unsupported dataset lifecycle: deleted"));

        let document_error = parse_document_lifecycle_update(Some("deleted".to_string()))
            .expect_err("unsupported document lifecycle should fail");
        assert_eq!(document_error.status, StatusCode::BAD_REQUEST);
        assert_eq!(document_error.payload.code, "validation_error");
        assert!(document_error
            .payload
            .message
            .contains("unsupported document lifecycle: deleted"));
    }
}
