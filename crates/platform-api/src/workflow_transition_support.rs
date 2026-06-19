use contracts::WorkflowSignalRequest;
use domain_model::{WorkflowExecution, WorkflowStatus};
use serde_json::{Map, Value};
use workflow_engine::{WorkflowRuntimeState, WorkflowSignal};

use crate::{required_field, ApiError};

pub(crate) fn build_workflow_signal(
    request: WorkflowSignalRequest,
) -> std::result::Result<WorkflowSignal, ApiError> {
    match request.kind {
        contracts::WorkflowSignalKindView::Start => Ok(WorkflowSignal::Start),
        contracts::WorkflowSignalKindView::StepCompleted => Ok(WorkflowSignal::StepCompleted {
            task_key: required_field("task_key", request.task_key)?,
            output: request.output,
        }),
        contracts::WorkflowSignalKindView::StepFailed => Ok(WorkflowSignal::StepFailed {
            task_key: required_field("task_key", request.task_key)?,
            error: required_field("error", request.error)?,
        }),
        contracts::WorkflowSignalKindView::RetryRequested => Ok(WorkflowSignal::RetryRequested {
            reason: required_field("reason", request.reason)?,
        }),
        contracts::WorkflowSignalKindView::CancelRequested => Ok(WorkflowSignal::CancelRequested {
            reason: required_field("reason", request.reason)?,
        }),
        contracts::WorkflowSignalKindView::PublishRequested => {
            Ok(WorkflowSignal::PublishRequested { note: request.note })
        }
        contracts::WorkflowSignalKindView::Other(other) => Err(ApiError::bad_request(
            "invalid_signal_kind",
            format!("{} is not a supported workflow signal", other.trim()),
        )),
    }
}

pub(crate) fn workflow_runtime_state_from_execution(
    execution: &WorkflowExecution,
) -> std::result::Result<WorkflowRuntimeState, ApiError> {
    let mut context = extract_context_object(&execution.context)?;
    let retries_remaining = context
        .remove("retries_remaining")
        .and_then(|value| value.as_u64())
        .unwrap_or(3) as u32;

    Ok(WorkflowRuntimeState {
        execution_id: execution.id,
        kind: execution.kind.clone(),
        version: execution.version.clone(),
        stage: execution.stage.clone(),
        status: execution.status.clone(),
        retries_remaining,
        context,
        updated_at: execution.updated_at,
    })
}

pub(crate) fn workflow_execution_from_transition(
    previous: &WorkflowExecution,
    next_state: WorkflowRuntimeState,
) -> std::result::Result<WorkflowExecution, ApiError> {
    let next_attempt = next_attempt(&previous.status, previous.attempt, &next_state.status);
    let WorkflowRuntimeState {
        kind,
        version,
        stage,
        status,
        retries_remaining,
        mut context,
        updated_at,
        ..
    } = next_state;

    context.insert(
        "retries_remaining".to_string(),
        Value::Number(retries_remaining.into()),
    );

    Ok(WorkflowExecution {
        id: previous.id,
        tenant_id: previous.tenant_id,
        dataset_id: previous.dataset_id,
        report_plan_id: previous.report_plan_id,
        kind,
        version,
        stage,
        status,
        attempt: next_attempt,
        context: Value::Object(context),
        created_at: previous.created_at,
        updated_at,
    })
}

fn next_attempt(
    previous_status: &WorkflowStatus,
    previous_attempt: u32,
    next_status: &WorkflowStatus,
) -> u32 {
    if *previous_status == WorkflowStatus::Pending && *next_status == WorkflowStatus::Running {
        previous_attempt + 1
    } else {
        previous_attempt
    }
}

fn extract_context_object(value: &Value) -> std::result::Result<Map<String, Value>, ApiError> {
    match value {
        Value::Object(map) => Ok(map.clone()),
        Value::Null => Ok(Map::new()),
        _ => Err(ApiError::internal(
            "invalid_execution_context",
            "workflow execution context must be a JSON object".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use chrono::Utc;
    use domain_model::{
        DatasetId, ReportPlanId, TenantId, WorkflowExecutionId, WorkflowKind, WorkflowStatus,
    };
    use serde_json::json;

    #[test]
    fn build_workflow_signal_parses_step_completed_request() {
        let signal = build_workflow_signal(WorkflowSignalRequest {
            kind: contracts::WorkflowSignalKindView::StepCompleted,
            task_key: Some("plan_report_ast".to_string()),
            output: Some(json!({ "status": "ok" })),
            error: None,
            reason: None,
            note: None,
        })
        .expect("step_completed request should parse");

        assert!(matches!(
            signal,
            WorkflowSignal::StepCompleted {
                ref task_key,
                output: Some(_)
            } if task_key == "plan_report_ast"
        ));
    }

    #[test]
    fn build_workflow_signal_rejects_missing_required_field() {
        let error = build_workflow_signal(WorkflowSignalRequest {
            kind: contracts::WorkflowSignalKindView::StepFailed,
            task_key: Some("plan_report_ast".to_string()),
            output: None,
            error: None,
            reason: None,
            note: None,
        })
        .expect_err("step_failed without error should be rejected");

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.payload.code, "validation_error");
        assert!(error.payload.message.contains("error is required"));
    }

    #[test]
    fn build_workflow_signal_rejects_unknown_kind() {
        let error = build_workflow_signal(WorkflowSignalRequest {
            kind: contracts::WorkflowSignalKindView::Other("custom_signal".to_string()),
            task_key: None,
            output: None,
            error: None,
            reason: None,
            note: None,
        })
        .expect_err("unknown kinds should be rejected");

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.payload.code, "invalid_signal_kind");
        assert!(error.payload.message.contains("custom_signal"));
    }

    #[test]
    fn workflow_execution_from_transition_preserves_context_and_bumps_attempt() {
        let now = Utc::now();
        let previous = WorkflowExecution {
            id: WorkflowExecutionId::new(),
            tenant_id: TenantId::new(),
            dataset_id: Some(DatasetId::new()),
            report_plan_id: Some(ReportPlanId::new()),
            kind: WorkflowKind::ReportPlan,
            version: "report_plan/v1".to_string(),
            stage: "queued".to_string(),
            status: WorkflowStatus::Pending,
            attempt: 0,
            context: json!({
                "report_plan_id": "rp_123",
                "retries_remaining": 3
            }),
            created_at: now,
            updated_at: now,
        };
        let next_state = WorkflowRuntimeState {
            execution_id: previous.id,
            kind: WorkflowKind::ReportPlan,
            version: "report_plan/v1".to_string(),
            stage: "plan_report_ast".to_string(),
            status: WorkflowStatus::Running,
            retries_remaining: 2,
            context: Map::from_iter([(
                "report_plan_id".to_string(),
                Value::String("rp_123".to_string()),
            )]),
            updated_at: now,
        };

        let execution =
            workflow_execution_from_transition(&previous, next_state).expect("transition applies");

        assert_eq!(execution.attempt, 1);
        assert_eq!(execution.stage, "plan_report_ast");
        assert_eq!(execution.status, WorkflowStatus::Running);
        assert_eq!(execution.dataset_id, previous.dataset_id);
        assert_eq!(execution.report_plan_id, previous.report_plan_id);
        assert_eq!(execution.context["report_plan_id"], json!("rp_123"));
        assert_eq!(execution.context["retries_remaining"], json!(2));
    }
}
