use chrono::Utc;
use domain_model::{
    Document, WorkflowEventId, WorkflowEventRecord, WorkflowExecution, WorkflowExecutionId,
    WorkflowKind,
};
use serde_json::{json, Value};
use workflow_engine::WorkflowDefinition;

use crate::{ApiError, AppState};

pub(crate) fn build_initial_upload_ingest_execution(
    state: &AppState,
    document: &Document,
) -> std::result::Result<WorkflowExecution, ApiError> {
    let definition = state
        .workflow_catalog
        .find_definition(WorkflowKind::UploadIngest)
        .ok_or_else(|| {
            ApiError::internal(
                "workflow_definition_missing",
                "upload_ingest workflow definition is not registered".to_string(),
            )
        })?;
    upload_ingest_execution_from_definition(state, document, definition.as_ref())
}

fn upload_ingest_execution_from_definition(
    state: &AppState,
    document: &Document,
    definition: &dyn WorkflowDefinition,
) -> std::result::Result<WorkflowExecution, ApiError> {
    let now = Utc::now();
    let execution_id = WorkflowExecutionId::new();
    let runtime_state = definition.initial_state(execution_id, now);
    let mut context = runtime_state.context;
    context.insert(
        "retries_remaining".to_string(),
        Value::Number(runtime_state.retries_remaining.into()),
    );
    context.insert(
        "document_id".to_string(),
        Value::String(document.id.to_string()),
    );
    context.insert(
        "content_type".to_string(),
        Value::String(document.content_type.clone()),
    );
    context.insert(
        "object_key".to_string(),
        Value::String(document.object_key.clone()),
    );

    Ok(WorkflowExecution {
        id: execution_id,
        tenant_id: state.tenant_id,
        dataset_id: Some(document.dataset_id),
        report_plan_id: None,
        kind: WorkflowKind::UploadIngest,
        version: runtime_state.version,
        stage: runtime_state.stage,
        status: runtime_state.status,
        attempt: 0,
        context: Value::Object(context),
        created_at: now,
        updated_at: now,
    })
}

pub(crate) fn build_initial_upload_ingest_event(
    execution: &WorkflowExecution,
    document: &Document,
) -> WorkflowEventRecord {
    WorkflowEventRecord {
        id: WorkflowEventId::new(),
        execution_id: execution.id,
        sequence_no: 1,
        event_name: "workflow.execution_created".to_string(),
        payload: json!({
            "kind": execution.kind.as_str(),
            "version": execution.version,
            "status": execution.status.as_str(),
            "stage": execution.stage,
            "document_id": document.id,
            "dataset_id": document.dataset_id,
            "content_type": document.content_type,
        }),
        created_at: execution.created_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{DatasetId, DocumentId, DocumentLifecycle, TenantId};
    use std::collections::BTreeMap;
    use uuid::Uuid;

    fn document() -> Document {
        Document {
            id: DocumentId(Uuid::from_u128(1)),
            tenant_id: TenantId(Uuid::from_u128(2)),
            dataset_id: DatasetId(Uuid::from_u128(3)),
            owner_user_id: None,
            title: "Upload".to_string(),
            object_key: "documents/upload.md".to_string(),
            content_type: "text/markdown".to_string(),
            lifecycle: DocumentLifecycle::Received,
            secret_binding_ids: Vec::new(),
            metadata: BTreeMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn workflow_execution(document: &Document) -> WorkflowExecution {
        WorkflowExecution {
            id: WorkflowExecutionId(Uuid::from_u128(4)),
            tenant_id: document.tenant_id,
            dataset_id: Some(document.dataset_id),
            report_plan_id: None,
            kind: WorkflowKind::UploadIngest,
            version: "1".to_string(),
            stage: "ingest".to_string(),
            status: domain_model::WorkflowStatus::Pending,
            attempt: 0,
            context: json!({
                "document_id": document.id.to_string(),
                "content_type": document.content_type,
                "object_key": document.object_key,
            }),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn upload_ingest_event_preserves_initial_payload_shape() {
        let document = document();
        let execution = workflow_execution(&document);
        let event = build_initial_upload_ingest_event(&execution, &document);

        assert_eq!(execution.dataset_id, Some(document.dataset_id));
        assert_eq!(execution.kind, WorkflowKind::UploadIngest);
        assert_eq!(
            execution.context["document_id"],
            json!(document.id.to_string())
        );
        assert_eq!(execution.context["content_type"], json!("text/markdown"));
        assert_eq!(
            execution.context["object_key"],
            json!("documents/upload.md")
        );
        assert_eq!(event.sequence_no, 1);
        assert_eq!(event.event_name, "workflow.execution_created");
        assert_eq!(event.payload["document_id"], json!(document.id));
        assert_eq!(event.payload["dataset_id"], json!(document.dataset_id));
        assert_eq!(event.payload["content_type"], json!("text/markdown"));
    }
}
