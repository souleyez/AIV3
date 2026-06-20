use crate::ApiError;

pub(crate) fn validate_required(
    field: &'static str,
    value: &str,
) -> std::result::Result<(), ApiError> {
    if value.trim().is_empty() {
        return Err(ApiError::bad_request(
            "validation_error",
            format!("{field} must not be empty"),
        ));
    }

    Ok(())
}

pub(crate) fn required_field(
    field: &'static str,
    value: Option<String>,
) -> std::result::Result<String, ApiError> {
    let value = value
        .ok_or_else(|| ApiError::bad_request("validation_error", format!("{field} is required")))?;
    validate_required(field, &value)?;
    Ok(value.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    #[test]
    fn required_field_trims_present_value() {
        assert_eq!(
            required_field("title", Some("  Report  ".to_string())).expect("valid field"),
            "Report"
        );
    }

    #[test]
    fn validate_required_accepts_trimmed_non_empty_value() {
        validate_required("title", "  Report  ").expect("non-empty value should pass");
    }

    #[test]
    fn validate_required_rejects_blank_value_with_stable_error() {
        let error = validate_required("title", "   ").expect_err("blank value should fail");

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.payload.code, "validation_error");
        assert_eq!(error.payload.message, "title must not be empty");
    }

    #[test]
    fn required_field_rejects_missing_value() {
        let error = required_field("title", None).expect_err("missing value should fail");

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.payload.code, "validation_error");
        assert!(error.payload.message.contains("title is required"));
    }

    #[test]
    fn required_field_rejects_blank_value() {
        let error =
            required_field("title", Some("   ".to_string())).expect_err("blank value should fail");

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.payload.code, "validation_error");
        assert!(error.payload.message.contains("title must not be empty"));
    }
}
