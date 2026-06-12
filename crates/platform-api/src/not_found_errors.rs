use crate::ApiError;
use domain_model::{
    AssistantRunId, ChatSessionId, DatasetId, DatasetOutputId, DocumentId, ReportPlanId,
    StaticPageDraftId, StaticPageImageJobId, WorkflowExecutionId,
};

pub(crate) fn dataset_not_found_error(dataset_id: DatasetId) -> ApiError {
    ApiError::not_found(
        "dataset_not_found",
        format!(
            "dataset {} was not found for tenant or current access scope",
            dataset_id
        ),
    )
}

pub(crate) fn document_not_found_error(document_id: DocumentId) -> ApiError {
    ApiError::not_found(
        "document_not_found",
        format!("document {} was not found", document_id),
    )
}

pub(crate) fn dataset_output_not_found_error(output_id: DatasetOutputId) -> ApiError {
    ApiError::not_found(
        "dataset_output_not_found",
        format!("dataset output {} was not found", output_id),
    )
}

pub(crate) fn chat_session_not_found_error(session_id: ChatSessionId) -> ApiError {
    ApiError::not_found(
        "chat_session_not_found",
        format!("chat session {} was not found", session_id),
    )
}

pub(crate) fn assistant_run_not_found_error(run_id: AssistantRunId) -> ApiError {
    ApiError::not_found(
        "assistant_run_not_found",
        format!("assistant run {} was not found", run_id),
    )
}

pub(crate) fn workflow_execution_not_found_error(execution_id: WorkflowExecutionId) -> ApiError {
    ApiError::not_found(
        "workflow_execution_not_found",
        format!("workflow execution {} was not found", execution_id),
    )
}

pub(crate) fn report_plan_not_found_error(plan_id: ReportPlanId) -> ApiError {
    ApiError::not_found(
        "report_plan_not_found",
        format!("report plan {} was not found", plan_id),
    )
}

pub(crate) fn static_page_draft_not_found_error(draft_id: StaticPageDraftId) -> ApiError {
    ApiError::not_found(
        "static_page_draft_not_found",
        format!("static page draft {} was not found", draft_id),
    )
}

pub(crate) fn static_page_image_job_not_found_error(job_id: StaticPageImageJobId) -> ApiError {
    ApiError::not_found(
        "static_page_image_job_not_found",
        format!("static page image job {} was not found", job_id),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    fn assert_not_found(error: ApiError, code: &str, message_fragment: &str) {
        assert_eq!(error.status, StatusCode::NOT_FOUND);
        assert_eq!(error.payload.code, code);
        assert!(error.payload.message.contains(message_fragment));
    }

    #[test]
    fn not_found_helpers_preserve_existing_codes_and_messages() {
        let dataset_id = DatasetId::new();
        assert_not_found(
            dataset_not_found_error(dataset_id),
            "dataset_not_found",
            &format!("dataset {dataset_id} was not found for tenant or current access scope"),
        );

        let document_id = DocumentId::new();
        assert_not_found(
            document_not_found_error(document_id),
            "document_not_found",
            &format!("document {document_id} was not found"),
        );

        let output_id = DatasetOutputId::new();
        assert_not_found(
            dataset_output_not_found_error(output_id),
            "dataset_output_not_found",
            &format!("dataset output {output_id} was not found"),
        );

        let session_id = ChatSessionId::new();
        assert_not_found(
            chat_session_not_found_error(session_id),
            "chat_session_not_found",
            &format!("chat session {session_id} was not found"),
        );

        let run_id = AssistantRunId::new();
        assert_not_found(
            assistant_run_not_found_error(run_id),
            "assistant_run_not_found",
            &format!("assistant run {run_id} was not found"),
        );

        let execution_id = WorkflowExecutionId::new();
        assert_not_found(
            workflow_execution_not_found_error(execution_id),
            "workflow_execution_not_found",
            &format!("workflow execution {execution_id} was not found"),
        );

        let plan_id = ReportPlanId::new();
        assert_not_found(
            report_plan_not_found_error(plan_id),
            "report_plan_not_found",
            &format!("report plan {plan_id} was not found"),
        );

        let draft_id = StaticPageDraftId::new();
        assert_not_found(
            static_page_draft_not_found_error(draft_id),
            "static_page_draft_not_found",
            &format!("static page draft {draft_id} was not found"),
        );

        let job_id = StaticPageImageJobId::new();
        assert_not_found(
            static_page_image_job_not_found_error(job_id),
            "static_page_image_job_not_found",
            &format!("static page image job {job_id} was not found"),
        );
    }
}
