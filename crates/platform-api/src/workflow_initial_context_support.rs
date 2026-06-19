use chrono::{DateTime, Utc};
use domain_model::{
    DatasetId, MemoryDirectory, ReportPlanId, TenantId, WorkflowExecution, WorkflowExecutionId,
    WorkflowKind,
};
use serde_json::{json, Map, Value};
use workflow_engine::{WorkflowCatalog, WorkflowRuntimeState};

use crate::ApiError;

pub(crate) struct WorkflowInitialRuntimeParts {
    pub(crate) execution_id: WorkflowExecutionId,
    pub(crate) kind: WorkflowKind,
    pub(crate) now: DateTime<Utc>,
    pub(crate) runtime_state: WorkflowRuntimeState,
}

pub(crate) fn workflow_initial_runtime_parts(
    workflow_catalog: &WorkflowCatalog,
    kind: WorkflowKind,
    now: DateTime<Utc>,
) -> std::result::Result<WorkflowInitialRuntimeParts, ApiError> {
    let definition = workflow_catalog
        .find_definition(kind.clone())
        .ok_or_else(|| workflow_definition_missing_error(&kind))?;
    let execution_id = WorkflowExecutionId::new();
    let runtime_state = definition.initial_state(execution_id, now);
    Ok(WorkflowInitialRuntimeParts {
        execution_id,
        kind,
        now,
        runtime_state,
    })
}

pub(crate) fn workflow_initial_execution_from_parts(
    parts: WorkflowInitialRuntimeParts,
    tenant_id: TenantId,
    dataset_id: Option<DatasetId>,
    report_plan_id: Option<ReportPlanId>,
    context: Map<String, Value>,
) -> WorkflowExecution {
    WorkflowExecution {
        id: parts.execution_id,
        tenant_id,
        dataset_id,
        report_plan_id,
        kind: parts.kind,
        version: parts.runtime_state.version,
        stage: parts.runtime_state.stage,
        status: parts.runtime_state.status,
        attempt: 0,
        context: Value::Object(context),
        created_at: parts.now,
        updated_at: parts.now,
    }
}

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

pub(crate) fn insert_prompt_context(context: &mut Map<String, Value>, prompt: &str) {
    context.insert(
        "prompt".to_string(),
        Value::String(prompt.trim().to_string()),
    );
}

pub(crate) fn insert_optional_memory_directory_context(
    context: &mut Map<String, Value>,
    memory_directory: Option<&MemoryDirectory>,
) {
    if let Some(memory_directory) = memory_directory {
        context.insert(
            "memory_directory_id".to_string(),
            Value::String(memory_directory.id.to_string()),
        );
        context.insert(
            "memory_directory_version_no".to_string(),
            Value::Number(memory_directory.version_no.into()),
        );
    }
}

pub(crate) fn insert_report_default_context(
    context: &mut Map<String, Value>,
    report_time_range_contract: Value,
) {
    context.insert(
        "report_time_range_contract".to_string(),
        report_time_range_contract,
    );
    context.insert(
        "report_default_controls".to_string(),
        json!([
            "time_range",
            "primary_partition",
            "manual_refresh",
            "auto_refresh"
        ]),
    );
}

fn workflow_definition_missing_error(kind: &WorkflowKind) -> ApiError {
    let definition_key = kind.as_str().trim_end_matches("_workflow");
    ApiError::internal(
        "workflow_definition_missing",
        format!("{definition_key} workflow definition is not registered"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{MemoryDirectoryId, WorkflowExecutionId, WorkflowKind, WorkflowStatus};

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

    #[test]
    fn initial_runtime_parts_loads_definition_and_initial_state() {
        let now = Utc::now();
        let parts = workflow_initial_runtime_parts(
            &workflow_definitions::catalog(),
            WorkflowKind::ReportPlan,
            now,
        )
        .expect("report plan workflow should be registered");

        assert_eq!(parts.now, now);
        assert_eq!(parts.kind, WorkflowKind::ReportPlan);
        assert_eq!(parts.runtime_state.execution_id, parts.execution_id);
        assert_eq!(parts.runtime_state.kind, WorkflowKind::ReportPlan);
        assert_eq!(parts.runtime_state.updated_at, now);
    }

    #[test]
    fn workflow_definition_missing_error_uses_existing_message_shape() {
        let error = workflow_definition_missing_error(&WorkflowKind::ReportPlan);

        assert_eq!(error.payload.code, "workflow_definition_missing");
        assert_eq!(
            error.payload.message,
            "report_plan workflow definition is not registered"
        );
    }

    #[test]
    fn initial_execution_from_parts_preserves_scope_and_runtime_fields() {
        let now = Utc::now();
        let tenant_id = TenantId::new();
        let dataset_id = DatasetId::new();
        let report_plan_id = ReportPlanId::new();
        let parts = workflow_initial_runtime_parts(
            &workflow_definitions::catalog(),
            WorkflowKind::ReportRender,
            now,
        )
        .expect("report render workflow should be registered");
        let execution_id = parts.execution_id;
        let version = parts.runtime_state.version.clone();
        let stage = parts.runtime_state.stage.clone();
        let status = parts.runtime_state.status.clone();
        let mut context = Map::new();
        context.insert("surface".to_string(), Value::String("html".to_string()));

        let execution = workflow_initial_execution_from_parts(
            parts,
            tenant_id,
            Some(dataset_id),
            Some(report_plan_id),
            context,
        );

        assert_eq!(execution.id, execution_id);
        assert_eq!(execution.tenant_id, tenant_id);
        assert_eq!(execution.dataset_id, Some(dataset_id));
        assert_eq!(execution.report_plan_id, Some(report_plan_id));
        assert_eq!(execution.kind, WorkflowKind::ReportRender);
        assert_eq!(execution.version, version);
        assert_eq!(execution.stage, stage);
        assert_eq!(execution.status, status);
        assert_eq!(execution.attempt, 0);
        assert_eq!(execution.created_at, now);
        assert_eq!(execution.updated_at, now);
        assert_eq!(
            execution.context["surface"],
            Value::String("html".to_string())
        );
    }

    #[test]
    fn insert_prompt_context_trims_prompt_value() {
        let mut context = Map::new();

        insert_prompt_context(&mut context, "  build a report  ");

        assert_eq!(
            context["prompt"],
            Value::String("build a report".to_string())
        );
    }

    #[test]
    fn optional_memory_directory_context_preserves_id_and_version_only_when_present() {
        let memory_directory = MemoryDirectory {
            id: MemoryDirectoryId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            execution_id: WorkflowExecutionId::new(),
            owner_user_id: None,
            source_document_ids: Vec::new(),
            version_no: 7,
            directory_nodes: 0,
            refreshed_chunks: 0,
            directory_manifest: Value::Object(Map::new()),
            created_at: Utc::now(),
        };
        let mut context = Map::new();

        insert_optional_memory_directory_context(&mut context, Some(&memory_directory));

        assert_eq!(
            context["memory_directory_id"],
            Value::String(memory_directory.id.to_string())
        );
        assert_eq!(
            context["memory_directory_version_no"],
            Value::Number(7.into())
        );

        let mut empty_context = Map::new();
        insert_optional_memory_directory_context(&mut empty_context, None);

        assert!(empty_context.get("memory_directory_id").is_none());
        assert!(empty_context.get("memory_directory_version_no").is_none());
    }

    #[test]
    fn insert_report_default_context_preserves_contract_and_control_order() {
        let mut context = Map::new();
        let contract = json!({
            "default_preset": "this_month",
            "default_granularity": "day"
        });

        insert_report_default_context(&mut context, contract.clone());

        assert_eq!(context["report_time_range_contract"], contract);
        assert_eq!(
            context["report_default_controls"],
            json!([
                "time_range",
                "primary_partition",
                "manual_refresh",
                "auto_refresh"
            ])
        );
    }
}
