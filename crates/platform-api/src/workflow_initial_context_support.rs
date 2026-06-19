use serde_json::{Map, Value};
use workflow_engine::WorkflowRuntimeState;

pub(crate) fn workflow_initial_context_with_retries(
    runtime_state: &WorkflowRuntimeState,
) -> Map<String, Value> {
    let mut context = runtime_state.context.clone();
    context.insert(
        "retries_remaining".to_string(),
        Value::Number(runtime_state.retries_remaining.into()),
    );
    context
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{WorkflowExecutionId, WorkflowKind, WorkflowStatus};

    #[test]
    fn initial_context_preserves_existing_values_and_sets_retries_remaining() {
        let mut source_context = Map::new();
        source_context.insert(
            "dataset_id".to_string(),
            Value::String("dataset-1".to_string()),
        );
        let runtime_state = WorkflowRuntimeState {
            execution_id: WorkflowExecutionId::new(),
            kind: WorkflowKind::DatasetOutput,
            version: "dataset_output/v1".to_string(),
            stage: "queued".to_string(),
            status: WorkflowStatus::Pending,
            retries_remaining: 2,
            context: source_context,
            updated_at: Utc::now(),
        };

        let context = workflow_initial_context_with_retries(&runtime_state);

        assert_eq!(
            context["dataset_id"],
            Value::String("dataset-1".to_string())
        );
        assert_eq!(context["retries_remaining"], Value::Number(2.into()));
    }

    #[test]
    fn initial_context_overwrites_existing_retries_remaining() {
        let mut source_context = Map::new();
        source_context.insert("retries_remaining".to_string(), Value::Number(99.into()));
        let runtime_state = WorkflowRuntimeState {
            execution_id: WorkflowExecutionId::new(),
            kind: WorkflowKind::ReportPlan,
            version: "report_plan/v1".to_string(),
            stage: "queued".to_string(),
            status: WorkflowStatus::Pending,
            retries_remaining: 3,
            context: source_context,
            updated_at: Utc::now(),
        };

        let context = workflow_initial_context_with_retries(&runtime_state);

        assert_eq!(context["retries_remaining"], Value::Number(3.into()));
    }
}
