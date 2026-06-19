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
    workflow_execution_created_event(execution, report_plan_event_extra(report_plan_id))
}

pub(crate) fn build_initial_external_source_sync_event(
    execution: &WorkflowExecution,
    source: &ExternalSourceConnectionSummary,
    sync_run_id: Uuid,
    sync_kind: &str,
) -> WorkflowEventRecord {
    workflow_execution_created_event(
        execution,
        external_source_sync_event_extra(source, sync_run_id, sync_kind),
    )
}

pub(crate) fn build_initial_external_action_dispatch_event(
    execution: &WorkflowExecution,
    connection_id: &str,
    action_id: &str,
) -> WorkflowEventRecord {
    workflow_execution_created_event(
        execution,
        external_action_dispatch_event_extra(connection_id, action_id),
    )
}

pub(crate) fn build_initial_static_page_image_generation_event(
    execution: &WorkflowExecution,
    draft: &StaticPageDraft,
    job: &StaticPageImageJob,
) -> WorkflowEventRecord {
    workflow_execution_created_event(
        execution,
        static_page_image_generation_event_extra(draft, job),
    )
}

pub(crate) fn build_initial_static_page_render_event(
    execution: &WorkflowExecution,
    draft: &StaticPageDraft,
    render_output: &StaticPageRenderOutput,
) -> WorkflowEventRecord {
    workflow_execution_created_event(
        execution,
        static_page_render_output_event_extra(draft, render_output),
    )
}

pub(crate) fn build_initial_memory_directory_event(
    execution: &WorkflowExecution,
) -> WorkflowEventRecord {
    workflow_execution_created_event(execution, memory_directory_event_extra(execution))
}

pub(crate) fn build_initial_dataset_output_event(
    execution: &WorkflowExecution,
    chat_session_id: Option<ChatSessionId>,
    prompt: &str,
) -> WorkflowEventRecord {
    workflow_execution_created_event(
        execution,
        dataset_prompt_event_extra(execution, chat_session_id, prompt),
    )
}

pub(crate) fn build_initial_chat_session_event(
    execution: &WorkflowExecution,
    chat_session_id: ChatSessionId,
    prompt: &str,
) -> WorkflowEventRecord {
    workflow_execution_created_event(
        execution,
        dataset_prompt_event_extra(execution, Some(chat_session_id), prompt),
    )
}

pub(crate) fn build_initial_render_execution_event(
    execution: &WorkflowExecution,
    ast_version_id: ReportPlanAstVersionId,
    surface: &PublishedSurface,
) -> WorkflowEventRecord {
    workflow_execution_created_event(
        execution,
        report_render_event_extra(execution, ast_version_id, surface),
    )
}

fn workflow_execution_created_event(
    execution: &WorkflowExecution,
    extra: Value,
) -> WorkflowEventRecord {
    WorkflowEventRecord {
        id: WorkflowEventId::new(),
        execution_id: execution.id,
        sequence_no: 1,
        event_name: "workflow.execution_created".to_string(),
        payload: json!(initial_payload(execution, extra)),
        created_at: execution.created_at,
    }
}

fn dataset_prompt_event_extra(
    execution: &WorkflowExecution,
    chat_session_id: Option<ChatSessionId>,
    prompt: &str,
) -> Value {
    json!({
        "dataset_id": execution.dataset_id,
        "chat_session_id": chat_session_id,
        "prompt": prompt.trim(),
    })
}

fn memory_directory_event_extra(execution: &WorkflowExecution) -> Value {
    json!({
        "dataset_id": execution.dataset_id,
        "include_directory": execution
            .context
            .get("include_directory")
            .and_then(Value::as_bool)
            .unwrap_or(true),
    })
}

fn static_page_draft_event_extra(draft: &StaticPageDraft, extra: Value) -> Value {
    let mut payload = json!({
        "assistant_run_id": draft.assistant_run_id,
        "static_page_draft_id": draft.id,
    });
    merge_object_fields(&mut payload, &extra);
    payload
}

fn static_page_image_generation_event_extra(
    draft: &StaticPageDraft,
    job: &StaticPageImageJob,
) -> Value {
    static_page_draft_event_extra(
        draft,
        json!({
            "static_page_image_job_id": job.id,
        }),
    )
}

fn static_page_render_output_event_extra(
    draft: &StaticPageDraft,
    render_output: &StaticPageRenderOutput,
) -> Value {
    static_page_draft_event_extra(
        draft,
        json!({
            "static_page_render_output_id": render_output.id,
            "static_page_image_job_id": render_output.image_job_id,
        }),
    )
}

fn report_plan_event_extra(report_plan_id: ReportPlanId) -> Value {
    json!({
        "report_plan_id": report_plan_id,
    })
}

fn report_render_event_extra(
    execution: &WorkflowExecution,
    ast_version_id: ReportPlanAstVersionId,
    surface: &PublishedSurface,
) -> Value {
    json!({
        "report_plan_id": execution.report_plan_id,
        "report_plan_ast_version_id": ast_version_id,
        "surface": surface.as_str(),
    })
}

fn external_source_sync_event_extra(
    source: &ExternalSourceConnectionSummary,
    sync_run_id: Uuid,
    sync_kind: &str,
) -> Value {
    json!({
        "source_id": source.source_id,
        "external_sync_run_id": sync_run_id,
        "sync_kind": sync_kind,
        "connector_kind": source.connector_kind,
        "sync_mode": source.sync_mode,
        "permission_mode": source.permission_mode,
    })
}

fn external_action_dispatch_event_extra(connection_id: &str, action_id: &str) -> Value {
    json!({
        "channel_connection_id": connection_id,
        "external_action_id": action_id,
    })
}

fn initial_payload(execution: &WorkflowExecution, extra: Value) -> Value {
    let mut payload = workflow_execution_base_payload(execution);
    merge_object_fields(&mut payload, &extra);
    payload
}

fn workflow_execution_base_payload(execution: &WorkflowExecution) -> Value {
    json!({
        "kind": execution.kind.as_str(),
        "version": execution.version,
        "status": execution.status.as_str(),
        "stage": execution.stage,
    })
}

fn merge_object_fields(target: &mut Value, extra: &Value) {
    if let (Some(target), Some(extra)) = (target.as_object_mut(), extra.as_object()) {
        for (key, value) in extra {
            target.insert(key.clone(), value.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{
        AssistantRunId, DatasetId, StaticPageDraftId, StaticPageDraftStatus, StaticPageImageJobId,
        StaticPageImageJobStatus, StaticPageRenderOutputId, StaticPageRenderOutputStatus, TenantId,
        WorkflowExecutionId, WorkflowKind, WorkflowStatus,
    };

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

    fn static_page_draft() -> StaticPageDraft {
        let now = Utc::now();
        StaticPageDraft {
            id: StaticPageDraftId(Uuid::from_u128(6)),
            tenant_id: TenantId(Uuid::from_u128(2)),
            assistant_run_id: AssistantRunId(Uuid::from_u128(7)),
            owner_user_id: None,
            title: "Static Page".to_string(),
            status: StaticPageDraftStatus::Queued,
            selected_scope: json!({}),
            visibility_snapshot: json!({}),
            source_refs: json!({}),
            draft_payload: json!({}),
            created_at: now,
            updated_at: now,
        }
    }

    fn static_page_image_job(draft: &StaticPageDraft) -> StaticPageImageJob {
        let now = Utc::now();
        StaticPageImageJob {
            id: StaticPageImageJobId(Uuid::from_u128(10)),
            tenant_id: draft.tenant_id,
            draft_id: draft.id,
            assistant_run_id: draft.assistant_run_id,
            status: StaticPageImageJobStatus::Queued,
            queue_position: Some(1),
            image_prompt_payload: json!({}),
            preview_asset_key: None,
            failure_reason: None,
            confirmed_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn static_page_render_output(draft: &StaticPageDraft) -> StaticPageRenderOutput {
        StaticPageRenderOutput {
            id: StaticPageRenderOutputId(Uuid::from_u128(11)),
            tenant_id: draft.tenant_id,
            draft_id: draft.id,
            assistant_run_id: draft.assistant_run_id,
            owner_user_id: None,
            image_job_id: Some(StaticPageImageJobId(Uuid::from_u128(10))),
            status: StaticPageRenderOutputStatus::Queued,
            html: String::new(),
            asset_manifest: json!({}),
            created_at: Utc::now(),
        }
    }

    fn external_source_summary() -> ExternalSourceConnectionSummary {
        ExternalSourceConnectionSummary {
            source_id: "source-1".to_string(),
            connector_kind: "mysql".to_string(),
            display_name: "Traffic Area".to_string(),
            sync_mode: "snapshot".to_string(),
            permission_mode: "tenant_private".to_string(),
            health_status: "healthy".to_string(),
            config_redacted: json!({}),
            disabled_at: None,
        }
    }

    #[test]
    fn workflow_execution_created_event_preserves_common_envelope_and_merges_extra_payload() {
        let execution = workflow_execution(WorkflowKind::ExternalActionDispatch);

        let event = workflow_execution_created_event(
            &execution,
            json!({
                "external_action_id": "action-1",
            }),
        );

        assert_eq!(event.execution_id, execution.id);
        assert_eq!(event.sequence_no, 1);
        assert_eq!(event.event_name, "workflow.execution_created");
        assert_eq!(event.created_at, execution.created_at);
        assert_eq!(event.payload["kind"], json!(execution.kind.as_str()));
        assert_eq!(event.payload["version"], json!("workflow/v1"));
        assert_eq!(event.payload["status"], json!("pending"));
        assert_eq!(event.payload["stage"], json!("created"));
        assert_eq!(event.payload["external_action_id"], json!("action-1"));
    }

    #[test]
    fn workflow_execution_base_payload_preserves_common_workflow_fields() {
        let execution = workflow_execution(WorkflowKind::ReportPlan);

        let payload = workflow_execution_base_payload(&execution);

        assert_eq!(payload["kind"], json!(execution.kind.as_str()));
        assert_eq!(payload["version"], json!("workflow/v1"));
        assert_eq!(payload["status"], json!("pending"));
        assert_eq!(payload["stage"], json!("created"));
    }

    #[test]
    fn dataset_prompt_event_extra_preserves_dataset_optional_session_and_trimmed_prompt() {
        let execution = workflow_execution(WorkflowKind::DatasetOutput);

        let extra = dataset_prompt_event_extra(&execution, None, "  hello  ");

        assert_eq!(extra["dataset_id"], json!(execution.dataset_id));
        assert_eq!(extra["chat_session_id"], Value::Null);
        assert_eq!(extra["prompt"], json!("hello"));
    }

    #[test]
    fn memory_directory_event_extra_preserves_dataset_and_include_directory_default() {
        let execution = workflow_execution(WorkflowKind::MemoryDirectory);

        let extra = memory_directory_event_extra(&execution);

        assert_eq!(extra["dataset_id"], json!(execution.dataset_id));
        assert_eq!(extra["include_directory"], json!(false));

        let mut default_execution = workflow_execution(WorkflowKind::MemoryDirectory);
        default_execution.context = json!({});

        let default_extra = memory_directory_event_extra(&default_execution);

        assert_eq!(
            default_extra["dataset_id"],
            json!(default_execution.dataset_id)
        );
        assert_eq!(default_extra["include_directory"], json!(true));
    }

    #[test]
    fn static_page_draft_event_extra_preserves_common_fields_and_merges_extra_payload() {
        let draft = static_page_draft();

        let extra = static_page_draft_event_extra(
            &draft,
            json!({
                "static_page_image_job_id": "job-1",
            }),
        );

        assert_eq!(extra["assistant_run_id"], json!(draft.assistant_run_id));
        assert_eq!(extra["static_page_draft_id"], json!(draft.id));
        assert_eq!(extra["static_page_image_job_id"], json!("job-1"));
    }

    #[test]
    fn static_page_image_generation_event_extra_preserves_draft_and_job_ids() {
        let draft = static_page_draft();
        let job = static_page_image_job(&draft);

        let extra = static_page_image_generation_event_extra(&draft, &job);

        assert_eq!(extra["assistant_run_id"], json!(draft.assistant_run_id));
        assert_eq!(extra["static_page_draft_id"], json!(draft.id));
        assert_eq!(extra["static_page_image_job_id"], json!(job.id));
    }

    #[test]
    fn static_page_render_output_event_extra_preserves_render_and_image_job_ids() {
        let draft = static_page_draft();
        let render_output = static_page_render_output(&draft);

        let extra = static_page_render_output_event_extra(&draft, &render_output);

        assert_eq!(extra["assistant_run_id"], json!(draft.assistant_run_id));
        assert_eq!(extra["static_page_draft_id"], json!(draft.id));
        assert_eq!(
            extra["static_page_render_output_id"],
            json!(render_output.id)
        );
        assert_eq!(
            extra["static_page_image_job_id"],
            json!(render_output.image_job_id)
        );
    }

    #[test]
    fn merge_object_fields_merges_objects_and_ignores_non_object_extra() {
        let mut target = json!({
            "kind": "workflow",
        });

        merge_object_fields(
            &mut target,
            &json!({
                "stage": "created",
            }),
        );

        assert_eq!(target["kind"], json!("workflow"));
        assert_eq!(target["stage"], json!("created"));

        merge_object_fields(&mut target, &json!("ignored"));

        assert_eq!(target["kind"], json!("workflow"));
        assert_eq!(target["stage"], json!("created"));
    }

    #[test]
    fn report_render_event_extra_preserves_report_plan_ast_version_and_surface() {
        let execution = workflow_execution(WorkflowKind::ReportRender);
        let ast_version_id = ReportPlanAstVersionId(Uuid::from_u128(8));
        let surface = PublishedSurface::Pc;

        let extra = report_render_event_extra(&execution, ast_version_id, &surface);

        assert_eq!(extra["report_plan_id"], json!(execution.report_plan_id));
        assert_eq!(extra["report_plan_ast_version_id"], json!(ast_version_id));
        assert_eq!(extra["surface"], json!("pc"));
    }

    #[test]
    fn external_source_sync_event_extra_preserves_sync_fields() {
        let source = external_source_summary();
        let sync_run_id = Uuid::from_u128(9);

        let extra = external_source_sync_event_extra(&source, sync_run_id, "manual");

        assert_eq!(extra["source_id"], json!("source-1"));
        assert_eq!(extra["external_sync_run_id"], json!(sync_run_id));
        assert_eq!(extra["sync_kind"], json!("manual"));
        assert_eq!(extra["connector_kind"], json!("mysql"));
        assert_eq!(extra["sync_mode"], json!("snapshot"));
        assert_eq!(extra["permission_mode"], json!("tenant_private"));
    }

    #[test]
    fn external_action_dispatch_event_extra_preserves_connection_and_action_ids() {
        let extra = external_action_dispatch_event_extra("channel-main", "action-1");

        assert_eq!(extra["channel_connection_id"], json!("channel-main"));
        assert_eq!(extra["external_action_id"], json!("action-1"));
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
