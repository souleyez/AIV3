use axum::http::StatusCode;
use domain_model::{
    AssistantRunId, ChatSessionId, SecretBindingId, StaticPageDraftId, UserId, WorkflowExecution,
    WorkflowExecutionId,
};

use crate::{
    load_visible_assistant_run_for_user, load_visible_chat_session_for_user,
    load_visible_dataset_for_user, load_visible_dataset_output_for_user,
    load_visible_report_plan_for_user, load_visible_static_page_draft,
    not_found_errors::workflow_execution_not_found_error,
    workflow_context_support::workflow_context_uuid, ApiError, AppState,
};

pub(crate) async fn ensure_workflow_execution_visible_for_user(
    state: &AppState,
    execution: &WorkflowExecution,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
) -> std::result::Result<(), ApiError> {
    if let Some(output) = state
        .storage
        .dataset_outputs()
        .get_by_execution_id(state.tenant_id, execution.id)
        .await
        .map_err(ApiError::from_storage)?
    {
        load_visible_dataset_output_for_user(
            state,
            output.id,
            active_secret_binding_ids,
            current_user_id,
        )
        .await?;
    }

    if let Some(session) = state
        .storage
        .chat_sessions()
        .get_by_execution_id(state.tenant_id, execution.id)
        .await
        .map_err(ApiError::from_storage)?
    {
        load_visible_chat_session_for_user(
            state,
            session.id,
            active_secret_binding_ids,
            current_user_id,
        )
        .await?;
    }

    if let Some(plan_id) = execution.report_plan_id {
        load_visible_report_plan_for_user(
            state,
            plan_id,
            active_secret_binding_ids,
            current_user_id,
        )
        .await?;
    }

    if let Some(raw_id) = workflow_context_uuid(&execution.context, "static_page_draft_id") {
        load_visible_static_page_draft(state, StaticPageDraftId(raw_id), current_user_id).await?;
    }

    if let Some(raw_id) = workflow_context_uuid(&execution.context, "assistant_run_id") {
        load_visible_assistant_run_for_user(state, AssistantRunId(raw_id), current_user_id).await?;
    }

    if let Some(raw_id) = workflow_context_uuid(&execution.context, "chat_session_id") {
        load_visible_chat_session_for_user(
            state,
            ChatSessionId(raw_id),
            active_secret_binding_ids,
            current_user_id,
        )
        .await?;
    }

    if let Some(dataset_id) = execution.dataset_id {
        load_visible_dataset_for_user(
            state,
            dataset_id,
            active_secret_binding_ids,
            current_user_id,
        )
        .await?;
    }

    Ok(())
}

pub(crate) async fn load_visible_workflow_execution_for_user(
    state: &AppState,
    execution_id: WorkflowExecutionId,
    active_secret_binding_ids: &[SecretBindingId],
    current_user_id: Option<UserId>,
) -> std::result::Result<WorkflowExecution, ApiError> {
    let execution = state
        .storage
        .workflow_executions()
        .get_by_id(state.tenant_id, execution_id)
        .await
        .map_err(ApiError::from_storage)?
        .ok_or_else(|| workflow_execution_not_found_error(execution_id))?;
    ensure_workflow_execution_visible_for_user(
        state,
        &execution,
        active_secret_binding_ids,
        current_user_id,
    )
    .await
    .map_err(|error| mask_visibility_error_for_workflow_execution(execution_id, error))?;
    Ok(execution)
}

fn mask_visibility_error_for_workflow_execution(
    execution_id: WorkflowExecutionId,
    error: ApiError,
) -> ApiError {
    if error.status == StatusCode::NOT_FOUND {
        workflow_execution_not_found_error(execution_id)
    } else {
        error
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_execution_visibility_masks_nested_not_found_only() {
        let execution_id = WorkflowExecutionId::new();
        let nested_not_found = ApiError::not_found("nested_not_found", "hidden".to_string());
        let masked = mask_visibility_error_for_workflow_execution(execution_id, nested_not_found);

        assert_eq!(masked.status, StatusCode::NOT_FOUND);
        assert_eq!(masked.payload.code, "workflow_execution_not_found");
        assert!(masked.payload.message.contains(&execution_id.to_string()));

        let bad_request = ApiError::bad_request("invalid_signal", "keep original".to_string());
        let unchanged = mask_visibility_error_for_workflow_execution(execution_id, bad_request);

        assert_eq!(unchanged.status, StatusCode::BAD_REQUEST);
        assert_eq!(unchanged.payload.code, "invalid_signal");
    }
}
