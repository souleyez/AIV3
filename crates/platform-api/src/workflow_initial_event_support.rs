use domain_model::{
    ChatSessionId, PublishedSurface, ReportPlanAstVersionId, ReportPlanId, StaticPageDraft,
    StaticPageImageJob, StaticPageRenderOutput, WorkflowEventId, WorkflowEventRecord,
    WorkflowExecution,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::ExternalSourceConnectionSummary;

pub(crate) fn build_initial_execution_event(
    execution: &WorkflowExecution,
    report_plan_id: ReportPlanId,
) -> WorkflowEventRecord {
    WorkflowEventRecord {
        id: WorkflowEventId::new(),
        execution_id: execution.id,
        sequence_no: 1,
        event_name: "workflow.execution_created".to_string(),
        payload: json!(initial_payload(
            execution,
            json!({
                "report_plan_id": report_plan_id,
            })
        )),
        created_at: execution.created_at,
    }
}

pub(crate) fn build_initial_external_source_sync_event(
    execution: &WorkflowExecution,
    source: &ExternalSourceConnectionSummary,
    sync_run_id: Uuid,
    sync_kind: &str,
) -> WorkflowEventRecord {
    WorkflowEventRecord {
        id: WorkflowEventId::new(),
        execution_id: execution.id,
        sequence_no: 1,
        event_name: "workflow.execution_created".to_string(),
        payload: json!(initial_payload(
            execution,
            json!({
                "source_id": source.source_id,
                "external_sync_run_id": sync_run_id,
                "sync_kind": sync_kind,
                "connector_kind": source.connector_kind,
                "sync_mode": source.sync_mode,
                "permission_mode": source.permission_mode,
            })
        )),
        created_at: execution.created_at,
    }
}

pub(crate) fn build_initial_external_action_dispatch_event(
    execution: &WorkflowExecution,
    connection_id: &str,
    action_id: &str,
) -> WorkflowEventRecord {
    WorkflowEventRecord {
        id: WorkflowEventId::new(),
        execution_id: execution.id,
        sequence_no: 1,
        event_name: "workflow.execution_created".to_string(),
        payload: json!(initial_payload(
            execution,
            json!({
                "channel_connection_id": connection_id,
                "external_action_id": action_id,
            })
        )),
        created_at: execution.created_at,
    }
}

pub(crate) fn build_initial_static_page_image_generation_event(
    execution: &WorkflowExecution,
    draft: &StaticPageDraft,
    job: &StaticPageImageJob,
) -> WorkflowEventRecord {
    WorkflowEventRecord {
        id: WorkflowEventId::new(),
        execution_id: execution.id,
        sequence_no: 1,
        event_name: "workflow.execution_created".to_string(),
        payload: json!(initial_payload(
            execution,
            json!({
                "assistant_run_id": draft.assistant_run_id,
                "static_page_draft_id": draft.id,
                "static_page_image_job_id": job.id,
            })
        )),
        created_at: execution.created_at,
    }
}

pub(crate) fn build_initial_static_page_render_event(
    execution: &WorkflowExecution,
    draft: &StaticPageDraft,
    render_output: &StaticPageRenderOutput,
) -> WorkflowEventRecord {
    WorkflowEventRecord {
        id: WorkflowEventId::new(),
        execution_id: execution.id,
        sequence_no: 1,
        event_name: "workflow.execution_created".to_string(),
        payload: json!(initial_payload(
            execution,
            json!({
                "assistant_run_id": draft.assistant_run_id,
                "static_page_draft_id": draft.id,
                "static_page_render_output_id": render_output.id,
                "static_page_image_job_id": render_output.image_job_id,
            })
        )),
        created_at: execution.created_at,
    }
}

pub(crate) fn build_initial_memory_directory_event(
    execution: &WorkflowExecution,
) -> WorkflowEventRecord {
    WorkflowEventRecord {
        id: WorkflowEventId::new(),
        execution_id: execution.id,
        sequence_no: 1,
        event_name: "workflow.execution_created".to_string(),
        payload: json!(initial_payload(
            execution,
            json!({
                "dataset_id": execution.dataset_id,
                "include_directory": execution
                    .context
                    .get("include_directory")
                    .and_then(Value::as_bool)
                    .unwrap_or(true),
            })
        )),
        created_at: execution.created_at,
    }
}

pub(crate) fn build_initial_dataset_output_event(
    execution: &WorkflowExecution,
    chat_session_id: Option<ChatSessionId>,
    prompt: &str,
) -> WorkflowEventRecord {
    WorkflowEventRecord {
        id: WorkflowEventId::new(),
        execution_id: execution.id,
        sequence_no: 1,
        event_name: "workflow.execution_created".to_string(),
        payload: json!(initial_payload(
            execution,
            json!({
                "dataset_id": execution.dataset_id,
                "chat_session_id": chat_session_id,
                "prompt": prompt.trim(),
            })
        )),
        created_at: execution.created_at,
    }
}

pub(crate) fn build_initial_chat_session_event(
    execution: &WorkflowExecution,
    chat_session_id: ChatSessionId,
    prompt: &str,
) -> WorkflowEventRecord {
    WorkflowEventRecord {
        id: WorkflowEventId::new(),
        execution_id: execution.id,
        sequence_no: 1,
        event_name: "workflow.execution_created".to_string(),
        payload: json!(initial_payload(
            execution,
            json!({
                "dataset_id": execution.dataset_id,
                "chat_session_id": chat_session_id,
                "prompt": prompt.trim(),
            })
        )),
        created_at: execution.created_at,
    }
}

pub(crate) fn build_initial_render_execution_event(
    execution: &WorkflowExecution,
    ast_version_id: ReportPlanAstVersionId,
    surface: &PublishedSurface,
) -> WorkflowEventRecord {
    WorkflowEventRecord {
        id: WorkflowEventId::new(),
        execution_id: execution.id,
        sequence_no: 1,
        event_name: "workflow.execution_created".to_string(),
        payload: json!(initial_payload(
            execution,
            json!({
                "report_plan_id": execution.report_plan_id,
                "report_plan_ast_version_id": ast_version_id,
                "surface": surface.as_str(),
            })
        )),
        created_at: execution.created_at,
    }
}

fn initial_payload(execution: &WorkflowExecution, extra: Value) -> Value {
    let mut payload = json!({
        "kind": execution.kind.as_str(),
        "version": execution.version,
        "status": execution.status.as_str(),
        "stage": execution.stage,
    });
    if let (Some(payload), Some(extra)) = (payload.as_object_mut(), extra.as_object()) {
        for (key, value) in extra {
            payload.insert(key.clone(), value.clone());
        }
    }
    payload
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{DatasetId, TenantId, WorkflowExecutionId, WorkflowKind, WorkflowStatus};

    fn workflow_execution(kind: WorkflowKind) -> WorkflowExecution {
        WorkflowExecution {
            id: WorkflowExecutionId(Uuid::from_u128(1)),
            tenant_id: TenantId(Uuid::from_u128(2)),
            dataset_id: Some(DatasetId(Uuid::from_u128(3))),
            report_plan_id: Some(ReportPlanId(Uuid::from_u128(4))),
            kind,
            version: "workflow/v1".to_string(),
            stage: "created".to_string(),
            status: WorkflowStatus::Pending,
            attempt: 0,
            context: json!({
                "include_directory": false
            }),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn initial_report_plan_event_preserves_base_payload_shape() {
        let execution = workflow_execution(WorkflowKind::ReportPlan);
        let event = build_initial_execution_event(
            &execution,
            execution
                .report_plan_id
                .expect("sample execution has report plan"),
        );

        assert_eq!(event.execution_id, execution.id);
        assert_eq!(event.sequence_no, 1);
        assert_eq!(event.event_name, "workflow.execution_created");
        assert_eq!(event.payload["kind"], json!(execution.kind.as_str()));
        assert_eq!(event.payload["version"], json!("workflow/v1"));
        assert_eq!(event.payload["status"], json!("pending"));
        assert_eq!(event.payload["stage"], json!("created"));
        assert_eq!(
            event.payload["report_plan_id"],
            json!(execution.report_plan_id)
        );
        assert_eq!(event.created_at, execution.created_at);
    }

    #[test]
    fn initial_memory_directory_event_preserves_include_directory_flag() {
        let execution = workflow_execution(WorkflowKind::MemoryDirectory);
        let event = build_initial_memory_directory_event(&execution);

        assert_eq!(event.payload["dataset_id"], json!(execution.dataset_id));
        assert_eq!(event.payload["include_directory"], json!(false));
    }

    #[test]
    fn initial_dataset_output_event_trims_prompt_and_keeps_optional_session() {
        let execution = workflow_execution(WorkflowKind::DatasetOutput);
        let chat_session_id = ChatSessionId(Uuid::from_u128(5));
        let event =
            build_initial_dataset_output_event(&execution, Some(chat_session_id), "  hello  ");

        assert_eq!(event.payload["dataset_id"], json!(execution.dataset_id));
        assert_eq!(event.payload["chat_session_id"], json!(chat_session_id));
        assert_eq!(event.payload["prompt"], json!("hello"));
    }
}
