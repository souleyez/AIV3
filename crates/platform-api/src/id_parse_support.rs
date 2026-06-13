use domain_model::{
    AssistantRunId, ChatSessionId, DatasetId, DatasetOutputId, DocumentId, PublishedReportId,
    ReportPlanId, StaticPageDraftId, StaticPageImageJobId, StaticPageRenderOutputId,
    WorkflowExecutionId,
};
use uuid::Uuid;

use crate::ApiError;

pub(crate) fn parse_execution_id(raw: &str) -> std::result::Result<WorkflowExecutionId, ApiError> {
    Uuid::parse_str(raw).map(WorkflowExecutionId).map_err(|_| {
        ApiError::bad_request("invalid_execution_id", format!("{raw} is not a valid UUID"))
    })
}

pub(crate) fn parse_dataset_id(raw: &str) -> std::result::Result<DatasetId, ApiError> {
    Uuid::parse_str(raw).map(DatasetId).map_err(|_| {
        ApiError::bad_request("invalid_dataset_id", format!("{raw} is not a valid UUID"))
    })
}

pub(crate) fn parse_chat_session_id(raw: &str) -> std::result::Result<ChatSessionId, ApiError> {
    Uuid::parse_str(raw).map(ChatSessionId).map_err(|_| {
        ApiError::bad_request(
            "invalid_chat_session_id",
            format!("{raw} is not a valid UUID"),
        )
    })
}

pub(crate) fn parse_assistant_run_id(raw: &str) -> std::result::Result<AssistantRunId, ApiError> {
    Uuid::parse_str(raw).map(AssistantRunId).map_err(|_| {
        ApiError::bad_request(
            "invalid_assistant_run_id",
            format!("{raw} is not a valid UUID"),
        )
    })
}

pub(crate) fn parse_static_page_draft_id(
    raw: &str,
) -> std::result::Result<StaticPageDraftId, ApiError> {
    Uuid::parse_str(raw).map(StaticPageDraftId).map_err(|_| {
        ApiError::bad_request(
            "invalid_static_page_draft_id",
            format!("{raw} is not a valid UUID"),
        )
    })
}

pub(crate) fn parse_static_page_image_job_id(
    raw: &str,
) -> std::result::Result<StaticPageImageJobId, ApiError> {
    Uuid::parse_str(raw).map(StaticPageImageJobId).map_err(|_| {
        ApiError::bad_request(
            "invalid_static_page_image_job_id",
            format!("{raw} is not a valid UUID"),
        )
    })
}

pub(crate) fn parse_static_page_render_output_id(
    raw: &str,
) -> std::result::Result<StaticPageRenderOutputId, ApiError> {
    Uuid::parse_str(raw)
        .map(StaticPageRenderOutputId)
        .map_err(|_| {
            ApiError::bad_request(
                "invalid_static_page_render_output_id",
                format!("{raw} is not a valid UUID"),
            )
        })
}

pub(crate) fn parse_published_report_id(
    raw: &str,
) -> std::result::Result<PublishedReportId, ApiError> {
    Uuid::parse_str(raw).map(PublishedReportId).map_err(|_| {
        ApiError::bad_request(
            "invalid_published_report_id",
            format!("{raw} is not a valid UUID"),
        )
    })
}

pub(crate) fn parse_dataset_output_id(raw: &str) -> std::result::Result<DatasetOutputId, ApiError> {
    Uuid::parse_str(raw).map(DatasetOutputId).map_err(|_| {
        ApiError::bad_request(
            "invalid_dataset_output_id",
            format!("{raw} is not a valid UUID"),
        )
    })
}

pub(crate) fn parse_plan_id(raw: &str) -> std::result::Result<ReportPlanId, ApiError> {
    Uuid::parse_str(raw).map(ReportPlanId).map_err(|_| {
        ApiError::bad_request(
            "invalid_report_plan_id",
            format!("{raw} is not a valid UUID"),
        )
    })
}

pub(crate) fn parse_document_id(raw: &str) -> std::result::Result<DocumentId, ApiError> {
    Uuid::parse_str(raw).map(DocumentId).map_err(|_| {
        ApiError::bad_request("invalid_document_id", format!("{raw} is not a valid UUID"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    fn error_code<T>(result: std::result::Result<T, ApiError>) -> Option<String> {
        let error = result.err()?;
        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        Some(error.payload.code)
    }

    #[test]
    fn parse_ids_accept_valid_uuids_without_changing_type_wrappers() {
        let raw = Uuid::new_v4().to_string();

        assert_eq!(parse_execution_id(&raw).expect("valid").0.to_string(), raw);
        assert_eq!(parse_dataset_id(&raw).expect("valid").0.to_string(), raw);
        assert_eq!(
            parse_chat_session_id(&raw).expect("valid").0.to_string(),
            raw
        );
        assert_eq!(
            parse_assistant_run_id(&raw).expect("valid").0.to_string(),
            raw
        );
        assert_eq!(
            parse_static_page_draft_id(&raw)
                .expect("valid")
                .0
                .to_string(),
            raw
        );
        assert_eq!(
            parse_static_page_image_job_id(&raw)
                .expect("valid")
                .0
                .to_string(),
            raw
        );
        assert_eq!(
            parse_static_page_render_output_id(&raw)
                .expect("valid")
                .0
                .to_string(),
            raw
        );
        assert_eq!(
            parse_published_report_id(&raw)
                .expect("valid")
                .0
                .to_string(),
            raw
        );
        assert_eq!(
            parse_dataset_output_id(&raw).expect("valid").0.to_string(),
            raw
        );
        assert_eq!(parse_plan_id(&raw).expect("valid").0.to_string(), raw);
        assert_eq!(parse_document_id(&raw).expect("valid").0.to_string(), raw);
    }

    #[test]
    fn parse_ids_keep_existing_error_codes_for_invalid_uuids() {
        assert_eq!(
            error_code(parse_execution_id("not-a-uuid")).as_deref(),
            Some("invalid_execution_id")
        );
        assert_eq!(
            error_code(parse_dataset_id("not-a-uuid")).as_deref(),
            Some("invalid_dataset_id")
        );
        assert_eq!(
            error_code(parse_chat_session_id("not-a-uuid")).as_deref(),
            Some("invalid_chat_session_id")
        );
        assert_eq!(
            error_code(parse_assistant_run_id("not-a-uuid")).as_deref(),
            Some("invalid_assistant_run_id")
        );
        assert_eq!(
            error_code(parse_static_page_draft_id("not-a-uuid")).as_deref(),
            Some("invalid_static_page_draft_id")
        );
        assert_eq!(
            error_code(parse_static_page_image_job_id("not-a-uuid")).as_deref(),
            Some("invalid_static_page_image_job_id")
        );
        assert_eq!(
            error_code(parse_static_page_render_output_id("not-a-uuid")).as_deref(),
            Some("invalid_static_page_render_output_id")
        );
        assert_eq!(
            error_code(parse_published_report_id("not-a-uuid")).as_deref(),
            Some("invalid_published_report_id")
        );
        assert_eq!(
            error_code(parse_dataset_output_id("not-a-uuid")).as_deref(),
            Some("invalid_dataset_output_id")
        );
        assert_eq!(
            error_code(parse_plan_id("not-a-uuid")).as_deref(),
            Some("invalid_report_plan_id")
        );
        assert_eq!(
            error_code(parse_document_id("not-a-uuid")).as_deref(),
            Some("invalid_document_id")
        );
    }
}
