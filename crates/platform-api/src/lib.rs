use assistant_runtime::{candidates_to_values, plan_scope, ScopePlannerInput};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use contracts::{
    AdvanceWorkflowExecutionResponse, ApiErrorResponse, AppendAssistantRunEventRequest,
    AppendAssistantRunEventResponse, AppendChatSessionTurnRequest, AppendChatSessionTurnResponse,
    AppendStaticPageDraftOperationsRequest, AppendStaticPageDraftOperationsResponse,
    ApplyStaticPageDraftIntentRequest, ApplyStaticPageDraftIntentResponse, AssistantRunDetailView,
    AssistantRunEventView, AssistantRunMessageView, AssistantRunView, ChatMessageView,
    ChatSessionView, CompareDocumentsRequest, CompareDocumentsView,
    ConfirmStaticPageImageJobRequest, ConfirmStaticPageImageJobResponse,
    ContinueAssistantRunRequest, ContinueAssistantRunResponse, ConversationMemoryItemView,
    CreateAssistantRunRequest, CreateAssistantRunResponse, CreateChatSessionRequest,
    CreateChatSessionResponse, CreateConversationMemoryItemRequest, CreateDatasetOutputRequest,
    CreateDatasetOutputResponse, CreateDatasetRequest, CreateDatasetSecretBindingRequest,
    CreateDatasetSecretBindingResponse, CreateDocumentIngestResponse,
    CreateMemoryDirectoryRefreshResponse, CreateReportPlanResponse, CreateReportRenderRequest,
    CreateReportRenderResponse, CreateStaticPageDraftRequest, CreateStaticPageDraftResponse,
    CreateStaticPageImageJobRequest, CreateStaticPageImageJobResponse,
    CreateStaticPageRenderRequest, CreateStaticPageRenderResponse, DatasetOutputView,
    DatasetSummary, DocumentChunkView, DocumentDetailView, DocumentSummary, HealthResponse,
    LlmInvocationView, MemoryDirectoryView, PlanReportRequest, PublishReportRequest,
    PublishReportResponse, PublishedReportDetailView, PublishedReportVersionView,
    PublishedReportView, RegisterDocumentRequest, RegisterDocumentResponse,
    ReportPlanAstVersionView, ReportPlanSummary, ReportRenderOutputView,
    ResolveDatasetSecretBindingsRequest, ResolveDatasetSecretBindingsResponse,
    RetrievalEvidenceView, RetrievalSearchHitView, RetrievalSearchResponse,
    RetryWorkflowExecutionRequest, RetryWorkflowExecutionResponse, StaticPageDraftView,
    StaticPageImageJobView, StaticPageRenderOutputView, ToolDefinitionView, ToolExecutionView,
    UpdateChatSessionReportEntryRequest, UpdateChatSessionReportEntryResponse,
    UpdateStaticPageDraftRequest, UpdateStaticPageDraftResponse, WorkflowDefinitionView,
    WorkflowEventView, WorkflowExecutionView, WorkflowRuntimeInspectView, WorkflowSignalRequest,
    WorkflowTaskView,
};
use domain_model::{
    AssistantRun, AssistantRunEvent, AssistantRunId, ChatMessage, ChatMessageId, ChatMessageRole,
    ChatSession, ChatSessionId, ConversationMemoryItem, Dataset, DatasetId, DatasetOutput,
    DatasetOutputId, DatasetVisibility, Document, DocumentChunk, DocumentChunkId, DocumentId,
    LlmInvocation, LlmInvocationFinishReason, LlmInvocationMode, LlmInvocationSourceKind,
    MemoryDirectory, MemoryDirectoryId, PublishedReport, PublishedReportId, PublishedReportVersion,
    PublishedSurface, ReportPlan, ReportPlanAstVersion, ReportPlanId, ReportRenderOutput,
    RetrievalEvidence, RetrievalEvidenceId, SecretBindingId, SecretScopeLevel, StaticPageDraft,
    StaticPageDraftId, StaticPageDraftStatus, StaticPageImageJob, StaticPageImageJobId,
    StaticPageImageJobStatus, StaticPageRenderOutput, StaticPageRenderOutputStatus, TenantId,
    ToolExecution, ToolExecutionSourceKind, ToolExecutionStatus, WorkflowEventRecord,
    WorkflowExecution, WorkflowExecutionId, WorkflowKind, WorkflowStatus, WorkflowTask,
};
use event_bus::{
    workflow_execution_transition_subject, workflow_task_enqueued_subject, EventBus, EventEnvelope,
};
use llm_gateway::{build_provider_from_env, render_runtime_manifest, LlmRequest};
use prompt_registry::bootstrap_default_prompt_registry;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use static_page_renderer::{render_static_page, StaticPageRenderRequest};
use static_page_runtime::{
    interpret_static_page_intent_deterministic, interpret_static_page_intent_with_provider,
    sanitize_static_page_operations, StaticPageIntentOutcome, StaticPageIntentRequest,
};
use std::{
    collections::{BTreeMap, HashSet},
    fmt::Display,
};
use storage::{
    NewAssistantRun, NewAssistantRunEvent, NewChatMessage, NewChatSession,
    NewConversationMemoryItem, NewDataset, NewDocument, NewPublishedReport,
    NewPublishedReportVersion, NewReportPlan, NewSecretBinding, NewStaticPageDraft,
    NewStaticPageImageJob, NewStaticPageRenderOutput, NewWorkflowTask, PgStorage,
};
use tool_registry::{
    bootstrap_default_tool_registry, ToolCliOutputMode, ToolDefinition, ToolInvocationMode,
};
use uuid::Uuid;
use workflow_engine::{WorkflowCatalog, WorkflowRuntimeState, WorkflowSignal};

const DATASET_OUTPUT_RETRIEVAL_SCAN_LIMIT: i64 = 512;
const DATASET_OUTPUT_RETRIEVAL_BIND_LIMIT: usize = 8;
const RETRIEVAL_SEARCH_DEFAULT_LIMIT: usize = 8;
const RETRIEVAL_SEARCH_MAX_LIMIT: usize = 20;
const DEFAULT_ASSISTANT_RUN_RUNTIME_MODEL: &str = "placeholder-assistant-run-v1";
const DEFAULT_STATIC_PAGE_INTENT_RUNTIME_MODEL: &str = "static-page-intent-v1";
const ASSISTANT_RUN_EVIDENCE_DEFAULT_LIMIT: usize = 4;
const ASSISTANT_RUN_EVIDENCE_MAX_LIMIT: usize = 8;
const ASSISTANT_RUN_EVIDENCE_DATASET_LIMIT: usize = 2;
const ASSISTANT_RUN_CONVERSATION_MEMORY_DEFAULT_LIMIT: i64 = 4;
const ASSISTANT_RUN_CONVERSATION_MEMORY_MAX_LIMIT: i64 = 8;
const ASSISTANT_RUN_CONTINUE_DEFAULT_MAX_STEPS: usize = 3;
const ASSISTANT_RUN_CONTINUE_MAX_STEPS: usize = 5;
const STATIC_PAGE_DRAFT_LIST_DEFAULT_LIMIT: i64 = 12;
const STATIC_PAGE_DRAFT_LIST_MAX_LIMIT: i64 = 50;
const ACTIVE_SECRET_BINDING_IDS_HEADER: &str = "x-ai-data-platform-secret-binding-ids";
const PUBLIC_DATASET_WARNING: &str = "该数据集是公开数据集，所有用户可见";

const DEFAULT_PUBLIC_DATASETS: &[(&str, &str, &str)] = &[
    ("orders", "订单", "默认公开订单数据集。"),
    ("customer-service", "客服", "默认公开客服数据集。"),
    ("enterprise-qa", "企业问答", "默认公开企业问答数据集。"),
    ("web-capture", "网页采集", "默认公开网页采集数据集。"),
    ("unclassified", "未分类", "默认公开未分类数据集。"),
];

#[derive(Debug, Deserialize)]
struct ConversationMemoryQuery {
    local_thread_id: Option<String>,
    query: Option<String>,
    limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct StaticPageDraftListQuery {
    local_thread_id: Option<String>,
    assistant_run_id: Option<String>,
    limit: Option<i64>,
}

#[derive(Clone)]
pub struct AppState {
    event_bus: EventBus,
    tools: Vec<ToolDefinitionView>,
    workflows: Vec<WorkflowDefinitionView>,
    workflow_catalog: WorkflowCatalog,
    storage: PgStorage,
    tenant_id: TenantId,
}

impl AppState {
    pub fn new(
        storage: PgStorage,
        workflow_catalog: WorkflowCatalog,
        tenant_id: TenantId,
        event_bus: EventBus,
    ) -> Self {
        let tool_registry = bootstrap_default_tool_registry();
        let tools = tool_registry
            .list()
            .into_iter()
            .map(to_tool_definition_view)
            .collect();
        let workflows = workflow_catalog
            .descriptors()
            .into_iter()
            .map(|definition| WorkflowDefinitionView {
                kind: definition.kind,
                version: definition.version,
                summary: definition.summary,
                accepted_signals: definition
                    .accepted_signals
                    .into_iter()
                    .map(|signal| signal.as_str().to_string())
                    .collect(),
            })
            .collect();

        Self {
            event_bus,
            tools,
            workflows,
            workflow_catalog,
            storage,
            tenant_id,
        }
    }
}

pub fn router(
    storage: PgStorage,
    workflow_catalog: WorkflowCatalog,
    tenant_id: TenantId,
    event_bus: EventBus,
) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/v1/datasets", get(list_datasets).post(create_dataset))
        .route(
            "/v1/dataset-secret-bindings",
            axum::routing::post(create_dataset_secret_binding),
        )
        .route(
            "/v1/dataset-secret-bindings/resolve",
            axum::routing::post(resolve_dataset_secret_bindings),
        )
        .route(
            "/v1/datasets/{dataset_id}/memory-directories",
            get(list_memory_directories),
        )
        .route(
            "/v1/datasets/{dataset_id}/retrieval-evidences",
            get(list_dataset_retrieval_evidences),
        )
        .route(
            "/v1/datasets/{dataset_id}/memory-directory-refresh",
            axum::routing::post(create_memory_directory_refresh),
        )
        .route(
            "/v1/datasets/{dataset_id}/outputs",
            get(list_dataset_outputs).post(create_dataset_output),
        )
        .route(
            "/v1/dataset-outputs/{output_id}/retrieval-evidences",
            get(list_dataset_output_retrieval_evidences),
        )
        .route(
            "/v1/datasets/{dataset_id}/chat-sessions",
            get(list_chat_sessions).post(create_chat_session),
        )
        .route("/v1/documents", get(list_documents).post(register_document))
        .route(
            "/v1/documents/compare",
            axum::routing::post(compare_documents_route),
        )
        .route(
            "/v1/chat-sessions/{session_id}/messages",
            get(list_chat_messages),
        )
        .route(
            "/v1/chat-sessions/{session_id}/turns",
            axum::routing::post(append_chat_session_turn),
        )
        .route(
            "/v1/chat-sessions/{session_id}/report-entry",
            axum::routing::post(update_chat_session_report_entry_route),
        )
        .route(
            "/v1/assistant-runs",
            axum::routing::post(create_assistant_run),
        )
        .route("/v1/assistant-runs/{run_id}", get(get_assistant_run))
        .route(
            "/v1/assistant-runs/{run_id}/events",
            axum::routing::post(append_assistant_run_event),
        )
        .route(
            "/v1/assistant-runs/{run_id}/continue",
            axum::routing::post(continue_assistant_run),
        )
        .route(
            "/v1/assistant-runs/{run_id}/conversation-memory-candidates",
            get(list_assistant_run_conversation_memory_candidates),
        )
        .route(
            "/v1/assistant-runs/{run_id}/static-page-drafts",
            axum::routing::post(create_static_page_draft_for_assistant_run),
        )
        .route("/v1/static-page-drafts", get(list_static_page_drafts))
        .route(
            "/v1/static-page-drafts/{draft_id}",
            get(get_static_page_draft).patch(update_static_page_draft),
        )
        .route(
            "/v1/static-page-drafts/{draft_id}/operations",
            axum::routing::post(append_static_page_draft_operations),
        )
        .route(
            "/v1/static-page-drafts/{draft_id}/intent",
            axum::routing::post(apply_static_page_draft_intent),
        )
        .route(
            "/v1/static-page-drafts/{draft_id}/image-jobs",
            get(list_static_page_image_jobs).post(create_static_page_image_job),
        )
        .route(
            "/v1/static-page-image-jobs/{job_id}",
            get(get_static_page_image_job),
        )
        .route(
            "/v1/static-page-image-jobs/{job_id}/confirm",
            axum::routing::post(confirm_static_page_image_job),
        )
        .route(
            "/v1/static-page-drafts/{draft_id}/renders",
            get(list_static_page_render_outputs).post(create_static_page_render),
        )
        .route(
            "/v1/conversation-memory-items",
            get(list_conversation_memory_items).post(create_conversation_memory_item),
        )
        .route(
            "/v1/documents/{document_id}/chunks",
            get(list_document_chunks),
        )
        .route(
            "/v1/documents/{document_id}/detail",
            get(get_document_detail),
        )
        .route(
            "/v1/documents/{document_id}/retrieval-evidences",
            get(list_document_retrieval_evidences),
        )
        .route(
            "/v1/documents/{document_id}/ingest",
            axum::routing::post(create_document_ingest),
        )
        .route(
            "/v1/report-plans",
            get(list_report_plans).post(create_report_plan),
        )
        .route(
            "/v1/report-plans/{plan_id}/renders",
            axum::routing::post(create_report_render),
        )
        .route(
            "/v1/report-plans/{plan_id}/ast-versions",
            get(list_report_plan_ast_versions),
        )
        .route(
            "/v1/report-plans/{plan_id}/continue",
            axum::routing::post(continue_report_plan),
        )
        .route(
            "/v1/report-plans/{plan_id}/publish",
            axum::routing::post(publish_report),
        )
        .route(
            "/v1/report-plans/{plan_id}/published-report",
            get(get_report_plan_published_report),
        )
        .route(
            "/v1/report-plans/{plan_id}/render-outputs",
            get(list_report_render_outputs),
        )
        .route("/v1/published-reports", get(list_published_reports))
        .route(
            "/v1/published-reports/{report_id}",
            get(get_published_report),
        )
        .route("/v1/workflow-executions", get(list_workflow_executions))
        .route(
            "/v1/workflow-executions/{execution_id}",
            get(get_workflow_execution),
        )
        .route(
            "/v1/workflow-executions/{execution_id}/start",
            axum::routing::post(start_workflow_execution),
        )
        .route(
            "/v1/workflow-executions/{execution_id}/retry",
            axum::routing::post(retry_workflow_execution_route),
        )
        .route(
            "/v1/workflow-executions/{execution_id}/signals",
            axum::routing::post(send_workflow_signal),
        )
        .route(
            "/v1/workflow-executions/{execution_id}/events",
            get(list_workflow_events),
        )
        .route(
            "/v1/workflow-executions/{execution_id}/tasks",
            get(list_workflow_tasks),
        )
        .route(
            "/v1/workflow-executions/{execution_id}/runtime-inspect",
            get(get_workflow_runtime_inspect),
        )
        .route(
            "/v1/workflow-executions/{execution_id}/llm-invocations",
            get(list_llm_invocations),
        )
        .route(
            "/v1/workflow-executions/{execution_id}/tool-executions",
            get(list_tool_executions),
        )
        .route("/v1/tools", get(list_tools))
        .route("/v1/workflows/definitions", get(list_workflows))
        .with_state(AppState::new(
            storage,
            workflow_catalog,
            tenant_id,
            event_bus,
        ))
}

pub async fn load_workflow_runtime_inspect(
    storage: PgStorage,
    tenant_id: TenantId,
    execution_id: WorkflowExecutionId,
) -> anyhow::Result<WorkflowRuntimeInspectView> {
    let state = AppState::new(
        storage,
        workflow_definitions::catalog(),
        tenant_id,
        EventBus::Disabled,
    );
    load_workflow_runtime_inspect_view(&state, execution_id)
        .await
        .map_err(|error| anyhow::anyhow!(error.payload.message))
}

pub async fn apply_chat_session_report_entry_update(
    storage: PgStorage,
    tenant_id: TenantId,
    session_id: ChatSessionId,
    request: UpdateChatSessionReportEntryRequest,
) -> std::result::Result<UpdateChatSessionReportEntryResponse, ApiError> {
    let state = AppState::new(
        storage,
        workflow_definitions::catalog(),
        tenant_id,
        EventBus::Disabled,
    );
    apply_chat_session_report_entry_update_with_state(&state, session_id, request).await
}

pub async fn request_report_render(
    storage: PgStorage,
    tenant_id: TenantId,
    plan_id: ReportPlanId,
    request: CreateReportRenderRequest,
) -> std::result::Result<CreateReportRenderResponse, ApiError> {
    let state = AppState::new(
        storage,
        workflow_definitions::catalog(),
        tenant_id,
        EventBus::Disabled,
    );
    create_report_render_response(&state, plan_id, request, &[]).await
}

pub async fn request_memory_directory_refresh(
    storage: PgStorage,
    tenant_id: TenantId,
    dataset_id: DatasetId,
) -> std::result::Result<CreateMemoryDirectoryRefreshResponse, ApiError> {
    let state = AppState::new(
        storage,
        workflow_definitions::catalog(),
        tenant_id,
        EventBus::Disabled,
    );
    create_memory_directory_refresh_response(&state, dataset_id, &[]).await
}

pub async fn load_document_detail(
    storage: PgStorage,
    tenant_id: TenantId,
    document_id: DocumentId,
) -> std::result::Result<DocumentDetailView, ApiError> {
    let state = AppState::new(
        storage,
        workflow_definitions::catalog(),
        tenant_id,
        EventBus::Disabled,
    );
    load_document_detail_with_state(&state, document_id, &[]).await
}

pub async fn compare_documents(
    storage: PgStorage,
    tenant_id: TenantId,
    request: CompareDocumentsRequest,
) -> std::result::Result<CompareDocumentsView, ApiError> {
    let state = AppState::new(
        storage,
        workflow_definitions::catalog(),
        tenant_id,
        EventBus::Disabled,
    );
    compare_documents_with_state(&state, request, &[]).await
}

pub async fn search_dataset_retrieval(
    storage: PgStorage,
    tenant_id: TenantId,
    dataset_id: DatasetId,
    query: String,
    limit: Option<usize>,
) -> std::result::Result<RetrievalSearchResponse, ApiError> {
    let state = AppState::new(
        storage,
        workflow_definitions::catalog(),
        tenant_id,
        EventBus::Disabled,
    );
    search_dataset_retrieval_with_state(&state, dataset_id, &query, limit, &[]).await
}

pub async fn request_workflow_retry(
    storage: PgStorage,
    tenant_id: TenantId,
    execution_id: WorkflowExecutionId,
    request: RetryWorkflowExecutionRequest,
) -> std::result::Result<RetryWorkflowExecutionResponse, ApiError> {
    let state = AppState::new(
        storage,
        workflow_definitions::catalog(),
        tenant_id,
        EventBus::Disabled,
    );
    retry_workflow_execution_with_state(&state, execution_id, request).await
}

pub async fn request_report_plan_continue(
    storage: PgStorage,
    tenant_id: TenantId,
    plan_id: ReportPlanId,
) -> std::result::Result<CreateReportPlanResponse, ApiError> {
    let state = AppState::new(
        storage,
        workflow_definitions::catalog(),
        tenant_id,
        EventBus::Disabled,
    );
    continue_report_plan_response(&state, plan_id, &[]).await
}

pub async fn request_report_publish(
    storage: PgStorage,
    tenant_id: TenantId,
    plan_id: ReportPlanId,
    request: PublishReportRequest,
) -> std::result::Result<PublishReportResponse, ApiError> {
    let state = AppState::new(
        storage,
        workflow_definitions::catalog(),
        tenant_id,
        EventBus::Disabled,
    );
    publish_report_response(&state, plan_id, request, &[]).await
}

pub async fn load_published_report(
    storage: PgStorage,
    tenant_id: TenantId,
    report_id: PublishedReportId,
) -> std::result::Result<PublishedReportDetailView, ApiError> {
    let state = AppState::new(
        storage,
        workflow_definitions::catalog(),
        tenant_id,
        EventBus::Disabled,
    );
    load_published_report_detail_with_state(&state, report_id, &[]).await
}

pub async fn load_published_report_by_plan(
    storage: PgStorage,
    tenant_id: TenantId,
    plan_id: ReportPlanId,
) -> std::result::Result<PublishedReportDetailView, ApiError> {
    let state = AppState::new(
        storage,
        workflow_definitions::catalog(),
        tenant_id,
        EventBus::Disabled,
    );
    load_published_report_detail_by_plan_with_state(&state, plan_id, &[]).await
}

pub fn render_workflow_runtime_pretty_summaries(
    inspect: &WorkflowRuntimeInspectView,
) -> Vec<String> {
    [
        render_model_facing_runtime_summary(inspect),
        render_execution_scope_runtime_summary(inspect),
        render_dataset_output_runtime_summary(inspect),
        render_report_plan_runtime_summary(inspect),
        render_report_render_output_runtime_summary(inspect),
        render_latest_assistant_turn_summary(inspect),
    ]
    .into_iter()
    .flatten()
    .collect()
}

pub fn render_model_facing_runtime_summary(inspect: &WorkflowRuntimeInspectView) -> Option<String> {
    let summary = inspect.model_facing.as_ref()?;
    let mut lines = begin_summary_block("Model-facing Runtime");
    push_summary_line(
        &mut lines,
        "capability_class",
        format_model_facing_capability_class(&summary.capability_class),
    );
    push_summary_line(
        &mut lines,
        "service_lane",
        format_model_facing_service_lane(&summary.service_lane),
    );
    push_summary_line(
        &mut lines,
        "report_entry_state",
        format_model_facing_report_entry_state(&summary.report_entry_state),
    );
    push_summary_line(
        &mut lines,
        "evidence_state",
        format_model_facing_evidence_state(&summary.evidence_state),
    );
    push_summary_line(
        &mut lines,
        "continuation_state",
        format_model_facing_continuation_state(&summary.continuation_state),
    );
    if let Some(action) = summary.recommended_next_action.as_ref() {
        push_summary_line(
            &mut lines,
            "recommended_next_action",
            format_model_facing_next_action(action),
        );
    }
    if !summary.allowed_next_actions.is_empty() {
        push_summary_line(
            &mut lines,
            "allowed_next_actions",
            summary
                .allowed_next_actions
                .iter()
                .map(format_model_facing_next_action)
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    if let Some(tool_key) = summary.recommended_tool_key.as_ref() {
        push_summary_line(&mut lines, "recommended_tool_key", tool_key);
    }
    if !summary.allowed_tool_keys.is_empty() {
        push_summary_line(
            &mut lines,
            "allowed_tool_keys",
            summary.allowed_tool_keys.join(", "),
        );
    }
    if !summary.signals.is_empty() {
        push_summary_line(&mut lines, "signals", summary.signals.join(", "));
    }

    Some(lines.join("\n"))
}

pub fn render_execution_scope_runtime_summary(
    inspect: &WorkflowRuntimeInspectView,
) -> Option<String> {
    let summary = inspect.execution_scope_runtime.as_ref()?;
    let mut lines = begin_summary_block("Execution-scope Runtime");
    push_summary_line(
        &mut lines,
        "llm_invocation_count",
        summary.llm_invocation_count,
    );
    push_summary_line(
        &mut lines,
        "tool_execution_count",
        summary.tool_execution_count,
    );
    push_summary_line(
        &mut lines,
        "failed_tool_execution_count",
        summary.failed_tool_execution_count,
    );
    push_optional_summary_line(
        &mut lines,
        "latest_provider",
        summary.latest_provider.as_deref(),
    );
    push_optional_summary_line(&mut lines, "latest_model", summary.latest_model.as_deref());
    push_optional_summary_line(
        &mut lines,
        "latest_request_id",
        summary.latest_request_id.as_deref(),
    );
    push_optional_summary_line(
        &mut lines,
        "latest_finish_reason",
        summary
            .latest_finish_reason
            .as_ref()
            .map(|value| value.as_str()),
    );
    push_optional_summary_line(
        &mut lines,
        "latest_tool_trace_count",
        summary.latest_tool_trace_count,
    );

    Some(lines.join("\n"))
}

pub fn render_latest_assistant_turn_summary(
    inspect: &WorkflowRuntimeInspectView,
) -> Option<String> {
    let assistant_message = inspect
        .chat_session
        .as_ref()?
        .latest_assistant_message
        .as_ref()?;
    let manifest = assistant_message.message_manifest_view.as_ref()?;
    let turn = manifest.turn.as_ref()?;
    let mut lines = begin_summary_block("Latest Assistant Turn");
    push_summary_line(&mut lines, "assistant_message_id", assistant_message.id);
    push_summary_line(&mut lines, "turn_id", &turn.turn_id);
    push_summary_line(&mut lines, "status", format!("{:?}", turn.status));
    push_summary_line(&mut lines, "stream_mode", format!("{:?}", turn.stream_mode));
    push_summary_line(
        &mut lines,
        "stream_status",
        format!("{:?}", turn.stream_status),
    );
    push_summary_line(
        &mut lines,
        "artifact_commit_status",
        format!("{:?}", turn.artifact_commit_status),
    );
    push_summary_line(
        &mut lines,
        "provider_status",
        format!("{:?}", turn.provider_status),
    );
    push_summary_line(
        &mut lines,
        "tool_loop_status",
        format!("{:?}", turn.tool_loop_status),
    );
    push_summary_line(&mut lines, "tool_trace_count", turn.tool_trace_count);
    push_summary_line(
        &mut lines,
        "llm_invocation_count",
        assistant_message.llm_invocations.len(),
    );
    push_summary_line(
        &mut lines,
        "tool_execution_count",
        assistant_message.tool_executions.len(),
    );
    push_optional_summary_line(
        &mut lines,
        "provider_request_id",
        turn.provider_request_id.as_deref(),
    );
    push_optional_summary_line(
        &mut lines,
        "finish_reason",
        turn.finish_reason.as_ref().map(|value| value.as_str()),
    );
    if let Some(failure) = turn.provider_failure.as_ref() {
        push_summary_line(&mut lines, "provider_failure_kind", failure.kind.as_str());
        push_summary_line(
            &mut lines,
            "provider_failure_message",
            failure.message.as_str(),
        );
    }
    push_optional_summary_line(
        &mut lines,
        "provider_requested_at",
        turn.provider_requested_at,
    );
    push_optional_summary_line(
        &mut lines,
        "provider_responded_at",
        turn.provider_responded_at,
    );
    push_optional_summary_line(&mut lines, "first_token_at", turn.first_token_at);
    push_optional_summary_line(&mut lines, "stream_completed_at", turn.stream_completed_at);
    push_optional_summary_line(
        &mut lines,
        "artifact_commit_ready_at",
        turn.artifact_commit_ready_at,
    );
    push_optional_summary_line(
        &mut lines,
        "artifact_commit_failure_source",
        turn.artifact_commit_failure_source
            .as_ref()
            .map(|value| format!("{value:?}")),
    );
    push_optional_summary_line(
        &mut lines,
        "tool_calls_emitted_at",
        turn.tool_calls_emitted_at,
    );
    push_optional_summary_line(
        &mut lines,
        "tool_loop_settled_at",
        turn.tool_loop_settled_at,
    );
    push_optional_summary_line(
        &mut lines,
        "assistant_message_persisted_at",
        turn.assistant_message_persisted_at,
    );
    push_optional_summary_line(&mut lines, "completed_at", turn.completed_at);
    if let Some(summary) = turn.tool_status_summary.as_ref() {
        push_summary_line(
            &mut lines,
            "tool_status_summary",
            format_tool_status_summary(
                summary.requested_count,
                summary.completed_count,
                summary.failed_count,
            ),
        );
    }

    Some(lines.join("\n"))
}

pub fn render_dataset_output_runtime_summary(
    inspect: &WorkflowRuntimeInspectView,
) -> Option<String> {
    let output = inspect.dataset_output.as_ref()?;
    let manifest = output.output_manifest_view.as_ref()?;
    let runtime = manifest.runtime.as_ref()?;
    let mut lines = begin_summary_block("Dataset Output Runtime");
    push_summary_line(&mut lines, "dataset_output_id", output.id);
    push_summary_line(
        &mut lines,
        "llm_invocation_count",
        output.llm_invocations.len(),
    );
    push_summary_line(
        &mut lines,
        "tool_execution_count",
        output.tool_executions.len(),
    );
    push_summary_line(
        &mut lines,
        "retrieval_evidence_count",
        output.retrieval_evidence_ids.len(),
    );
    push_summary_line(
        &mut lines,
        "output_section_count",
        manifest
            .output
            .as_ref()
            .map(|output| output.sections.len())
            .unwrap_or(0),
    );
    push_summary_line(&mut lines, "runtime_mode", format!("{:?}", runtime.mode));
    push_optional_summary_line(&mut lines, "provider", runtime.provider.as_deref());
    push_optional_summary_line(&mut lines, "model", runtime.model.as_deref());
    push_optional_summary_line(&mut lines, "request_id", runtime.request_id.as_deref());
    push_optional_summary_line(
        &mut lines,
        "finish_reason",
        runtime.finish_reason.as_ref().map(|value| value.as_str()),
    );
    if let Some(failure) = runtime.provider_failure.as_ref() {
        push_summary_line(&mut lines, "provider_failure_kind", failure.kind.as_str());
        push_summary_line(
            &mut lines,
            "provider_failure_message",
            failure.message.as_str(),
        );
    }
    push_optional_summary_line(&mut lines, "latency_ms", runtime.latency_ms);
    if let Some(usage) = runtime.usage.as_ref() {
        push_summary_line(
            &mut lines,
            "token_usage",
            format!(
                "input={}, output={}, total={}",
                usage.input_tokens, usage.output_tokens, usage.total_tokens
            ),
        );
    }
    push_optional_summary_line(
        &mut lines,
        "runtime_tool_trace_count",
        runtime.tool_trace_count,
    );
    if let Some((requested_count, completed_count, failed_count)) =
        summarize_tool_execution_status_counts(&output.tool_executions)
    {
        push_summary_line(
            &mut lines,
            "tool_status_summary",
            format_tool_status_summary(requested_count, completed_count, failed_count),
        );
    }

    Some(lines.join("\n"))
}

pub fn render_report_plan_runtime_summary(inspect: &WorkflowRuntimeInspectView) -> Option<String> {
    let plan = inspect.report_plan.as_ref()?;
    let mut lines = begin_summary_block("Report Plan Runtime");
    push_summary_line(&mut lines, "report_plan_id", plan.id);
    push_summary_line(&mut lines, "title", &plan.title);
    push_summary_line(&mut lines, "status", format!("{:?}", plan.status));
    push_summary_line(&mut lines, "theme_key", &plan.theme_key);
    push_optional_summary_line(
        &mut lines,
        "current_ast_version_id",
        plan.current_ast_version_id,
    );
    if let Some(handoff) = plan.service_handoff.as_ref() {
        push_summary_line(
            &mut lines,
            "service_handoff_source",
            format_manifest_service_handoff_source(&handoff.source),
        );
        push_summary_line(
            &mut lines,
            "report_entry_state",
            format_model_facing_report_entry_state(&handoff.report_entry_state),
        );
        push_optional_summary_line(
            &mut lines,
            "confirmed_report_plan_id",
            handoff.confirmed_report_plan_id,
        );
    }

    Some(lines.join("\n"))
}

pub fn render_report_render_output_runtime_summary(
    inspect: &WorkflowRuntimeInspectView,
) -> Option<String> {
    let output = inspect.report_render_output.as_ref()?;
    let mut lines = begin_summary_block("Report Render Runtime");
    push_summary_line(&mut lines, "report_render_output_id", output.id);
    push_summary_line(&mut lines, "report_plan_id", output.plan_id);
    push_summary_line(&mut lines, "surface", output.surface.as_str());
    push_summary_line(&mut lines, "status", format!("{:?}", output.status));
    push_summary_line(
        &mut lines,
        "has_asset_path",
        report_render_output_has_asset_path(output),
    );
    push_optional_summary_line(
        &mut lines,
        "asset_kind",
        report_render_output_asset_kind(output),
    );
    if let Some(handoff) = output.service_handoff.as_ref() {
        push_summary_line(
            &mut lines,
            "service_handoff_source",
            format_manifest_service_handoff_source(&handoff.source),
        );
        push_summary_line(
            &mut lines,
            "report_entry_state",
            format_model_facing_report_entry_state(&handoff.report_entry_state),
        );
        push_optional_summary_line(
            &mut lines,
            "confirmed_report_plan_id",
            handoff.confirmed_report_plan_id,
        );
    }

    Some(lines.join("\n"))
}

fn begin_summary_block(title: &str) -> Vec<String> {
    vec![title.to_string()]
}

fn push_summary_line(lines: &mut Vec<String>, label: &str, value: impl Display) {
    lines.push(format!("  {label}: {value}"));
}

fn push_optional_summary_line<T: Display>(lines: &mut Vec<String>, label: &str, value: Option<T>) {
    if let Some(value) = value {
        push_summary_line(lines, label, value);
    }
}

fn format_tool_status_summary(
    requested_count: usize,
    completed_count: usize,
    failed_count: usize,
) -> String {
    format!("requested={requested_count}, completed={completed_count}, failed={failed_count}")
}

fn summarize_tool_execution_status_counts(
    tool_executions: &[ToolExecutionView],
) -> Option<(usize, usize, usize)> {
    if tool_executions.is_empty() {
        return None;
    }

    let mut requested_count = 0usize;
    let mut completed_count = 0usize;
    let mut failed_count = 0usize;
    for execution in tool_executions {
        match execution.status {
            contracts::ManifestToolCallStatusView::Requested => requested_count += 1,
            contracts::ManifestToolCallStatusView::Completed => completed_count += 1,
            contracts::ManifestToolCallStatusView::Failed => failed_count += 1,
        }
    }

    Some((requested_count, completed_count, failed_count))
}

fn format_model_facing_capability_class(
    value: &contracts::ModelFacingCapabilityClassView,
) -> &'static str {
    match value {
        contracts::ModelFacingCapabilityClassView::DatasetDirectoryAwareness => {
            "dataset_directory_awareness"
        }
        contracts::ModelFacingCapabilityClassView::EvidenceRetrieval => "evidence_retrieval",
        contracts::ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis => {
            "material_explanation_and_synthesis"
        }
        contracts::ModelFacingCapabilityClassView::ReportPlanning => "report_planning",
        contracts::ModelFacingCapabilityClassView::ReportGenerationAndEditing => {
            "report_generation_and_editing"
        }
        contracts::ModelFacingCapabilityClassView::ControlledPlatformAction => {
            "controlled_platform_action"
        }
    }
}

fn format_model_facing_evidence_state(
    value: &contracts::ModelFacingEvidenceStateView,
) -> &'static str {
    match value {
        contracts::ModelFacingEvidenceStateView::CatalogMemory => "catalog_memory",
        contracts::ModelFacingEvidenceStateView::SupplyOnly => "supply_only",
        contracts::ModelFacingEvidenceStateView::LiveDetail => "live_detail",
        contracts::ModelFacingEvidenceStateView::Mixed => "mixed",
        contracts::ModelFacingEvidenceStateView::Degraded => "degraded",
    }
}

fn format_model_facing_service_lane(value: &contracts::ModelFacingServiceLaneView) -> &'static str {
    match value {
        contracts::ModelFacingServiceLaneView::MaterialService => "material_service",
        contracts::ModelFacingServiceLaneView::ReportService => "report_service",
        contracts::ModelFacingServiceLaneView::ControlledPlatformAction => {
            "controlled_platform_action"
        }
    }
}

fn format_model_facing_report_entry_state(
    value: &contracts::ModelFacingReportEntryStateView,
) -> &'static str {
    match value {
        contracts::ModelFacingReportEntryStateView::NotApplicable => "not_applicable",
        contracts::ModelFacingReportEntryStateView::ConfirmationRequired => "confirmation_required",
        contracts::ModelFacingReportEntryStateView::Confirmed => "confirmed",
    }
}

fn format_chat_session_report_entry_resolution(
    value: &contracts::ChatSessionReportEntryResolutionView,
) -> &'static str {
    match value {
        contracts::ChatSessionReportEntryResolutionView::StayMaterialService => {
            "stay_material_service"
        }
        contracts::ChatSessionReportEntryResolutionView::EnterReportService => {
            "enter_report_service"
        }
    }
}

fn format_manifest_service_handoff_source(
    value: &contracts::ManifestServiceHandoffSourceView,
) -> &'static str {
    match value {
        contracts::ManifestServiceHandoffSourceView::ChatSessionReportEntry => {
            "chat_session_report_entry"
        }
    }
}

fn format_model_facing_next_action(value: &contracts::ModelFacingNextActionView) -> &'static str {
    match value {
        contracts::ModelFacingNextActionView::AnswerDirectly => "answer_directly",
        contracts::ModelFacingNextActionView::ReadDocumentDetail => "read_document_detail",
        contracts::ModelFacingNextActionView::CompareDocuments => "compare_documents",
        contracts::ModelFacingNextActionView::RequestReportEntryConfirmation => {
            "request_report_entry_confirmation"
        }
        contracts::ModelFacingNextActionView::WaitForToolLoop => "wait_for_tool_loop",
        contracts::ModelFacingNextActionView::FinalizeArtifactCommit => "finalize_artifact_commit",
        contracts::ModelFacingNextActionView::RetryExecution => "retry_execution",
        contracts::ModelFacingNextActionView::RefreshDirectory => "refresh_directory",
        contracts::ModelFacingNextActionView::ContinueReportPlanning => "continue_report_planning",
        contracts::ModelFacingNextActionView::GenerateReportOutput => "generate_report_output",
        contracts::ModelFacingNextActionView::PublishReport => "publish_report",
    }
}

fn default_tool_key_for_model_facing_next_action(
    value: &contracts::ModelFacingNextActionView,
) -> Option<&'static str> {
    match value {
        contracts::ModelFacingNextActionView::RequestReportEntryConfirmation => {
            Some("chat_session.report_entry")
        }
        contracts::ModelFacingNextActionView::ReadDocumentDetail => Some("document.read_detail"),
        contracts::ModelFacingNextActionView::CompareDocuments => Some("document.compare"),
        contracts::ModelFacingNextActionView::RetryExecution => Some("workflow.retry"),
        contracts::ModelFacingNextActionView::RefreshDirectory => Some("memory_directory.refresh"),
        contracts::ModelFacingNextActionView::ContinueReportPlanning => Some("report.plan"),
        contracts::ModelFacingNextActionView::GenerateReportOutput => Some("report.render"),
        contracts::ModelFacingNextActionView::PublishReport => Some("report.publish"),
        _ => None,
    }
}

fn default_tool_keys_for_model_facing_next_actions(
    actions: &[contracts::ModelFacingNextActionView],
) -> Vec<String> {
    let mut tool_keys = Vec::new();
    for action in actions {
        let Some(tool_key) = default_tool_key_for_model_facing_next_action(action) else {
            continue;
        };
        if !tool_keys.iter().any(|existing| existing == tool_key) {
            tool_keys.push(tool_key.to_string());
        }
    }
    tool_keys
}

fn format_model_facing_continuation_state(
    value: &contracts::ModelFacingContinuationStateView,
) -> &'static str {
    match value {
        contracts::ModelFacingContinuationStateView::ReadyToAnswer => "ready_to_answer",
        contracts::ModelFacingContinuationStateView::NeedsPlatformContinuation => {
            "needs_platform_continuation"
        }
        contracts::ModelFacingContinuationStateView::NeedsUserConfirmation => {
            "needs_user_confirmation"
        }
        contracts::ModelFacingContinuationStateView::WaitingForRuntime => "waiting_for_runtime",
        contracts::ModelFacingContinuationStateView::RetryRequired => "retry_required",
    }
}

fn format_chat_turn_status(value: &contracts::ChatTurnStatusView) -> &'static str {
    match value {
        contracts::ChatTurnStatusView::Pending => "pending",
        contracts::ChatTurnStatusView::Completed => "completed",
        contracts::ChatTurnStatusView::Failed => "failed",
    }
}

fn format_chat_turn_artifact_commit_status(
    value: &contracts::ChatTurnArtifactCommitStatusView,
) -> &'static str {
    match value {
        contracts::ChatTurnArtifactCommitStatusView::NotReady => "not_ready",
        contracts::ChatTurnArtifactCommitStatusView::Pending => "pending",
        contracts::ChatTurnArtifactCommitStatusView::Failed => "failed",
        contracts::ChatTurnArtifactCommitStatusView::Completed => "completed",
    }
}

fn build_model_facing_summary(
    capability_class: contracts::ModelFacingCapabilityClassView,
    evidence_state: contracts::ModelFacingEvidenceStateView,
    mut allowed_next_actions: Vec<contracts::ModelFacingNextActionView>,
    signals: Vec<String>,
) -> contracts::WorkflowModelFacingSummaryView {
    allowed_next_actions.dedup();
    let service_lane = infer_model_facing_service_lane(&capability_class);
    let report_entry_state = infer_model_facing_report_entry_state(&capability_class);
    let continuation_state =
        infer_model_facing_continuation_state(&evidence_state, &allowed_next_actions);
    let recommended_next_action =
        infer_model_facing_recommended_next_action(&continuation_state, &allowed_next_actions);
    let recommended_tool_key = recommended_next_action
        .as_ref()
        .and_then(default_tool_key_for_model_facing_next_action)
        .map(str::to_string);
    let allowed_tool_keys = default_tool_keys_for_model_facing_next_actions(&allowed_next_actions);

    contracts::WorkflowModelFacingSummaryView {
        capability_class,
        service_lane,
        report_entry_state,
        evidence_state,
        continuation_state,
        recommended_next_action,
        allowed_next_actions,
        recommended_tool_key,
        allowed_tool_keys,
        signals,
    }
}

fn infer_model_facing_service_lane(
    capability_class: &contracts::ModelFacingCapabilityClassView,
) -> contracts::ModelFacingServiceLaneView {
    match capability_class {
        contracts::ModelFacingCapabilityClassView::DatasetDirectoryAwareness
        | contracts::ModelFacingCapabilityClassView::EvidenceRetrieval
        | contracts::ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis => {
            contracts::ModelFacingServiceLaneView::MaterialService
        }
        contracts::ModelFacingCapabilityClassView::ReportPlanning
        | contracts::ModelFacingCapabilityClassView::ReportGenerationAndEditing => {
            contracts::ModelFacingServiceLaneView::ReportService
        }
        contracts::ModelFacingCapabilityClassView::ControlledPlatformAction => {
            contracts::ModelFacingServiceLaneView::ControlledPlatformAction
        }
    }
}

fn infer_model_facing_report_entry_state(
    capability_class: &contracts::ModelFacingCapabilityClassView,
) -> contracts::ModelFacingReportEntryStateView {
    match capability_class {
        contracts::ModelFacingCapabilityClassView::ReportPlanning
        | contracts::ModelFacingCapabilityClassView::ReportGenerationAndEditing => {
            contracts::ModelFacingReportEntryStateView::Confirmed
        }
        contracts::ModelFacingCapabilityClassView::DatasetDirectoryAwareness
        | contracts::ModelFacingCapabilityClassView::EvidenceRetrieval
        | contracts::ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis
        | contracts::ModelFacingCapabilityClassView::ControlledPlatformAction => {
            contracts::ModelFacingReportEntryStateView::NotApplicable
        }
    }
}

fn infer_model_facing_continuation_state(
    evidence_state: &contracts::ModelFacingEvidenceStateView,
    allowed_next_actions: &[contracts::ModelFacingNextActionView],
) -> contracts::ModelFacingContinuationStateView {
    if *evidence_state == contracts::ModelFacingEvidenceStateView::Degraded {
        return contracts::ModelFacingContinuationStateView::RetryRequired;
    }
    if allowed_next_actions.iter().any(|action| {
        *action == contracts::ModelFacingNextActionView::RequestReportEntryConfirmation
    }) {
        return contracts::ModelFacingContinuationStateView::NeedsUserConfirmation;
    }
    if allowed_next_actions.iter().any(|action| {
        matches!(
            action,
            contracts::ModelFacingNextActionView::WaitForToolLoop
                | contracts::ModelFacingNextActionView::FinalizeArtifactCommit
        )
    }) {
        return contracts::ModelFacingContinuationStateView::WaitingForRuntime;
    }
    if allowed_next_actions
        .iter()
        .any(|action| *action == contracts::ModelFacingNextActionView::AnswerDirectly)
        || allowed_next_actions.is_empty()
    {
        return contracts::ModelFacingContinuationStateView::ReadyToAnswer;
    }

    contracts::ModelFacingContinuationStateView::NeedsPlatformContinuation
}

fn infer_model_facing_recommended_next_action(
    continuation_state: &contracts::ModelFacingContinuationStateView,
    allowed_next_actions: &[contracts::ModelFacingNextActionView],
) -> Option<contracts::ModelFacingNextActionView> {
    match continuation_state {
        contracts::ModelFacingContinuationStateView::RetryRequired => allowed_next_actions
            .iter()
            .find(|action| **action == contracts::ModelFacingNextActionView::RetryExecution)
            .cloned()
            .or_else(|| allowed_next_actions.first().cloned()),
        contracts::ModelFacingContinuationStateView::NeedsUserConfirmation => allowed_next_actions
            .iter()
            .find(|action| {
                **action == contracts::ModelFacingNextActionView::RequestReportEntryConfirmation
            })
            .cloned()
            .or_else(|| allowed_next_actions.first().cloned()),
        contracts::ModelFacingContinuationStateView::WaitingForRuntime => allowed_next_actions
            .iter()
            .find(|action| **action == contracts::ModelFacingNextActionView::WaitForToolLoop)
            .cloned()
            .or_else(|| {
                allowed_next_actions
                    .iter()
                    .find(|action| {
                        **action == contracts::ModelFacingNextActionView::FinalizeArtifactCommit
                    })
                    .cloned()
            })
            .or_else(|| allowed_next_actions.first().cloned()),
        contracts::ModelFacingContinuationStateView::ReadyToAnswer => allowed_next_actions
            .iter()
            .find(|action| **action == contracts::ModelFacingNextActionView::AnswerDirectly)
            .cloned()
            .or_else(|| allowed_next_actions.first().cloned()),
        contracts::ModelFacingContinuationStateView::NeedsPlatformContinuation => {
            allowed_next_actions
                .iter()
                .find(|action| {
                    !matches!(
                        action,
                        contracts::ModelFacingNextActionView::AnswerDirectly
                            | contracts::ModelFacingNextActionView::WaitForToolLoop
                            | contracts::ModelFacingNextActionView::FinalizeArtifactCommit
                            | contracts::ModelFacingNextActionView::RetryExecution
                    )
                })
                .cloned()
                .or_else(|| allowed_next_actions.first().cloned())
        }
    }
}

fn derive_model_facing_summary(
    inspect: &WorkflowRuntimeInspectView,
) -> contracts::WorkflowModelFacingSummaryView {
    if let Some(session) = inspect.chat_session.as_ref() {
        let summary = session
            .model_facing
            .clone()
            .unwrap_or_else(|| derive_chat_session_model_facing_summary(session));
        if inspect.execution.status == WorkflowStatus::Failed
            || inspect.execution.status == WorkflowStatus::DeadLettered
        {
            return degraded_model_facing_summary(summary.capability_class, summary.signals);
        }
        return summary;
    }
    if let Some(output) = inspect.dataset_output.as_ref() {
        let summary = output
            .model_facing
            .clone()
            .unwrap_or_else(|| derive_dataset_output_model_facing_summary(output));
        if inspect.execution.status == WorkflowStatus::Failed
            || inspect.execution.status == WorkflowStatus::DeadLettered
        {
            return degraded_model_facing_summary(summary.capability_class, summary.signals);
        }
        return summary;
    }
    if let Some(output) = inspect.report_render_output.as_ref() {
        let summary = output
            .model_facing
            .clone()
            .unwrap_or_else(|| derive_report_render_output_model_facing_summary(output));
        if inspect.execution.status == WorkflowStatus::Failed
            || inspect.execution.status == WorkflowStatus::DeadLettered
        {
            return degraded_model_facing_summary(summary.capability_class, summary.signals);
        }
        return summary;
    }
    if let Some(plan) = inspect.report_plan.as_ref() {
        let summary = plan
            .model_facing
            .clone()
            .unwrap_or_else(|| derive_report_plan_model_facing_summary(plan));
        if inspect.execution.status == WorkflowStatus::Failed
            || inspect.execution.status == WorkflowStatus::DeadLettered
        {
            return degraded_model_facing_summary(summary.capability_class, summary.signals);
        }
        return summary;
    }

    let capability_class = infer_model_facing_capability_class(inspect);
    let evidence_state = infer_model_facing_evidence_state(inspect);
    let allowed_next_actions =
        infer_model_facing_next_actions(inspect, &capability_class, &evidence_state);
    build_model_facing_summary(
        capability_class,
        evidence_state,
        allowed_next_actions,
        collect_model_facing_signals(inspect),
    )
}

fn derive_dataset_output_model_facing_summary(
    output: &DatasetOutputView,
) -> contracts::WorkflowModelFacingSummaryView {
    let base_capability_class = infer_dataset_output_model_facing_capability_class(output);
    let evidence_state = infer_dataset_output_model_facing_evidence_state(output);
    let mut signals = collect_dataset_output_model_facing_signals(output);

    if let Some(handoff) = dataset_output_service_handoff(output) {
        signals.extend(collect_service_handoff_signals(handoff));
        match handoff.report_entry_state {
            contracts::ModelFacingReportEntryStateView::ConfirmationRequired => {
                let mut summary = build_model_facing_summary(
                    base_capability_class,
                    evidence_state,
                    vec![contracts::ModelFacingNextActionView::RequestReportEntryConfirmation],
                    signals,
                );
                summary.service_lane = handoff.service_lane.clone();
                summary.report_entry_state = handoff.report_entry_state.clone();
                return summary;
            }
            contracts::ModelFacingReportEntryStateView::Confirmed => {
                let capability_class =
                    infer_service_handoff_capability_class(handoff, &base_capability_class);
                let allowed_next_actions = infer_service_handoff_next_actions(
                    handoff,
                    &base_capability_class,
                    &evidence_state,
                );
                let mut summary = build_model_facing_summary(
                    capability_class,
                    evidence_state,
                    allowed_next_actions,
                    signals,
                );
                summary.service_lane = handoff.service_lane.clone();
                summary.report_entry_state = handoff.report_entry_state.clone();
                return summary;
            }
            contracts::ModelFacingReportEntryStateView::NotApplicable => {}
        }
    }

    let allowed_next_actions = infer_dataset_output_model_facing_next_actions(
        output,
        &base_capability_class,
        &evidence_state,
    );
    build_model_facing_summary(
        base_capability_class,
        evidence_state,
        allowed_next_actions,
        signals,
    )
}

fn infer_dataset_output_model_facing_capability_class(
    output: &DatasetOutputView,
) -> contracts::ModelFacingCapabilityClassView {
    let has_answer_content = dataset_output_has_answer_content(output);
    let retrieval_evidence_count = output.retrieval_evidence_ids.len();

    if !has_answer_content && retrieval_evidence_count == 0 && output.memory_directory_id.is_some()
    {
        return contracts::ModelFacingCapabilityClassView::DatasetDirectoryAwareness;
    }
    if !has_answer_content && retrieval_evidence_count > 0 {
        return contracts::ModelFacingCapabilityClassView::EvidenceRetrieval;
    }

    contracts::ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis
}

fn infer_dataset_output_model_facing_evidence_state(
    output: &DatasetOutputView,
) -> contracts::ModelFacingEvidenceStateView {
    let has_memory_directory = output.memory_directory_id.is_some();
    let retrieval_evidence_count = output.retrieval_evidence_ids.len();
    let has_answer_content = dataset_output_has_answer_content(output);
    let document_focus = dataset_output_document_focus(output);

    if output
        .tool_executions
        .iter()
        .any(|execution| execution.status == contracts::ManifestToolCallStatusView::Failed)
    {
        return contracts::ModelFacingEvidenceStateView::Degraded;
    }
    if retrieval_evidence_count > 0 {
        if document_focus == ModelFacingDocumentFocus::SingleDocument && has_answer_content {
            return contracts::ModelFacingEvidenceStateView::LiveDetail;
        }
        if document_focus == ModelFacingDocumentFocus::MultiDocument
            || (has_memory_directory && has_answer_content)
        {
            return contracts::ModelFacingEvidenceStateView::Mixed;
        }
        return contracts::ModelFacingEvidenceStateView::SupplyOnly;
    }
    if has_memory_directory {
        return contracts::ModelFacingEvidenceStateView::CatalogMemory;
    }

    contracts::ModelFacingEvidenceStateView::CatalogMemory
}

fn infer_dataset_output_model_facing_next_actions(
    output: &DatasetOutputView,
    capability_class: &contracts::ModelFacingCapabilityClassView,
    evidence_state: &contracts::ModelFacingEvidenceStateView,
) -> Vec<contracts::ModelFacingNextActionView> {
    if *evidence_state == contracts::ModelFacingEvidenceStateView::Degraded {
        return vec![contracts::ModelFacingNextActionView::RetryExecution];
    }

    let document_focus = dataset_output_document_focus(output);
    let mut actions = Vec::new();
    match capability_class {
        contracts::ModelFacingCapabilityClassView::DatasetDirectoryAwareness => {
            actions.push(contracts::ModelFacingNextActionView::RefreshDirectory);
            actions.push(contracts::ModelFacingNextActionView::AnswerDirectly);
        }
        contracts::ModelFacingCapabilityClassView::EvidenceRetrieval => {
            if document_focus == ModelFacingDocumentFocus::MultiDocument {
                actions.push(contracts::ModelFacingNextActionView::CompareDocuments);
            }
            actions.push(contracts::ModelFacingNextActionView::ReadDocumentDetail);
        }
        contracts::ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis => {
            if document_focus == ModelFacingDocumentFocus::MultiDocument {
                actions.push(contracts::ModelFacingNextActionView::CompareDocuments);
            }
            if !output.retrieval_evidence_ids.is_empty() {
                actions.push(contracts::ModelFacingNextActionView::ReadDocumentDetail);
            }
            actions.push(contracts::ModelFacingNextActionView::AnswerDirectly);
        }
        contracts::ModelFacingCapabilityClassView::ReportPlanning => {
            actions.push(contracts::ModelFacingNextActionView::ContinueReportPlanning);
        }
        contracts::ModelFacingCapabilityClassView::ReportGenerationAndEditing => {
            actions.push(contracts::ModelFacingNextActionView::GenerateReportOutput);
        }
        contracts::ModelFacingCapabilityClassView::ControlledPlatformAction => {
            actions.push(contracts::ModelFacingNextActionView::RetryExecution);
        }
    }
    actions
}

fn collect_dataset_output_model_facing_signals(output: &DatasetOutputView) -> Vec<String> {
    let document_focus = dataset_output_document_focus(output);
    let distinct_document_count = dataset_output_distinct_document_count(output);
    let indexed_document_count = output
        .output_manifest_view
        .as_ref()
        .map(|manifest| manifest.indexed_document_count)
        .unwrap_or(0);
    vec![
        format!("workflow_kind={}", WorkflowKind::DatasetOutput.as_str()),
        format!(
            "retrieval_evidence_count={}",
            output.retrieval_evidence_ids.len()
        ),
        format!(
            "answer_content_present={}",
            dataset_output_has_answer_content(output)
        ),
        format!(
            "document_focus={}",
            format_model_facing_document_focus(document_focus)
        ),
        format!("distinct_document_count={distinct_document_count}"),
        format!("indexed_document_count={indexed_document_count}"),
        format!(
            "has_memory_directory={}",
            output.memory_directory_id.is_some()
        ),
        format!("tool_execution_count={}", output.tool_executions.len()),
    ]
}

fn derive_chat_session_model_facing_summary(
    session: &ChatSessionView,
) -> contracts::WorkflowModelFacingSummaryView {
    let base_capability_class = infer_chat_session_model_facing_capability_class(session);
    let evidence_state = infer_chat_session_model_facing_evidence_state(session);
    let report_entry = chat_session_report_entry(session);
    let mut signals = collect_chat_session_model_facing_signals(session);

    match report_entry.map(|entry| entry.state.clone()) {
        Some(contracts::ModelFacingReportEntryStateView::ConfirmationRequired) => {
            let mut summary = build_model_facing_summary(
                base_capability_class,
                evidence_state,
                vec![contracts::ModelFacingNextActionView::RequestReportEntryConfirmation],
                signals,
            );
            summary.report_entry_state =
                contracts::ModelFacingReportEntryStateView::ConfirmationRequired;
            summary
        }
        Some(contracts::ModelFacingReportEntryStateView::Confirmed) => {
            if let Some(report_plan_id) =
                report_entry.and_then(|entry| entry.confirmed_report_plan_id)
            {
                signals.push(format!("confirmed_report_plan_id={report_plan_id}"));
            }
            build_model_facing_summary(
                contracts::ModelFacingCapabilityClassView::ReportPlanning,
                evidence_state,
                vec![contracts::ModelFacingNextActionView::ContinueReportPlanning],
                signals,
            )
        }
        _ => {
            let allowed_next_actions = infer_chat_session_model_facing_next_actions(
                session,
                &base_capability_class,
                &evidence_state,
            );
            build_model_facing_summary(
                base_capability_class,
                evidence_state,
                allowed_next_actions,
                signals,
            )
        }
    }
}

fn infer_chat_session_model_facing_capability_class(
    session: &ChatSessionView,
) -> contracts::ModelFacingCapabilityClassView {
    let has_answer_content = chat_session_has_answer_content(session);
    let retrieval_evidence_count = count_chat_session_model_facing_retrieval_evidences(session);

    if !has_answer_content
        && retrieval_evidence_count == 0
        && (session.latest_memory_directory_id.is_some()
            || session
                .latest_dataset_output
                .as_ref()
                .and_then(|output| output.memory_directory_id)
                .is_some())
    {
        return contracts::ModelFacingCapabilityClassView::DatasetDirectoryAwareness;
    }
    if !has_answer_content && retrieval_evidence_count > 0 {
        return contracts::ModelFacingCapabilityClassView::EvidenceRetrieval;
    }

    contracts::ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis
}

fn infer_chat_session_model_facing_evidence_state(
    session: &ChatSessionView,
) -> contracts::ModelFacingEvidenceStateView {
    if latest_chat_session_turn(session)
        .map(|turn| {
            turn.status == contracts::ChatTurnStatusView::Failed
                || turn.artifact_commit_status
                    == contracts::ChatTurnArtifactCommitStatusView::Failed
                || turn.stream_status == contracts::ChatTurnStreamStatusView::Failed
                || turn.tool_loop_status == contracts::ChatTurnToolLoopStatusView::Failed
                || turn.provider_status == contracts::ChatTurnProviderStatusView::Failed
        })
        .unwrap_or(false)
    {
        return contracts::ModelFacingEvidenceStateView::Degraded;
    }

    let has_memory_directory = session.latest_memory_directory_id.is_some()
        || session
            .latest_dataset_output
            .as_ref()
            .and_then(|output| output.memory_directory_id)
            .is_some();
    let retrieval_evidence_count = count_chat_session_model_facing_retrieval_evidences(session);
    let has_answer_content = chat_session_has_answer_content(session);
    let document_focus = chat_session_document_focus(session);

    if retrieval_evidence_count > 0 {
        if document_focus == ModelFacingDocumentFocus::SingleDocument && has_answer_content {
            return contracts::ModelFacingEvidenceStateView::LiveDetail;
        }
        if document_focus == ModelFacingDocumentFocus::MultiDocument
            || (has_memory_directory && has_answer_content)
        {
            return contracts::ModelFacingEvidenceStateView::Mixed;
        }
        return contracts::ModelFacingEvidenceStateView::SupplyOnly;
    }
    if has_memory_directory {
        return contracts::ModelFacingEvidenceStateView::CatalogMemory;
    }

    contracts::ModelFacingEvidenceStateView::CatalogMemory
}

fn infer_chat_session_model_facing_next_actions(
    session: &ChatSessionView,
    capability_class: &contracts::ModelFacingCapabilityClassView,
    evidence_state: &contracts::ModelFacingEvidenceStateView,
) -> Vec<contracts::ModelFacingNextActionView> {
    if *evidence_state == contracts::ModelFacingEvidenceStateView::Degraded {
        return vec![contracts::ModelFacingNextActionView::RetryExecution];
    }

    let document_focus = chat_session_document_focus(session);
    let mut actions = Vec::new();
    if let Some(turn) = latest_chat_session_turn(session) {
        if turn.tool_loop_status == contracts::ChatTurnToolLoopStatusView::Pending {
            actions.push(contracts::ModelFacingNextActionView::WaitForToolLoop);
        }
        if turn.artifact_commit_status == contracts::ChatTurnArtifactCommitStatusView::Pending {
            actions.push(contracts::ModelFacingNextActionView::FinalizeArtifactCommit);
        }
    }
    match capability_class {
        contracts::ModelFacingCapabilityClassView::DatasetDirectoryAwareness => {
            actions.push(contracts::ModelFacingNextActionView::RefreshDirectory);
            actions.push(contracts::ModelFacingNextActionView::AnswerDirectly);
        }
        contracts::ModelFacingCapabilityClassView::EvidenceRetrieval => {
            if document_focus == ModelFacingDocumentFocus::MultiDocument {
                actions.push(contracts::ModelFacingNextActionView::CompareDocuments);
            }
            actions.push(contracts::ModelFacingNextActionView::ReadDocumentDetail);
        }
        contracts::ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis => {
            let retrieval_evidence_count =
                count_chat_session_model_facing_retrieval_evidences(session);
            if document_focus == ModelFacingDocumentFocus::MultiDocument
                || retrieval_evidence_count > 1
            {
                actions.push(contracts::ModelFacingNextActionView::CompareDocuments);
            }
            if retrieval_evidence_count > 0 {
                actions.push(contracts::ModelFacingNextActionView::ReadDocumentDetail);
            }
            actions.push(contracts::ModelFacingNextActionView::AnswerDirectly);
        }
        contracts::ModelFacingCapabilityClassView::ReportPlanning => {
            actions.push(contracts::ModelFacingNextActionView::ContinueReportPlanning);
        }
        contracts::ModelFacingCapabilityClassView::ReportGenerationAndEditing => {
            actions.push(contracts::ModelFacingNextActionView::GenerateReportOutput);
        }
        contracts::ModelFacingCapabilityClassView::ControlledPlatformAction => {
            actions.push(contracts::ModelFacingNextActionView::RetryExecution);
        }
    }
    actions
}

fn collect_chat_session_model_facing_signals(session: &ChatSessionView) -> Vec<String> {
    let document_focus = chat_session_document_focus(session);
    let distinct_document_count = chat_session_distinct_document_count(session);
    let indexed_document_count = chat_session_indexed_document_count(session);
    let mut signals = vec![
        format!("workflow_kind={}", WorkflowKind::ChatSession.as_str()),
        format!(
            "retrieval_evidence_count={}",
            count_chat_session_model_facing_retrieval_evidences(session)
        ),
        format!(
            "answer_content_present={}",
            chat_session_has_answer_content(session)
        ),
        format!(
            "document_focus={}",
            format_model_facing_document_focus(document_focus)
        ),
        format!("distinct_document_count={distinct_document_count}"),
        format!("indexed_document_count={indexed_document_count}"),
        format!(
            "has_memory_directory={}",
            session.latest_memory_directory_id.is_some()
                || session
                    .latest_dataset_output
                    .as_ref()
                    .and_then(|output| output.memory_directory_id)
                    .is_some()
        ),
    ];
    if let Some(turn) = latest_chat_session_turn(session) {
        signals.push(format!(
            "chat_turn_status={}",
            format_chat_turn_status(&turn.status)
        ));
        signals.push(format!(
            "artifact_commit_status={}",
            format_chat_turn_artifact_commit_status(&turn.artifact_commit_status)
        ));
    }
    if let Some(report_entry) = chat_session_report_entry(session) {
        signals.push(format!(
            "report_entry_state={}",
            format_model_facing_report_entry_state(&report_entry.state)
        ));
        if let Some(resolved_action) = report_entry.resolved_action.as_ref() {
            signals.push(format!(
                "report_entry_resolved_action={}",
                format_chat_session_report_entry_resolution(resolved_action)
            ));
        }
        if let Some(report_plan_id) = report_entry.confirmed_report_plan_id {
            signals.push(format!("confirmed_report_plan_id={report_plan_id}"));
        }
    }
    signals
}

fn count_chat_session_model_facing_retrieval_evidences(session: &ChatSessionView) -> usize {
    let latest_assistant_message_count = session
        .latest_assistant_message
        .as_ref()
        .and_then(|message| message.message_manifest_view.as_ref())
        .and_then(|manifest| manifest.output.as_ref())
        .map(|output| {
            output
                .sections
                .iter()
                .map(|section| section.retrieval_evidence_ids.len())
                .sum::<usize>()
        })
        .unwrap_or(0);
    let latest_dataset_output_count = session
        .latest_dataset_output
        .as_ref()
        .map(|output| output.retrieval_evidence_ids.len())
        .unwrap_or(0);

    latest_assistant_message_count.max(latest_dataset_output_count)
}

fn latest_chat_session_turn(session: &ChatSessionView) -> Option<&contracts::ChatTurnRuntimeView> {
    session
        .latest_assistant_message
        .as_ref()
        .and_then(|message| message.message_manifest_view.as_ref())
        .and_then(|manifest| manifest.turn.as_ref())
        .or_else(|| {
            session
                .session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
        })
}

fn chat_session_report_entry(
    session: &ChatSessionView,
) -> Option<&contracts::ChatSessionReportEntryView> {
    session
        .session_manifest_view
        .as_ref()
        .and_then(|manifest| manifest.report_entry.as_ref())
}

async fn load_report_plan_service_handoff(
    state: &AppState,
    report_plan_id: ReportPlanId,
) -> std::result::Result<Option<contracts::ManifestServiceHandoffView>, ApiError> {
    let execution = state
        .storage
        .workflow_executions()
        .get_latest_by_report_plan_and_kind(
            state.tenant_id,
            report_plan_id,
            WorkflowKind::ReportPlan,
        )
        .await
        .map_err(ApiError::from_storage)?;
    Ok(execution
        .as_ref()
        .and_then(workflow_execution_context_service_handoff))
}

fn workflow_execution_context_service_handoff(
    execution: &WorkflowExecution,
) -> Option<contracts::ManifestServiceHandoffView> {
    execution
        .context
        .as_object()
        .and_then(|context| context.get("service_handoff"))
        .and_then(parse_manifest_service_handoff)
}

fn dataset_output_service_handoff(
    output: &DatasetOutputView,
) -> Option<&contracts::ManifestServiceHandoffView> {
    output
        .output_manifest_view
        .as_ref()
        .and_then(|manifest| manifest.service_handoff.as_ref())
}

fn chat_message_service_handoff(
    message: &ChatMessageView,
) -> Option<&contracts::ManifestServiceHandoffView> {
    message
        .message_manifest_view
        .as_ref()
        .and_then(|manifest| manifest.service_handoff.as_ref())
}

fn collect_service_handoff_signals(handoff: &contracts::ManifestServiceHandoffView) -> Vec<String> {
    let mut signals = vec![
        format!(
            "service_handoff_source={}",
            format_manifest_service_handoff_source(&handoff.source)
        ),
        format!(
            "service_handoff_lane={}",
            format_model_facing_service_lane(&handoff.service_lane)
        ),
        format!(
            "report_entry_state={}",
            format_model_facing_report_entry_state(&handoff.report_entry_state)
        ),
        format!(
            "service_handoff_suggested_title_present={}",
            handoff.suggested_title.is_some()
        ),
        format!(
            "service_handoff_suggested_objective_present={}",
            handoff.suggested_objective.is_some()
        ),
    ];
    if let Some(resolved_action) = handoff.resolved_action.as_ref() {
        signals.push(format!(
            "report_entry_resolved_action={}",
            format_chat_session_report_entry_resolution(resolved_action)
        ));
    }
    if let Some(report_plan_id) = handoff.confirmed_report_plan_id {
        signals.push(format!("confirmed_report_plan_id={report_plan_id}"));
    }
    signals
}

fn derive_chat_message_model_facing_summary(
    message: &ChatMessageView,
) -> Option<contracts::WorkflowModelFacingSummaryView> {
    if !matches!(message.role, ChatMessageRole::Assistant) {
        return None;
    }

    let base_capability_class = infer_chat_message_model_facing_capability_class(message);
    let evidence_state = infer_chat_message_model_facing_evidence_state(message);
    let mut signals = collect_chat_message_model_facing_signals(message);

    if let Some(handoff) = chat_message_service_handoff(message) {
        signals.extend(collect_service_handoff_signals(handoff));
        match handoff.report_entry_state {
            contracts::ModelFacingReportEntryStateView::ConfirmationRequired => {
                let mut summary = build_model_facing_summary(
                    base_capability_class,
                    evidence_state,
                    vec![contracts::ModelFacingNextActionView::RequestReportEntryConfirmation],
                    signals,
                );
                summary.service_lane = handoff.service_lane.clone();
                summary.report_entry_state = handoff.report_entry_state.clone();
                return Some(summary);
            }
            contracts::ModelFacingReportEntryStateView::Confirmed => {
                let capability_class =
                    infer_service_handoff_capability_class(handoff, &base_capability_class);
                let allowed_next_actions = infer_service_handoff_next_actions(
                    handoff,
                    &base_capability_class,
                    &evidence_state,
                );
                let mut summary = build_model_facing_summary(
                    capability_class,
                    evidence_state,
                    allowed_next_actions,
                    signals,
                );
                summary.service_lane = handoff.service_lane.clone();
                summary.report_entry_state = handoff.report_entry_state.clone();
                return Some(summary);
            }
            contracts::ModelFacingReportEntryStateView::NotApplicable => {}
        }
    }

    let allowed_next_actions = infer_chat_message_model_facing_next_actions(
        message,
        &base_capability_class,
        &evidence_state,
    );
    Some(build_model_facing_summary(
        base_capability_class,
        evidence_state,
        allowed_next_actions,
        signals,
    ))
}

fn infer_service_handoff_capability_class(
    handoff: &contracts::ManifestServiceHandoffView,
    fallback: &contracts::ModelFacingCapabilityClassView,
) -> contracts::ModelFacingCapabilityClassView {
    match handoff.service_lane {
        contracts::ModelFacingServiceLaneView::ReportService => {
            contracts::ModelFacingCapabilityClassView::ReportPlanning
        }
        contracts::ModelFacingServiceLaneView::ControlledPlatformAction => {
            contracts::ModelFacingCapabilityClassView::ControlledPlatformAction
        }
        contracts::ModelFacingServiceLaneView::MaterialService => fallback.clone(),
    }
}

fn infer_service_handoff_next_actions(
    handoff: &contracts::ManifestServiceHandoffView,
    fallback: &contracts::ModelFacingCapabilityClassView,
    evidence_state: &contracts::ModelFacingEvidenceStateView,
) -> Vec<contracts::ModelFacingNextActionView> {
    if *evidence_state == contracts::ModelFacingEvidenceStateView::Degraded {
        return vec![contracts::ModelFacingNextActionView::RetryExecution];
    }

    match infer_service_handoff_capability_class(handoff, fallback) {
        contracts::ModelFacingCapabilityClassView::ReportPlanning => {
            vec![contracts::ModelFacingNextActionView::ContinueReportPlanning]
        }
        contracts::ModelFacingCapabilityClassView::ReportGenerationAndEditing => {
            vec![contracts::ModelFacingNextActionView::GenerateReportOutput]
        }
        contracts::ModelFacingCapabilityClassView::ControlledPlatformAction => {
            vec![contracts::ModelFacingNextActionView::RetryExecution]
        }
        contracts::ModelFacingCapabilityClassView::DatasetDirectoryAwareness
        | contracts::ModelFacingCapabilityClassView::EvidenceRetrieval
        | contracts::ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis => {
            vec![contracts::ModelFacingNextActionView::AnswerDirectly]
        }
    }
}

fn infer_chat_message_model_facing_capability_class(
    message: &ChatMessageView,
) -> contracts::ModelFacingCapabilityClassView {
    let has_answer_content = chat_message_has_answer_content(message);
    let retrieval_evidence_count = count_chat_message_model_facing_retrieval_evidences(message);

    if !has_answer_content
        && retrieval_evidence_count == 0
        && message
            .message_manifest_view
            .as_ref()
            .and_then(|manifest| manifest.latest_memory_directory_id)
            .is_some()
    {
        return contracts::ModelFacingCapabilityClassView::DatasetDirectoryAwareness;
    }
    if !has_answer_content && retrieval_evidence_count > 0 {
        return contracts::ModelFacingCapabilityClassView::EvidenceRetrieval;
    }

    contracts::ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis
}

fn infer_chat_message_model_facing_evidence_state(
    message: &ChatMessageView,
) -> contracts::ModelFacingEvidenceStateView {
    if chat_message_turn(message)
        .map(|turn| {
            turn.status == contracts::ChatTurnStatusView::Failed
                || turn.artifact_commit_status
                    == contracts::ChatTurnArtifactCommitStatusView::Failed
                || turn.stream_status == contracts::ChatTurnStreamStatusView::Failed
                || turn.tool_loop_status == contracts::ChatTurnToolLoopStatusView::Failed
                || turn.provider_status == contracts::ChatTurnProviderStatusView::Failed
        })
        .unwrap_or(false)
    {
        return contracts::ModelFacingEvidenceStateView::Degraded;
    }

    let has_memory_directory = message
        .message_manifest_view
        .as_ref()
        .and_then(|manifest| manifest.latest_memory_directory_id)
        .is_some();
    let retrieval_evidence_count = count_chat_message_model_facing_retrieval_evidences(message);
    let has_answer_content = chat_message_has_answer_content(message);
    let document_focus = chat_message_document_focus(message);

    if retrieval_evidence_count > 0 {
        if document_focus == ModelFacingDocumentFocus::SingleDocument && has_answer_content {
            return contracts::ModelFacingEvidenceStateView::LiveDetail;
        }
        if document_focus == ModelFacingDocumentFocus::MultiDocument
            || (has_memory_directory && has_answer_content)
        {
            return contracts::ModelFacingEvidenceStateView::Mixed;
        }
        return contracts::ModelFacingEvidenceStateView::SupplyOnly;
    }
    if has_memory_directory {
        return contracts::ModelFacingEvidenceStateView::CatalogMemory;
    }

    contracts::ModelFacingEvidenceStateView::CatalogMemory
}

fn infer_chat_message_model_facing_next_actions(
    message: &ChatMessageView,
    capability_class: &contracts::ModelFacingCapabilityClassView,
    evidence_state: &contracts::ModelFacingEvidenceStateView,
) -> Vec<contracts::ModelFacingNextActionView> {
    if *evidence_state == contracts::ModelFacingEvidenceStateView::Degraded {
        return vec![contracts::ModelFacingNextActionView::RetryExecution];
    }

    let document_focus = chat_message_document_focus(message);
    let mut actions = Vec::new();
    if let Some(turn) = chat_message_turn(message) {
        if turn.tool_loop_status == contracts::ChatTurnToolLoopStatusView::Pending {
            actions.push(contracts::ModelFacingNextActionView::WaitForToolLoop);
        }
        if turn.artifact_commit_status == contracts::ChatTurnArtifactCommitStatusView::Pending {
            actions.push(contracts::ModelFacingNextActionView::FinalizeArtifactCommit);
        }
    }

    match capability_class {
        contracts::ModelFacingCapabilityClassView::DatasetDirectoryAwareness => {
            actions.push(contracts::ModelFacingNextActionView::RefreshDirectory);
            actions.push(contracts::ModelFacingNextActionView::AnswerDirectly);
        }
        contracts::ModelFacingCapabilityClassView::EvidenceRetrieval => {
            if document_focus == ModelFacingDocumentFocus::MultiDocument {
                actions.push(contracts::ModelFacingNextActionView::CompareDocuments);
            }
            actions.push(contracts::ModelFacingNextActionView::ReadDocumentDetail);
        }
        contracts::ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis => {
            let retrieval_evidence_count =
                count_chat_message_model_facing_retrieval_evidences(message);
            if document_focus == ModelFacingDocumentFocus::MultiDocument
                || retrieval_evidence_count > 1
            {
                actions.push(contracts::ModelFacingNextActionView::CompareDocuments);
            }
            if retrieval_evidence_count > 0 {
                actions.push(contracts::ModelFacingNextActionView::ReadDocumentDetail);
            }
            actions.push(contracts::ModelFacingNextActionView::AnswerDirectly);
        }
        contracts::ModelFacingCapabilityClassView::ReportPlanning => {
            actions.push(contracts::ModelFacingNextActionView::ContinueReportPlanning);
        }
        contracts::ModelFacingCapabilityClassView::ReportGenerationAndEditing => {
            actions.push(contracts::ModelFacingNextActionView::GenerateReportOutput);
        }
        contracts::ModelFacingCapabilityClassView::ControlledPlatformAction => {
            actions.push(contracts::ModelFacingNextActionView::RetryExecution);
        }
    }

    actions
}

fn collect_chat_message_model_facing_signals(message: &ChatMessageView) -> Vec<String> {
    let document_focus = chat_message_document_focus(message);
    let indexed_document_count = chat_message_indexed_document_count(message);
    let mut signals = vec![
        format!("workflow_kind={}", WorkflowKind::ChatSession.as_str()),
        format!(
            "retrieval_evidence_count={}",
            count_chat_message_model_facing_retrieval_evidences(message)
        ),
        format!(
            "answer_content_present={}",
            chat_message_has_answer_content(message)
        ),
        format!(
            "document_focus={}",
            format_model_facing_document_focus(document_focus)
        ),
        "distinct_document_count=0".to_string(),
        format!("indexed_document_count={indexed_document_count}"),
        format!(
            "has_memory_directory={}",
            message
                .message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.latest_memory_directory_id)
                .is_some()
        ),
    ];
    if let Some(turn) = chat_message_turn(message) {
        signals.push(format!(
            "chat_turn_status={}",
            format_chat_turn_status(&turn.status)
        ));
        signals.push(format!(
            "artifact_commit_status={}",
            format_chat_turn_artifact_commit_status(&turn.artifact_commit_status)
        ));
    }
    signals
}

fn derive_document_detail_model_facing_summary(
    detail: &DocumentDetailView,
) -> contracts::WorkflowModelFacingSummaryView {
    let capability_class =
        contracts::ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis;
    let signals = collect_document_detail_model_facing_signals(detail);
    let evidence_state = infer_document_detail_model_facing_evidence_state(detail);

    if evidence_state == contracts::ModelFacingEvidenceStateView::Degraded {
        return degraded_model_facing_summary(capability_class, signals);
    }

    build_model_facing_summary(
        capability_class,
        evidence_state,
        vec![contracts::ModelFacingNextActionView::AnswerDirectly],
        signals,
    )
}

fn infer_document_detail_model_facing_evidence_state(
    detail: &DocumentDetailView,
) -> contracts::ModelFacingEvidenceStateView {
    if detail.document.lifecycle == contracts::DocumentLifecycleView::Failed
        || document_detail_failed_retrieval_evidence_count(detail) > 0
    {
        return contracts::ModelFacingEvidenceStateView::Degraded;
    }

    if !detail.retrieval_evidences.is_empty()
        || detail.document.lifecycle == contracts::DocumentLifecycleView::Indexed
    {
        return contracts::ModelFacingEvidenceStateView::LiveDetail;
    }
    if !detail.chunks.is_empty() {
        return contracts::ModelFacingEvidenceStateView::SupplyOnly;
    }

    contracts::ModelFacingEvidenceStateView::CatalogMemory
}

fn collect_document_detail_model_facing_signals(detail: &DocumentDetailView) -> Vec<String> {
    vec![
        "workflow_kind=document_detail".to_string(),
        "document_focus=single_document".to_string(),
        format!(
            "document_lifecycle={}",
            format_document_lifecycle_view(detail.document.lifecycle.clone())
        ),
        format!("chunk_count={}", detail.chunks.len()),
        format!(
            "indexed_chunk_count={}",
            detail
                .chunks
                .iter()
                .filter(|chunk| chunk.state == contracts::DocumentChunkStateView::Indexed)
                .count()
        ),
        format!(
            "retrieval_evidence_count={}",
            detail.retrieval_evidences.len()
        ),
        format!(
            "failed_retrieval_evidence_count={}",
            document_detail_failed_retrieval_evidence_count(detail)
        ),
    ]
}

fn document_detail_failed_retrieval_evidence_count(detail: &DocumentDetailView) -> usize {
    detail
        .retrieval_evidences
        .iter()
        .filter(|evidence| retrieval_evidence_has_failed_state(evidence))
        .count()
}

fn derive_compare_documents_model_facing_summary(
    comparison: &CompareDocumentsView,
) -> contracts::WorkflowModelFacingSummaryView {
    let capability_class =
        contracts::ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis;
    let signals = collect_compare_documents_model_facing_signals(comparison);
    let evidence_state = infer_compare_documents_model_facing_evidence_state(comparison);

    if evidence_state == contracts::ModelFacingEvidenceStateView::Degraded {
        return degraded_model_facing_summary(capability_class, signals);
    }

    let mut allowed_next_actions = vec![contracts::ModelFacingNextActionView::AnswerDirectly];
    if !comparison.documents.is_empty() {
        allowed_next_actions.insert(0, contracts::ModelFacingNextActionView::ReadDocumentDetail);
    }

    build_model_facing_summary(
        capability_class,
        evidence_state,
        allowed_next_actions,
        signals,
    )
}

fn infer_compare_documents_model_facing_evidence_state(
    comparison: &CompareDocumentsView,
) -> contracts::ModelFacingEvidenceStateView {
    let failed_document_count = comparison
        .documents
        .iter()
        .filter(|detail| detail.document.lifecycle == contracts::DocumentLifecycleView::Failed)
        .count();
    let failed_retrieval_evidence_count = comparison
        .documents
        .iter()
        .map(document_detail_failed_retrieval_evidence_count)
        .sum::<usize>();
    let total_chunk_count = comparison
        .documents
        .iter()
        .map(|detail| detail.chunks.len())
        .sum::<usize>();
    let total_retrieval_evidence_count = comparison
        .documents
        .iter()
        .map(|detail| detail.retrieval_evidences.len())
        .sum::<usize>();

    if failed_document_count > 0 || failed_retrieval_evidence_count > 0 {
        return contracts::ModelFacingEvidenceStateView::Degraded;
    }
    if comparison.documents.len() >= 2
        && (total_chunk_count > 0 || total_retrieval_evidence_count > 0)
    {
        return contracts::ModelFacingEvidenceStateView::Mixed;
    }
    if total_retrieval_evidence_count > 0 {
        return contracts::ModelFacingEvidenceStateView::SupplyOnly;
    }
    if total_chunk_count > 0 {
        return contracts::ModelFacingEvidenceStateView::CatalogMemory;
    }

    contracts::ModelFacingEvidenceStateView::CatalogMemory
}

fn collect_compare_documents_model_facing_signals(
    comparison: &CompareDocumentsView,
) -> Vec<String> {
    let document_focus = infer_model_facing_document_focus(comparison.documents.len(), 0);
    vec![
        "workflow_kind=document_compare".to_string(),
        format!(
            "document_focus={}",
            format_model_facing_document_focus(document_focus)
        ),
        format!("distinct_document_count={}", comparison.documents.len()),
        format!(
            "indexed_document_count={}",
            comparison
                .documents
                .iter()
                .filter(
                    |detail| detail.document.lifecycle == contracts::DocumentLifecycleView::Indexed
                )
                .count()
        ),
        format!(
            "chunk_count={}",
            comparison
                .documents
                .iter()
                .map(|detail| detail.chunks.len())
                .sum::<usize>()
        ),
        format!(
            "retrieval_evidence_count={}",
            comparison
                .documents
                .iter()
                .map(|detail| detail.retrieval_evidences.len())
                .sum::<usize>()
        ),
        format!(
            "failed_document_count={}",
            comparison
                .documents
                .iter()
                .filter(
                    |detail| detail.document.lifecycle == contracts::DocumentLifecycleView::Failed
                )
                .count()
        ),
        format!(
            "failed_retrieval_evidence_count={}",
            comparison
                .documents
                .iter()
                .map(document_detail_failed_retrieval_evidence_count)
                .sum::<usize>()
        ),
    ]
}

fn retrieval_evidence_has_failed_state(evidence: &RetrievalEvidenceView) -> bool {
    evidence
        .evidence_manifest_view
        .as_ref()
        .map(|manifest| {
            manifest.embedding.status == contracts::RetrievalEmbeddingStatusView::Failed
                || manifest.recall.status == contracts::RetrievalRecallStatusView::Failed
        })
        .unwrap_or(false)
}

fn format_document_lifecycle_view(value: contracts::DocumentLifecycleView) -> &'static str {
    match value {
        contracts::DocumentLifecycleView::Received => "received",
        contracts::DocumentLifecycleView::Extracted => "extracted",
        contracts::DocumentLifecycleView::Indexed => "indexed",
        contracts::DocumentLifecycleView::Failed => "failed",
    }
}

fn derive_report_plan_model_facing_summary(
    plan: &ReportPlanSummary,
) -> contracts::WorkflowModelFacingSummaryView {
    let capability_class = contracts::ModelFacingCapabilityClassView::ReportPlanning;
    let evidence_state = infer_report_plan_model_facing_evidence_state(plan);
    let allowed_next_actions = infer_report_plan_model_facing_next_actions(plan, &evidence_state);
    let mut signals = collect_report_plan_model_facing_signals(plan);
    if let Some(handoff) = plan.service_handoff.as_ref() {
        signals.extend(collect_service_handoff_signals(handoff));
    }
    let mut summary = build_model_facing_summary(
        capability_class,
        evidence_state,
        allowed_next_actions,
        signals,
    );
    if let Some(handoff) = plan.service_handoff.as_ref() {
        summary.service_lane = handoff.service_lane.clone();
        summary.report_entry_state = handoff.report_entry_state.clone();
    }
    summary
}

fn infer_report_plan_model_facing_evidence_state(
    plan: &ReportPlanSummary,
) -> contracts::ModelFacingEvidenceStateView {
    let has_current_ast_version = plan.current_ast_version_id.is_some();
    match plan.status {
        contracts::ReportPlanStatusView::Draft => {
            contracts::ModelFacingEvidenceStateView::CatalogMemory
        }
        contracts::ReportPlanStatusView::Planned
        | contracts::ReportPlanStatusView::Rendered
        | contracts::ReportPlanStatusView::Published => {
            if has_current_ast_version {
                contracts::ModelFacingEvidenceStateView::Mixed
            } else {
                contracts::ModelFacingEvidenceStateView::Degraded
            }
        }
    }
}

fn infer_report_plan_model_facing_next_actions(
    plan: &ReportPlanSummary,
    evidence_state: &contracts::ModelFacingEvidenceStateView,
) -> Vec<contracts::ModelFacingNextActionView> {
    if *evidence_state == contracts::ModelFacingEvidenceStateView::Degraded {
        return vec![contracts::ModelFacingNextActionView::RetryExecution];
    }

    if plan.current_ast_version_id.is_some()
        && matches!(
            plan.status,
            contracts::ReportPlanStatusView::Planned
                | contracts::ReportPlanStatusView::Rendered
                | contracts::ReportPlanStatusView::Published
        )
    {
        return vec![contracts::ModelFacingNextActionView::GenerateReportOutput];
    }

    vec![contracts::ModelFacingNextActionView::ContinueReportPlanning]
}

fn collect_report_plan_model_facing_signals(plan: &ReportPlanSummary) -> Vec<String> {
    vec![
        "workflow_kind=report_plan".to_string(),
        format!("report_plan_status={:?}", plan.status),
        format!(
            "has_current_ast_version={}",
            plan.current_ast_version_id.is_some()
        ),
        format!("theme_key={}", plan.theme_key),
    ]
}

fn derive_report_render_output_model_facing_summary(
    output: &ReportRenderOutputView,
) -> contracts::WorkflowModelFacingSummaryView {
    let capability_class = contracts::ModelFacingCapabilityClassView::ReportGenerationAndEditing;
    let evidence_state = infer_report_render_output_model_facing_evidence_state(output);
    let allowed_next_actions =
        infer_report_render_output_model_facing_next_actions(output, &evidence_state);
    let mut signals = collect_report_render_output_model_facing_signals(output);
    if let Some(handoff) = output.service_handoff.as_ref() {
        signals.extend(collect_service_handoff_signals(handoff));
    }
    let mut summary = build_model_facing_summary(
        capability_class,
        evidence_state,
        allowed_next_actions,
        signals,
    );
    if let Some(handoff) = output.service_handoff.as_ref() {
        summary.service_lane = handoff.service_lane.clone();
        summary.report_entry_state = handoff.report_entry_state.clone();
    }
    summary
}

fn infer_report_render_output_model_facing_evidence_state(
    output: &ReportRenderOutputView,
) -> contracts::ModelFacingEvidenceStateView {
    match output.status {
        contracts::ReportRenderOutputStatusView::Rendered => {
            contracts::ModelFacingEvidenceStateView::Mixed
        }
        contracts::ReportRenderOutputStatusView::Failed => {
            contracts::ModelFacingEvidenceStateView::Degraded
        }
    }
}

fn infer_report_render_output_model_facing_next_actions(
    output: &ReportRenderOutputView,
    evidence_state: &contracts::ModelFacingEvidenceStateView,
) -> Vec<contracts::ModelFacingNextActionView> {
    if *evidence_state == contracts::ModelFacingEvidenceStateView::Degraded {
        return vec![contracts::ModelFacingNextActionView::RetryExecution];
    }

    let mut actions = Vec::new();
    if report_render_output_has_asset_path(output) {
        actions.push(contracts::ModelFacingNextActionView::PublishReport);
    }
    actions
}

fn collect_report_render_output_model_facing_signals(
    output: &ReportRenderOutputView,
) -> Vec<String> {
    let mut signals = vec![
        "workflow_kind=report_render".to_string(),
        format!("report_render_status={:?}", output.status),
        format!("surface={}", output.surface.as_str()),
        format!(
            "has_asset_path={}",
            report_render_output_has_asset_path(output)
        ),
    ];
    if let Some(kind) = report_render_output_asset_kind(output) {
        signals.push(format!("asset_kind={kind}"));
    }
    signals
}

fn report_render_output_has_asset_path(output: &ReportRenderOutputView) -> bool {
    output
        .asset_manifest
        .as_object()
        .and_then(|manifest| manifest.get("path"))
        .and_then(Value::as_str)
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

fn report_render_output_asset_kind(output: &ReportRenderOutputView) -> Option<String> {
    output
        .asset_manifest
        .as_object()
        .and_then(|manifest| manifest.get("kind"))
        .and_then(Value::as_str)
        .map(|value| value.to_string())
}

fn count_chat_message_model_facing_retrieval_evidences(message: &ChatMessageView) -> usize {
    message
        .message_manifest_view
        .as_ref()
        .and_then(|manifest| manifest.output.as_ref())
        .map(|output| {
            output
                .sections
                .iter()
                .map(|section| section.retrieval_evidence_ids.len())
                .sum::<usize>()
        })
        .unwrap_or(0)
}

fn chat_message_has_answer_content(message: &ChatMessageView) -> bool {
    !message.content.trim().is_empty()
        || message
            .message_manifest_view
            .as_ref()
            .and_then(|manifest| manifest.output.as_ref())
            .map(|content| {
                content
                    .sections
                    .iter()
                    .any(|section| !section.content.trim().is_empty())
            })
            .unwrap_or(false)
}

fn chat_message_indexed_document_count(message: &ChatMessageView) -> usize {
    message
        .message_manifest_view
        .as_ref()
        .map(|manifest| manifest.indexed_document_count)
        .unwrap_or(0)
}

fn chat_message_document_focus(message: &ChatMessageView) -> ModelFacingDocumentFocus {
    infer_model_facing_document_focus(0, chat_message_indexed_document_count(message))
}

fn chat_message_turn(message: &ChatMessageView) -> Option<&contracts::ChatTurnRuntimeView> {
    message
        .message_manifest_view
        .as_ref()
        .and_then(|manifest| manifest.turn.as_ref())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModelFacingDocumentFocus {
    Unknown,
    SingleDocument,
    MultiDocument,
}

fn format_model_facing_document_focus(value: ModelFacingDocumentFocus) -> &'static str {
    match value {
        ModelFacingDocumentFocus::Unknown => "unknown",
        ModelFacingDocumentFocus::SingleDocument => "single_document",
        ModelFacingDocumentFocus::MultiDocument => "multi_document",
    }
}

fn infer_model_facing_document_focus(
    distinct_document_count: usize,
    indexed_document_count: usize,
) -> ModelFacingDocumentFocus {
    let count = if distinct_document_count > 0 {
        distinct_document_count
    } else {
        indexed_document_count
    };

    match count {
        0 => ModelFacingDocumentFocus::Unknown,
        1 => ModelFacingDocumentFocus::SingleDocument,
        _ => ModelFacingDocumentFocus::MultiDocument,
    }
}

fn dataset_output_has_answer_content(output: &DatasetOutputView) -> bool {
    !output.output_text.trim().is_empty()
        || output
            .output_manifest_view
            .as_ref()
            .and_then(|manifest| manifest.output.as_ref())
            .map(|content| {
                content
                    .sections
                    .iter()
                    .any(|section| !section.content.trim().is_empty())
            })
            .unwrap_or(false)
}

fn dataset_output_distinct_document_count(output: &DatasetOutputView) -> usize {
    output
        .retrieval_evidences
        .iter()
        .map(|evidence| evidence.document_id)
        .collect::<std::collections::BTreeSet<_>>()
        .len()
}

fn dataset_output_indexed_document_count(output: &DatasetOutputView) -> usize {
    output
        .output_manifest_view
        .as_ref()
        .map(|manifest| manifest.indexed_document_count)
        .unwrap_or(0)
}

fn dataset_output_document_focus(output: &DatasetOutputView) -> ModelFacingDocumentFocus {
    infer_model_facing_document_focus(
        dataset_output_distinct_document_count(output),
        dataset_output_indexed_document_count(output),
    )
}

fn chat_session_has_answer_content(session: &ChatSessionView) -> bool {
    session
        .latest_assistant_message
        .as_ref()
        .map(chat_message_has_answer_content)
        .unwrap_or(false)
}

fn chat_session_distinct_document_count(session: &ChatSessionView) -> usize {
    session
        .latest_dataset_output
        .as_ref()
        .map(dataset_output_distinct_document_count)
        .unwrap_or(0)
}

fn chat_session_indexed_document_count(session: &ChatSessionView) -> usize {
    session
        .latest_assistant_message
        .as_ref()
        .map(chat_message_indexed_document_count)
        .or_else(|| {
            session
                .latest_dataset_output
                .as_ref()
                .and_then(|output| output.output_manifest_view.as_ref())
                .map(|manifest| manifest.indexed_document_count)
        })
        .unwrap_or(0)
}

fn chat_session_document_focus(session: &ChatSessionView) -> ModelFacingDocumentFocus {
    infer_model_facing_document_focus(
        chat_session_distinct_document_count(session),
        chat_session_indexed_document_count(session),
    )
}

fn degraded_model_facing_summary(
    capability_class: contracts::ModelFacingCapabilityClassView,
    mut signals: Vec<String>,
) -> contracts::WorkflowModelFacingSummaryView {
    if !signals
        .iter()
        .any(|signal| signal == "execution_status=failed")
    {
        signals.push("execution_status=failed".to_string());
    }
    build_model_facing_summary(
        capability_class,
        contracts::ModelFacingEvidenceStateView::Degraded,
        vec![contracts::ModelFacingNextActionView::RetryExecution],
        signals,
    )
}

fn infer_model_facing_capability_class(
    inspect: &WorkflowRuntimeInspectView,
) -> contracts::ModelFacingCapabilityClassView {
    match inspect.execution.kind {
        WorkflowKind::MemoryDirectory => {
            contracts::ModelFacingCapabilityClassView::DatasetDirectoryAwareness
        }
        WorkflowKind::DatasetOutput | WorkflowKind::ChatSession => {
            contracts::ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis
        }
        WorkflowKind::ReportPlan => contracts::ModelFacingCapabilityClassView::ReportPlanning,
        WorkflowKind::ReportRender | WorkflowKind::StaticPageImageGeneration => {
            contracts::ModelFacingCapabilityClassView::ReportGenerationAndEditing
        }
        WorkflowKind::UploadIngest => {
            contracts::ModelFacingCapabilityClassView::ControlledPlatformAction
        }
    }
}

fn infer_model_facing_evidence_state(
    inspect: &WorkflowRuntimeInspectView,
) -> contracts::ModelFacingEvidenceStateView {
    if inspect.execution.status == WorkflowStatus::Failed
        || inspect.execution.status == WorkflowStatus::DeadLettered
    {
        return contracts::ModelFacingEvidenceStateView::Degraded;
    }

    if latest_assistant_turn(inspect)
        .map(|turn| {
            turn.status == contracts::ChatTurnStatusView::Failed
                || turn.artifact_commit_status
                    == contracts::ChatTurnArtifactCommitStatusView::Failed
                || turn.stream_status == contracts::ChatTurnStreamStatusView::Failed
                || turn.tool_loop_status == contracts::ChatTurnToolLoopStatusView::Failed
                || turn.provider_status == contracts::ChatTurnProviderStatusView::Failed
        })
        .unwrap_or(false)
    {
        return contracts::ModelFacingEvidenceStateView::Degraded;
    }

    let has_memory_directory = inspect
        .chat_session
        .as_ref()
        .and_then(|session| session.latest_memory_directory_id)
        .is_some()
        || inspect
            .dataset_output
            .as_ref()
            .and_then(|output| output.memory_directory_id)
            .is_some();
    let retrieval_evidence_count = count_model_facing_retrieval_evidences(inspect);

    if has_memory_directory && retrieval_evidence_count > 0 {
        return contracts::ModelFacingEvidenceStateView::Mixed;
    }
    if retrieval_evidence_count > 0 {
        return contracts::ModelFacingEvidenceStateView::SupplyOnly;
    }
    if has_memory_directory {
        return contracts::ModelFacingEvidenceStateView::CatalogMemory;
    }

    contracts::ModelFacingEvidenceStateView::CatalogMemory
}

fn infer_model_facing_next_actions(
    inspect: &WorkflowRuntimeInspectView,
    capability_class: &contracts::ModelFacingCapabilityClassView,
    evidence_state: &contracts::ModelFacingEvidenceStateView,
) -> Vec<contracts::ModelFacingNextActionView> {
    let mut actions = Vec::new();

    if *evidence_state == contracts::ModelFacingEvidenceStateView::Degraded {
        actions.push(contracts::ModelFacingNextActionView::RetryExecution);
        return actions;
    }

    if let Some(turn) = latest_assistant_turn(inspect) {
        if turn.tool_loop_status == contracts::ChatTurnToolLoopStatusView::Pending {
            actions.push(contracts::ModelFacingNextActionView::WaitForToolLoop);
        }
        if turn.artifact_commit_status == contracts::ChatTurnArtifactCommitStatusView::Pending {
            actions.push(contracts::ModelFacingNextActionView::FinalizeArtifactCommit);
        }
    }

    match capability_class {
        contracts::ModelFacingCapabilityClassView::DatasetDirectoryAwareness => {
            actions.push(contracts::ModelFacingNextActionView::RefreshDirectory);
            actions.push(contracts::ModelFacingNextActionView::AnswerDirectly);
        }
        contracts::ModelFacingCapabilityClassView::EvidenceRetrieval => {
            actions.push(contracts::ModelFacingNextActionView::ReadDocumentDetail);
        }
        contracts::ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis => {
            let retrieval_evidence_count = count_model_facing_retrieval_evidences(inspect);
            if retrieval_evidence_count > 1 {
                actions.push(contracts::ModelFacingNextActionView::CompareDocuments);
            }
            if retrieval_evidence_count > 0 {
                actions.push(contracts::ModelFacingNextActionView::ReadDocumentDetail);
            }
            actions.push(contracts::ModelFacingNextActionView::AnswerDirectly);
        }
        contracts::ModelFacingCapabilityClassView::ReportPlanning => {
            actions.push(contracts::ModelFacingNextActionView::ContinueReportPlanning);
        }
        contracts::ModelFacingCapabilityClassView::ReportGenerationAndEditing => {
            actions.push(contracts::ModelFacingNextActionView::GenerateReportOutput);
        }
        contracts::ModelFacingCapabilityClassView::ControlledPlatformAction => {
            actions.push(contracts::ModelFacingNextActionView::RetryExecution);
        }
    }

    actions
}

fn collect_model_facing_signals(inspect: &WorkflowRuntimeInspectView) -> Vec<String> {
    let mut signals = vec![format!("workflow_kind={}", inspect.execution.kind.as_str())];
    let retrieval_evidence_count = count_model_facing_retrieval_evidences(inspect);
    signals.push(format!(
        "retrieval_evidence_count={retrieval_evidence_count}"
    ));
    signals.push(format!(
        "has_memory_directory={}",
        inspect
            .chat_session
            .as_ref()
            .and_then(|session| session.latest_memory_directory_id)
            .is_some()
            || inspect
                .dataset_output
                .as_ref()
                .and_then(|output| output.memory_directory_id)
                .is_some()
    ));
    if let Some(turn) = latest_assistant_turn(inspect) {
        signals.push(format!(
            "chat_turn_status={}",
            format_chat_turn_status(&turn.status)
        ));
        signals.push(format!(
            "artifact_commit_status={}",
            format_chat_turn_artifact_commit_status(&turn.artifact_commit_status)
        ));
    }
    signals
}

fn count_model_facing_retrieval_evidences(inspect: &WorkflowRuntimeInspectView) -> usize {
    let dataset_output_count = inspect
        .dataset_output
        .as_ref()
        .map(|output| output.retrieval_evidence_ids.len())
        .unwrap_or(0);
    let assistant_message_count = inspect
        .chat_session
        .as_ref()
        .and_then(|session| session.latest_assistant_message.as_ref())
        .and_then(|message| message.message_manifest_view.as_ref())
        .and_then(|manifest| manifest.output.as_ref())
        .map(|output| {
            output
                .sections
                .iter()
                .map(|section| section.retrieval_evidence_ids.len())
                .sum::<usize>()
        })
        .unwrap_or(0);

    dataset_output_count.max(assistant_message_count)
}

fn latest_assistant_turn(
    inspect: &WorkflowRuntimeInspectView,
) -> Option<&contracts::ChatTurnRuntimeView> {
    inspect
        .chat_session
        .as_ref()?
        .latest_assistant_message
        .as_ref()?
        .message_manifest_view
        .as_ref()?
        .turn
        .as_ref()
}

async fn healthz() -> Json<HealthResponse> {
    Json(HealthResponse {
        service: "platform-api".to_string(),
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        checked_at: Utc::now(),
    })
}

async fn readyz(State(state): State<AppState>) -> (StatusCode, Json<HealthResponse>) {
    let checked_at = Utc::now();

    match state.storage.ping().await {
        Ok(()) => (
            StatusCode::OK,
            Json(HealthResponse {
                service: "platform-api".to_string(),
                status: "ready".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                checked_at,
            }),
        ),
        Err(error) => {
            tracing::error!(%error, "platform-api readiness check failed");

            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(HealthResponse {
                    service: "platform-api".to_string(),
                    status: "not_ready".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    checked_at,
                }),
            )
        }
    }
}

async fn list_workflows(State(state): State<AppState>) -> Json<Vec<WorkflowDefinitionView>> {
    Json(state.workflows)
}

async fn list_tools(State(state): State<AppState>) -> Json<Vec<ToolDefinitionView>> {
    Json(state.tools)
}

fn active_secret_binding_ids_from_headers(
    headers: &HeaderMap,
) -> std::result::Result<Vec<SecretBindingId>, ApiError> {
    let Some(value) = headers.get(ACTIVE_SECRET_BINDING_IDS_HEADER) else {
        return Ok(Vec::new());
    };
    let raw = value.to_str().map_err(|_| {
        ApiError::bad_request(
            "invalid_secret_binding_ids_header",
            format!("{ACTIVE_SECRET_BINDING_IDS_HEADER} must be valid utf-8"),
        )
    })?;
    raw.split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            Uuid::parse_str(entry)
                .map(SecretBindingId)
                .map_err(|error| {
                    ApiError::bad_request(
                        "invalid_secret_binding_id",
                        format!("invalid secret binding id {entry}: {error}"),
                    )
                })
        })
        .collect()
}

fn merge_secret_binding_ids(
    left: &[SecretBindingId],
    right: &[SecretBindingId],
) -> Vec<SecretBindingId> {
    let mut merged = Vec::with_capacity(left.len() + right.len());
    for id in left.iter().chain(right.iter()) {
        if !merged.iter().any(|existing| existing == id) {
            merged.push(*id);
        }
    }
    merged
}

fn dataset_is_visible(dataset: &Dataset, active_secret_binding_ids: &[SecretBindingId]) -> bool {
    dataset.visibility == DatasetVisibility::Public
        || dataset.default_secret_binding_ids.iter().any(|secret_id| {
            active_secret_binding_ids
                .iter()
                .any(|active_id| active_id == secret_id)
        })
}

fn filter_visible_datasets(
    datasets: Vec<Dataset>,
    active_secret_binding_ids: &[SecretBindingId],
) -> Vec<Dataset> {
    datasets
        .into_iter()
        .filter(|dataset| dataset_is_visible(dataset, active_secret_binding_ids))
        .collect()
}

fn dataset_not_found_error(dataset_id: DatasetId) -> ApiError {
    ApiError::not_found(
        "dataset_not_found",
        format!(
            "dataset {} was not found for tenant or current access scope",
            dataset_id
        ),
    )
}

async fn load_visible_dataset(
    state: &AppState,
    dataset_id: DatasetId,
    active_secret_binding_ids: &[SecretBindingId],
) -> std::result::Result<Dataset, ApiError> {
    let dataset = state
        .storage
        .datasets()
        .get_by_id(state.tenant_id, dataset_id)
        .await
        .map_err(ApiError::from_storage)?
        .ok_or_else(|| dataset_not_found_error(dataset_id))?;
    if !dataset_is_visible(&dataset, active_secret_binding_ids) {
        return Err(dataset_not_found_error(dataset_id));
    }
    Ok(dataset)
}

async fn load_visible_document(
    state: &AppState,
    document_id: DocumentId,
    active_secret_binding_ids: &[SecretBindingId],
) -> std::result::Result<Document, ApiError> {
    let document = state
        .storage
        .documents()
        .get_by_id(state.tenant_id, document_id)
        .await
        .map_err(ApiError::from_storage)?
        .ok_or_else(|| {
            ApiError::not_found(
                "document_not_found",
                format!("document {} was not found", document_id),
            )
        })?;
    load_visible_dataset(state, document.dataset_id, active_secret_binding_ids).await?;
    Ok(document)
}

async fn load_visible_report_plan(
    state: &AppState,
    plan_id: ReportPlanId,
    active_secret_binding_ids: &[SecretBindingId],
) -> std::result::Result<ReportPlan, ApiError> {
    let plan = state
        .storage
        .report_plans()
        .get_by_id(state.tenant_id, plan_id)
        .await
        .map_err(ApiError::from_storage)?
        .ok_or_else(|| {
            ApiError::not_found(
                "report_plan_not_found",
                format!("report plan {} was not found", plan_id),
            )
        })?;
    load_visible_dataset(state, plan.dataset_id, active_secret_binding_ids).await?;
    Ok(plan)
}

async fn ensure_default_public_datasets(state: &AppState) -> std::result::Result<(), ApiError> {
    let existing = state
        .storage
        .datasets()
        .list_by_tenant(state.tenant_id)
        .await
        .map_err(ApiError::from_storage)?;
    let existing_keys: HashSet<String> = existing.into_iter().map(|dataset| dataset.key).collect();
    for (key, title, description) in DEFAULT_PUBLIC_DATASETS {
        if existing_keys.contains(*key) {
            continue;
        }
        state
            .storage
            .datasets()
            .create_with_metadata(
                state.tenant_id,
                NewDataset {
                    key: (*key).to_string(),
                    title: (*title).to_string(),
                    description: Some((*description).to_string()),
                },
                json!({
                    "visibility": DatasetVisibility::Public.as_str(),
                    "default_secret_binding_ids": [],
                    "default_dataset": true,
                }),
            )
            .await
            .map_err(ApiError::from_storage)?;
    }
    Ok(())
}

fn dataset_summary(dataset: Dataset, access_warning: Option<String>) -> DatasetSummary {
    DatasetSummary {
        id: dataset.id,
        key: dataset.key,
        title: dataset.title,
        lifecycle: dataset.lifecycle,
        visibility: dataset.visibility,
        secret_binding_ids: dataset.default_secret_binding_ids,
        access_warning,
    }
}

async fn create_dataset_secret_binding(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateDatasetSecretBindingRequest>,
) -> std::result::Result<(StatusCode, Json<CreateDatasetSecretBindingResponse>), ApiError> {
    validate_required("fingerprint", &request.fingerprint)?;
    let active_secret_binding_ids = active_secret_binding_ids_from_headers(&headers)?;
    let dataset =
        load_visible_dataset(&state, request.dataset_id, &active_secret_binding_ids).await?;
    let label = trim_optional(request.label).unwrap_or_else(|| "local-browser-key".to_string());
    let binding = state
        .storage
        .secret_bindings()
        .create(
            state.tenant_id,
            NewSecretBinding {
                dataset_id: dataset.id,
                document_id: None,
                scope_level: SecretScopeLevel::Dataset,
                provider_key: label,
                cipher_text: "local-only".to_string(),
                fingerprint: request.fingerprint.trim().to_string(),
            },
        )
        .await
        .map_err(ApiError::from_storage)?;
    let next_secret_binding_ids =
        merge_secret_binding_ids(&dataset.default_secret_binding_ids, &[binding.id]);
    let updated_dataset = state
        .storage
        .datasets()
        .update_metadata(
            state.tenant_id,
            dataset.id,
            &json!({
                "visibility": DatasetVisibility::Private.as_str(),
                "default_secret_binding_ids": next_secret_binding_ids,
            }),
        )
        .await
        .map_err(ApiError::from_storage)?;
    let active_secret_binding_ids =
        merge_secret_binding_ids(&active_secret_binding_ids, &[binding.id]);

    Ok((
        StatusCode::CREATED,
        Json(CreateDatasetSecretBindingResponse {
            dataset: dataset_summary(updated_dataset, None),
            secret_binding_id: binding.id,
            active_secret_binding_ids,
        }),
    ))
}

async fn resolve_dataset_secret_bindings(
    State(state): State<AppState>,
    Json(request): Json<ResolveDatasetSecretBindingsRequest>,
) -> std::result::Result<Json<ResolveDatasetSecretBindingsResponse>, ApiError> {
    validate_required("fingerprint", &request.fingerprint)?;
    let bindings = state
        .storage
        .secret_bindings()
        .list_by_fingerprint(state.tenant_id, request.fingerprint.trim())
        .await
        .map_err(ApiError::from_storage)?;
    let secret_binding_ids: Vec<SecretBindingId> =
        bindings.iter().map(|binding| binding.id).collect();
    let mut datasets = Vec::new();
    for binding in bindings {
        if datasets
            .iter()
            .any(|dataset: &DatasetSummary| dataset.id == binding.dataset_id)
        {
            continue;
        }
        if let Some(dataset) = state
            .storage
            .datasets()
            .get_by_id(state.tenant_id, binding.dataset_id)
            .await
            .map_err(ApiError::from_storage)?
        {
            datasets.push(dataset_summary(dataset, None));
        }
    }

    Ok(Json(ResolveDatasetSecretBindingsResponse {
        secret_binding_ids,
        datasets,
    }))
}

async fn list_datasets(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> std::result::Result<Json<Vec<DatasetSummary>>, ApiError> {
    ensure_default_public_datasets(&state).await?;
    let active_secret_binding_ids = active_secret_binding_ids_from_headers(&headers)?;
    let datasets = state
        .storage
        .datasets()
        .list_by_tenant(state.tenant_id)
        .await
        .map_err(ApiError::from_storage)?;

    Ok(Json(
        filter_visible_datasets(datasets, &active_secret_binding_ids)
            .into_iter()
            .map(|dataset| dataset_summary(dataset, None))
            .collect(),
    ))
}

async fn create_dataset(
    State(state): State<AppState>,
    Json(request): Json<CreateDatasetRequest>,
) -> std::result::Result<(StatusCode, Json<DatasetSummary>), ApiError> {
    validate_required("key", &request.key)?;
    validate_required("title", &request.title)?;

    let secret_fingerprint = trim_optional(request.secret_fingerprint);
    let requested_secret_binding_ids = request.secret_binding_ids.clone();
    let visibility = if requested_secret_binding_ids.is_empty() && secret_fingerprint.is_none() {
        DatasetVisibility::Public
    } else {
        request.visibility.unwrap_or(DatasetVisibility::Private)
    };
    let access_warning =
        (visibility == DatasetVisibility::Public).then(|| PUBLIC_DATASET_WARNING.to_string());

    let dataset = state
        .storage
        .datasets()
        .create_with_metadata(
            state.tenant_id,
            NewDataset {
                key: request.key.trim().to_string(),
                title: request.title.trim().to_string(),
                description: trim_optional(request.description),
            },
            json!({
                "visibility": visibility.as_str(),
                "default_secret_binding_ids": requested_secret_binding_ids,
            }),
        )
        .await
        .map_err(ApiError::from_storage)?;
    let dataset = if let Some(fingerprint) = secret_fingerprint {
        validate_required("secret_fingerprint", &fingerprint)?;
        let label =
            trim_optional(request.secret_label).unwrap_or_else(|| "local-browser-key".to_string());
        let binding = state
            .storage
            .secret_bindings()
            .create(
                state.tenant_id,
                NewSecretBinding {
                    dataset_id: dataset.id,
                    document_id: None,
                    scope_level: SecretScopeLevel::Dataset,
                    provider_key: label,
                    cipher_text: "local-only".to_string(),
                    fingerprint,
                },
            )
            .await
            .map_err(ApiError::from_storage)?;
        let next_secret_binding_ids =
            merge_secret_binding_ids(&dataset.default_secret_binding_ids, &[binding.id]);
        state
            .storage
            .datasets()
            .update_metadata(
                state.tenant_id,
                dataset.id,
                &json!({
                    "visibility": DatasetVisibility::Private.as_str(),
                    "default_secret_binding_ids": next_secret_binding_ids,
                }),
            )
            .await
            .map_err(ApiError::from_storage)?
    } else {
        dataset
    };

    Ok((
        StatusCode::CREATED,
        Json(dataset_summary(dataset, access_warning)),
    ))
}

async fn list_memory_directories(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(dataset_id): Path<String>,
) -> std::result::Result<Json<Vec<MemoryDirectoryView>>, ApiError> {
    let dataset_id = parse_dataset_id(&dataset_id)?;
    let active_secret_binding_ids = active_secret_binding_ids_from_headers(&headers)?;
    load_visible_dataset(&state, dataset_id, &active_secret_binding_ids).await?;

    let directories = state
        .storage
        .memory_directories()
        .list_by_dataset(state.tenant_id, dataset_id)
        .await
        .map_err(ApiError::from_storage)?;

    Ok(Json(
        directories
            .into_iter()
            .map(to_memory_directory_view)
            .collect(),
    ))
}

async fn list_dataset_retrieval_evidences(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(dataset_id): Path<String>,
) -> std::result::Result<Json<Vec<RetrievalEvidenceView>>, ApiError> {
    let dataset_id = parse_dataset_id(&dataset_id)?;
    let active_secret_binding_ids = active_secret_binding_ids_from_headers(&headers)?;
    load_visible_dataset(&state, dataset_id, &active_secret_binding_ids).await?;

    let mut evidences = state
        .storage
        .retrieval_evidences()
        .list_latest_by_dataset(state.tenant_id, dataset_id, 100)
        .await
        .map_err(ApiError::from_storage)?;
    sort_retrieval_evidences_by_relevance(&mut evidences);

    Ok(Json(
        evidences
            .into_iter()
            .map(to_retrieval_evidence_view)
            .collect(),
    ))
}

async fn search_dataset_retrieval_with_state(
    state: &AppState,
    dataset_id: DatasetId,
    query: &str,
    limit: Option<usize>,
    active_secret_binding_ids: &[SecretBindingId],
) -> std::result::Result<RetrievalSearchResponse, ApiError> {
    validate_required("query", query)?;

    load_visible_dataset(state, dataset_id, active_secret_binding_ids).await?;

    let limit = normalize_retrieval_search_limit(limit);
    let evidences = state
        .storage
        .retrieval_evidences()
        .list_latest_by_dataset(
            state.tenant_id,
            dataset_id,
            retrieval_search_scan_limit(limit),
        )
        .await
        .map_err(ApiError::from_storage)?;

    Ok(RetrievalSearchResponse {
        hits: search_retrieval_hits(&evidences, query, limit),
    })
}

async fn create_memory_directory_refresh(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(dataset_id): Path<String>,
) -> std::result::Result<(StatusCode, Json<CreateMemoryDirectoryRefreshResponse>), ApiError> {
    let dataset_id = parse_dataset_id(&dataset_id)?;
    let active_secret_binding_ids = active_secret_binding_ids_from_headers(&headers)?;
    let response =
        create_memory_directory_refresh_response(&state, dataset_id, &active_secret_binding_ids)
            .await?;

    Ok((StatusCode::CREATED, Json(response)))
}

async fn create_memory_directory_refresh_response(
    state: &AppState,
    dataset_id: DatasetId,
    active_secret_binding_ids: &[SecretBindingId],
) -> std::result::Result<CreateMemoryDirectoryRefreshResponse, ApiError> {
    let dataset = load_visible_dataset(state, dataset_id, active_secret_binding_ids).await?;

    let execution = build_initial_memory_directory_execution(&state, dataset.id)?;
    let initial_event = build_initial_memory_directory_event(&execution);
    state
        .storage
        .workflow_executions()
        .create_with_initial_event(&execution, &initial_event)
        .await
        .map_err(ApiError::from_storage)?;

    Ok(CreateMemoryDirectoryRefreshResponse {
        workflow_execution: to_workflow_execution_view(execution),
    })
}

async fn list_dataset_outputs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(dataset_id): Path<String>,
) -> std::result::Result<Json<Vec<DatasetOutputView>>, ApiError> {
    let dataset_id = parse_dataset_id(&dataset_id)?;
    let active_secret_binding_ids = active_secret_binding_ids_from_headers(&headers)?;
    load_visible_dataset(&state, dataset_id, &active_secret_binding_ids).await?;

    let outputs = state
        .storage
        .dataset_outputs()
        .list_by_dataset(state.tenant_id, dataset_id)
        .await
        .map_err(ApiError::from_storage)?;

    let mut views = Vec::with_capacity(outputs.len());
    for output in outputs {
        views.push(hydrate_dataset_output_view(&state, output).await?);
    }

    Ok(Json(views))
}

async fn create_dataset_output(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(dataset_id): Path<String>,
    Json(request): Json<CreateDatasetOutputRequest>,
) -> std::result::Result<(StatusCode, Json<CreateDatasetOutputResponse>), ApiError> {
    let dataset_id = parse_dataset_id(&dataset_id)?;
    validate_required("prompt", &request.prompt)?;

    let active_secret_binding_ids = active_secret_binding_ids_from_headers(&headers)?;
    let dataset = load_visible_dataset(&state, dataset_id, &active_secret_binding_ids).await?;
    let bound_chat_session = match request.chat_session_id {
        Some(chat_session_id) => {
            let session = state
                .storage
                .chat_sessions()
                .get_by_id(state.tenant_id, chat_session_id)
                .await
                .map_err(ApiError::from_storage)?
                .ok_or_else(|| {
                    ApiError::not_found(
                        "chat_session_not_found",
                        format!("chat session {} was not found", chat_session_id),
                    )
                })?;
            if session.dataset_id != dataset.id {
                return Err(ApiError::bad_request(
                    "chat_session_dataset_mismatch",
                    format!(
                        "chat session {} belongs to dataset {}, expected {}",
                        chat_session_id, session.dataset_id, dataset.id
                    ),
                ));
            }
            Some(session)
        }
        None => None,
    };
    let latest_memory_directory = state
        .storage
        .memory_directories()
        .list_by_dataset(state.tenant_id, dataset.id)
        .await
        .map_err(ApiError::from_storage)?
        .into_iter()
        .next();
    let bound_retrieval_evidences = state
        .storage
        .retrieval_evidences()
        .list_latest_by_dataset(
            state.tenant_id,
            dataset.id,
            DATASET_OUTPUT_RETRIEVAL_SCAN_LIMIT,
        )
        .await
        .map_err(ApiError::from_storage)?;
    let bound_retrieval_evidence_ids = select_retrieval_evidence_ids_for_prompt(
        &bound_retrieval_evidences,
        request.prompt.trim(),
        DATASET_OUTPUT_RETRIEVAL_BIND_LIMIT,
    );

    let execution = build_initial_dataset_output_execution(
        &state,
        dataset.id,
        &request.prompt,
        bound_chat_session.as_ref().map(|session| session.id),
        latest_memory_directory.as_ref(),
        &bound_retrieval_evidence_ids,
    )?;
    let initial_event = build_initial_dataset_output_event(
        &execution,
        bound_chat_session.as_ref().map(|session| session.id),
        &request.prompt,
    );
    state
        .storage
        .workflow_executions()
        .create_with_initial_event(&execution, &initial_event)
        .await
        .map_err(ApiError::from_storage)?;

    Ok((
        StatusCode::CREATED,
        Json(CreateDatasetOutputResponse {
            workflow_execution: to_workflow_execution_view(execution),
        }),
    ))
}

async fn list_dataset_output_retrieval_evidences(
    State(state): State<AppState>,
    Path(output_id): Path<String>,
) -> std::result::Result<Json<Vec<RetrievalEvidenceView>>, ApiError> {
    let output_id = parse_dataset_output_id(&output_id)?;
    let output = state
        .storage
        .dataset_outputs()
        .get_by_id(state.tenant_id, output_id)
        .await
        .map_err(ApiError::from_storage)?;
    let Some(output) = output else {
        return Err(ApiError::not_found(
            "dataset_output_not_found",
            format!("dataset output {} was not found", output_id),
        ));
    };

    let evidences = state
        .storage
        .retrieval_evidences()
        .list_by_ids(state.tenant_id, &output.retrieval_evidence_ids)
        .await
        .map_err(ApiError::from_storage)?;

    Ok(Json(
        evidences
            .into_iter()
            .map(to_retrieval_evidence_view)
            .collect(),
    ))
}

async fn list_chat_sessions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(dataset_id): Path<String>,
) -> std::result::Result<Json<Vec<ChatSessionView>>, ApiError> {
    let dataset_id = parse_dataset_id(&dataset_id)?;
    let active_secret_binding_ids = active_secret_binding_ids_from_headers(&headers)?;
    load_visible_dataset(&state, dataset_id, &active_secret_binding_ids).await?;

    let sessions = state
        .storage
        .chat_sessions()
        .list_by_dataset(state.tenant_id, dataset_id)
        .await
        .map_err(ApiError::from_storage)?;

    let mut views = Vec::with_capacity(sessions.len());
    for session in sessions {
        views.push(hydrate_chat_session_view(&state, session).await?);
    }

    Ok(Json(views))
}

async fn create_chat_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(dataset_id): Path<String>,
    Json(request): Json<CreateChatSessionRequest>,
) -> std::result::Result<(StatusCode, Json<CreateChatSessionResponse>), ApiError> {
    let dataset_id = parse_dataset_id(&dataset_id)?;
    validate_required("prompt", &request.prompt)?;

    let active_secret_binding_ids = active_secret_binding_ids_from_headers(&headers)?;
    let dataset = load_visible_dataset(&state, dataset_id, &active_secret_binding_ids).await?;
    let latest_memory_directory = state
        .storage
        .memory_directories()
        .list_by_dataset(state.tenant_id, dataset.id)
        .await
        .map_err(ApiError::from_storage)?
        .into_iter()
        .next();
    let latest_dataset_output = state
        .storage
        .dataset_outputs()
        .list_by_dataset(state.tenant_id, dataset.id)
        .await
        .map_err(ApiError::from_storage)?
        .into_iter()
        .next();
    let chat_session_id = ChatSessionId::new();
    let execution = build_initial_chat_session_execution(
        &state,
        dataset.id,
        chat_session_id,
        &request.prompt,
        latest_memory_directory.as_ref(),
        latest_dataset_output.as_ref().map(|entry| entry.id),
    )?;
    let initial_event =
        build_initial_chat_session_event(&execution, chat_session_id, &request.prompt);
    let chat_turn_id = execution
        .context
        .get("chat_turn_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            ApiError::internal(
                "chat_turn_id_missing",
                "chat session execution context missing chat_turn_id".to_string(),
            )
        })?
        .to_string();

    state
        .storage
        .workflow_executions()
        .create_with_initial_event(&execution, &initial_event)
        .await
        .map_err(ApiError::from_storage)?;

    let session = state
        .storage
        .chat_sessions()
        .create(
            state.tenant_id,
            &NewChatSession {
                id: chat_session_id,
                execution_id: execution.id,
                dataset_id: dataset.id,
                title: derive_chat_session_title(&request.prompt),
                latest_memory_directory_id: latest_memory_directory.as_ref().map(|entry| entry.id),
                latest_dataset_output_id: latest_dataset_output.as_ref().map(|entry| entry.id),
                session_manifest: json!({
                    "generator": "chat-session-workflow",
                    "schema_version": "0.3.0",
                    "status": "pending_assistant_reply",
                    "initial_prompt": request.prompt.trim(),
                    "context_binding": "creation_time",
                    "latest_memory_directory_id": latest_memory_directory.as_ref().map(|entry| entry.id),
                    "latest_memory_directory_version_no": latest_memory_directory.as_ref().map(|entry| entry.version_no),
                    "latest_dataset_output_id": latest_dataset_output.as_ref().map(|entry| entry.id),
                    "last_turn": {
                        "turn_id": chat_turn_id,
                    "status": "pending",
                    "stream_mode": "buffered",
                    "provider_request_id": Value::Null,
                    "provider_status": "pending",
                    "finish_reason": Value::Null,
                    "assistant_message_id": Value::Null,
                    "tool_trace_count": 0,
                    "events": [{
                        "kind": "turn_started",
                        "at": execution.created_at,
                        "provider_request_id": Value::Null,
                        "finish_reason": Value::Null,
                        "tool_trace_count": Value::Null,
                    }],
                    "started_at": execution.created_at,
                    "completed_at": Value::Null,
                },
                }),
                created_at: execution.created_at,
            },
        )
        .await
        .map_err(ApiError::from_storage)?;
    state
        .storage
        .chat_messages()
        .create(
            state.tenant_id,
            &NewChatMessage {
                session_id: session.id,
                role: ChatMessageRole::User,
                turn_index: 0,
                content: request.prompt.trim().to_string(),
                message_manifest: json!({
                    "source": "user_prompt",
                    "workflow_execution_id": execution.id,
                }),
                created_at: execution.created_at,
            },
        )
        .await
        .map_err(ApiError::from_storage)?;

    Ok((
        StatusCode::CREATED,
        Json(CreateChatSessionResponse {
            chat_session: hydrate_chat_session_view(&state, session).await?,
            workflow_execution: to_workflow_execution_view(execution),
        }),
    ))
}

async fn append_chat_session_turn(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(request): Json<AppendChatSessionTurnRequest>,
) -> std::result::Result<(StatusCode, Json<AppendChatSessionTurnResponse>), ApiError> {
    let session_id = parse_chat_session_id(&session_id)?;
    validate_required("prompt", &request.prompt)?;

    let session = state
        .storage
        .chat_sessions()
        .get_by_id(state.tenant_id, session_id)
        .await
        .map_err(ApiError::from_storage)?
        .ok_or_else(|| {
            ApiError::not_found(
                "chat_session_not_found",
                format!("chat session {} was not found", session_id),
            )
        })?;
    if chat_session_has_in_progress_turn(&session.session_manifest) {
        return Err(ApiError::bad_request(
            "chat_session_turn_in_progress",
            format!("chat session {} already has a turn in progress", session_id),
        ));
    }

    let latest_memory_directory = state
        .storage
        .memory_directories()
        .list_by_dataset(state.tenant_id, session.dataset_id)
        .await
        .map_err(ApiError::from_storage)?
        .into_iter()
        .next();
    let latest_dataset_output = state
        .storage
        .dataset_outputs()
        .list_by_dataset(state.tenant_id, session.dataset_id)
        .await
        .map_err(ApiError::from_storage)?
        .into_iter()
        .next();
    let existing_messages = state
        .storage
        .chat_messages()
        .list_by_session(state.tenant_id, session.id)
        .await
        .map_err(ApiError::from_storage)?;
    let next_turn_index = existing_messages
        .iter()
        .map(|message| message.turn_index)
        .max()
        .map_or(0, |turn_index| turn_index + 1);
    let execution = build_initial_chat_session_execution(
        &state,
        session.dataset_id,
        session.id,
        &request.prompt,
        latest_memory_directory.as_ref(),
        latest_dataset_output.as_ref().map(|entry| entry.id),
    )?;
    let initial_event = build_initial_chat_session_event(&execution, session.id, &request.prompt);
    let chat_turn_id = execution
        .context
        .get("chat_turn_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            ApiError::internal(
                "chat_turn_id_missing",
                "chat session execution context missing chat_turn_id".to_string(),
            )
        })?
        .to_string();

    state
        .storage
        .workflow_executions()
        .create_with_initial_event(&execution, &initial_event)
        .await
        .map_err(ApiError::from_storage)?;
    let user_message = state
        .storage
        .chat_messages()
        .create(
            state.tenant_id,
            &NewChatMessage {
                session_id: session.id,
                role: ChatMessageRole::User,
                turn_index: next_turn_index,
                content: request.prompt.trim().to_string(),
                message_manifest: json!({
                    "source": "user_prompt",
                    "workflow_execution_id": execution.id,
                    "chat_turn_id": chat_turn_id,
                    "append_turn": true,
                }),
                created_at: execution.created_at,
            },
        )
        .await
        .map_err(ApiError::from_storage)?;
    let session_manifest = build_pending_chat_session_turn_manifest(
        &session.session_manifest,
        &execution,
        &request.prompt,
        latest_memory_directory.as_ref(),
        latest_dataset_output.as_ref(),
        &chat_turn_id,
    );
    let session = state
        .storage
        .chat_sessions()
        .update_context(
            state.tenant_id,
            session.id,
            latest_memory_directory.as_ref().map(|entry| entry.id),
            latest_dataset_output.as_ref().map(|entry| entry.id),
            &session_manifest,
            execution.created_at,
        )
        .await
        .map_err(ApiError::from_storage)?;

    Ok((
        StatusCode::CREATED,
        Json(AppendChatSessionTurnResponse {
            chat_session: hydrate_chat_session_view(&state, session).await?,
            user_message: hydrate_chat_message_view(&state, user_message).await?,
            workflow_execution: to_workflow_execution_view(execution),
        }),
    ))
}

async fn create_assistant_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(mut request): Json<CreateAssistantRunRequest>,
) -> std::result::Result<(StatusCode, Json<CreateAssistantRunResponse>), ApiError> {
    validate_required("prompt", &request.prompt)?;
    let active_secret_binding_ids = active_secret_binding_ids_from_headers(&headers)?;
    ensure_default_public_datasets(&state).await?;
    let visible_datasets = filter_visible_datasets(
        state
            .storage
            .datasets()
            .list_by_tenant(state.tenant_id)
            .await
            .map_err(ApiError::from_storage)?,
        &active_secret_binding_ids,
    );
    let selected_dataset_id = request
        .selected_scope
        .as_ref()
        .and_then(selected_dataset_id_from_scope);
    let local_thread_id = request
        .local_thread_id
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let conversation_memory_available = if let Some(local_thread_id) = local_thread_id.as_ref() {
        !state
            .storage
            .conversation_memory_items()
            .list_by_local_thread(state.tenant_id, local_thread_id, None, 1)
            .await
            .map_err(ApiError::from_storage)?
            .is_empty()
    } else {
        false
    };
    let scope_plan = plan_scope(ScopePlannerInput {
        prompt: &request.prompt,
        visible_datasets: &visible_datasets,
        selected_dataset_id,
        conversation_memory_available,
    });
    request.scope_candidates = candidates_to_values(&scope_plan.candidates);
    request.selected_scope = Some(scope_plan.selected_scope.clone());
    let selected_scope = scope_plan.selected_scope.clone();
    let evidence_state = build_assistant_run_evidence_state(
        &state,
        &selected_scope,
        &request.prompt,
        local_thread_id.as_deref(),
        &active_secret_binding_ids,
    )
    .await?;

    let runtime_mode =
        std::env::var("ASSISTANT_RUN_RUNTIME_MODE").unwrap_or_else(|_| "placeholder".to_string());
    let runtime_provider =
        std::env::var("ASSISTANT_RUN_RUNTIME_PROVIDER").unwrap_or_else(|_| runtime_mode.clone());
    let runtime_model = std::env::var("ASSISTANT_RUN_RUNTIME_MODEL")
        .unwrap_or_else(|_| DEFAULT_ASSISTANT_RUN_RUNTIME_MODEL.to_string());
    let runtime_mode_for_trail = runtime_mode.clone();
    let runtime_provider_for_trail = runtime_provider.clone();
    let runtime_model_for_trail = runtime_model.clone();
    let provider_input = if runtime_mode == "placeholder" {
        format!(
            "普通聊天运行时占位回复：后端 AssistantRun 已接收问题，真实模型接入后会直接返回模型回答。\n\nPrompt: {}\n\n供料状态: {}",
            request.prompt.trim(),
            assistant_run_evidence_status_label(&evidence_state)
        )
    } else {
        build_assistant_run_provider_input_with_evidence(&request, Some(&evidence_state))
    };
    let scope_candidates = request.scope_candidates.clone();

    let response = tokio::task::spawn_blocking(move || {
        let provider = build_provider_from_env(
            "ASSISTANT_RUN",
            &runtime_mode,
            runtime_provider,
            bootstrap_default_prompt_registry(),
        )?;
        provider.complete(&LlmRequest {
            model: runtime_model,
            system_prompt_key: None,
            input: provider_input,
        })
    })
    .await
    .map_err(|error| {
        ApiError::internal(
            "assistant_run_join_failed",
            format!("assistant run worker join failed: {error}"),
        )
    })?
    .map_err(|error| ApiError::internal("assistant_run_provider_failed", error.to_string()))?;

    let now = Utc::now();
    let runtime_manifest = render_runtime_manifest(&response.runtime);
    let planned_candidate_count = scope_candidates.len();
    let scope_candidates = Value::Array(scope_candidates.clone());
    let context_policy = request.context_policy_hint.clone().unwrap_or_else(|| {
        json!({
            "history_policy": "intent_gated",
            "supply_policy": "host_supplies_model_answers",
        })
    });
    let supplied_evidence_count = assistant_run_evidence_supplied_count(&evidence_state);
    let execution_trail = vec![
        json!({
            "status": "completed",
            "label": "接收用户问题",
            "at": now,
        }),
        json!({
            "status": "completed",
            "label": "规划供料候选",
            "candidate_count": planned_candidate_count,
            "hint": scope_plan.hint,
            "at": now,
        }),
        json!({
            "status": "completed",
            "label": "检索供料证据",
            "supplied_count": supplied_evidence_count,
            "evidence_status": evidence_state.get("status").and_then(Value::as_str).unwrap_or("unknown"),
            "at": now,
        }),
        json!({
            "status": "completed",
            "label": "普通聊天运行时返回",
            "runtime_mode": runtime_mode_for_trail,
            "provider": runtime_provider_for_trail,
            "model": runtime_model_for_trail,
            "at": now,
        }),
    ];
    let output_artifacts = vec![json!({
        "type": "assistant_message",
        "role": ChatMessageRole::Assistant.as_str(),
        "content": response.output_text,
    })];
    let run = state
        .storage
        .assistant_runs()
        .create(
            state.tenant_id,
            &NewAssistantRun {
                local_thread_id,
                user_prompt: request.prompt.trim().to_string(),
                startup_briefing: request
                    .startup_briefing
                    .clone()
                    .unwrap_or_else(|| json!({})),
                selected_scope: selected_scope.clone(),
                scope_candidates: scope_candidates.clone(),
                context_policy,
                evidence_state: evidence_state.clone(),
                service_lane: "ordinary_chat".to_string(),
                execution_trail: Value::Array(execution_trail.clone()),
                output_artifacts: Value::Array(output_artifacts.clone()),
                runtime_manifest: runtime_manifest.clone(),
                created_at: now,
            },
        )
        .await
        .map_err(ApiError::from_storage)?;
    state
        .storage
        .assistant_runs()
        .append_event(
            state.tenant_id,
            run.id,
            &NewAssistantRunEvent {
                event_name: "assistant_run.completed".to_string(),
                payload: json!({
                    "service_lane": run.service_lane.clone(),
                    "runtime": runtime_manifest.clone(),
                }),
                created_at: now,
            },
        )
        .await
        .map_err(ApiError::from_storage)?;

    Ok((
        StatusCode::CREATED,
        Json(CreateAssistantRunResponse {
            assistant_run_id: run.id,
            assistant_message: AssistantRunMessageView {
                role: ChatMessageRole::Assistant,
                content: output_artifacts
                    .first()
                    .and_then(|artifact| artifact.get("content"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            },
            runtime: run.runtime_manifest,
            selected_scope,
            scope_candidates: value_array(scope_candidates),
            evidence_state,
            execution_trail,
            output_artifacts,
            required_confirmations: Vec::new(),
        }),
    ))
}

async fn get_assistant_run(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
) -> std::result::Result<Json<AssistantRunDetailView>, ApiError> {
    let run_id = parse_assistant_run_id(&run_id)?;
    let run = state
        .storage
        .assistant_runs()
        .get_by_id(state.tenant_id, run_id)
        .await
        .map_err(ApiError::from_storage)?
        .ok_or_else(|| {
            ApiError::not_found(
                "assistant_run_not_found",
                format!("assistant run {} was not found", run_id),
            )
        })?;
    let events = state
        .storage
        .assistant_runs()
        .list_events(state.tenant_id, run_id)
        .await
        .map_err(ApiError::from_storage)?;

    Ok(Json(AssistantRunDetailView {
        run: to_assistant_run_view(run),
        events: events
            .into_iter()
            .map(to_assistant_run_event_view)
            .collect(),
    }))
}

async fn append_assistant_run_event(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
    Json(request): Json<AppendAssistantRunEventRequest>,
) -> std::result::Result<(StatusCode, Json<AppendAssistantRunEventResponse>), ApiError> {
    let run_id = parse_assistant_run_id(&run_id)?;
    validate_required("event_name", &request.event_name)?;
    let run = state
        .storage
        .assistant_runs()
        .get_by_id(state.tenant_id, run_id)
        .await
        .map_err(ApiError::from_storage)?
        .ok_or_else(|| {
            ApiError::not_found(
                "assistant_run_not_found",
                format!("assistant run {} was not found", run_id),
            )
        })?;
    let event = state
        .storage
        .assistant_runs()
        .append_event(
            state.tenant_id,
            run_id,
            &NewAssistantRunEvent {
                event_name: request.event_name.trim().to_string(),
                payload: request.payload,
                created_at: Utc::now(),
            },
        )
        .await
        .map_err(ApiError::from_storage)?;

    Ok((
        StatusCode::CREATED,
        Json(AppendAssistantRunEventResponse {
            run: to_assistant_run_view(run),
            event: to_assistant_run_event_view(event),
        }),
    ))
}

async fn continue_assistant_run(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
    Json(request): Json<ContinueAssistantRunRequest>,
) -> std::result::Result<(StatusCode, Json<ContinueAssistantRunResponse>), ApiError> {
    let run_id = parse_assistant_run_id(&run_id)?;
    let run = state
        .storage
        .assistant_runs()
        .get_by_id(state.tenant_id, run_id)
        .await
        .map_err(ApiError::from_storage)?
        .ok_or_else(|| {
            ApiError::not_found(
                "assistant_run_not_found",
                format!("assistant run {} was not found", run_id),
            )
        })?;

    let continue_prompt = request
        .prompt
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("继续执行当前 AssistantRun。")
        .to_string();
    let max_steps = normalize_assistant_run_continue_max_steps(request.max_steps);
    let runtime_mode =
        std::env::var("ASSISTANT_RUN_RUNTIME_MODE").unwrap_or_else(|_| "placeholder".to_string());
    let runtime_provider =
        std::env::var("ASSISTANT_RUN_RUNTIME_PROVIDER").unwrap_or_else(|_| runtime_mode.clone());
    let runtime_model = std::env::var("ASSISTANT_RUN_RUNTIME_MODEL")
        .unwrap_or_else(|_| DEFAULT_ASSISTANT_RUN_RUNTIME_MODEL.to_string());
    let runtime_mode_for_trail = runtime_mode.clone();
    let runtime_provider_for_trail = runtime_provider.clone();
    let runtime_model_for_trail = runtime_model.clone();
    let provider_input = if runtime_mode == "placeholder" {
        format!(
            "AssistantRun 继续执行占位回复：后端已接收继续指令，真实模型接入后会基于同一个运行上下文继续回答或请求平台能力。\n\nRun: {}\nContinue prompt: {}\nMax steps: {}\n供料状态: {}",
            run.id,
            continue_prompt,
            max_steps,
            assistant_run_evidence_status_label(&run.evidence_state)
        )
    } else {
        build_assistant_run_continue_provider_input(&run, &request, &continue_prompt, max_steps)
    };

    let response = tokio::task::spawn_blocking(move || {
        let provider = build_provider_from_env(
            "ASSISTANT_RUN",
            &runtime_mode,
            runtime_provider,
            bootstrap_default_prompt_registry(),
        )?;
        provider.complete(&LlmRequest {
            model: runtime_model,
            system_prompt_key: None,
            input: provider_input,
        })
    })
    .await
    .map_err(|error| {
        ApiError::internal(
            "assistant_run_continue_join_failed",
            format!("assistant run continue worker join failed: {error}"),
        )
    })?
    .map_err(|error| {
        ApiError::internal("assistant_run_continue_provider_failed", error.to_string())
    })?;

    let now = Utc::now();
    let runtime_manifest = render_runtime_manifest(&response.runtime);
    let mut execution_trail = value_array(run.execution_trail.clone());
    execution_trail.push(json!({
        "status": "completed",
        "label": "继续执行",
        "prompt": continue_prompt,
        "max_steps": max_steps,
        "runtime_mode": runtime_mode_for_trail,
        "provider": runtime_provider_for_trail,
        "model": runtime_model_for_trail,
        "at": now,
    }));
    let mut output_artifacts = value_array(run.output_artifacts.clone());
    output_artifacts.push(json!({
        "type": "assistant_message",
        "role": ChatMessageRole::Assistant.as_str(),
        "content": response.output_text,
        "source": "assistant_run_continue",
        "created_at": now,
    }));

    state
        .storage
        .assistant_runs()
        .update_execution_trail(
            state.tenant_id,
            run_id,
            &Value::Array(execution_trail.clone()),
        )
        .await
        .map_err(ApiError::from_storage)?;
    let updated_run = state
        .storage
        .assistant_runs()
        .attach_output_artifacts(
            state.tenant_id,
            run_id,
            &Value::Array(output_artifacts.clone()),
        )
        .await
        .map_err(ApiError::from_storage)?;
    let event = state
        .storage
        .assistant_runs()
        .append_event(
            state.tenant_id,
            run_id,
            &NewAssistantRunEvent {
                event_name: "assistant_run.continued".to_string(),
                payload: json!({
                    "prompt": continue_prompt,
                    "max_steps": max_steps,
                    "runtime": runtime_manifest.clone(),
                }),
                created_at: now,
            },
        )
        .await
        .map_err(ApiError::from_storage)?;
    let assistant_message = AssistantRunMessageView {
        role: ChatMessageRole::Assistant,
        content: output_artifacts
            .last()
            .and_then(|artifact| artifact.get("content"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    };
    let selected_scope = updated_run.selected_scope.clone();
    let evidence_state = updated_run.evidence_state.clone();
    let run_view = to_assistant_run_view(updated_run);

    Ok((
        StatusCode::CREATED,
        Json(ContinueAssistantRunResponse {
            run: run_view,
            assistant_message,
            runtime: runtime_manifest,
            event: to_assistant_run_event_view(event),
            selected_scope,
            evidence_state,
            execution_trail,
            output_artifacts,
            required_confirmations: Vec::new(),
        }),
    ))
}

async fn create_conversation_memory_item(
    State(state): State<AppState>,
    Json(request): Json<CreateConversationMemoryItemRequest>,
) -> std::result::Result<(StatusCode, Json<ConversationMemoryItemView>), ApiError> {
    validate_required("local_thread_id", &request.local_thread_id)?;
    validate_required("item_kind", &request.item_kind)?;
    validate_required("summary", &request.summary)?;
    let item = state
        .storage
        .conversation_memory_items()
        .create(
            state.tenant_id,
            &NewConversationMemoryItem {
                local_thread_id: request.local_thread_id.trim().to_string(),
                role: request.role,
                item_kind: request.item_kind.trim().to_string(),
                summary: request.summary.trim().to_string(),
                source_message_refs: request.source_message_refs,
                artifact_refs: request.artifact_refs,
                metadata: request.metadata,
                created_at: Utc::now(),
            },
        )
        .await
        .map_err(ApiError::from_storage)?;

    Ok((
        StatusCode::CREATED,
        Json(to_conversation_memory_item_view(item)),
    ))
}

async fn create_static_page_draft_for_assistant_run(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
    Json(request): Json<CreateStaticPageDraftRequest>,
) -> std::result::Result<(StatusCode, Json<CreateStaticPageDraftResponse>), ApiError> {
    let run_id = parse_assistant_run_id(&run_id)?;
    let run = state
        .storage
        .assistant_runs()
        .get_by_id(state.tenant_id, run_id)
        .await
        .map_err(ApiError::from_storage)?
        .ok_or_else(|| {
            ApiError::not_found(
                "assistant_run_not_found",
                format!("assistant run {} was not found", run_id),
            )
        })?;
    let prompt = request
        .prompt
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(run.user_prompt.trim());
    let title = request
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| derive_static_page_draft_title(prompt));
    let selected_scope = request
        .selected_scope
        .clone()
        .unwrap_or_else(|| run.selected_scope.clone());
    let visibility_snapshot = request
        .visibility_snapshot
        .clone()
        .unwrap_or_else(|| build_static_page_visibility_snapshot(&run, &selected_scope));
    let source_refs = if request.source_refs.is_null() {
        build_static_page_source_refs(&run)
    } else {
        request.source_refs.clone()
    };
    let draft_payload = if request.draft_payload.is_null() {
        build_initial_static_page_draft_payload(&run, prompt)
    } else {
        request.draft_payload.clone()
    };
    let status =
        status_from_static_page_payload(&draft_payload).unwrap_or(StaticPageDraftStatus::Draft);

    let draft = state
        .storage
        .static_page_drafts()
        .create(
            state.tenant_id,
            &NewStaticPageDraft {
                assistant_run_id: run.id,
                title,
                status,
                selected_scope,
                visibility_snapshot,
                source_refs,
                draft_payload,
                created_at: Utc::now(),
            },
        )
        .await
        .map_err(ApiError::from_storage)?;
    state
        .storage
        .assistant_runs()
        .append_event(
            state.tenant_id,
            run.id,
            &NewAssistantRunEvent {
                event_name: "static_page_draft.created".to_string(),
                payload: json!({
                    "draft_id": draft.id,
                    "title": draft.title,
                    "status": draft.status.as_str(),
                }),
                created_at: draft.created_at,
            },
        )
        .await
        .map_err(ApiError::from_storage)?;

    Ok((
        StatusCode::CREATED,
        Json(CreateStaticPageDraftResponse {
            draft: to_static_page_draft_view(draft),
        }),
    ))
}

async fn list_static_page_drafts(
    State(state): State<AppState>,
    Query(query): Query<StaticPageDraftListQuery>,
) -> std::result::Result<Json<Vec<StaticPageDraftView>>, ApiError> {
    let limit = normalize_static_page_draft_list_limit(query.limit);
    let drafts = if let Some(run_id) = query
        .assistant_run_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let run_id = parse_assistant_run_id(run_id)?;
        state
            .storage
            .static_page_drafts()
            .list_by_assistant_run(state.tenant_id, run_id)
            .await
            .map_err(ApiError::from_storage)?
            .into_iter()
            .take(limit as usize)
            .collect()
    } else {
        let local_thread_id = required_field("local_thread_id", query.local_thread_id)?;
        validate_required("local_thread_id", &local_thread_id)?;
        state
            .storage
            .static_page_drafts()
            .list_by_local_thread(state.tenant_id, local_thread_id.trim(), limit)
            .await
            .map_err(ApiError::from_storage)?
    };

    Ok(Json(
        drafts.into_iter().map(to_static_page_draft_view).collect(),
    ))
}

async fn get_static_page_draft(
    State(state): State<AppState>,
    Path(draft_id): Path<String>,
) -> std::result::Result<Json<StaticPageDraftView>, ApiError> {
    let draft_id = parse_static_page_draft_id(&draft_id)?;
    let draft = load_static_page_draft_or_404(&state, draft_id).await?;

    Ok(Json(to_static_page_draft_view(draft)))
}

async fn update_static_page_draft(
    State(state): State<AppState>,
    Path(draft_id): Path<String>,
    Json(request): Json<UpdateStaticPageDraftRequest>,
) -> std::result::Result<Json<UpdateStaticPageDraftResponse>, ApiError> {
    let draft_id = parse_static_page_draft_id(&draft_id)?;
    let mut draft = load_static_page_draft_or_404(&state, draft_id).await?;

    if let Some(title) = request.title {
        validate_required("title", &title)?;
        draft.title = title.trim().to_string();
    }
    if let Some(status) = request.status {
        draft.status = status.to_domain();
    }
    if let Some(selected_scope) = request.selected_scope {
        draft.selected_scope = selected_scope;
    }
    if let Some(visibility_snapshot) = request.visibility_snapshot {
        draft.visibility_snapshot = visibility_snapshot;
    }
    if let Some(source_refs) = request.source_refs {
        draft.source_refs = source_refs;
    }
    if let Some(draft_payload) = request.draft_payload {
        draft.status = status_from_static_page_payload(&draft_payload).unwrap_or(draft.status);
        draft.draft_payload = draft_payload;
    }

    let updated = state
        .storage
        .static_page_drafts()
        .update(state.tenant_id, &draft)
        .await
        .map_err(ApiError::from_storage)?;
    append_static_page_draft_run_event(
        &state,
        &updated,
        "static_page_draft.updated",
        json!({
            "draft_id": updated.id,
            "title": updated.title,
            "status": updated.status.as_str(),
        }),
    )
    .await?;

    Ok(Json(UpdateStaticPageDraftResponse {
        draft: to_static_page_draft_view(updated),
    }))
}

async fn append_static_page_draft_operations(
    State(state): State<AppState>,
    Path(draft_id): Path<String>,
    Json(request): Json<AppendStaticPageDraftOperationsRequest>,
) -> std::result::Result<(StatusCode, Json<AppendStaticPageDraftOperationsResponse>), ApiError> {
    let draft_id = parse_static_page_draft_id(&draft_id)?;
    let mut draft = load_static_page_draft_or_404(&state, draft_id).await?;
    let operations = validate_static_page_operations(request.operations)?;
    if operations.is_empty() && request.draft_payload.is_none() {
        return Err(ApiError::bad_request(
            "validation_error",
            "operations or draft_payload is required".to_string(),
        ));
    }

    let summary = request
        .summary
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| summarize_static_page_operations(&operations));
    let draft_payload = request.draft_payload.unwrap_or_else(|| {
        let mut payload = apply_static_page_operations_to_payload(
            draft.draft_payload.clone(),
            &operations,
            Some(&summary),
        );
        append_static_page_operations_metadata(
            &mut payload,
            &operations,
            request.prompt.as_deref(),
            &summary,
        );
        payload
    });
    draft.status = status_from_static_page_payload(&draft_payload)
        .or_else(|| status_from_static_page_operations(&operations))
        .unwrap_or(draft.status);
    draft.draft_payload = draft_payload;

    let updated = state
        .storage
        .static_page_drafts()
        .update(state.tenant_id, &draft)
        .await
        .map_err(ApiError::from_storage)?;
    append_static_page_draft_run_event(
        &state,
        &updated,
        "static_page_draft.operations_appended",
        json!({
            "draft_id": updated.id,
            "operation_count": operations.len(),
            "summary": summary,
        }),
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(AppendStaticPageDraftOperationsResponse {
            draft: to_static_page_draft_view(updated),
            operations,
            summary,
        }),
    ))
}

async fn apply_static_page_draft_intent(
    State(state): State<AppState>,
    Path(draft_id): Path<String>,
    Json(request): Json<ApplyStaticPageDraftIntentRequest>,
) -> std::result::Result<(StatusCode, Json<ApplyStaticPageDraftIntentResponse>), ApiError> {
    let draft_id = parse_static_page_draft_id(&draft_id)?;
    validate_required("prompt", &request.prompt)?;
    let mut draft = load_static_page_draft_or_404(&state, draft_id).await?;
    let prompt = request.prompt.trim().to_string();
    let message_count = request.messages.len();
    let messages = request.messages;
    let base_payload = request
        .draft_payload
        .unwrap_or_else(|| draft.draft_payload.clone());
    let intent = interpret_static_page_draft_intent_for_api(
        &state,
        &draft,
        &prompt,
        &base_payload,
        messages,
    )
    .await?;
    let mut draft_payload = apply_static_page_operations_to_payload(
        base_payload,
        &intent.operations,
        Some(&intent.summary),
    );
    append_static_page_operations_metadata(
        &mut draft_payload,
        &intent.operations,
        Some(&prompt),
        &intent.summary,
    );
    draft.status = status_from_static_page_payload(&draft_payload)
        .or_else(|| status_from_static_page_operations(&intent.operations))
        .unwrap_or(StaticPageDraftStatus::Planned);
    draft.draft_payload = draft_payload;

    let updated = state
        .storage
        .static_page_drafts()
        .update(state.tenant_id, &draft)
        .await
        .map_err(ApiError::from_storage)?;
    append_static_page_draft_run_event(
        &state,
        &updated,
        "static_page_draft.intent_applied",
        json!({
            "draft_id": updated.id,
            "prompt": prompt,
            "summary": intent.summary,
            "operation_count": intent.operations.len(),
            "message_count": message_count,
            "runtime": intent.runtime,
        }),
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(ApplyStaticPageDraftIntentResponse {
            draft: to_static_page_draft_view(updated),
            operations: intent.operations,
            summary: intent.summary,
        }),
    ))
}

async fn list_static_page_image_jobs(
    State(state): State<AppState>,
    Path(draft_id): Path<String>,
) -> std::result::Result<Json<Vec<StaticPageImageJobView>>, ApiError> {
    let draft_id = parse_static_page_draft_id(&draft_id)?;
    let draft = load_static_page_draft_or_404(&state, draft_id).await?;
    let jobs = state
        .storage
        .static_page_image_jobs()
        .list_by_draft(state.tenant_id, draft.id)
        .await
        .map_err(ApiError::from_storage)?;
    Ok(Json(
        jobs.into_iter()
            .map(to_static_page_image_job_view)
            .collect(),
    ))
}

async fn create_static_page_image_job(
    State(state): State<AppState>,
    Path(draft_id): Path<String>,
    Json(request): Json<CreateStaticPageImageJobRequest>,
) -> std::result::Result<(StatusCode, Json<CreateStaticPageImageJobResponse>), ApiError> {
    let draft_id = parse_static_page_draft_id(&draft_id)?;
    let mut draft = load_static_page_draft_or_404(&state, draft_id).await?;
    let image_prompt_payload = if request.image_prompt_payload.is_null() {
        build_static_page_image_prompt_payload(&draft, request.prompt.as_deref())
    } else {
        request.image_prompt_payload
    };
    let job = state
        .storage
        .static_page_image_jobs()
        .create(
            state.tenant_id,
            &NewStaticPageImageJob {
                draft_id: draft.id,
                assistant_run_id: draft.assistant_run_id,
                status: StaticPageImageJobStatus::Queued,
                queue_position: Some(1),
                image_prompt_payload,
                preview_asset_key: None,
                failure_reason: None,
                confirmed_at: None,
                created_at: Utc::now(),
            },
        )
        .await
        .map_err(ApiError::from_storage)?;
    let workflow_execution = build_initial_static_page_image_generation_execution(
        &state,
        &draft,
        &job,
        request.prompt.as_deref(),
    )?;
    let initial_event =
        build_initial_static_page_image_generation_event(&workflow_execution, &draft, &job);
    state
        .storage
        .workflow_executions()
        .create_with_initial_event(&workflow_execution, &initial_event)
        .await
        .map_err(ApiError::from_storage)?;
    let started = apply_workflow_signal_with_dependencies(
        &state.storage,
        &state.workflow_catalog,
        &state.event_bus,
        state.tenant_id,
        workflow_execution.id,
        WorkflowSignal::Start,
    )
    .await?;
    let operations = vec![json!({
        "type": "queue_image_job",
        "jobId": job.id,
        "queuePosition": job.queue_position,
        "queueMessage": "资源正在排队，可以联系商务开通高级用户跳过等待。",
    })];
    draft.draft_payload = apply_static_page_operations_to_payload(
        draft.draft_payload,
        &operations,
        Some("效果图任务已进入资源队列。"),
    );
    append_static_page_operations_metadata(
        &mut draft.draft_payload,
        &operations,
        request.prompt.as_deref(),
        "效果图任务已进入资源队列。",
    );
    draft.status = StaticPageDraftStatus::Queued;
    let draft = state
        .storage
        .static_page_drafts()
        .update(state.tenant_id, &draft)
        .await
        .map_err(ApiError::from_storage)?;
    append_static_page_draft_run_event(
        &state,
        &draft,
        "static_page_image_job.created",
        json!({
            "draft_id": draft.id,
            "image_job_id": job.id,
            "status": job.status.as_str(),
            "queue_position": job.queue_position,
            "workflow_execution_id": workflow_execution.id,
            "workflow_task_id": started.enqueued_tasks.first().map(|task| task.id),
        }),
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(CreateStaticPageImageJobResponse {
            image_job: to_static_page_image_job_view(job),
        }),
    ))
}

async fn get_static_page_image_job(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> std::result::Result<Json<StaticPageImageJobView>, ApiError> {
    let job_id = parse_static_page_image_job_id(&job_id)?;
    let job = load_static_page_image_job_or_404(&state, job_id).await?;
    Ok(Json(to_static_page_image_job_view(job)))
}

async fn confirm_static_page_image_job(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
    Json(request): Json<ConfirmStaticPageImageJobRequest>,
) -> std::result::Result<Json<ConfirmStaticPageImageJobResponse>, ApiError> {
    let job_id = parse_static_page_image_job_id(&job_id)?;
    let mut job = load_static_page_image_job_or_404(&state, job_id).await?;
    if matches!(job.status, StaticPageImageJobStatus::Failed) {
        return Err(ApiError::bad_request(
            "static_page_image_job_failed",
            "failed image jobs cannot be confirmed".to_string(),
        ));
    }
    let preview_asset_key = request
        .preview_asset_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| job.preview_asset_key.clone())
        .unwrap_or_else(|| format!("static-page-previews/{}.json", job.id));
    job.status = StaticPageImageJobStatus::Confirmed;
    job.queue_position = None;
    job.preview_asset_key = Some(preview_asset_key.clone());
    job.failure_reason = None;
    job.confirmed_at = Some(Utc::now());
    let job = state
        .storage
        .static_page_image_jobs()
        .update(state.tenant_id, &job)
        .await
        .map_err(ApiError::from_storage)?;
    let mut draft = load_static_page_draft_or_404(&state, job.draft_id).await?;
    let operations = vec![json!({
        "type": "confirm_preview",
        "previewImage": {
            "kind": "static-page-effect-preview",
            "assetKey": preview_asset_key,
            "imageJobId": job.id,
        }
    })];
    draft.draft_payload = apply_static_page_operations_to_payload(
        draft.draft_payload,
        &operations,
        Some("效果图已确认，可以进入最终静态页渲染。"),
    );
    append_static_page_operations_metadata(
        &mut draft.draft_payload,
        &operations,
        None,
        "效果图已确认，可以进入最终静态页渲染。",
    );
    draft.status = StaticPageDraftStatus::Confirmed;
    let draft = state
        .storage
        .static_page_drafts()
        .update(state.tenant_id, &draft)
        .await
        .map_err(ApiError::from_storage)?;
    append_static_page_draft_run_event(
        &state,
        &draft,
        "static_page_image_job.confirmed",
        json!({
            "draft_id": draft.id,
            "image_job_id": job.id,
            "preview_asset_key": job.preview_asset_key,
        }),
    )
    .await?;

    Ok(Json(ConfirmStaticPageImageJobResponse {
        image_job: to_static_page_image_job_view(job),
        draft: to_static_page_draft_view(draft),
    }))
}

async fn create_static_page_render(
    State(state): State<AppState>,
    Path(draft_id): Path<String>,
    Json(request): Json<CreateStaticPageRenderRequest>,
) -> std::result::Result<(StatusCode, Json<CreateStaticPageRenderResponse>), ApiError> {
    let draft_id = parse_static_page_draft_id(&draft_id)?;
    let mut draft = load_static_page_draft_or_404(&state, draft_id).await?;
    let image_job =
        resolve_confirmed_static_page_image_job(&state, &draft, request.image_job_id).await?;
    let rendered = render_static_page(&StaticPageRenderRequest {
        draft_id: draft.id.to_string(),
        assistant_run_id: draft.assistant_run_id.to_string(),
        title: draft.title.clone(),
        draft_payload: draft.draft_payload.clone(),
        selected_scope: draft.selected_scope.clone(),
        visibility_snapshot: draft.visibility_snapshot.clone(),
        preview_asset_key: image_job
            .as_ref()
            .and_then(|job| job.preview_asset_key.clone()),
        image_job_id: image_job.as_ref().map(|job| job.id.to_string()),
    });
    let render_output = state
        .storage
        .static_page_render_outputs()
        .create(
            state.tenant_id,
            &NewStaticPageRenderOutput {
                draft_id: draft.id,
                assistant_run_id: draft.assistant_run_id,
                image_job_id: image_job.as_ref().map(|job| job.id),
                status: StaticPageRenderOutputStatus::Rendered,
                html: rendered.html,
                asset_manifest: rendered.asset_manifest,
                created_at: Utc::now(),
            },
        )
        .await
        .map_err(ApiError::from_storage)?;
    let operations = vec![json!({
        "type": "request_final_render",
        "finalPage": {
            "status": "rendered",
            "renderOutputId": render_output.id,
            "assetManifest": render_output.asset_manifest,
        }
    })];
    draft.draft_payload = apply_static_page_operations_to_payload(
        draft.draft_payload,
        &operations,
        Some("最终静态页已根据确认效果图和模块规划生成。"),
    );
    append_static_page_operations_metadata(
        &mut draft.draft_payload,
        &operations,
        None,
        "最终静态页已根据确认效果图和模块规划生成。",
    );
    draft.status = StaticPageDraftStatus::Rendered;
    let draft = state
        .storage
        .static_page_drafts()
        .update(state.tenant_id, &draft)
        .await
        .map_err(ApiError::from_storage)?;
    append_static_page_draft_run_event(
        &state,
        &draft,
        "static_page_render.created",
        json!({
            "draft_id": draft.id,
            "render_output_id": render_output.id,
            "image_job_id": render_output.image_job_id,
        }),
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(CreateStaticPageRenderResponse {
            render_output: to_static_page_render_output_view(render_output),
            draft: to_static_page_draft_view(draft),
        }),
    ))
}

async fn list_static_page_render_outputs(
    State(state): State<AppState>,
    Path(draft_id): Path<String>,
) -> std::result::Result<Json<Vec<StaticPageRenderOutputView>>, ApiError> {
    let draft_id = parse_static_page_draft_id(&draft_id)?;
    let draft = load_static_page_draft_or_404(&state, draft_id).await?;
    let outputs = state
        .storage
        .static_page_render_outputs()
        .list_by_draft(state.tenant_id, draft.id)
        .await
        .map_err(ApiError::from_storage)?;
    Ok(Json(
        outputs
            .into_iter()
            .map(to_static_page_render_output_view)
            .collect(),
    ))
}

async fn list_conversation_memory_items(
    State(state): State<AppState>,
    Query(query): Query<ConversationMemoryQuery>,
) -> std::result::Result<Json<Vec<ConversationMemoryItemView>>, ApiError> {
    let local_thread_id = required_field("local_thread_id", query.local_thread_id)?;
    validate_required("local_thread_id", &local_thread_id)?;
    let items = state
        .storage
        .conversation_memory_items()
        .list_by_local_thread(
            state.tenant_id,
            local_thread_id.trim(),
            query.query.as_deref(),
            query.limit.unwrap_or(20),
        )
        .await
        .map_err(ApiError::from_storage)?;

    Ok(Json(
        items
            .into_iter()
            .map(to_conversation_memory_item_view)
            .collect(),
    ))
}

async fn list_assistant_run_conversation_memory_candidates(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
    Query(query): Query<ConversationMemoryQuery>,
) -> std::result::Result<Json<Vec<ConversationMemoryItemView>>, ApiError> {
    let run_id = parse_assistant_run_id(&run_id)?;
    let run = state
        .storage
        .assistant_runs()
        .get_by_id(state.tenant_id, run_id)
        .await
        .map_err(ApiError::from_storage)?
        .ok_or_else(|| {
            ApiError::not_found(
                "assistant_run_not_found",
                format!("assistant run {} was not found", run_id),
            )
        })?;
    let local_thread_id = run.local_thread_id.ok_or_else(|| {
        ApiError::bad_request(
            "assistant_run_local_thread_missing",
            "assistant run has no local thread id for conversation memory lookup".to_string(),
        )
    })?;
    let items = state
        .storage
        .conversation_memory_items()
        .list_by_local_thread(
            state.tenant_id,
            local_thread_id.trim(),
            query.query.as_deref(),
            query.limit.unwrap_or(20),
        )
        .await
        .map_err(ApiError::from_storage)?;

    Ok(Json(
        items
            .into_iter()
            .map(to_conversation_memory_item_view)
            .collect(),
    ))
}

#[cfg(test)]
fn build_assistant_run_provider_input(request: &CreateAssistantRunRequest) -> String {
    build_assistant_run_provider_input_with_evidence(request, None)
}

fn build_assistant_run_provider_input_with_evidence(
    request: &CreateAssistantRunRequest,
    evidence_state: Option<&Value>,
) -> String {
    let mut sections = vec![
        "你是智能数据工作台里的普通聊天运行时。".to_string(),
        "原则：不替用户编排答案；只根据用户问题、启动简报、范围候选和必要历史直接回答。"
            .to_string(),
    ];

    if let Some(briefing) = request.startup_briefing.as_ref() {
        sections.push(format!(
            "启动简报：{}",
            serde_json::to_string(briefing).unwrap_or_else(|_| "{}".to_string())
        ));
    }
    if !request.scope_candidates.is_empty() {
        sections.push(format!(
            "范围候选：{}",
            serde_json::to_string(&request.scope_candidates).unwrap_or_else(|_| "[]".to_string())
        ));
    }
    if let Some(selected_scope) = request.selected_scope.as_ref() {
        sections.push(format!(
            "当前选中范围：{}",
            serde_json::to_string(selected_scope).unwrap_or_else(|_| "{}".to_string())
        ));
    }
    if let Some(evidence_state) = evidence_state {
        if evidence_state.get("status").and_then(Value::as_str) != Some("not_requested") {
            sections.push(format!(
                "供料证据：{}",
                serde_json::to_string(evidence_state).unwrap_or_else(|_| "{}".to_string())
            ));
        }
    }
    if let Some(current_artifact) = request.current_artifact.as_ref() {
        sections.push(format!(
            "当前打开产物：{}",
            serde_json::to_string(current_artifact).unwrap_or_else(|_| "{}".to_string())
        ));
    }

    let history = request
        .messages
        .iter()
        .rev()
        .take(12)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|message| format!("{}: {}", message.role.as_str(), message.content.trim()))
        .collect::<Vec<_>>();
    if !history.is_empty() {
        sections.push(format!("最近对话：\n{}", history.join("\n")));
    }

    sections.push(format!("用户问题：{}", request.prompt.trim()));
    sections.join("\n\n")
}

fn build_assistant_run_continue_provider_input(
    run: &AssistantRun,
    request: &ContinueAssistantRunRequest,
    continue_prompt: &str,
    max_steps: usize,
) -> String {
    let mut sections = vec![
        "你是智能数据工作台里的 AssistantRun 连续执行运行时。".to_string(),
        "原则：不替用户编造系统动作；如果需要平台能力，只提出下一步动作需求；基于已有供料直接继续回答。"
            .to_string(),
        format!("运行ID：{}", run.id),
        format!("原始问题：{}", run.user_prompt.trim()),
        format!("继续指令：{}", continue_prompt.trim()),
        format!("本次最多连续动作数：{}", max_steps),
        format!(
            "当前选中范围：{}",
            serde_json::to_string(&run.selected_scope).unwrap_or_else(|_| "{}".to_string())
        ),
        format!(
            "供料状态：{}",
            serde_json::to_string(&run.evidence_state).unwrap_or_else(|_| "{}".to_string())
        ),
    ];

    if let Some(current_artifact) = request.current_artifact.as_ref() {
        sections.push(format!(
            "当前打开产物：{}",
            serde_json::to_string(current_artifact).unwrap_or_else(|_| "{}".to_string())
        ));
    }

    let history = request
        .messages
        .iter()
        .rev()
        .take(8)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|message| format!("{}: {}", message.role.as_str(), message.content.trim()))
        .collect::<Vec<_>>();
    if !history.is_empty() {
        sections.push(format!("继续前最近对话：\n{}", history.join("\n")));
    }

    sections.join("\n\n")
}

fn normalize_assistant_run_continue_max_steps(value: Option<usize>) -> usize {
    value
        .unwrap_or(ASSISTANT_RUN_CONTINUE_DEFAULT_MAX_STEPS)
        .clamp(1, ASSISTANT_RUN_CONTINUE_MAX_STEPS)
}

async fn build_assistant_run_evidence_state(
    state: &AppState,
    selected_scope: &Value,
    prompt: &str,
    local_thread_id: Option<&str>,
    active_secret_binding_ids: &[SecretBindingId],
) -> std::result::Result<Value, ApiError> {
    let dataset_ids = selected_dataset_ids_from_scope(selected_scope);
    let conversation_memory_requested = selected_scope_requests_conversation_memory(selected_scope);
    if dataset_ids.is_empty() && !conversation_memory_requested {
        return Ok(json!({
            "status": "not_requested",
            "policy": "host_supplies_model_answers",
            "supplied_items": [],
        }));
    }

    let limit = assistant_run_evidence_limit();
    let mut supplied_items = Vec::new();
    let mut supplied_datasets = Vec::new();
    let mut supplied_memory_items = Vec::new();

    for dataset_id in dataset_ids
        .into_iter()
        .take(ASSISTANT_RUN_EVIDENCE_DATASET_LIMIT)
    {
        let dataset = load_visible_dataset(state, dataset_id, active_secret_binding_ids).await?;
        supplied_datasets.push(json!({
            "id": dataset.id,
            "key": dataset.key.clone(),
            "title": dataset.title.clone(),
            "visibility": dataset.visibility.as_str(),
        }));

        let evidences = state
            .storage
            .retrieval_evidences()
            .list_latest_by_dataset(
                state.tenant_id,
                dataset.id,
                retrieval_search_scan_limit(limit),
            )
            .await
            .map_err(ApiError::from_storage)?;

        for ranked in rank_retrieval_evidences_for_prompt(&evidences, prompt, limit) {
            supplied_items.push(json!({
                "type": "retrieval_evidence",
                "dataset_id": ranked.evidence.dataset_id,
                "document_id": ranked.evidence.document_id,
                "document_chunk_id": ranked.evidence.document_chunk_id,
                "retrieval_evidence_id": ranked.evidence.id,
                "chunk_index": ranked.evidence.chunk_index,
                "source_locator": ranked.evidence.source_locator.clone(),
                "summary": ranked.evidence.summary.clone(),
                "content_excerpt": ranked.evidence.content_excerpt.clone(),
                "payload_filter_key": ranked.evidence.payload_filter_key.clone(),
                "score": ranked.score,
                "lexical_score": ranked.lexical_score,
                "recall_score": ranked.recall_score,
                "evidence_manifest": ranked.evidence.evidence_manifest.clone(),
            }));
        }
    }

    if conversation_memory_requested {
        if let Some(local_thread_id) = local_thread_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let items = state
                .storage
                .conversation_memory_items()
                .list_by_local_thread(
                    state.tenant_id,
                    local_thread_id,
                    None,
                    assistant_run_conversation_memory_limit(),
                )
                .await
                .map_err(ApiError::from_storage)?;

            for item in items
                .into_iter()
                .filter(assistant_run_memory_item_is_supply_eligible)
            {
                let supplied_item = json!({
                    "type": "conversation_memory_item",
                    "conversation_memory_item_id": item.id,
                    "local_thread_id": item.local_thread_id,
                    "role": item.role.as_str(),
                    "item_kind": item.item_kind,
                    "summary": item.summary,
                    "source_message_refs": item.source_message_refs,
                    "artifact_refs": item.artifact_refs,
                    "metadata": item.metadata,
                    "created_at": item.created_at,
                    "updated_at": item.updated_at,
                });
                supplied_memory_items.push(supplied_item.clone());
                supplied_items.push(supplied_item);
            }
        }
    }

    let status = if supplied_items.is_empty() {
        "empty"
    } else {
        "supplied"
    };
    Ok(json!({
        "status": status,
        "policy": "host_supplies_model_answers",
        "selected_scope": selected_scope,
        "datasets": supplied_datasets,
        "conversation_memory_items": supplied_memory_items,
        "supplied_items": supplied_items,
        "limit": limit,
    }))
}

fn assistant_run_evidence_limit() -> usize {
    std::env::var("ASSISTANT_RUN_EVIDENCE_LIMIT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(ASSISTANT_RUN_EVIDENCE_DEFAULT_LIMIT)
        .clamp(1, ASSISTANT_RUN_EVIDENCE_MAX_LIMIT)
}

fn assistant_run_conversation_memory_limit() -> i64 {
    std::env::var("ASSISTANT_RUN_CONVERSATION_MEMORY_LIMIT")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(ASSISTANT_RUN_CONVERSATION_MEMORY_DEFAULT_LIMIT)
        .clamp(1, ASSISTANT_RUN_CONVERSATION_MEMORY_MAX_LIMIT)
}

fn assistant_run_memory_item_is_supply_eligible(item: &ConversationMemoryItem) -> bool {
    item.role == ChatMessageRole::User
        && !matches!(
            item.item_kind.as_str(),
            "assistant_output" | "artifact_output" | "generated_artifact"
        )
}

fn assistant_run_evidence_supplied_count(evidence_state: &Value) -> usize {
    evidence_state
        .get("supplied_items")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0)
}

fn assistant_run_evidence_status_label(evidence_state: &Value) -> String {
    let status = evidence_state
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    match status {
        "supplied" => format!(
            "已检索 {} 条供料项",
            assistant_run_evidence_supplied_count(evidence_state)
        ),
        "empty" => "已请求供料，但暂未检索到可用内容".to_string(),
        "not_requested" => "未请求数据集供料".to_string(),
        other => other.to_string(),
    }
}

async fn list_chat_messages(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> std::result::Result<Json<Vec<ChatMessageView>>, ApiError> {
    let session_id = parse_chat_session_id(&session_id)?;
    let session = state
        .storage
        .chat_sessions()
        .get_by_id(state.tenant_id, session_id)
        .await
        .map_err(ApiError::from_storage)?;
    if session.is_none() {
        return Err(ApiError::not_found(
            "chat_session_not_found",
            format!("chat session {} was not found", session_id),
        ));
    }

    let messages = state
        .storage
        .chat_messages()
        .list_by_session(state.tenant_id, session_id)
        .await
        .map_err(ApiError::from_storage)?;

    let mut views = Vec::with_capacity(messages.len());
    for message in messages {
        views.push(hydrate_chat_message_view(&state, message).await?);
    }

    Ok(Json(views))
}

async fn update_chat_session_report_entry_route(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(request): Json<UpdateChatSessionReportEntryRequest>,
) -> std::result::Result<(StatusCode, Json<UpdateChatSessionReportEntryResponse>), ApiError> {
    let session_id = parse_chat_session_id(&session_id)?;
    let response =
        apply_chat_session_report_entry_update_with_state(&state, session_id, request).await?;

    Ok((StatusCode::OK, Json(response)))
}

async fn apply_chat_session_report_entry_update_with_state(
    state: &AppState,
    session_id: ChatSessionId,
    request: UpdateChatSessionReportEntryRequest,
) -> std::result::Result<UpdateChatSessionReportEntryResponse, ApiError> {
    let session = state
        .storage
        .chat_sessions()
        .get_by_id(state.tenant_id, session_id)
        .await
        .map_err(ApiError::from_storage)?
        .ok_or_else(|| {
            ApiError::not_found(
                "chat_session_not_found",
                format!("chat session {} was not found", session_id),
            )
        })?;

    let mut session_manifest = session.session_manifest.clone();
    let current_report_entry = parse_chat_session_manifest(&session.session_manifest)
        .and_then(|manifest| manifest.report_entry);
    let now = Utc::now();
    let plan = plan_chat_session_report_entry_update(
        &session,
        current_report_entry.as_ref(),
        &request,
        now,
    )?;

    match plan {
        ChatSessionReportEntryUpdatePlan::RequestConfirmation(entry) => {
            write_chat_session_report_entry(
                &mut session_manifest,
                contracts::ModelFacingReportEntryStateView::ConfirmationRequired,
                &entry,
                None,
                None,
                None,
            )?;

            let updated_session = state
                .storage
                .chat_sessions()
                .update_context(
                    state.tenant_id,
                    session.id,
                    session.latest_memory_directory_id,
                    session.latest_dataset_output_id,
                    &session_manifest,
                    now,
                )
                .await
                .map_err(ApiError::from_storage)?;

            Ok(UpdateChatSessionReportEntryResponse {
                chat_session: hydrate_chat_session_view(state, updated_session).await?,
                report_plan: None,
                workflow_execution: None,
            })
        }
        ChatSessionReportEntryUpdatePlan::StayMaterialService(entry) => {
            write_chat_session_report_entry(
                &mut session_manifest,
                contracts::ModelFacingReportEntryStateView::NotApplicable,
                &entry,
                Some(now),
                Some(contracts::ChatSessionReportEntryResolutionView::StayMaterialService),
                None,
            )?;

            let updated_session = state
                .storage
                .chat_sessions()
                .update_context(
                    state.tenant_id,
                    session.id,
                    session.latest_memory_directory_id,
                    session.latest_dataset_output_id,
                    &session_manifest,
                    now,
                )
                .await
                .map_err(ApiError::from_storage)?;

            Ok(UpdateChatSessionReportEntryResponse {
                chat_session: hydrate_chat_session_view(state, updated_session).await?,
                report_plan: None,
                workflow_execution: None,
            })
        }
        ChatSessionReportEntryUpdatePlan::EnterReportService(entry) => {
            let report_service_handoff = confirmed_report_entry_service_handoff(&entry, now);
            let (plan, execution) = create_report_plan_and_execution(
                state,
                session.dataset_id,
                &entry.title,
                &entry.objective,
                Some(report_service_handoff),
            )
            .await?;
            write_chat_session_report_entry(
                &mut session_manifest,
                contracts::ModelFacingReportEntryStateView::Confirmed,
                &entry,
                Some(now),
                Some(contracts::ChatSessionReportEntryResolutionView::EnterReportService),
                Some(plan.id),
            )?;

            let updated_session = state
                .storage
                .chat_sessions()
                .update_context(
                    state.tenant_id,
                    session.id,
                    session.latest_memory_directory_id,
                    session.latest_dataset_output_id,
                    &session_manifest,
                    now,
                )
                .await
                .map_err(ApiError::from_storage)?;

            Ok(UpdateChatSessionReportEntryResponse {
                chat_session: hydrate_chat_session_view(state, updated_session).await?,
                report_plan: Some(to_report_plan_summary(
                    plan,
                    workflow_execution_context_service_handoff(&execution),
                )),
                workflow_execution: Some(to_workflow_execution_view(execution)),
            })
        }
    }
}

async fn list_documents(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> std::result::Result<Json<Vec<DocumentSummary>>, ApiError> {
    let active_secret_binding_ids = active_secret_binding_ids_from_headers(&headers)?;
    let visible_dataset_ids: HashSet<DatasetId> = filter_visible_datasets(
        state
            .storage
            .datasets()
            .list_by_tenant(state.tenant_id)
            .await
            .map_err(ApiError::from_storage)?,
        &active_secret_binding_ids,
    )
    .into_iter()
    .map(|dataset| dataset.id)
    .collect();
    let documents = state
        .storage
        .documents()
        .list_by_tenant(state.tenant_id)
        .await
        .map_err(ApiError::from_storage)?;

    Ok(Json(
        documents
            .into_iter()
            .filter(|document| visible_dataset_ids.contains(&document.dataset_id))
            .map(to_document_summary)
            .collect(),
    ))
}

async fn get_document_detail(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(document_id): Path<String>,
) -> std::result::Result<Json<DocumentDetailView>, ApiError> {
    let document_id = parse_document_id(&document_id)?;
    let active_secret_binding_ids = active_secret_binding_ids_from_headers(&headers)?;
    let detail =
        load_document_detail_with_state(&state, document_id, &active_secret_binding_ids).await?;
    Ok(Json(detail))
}

async fn compare_documents_route(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CompareDocumentsRequest>,
) -> std::result::Result<Json<CompareDocumentsView>, ApiError> {
    let active_secret_binding_ids = active_secret_binding_ids_from_headers(&headers)?;
    let comparison =
        compare_documents_with_state(&state, request, &active_secret_binding_ids).await?;
    Ok(Json(comparison))
}

async fn list_document_chunks(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(document_id): Path<String>,
) -> std::result::Result<Json<Vec<DocumentChunkView>>, ApiError> {
    let document_id = parse_document_id(&document_id)?;
    let active_secret_binding_ids = active_secret_binding_ids_from_headers(&headers)?;
    load_visible_document(&state, document_id, &active_secret_binding_ids).await?;

    let chunks = state
        .storage
        .document_chunks()
        .list_by_document(state.tenant_id, document_id)
        .await
        .map_err(ApiError::from_storage)?;

    Ok(Json(
        chunks.into_iter().map(to_document_chunk_view).collect(),
    ))
}

async fn list_document_retrieval_evidences(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(document_id): Path<String>,
) -> std::result::Result<Json<Vec<RetrievalEvidenceView>>, ApiError> {
    let document_id = parse_document_id(&document_id)?;
    let active_secret_binding_ids = active_secret_binding_ids_from_headers(&headers)?;
    load_visible_document(&state, document_id, &active_secret_binding_ids).await?;

    let mut evidences = state
        .storage
        .retrieval_evidences()
        .list_by_document(state.tenant_id, document_id)
        .await
        .map_err(ApiError::from_storage)?;
    sort_retrieval_evidences_by_relevance(&mut evidences);

    Ok(Json(
        evidences
            .into_iter()
            .map(to_retrieval_evidence_view)
            .collect(),
    ))
}

async fn load_document_detail_with_state(
    state: &AppState,
    document_id: DocumentId,
    active_secret_binding_ids: &[SecretBindingId],
) -> std::result::Result<DocumentDetailView, ApiError> {
    let document = load_visible_document(state, document_id, active_secret_binding_ids).await?;
    let chunks = state
        .storage
        .document_chunks()
        .list_by_document(state.tenant_id, document_id)
        .await
        .map_err(ApiError::from_storage)?;
    let mut retrieval_evidences = state
        .storage
        .retrieval_evidences()
        .list_by_document(state.tenant_id, document_id)
        .await
        .map_err(ApiError::from_storage)?;
    sort_retrieval_evidences_by_relevance(&mut retrieval_evidences);

    Ok(to_document_detail_view(
        document,
        chunks,
        retrieval_evidences,
    ))
}

async fn compare_documents_with_state(
    state: &AppState,
    request: CompareDocumentsRequest,
    active_secret_binding_ids: &[SecretBindingId],
) -> std::result::Result<CompareDocumentsView, ApiError> {
    let mut unique_document_ids = Vec::new();
    for document_id in request.document_ids {
        if !unique_document_ids
            .iter()
            .any(|existing| *existing == document_id)
        {
            unique_document_ids.push(document_id);
        }
    }

    if unique_document_ids.len() < 2 {
        return Err(ApiError::bad_request(
            "compare_documents_requires_multiple_documents",
            "compare_documents requires at least 2 distinct document_ids".to_string(),
        ));
    }

    let mut expected_dataset: Option<(DocumentId, DatasetId)> = None;
    for document_id in &unique_document_ids {
        let document =
            load_visible_document(state, *document_id, active_secret_binding_ids).await?;

        if let Some((expected_document_id, expected_dataset_id)) = expected_dataset {
            if document.dataset_id != expected_dataset_id {
                return Err(ApiError::bad_request(
                    "compare_documents_requires_same_dataset",
                    format!(
                        "document {} belongs to dataset {}, which does not match document {} in dataset {}",
                        document.id, document.dataset_id, expected_document_id, expected_dataset_id
                    ),
                ));
            }
        } else {
            expected_dataset = Some((document.id, document.dataset_id));
        }
    }

    let mut documents = Vec::with_capacity(unique_document_ids.len());
    for document_id in unique_document_ids {
        documents.push(
            load_document_detail_with_state(state, document_id, active_secret_binding_ids).await?,
        );
    }

    Ok(to_compare_documents_view(documents))
}

async fn register_document(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RegisterDocumentRequest>,
) -> std::result::Result<(StatusCode, Json<RegisterDocumentResponse>), ApiError> {
    validate_required("title", &request.title)?;
    validate_required("object_key", &request.object_key)?;
    validate_required("content_type", &request.content_type)?;

    let active_secret_binding_ids = merge_secret_binding_ids(
        &active_secret_binding_ids_from_headers(&headers)?,
        &request.secret_binding_ids,
    );
    load_visible_dataset(&state, request.dataset_id, &active_secret_binding_ids).await?;

    let document = state
        .storage
        .documents()
        .create(
            state.tenant_id,
            NewDocument {
                dataset_id: request.dataset_id,
                title: request.title.trim().to_string(),
                object_key: request.object_key.trim().to_string(),
                content_type: request.content_type.trim().to_string(),
                secret_binding_ids: request.secret_binding_ids,
                metadata: request.metadata,
            },
        )
        .await
        .map_err(ApiError::from_storage)?;

    Ok((
        StatusCode::CREATED,
        Json(RegisterDocumentResponse {
            document: to_document_summary(document),
        }),
    ))
}

async fn create_document_ingest(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(document_id): Path<String>,
) -> std::result::Result<(StatusCode, Json<CreateDocumentIngestResponse>), ApiError> {
    let document_id = parse_document_id(&document_id)?;
    let active_secret_binding_ids = active_secret_binding_ids_from_headers(&headers)?;
    let document = load_visible_document(&state, document_id, &active_secret_binding_ids).await?;

    let execution = build_initial_upload_ingest_execution(&state, &document)?;
    let initial_event = build_initial_upload_ingest_event(&execution, &document);
    state
        .storage
        .workflow_executions()
        .create_with_initial_event(&execution, &initial_event)
        .await
        .map_err(ApiError::from_storage)?;

    Ok((
        StatusCode::CREATED,
        Json(CreateDocumentIngestResponse {
            document: to_document_summary(document),
            workflow_execution: to_workflow_execution_view(execution),
        }),
    ))
}

async fn create_report_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PlanReportRequest>,
) -> std::result::Result<(StatusCode, Json<CreateReportPlanResponse>), ApiError> {
    validate_required("title", &request.title)?;
    validate_required("objective", &request.objective)?;

    let active_secret_binding_ids = active_secret_binding_ids_from_headers(&headers)?;
    load_visible_dataset(&state, request.dataset_id, &active_secret_binding_ids).await?;

    let (plan, execution) = create_report_plan_and_execution(
        &state,
        request.dataset_id,
        request.title.trim(),
        request.objective.trim(),
        None,
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(CreateReportPlanResponse {
            plan: to_report_plan_summary(
                plan,
                workflow_execution_context_service_handoff(&execution),
            ),
            workflow_execution: to_workflow_execution_view(execution),
        }),
    ))
}

async fn list_report_plans(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> std::result::Result<Json<Vec<ReportPlanSummary>>, ApiError> {
    let active_secret_binding_ids = active_secret_binding_ids_from_headers(&headers)?;
    let visible_dataset_ids: HashSet<DatasetId> = filter_visible_datasets(
        state
            .storage
            .datasets()
            .list_by_tenant(state.tenant_id)
            .await
            .map_err(ApiError::from_storage)?,
        &active_secret_binding_ids,
    )
    .into_iter()
    .map(|dataset| dataset.id)
    .collect();
    let plans = state
        .storage
        .report_plans()
        .list_by_tenant(state.tenant_id)
        .await
        .map_err(ApiError::from_storage)?;

    let mut views = Vec::with_capacity(plans.len());
    for plan in plans {
        if !visible_dataset_ids.contains(&plan.dataset_id) {
            continue;
        }
        views.push(hydrate_report_plan_summary(&state, plan).await?);
    }

    Ok(Json(views))
}

async fn list_report_plan_ast_versions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(plan_id): Path<String>,
) -> std::result::Result<Json<Vec<ReportPlanAstVersionView>>, ApiError> {
    let plan_id = parse_plan_id(&plan_id)?;
    let active_secret_binding_ids = active_secret_binding_ids_from_headers(&headers)?;
    load_visible_report_plan(&state, plan_id, &active_secret_binding_ids).await?;

    let versions = state
        .storage
        .report_plan_ast_versions()
        .list_by_plan(state.tenant_id, plan_id)
        .await
        .map_err(ApiError::from_storage)?;

    Ok(Json(
        versions
            .into_iter()
            .map(to_report_plan_ast_version_view)
            .collect(),
    ))
}

async fn continue_report_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(plan_id): Path<String>,
) -> std::result::Result<Json<CreateReportPlanResponse>, ApiError> {
    let plan_id = parse_plan_id(&plan_id)?;
    let active_secret_binding_ids = active_secret_binding_ids_from_headers(&headers)?;
    let response =
        continue_report_plan_response(&state, plan_id, &active_secret_binding_ids).await?;
    Ok(Json(response))
}

async fn create_report_plan_and_execution(
    state: &AppState,
    dataset_id: DatasetId,
    title: &str,
    objective: &str,
    service_handoff: Option<contracts::ManifestServiceHandoffView>,
) -> std::result::Result<(ReportPlan, WorkflowExecution), ApiError> {
    let plan = state
        .storage
        .report_plans()
        .create(
            state.tenant_id,
            NewReportPlan {
                dataset_id,
                title: title.trim().to_string(),
                objective: objective.trim().to_string(),
                theme_key: "default-local".to_string(),
            },
        )
        .await
        .map_err(ApiError::from_storage)?;

    let execution = build_initial_report_plan_execution(
        state,
        &plan,
        finalize_report_service_handoff(service_handoff, plan.id).as_ref(),
    )?;
    let initial_event = build_initial_execution_event(&execution, plan.id);
    state
        .storage
        .workflow_executions()
        .create_with_initial_event(&execution, &initial_event)
        .await
        .map_err(ApiError::from_storage)?;

    Ok((plan, execution))
}

async fn continue_report_plan_response(
    state: &AppState,
    plan_id: ReportPlanId,
    active_secret_binding_ids: &[SecretBindingId],
) -> std::result::Result<CreateReportPlanResponse, ApiError> {
    let plan = load_visible_report_plan(state, plan_id, active_secret_binding_ids).await?;
    if plan.status != domain_model::ReportPlanStatus::Draft || plan.current_ast_version_id.is_some()
    {
        return Err(ApiError::bad_request(
            "report_plan_continue_not_allowed",
            format!("report plan {} is not a draft planning candidate", plan_id),
        ));
    }

    let latest_execution = state
        .storage
        .workflow_executions()
        .get_latest_by_report_plan_and_kind(state.tenant_id, plan_id, WorkflowKind::ReportPlan)
        .await
        .map_err(ApiError::from_storage)?;
    if let Some(execution) = latest_execution {
        if matches!(
            execution.status,
            WorkflowStatus::Pending | WorkflowStatus::Running
        ) {
            return Err(ApiError::bad_request(
                "report_plan_execution_in_progress",
                format!(
                    "report plan {} already has an active planning execution {}",
                    plan_id, execution.id
                ),
            ));
        }
    }

    let service_handoff = load_report_plan_service_handoff(state, plan_id).await?;
    let execution = build_initial_report_plan_execution(state, &plan, service_handoff.as_ref())?;
    let initial_event = build_initial_execution_event(&execution, plan.id);
    state
        .storage
        .workflow_executions()
        .create_with_initial_event(&execution, &initial_event)
        .await
        .map_err(ApiError::from_storage)?;

    Ok(CreateReportPlanResponse {
        plan: hydrate_report_plan_summary(state, plan).await?,
        workflow_execution: to_workflow_execution_view(execution),
    })
}

async fn publish_report(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(plan_id): Path<String>,
    Json(request): Json<PublishReportRequest>,
) -> std::result::Result<(StatusCode, Json<PublishReportResponse>), ApiError> {
    let plan_id = parse_plan_id(&plan_id)?;
    let active_secret_binding_ids = active_secret_binding_ids_from_headers(&headers)?;
    let response =
        publish_report_response(&state, plan_id, request, &active_secret_binding_ids).await?;

    Ok((StatusCode::CREATED, Json(response)))
}

async fn publish_report_response(
    state: &AppState,
    plan_id: ReportPlanId,
    request: PublishReportRequest,
    active_secret_binding_ids: &[SecretBindingId],
) -> std::result::Result<PublishReportResponse, ApiError> {
    let plan = load_visible_report_plan(state, plan_id, active_secret_binding_ids).await?;
    let render_output = state
        .storage
        .report_render_outputs()
        .get_latest_rendered_for_surface(state.tenant_id, plan_id, request.surface.clone())
        .await
        .map_err(ApiError::from_storage)?
        .ok_or_else(|| {
            ApiError::bad_request(
                "report_render_output_not_found",
                format!(
                    "report plan {} does not have a rendered output for surface {}",
                    plan_id,
                    request.surface.as_str()
                ),
            )
        })?;
    let source_render_output =
        hydrate_report_render_output_view(state, render_output.clone()).await?;
    if !report_render_output_has_asset_path(&source_render_output) {
        return Err(ApiError::bad_request(
            "report_render_output_not_publishable",
            format!(
                "report render output {} does not expose a publishable asset path",
                render_output.id
            ),
        ));
    }

    let published_at = Utc::now();
    let report = match state
        .storage
        .published_reports()
        .get_by_plan(state.tenant_id, plan_id)
        .await
        .map_err(ApiError::from_storage)?
    {
        Some(report) => report,
        None => state
            .storage
            .published_reports()
            .create(
                state.tenant_id,
                &NewPublishedReport {
                    dataset_id: plan.dataset_id,
                    plan_id,
                    slug: build_published_report_slug(&plan),
                    created_at: published_at,
                    updated_at: published_at,
                },
            )
            .await
            .map_err(ApiError::from_storage)?,
    };
    let version = state
        .storage
        .published_report_versions()
        .create_next_version(
            state.tenant_id,
            report.id,
            &NewPublishedReportVersion {
                surface: request.surface,
                asset_manifest: build_published_report_version_manifest(
                    &render_output,
                    request.publish_note.as_deref(),
                    published_at,
                ),
                created_at: published_at,
            },
        )
        .await
        .map_err(ApiError::from_storage)?;
    let report = state
        .storage
        .published_reports()
        .set_current_version(state.tenant_id, report.id, version.id, published_at)
        .await
        .map_err(ApiError::from_storage)?;
    state
        .storage
        .report_plans()
        .mark_published(state.tenant_id, plan_id, published_at)
        .await
        .map_err(ApiError::from_storage)?;

    Ok(PublishReportResponse {
        report: to_published_report_view(report),
        version: to_published_report_version_view(version),
        source_render_output,
    })
}

async fn create_report_render(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(plan_id): Path<String>,
    Json(request): Json<CreateReportRenderRequest>,
) -> std::result::Result<(StatusCode, Json<CreateReportRenderResponse>), ApiError> {
    let plan_id = parse_plan_id(&plan_id)?;
    let active_secret_binding_ids = active_secret_binding_ids_from_headers(&headers)?;
    let response =
        create_report_render_response(&state, plan_id, request, &active_secret_binding_ids).await?;

    Ok((StatusCode::CREATED, Json(response)))
}

async fn create_report_render_response(
    state: &AppState,
    plan_id: ReportPlanId,
    request: CreateReportRenderRequest,
    active_secret_binding_ids: &[SecretBindingId],
) -> std::result::Result<CreateReportRenderResponse, ApiError> {
    let surface = request.surface;
    let plan = load_visible_report_plan(state, plan_id, active_secret_binding_ids).await?;
    let ast_version_id = plan.current_ast_version_id.ok_or_else(|| {
        ApiError::bad_request(
            "report_plan_not_planned",
            format!(
                "report plan {} does not have a current AST version",
                plan_id
            ),
        )
    })?;

    let report_service_handoff = load_report_plan_service_handoff(&state, plan.id).await?;
    let execution = build_initial_report_render_execution(
        &state,
        &plan,
        ast_version_id,
        surface.clone(),
        report_service_handoff.as_ref(),
    )?;
    let initial_event = build_initial_render_execution_event(&execution, ast_version_id, &surface);
    state
        .storage
        .workflow_executions()
        .create_with_initial_event(&execution, &initial_event)
        .await
        .map_err(ApiError::from_storage)?;

    Ok(CreateReportRenderResponse {
        workflow_execution: to_workflow_execution_view(execution),
        requested_ast_version_id: ast_version_id,
        surface,
    })
}

async fn list_report_render_outputs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(plan_id): Path<String>,
) -> std::result::Result<Json<Vec<ReportRenderOutputView>>, ApiError> {
    let plan_id = parse_plan_id(&plan_id)?;
    let active_secret_binding_ids = active_secret_binding_ids_from_headers(&headers)?;
    load_visible_report_plan(&state, plan_id, &active_secret_binding_ids).await?;

    let outputs = state
        .storage
        .report_render_outputs()
        .list_by_plan(state.tenant_id, plan_id)
        .await
        .map_err(ApiError::from_storage)?;

    let mut views = Vec::with_capacity(outputs.len());
    for output in outputs {
        views.push(hydrate_report_render_output_view(&state, output).await?);
    }

    Ok(Json(views))
}

async fn get_report_plan_published_report(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(plan_id): Path<String>,
) -> std::result::Result<Json<PublishedReportDetailView>, ApiError> {
    let plan_id = parse_plan_id(&plan_id)?;
    let active_secret_binding_ids = active_secret_binding_ids_from_headers(&headers)?;
    let detail = load_published_report_detail_by_plan_with_state(
        &state,
        plan_id,
        &active_secret_binding_ids,
    )
    .await?;
    Ok(Json(detail))
}

async fn list_published_reports(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> std::result::Result<Json<Vec<PublishedReportView>>, ApiError> {
    let active_secret_binding_ids = active_secret_binding_ids_from_headers(&headers)?;
    let visible_dataset_ids: HashSet<DatasetId> = filter_visible_datasets(
        state
            .storage
            .datasets()
            .list_by_tenant(state.tenant_id)
            .await
            .map_err(ApiError::from_storage)?,
        &active_secret_binding_ids,
    )
    .into_iter()
    .map(|dataset| dataset.id)
    .collect();
    let reports = state
        .storage
        .published_reports()
        .list_by_tenant(state.tenant_id)
        .await
        .map_err(ApiError::from_storage)?;

    Ok(Json(
        reports
            .into_iter()
            .filter(|report| visible_dataset_ids.contains(&report.dataset_id))
            .map(to_published_report_view)
            .collect(),
    ))
}

async fn get_published_report(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(report_id): Path<String>,
) -> std::result::Result<Json<PublishedReportDetailView>, ApiError> {
    let report_id = parse_published_report_id(&report_id)?;
    let active_secret_binding_ids = active_secret_binding_ids_from_headers(&headers)?;
    let detail =
        load_published_report_detail_with_state(&state, report_id, &active_secret_binding_ids)
            .await?;
    Ok(Json(detail))
}

async fn list_workflow_executions(
    State(state): State<AppState>,
) -> std::result::Result<Json<Vec<WorkflowExecutionView>>, ApiError> {
    let executions = state
        .storage
        .workflow_executions()
        .list_by_tenant(state.tenant_id)
        .await
        .map_err(ApiError::from_storage)?;

    Ok(Json(
        executions
            .into_iter()
            .map(to_workflow_execution_view)
            .collect(),
    ))
}

async fn get_workflow_execution(
    State(state): State<AppState>,
    Path(execution_id): Path<String>,
) -> std::result::Result<Json<WorkflowExecutionView>, ApiError> {
    let execution_id = parse_execution_id(&execution_id)?;
    let execution = state
        .storage
        .workflow_executions()
        .get_by_id(state.tenant_id, execution_id)
        .await
        .map_err(ApiError::from_storage)?;

    let execution = execution.ok_or_else(|| {
        ApiError::not_found(
            "workflow_execution_not_found",
            format!("workflow execution {} was not found", execution_id),
        )
    })?;

    Ok(Json(to_workflow_execution_view(execution)))
}

async fn list_workflow_events(
    State(state): State<AppState>,
    Path(execution_id): Path<String>,
) -> std::result::Result<Json<Vec<WorkflowEventView>>, ApiError> {
    let execution_id = parse_execution_id(&execution_id)?;
    let execution_exists = state
        .storage
        .workflow_executions()
        .get_by_id(state.tenant_id, execution_id)
        .await
        .map_err(ApiError::from_storage)?;
    if execution_exists.is_none() {
        return Err(ApiError::not_found(
            "workflow_execution_not_found",
            format!("workflow execution {} was not found", execution_id),
        ));
    }

    let events = state
        .storage
        .workflow_events()
        .list_by_execution(execution_id)
        .await
        .map_err(ApiError::from_storage)?;

    Ok(Json(
        events
            .into_iter()
            .map(|event| WorkflowEventView {
                id: event.id,
                sequence_no: event.sequence_no,
                event_name: event.event_name,
                payload: event.payload,
                created_at: event.created_at,
            })
            .collect(),
    ))
}

async fn list_workflow_tasks(
    State(state): State<AppState>,
    Path(execution_id): Path<String>,
) -> std::result::Result<Json<Vec<WorkflowTaskView>>, ApiError> {
    let execution_id = parse_execution_id(&execution_id)?;
    let execution_exists = state
        .storage
        .workflow_executions()
        .get_by_id(state.tenant_id, execution_id)
        .await
        .map_err(ApiError::from_storage)?;
    if execution_exists.is_none() {
        return Err(ApiError::not_found(
            "workflow_execution_not_found",
            format!("workflow execution {} was not found", execution_id),
        ));
    }

    let tasks = state
        .storage
        .workflow_tasks()
        .list_by_execution(execution_id)
        .await
        .map_err(ApiError::from_storage)?;

    Ok(Json(tasks.into_iter().map(to_workflow_task_view).collect()))
}

async fn get_workflow_runtime_inspect(
    State(state): State<AppState>,
    Path(execution_id): Path<String>,
) -> std::result::Result<Json<WorkflowRuntimeInspectView>, ApiError> {
    let execution_id = parse_execution_id(&execution_id)?;
    let view = load_workflow_runtime_inspect_view(&state, execution_id).await?;

    Ok(Json(view))
}

async fn load_workflow_runtime_inspect_view(
    state: &AppState,
    execution_id: WorkflowExecutionId,
) -> std::result::Result<WorkflowRuntimeInspectView, ApiError> {
    let execution = state
        .storage
        .workflow_executions()
        .get_by_id(state.tenant_id, execution_id)
        .await
        .map_err(ApiError::from_storage)?
        .ok_or_else(|| {
            ApiError::not_found(
                "workflow_execution_not_found",
                format!("workflow execution {} was not found", execution_id),
            )
        })?;

    let dataset_output = match state
        .storage
        .dataset_outputs()
        .get_by_execution_id(state.tenant_id, execution_id)
        .await
        .map_err(ApiError::from_storage)?
    {
        Some(output) => Some(hydrate_dataset_output_view(&state, output).await?),
        None => None,
    };
    let chat_session_record = state
        .storage
        .chat_sessions()
        .get_by_execution_id(state.tenant_id, execution_id)
        .await
        .map_err(ApiError::from_storage)?;
    let (chat_session, chat_messages) = match chat_session_record {
        Some(session) => {
            let chat_messages = load_chat_message_views_for_session(state, session.id).await?;
            let latest_assistant_message = latest_assistant_message_from_views(&chat_messages);
            let chat_session = hydrate_chat_session_view_with_latest_assistant_message(
                state,
                session,
                latest_assistant_message,
            )
            .await?;
            (Some(chat_session), chat_messages)
        }
        None => (None, Vec::new()),
    };
    let report_plan = match execution.report_plan_id {
        Some(report_plan_id) => match state
            .storage
            .report_plans()
            .get_by_id(state.tenant_id, report_plan_id)
            .await
            .map_err(ApiError::from_storage)?
        {
            Some(plan) => Some(hydrate_report_plan_summary(state, plan).await?),
            None => None,
        },
        None => None,
    };
    let report_render_output = match state
        .storage
        .report_render_outputs()
        .get_by_execution_id(state.tenant_id, execution_id)
        .await
        .map_err(ApiError::from_storage)?
    {
        Some(output) => Some(hydrate_report_render_output_view(state, output).await?),
        None => None,
    };
    let llm_invocations: Vec<LlmInvocationView> = state
        .storage
        .llm_invocations()
        .list_by_execution(state.tenant_id, execution_id)
        .await
        .map_err(ApiError::from_storage)?
        .into_iter()
        .map(to_llm_invocation_view)
        .collect();
    let tool_executions: Vec<ToolExecutionView> = state
        .storage
        .tool_executions()
        .list_by_execution(state.tenant_id, execution_id)
        .await
        .map_err(ApiError::from_storage)?
        .into_iter()
        .map(to_tool_execution_view)
        .collect();
    let execution_scope_runtime: Option<contracts::WorkflowExecutionRuntimeSummaryView> =
        summarize_execution_scope_runtime(&llm_invocations, &tool_executions);

    let mut inspect = WorkflowRuntimeInspectView {
        execution: to_workflow_execution_view(execution),
        execution_scope_runtime,
        dataset_output,
        chat_session,
        report_plan,
        report_render_output,
        chat_messages,
        llm_invocations,
        tool_executions,
        model_facing: None,
        pretty_summaries: Vec::new(),
    };
    inspect.model_facing = Some(derive_model_facing_summary(&inspect));
    inspect.pretty_summaries = render_workflow_runtime_pretty_summaries(&inspect);

    Ok(inspect)
}

async fn list_llm_invocations(
    State(state): State<AppState>,
    Path(execution_id): Path<String>,
) -> std::result::Result<Json<Vec<LlmInvocationView>>, ApiError> {
    let execution_id = parse_execution_id(&execution_id)?;
    let execution_exists = state
        .storage
        .workflow_executions()
        .get_by_id(state.tenant_id, execution_id)
        .await
        .map_err(ApiError::from_storage)?;
    if execution_exists.is_none() {
        return Err(ApiError::not_found(
            "workflow_execution_not_found",
            format!("workflow execution {} was not found", execution_id),
        ));
    }

    let llm_invocations = state
        .storage
        .llm_invocations()
        .list_by_execution(state.tenant_id, execution_id)
        .await
        .map_err(ApiError::from_storage)?;

    Ok(Json(
        llm_invocations
            .into_iter()
            .map(to_llm_invocation_view)
            .collect(),
    ))
}

async fn list_tool_executions(
    State(state): State<AppState>,
    Path(execution_id): Path<String>,
) -> std::result::Result<Json<Vec<ToolExecutionView>>, ApiError> {
    let execution_id = parse_execution_id(&execution_id)?;
    let execution_exists = state
        .storage
        .workflow_executions()
        .get_by_id(state.tenant_id, execution_id)
        .await
        .map_err(ApiError::from_storage)?;
    if execution_exists.is_none() {
        return Err(ApiError::not_found(
            "workflow_execution_not_found",
            format!("workflow execution {} was not found", execution_id),
        ));
    }

    let tool_executions = state
        .storage
        .tool_executions()
        .list_by_execution(state.tenant_id, execution_id)
        .await
        .map_err(ApiError::from_storage)?;

    Ok(Json(
        tool_executions
            .into_iter()
            .map(to_tool_execution_view)
            .collect(),
    ))
}

async fn start_workflow_execution(
    State(state): State<AppState>,
    Path(execution_id): Path<String>,
) -> std::result::Result<Json<AdvanceWorkflowExecutionResponse>, ApiError> {
    let execution_id = parse_execution_id(&execution_id)?;
    let response = apply_workflow_signal(&state, execution_id, WorkflowSignal::Start).await?;

    Ok(Json(response))
}

async fn retry_workflow_execution_route(
    State(state): State<AppState>,
    Path(execution_id): Path<String>,
    Json(request): Json<RetryWorkflowExecutionRequest>,
) -> std::result::Result<Json<RetryWorkflowExecutionResponse>, ApiError> {
    let execution_id = parse_execution_id(&execution_id)?;
    let response = retry_workflow_execution_with_state(&state, execution_id, request).await?;

    Ok(Json(response))
}

async fn send_workflow_signal(
    State(state): State<AppState>,
    Path(execution_id): Path<String>,
    Json(request): Json<WorkflowSignalRequest>,
) -> std::result::Result<Json<AdvanceWorkflowExecutionResponse>, ApiError> {
    let execution_id = parse_execution_id(&execution_id)?;
    let signal = build_workflow_signal(request)?;
    let response = apply_workflow_signal(&state, execution_id, signal).await?;

    Ok(Json(response))
}

async fn retry_workflow_execution_with_state(
    state: &AppState,
    execution_id: WorkflowExecutionId,
    request: RetryWorkflowExecutionRequest,
) -> std::result::Result<RetryWorkflowExecutionResponse, ApiError> {
    validate_required("reason", &request.reason)?;

    let retry_transition = apply_workflow_signal(
        state,
        execution_id,
        WorkflowSignal::RetryRequested {
            reason: request.reason.trim().to_string(),
        },
    )
    .await?;
    let restart_transition = if retry_transition.execution.status == WorkflowStatus::Pending {
        Some(apply_workflow_signal(state, execution_id, WorkflowSignal::Start).await?)
    } else {
        None
    };

    Ok(RetryWorkflowExecutionResponse {
        retry_transition,
        restart_transition,
    })
}

async fn apply_workflow_signal(
    state: &AppState,
    execution_id: WorkflowExecutionId,
    signal: WorkflowSignal,
) -> std::result::Result<AdvanceWorkflowExecutionResponse, ApiError> {
    apply_workflow_signal_with_dependencies(
        &state.storage,
        &state.workflow_catalog,
        &state.event_bus,
        state.tenant_id,
        execution_id,
        signal,
    )
    .await
}

pub async fn apply_workflow_signal_with_dependencies(
    storage: &PgStorage,
    workflow_catalog: &WorkflowCatalog,
    event_bus: &EventBus,
    tenant_id: TenantId,
    execution_id: WorkflowExecutionId,
    signal: WorkflowSignal,
) -> std::result::Result<AdvanceWorkflowExecutionResponse, ApiError> {
    let execution = storage
        .workflow_executions()
        .get_by_id(tenant_id, execution_id)
        .await
        .map_err(ApiError::from_storage)?
        .ok_or_else(|| {
            ApiError::not_found(
                "workflow_execution_not_found",
                format!("workflow execution {} was not found", execution_id),
            )
        })?;

    let definition = workflow_catalog
        .find_definition(execution.kind.clone())
        .ok_or_else(|| {
            ApiError::internal(
                "workflow_definition_missing",
                format!(
                    "workflow definition {} is not registered",
                    execution.kind.as_str()
                ),
            )
        })?;
    let now = Utc::now();
    let runtime_state = workflow_runtime_state_from_execution(&execution)?;
    let transition = definition
        .transition(&runtime_state, signal, now)
        .map_err(ApiError::from_transition)?;
    let workflow_engine::WorkflowTransition {
        next_state,
        persisted_event,
        enqueued_tasks,
    } = transition;
    let next_execution = workflow_execution_from_transition(&execution, next_state)?;
    let pending_tasks: Vec<NewWorkflowTask> = enqueued_tasks
        .into_iter()
        .map(|task| NewWorkflowTask {
            queue: task.queue,
            task_key: task.task_key,
            payload: task.payload,
            available_at: persisted_event.occurred_at,
            max_attempts: 3,
        })
        .collect();
    let (persisted_event, persisted_tasks) = storage
        .workflow_executions()
        .advance_with_tasks(
            &next_execution,
            &persisted_event.name,
            &persisted_event.detail,
            persisted_event.occurred_at,
            &pending_tasks,
        )
        .await
        .map_err(ApiError::from_storage)?;
    publish_workflow_transition_events(
        event_bus,
        &next_execution,
        &persisted_event,
        &persisted_tasks,
    )
    .await;

    Ok(AdvanceWorkflowExecutionResponse {
        execution: to_workflow_execution_view(next_execution),
        persisted_event: to_workflow_event_view(persisted_event),
        enqueued_tasks: persisted_tasks
            .into_iter()
            .map(to_workflow_task_view)
            .collect(),
    })
}

async fn publish_workflow_transition_events(
    event_bus: &EventBus,
    execution: &WorkflowExecution,
    persisted_event: &WorkflowEventRecord,
    persisted_tasks: &[WorkflowTask],
) {
    let execution_event = EventEnvelope {
        subject: workflow_execution_transition_subject(execution.kind.as_str()),
        payload: json!({
            "execution_id": execution.id,
            "tenant_id": execution.tenant_id,
            "kind": execution.kind.as_str(),
            "status": execution.status.as_str(),
            "stage": execution.stage,
            "event_name": persisted_event.event_name,
            "event_sequence_no": persisted_event.sequence_no,
        }),
        published_at: persisted_event.created_at,
    };
    event_bus.publish(execution_event).await;

    for task in persisted_tasks {
        let task_event = EventEnvelope {
            subject: workflow_task_enqueued_subject(&task.queue, &task.task_key),
            payload: json!({
                "task_id": task.id,
                "tenant_id": task.tenant_id,
                "execution_id": task.execution_id,
                "queue": task.queue,
                "task_key": task.task_key,
                "status": task.status.as_str(),
                "available_at": task.available_at,
            }),
            published_at: task.created_at,
        };
        event_bus.publish(task_event).await;
    }
}

fn build_initial_report_plan_execution(
    state: &AppState,
    plan: &ReportPlan,
    service_handoff: Option<&contracts::ManifestServiceHandoffView>,
) -> std::result::Result<WorkflowExecution, ApiError> {
    let definition = state
        .workflow_catalog
        .find_definition(WorkflowKind::ReportPlan)
        .ok_or_else(|| {
            ApiError::internal(
                "workflow_definition_missing",
                "report_plan workflow definition is not registered".to_string(),
            )
        })?;
    let now = Utc::now();
    let execution_id = WorkflowExecutionId::new();
    let runtime_state = definition.initial_state(execution_id, now);
    let mut context = runtime_state.context;
    context.insert(
        "retries_remaining".to_string(),
        Value::Number(runtime_state.retries_remaining.into()),
    );
    if let Some(service_handoff) = service_handoff {
        context.insert("service_handoff".to_string(), json!(service_handoff));
    }

    Ok(WorkflowExecution {
        id: execution_id,
        tenant_id: state.tenant_id,
        dataset_id: Some(plan.dataset_id),
        report_plan_id: Some(plan.id),
        kind: WorkflowKind::ReportPlan,
        version: runtime_state.version,
        stage: runtime_state.stage,
        status: runtime_state.status,
        attempt: 0,
        context: Value::Object(context),
        created_at: now,
        updated_at: now,
    })
}

fn build_initial_memory_directory_execution(
    state: &AppState,
    dataset_id: DatasetId,
) -> std::result::Result<WorkflowExecution, ApiError> {
    let definition = state
        .workflow_catalog
        .find_definition(WorkflowKind::MemoryDirectory)
        .ok_or_else(|| {
            ApiError::internal(
                "workflow_definition_missing",
                "memory_directory workflow definition is not registered".to_string(),
            )
        })?;
    let now = Utc::now();
    let execution_id = WorkflowExecutionId::new();
    let runtime_state = definition.initial_state(execution_id, now);
    let mut context = runtime_state.context;
    context.insert(
        "retries_remaining".to_string(),
        Value::Number(runtime_state.retries_remaining.into()),
    );
    context.insert("include_directory".to_string(), Value::Bool(true));

    Ok(WorkflowExecution {
        id: execution_id,
        tenant_id: state.tenant_id,
        dataset_id: Some(dataset_id),
        report_plan_id: None,
        kind: WorkflowKind::MemoryDirectory,
        version: runtime_state.version,
        stage: runtime_state.stage,
        status: runtime_state.status,
        attempt: 0,
        context: Value::Object(context),
        created_at: now,
        updated_at: now,
    })
}

fn build_initial_dataset_output_execution(
    state: &AppState,
    dataset_id: DatasetId,
    prompt: &str,
    chat_session_id: Option<ChatSessionId>,
    memory_directory: Option<&MemoryDirectory>,
    retrieval_evidence_ids: &[domain_model::RetrievalEvidenceId],
) -> std::result::Result<WorkflowExecution, ApiError> {
    let definition = state
        .workflow_catalog
        .find_definition(WorkflowKind::DatasetOutput)
        .ok_or_else(|| {
            ApiError::internal(
                "workflow_definition_missing",
                "dataset_output workflow definition is not registered".to_string(),
            )
        })?;
    let now = Utc::now();
    let execution_id = WorkflowExecutionId::new();
    let runtime_state = definition.initial_state(execution_id, now);
    let mut context = runtime_state.context;
    context.insert(
        "retries_remaining".to_string(),
        Value::Number(runtime_state.retries_remaining.into()),
    );
    context.insert(
        "prompt".to_string(),
        Value::String(prompt.trim().to_string()),
    );
    if let Some(chat_session_id) = chat_session_id {
        context.insert(
            "chat_session_id".to_string(),
            Value::String(chat_session_id.to_string()),
        );
    }
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
    if !retrieval_evidence_ids.is_empty() {
        context.insert(
            "retrieval_evidence_ids".to_string(),
            Value::Array(
                retrieval_evidence_ids
                    .iter()
                    .map(|id| Value::String(id.to_string()))
                    .collect(),
            ),
        );
    }

    Ok(WorkflowExecution {
        id: execution_id,
        tenant_id: state.tenant_id,
        dataset_id: Some(dataset_id),
        report_plan_id: None,
        kind: WorkflowKind::DatasetOutput,
        version: runtime_state.version,
        stage: runtime_state.stage,
        status: runtime_state.status,
        attempt: 0,
        context: Value::Object(context),
        created_at: now,
        updated_at: now,
    })
}

fn build_initial_chat_session_execution(
    state: &AppState,
    dataset_id: DatasetId,
    chat_session_id: ChatSessionId,
    prompt: &str,
    memory_directory: Option<&MemoryDirectory>,
    dataset_output_id: Option<domain_model::DatasetOutputId>,
) -> std::result::Result<WorkflowExecution, ApiError> {
    let definition = state
        .workflow_catalog
        .find_definition(WorkflowKind::ChatSession)
        .ok_or_else(|| {
            ApiError::internal(
                "workflow_definition_missing",
                "chat_session workflow definition is not registered".to_string(),
            )
        })?;
    let now = Utc::now();
    let execution_id = WorkflowExecutionId::new();
    let runtime_state = definition.initial_state(execution_id, now);
    let mut context = runtime_state.context;
    context.insert(
        "retries_remaining".to_string(),
        Value::Number(runtime_state.retries_remaining.into()),
    );
    context.insert(
        "chat_session_id".to_string(),
        Value::String(chat_session_id.to_string()),
    );
    context.insert(
        "chat_turn_id".to_string(),
        Value::String(Uuid::new_v4().to_string()),
    );
    context.insert(
        "prompt".to_string(),
        Value::String(prompt.trim().to_string()),
    );
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
    if let Some(dataset_output_id) = dataset_output_id {
        context.insert(
            "dataset_output_id".to_string(),
            Value::String(dataset_output_id.to_string()),
        );
    }

    Ok(WorkflowExecution {
        id: execution_id,
        tenant_id: state.tenant_id,
        dataset_id: Some(dataset_id),
        report_plan_id: None,
        kind: WorkflowKind::ChatSession,
        version: runtime_state.version,
        stage: runtime_state.stage,
        status: runtime_state.status,
        attempt: 0,
        context: Value::Object(context),
        created_at: now,
        updated_at: now,
    })
}

fn build_initial_upload_ingest_execution(
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

fn build_initial_report_render_execution(
    state: &AppState,
    plan: &ReportPlan,
    ast_version_id: domain_model::ReportPlanAstVersionId,
    surface: PublishedSurface,
    service_handoff: Option<&contracts::ManifestServiceHandoffView>,
) -> std::result::Result<WorkflowExecution, ApiError> {
    let definition = state
        .workflow_catalog
        .find_definition(WorkflowKind::ReportRender)
        .ok_or_else(|| {
            ApiError::internal(
                "workflow_definition_missing",
                "report_render workflow definition is not registered".to_string(),
            )
        })?;
    let now = Utc::now();
    let execution_id = WorkflowExecutionId::new();
    let runtime_state = definition.initial_state(execution_id, now);
    let mut context = runtime_state.context;
    context.insert(
        "retries_remaining".to_string(),
        Value::Number(runtime_state.retries_remaining.into()),
    );
    context.insert(
        "surface".to_string(),
        Value::String(surface.as_str().to_string()),
    );
    context.insert(
        "report_plan_ast_version_id".to_string(),
        Value::String(ast_version_id.to_string()),
    );
    context.insert(
        "theme_key".to_string(),
        Value::String(plan.theme_key.clone()),
    );
    if let Some(service_handoff) = service_handoff {
        context.insert("service_handoff".to_string(), json!(service_handoff));
    }

    Ok(WorkflowExecution {
        id: execution_id,
        tenant_id: state.tenant_id,
        dataset_id: Some(plan.dataset_id),
        report_plan_id: Some(plan.id),
        kind: WorkflowKind::ReportRender,
        version: runtime_state.version,
        stage: runtime_state.stage,
        status: runtime_state.status,
        attempt: 0,
        context: Value::Object(context),
        created_at: now,
        updated_at: now,
    })
}

fn build_initial_static_page_image_generation_execution(
    state: &AppState,
    draft: &StaticPageDraft,
    job: &StaticPageImageJob,
    prompt: Option<&str>,
) -> std::result::Result<WorkflowExecution, ApiError> {
    let definition = state
        .workflow_catalog
        .find_definition(WorkflowKind::StaticPageImageGeneration)
        .ok_or_else(|| {
            ApiError::internal(
                "workflow_definition_missing",
                "static_page_image_generation workflow definition is not registered".to_string(),
            )
        })?;
    let now = Utc::now();
    let execution_id = WorkflowExecutionId::new();
    let runtime_state = definition.initial_state(execution_id, now);
    let mut context = runtime_state.context;
    context.insert(
        "retries_remaining".to_string(),
        Value::Number(runtime_state.retries_remaining.into()),
    );
    context.insert(
        "static_page_draft_id".to_string(),
        Value::String(draft.id.to_string()),
    );
    context.insert(
        "static_page_image_job_id".to_string(),
        Value::String(job.id.to_string()),
    );
    context.insert(
        "assistant_run_id".to_string(),
        Value::String(draft.assistant_run_id.to_string()),
    );
    if let Some(prompt) = prompt.map(str::trim).filter(|value| !value.is_empty()) {
        context.insert("prompt".to_string(), Value::String(prompt.to_string()));
    }

    Ok(WorkflowExecution {
        id: execution_id,
        tenant_id: state.tenant_id,
        dataset_id: None,
        report_plan_id: None,
        kind: WorkflowKind::StaticPageImageGeneration,
        version: runtime_state.version,
        stage: runtime_state.stage,
        status: runtime_state.status,
        attempt: 0,
        context: Value::Object(context),
        created_at: now,
        updated_at: now,
    })
}

fn build_initial_execution_event(
    execution: &WorkflowExecution,
    report_plan_id: domain_model::ReportPlanId,
) -> WorkflowEventRecord {
    WorkflowEventRecord {
        id: domain_model::WorkflowEventId::new(),
        execution_id: execution.id,
        sequence_no: 1,
        event_name: "workflow.execution_created".to_string(),
        payload: json!({
            "kind": execution.kind.as_str(),
            "version": execution.version,
            "status": execution.status.as_str(),
            "stage": execution.stage,
            "report_plan_id": report_plan_id,
        }),
        created_at: execution.created_at,
    }
}

fn build_initial_static_page_image_generation_event(
    execution: &WorkflowExecution,
    draft: &StaticPageDraft,
    job: &StaticPageImageJob,
) -> WorkflowEventRecord {
    WorkflowEventRecord {
        id: domain_model::WorkflowEventId::new(),
        execution_id: execution.id,
        sequence_no: 1,
        event_name: "workflow.execution_created".to_string(),
        payload: json!({
            "kind": execution.kind.as_str(),
            "version": execution.version,
            "status": execution.status.as_str(),
            "stage": execution.stage,
            "assistant_run_id": draft.assistant_run_id,
            "static_page_draft_id": draft.id,
            "static_page_image_job_id": job.id,
        }),
        created_at: execution.created_at,
    }
}

fn build_initial_memory_directory_event(execution: &WorkflowExecution) -> WorkflowEventRecord {
    WorkflowEventRecord {
        id: domain_model::WorkflowEventId::new(),
        execution_id: execution.id,
        sequence_no: 1,
        event_name: "workflow.execution_created".to_string(),
        payload: json!({
            "kind": execution.kind.as_str(),
            "version": execution.version,
            "status": execution.status.as_str(),
            "stage": execution.stage,
            "dataset_id": execution.dataset_id,
            "include_directory": execution
                .context
                .get("include_directory")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        }),
        created_at: execution.created_at,
    }
}

fn build_initial_dataset_output_event(
    execution: &WorkflowExecution,
    chat_session_id: Option<ChatSessionId>,
    prompt: &str,
) -> WorkflowEventRecord {
    WorkflowEventRecord {
        id: domain_model::WorkflowEventId::new(),
        execution_id: execution.id,
        sequence_no: 1,
        event_name: "workflow.execution_created".to_string(),
        payload: json!({
            "kind": execution.kind.as_str(),
            "version": execution.version,
            "status": execution.status.as_str(),
            "stage": execution.stage,
            "dataset_id": execution.dataset_id,
            "chat_session_id": chat_session_id,
            "prompt": prompt.trim(),
        }),
        created_at: execution.created_at,
    }
}

fn build_initial_chat_session_event(
    execution: &WorkflowExecution,
    chat_session_id: ChatSessionId,
    prompt: &str,
) -> WorkflowEventRecord {
    WorkflowEventRecord {
        id: domain_model::WorkflowEventId::new(),
        execution_id: execution.id,
        sequence_no: 1,
        event_name: "workflow.execution_created".to_string(),
        payload: json!({
            "kind": execution.kind.as_str(),
            "version": execution.version,
            "status": execution.status.as_str(),
            "stage": execution.stage,
            "dataset_id": execution.dataset_id,
            "chat_session_id": chat_session_id,
            "prompt": prompt.trim(),
        }),
        created_at: execution.created_at,
    }
}

fn chat_session_has_in_progress_turn(session_manifest: &Value) -> bool {
    let status = session_manifest
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if status != "pending_assistant_reply" {
        return false;
    }

    let turn_status = session_manifest
        .get("last_turn")
        .and_then(|turn| turn.get("status"))
        .and_then(Value::as_str);
    !matches!(turn_status, Some("completed" | "failed"))
}

fn build_pending_chat_session_turn_manifest(
    current_manifest: &Value,
    execution: &WorkflowExecution,
    prompt: &str,
    latest_memory_directory: Option<&MemoryDirectory>,
    latest_dataset_output: Option<&DatasetOutput>,
    chat_turn_id: &str,
) -> Value {
    let mut manifest = current_manifest.as_object().cloned().unwrap_or_default();
    manifest.insert(
        "generator".to_string(),
        Value::String("chat-session-workflow".to_string()),
    );
    manifest.insert(
        "schema_version".to_string(),
        Value::String("0.3.0".to_string()),
    );
    manifest.insert(
        "status".to_string(),
        Value::String("pending_assistant_reply".to_string()),
    );
    manifest
        .entry("initial_prompt".to_string())
        .or_insert_with(|| Value::String(prompt.trim().to_string()));
    manifest.insert(
        "last_prompt".to_string(),
        Value::String(prompt.trim().to_string()),
    );
    manifest.insert(
        "last_turn_kind".to_string(),
        Value::String("placeholder_orchestration".to_string()),
    );
    manifest
        .entry("context_binding".to_string())
        .or_insert_with(|| Value::String("creation_time".to_string()));
    manifest.insert(
        "latest_memory_directory_id".to_string(),
        json!(latest_memory_directory.map(|entry| entry.id)),
    );
    manifest.insert(
        "latest_memory_directory_version_no".to_string(),
        json!(latest_memory_directory.map(|entry| entry.version_no)),
    );
    manifest.insert(
        "latest_dataset_output_id".to_string(),
        json!(latest_dataset_output.map(|entry| entry.id)),
    );
    manifest.insert(
        "last_turn".to_string(),
        json!({
            "turn_id": chat_turn_id,
            "status": "pending",
            "stream_mode": "buffered",
            "provider_request_id": Value::Null,
            "provider_status": "pending",
            "finish_reason": Value::Null,
            "assistant_message_id": Value::Null,
            "tool_trace_count": 0,
            "events": [{
                "kind": "turn_started",
                "at": execution.created_at,
                "provider_request_id": Value::Null,
                "finish_reason": Value::Null,
                "tool_trace_count": Value::Null,
            }],
            "started_at": execution.created_at,
            "completed_at": Value::Null,
        }),
    );

    Value::Object(manifest)
}

fn build_initial_upload_ingest_event(
    execution: &WorkflowExecution,
    document: &Document,
) -> WorkflowEventRecord {
    WorkflowEventRecord {
        id: domain_model::WorkflowEventId::new(),
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

fn build_initial_render_execution_event(
    execution: &WorkflowExecution,
    ast_version_id: domain_model::ReportPlanAstVersionId,
    surface: &PublishedSurface,
) -> WorkflowEventRecord {
    WorkflowEventRecord {
        id: domain_model::WorkflowEventId::new(),
        execution_id: execution.id,
        sequence_no: 1,
        event_name: "workflow.execution_created".to_string(),
        payload: json!({
            "kind": execution.kind.as_str(),
            "version": execution.version,
            "status": execution.status.as_str(),
            "stage": execution.stage,
            "report_plan_id": execution.report_plan_id,
            "report_plan_ast_version_id": ast_version_id,
            "surface": surface.as_str(),
        }),
        created_at: execution.created_at,
    }
}

fn parse_execution_id(raw: &str) -> std::result::Result<WorkflowExecutionId, ApiError> {
    Uuid::parse_str(raw).map(WorkflowExecutionId).map_err(|_| {
        ApiError::bad_request("invalid_execution_id", format!("{raw} is not a valid UUID"))
    })
}

fn parse_dataset_id(raw: &str) -> std::result::Result<DatasetId, ApiError> {
    Uuid::parse_str(raw).map(DatasetId).map_err(|_| {
        ApiError::bad_request("invalid_dataset_id", format!("{raw} is not a valid UUID"))
    })
}

fn parse_chat_session_id(raw: &str) -> std::result::Result<ChatSessionId, ApiError> {
    Uuid::parse_str(raw).map(ChatSessionId).map_err(|_| {
        ApiError::bad_request(
            "invalid_chat_session_id",
            format!("{raw} is not a valid UUID"),
        )
    })
}

fn selected_dataset_id_from_scope(scope: &Value) -> Option<DatasetId> {
    selected_dataset_ids_from_scope(scope).into_iter().next()
}

fn selected_dataset_ids_from_scope(scope: &Value) -> Vec<DatasetId> {
    let Some(object) = scope.as_object() else {
        return Vec::new();
    };

    let mut dataset_ids = Vec::new();
    for key in ["datasets", "selected"] {
        let Some(items) = object.get(key).and_then(Value::as_array) else {
            continue;
        };
        for item in items {
            let Some(dataset_id) = dataset_id_from_scope_item(item) else {
                continue;
            };
            if !dataset_ids.contains(&dataset_id) {
                dataset_ids.push(dataset_id);
            }
        }
    }
    dataset_ids
}

fn selected_scope_requests_conversation_memory(scope: &Value) -> bool {
    scope
        .as_object()
        .and_then(|object| object.get("conversation_memory"))
        .and_then(Value::as_array)
        .map(|items| {
            items.iter().any(|item| {
                item.as_str()
                    .map(str::trim)
                    .map(|value| !value.is_empty())
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn dataset_id_from_scope_item(item: &Value) -> Option<DatasetId> {
    let raw = item.as_str().or_else(|| {
        item.as_object()
            .and_then(|object| object.get("id"))
            .and_then(Value::as_str)
    })?;
    Uuid::parse_str(raw).ok().map(DatasetId)
}

fn parse_assistant_run_id(raw: &str) -> std::result::Result<AssistantRunId, ApiError> {
    Uuid::parse_str(raw).map(AssistantRunId).map_err(|_| {
        ApiError::bad_request(
            "invalid_assistant_run_id",
            format!("{raw} is not a valid UUID"),
        )
    })
}

fn parse_static_page_draft_id(raw: &str) -> std::result::Result<StaticPageDraftId, ApiError> {
    Uuid::parse_str(raw).map(StaticPageDraftId).map_err(|_| {
        ApiError::bad_request(
            "invalid_static_page_draft_id",
            format!("{raw} is not a valid UUID"),
        )
    })
}

fn parse_static_page_image_job_id(
    raw: &str,
) -> std::result::Result<StaticPageImageJobId, ApiError> {
    Uuid::parse_str(raw).map(StaticPageImageJobId).map_err(|_| {
        ApiError::bad_request(
            "invalid_static_page_image_job_id",
            format!("{raw} is not a valid UUID"),
        )
    })
}

fn parse_published_report_id(raw: &str) -> std::result::Result<PublishedReportId, ApiError> {
    Uuid::parse_str(raw).map(PublishedReportId).map_err(|_| {
        ApiError::bad_request(
            "invalid_published_report_id",
            format!("{raw} is not a valid UUID"),
        )
    })
}

fn parse_dataset_output_id(raw: &str) -> std::result::Result<DatasetOutputId, ApiError> {
    Uuid::parse_str(raw).map(DatasetOutputId).map_err(|_| {
        ApiError::bad_request(
            "invalid_dataset_output_id",
            format!("{raw} is not a valid UUID"),
        )
    })
}

fn parse_plan_id(raw: &str) -> std::result::Result<domain_model::ReportPlanId, ApiError> {
    Uuid::parse_str(raw)
        .map(domain_model::ReportPlanId)
        .map_err(|_| {
            ApiError::bad_request(
                "invalid_report_plan_id",
                format!("{raw} is not a valid UUID"),
            )
        })
}

fn parse_document_id(raw: &str) -> std::result::Result<DocumentId, ApiError> {
    Uuid::parse_str(raw).map(DocumentId).map_err(|_| {
        ApiError::bad_request("invalid_document_id", format!("{raw} is not a valid UUID"))
    })
}

fn build_workflow_signal(
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

fn workflow_runtime_state_from_execution(
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

fn workflow_execution_from_transition(
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
    previous_status: &domain_model::WorkflowStatus,
    previous_attempt: u32,
    next_status: &domain_model::WorkflowStatus,
) -> u32 {
    if *previous_status == domain_model::WorkflowStatus::Pending
        && *next_status == domain_model::WorkflowStatus::Running
    {
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

fn required_field(
    field: &'static str,
    value: Option<String>,
) -> std::result::Result<String, ApiError> {
    let value = value
        .ok_or_else(|| ApiError::bad_request("validation_error", format!("{field} is required")))?;
    validate_required(field, &value)?;
    Ok(value.trim().to_string())
}

async fn hydrate_report_plan_summary(
    state: &AppState,
    plan: ReportPlan,
) -> std::result::Result<ReportPlanSummary, ApiError> {
    let service_handoff = load_report_plan_service_handoff(state, plan.id).await?;
    Ok(to_report_plan_summary(plan, service_handoff))
}

fn to_report_plan_summary(
    plan: ReportPlan,
    service_handoff: Option<contracts::ManifestServiceHandoffView>,
) -> ReportPlanSummary {
    let mut view = ReportPlanSummary {
        id: plan.id,
        dataset_id: plan.dataset_id,
        title: plan.title,
        objective: plan.objective,
        status: contracts::ReportPlanStatusView::from_domain(plan.status),
        theme_key: plan.theme_key,
        current_ast_version_id: plan.current_ast_version_id,
        service_handoff,
        model_facing: None,
    };
    view.model_facing = Some(derive_report_plan_model_facing_summary(&view));
    view
}

fn to_document_summary(document: Document) -> DocumentSummary {
    DocumentSummary {
        id: document.id,
        dataset_id: document.dataset_id,
        title: document.title,
        object_key: document.object_key,
        content_type: document.content_type,
        lifecycle: contracts::DocumentLifecycleView::from_domain(document.lifecycle),
        secret_binding_ids: document.secret_binding_ids,
        created_at: document.created_at,
        updated_at: document.updated_at,
    }
}

fn to_document_detail_view(
    document: Document,
    chunks: Vec<DocumentChunk>,
    retrieval_evidences: Vec<RetrievalEvidence>,
) -> DocumentDetailView {
    let mut view = DocumentDetailView {
        document: to_document_summary(document),
        chunks: chunks.into_iter().map(to_document_chunk_view).collect(),
        retrieval_evidences: retrieval_evidences
            .into_iter()
            .map(to_retrieval_evidence_view)
            .collect(),
        model_facing: None,
    };
    view.model_facing = Some(derive_document_detail_model_facing_summary(&view));
    view
}

fn to_compare_documents_view(documents: Vec<DocumentDetailView>) -> CompareDocumentsView {
    let mut view = CompareDocumentsView {
        documents,
        model_facing: None,
    };
    view.model_facing = Some(derive_compare_documents_model_facing_summary(&view));
    view
}

fn to_document_chunk_view(chunk: DocumentChunk) -> DocumentChunkView {
    DocumentChunkView {
        id: chunk.id,
        document_id: chunk.document_id,
        chunk_index: chunk.chunk_index,
        token_count: chunk.token_count,
        state: contracts::DocumentChunkStateView::from_domain(chunk.state),
        content: chunk.content,
        metadata: Value::Object(Map::from_iter(chunk.metadata)),
        created_at: chunk.created_at,
        updated_at: chunk.updated_at,
    }
}

fn to_retrieval_evidence_view(evidence: RetrievalEvidence) -> RetrievalEvidenceView {
    let evidence_manifest_view = parse_retrieval_evidence_manifest(&evidence);

    RetrievalEvidenceView {
        id: evidence.id,
        dataset_id: evidence.dataset_id,
        document_id: evidence.document_id,
        document_chunk_id: evidence.document_chunk_id,
        execution_id: evidence.execution_id,
        chunk_index: evidence.chunk_index,
        source_locator: evidence.source_locator,
        content_excerpt: evidence.content_excerpt,
        summary: evidence.summary,
        payload_filter_key: evidence.payload_filter_key,
        embedding_model: evidence.embedding_model,
        recall_score: evidence.recall_score,
        evidence_manifest: evidence.evidence_manifest,
        evidence_manifest_view,
        created_at: evidence.created_at,
    }
}

struct RankedRetrievalEvidence<'a> {
    evidence: &'a RetrievalEvidence,
    score: f64,
    lexical_score: f64,
    recall_score: f64,
    rank_hint: usize,
}

fn rank_retrieval_evidences_for_prompt<'a>(
    evidences: &'a [RetrievalEvidence],
    prompt: &str,
    limit: usize,
) -> Vec<RankedRetrievalEvidence<'a>> {
    if evidences.is_empty() || limit == 0 {
        return Vec::new();
    }

    let query_weights = lexical_query_term_weights(prompt);
    let query_norm = vector_norm(&query_weights);
    let mut ranked = evidences
        .iter()
        .map(|evidence| {
            let lexical_score = lexical_query_score(evidence, &query_weights, query_norm);
            let score = if lexical_score > 0.0 {
                lexical_score
            } else {
                evidence.recall_score
            };
            RankedRetrievalEvidence {
                evidence,
                score,
                lexical_score,
                recall_score: evidence.recall_score,
                rank_hint: rank_hint_from_evidence_manifest(evidence).unwrap_or(usize::MAX),
            }
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        right
            .lexical_score
            .partial_cmp(&left.lexical_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                right
                    .score
                    .partial_cmp(&left.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| {
                right
                    .recall_score
                    .partial_cmp(&left.recall_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| left.rank_hint.cmp(&right.rank_hint))
            .then_with(|| right.evidence.created_at.cmp(&left.evidence.created_at))
            .then_with(|| left.evidence.chunk_index.cmp(&right.evidence.chunk_index))
    });
    ranked.into_iter().take(limit).collect()
}

fn select_retrieval_evidence_ids_for_prompt(
    evidences: &[RetrievalEvidence],
    prompt: &str,
    limit: usize,
) -> Vec<RetrievalEvidenceId> {
    rank_retrieval_evidences_for_prompt(evidences, prompt, limit)
        .into_iter()
        .map(|ranked| ranked.evidence.id)
        .collect()
}

fn search_retrieval_hits(
    evidences: &[RetrievalEvidence],
    prompt: &str,
    limit: usize,
) -> Vec<RetrievalSearchHitView> {
    rank_retrieval_evidences_for_prompt(evidences, prompt, limit)
        .into_iter()
        .map(|ranked| RetrievalSearchHitView {
            retrieval_evidence_id: ranked.evidence.id,
            document_id: ranked.evidence.document_id,
            score: ranked.score,
            summary: ranked.evidence.summary.clone(),
            source_locator: ranked.evidence.source_locator.clone(),
        })
        .collect()
}

fn normalize_retrieval_search_limit(limit: Option<usize>) -> usize {
    limit
        .unwrap_or(RETRIEVAL_SEARCH_DEFAULT_LIMIT)
        .clamp(1, RETRIEVAL_SEARCH_MAX_LIMIT)
}

fn retrieval_search_scan_limit(limit: usize) -> i64 {
    let scan_limit = limit.saturating_mul(8);
    let clamped = scan_limit
        .max(RETRIEVAL_SEARCH_DEFAULT_LIMIT)
        .min(DATASET_OUTPUT_RETRIEVAL_SCAN_LIMIT as usize);
    i64::try_from(clamped).unwrap_or(DATASET_OUTPUT_RETRIEVAL_SCAN_LIMIT)
}

fn normalize_static_page_draft_list_limit(limit: Option<i64>) -> i64 {
    limit
        .unwrap_or(STATIC_PAGE_DRAFT_LIST_DEFAULT_LIMIT)
        .max(1)
        .min(STATIC_PAGE_DRAFT_LIST_MAX_LIMIT)
}

fn sort_retrieval_evidences_by_relevance(evidences: &mut [RetrievalEvidence]) {
    evidences.sort_by(|left, right| {
        right
            .recall_score
            .partial_cmp(&left.recall_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                rank_hint_from_evidence_manifest(left)
                    .unwrap_or(usize::MAX)
                    .cmp(&rank_hint_from_evidence_manifest(right).unwrap_or(usize::MAX))
            })
            .then_with(|| left.chunk_index.cmp(&right.chunk_index))
            .then_with(|| right.created_at.cmp(&left.created_at))
    });
}

fn lexical_query_score(
    evidence: &RetrievalEvidence,
    query_weights: &BTreeMap<String, f64>,
    query_norm: f64,
) -> f64 {
    if query_weights.is_empty() || query_norm <= 0.0 {
        return 0.0;
    }

    let evidence_weights = evidence_term_weights_from_manifest(evidence);
    let evidence_norm = vector_norm(&evidence_weights);
    if evidence_weights.is_empty() || evidence_norm <= 0.0 {
        return 0.0;
    }

    let dot_product = query_weights
        .iter()
        .filter_map(|(term, query_weight)| {
            evidence_weights
                .get(term)
                .map(|evidence_weight| query_weight * evidence_weight)
        })
        .sum::<f64>();
    if dot_product <= 0.0 {
        return 0.0;
    }

    (dot_product / (query_norm * evidence_norm) * 10_000.0).round() / 10_000.0
}

fn evidence_term_weights_from_manifest(evidence: &RetrievalEvidence) -> BTreeMap<String, f64> {
    evidence
        .evidence_manifest
        .get("embedding")
        .and_then(|value| value.get("term_weights"))
        .and_then(Value::as_object)
        .map(|weights| {
            weights
                .iter()
                .filter_map(|(term, weight)| weight.as_f64().map(|value| (term.clone(), value)))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default()
}

fn rank_hint_from_evidence_manifest(evidence: &RetrievalEvidence) -> Option<usize> {
    evidence
        .evidence_manifest
        .get("recall")
        .and_then(|value| value.get("rank_hint"))
        .and_then(Value::as_u64)
        .map(|value| value as usize)
}

fn lexical_query_term_weights(query: &str) -> BTreeMap<String, f64> {
    let mut frequencies: BTreeMap<String, f64> = BTreeMap::new();
    for token in lexical_query_tokens(query) {
        *frequencies.entry(token).or_insert(0.0) += 1.0;
    }

    frequencies
        .into_iter()
        .map(|(term, count)| (term, 1.0 + count.ln()))
        .collect()
}

fn vector_norm(weights: &BTreeMap<String, f64>) -> f64 {
    weights
        .values()
        .map(|weight| weight * weight)
        .sum::<f64>()
        .sqrt()
}

fn lexical_query_tokens(content: &str) -> Vec<String> {
    let normalized = content
        .chars()
        .map(|value| {
            if value.is_ascii_alphanumeric() {
                value.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>();

    normalized
        .split_whitespace()
        .filter_map(normalize_lexical_query_token)
        .collect()
}

fn normalize_lexical_query_token(token: &str) -> Option<String> {
    if token.len() < 2 || is_lexical_stop_word(token) {
        return None;
    }

    Some(token.to_string())
}

fn is_lexical_stop_word(token: &str) -> bool {
    matches!(
        token,
        "a" | "an"
            | "and"
            | "are"
            | "as"
            | "at"
            | "be"
            | "by"
            | "for"
            | "from"
            | "in"
            | "into"
            | "is"
            | "it"
            | "of"
            | "on"
            | "or"
            | "that"
            | "the"
            | "this"
            | "to"
            | "was"
            | "were"
            | "with"
    )
}

fn to_memory_directory_view(directory: MemoryDirectory) -> MemoryDirectoryView {
    MemoryDirectoryView {
        id: directory.id,
        dataset_id: directory.dataset_id,
        execution_id: directory.execution_id,
        version_no: directory.version_no,
        directory_nodes: directory.directory_nodes,
        refreshed_chunks: directory.refreshed_chunks,
        directory_tree: parse_memory_directory_manifest(
            &directory.directory_manifest,
            directory.version_no,
        ),
        directory_manifest: directory.directory_manifest,
        created_at: directory.created_at,
    }
}

fn parse_memory_directory_manifest(
    value: &Value,
    version_no: i32,
) -> Option<contracts::MemoryDirectoryManifestView> {
    let object = value.as_object()?;

    Some(contracts::MemoryDirectoryManifestView {
        schema_version: object.get("schema_version")?.as_str()?.to_string(),
        generator: object.get("generator")?.as_str()?.to_string(),
        dataset_id: DatasetId::from(Uuid::parse_str(object.get("dataset_id")?.as_str()?).ok()?),
        version_no: object
            .get("version_no")
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok())
            .unwrap_or(version_no),
        include_directory: object.get("include_directory")?.as_bool()?,
        root: parse_memory_directory_node(object.get("root")?, true, version_no)?,
    })
}

fn parse_memory_directory_scope(value: &str) -> Option<contracts::MemoryDirectoryNodeScopeView> {
    match value {
        "dataset" => Some(contracts::MemoryDirectoryNodeScopeView::Dataset),
        _ => None,
    }
}

fn parse_memory_directory_node(
    value: &Value,
    is_root: bool,
    root_version_no: i32,
) -> Option<contracts::MemoryDirectoryNodeView> {
    let object = value.as_object()?;
    let kind = match object.get("kind")?.as_str()? {
        "dataset" => contracts::MemoryDirectoryNodeKind::Dataset,
        "document" => contracts::MemoryDirectoryNodeKind::Document,
        _ => return None,
    };

    let document = match kind {
        contracts::MemoryDirectoryNodeKind::Dataset => None,
        contracts::MemoryDirectoryNodeKind::Document => {
            Some(contracts::MemoryDirectoryDocumentView {
                id: DocumentId::from(Uuid::parse_str(object.get("document_id")?.as_str()?).ok()?),
                lifecycle: contracts::DocumentLifecycleView::from_str(
                    object.get("lifecycle")?.as_str()?,
                )?,
                chunk_count: object.get("chunk_count")?.as_u64()? as usize,
            })
        }
    };

    let scope = object
        .get("scope")
        .and_then(Value::as_str)
        .and_then(parse_memory_directory_scope)
        .or_else(|| {
            if is_root {
                Some(contracts::MemoryDirectoryNodeScopeView::Dataset)
            } else {
                None
            }
        });
    let version_no = object
        .get("version_no")
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
        .or_else(|| if is_root { Some(root_version_no) } else { None });
    let children = object
        .get("children")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|child| parse_memory_directory_node(child, false, root_version_no))
        .collect::<Option<Vec<_>>>()?;

    Some(contracts::MemoryDirectoryNodeView {
        kind,
        title: object.get("title")?.as_str()?.to_string(),
        scope,
        version_no,
        document,
        children,
    })
}

fn parse_manifest_context_binding(value: &str) -> Option<contracts::ManifestContextBindingView> {
    match value {
        "creation_time" => Some(contracts::ManifestContextBindingView::CreationTime),
        _ => None,
    }
}

fn parse_manifest_runtime_mode(value: &str) -> Option<contracts::ManifestRuntimeModeView> {
    match value {
        "placeholder" => Some(contracts::ManifestRuntimeModeView::Placeholder),
        "provider" => Some(contracts::ManifestRuntimeModeView::Provider),
        _ => None,
    }
}

fn parse_manifest_finish_reason(value: &str) -> contracts::ManifestFinishReasonView {
    match value {
        "stop" => contracts::ManifestFinishReasonView::Stop,
        "tool_calls" => contracts::ManifestFinishReasonView::ToolCalls,
        "length" => contracts::ManifestFinishReasonView::Length,
        "content_filter" => contracts::ManifestFinishReasonView::ContentFilter,
        "error" => contracts::ManifestFinishReasonView::Error,
        _ => contracts::ManifestFinishReasonView::Other(value.to_string()),
    }
}

fn parse_manifest_provider_failure_kind(
    value: &str,
) -> Option<contracts::ManifestProviderFailureKindView> {
    match value {
        "request_failed" => Some(contracts::ManifestProviderFailureKindView::RequestFailed),
        "request_timeout" => Some(contracts::ManifestProviderFailureKindView::RequestTimeout),
        "http_status" => Some(contracts::ManifestProviderFailureKindView::HttpStatus),
        "response_body_read_failed" => {
            Some(contracts::ManifestProviderFailureKindView::ResponseBodyReadFailed)
        }
        "invalid_json" => Some(contracts::ManifestProviderFailureKindView::InvalidJson),
        "invalid_response" => Some(contracts::ManifestProviderFailureKindView::InvalidResponse),
        "finish_reason_error" => {
            Some(contracts::ManifestProviderFailureKindView::FinishReasonError)
        }
        _ => None,
    }
}

fn parse_manifest_provider_failure(
    value: &Value,
) -> Option<contracts::ManifestProviderFailureView> {
    let object = value.as_object()?;
    Some(contracts::ManifestProviderFailureView {
        kind: parse_manifest_provider_failure_kind(object.get("kind")?.as_str()?)?,
        message: object.get("message")?.as_str()?.to_string(),
    })
}

fn parse_manifest_usage(value: &Value) -> Option<contracts::ManifestTokenUsageView> {
    let object = value.as_object()?;

    Some(contracts::ManifestTokenUsageView {
        input_tokens: object.get("input_tokens")?.as_u64()? as usize,
        output_tokens: object.get("output_tokens")?.as_u64()? as usize,
        total_tokens: object.get("total_tokens")?.as_u64()? as usize,
    })
}

fn parse_manifest_runtime(value: &Value) -> Option<contracts::ManifestRuntimeView> {
    let object = value.as_object()?;

    Some(contracts::ManifestRuntimeView {
        mode: parse_manifest_runtime_mode(object.get("mode")?.as_str()?)?,
        provider: object
            .get("provider")
            .and_then(Value::as_str)
            .map(str::to_string),
        model: object
            .get("model")
            .and_then(Value::as_str)
            .map(str::to_string),
        request_id: object
            .get("request_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        finish_reason: object
            .get("finish_reason")
            .and_then(Value::as_str)
            .map(parse_manifest_finish_reason),
        provider_failure: object
            .get("provider_failure")
            .and_then(parse_manifest_provider_failure),
        latency_ms: object.get("latency_ms").and_then(Value::as_u64),
        usage: object.get("usage").and_then(parse_manifest_usage),
        system_prompt_key: object
            .get("system_prompt_key")
            .and_then(Value::as_str)
            .map(str::to_string),
        system_prompt_version: object
            .get("system_prompt_version")
            .and_then(Value::as_str)
            .map(str::to_string),
        tool_trace_count: object
            .get("tool_trace_count")
            .and_then(Value::as_u64)
            .map(|value| value as usize),
    })
}

fn parse_manifest_tool_call_status(value: &str) -> Option<contracts::ManifestToolCallStatusView> {
    match value {
        "requested" => Some(contracts::ManifestToolCallStatusView::Requested),
        "completed" => Some(contracts::ManifestToolCallStatusView::Completed),
        "failed" => Some(contracts::ManifestToolCallStatusView::Failed),
        _ => None,
    }
}

fn to_tool_reference_view(tool: &ToolDefinition) -> contracts::ToolReferenceView {
    contracts::ToolReferenceView {
        key: tool.key.clone(),
        title: tool.title.clone(),
        scope_policy: tool.scope_policy.clone(),
        invocation_mode: match tool.invocation_mode {
            ToolInvocationMode::Cli => contracts::ToolInvocationModeView::Cli,
            ToolInvocationMode::Internal => contracts::ToolInvocationModeView::Internal,
        },
        cli: tool.cli.as_ref().map(|cli| contracts::ToolCliContractView {
            argv: cli.argv.clone(),
            env_allowlist: cli.env_allowlist.clone(),
            output_mode: match cli.output_mode {
                ToolCliOutputMode::Json => contracts::ToolCliOutputModeView::Json,
                ToolCliOutputMode::Text => contracts::ToolCliOutputModeView::Text,
            },
            timeout_ms: cli.timeout_ms,
        }),
    }
}

fn find_registered_tool_reference(tool_name: &str) -> Option<contracts::ToolReferenceView> {
    let registry = bootstrap_default_tool_registry();
    registry.get(tool_name).map(to_tool_reference_view)
}

fn parse_tool_invocation_mode(value: &str) -> Option<contracts::ToolInvocationModeView> {
    match value {
        "cli" => Some(contracts::ToolInvocationModeView::Cli),
        "internal" => Some(contracts::ToolInvocationModeView::Internal),
        _ => None,
    }
}

fn parse_tool_cli_output_mode(value: &str) -> Option<contracts::ToolCliOutputModeView> {
    match value {
        "json" => Some(contracts::ToolCliOutputModeView::Json),
        "text" => Some(contracts::ToolCliOutputModeView::Text),
        _ => None,
    }
}

fn parse_tool_cli_contract(value: &Value) -> Option<contracts::ToolCliContractView> {
    let object = value.as_object()?;
    Some(contracts::ToolCliContractView {
        argv: object
            .get("argv")?
            .as_array()?
            .iter()
            .map(|value| value.as_str().map(str::to_string))
            .collect::<Option<Vec<_>>>()?,
        env_allowlist: object
            .get("env_allowlist")?
            .as_array()?
            .iter()
            .map(|value| value.as_str().map(str::to_string))
            .collect::<Option<Vec<_>>>()?,
        output_mode: parse_tool_cli_output_mode(object.get("output_mode")?.as_str()?)?,
        timeout_ms: object.get("timeout_ms").and_then(Value::as_u64),
    })
}

fn parse_tool_reference(value: &Value) -> Option<contracts::ToolReferenceView> {
    let object = value.as_object()?;
    Some(contracts::ToolReferenceView {
        key: object.get("key")?.as_str()?.to_string(),
        title: object.get("title")?.as_str()?.to_string(),
        scope_policy: object.get("scope_policy")?.as_str()?.to_string(),
        invocation_mode: parse_tool_invocation_mode(object.get("invocation_mode")?.as_str()?)?,
        cli: object.get("cli").and_then(parse_tool_cli_contract),
    })
}

fn parse_manifest_tool_call(value: &Value) -> Option<contracts::ManifestToolCallView> {
    match value {
        Value::String(tool_name) => Some(contracts::ManifestToolCallView {
            call_id: None,
            tool_name: tool_name.clone(),
            tool: find_registered_tool_reference(tool_name),
            status: contracts::ManifestToolCallStatusView::Completed,
            arguments: None,
            result: None,
        }),
        Value::Object(object) => Some(contracts::ManifestToolCallView {
            call_id: object
                .get("call_id")
                .and_then(Value::as_str)
                .map(str::to_string),
            tool_name: object.get("tool_name")?.as_str()?.to_string(),
            tool: object
                .get("tool")
                .and_then(parse_tool_reference)
                .or_else(|| find_registered_tool_reference(object.get("tool_name")?.as_str()?)),
            status: parse_manifest_tool_call_status(object.get("status")?.as_str()?)?,
            arguments: object.get("arguments").cloned(),
            result: object.get("result").cloned(),
        }),
        _ => None,
    }
}

fn parse_manifest_tool_trace(
    value: Option<&Value>,
) -> Option<Vec<contracts::ManifestToolCallView>> {
    let trace = match value {
        Some(Value::Array(entries)) => entries
            .iter()
            .map(parse_manifest_tool_call)
            .collect::<Option<Vec<_>>>()?,
        Some(_) => return None,
        None => Vec::new(),
    };

    Some(trace)
}

fn to_tool_definition_view(tool: &ToolDefinition) -> ToolDefinitionView {
    ToolDefinitionView {
        key: tool.key.clone(),
        title: tool.title.clone(),
        scope_policy: tool.scope_policy.clone(),
        input_schema: tool.input_schema.clone(),
        output_schema: tool.output_schema.clone(),
        invocation_mode: match tool.invocation_mode {
            ToolInvocationMode::Cli => contracts::ToolInvocationModeView::Cli,
            ToolInvocationMode::Internal => contracts::ToolInvocationModeView::Internal,
        },
        cli: tool.cli.as_ref().map(|cli| contracts::ToolCliContractView {
            argv: cli.argv.clone(),
            env_allowlist: cli.env_allowlist.clone(),
            output_mode: match cli.output_mode {
                ToolCliOutputMode::Json => contracts::ToolCliOutputModeView::Json,
                ToolCliOutputMode::Text => contracts::ToolCliOutputModeView::Text,
            },
            timeout_ms: cli.timeout_ms,
        }),
    }
}

fn parse_dataset_output_manifest(value: &Value) -> Option<contracts::DatasetOutputManifestView> {
    let object = value.as_object()?;
    let tool_trace = parse_manifest_tool_trace(object.get("tool_trace"))?;
    let runtime = object
        .get("runtime")
        .and_then(parse_manifest_runtime)
        .or_else(|| {
            object
                .get("generator")
                .and_then(Value::as_str)
                .filter(|generator| *generator == "dataset-output-worker")
                .map(|_| contracts::ManifestRuntimeView {
                    mode: contracts::ManifestRuntimeModeView::Placeholder,
                    provider: None,
                    model: None,
                    request_id: None,
                    finish_reason: None,
                    provider_failure: None,
                    latency_ms: None,
                    usage: None,
                    system_prompt_key: None,
                    system_prompt_version: None,
                    tool_trace_count: Some(tool_trace.len()),
                })
        });

    let parse_retrieval_evidence_ids = |value: &Value| {
        value
            .as_array()?
            .iter()
            .map(|value| {
                Uuid::parse_str(value.as_str()?)
                    .ok()
                    .map(RetrievalEvidenceId::from)
            })
            .collect::<Option<Vec<_>>>()
    };
    let parse_dataset_output_format = |value: &str| match value {
        "markdown" => Some(contracts::DatasetOutputFormatView::Markdown),
        _ => None,
    };
    let parse_dataset_output_section_kind = |value: &str| match value {
        "summary" => Some(contracts::DatasetOutputSectionKindView::Summary),
        _ => None,
    };
    let parse_dataset_output_content = |value: &Value| {
        let output = value.as_object()?;
        let sections = output
            .get("sections")?
            .as_array()?
            .iter()
            .map(|value| {
                let section = value.as_object()?;
                Some(contracts::DatasetOutputSectionView {
                    section_key: section.get("section_key")?.as_str()?.to_string(),
                    kind: parse_dataset_output_section_kind(section.get("kind")?.as_str()?)?,
                    title: section.get("title")?.as_str()?.to_string(),
                    content: section.get("content")?.as_str()?.to_string(),
                    retrieval_evidence_ids: parse_retrieval_evidence_ids(
                        section.get("retrieval_evidence_ids")?,
                    )?,
                })
            })
            .collect::<Option<Vec<_>>>()?;

        Some(contracts::DatasetOutputContentView {
            format: parse_dataset_output_format(output.get("format")?.as_str()?)?,
            sections,
        })
    };

    Some(contracts::DatasetOutputManifestView {
        generator: object.get("generator")?.as_str()?.to_string(),
        schema_version: object.get("schema_version")?.as_str()?.to_string(),
        dataset_id: DatasetId::from(Uuid::parse_str(object.get("dataset_id")?.as_str()?).ok()?),
        prompt: object.get("prompt")?.as_str()?.to_string(),
        indexed_document_count: object.get("indexed_document_count")?.as_u64()? as usize,
        refreshed_chunks: object.get("refreshed_chunks")?.as_u64()? as usize,
        memory_directory_id: object
            .get("memory_directory_id")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .map(MemoryDirectoryId::from),
        memory_directory_version_no: object
            .get("memory_directory_version_no")
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok()),
        retrieval_evidence_count: object.get("retrieval_evidence_count")?.as_u64()? as usize,
        retrieval_evidence_ids: parse_retrieval_evidence_ids(
            object.get("retrieval_evidence_ids")?,
        )?,
        output: object.get("output").and_then(parse_dataset_output_content),
        service_handoff: object
            .get("service_handoff")
            .and_then(parse_manifest_service_handoff),
        tool_trace,
        context_binding: parse_manifest_context_binding(object.get("context_binding")?.as_str()?)?,
        runtime,
    })
}

fn parse_chat_session_status(value: &str) -> Option<contracts::ChatSessionManifestStatusView> {
    match value {
        "pending_assistant_reply" => {
            Some(contracts::ChatSessionManifestStatusView::PendingAssistantReply)
        }
        "assistant_replied" => Some(contracts::ChatSessionManifestStatusView::AssistantReplied),
        _ => None,
    }
}

fn parse_chat_session_turn_kind(value: &str) -> Option<contracts::ChatSessionTurnKindView> {
    match value {
        "placeholder_orchestration" => {
            Some(contracts::ChatSessionTurnKindView::PlaceholderOrchestration)
        }
        _ => None,
    }
}

fn parse_model_facing_report_entry_state(
    value: &str,
) -> Option<contracts::ModelFacingReportEntryStateView> {
    match value {
        "not_applicable" => Some(contracts::ModelFacingReportEntryStateView::NotApplicable),
        "confirmation_required" => {
            Some(contracts::ModelFacingReportEntryStateView::ConfirmationRequired)
        }
        "confirmed" => Some(contracts::ModelFacingReportEntryStateView::Confirmed),
        _ => None,
    }
}

fn parse_model_facing_service_lane(value: &str) -> Option<contracts::ModelFacingServiceLaneView> {
    match value {
        "material_service" => Some(contracts::ModelFacingServiceLaneView::MaterialService),
        "report_service" => Some(contracts::ModelFacingServiceLaneView::ReportService),
        "controlled_platform_action" => {
            Some(contracts::ModelFacingServiceLaneView::ControlledPlatformAction)
        }
        _ => None,
    }
}

fn parse_chat_session_report_entry_resolution(
    value: &str,
) -> Option<contracts::ChatSessionReportEntryResolutionView> {
    match value {
        "stay_material_service" => {
            Some(contracts::ChatSessionReportEntryResolutionView::StayMaterialService)
        }
        "enter_report_service" => {
            Some(contracts::ChatSessionReportEntryResolutionView::EnterReportService)
        }
        _ => None,
    }
}

fn parse_manifest_service_handoff_source(
    value: &str,
) -> Option<contracts::ManifestServiceHandoffSourceView> {
    match value {
        "chat_session_report_entry" => {
            Some(contracts::ManifestServiceHandoffSourceView::ChatSessionReportEntry)
        }
        _ => None,
    }
}

fn parse_manifest_service_handoff(value: &Value) -> Option<contracts::ManifestServiceHandoffView> {
    let handoff = value.as_object()?;
    Some(contracts::ManifestServiceHandoffView {
        source: parse_manifest_service_handoff_source(handoff.get("source")?.as_str()?)?,
        service_lane: parse_model_facing_service_lane(handoff.get("service_lane")?.as_str()?)?,
        report_entry_state: parse_model_facing_report_entry_state(
            handoff.get("report_entry_state")?.as_str()?,
        )?,
        requested_at: handoff
            .get("requested_at")
            .and_then(parse_manifest_timestamp),
        resolved_at: handoff
            .get("resolved_at")
            .and_then(parse_manifest_timestamp),
        resolved_action: handoff
            .get("resolved_action")
            .and_then(Value::as_str)
            .and_then(parse_chat_session_report_entry_resolution),
        suggested_title: handoff
            .get("suggested_title")
            .and_then(Value::as_str)
            .map(str::to_string),
        suggested_objective: handoff
            .get("suggested_objective")
            .and_then(Value::as_str)
            .map(str::to_string),
        confirmed_report_plan_id: handoff
            .get("confirmed_report_plan_id")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .map(ReportPlanId::from),
    })
}

fn parse_manifest_timestamp(value: &Value) -> Option<DateTime<Utc>> {
    let timestamp = value.as_str()?;
    DateTime::parse_from_rfc3339(timestamp)
        .ok()
        .map(|value| value.with_timezone(&Utc))
}

fn parse_retrieval_embedding_status(
    value: &str,
) -> Option<contracts::RetrievalEmbeddingStatusView> {
    match value {
        "pending" => Some(contracts::RetrievalEmbeddingStatusView::Pending),
        "indexed" => Some(contracts::RetrievalEmbeddingStatusView::Indexed),
        "failed" => Some(contracts::RetrievalEmbeddingStatusView::Failed),
        _ => None,
    }
}

fn parse_retrieval_recall_status(value: &str) -> Option<contracts::RetrievalRecallStatusView> {
    match value {
        "pending" => Some(contracts::RetrievalRecallStatusView::Pending),
        "ready" => Some(contracts::RetrievalRecallStatusView::Ready),
        "failed" => Some(contracts::RetrievalRecallStatusView::Failed),
        _ => None,
    }
}

fn parse_retrieval_evidence_manifest(
    evidence: &RetrievalEvidence,
) -> Option<contracts::RetrievalEvidenceManifestView> {
    let object = evidence.evidence_manifest.as_object()?;
    let embedding = object.get("embedding")?.as_object()?;
    let recall = object.get("recall")?.as_object()?;
    let evidence_object = object.get("evidence")?.as_object()?;

    let parse_dataset_id = |key: &str| {
        object
            .get(key)
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .map(DatasetId::from)
    };
    let parse_document_id = |key: &str| {
        object
            .get(key)
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .map(DocumentId::from)
    };
    let parse_document_chunk_id = |container: &serde_json::Map<String, Value>, key: &str| {
        container
            .get(key)
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .map(DocumentChunkId::from)
    };

    Some(contracts::RetrievalEvidenceManifestView {
        schema_version: object
            .get("schema_version")
            .and_then(Value::as_str)
            .unwrap_or("0.2.0")
            .to_string(),
        generator: object
            .get("generator")
            .and_then(Value::as_str)
            .unwrap_or("retrieval-worker")
            .to_string(),
        dataset_id: parse_dataset_id("dataset_id").unwrap_or(evidence.dataset_id),
        document_id: parse_document_id("document_id").unwrap_or(evidence.document_id),
        document_chunk_id: parse_document_chunk_id(object, "document_chunk_id")
            .or_else(|| parse_document_chunk_id(evidence_object, "document_chunk_id"))
            .unwrap_or(evidence.document_chunk_id),
        chunk_index: object
            .get("chunk_index")
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok())
            .unwrap_or(evidence.chunk_index),
        indexed_at: object
            .get("indexed_at")
            .and_then(parse_manifest_timestamp)
            .unwrap_or(evidence.created_at),
        embedding: contracts::RetrievalEmbeddingManifestView {
            status: parse_retrieval_embedding_status(embedding.get("status")?.as_str()?)?,
            model: embedding
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or(evidence.embedding_model.as_str())
                .to_string(),
            token_count: embedding.get("token_count")?.as_u64()? as usize,
        },
        recall: contracts::RetrievalRecallManifestView {
            status: parse_retrieval_recall_status(recall.get("status")?.as_str()?)?,
            score: recall
                .get("score")
                .and_then(Value::as_f64)
                .unwrap_or(evidence.recall_score),
            rank_hint: recall
                .get("rank_hint")
                .and_then(Value::as_u64)
                .map(|value| value as usize)
                .or_else(|| {
                    usize::try_from(evidence.chunk_index)
                        .ok()
                        .map(|value| value + 1)
                })?,
        },
        evidence: contracts::RetrievalEvidenceLocatorManifestView {
            document_chunk_id: parse_document_chunk_id(object, "document_chunk_id")
                .or_else(|| parse_document_chunk_id(evidence_object, "document_chunk_id"))
                .unwrap_or(evidence.document_chunk_id),
            payload_filter_key: evidence_object
                .get("payload_filter_key")
                .and_then(Value::as_str)
                .unwrap_or(evidence.payload_filter_key.as_str())
                .to_string(),
            source_locator: evidence_object
                .get("source_locator")
                .and_then(Value::as_str)
                .unwrap_or(evidence.source_locator.as_str())
                .to_string(),
        },
    })
}

fn parse_chat_turn_status(value: &str) -> Option<contracts::ChatTurnStatusView> {
    match value {
        "pending" => Some(contracts::ChatTurnStatusView::Pending),
        "completed" => Some(contracts::ChatTurnStatusView::Completed),
        "failed" => Some(contracts::ChatTurnStatusView::Failed),
        _ => None,
    }
}

fn parse_chat_turn_stream_mode(value: &str) -> Option<contracts::ChatTurnStreamModeView> {
    match value {
        "buffered" => Some(contracts::ChatTurnStreamModeView::Buffered),
        "streaming" => Some(contracts::ChatTurnStreamModeView::Streaming),
        _ => None,
    }
}

fn parse_chat_turn_stream_status(value: &str) -> Option<contracts::ChatTurnStreamStatusView> {
    match value {
        "not_requested" => Some(contracts::ChatTurnStreamStatusView::NotRequested),
        "pending" => Some(contracts::ChatTurnStreamStatusView::Pending),
        "completed" => Some(contracts::ChatTurnStreamStatusView::Completed),
        "failed" => Some(contracts::ChatTurnStreamStatusView::Failed),
        _ => None,
    }
}

fn parse_chat_turn_artifact_commit_status(
    value: &str,
) -> Option<contracts::ChatTurnArtifactCommitStatusView> {
    match value {
        "not_ready" => Some(contracts::ChatTurnArtifactCommitStatusView::NotReady),
        "pending" => Some(contracts::ChatTurnArtifactCommitStatusView::Pending),
        "failed" => Some(contracts::ChatTurnArtifactCommitStatusView::Failed),
        "completed" => Some(contracts::ChatTurnArtifactCommitStatusView::Completed),
        _ => None,
    }
}

fn parse_chat_turn_artifact_commit_failure_source(
    value: &str,
) -> Option<contracts::ChatTurnArtifactCommitFailureSourceView> {
    match value {
        "workflow_event_recovery_persist" => {
            Some(contracts::ChatTurnArtifactCommitFailureSourceView::WorkflowEventRecoveryPersist)
        }
        "response_ready_session_update" => {
            Some(contracts::ChatTurnArtifactCommitFailureSourceView::ResponseReadySessionUpdate)
        }
        "assistant_message_create" => {
            Some(contracts::ChatTurnArtifactCommitFailureSourceView::AssistantMessageCreate)
        }
        "assistant_message_manifest_update" => {
            Some(contracts::ChatTurnArtifactCommitFailureSourceView::AssistantMessageManifestUpdate)
        }
        "llm_invocation_persist" => {
            Some(contracts::ChatTurnArtifactCommitFailureSourceView::LlmInvocationPersist)
        }
        "tool_execution_persist" => {
            Some(contracts::ChatTurnArtifactCommitFailureSourceView::ToolExecutionPersist)
        }
        "session_context_update" => {
            Some(contracts::ChatTurnArtifactCommitFailureSourceView::SessionContextUpdate)
        }
        _ => None,
    }
}

fn parse_chat_turn_provider_status(value: &str) -> Option<contracts::ChatTurnProviderStatusView> {
    match value {
        "pending" => Some(contracts::ChatTurnProviderStatusView::Pending),
        "responded" => Some(contracts::ChatTurnProviderStatusView::Responded),
        "failed" => Some(contracts::ChatTurnProviderStatusView::Failed),
        _ => None,
    }
}

fn parse_chat_turn_tool_loop_status(value: &str) -> Option<contracts::ChatTurnToolLoopStatusView> {
    match value {
        "not_requested" => Some(contracts::ChatTurnToolLoopStatusView::NotRequested),
        "pending" => Some(contracts::ChatTurnToolLoopStatusView::Pending),
        "completed" => Some(contracts::ChatTurnToolLoopStatusView::Completed),
        "failed" => Some(contracts::ChatTurnToolLoopStatusView::Failed),
        _ => None,
    }
}

fn parse_chat_turn_event_kind(value: &str) -> Option<contracts::ChatTurnEventKindView> {
    match value {
        "turn_started" => Some(contracts::ChatTurnEventKindView::TurnStarted),
        "provider_requested" => Some(contracts::ChatTurnEventKindView::ProviderRequested),
        "provider_responded" => Some(contracts::ChatTurnEventKindView::ProviderResponded),
        "first_token_emitted" => Some(contracts::ChatTurnEventKindView::FirstTokenEmitted),
        "stream_completed" => Some(contracts::ChatTurnEventKindView::StreamCompleted),
        "stream_failed" => Some(contracts::ChatTurnEventKindView::StreamFailed),
        "artifact_commit_ready" => Some(contracts::ChatTurnEventKindView::ArtifactCommitReady),
        "artifact_commit_failed" => Some(contracts::ChatTurnEventKindView::ArtifactCommitFailed),
        "tool_calls_emitted" => Some(contracts::ChatTurnEventKindView::ToolCallsEmitted),
        "tool_loop_completed" => Some(contracts::ChatTurnEventKindView::ToolLoopCompleted),
        "tool_loop_failed" => Some(contracts::ChatTurnEventKindView::ToolLoopFailed),
        "assistant_message_persisted" => {
            Some(contracts::ChatTurnEventKindView::AssistantMessagePersisted)
        }
        "turn_completed" => Some(contracts::ChatTurnEventKindView::TurnCompleted),
        "turn_failed" => Some(contracts::ChatTurnEventKindView::TurnFailed),
        _ => None,
    }
}

fn parse_chat_turn_tool_status_summary(
    value: &Value,
) -> Option<contracts::ChatTurnToolStatusSummaryView> {
    let object = value.as_object()?;
    Some(contracts::ChatTurnToolStatusSummaryView {
        requested_count: object.get("requested_count")?.as_u64()? as usize,
        completed_count: object.get("completed_count")?.as_u64()? as usize,
        failed_count: object.get("failed_count")?.as_u64()? as usize,
    })
}

fn build_chat_turn_events(
    turn: &contracts::ChatTurnRuntimeView,
) -> Vec<contracts::ChatTurnEventView> {
    let mut events = vec![contracts::ChatTurnEventView {
        kind: contracts::ChatTurnEventKindView::TurnStarted,
        at: turn.started_at,
        provider_request_id: None,
        finish_reason: None,
        tool_trace_count: None,
    }];

    if turn.provider_request_id.is_some() {
        events.push(contracts::ChatTurnEventView {
            kind: contracts::ChatTurnEventKindView::ProviderRequested,
            at: turn.provider_requested_at.unwrap_or(turn.started_at),
            provider_request_id: turn.provider_request_id.clone(),
            finish_reason: None,
            tool_trace_count: None,
        });
    }

    match turn.provider_status {
        contracts::ChatTurnProviderStatusView::Responded
        | contracts::ChatTurnProviderStatusView::Failed => {
            if let Some(provider_responded_at) = turn.provider_responded_at.or(turn.completed_at) {
                events.push(contracts::ChatTurnEventView {
                    kind: contracts::ChatTurnEventKindView::ProviderResponded,
                    at: provider_responded_at,
                    provider_request_id: turn.provider_request_id.clone(),
                    finish_reason: turn.finish_reason.clone(),
                    tool_trace_count: Some(turn.tool_trace_count),
                });
            }
        }
        contracts::ChatTurnProviderStatusView::Pending => {}
    }

    if let Some(first_token_at) = turn.first_token_at {
        events.push(contracts::ChatTurnEventView {
            kind: contracts::ChatTurnEventKindView::FirstTokenEmitted,
            at: first_token_at,
            provider_request_id: turn.provider_request_id.clone(),
            finish_reason: turn.finish_reason.clone(),
            tool_trace_count: Some(turn.tool_trace_count),
        });
    }

    if let Some(stream_completed_at) = turn.stream_completed_at {
        match turn.stream_status {
            contracts::ChatTurnStreamStatusView::Completed => {
                events.push(contracts::ChatTurnEventView {
                    kind: contracts::ChatTurnEventKindView::StreamCompleted,
                    at: stream_completed_at,
                    provider_request_id: turn.provider_request_id.clone(),
                    finish_reason: turn.finish_reason.clone(),
                    tool_trace_count: Some(turn.tool_trace_count),
                });
            }
            contracts::ChatTurnStreamStatusView::Failed => {
                events.push(contracts::ChatTurnEventView {
                    kind: contracts::ChatTurnEventKindView::StreamFailed,
                    at: stream_completed_at,
                    provider_request_id: turn.provider_request_id.clone(),
                    finish_reason: turn.finish_reason.clone(),
                    tool_trace_count: Some(turn.tool_trace_count),
                });
            }
            contracts::ChatTurnStreamStatusView::NotRequested
            | contracts::ChatTurnStreamStatusView::Pending => {}
        }
    }

    if let Some(artifact_commit_ready_at) = turn.artifact_commit_ready_at {
        events.push(contracts::ChatTurnEventView {
            kind: contracts::ChatTurnEventKindView::ArtifactCommitReady,
            at: artifact_commit_ready_at,
            provider_request_id: turn.provider_request_id.clone(),
            finish_reason: turn.finish_reason.clone(),
            tool_trace_count: Some(turn.tool_trace_count),
        });
    }

    if matches!(
        turn.artifact_commit_status,
        contracts::ChatTurnArtifactCommitStatusView::Failed
    ) {
        if let Some(artifact_commit_failed_at) = turn.completed_at.or(turn.artifact_commit_ready_at)
        {
            events.push(contracts::ChatTurnEventView {
                kind: contracts::ChatTurnEventKindView::ArtifactCommitFailed,
                at: artifact_commit_failed_at,
                provider_request_id: turn.provider_request_id.clone(),
                finish_reason: turn.finish_reason.clone(),
                tool_trace_count: Some(turn.tool_trace_count),
            });
        }
    }

    if let Some(tool_calls_emitted_at) = turn.tool_calls_emitted_at {
        events.push(contracts::ChatTurnEventView {
            kind: contracts::ChatTurnEventKindView::ToolCallsEmitted,
            at: tool_calls_emitted_at,
            provider_request_id: turn.provider_request_id.clone(),
            finish_reason: turn.finish_reason.clone(),
            tool_trace_count: Some(turn.tool_trace_count),
        });
    }

    if let Some(tool_loop_settled_at) = turn.tool_loop_settled_at {
        match turn.tool_loop_status {
            contracts::ChatTurnToolLoopStatusView::Completed => {
                events.push(contracts::ChatTurnEventView {
                    kind: contracts::ChatTurnEventKindView::ToolLoopCompleted,
                    at: tool_loop_settled_at,
                    provider_request_id: turn.provider_request_id.clone(),
                    finish_reason: turn.finish_reason.clone(),
                    tool_trace_count: Some(turn.tool_trace_count),
                });
            }
            contracts::ChatTurnToolLoopStatusView::Failed => {
                events.push(contracts::ChatTurnEventView {
                    kind: contracts::ChatTurnEventKindView::ToolLoopFailed,
                    at: tool_loop_settled_at,
                    provider_request_id: turn.provider_request_id.clone(),
                    finish_reason: turn.finish_reason.clone(),
                    tool_trace_count: Some(turn.tool_trace_count),
                });
            }
            contracts::ChatTurnToolLoopStatusView::NotRequested
            | contracts::ChatTurnToolLoopStatusView::Pending => {}
        }
    }

    if let Some(persisted_at) = turn.assistant_message_persisted_at {
        events.push(contracts::ChatTurnEventView {
            kind: contracts::ChatTurnEventKindView::AssistantMessagePersisted,
            at: persisted_at,
            provider_request_id: turn.provider_request_id.clone(),
            finish_reason: turn.finish_reason.clone(),
            tool_trace_count: Some(turn.tool_trace_count),
        });
    }

    if let Some(completed_at) = turn.completed_at {
        match turn.status {
            contracts::ChatTurnStatusView::Completed => {
                events.push(contracts::ChatTurnEventView {
                    kind: contracts::ChatTurnEventKindView::TurnCompleted,
                    at: completed_at,
                    provider_request_id: turn.provider_request_id.clone(),
                    finish_reason: turn.finish_reason.clone(),
                    tool_trace_count: Some(turn.tool_trace_count),
                });
            }
            contracts::ChatTurnStatusView::Failed => {
                events.push(contracts::ChatTurnEventView {
                    kind: contracts::ChatTurnEventKindView::TurnFailed,
                    at: completed_at,
                    provider_request_id: turn.provider_request_id.clone(),
                    finish_reason: turn.finish_reason.clone(),
                    tool_trace_count: Some(turn.tool_trace_count),
                });
            }
            contracts::ChatTurnStatusView::Pending => {}
        }
    }

    events
}

fn parse_chat_turn_events(
    value: Option<&Value>,
    fallback_turn: &contracts::ChatTurnRuntimeView,
) -> Option<Vec<contracts::ChatTurnEventView>> {
    let Some(value) = value else {
        return Some(build_chat_turn_events(fallback_turn));
    };

    value
        .as_array()?
        .iter()
        .map(|entry| {
            let entry = entry.as_object()?;
            Some(contracts::ChatTurnEventView {
                kind: parse_chat_turn_event_kind(entry.get("kind")?.as_str()?)?,
                at: parse_manifest_timestamp(entry.get("at")?)?,
                provider_request_id: entry
                    .get("provider_request_id")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                finish_reason: entry
                    .get("finish_reason")
                    .and_then(Value::as_str)
                    .map(parse_manifest_finish_reason),
                tool_trace_count: entry
                    .get("tool_trace_count")
                    .and_then(Value::as_u64)
                    .map(|value| value as usize),
            })
        })
        .collect::<Option<Vec<_>>>()
}

fn parse_chat_turn_runtime(value: &Value) -> Option<contracts::ChatTurnRuntimeView> {
    let object = value.as_object()?;
    let status = parse_chat_turn_status(object.get("status")?.as_str()?)?;
    let stream_mode = parse_chat_turn_stream_mode(object.get("stream_mode")?.as_str()?)?;
    let finish_reason = object
        .get("finish_reason")
        .and_then(Value::as_str)
        .map(parse_manifest_finish_reason);
    let tool_status_summary = object
        .get("tool_status_summary")
        .and_then(parse_chat_turn_tool_status_summary);
    let provider_requested_at = object
        .get("provider_requested_at")
        .and_then(parse_manifest_timestamp);
    let provider_responded_at = object
        .get("provider_responded_at")
        .and_then(parse_manifest_timestamp);
    let artifact_commit_ready_at = object
        .get("artifact_commit_ready_at")
        .and_then(parse_manifest_timestamp);
    let assistant_message_persisted_at = object
        .get("assistant_message_persisted_at")
        .and_then(parse_manifest_timestamp);
    let completed_at = object
        .get("completed_at")
        .and_then(parse_manifest_timestamp);
    let provider_status = object
        .get("provider_status")
        .and_then(Value::as_str)
        .and_then(parse_chat_turn_provider_status)
        .unwrap_or_else(
            || match (&status, finish_reason.as_ref(), provider_responded_at) {
                (contracts::ChatTurnStatusView::Pending, _, _) => {
                    contracts::ChatTurnProviderStatusView::Pending
                }
                (_, Some(contracts::ManifestFinishReasonView::Error), _) => {
                    contracts::ChatTurnProviderStatusView::Failed
                }
                (_, _, Some(_)) => contracts::ChatTurnProviderStatusView::Responded,
                (contracts::ChatTurnStatusView::Failed, _, None) => {
                    contracts::ChatTurnProviderStatusView::Failed
                }
                (_, _, None) => contracts::ChatTurnProviderStatusView::Responded,
            },
        );
    let provider_failure = object
        .get("provider_failure")
        .and_then(parse_manifest_provider_failure);
    let stream_status = object
        .get("stream_status")
        .and_then(Value::as_str)
        .and_then(parse_chat_turn_stream_status)
        .unwrap_or_else(|| {
            infer_chat_turn_stream_status(
                &stream_mode,
                &provider_status,
                provider_requested_at,
                provider_responded_at,
                completed_at,
            )
        });
    let artifact_commit_status = object
        .get("artifact_commit_status")
        .and_then(Value::as_str)
        .and_then(parse_chat_turn_artifact_commit_status)
        .unwrap_or_else(|| {
            infer_chat_turn_artifact_commit_status(
                &status,
                assistant_message_persisted_at,
                provider_responded_at,
                artifact_commit_ready_at,
            )
        });
    let artifact_commit_failure_source = object
        .get("artifact_commit_failure_source")
        .and_then(Value::as_str)
        .and_then(parse_chat_turn_artifact_commit_failure_source);
    let tool_loop_status = object
        .get("tool_loop_status")
        .and_then(Value::as_str)
        .and_then(parse_chat_turn_tool_loop_status)
        .unwrap_or_else(|| {
            infer_chat_turn_tool_loop_status(
                object
                    .get("tool_trace_count")
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as usize,
                tool_status_summary.as_ref(),
            )
        });

    let mut turn = contracts::ChatTurnRuntimeView {
        turn_id: object.get("turn_id")?.as_str()?.to_string(),
        status,
        stream_mode,
        stream_status,
        artifact_commit_status,
        provider_status,
        provider_failure,
        tool_loop_status,
        provider_request_id: object
            .get("provider_request_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        provider_requested_at,
        provider_responded_at,
        first_token_at: object
            .get("first_token_at")
            .and_then(parse_manifest_timestamp),
        stream_completed_at: object
            .get("stream_completed_at")
            .and_then(parse_manifest_timestamp),
        artifact_commit_ready_at,
        artifact_commit_failure_source,
        tool_calls_emitted_at: object
            .get("tool_calls_emitted_at")
            .and_then(parse_manifest_timestamp),
        tool_loop_settled_at: object
            .get("tool_loop_settled_at")
            .and_then(parse_manifest_timestamp),
        finish_reason,
        assistant_message_id: object
            .get("assistant_message_id")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .map(ChatMessageId::from),
        assistant_message_persisted_at,
        tool_trace_count: object.get("tool_trace_count")?.as_u64()? as usize,
        tool_status_summary,
        events: Vec::new(),
        started_at: parse_manifest_timestamp(object.get("started_at")?)?,
        completed_at,
    };
    if turn.tool_calls_emitted_at.is_none() && turn.tool_trace_count > 0 {
        turn.tool_calls_emitted_at = turn.provider_responded_at.or(turn.completed_at);
    }
    if turn.first_token_at.is_none() {
        turn.first_token_at = infer_chat_turn_first_token_at(&turn);
    }
    if turn.stream_completed_at.is_none() {
        turn.stream_completed_at = infer_chat_turn_stream_completed_at(&turn);
    }
    if turn.artifact_commit_ready_at.is_none() {
        turn.artifact_commit_ready_at = infer_chat_turn_artifact_commit_ready_at(&turn);
    }
    turn.artifact_commit_status = infer_chat_turn_artifact_commit_status(
        &turn.status,
        turn.assistant_message_persisted_at,
        turn.provider_responded_at,
        turn.artifact_commit_ready_at,
    );
    if !matches!(
        turn.artifact_commit_status,
        contracts::ChatTurnArtifactCommitStatusView::Failed
    ) {
        turn.artifact_commit_failure_source = None;
    }
    if turn.tool_loop_settled_at.is_none() {
        turn.tool_loop_settled_at = match turn.tool_loop_status {
            contracts::ChatTurnToolLoopStatusView::Completed
            | contracts::ChatTurnToolLoopStatusView::Failed => {
                turn.completed_at.or(turn.provider_responded_at)
            }
            contracts::ChatTurnToolLoopStatusView::NotRequested
            | contracts::ChatTurnToolLoopStatusView::Pending => None,
        };
    }
    turn.events = parse_chat_turn_events(object.get("events"), &turn)?;

    Some(turn)
}

fn parse_chat_session_manifest(value: &Value) -> Option<contracts::ChatSessionManifestView> {
    let object = value.as_object()?;
    let report_entry = object.get("report_entry").and_then(|value| {
        let entry = value.as_object()?;
        Some(contracts::ChatSessionReportEntryView {
            state: parse_model_facing_report_entry_state(entry.get("state")?.as_str()?)?,
            requested_at: entry.get("requested_at").and_then(parse_manifest_timestamp),
            resolved_at: entry.get("resolved_at").and_then(parse_manifest_timestamp),
            resolved_action: entry
                .get("resolved_action")
                .and_then(Value::as_str)
                .and_then(parse_chat_session_report_entry_resolution),
            suggested_title: entry
                .get("suggested_title")
                .and_then(Value::as_str)
                .map(str::to_string),
            suggested_objective: entry
                .get("suggested_objective")
                .and_then(Value::as_str)
                .map(str::to_string),
            confirmed_report_plan_id: entry
                .get("confirmed_report_plan_id")
                .and_then(Value::as_str)
                .and_then(|value| Uuid::parse_str(value).ok())
                .map(ReportPlanId::from),
        })
    });

    Some(contracts::ChatSessionManifestView {
        generator: object
            .get("generator")
            .and_then(Value::as_str)
            .map(str::to_string),
        schema_version: object
            .get("schema_version")
            .and_then(Value::as_str)
            .map(str::to_string),
        status: parse_chat_session_status(object.get("status")?.as_str()?)?,
        initial_prompt: object
            .get("initial_prompt")
            .and_then(Value::as_str)
            .map(str::to_string),
        last_prompt: object
            .get("last_prompt")
            .and_then(Value::as_str)
            .map(str::to_string),
        last_turn_kind: object
            .get("last_turn_kind")
            .and_then(Value::as_str)
            .and_then(parse_chat_session_turn_kind),
        context_binding: object
            .get("context_binding")
            .and_then(Value::as_str)
            .and_then(parse_manifest_context_binding),
        latest_memory_directory_id: object
            .get("latest_memory_directory_id")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .map(MemoryDirectoryId::from),
        latest_memory_directory_version_no: object
            .get("latest_memory_directory_version_no")
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok()),
        latest_dataset_output_id: object
            .get("latest_dataset_output_id")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .map(DatasetOutputId::from),
        report_entry,
        last_turn: object.get("last_turn").and_then(parse_chat_turn_runtime),
        runtime: object.get("runtime").and_then(parse_manifest_runtime),
    })
}

fn parse_chat_message_output(value: &Value) -> Option<contracts::ChatMessageOutputView> {
    let output = value.as_object()?;
    let parse_chat_message_output_format = |value: &str| match value {
        "markdown" => Some(contracts::ChatMessageOutputFormatView::Markdown),
        _ => None,
    };
    let parse_chat_message_section_kind = |value: &str| match value {
        "reply" => Some(contracts::ChatMessageSectionKindView::Reply),
        _ => None,
    };
    let sections = output
        .get("sections")?
        .as_array()?
        .iter()
        .map(|section| {
            let section = section.as_object()?;
            Some(contracts::ChatMessageSectionView {
                section_key: section.get("section_key")?.as_str()?.to_string(),
                kind: parse_chat_message_section_kind(section.get("kind")?.as_str()?)?,
                title: section.get("title")?.as_str()?.to_string(),
                content: section.get("content")?.as_str()?.to_string(),
                retrieval_evidence_ids: section
                    .get("retrieval_evidence_ids")?
                    .as_array()?
                    .iter()
                    .map(|evidence_id| {
                        let evidence_id = evidence_id.as_str()?;
                        Some(RetrievalEvidenceId::from(
                            Uuid::parse_str(evidence_id).ok()?,
                        ))
                    })
                    .collect::<Option<Vec<_>>>()?,
            })
        })
        .collect::<Option<Vec<_>>>()?;

    Some(contracts::ChatMessageOutputView {
        format: parse_chat_message_output_format(output.get("format")?.as_str()?)?,
        sections,
    })
}

fn parse_chat_message_manifest(value: &Value) -> Option<contracts::ChatMessageManifestView> {
    let object = value.as_object()?;
    let tool_trace = parse_manifest_tool_trace(object.get("tool_trace"))?;
    let legacy_tool_trace_count = Some(tool_trace.len());
    let runtime = object
        .get("runtime")
        .and_then(parse_manifest_runtime)
        .or_else(|| {
            object
                .get("generator")
                .and_then(Value::as_str)
                .filter(|generator| *generator == "chat-session-worker")
                .map(|_| contracts::ManifestRuntimeView {
                    mode: contracts::ManifestRuntimeModeView::Placeholder,
                    provider: None,
                    model: None,
                    request_id: None,
                    finish_reason: None,
                    provider_failure: None,
                    latency_ms: None,
                    usage: None,
                    system_prompt_key: None,
                    system_prompt_version: None,
                    tool_trace_count: legacy_tool_trace_count,
                })
        });

    Some(contracts::ChatMessageManifestView {
        generator: object.get("generator")?.as_str()?.to_string(),
        schema_version: object.get("schema_version")?.as_str()?.to_string(),
        dataset_id: DatasetId::from(Uuid::parse_str(object.get("dataset_id")?.as_str()?).ok()?),
        prompt: object.get("prompt")?.as_str()?.to_string(),
        indexed_document_count: object.get("indexed_document_count")?.as_u64()? as usize,
        refreshed_chunks: object.get("refreshed_chunks")?.as_u64()? as usize,
        prior_message_count: object.get("prior_message_count")?.as_u64()? as usize,
        latest_memory_directory_id: object
            .get("latest_memory_directory_id")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .map(MemoryDirectoryId::from),
        latest_memory_directory_version_no: object
            .get("latest_memory_directory_version_no")
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok()),
        latest_dataset_output_id: object
            .get("latest_dataset_output_id")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .map(DatasetOutputId::from),
        output: object.get("output").and_then(parse_chat_message_output),
        service_handoff: object
            .get("service_handoff")
            .and_then(parse_manifest_service_handoff),
        tool_trace,
        turn: object.get("turn").and_then(parse_chat_turn_runtime),
        context_binding: parse_manifest_context_binding(object.get("context_binding")?.as_str()?)?,
        runtime,
    })
}

fn manifest_finish_reason_from_invocation(
    reason: &contracts::LlmInvocationFinishReasonView,
) -> contracts::ManifestFinishReasonView {
    match reason {
        contracts::LlmInvocationFinishReasonView::Stop => contracts::ManifestFinishReasonView::Stop,
        contracts::LlmInvocationFinishReasonView::ToolCalls => {
            contracts::ManifestFinishReasonView::ToolCalls
        }
        contracts::LlmInvocationFinishReasonView::Length => {
            contracts::ManifestFinishReasonView::Length
        }
        contracts::LlmInvocationFinishReasonView::ContentFilter => {
            contracts::ManifestFinishReasonView::ContentFilter
        }
        contracts::LlmInvocationFinishReasonView::Error => {
            contracts::ManifestFinishReasonView::Error
        }
        contracts::LlmInvocationFinishReasonView::Other(value) => {
            contracts::ManifestFinishReasonView::Other(value.clone())
        }
    }
}

fn latest_llm_invocation(llm_invocations: &[LlmInvocationView]) -> Option<&LlmInvocationView> {
    llm_invocations
        .iter()
        .max_by_key(|invocation| invocation.sequence_no)
}

fn manifest_runtime_from_latest_llm_invocation(
    llm_invocations: &[LlmInvocationView],
) -> Option<contracts::ManifestRuntimeView> {
    let latest = latest_llm_invocation(llm_invocations)?;

    Some(contracts::ManifestRuntimeView {
        mode: match latest.mode {
            contracts::LlmInvocationModeView::Placeholder => {
                contracts::ManifestRuntimeModeView::Placeholder
            }
            contracts::LlmInvocationModeView::Provider => {
                contracts::ManifestRuntimeModeView::Provider
            }
        },
        provider: latest.provider.clone(),
        model: latest.model.clone(),
        request_id: latest.request_id.clone(),
        finish_reason: latest
            .finish_reason
            .as_ref()
            .map(manifest_finish_reason_from_invocation),
        provider_failure: None,
        latency_ms: latest.latency_ms,
        usage: latest
            .usage
            .as_ref()
            .map(|usage| contracts::ManifestTokenUsageView {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                total_tokens: usage.total_tokens,
            }),
        system_prompt_key: latest.system_prompt_key.clone(),
        system_prompt_version: latest.system_prompt_version.clone(),
        tool_trace_count: latest.tool_trace_count,
    })
}

fn hydrate_chat_turn_from_latest_llm_invocation(
    turn: &mut contracts::ChatTurnRuntimeView,
    llm_invocations: &[LlmInvocationView],
) {
    let Some(latest) = latest_llm_invocation(llm_invocations) else {
        return;
    };

    turn.provider_request_id = latest.request_id.clone();
    turn.finish_reason = latest
        .finish_reason
        .as_ref()
        .map(manifest_finish_reason_from_invocation);
    turn.provider_status = match latest.finish_reason.as_ref() {
        Some(contracts::LlmInvocationFinishReasonView::Error) => {
            contracts::ChatTurnProviderStatusView::Failed
        }
        _ => contracts::ChatTurnProviderStatusView::Responded,
    };
    if let Some(tool_trace_count) = latest.tool_trace_count {
        turn.tool_trace_count = tool_trace_count;
    }
    turn.stream_status = infer_chat_turn_stream_status(
        &turn.stream_mode,
        &turn.provider_status,
        turn.provider_requested_at,
        turn.provider_responded_at,
        turn.completed_at,
    );
    if turn.first_token_at.is_none() {
        turn.first_token_at = infer_chat_turn_first_token_at(turn);
    }
    if turn.stream_completed_at.is_none() {
        turn.stream_completed_at = infer_chat_turn_stream_completed_at(turn);
    }
    if turn.artifact_commit_ready_at.is_none() {
        turn.artifact_commit_ready_at = infer_chat_turn_artifact_commit_ready_at(turn);
    }
    turn.artifact_commit_status = infer_chat_turn_artifact_commit_status(
        &turn.status,
        turn.assistant_message_persisted_at,
        turn.provider_responded_at,
        turn.artifact_commit_ready_at,
    );
    if !matches!(
        turn.artifact_commit_status,
        contracts::ChatTurnArtifactCommitStatusView::Failed
    ) {
        turn.artifact_commit_failure_source = None;
    }
    if turn.tool_trace_count > 0 && turn.tool_calls_emitted_at.is_none() {
        turn.tool_calls_emitted_at = turn.provider_responded_at.or(turn.completed_at);
    }
    turn.tool_loop_status =
        infer_chat_turn_tool_loop_status(turn.tool_trace_count, turn.tool_status_summary.as_ref());
    if matches!(
        turn.tool_loop_status,
        contracts::ChatTurnToolLoopStatusView::Completed
            | contracts::ChatTurnToolLoopStatusView::Failed
    ) && turn.tool_loop_settled_at.is_none()
    {
        turn.tool_loop_settled_at = turn.completed_at.or(turn.provider_responded_at);
    }
    turn.events = build_chat_turn_events(turn);
}

fn manifest_tool_trace_from_tool_executions(
    tool_executions: &[ToolExecutionView],
) -> Vec<contracts::ManifestToolCallView> {
    let mut trace = tool_executions.to_vec();
    trace.sort_by_key(|execution| (execution.sequence_no, execution.created_at));
    trace
        .into_iter()
        .map(|execution| contracts::ManifestToolCallView {
            call_id: execution.call_id,
            tool_name: execution.tool_name,
            tool: execution.tool,
            status: execution.status,
            arguments: execution.arguments,
            result: execution.result,
        })
        .collect()
}

fn summarize_tool_execution_statuses(
    tool_executions: &[ToolExecutionView],
) -> Option<contracts::ChatTurnToolStatusSummaryView> {
    if tool_executions.is_empty() {
        return None;
    }

    let mut requested_count = 0usize;
    let mut completed_count = 0usize;
    let mut failed_count = 0usize;

    for execution in tool_executions {
        match execution.status {
            contracts::ManifestToolCallStatusView::Requested => requested_count += 1,
            contracts::ManifestToolCallStatusView::Completed => completed_count += 1,
            contracts::ManifestToolCallStatusView::Failed => failed_count += 1,
        }
    }

    Some(contracts::ChatTurnToolStatusSummaryView {
        requested_count,
        completed_count,
        failed_count,
    })
}

fn infer_chat_turn_tool_loop_status(
    tool_trace_count: usize,
    summary: Option<&contracts::ChatTurnToolStatusSummaryView>,
) -> contracts::ChatTurnToolLoopStatusView {
    let Some(summary) = summary else {
        return if tool_trace_count > 0 {
            contracts::ChatTurnToolLoopStatusView::Pending
        } else {
            contracts::ChatTurnToolLoopStatusView::NotRequested
        };
    };

    if summary.requested_count > 0 {
        contracts::ChatTurnToolLoopStatusView::Pending
    } else if summary.failed_count > 0 {
        contracts::ChatTurnToolLoopStatusView::Failed
    } else if summary.completed_count > 0 || tool_trace_count > 0 {
        contracts::ChatTurnToolLoopStatusView::Completed
    } else {
        contracts::ChatTurnToolLoopStatusView::NotRequested
    }
}

fn infer_chat_turn_stream_status(
    stream_mode: &contracts::ChatTurnStreamModeView,
    provider_status: &contracts::ChatTurnProviderStatusView,
    provider_requested_at: Option<DateTime<Utc>>,
    provider_responded_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
) -> contracts::ChatTurnStreamStatusView {
    if matches!(stream_mode, contracts::ChatTurnStreamModeView::Buffered) {
        return contracts::ChatTurnStreamStatusView::NotRequested;
    }

    match provider_status {
        contracts::ChatTurnProviderStatusView::Failed => {
            contracts::ChatTurnStreamStatusView::Failed
        }
        contracts::ChatTurnProviderStatusView::Responded => {
            if provider_responded_at.is_some() || completed_at.is_some() {
                contracts::ChatTurnStreamStatusView::Completed
            } else {
                contracts::ChatTurnStreamStatusView::Pending
            }
        }
        contracts::ChatTurnProviderStatusView::Pending => {
            if provider_requested_at.is_some() {
                contracts::ChatTurnStreamStatusView::Pending
            } else {
                contracts::ChatTurnStreamStatusView::NotRequested
            }
        }
    }
}

fn infer_chat_turn_first_token_at(turn: &contracts::ChatTurnRuntimeView) -> Option<DateTime<Utc>> {
    if matches!(
        turn.stream_mode,
        contracts::ChatTurnStreamModeView::Buffered
    ) {
        return None;
    }

    match turn.stream_status {
        contracts::ChatTurnStreamStatusView::Completed => {
            turn.provider_responded_at.or(turn.completed_at)
        }
        contracts::ChatTurnStreamStatusView::NotRequested
        | contracts::ChatTurnStreamStatusView::Pending
        | contracts::ChatTurnStreamStatusView::Failed => None,
    }
}

fn infer_chat_turn_stream_completed_at(
    turn: &contracts::ChatTurnRuntimeView,
) -> Option<DateTime<Utc>> {
    if matches!(
        turn.stream_mode,
        contracts::ChatTurnStreamModeView::Buffered
    ) {
        return None;
    }

    match turn.stream_status {
        contracts::ChatTurnStreamStatusView::Completed
        | contracts::ChatTurnStreamStatusView::Failed => {
            turn.provider_responded_at.or(turn.completed_at)
        }
        contracts::ChatTurnStreamStatusView::NotRequested
        | contracts::ChatTurnStreamStatusView::Pending => None,
    }
}

fn infer_chat_turn_artifact_commit_ready_at(
    turn: &contracts::ChatTurnRuntimeView,
) -> Option<DateTime<Utc>> {
    if matches!(
        turn.stream_mode,
        contracts::ChatTurnStreamModeView::Buffered
    ) {
        match turn.provider_status {
            contracts::ChatTurnProviderStatusView::Responded => {
                return turn.provider_responded_at.or(turn.completed_at);
            }
            contracts::ChatTurnProviderStatusView::Pending
            | contracts::ChatTurnProviderStatusView::Failed => return None,
        }
    }

    match turn.stream_status {
        contracts::ChatTurnStreamStatusView::Completed => turn
            .stream_completed_at
            .or(turn.provider_responded_at)
            .or(turn.completed_at),
        contracts::ChatTurnStreamStatusView::NotRequested
        | contracts::ChatTurnStreamStatusView::Pending
        | contracts::ChatTurnStreamStatusView::Failed => None,
    }
}

fn infer_chat_turn_artifact_commit_status(
    status: &contracts::ChatTurnStatusView,
    assistant_message_persisted_at: Option<DateTime<Utc>>,
    provider_responded_at: Option<DateTime<Utc>>,
    artifact_commit_ready_at: Option<DateTime<Utc>>,
) -> contracts::ChatTurnArtifactCommitStatusView {
    if assistant_message_persisted_at.is_some() {
        contracts::ChatTurnArtifactCommitStatusView::Completed
    } else if matches!(status, contracts::ChatTurnStatusView::Failed)
        && artifact_commit_ready_at.is_some()
    {
        contracts::ChatTurnArtifactCommitStatusView::Failed
    } else if artifact_commit_ready_at.is_some() || provider_responded_at.is_some() {
        contracts::ChatTurnArtifactCommitStatusView::Pending
    } else {
        contracts::ChatTurnArtifactCommitStatusView::NotReady
    }
}

fn infer_chat_turn_tool_loop_settled_at(
    turn: &contracts::ChatTurnRuntimeView,
    tool_executions: &[ToolExecutionView],
) -> Option<DateTime<Utc>> {
    match turn.tool_loop_status {
        contracts::ChatTurnToolLoopStatusView::Completed
        | contracts::ChatTurnToolLoopStatusView::Failed => tool_executions
            .iter()
            .map(|execution| execution.created_at)
            .max()
            .or(turn.tool_loop_settled_at)
            .or(turn.completed_at)
            .or(turn.provider_responded_at),
        contracts::ChatTurnToolLoopStatusView::NotRequested
        | contracts::ChatTurnToolLoopStatusView::Pending => None,
    }
}

fn hydrate_dataset_output_manifest_view(
    value: &Value,
    llm_invocations: &[LlmInvocationView],
    tool_executions: &[ToolExecutionView],
) -> Option<contracts::DatasetOutputManifestView> {
    let mut view = parse_dataset_output_manifest(value)?;
    if let Some(runtime) = manifest_runtime_from_latest_llm_invocation(llm_invocations) {
        view.runtime = Some(runtime);
    }
    if !tool_executions.is_empty() {
        view.tool_trace = manifest_tool_trace_from_tool_executions(tool_executions);
        if let Some(runtime) = view.runtime.as_mut() {
            runtime.tool_trace_count = Some(tool_executions.len());
        }
    }
    Some(view)
}

fn hydrate_chat_message_manifest_view(
    value: &Value,
    llm_invocations: &[LlmInvocationView],
    tool_executions: &[ToolExecutionView],
) -> Option<contracts::ChatMessageManifestView> {
    let mut view = parse_chat_message_manifest(value)?;
    if let Some(runtime) = manifest_runtime_from_latest_llm_invocation(llm_invocations) {
        view.runtime = Some(runtime);
    }
    if !tool_executions.is_empty() {
        view.tool_trace = manifest_tool_trace_from_tool_executions(tool_executions);
        if let Some(runtime) = view.runtime.as_mut() {
            runtime.tool_trace_count = Some(tool_executions.len());
        }
    }
    if let Some(turn) = view.turn.as_mut() {
        if turn.finish_reason.is_none() {
            turn.finish_reason = view
                .runtime
                .as_ref()
                .and_then(|runtime| runtime.finish_reason.clone());
        }
        if matches!(
            turn.provider_status,
            contracts::ChatTurnProviderStatusView::Pending
        ) {
            turn.provider_status = match turn.finish_reason {
                Some(contracts::ManifestFinishReasonView::Error) => {
                    contracts::ChatTurnProviderStatusView::Failed
                }
                Some(_) => contracts::ChatTurnProviderStatusView::Responded,
                None => turn.provider_status.clone(),
            };
        }
        turn.events = build_chat_turn_events(turn);
        hydrate_chat_turn_from_latest_llm_invocation(turn, llm_invocations);
        if !tool_executions.is_empty() {
            turn.tool_trace_count = tool_executions.len();
            turn.tool_status_summary = summarize_tool_execution_statuses(tool_executions);
            turn.tool_loop_status = infer_chat_turn_tool_loop_status(
                turn.tool_trace_count,
                turn.tool_status_summary.as_ref(),
            );
            if turn.tool_calls_emitted_at.is_none() {
                turn.tool_calls_emitted_at = tool_executions
                    .iter()
                    .map(|execution| execution.created_at)
                    .min()
                    .or(turn.provider_responded_at);
            }
            turn.tool_loop_settled_at = infer_chat_turn_tool_loop_settled_at(turn, tool_executions);
            turn.events = build_chat_turn_events(turn);
        }
        if turn.stream_status == contracts::ChatTurnStreamStatusView::NotRequested
            && matches!(
                turn.stream_mode,
                contracts::ChatTurnStreamModeView::Streaming
            )
        {
            turn.stream_status = infer_chat_turn_stream_status(
                &turn.stream_mode,
                &turn.provider_status,
                turn.provider_requested_at,
                turn.provider_responded_at,
                turn.completed_at,
            );
        }
        if turn.first_token_at.is_none() {
            turn.first_token_at = infer_chat_turn_first_token_at(turn);
        }
        if turn.stream_completed_at.is_none() {
            turn.stream_completed_at = infer_chat_turn_stream_completed_at(turn);
        }
    }
    Some(view)
}

fn hydrate_chat_session_manifest_view(
    manifest: &Value,
    latest_assistant_message: Option<&ChatMessageView>,
) -> Option<contracts::ChatSessionManifestView> {
    let mut view = parse_chat_session_manifest(manifest)?;
    if view.status != contracts::ChatSessionManifestStatusView::AssistantReplied {
        view.runtime = None;
        return Some(view);
    }

    let latest_message_manifest = latest_assistant_message.and_then(|message| {
        let mut manifest_view = message.message_manifest_view.clone().or_else(|| {
            hydrate_chat_message_manifest_view(
                &message.message_manifest,
                &message.llm_invocations,
                &message.tool_executions,
            )
        });
        hydrate_assistant_turn_from_message_metadata(
            message.id,
            message.created_at,
            &mut manifest_view,
        );
        manifest_view
    });

    if let Some(message_manifest) = latest_message_manifest {
        view.last_turn = message_manifest.turn;
    }
    // Completed sessions should source provider runtime from the assistant message artifact.
    // Keep the session-level turn summary for polling/read-model continuity, but stop
    // duplicating runtime metadata once the assistant message has been committed.
    view.runtime = None;

    Some(view)
}

fn to_dataset_output_view(
    output: DatasetOutput,
    memory_directory: Option<MemoryDirectoryView>,
    retrieval_evidences: Vec<RetrievalEvidenceView>,
    llm_invocations: Vec<LlmInvocationView>,
    tool_executions: Vec<ToolExecutionView>,
) -> DatasetOutputView {
    let output_manifest_view = hydrate_dataset_output_manifest_view(
        &output.output_manifest,
        &llm_invocations,
        &tool_executions,
    );

    DatasetOutputView {
        id: output.id,
        dataset_id: output.dataset_id,
        execution_id: output.execution_id,
        prompt: output.prompt,
        output_text: output.output_text,
        memory_directory_id: output.memory_directory_id,
        memory_directory,
        retrieval_evidence_ids: output.retrieval_evidence_ids,
        retrieval_evidences,
        llm_invocations,
        tool_executions,
        output_manifest_view,
        output_manifest: output.output_manifest,
        model_facing: None,
        created_at: output.created_at,
    }
}

fn to_chat_session_view(
    session: ChatSession,
    latest_memory_directory: Option<MemoryDirectoryView>,
    latest_dataset_output: Option<DatasetOutputView>,
    latest_assistant_message: Option<ChatMessageView>,
) -> ChatSessionView {
    let session_manifest_view = hydrate_chat_session_manifest_view(
        &session.session_manifest,
        latest_assistant_message.as_ref(),
    );

    ChatSessionView {
        id: session.id,
        dataset_id: session.dataset_id,
        execution_id: session.execution_id,
        title: session.title,
        latest_memory_directory_id: session.latest_memory_directory_id,
        latest_memory_directory,
        latest_dataset_output_id: session.latest_dataset_output_id,
        latest_dataset_output,
        latest_assistant_message_id: latest_assistant_message.as_ref().map(|message| message.id),
        latest_assistant_message,
        session_manifest_view,
        session_manifest: session.session_manifest,
        model_facing: None,
        created_at: session.created_at,
        updated_at: session.updated_at,
    }
}

async fn hydrate_dataset_output_view(
    state: &AppState,
    output: DatasetOutput,
) -> std::result::Result<DatasetOutputView, ApiError> {
    let llm_invocations = state
        .storage
        .llm_invocations()
        .list_by_dataset_output(state.tenant_id, output.id)
        .await
        .map_err(ApiError::from_storage)?
        .into_iter()
        .map(to_llm_invocation_view)
        .collect();
    let tool_executions = state
        .storage
        .tool_executions()
        .list_by_dataset_output(state.tenant_id, output.id)
        .await
        .map_err(ApiError::from_storage)?
        .into_iter()
        .map(to_tool_execution_view)
        .collect();
    let memory_directory = match output.memory_directory_id {
        Some(directory_id) => state
            .storage
            .memory_directories()
            .get_by_id(state.tenant_id, directory_id)
            .await
            .map_err(ApiError::from_storage)?
            .map(to_memory_directory_view),
        None => None,
    };
    let retrieval_evidences = state
        .storage
        .retrieval_evidences()
        .list_by_ids(state.tenant_id, &output.retrieval_evidence_ids)
        .await
        .map_err(ApiError::from_storage)?
        .into_iter()
        .map(to_retrieval_evidence_view)
        .collect();

    let mut view = to_dataset_output_view(
        output,
        memory_directory,
        retrieval_evidences,
        llm_invocations,
        tool_executions,
    );
    view.model_facing = Some(derive_dataset_output_model_facing_summary(&view));

    Ok(view)
}

async fn hydrate_chat_session_view(
    state: &AppState,
    session: ChatSession,
) -> std::result::Result<ChatSessionView, ApiError> {
    let latest_assistant_message = state
        .storage
        .chat_messages()
        .list_by_session(state.tenant_id, session.id)
        .await
        .map_err(ApiError::from_storage)?
        .into_iter()
        .rev()
        .find(|message| message.role == ChatMessageRole::Assistant);
    let latest_assistant_message = match latest_assistant_message {
        Some(message) => Some(hydrate_chat_message_view(state, message).await?),
        None => None,
    };

    hydrate_chat_session_view_with_latest_assistant_message(
        state,
        session,
        latest_assistant_message,
    )
    .await
}

async fn hydrate_chat_session_view_with_latest_assistant_message(
    state: &AppState,
    session: ChatSession,
    latest_assistant_message: Option<ChatMessageView>,
) -> std::result::Result<ChatSessionView, ApiError> {
    let latest_memory_directory = match session.latest_memory_directory_id {
        Some(directory_id) => state
            .storage
            .memory_directories()
            .get_by_id(state.tenant_id, directory_id)
            .await
            .map_err(ApiError::from_storage)?
            .map(to_memory_directory_view),
        None => None,
    };
    let latest_dataset_output = match session.latest_dataset_output_id {
        Some(output_id) => {
            let output = state
                .storage
                .dataset_outputs()
                .get_by_id(state.tenant_id, output_id)
                .await
                .map_err(ApiError::from_storage)?;
            match output {
                Some(output) => Some(hydrate_dataset_output_view(state, output).await?),
                None => None,
            }
        }
        None => None,
    };

    let mut view = to_chat_session_view(
        session,
        latest_memory_directory,
        latest_dataset_output,
        latest_assistant_message,
    );
    view.model_facing = Some(derive_chat_session_model_facing_summary(&view));

    Ok(view)
}

async fn load_chat_message_views_for_session(
    state: &AppState,
    session_id: ChatSessionId,
) -> std::result::Result<Vec<ChatMessageView>, ApiError> {
    let messages = state
        .storage
        .chat_messages()
        .list_by_session(state.tenant_id, session_id)
        .await
        .map_err(ApiError::from_storage)?;
    let mut views = Vec::with_capacity(messages.len());
    for message in messages {
        views.push(hydrate_chat_message_view(state, message).await?);
    }
    Ok(views)
}

fn latest_assistant_message_from_views(messages: &[ChatMessageView]) -> Option<ChatMessageView> {
    messages
        .iter()
        .rev()
        .find(|message| message.role == ChatMessageRole::Assistant)
        .cloned()
}

fn to_chat_message_view(
    message: ChatMessage,
    llm_invocations: Vec<LlmInvocationView>,
    tool_executions: Vec<ToolExecutionView>,
) -> ChatMessageView {
    let mut message_manifest_view = hydrate_chat_message_manifest_view(
        &message.message_manifest,
        &llm_invocations,
        &tool_executions,
    );
    hydrate_assistant_turn_from_message_record(&message, &mut message_manifest_view);

    let mut view = ChatMessageView {
        id: message.id,
        session_id: message.session_id,
        role: message.role,
        turn_index: message.turn_index,
        content: message.content,
        llm_invocations,
        tool_executions,
        message_manifest_view,
        message_manifest: message.message_manifest,
        model_facing: None,
        created_at: message.created_at,
    };
    view.model_facing = derive_chat_message_model_facing_summary(&view);

    view
}

fn hydrate_assistant_turn_from_message_record(
    message: &ChatMessage,
    manifest_view: &mut Option<contracts::ChatMessageManifestView>,
) {
    if !matches!(message.role, ChatMessageRole::Assistant) {
        return;
    }

    hydrate_assistant_turn_from_message_metadata(message.id, message.created_at, manifest_view);
}

fn hydrate_assistant_turn_from_message_metadata(
    message_id: ChatMessageId,
    persisted_at: DateTime<Utc>,
    manifest_view: &mut Option<contracts::ChatMessageManifestView>,
) {
    let Some(manifest_view) = manifest_view.as_mut() else {
        return;
    };
    let Some(turn) = manifest_view.turn.as_mut() else {
        return;
    };

    if turn.assistant_message_id.is_none() {
        turn.assistant_message_id = Some(message_id);
    }
    if turn.assistant_message_persisted_at.is_none() {
        turn.assistant_message_persisted_at = Some(persisted_at);
    }
    if turn.completed_at.is_none() {
        turn.completed_at = turn.assistant_message_persisted_at;
    }
    if turn.artifact_commit_ready_at.is_none() {
        turn.artifact_commit_ready_at = infer_chat_turn_artifact_commit_ready_at(turn);
    }
    turn.artifact_commit_status = infer_chat_turn_artifact_commit_status(
        &turn.status,
        turn.assistant_message_persisted_at,
        turn.provider_responded_at,
        turn.artifact_commit_ready_at,
    );
    if !matches!(
        turn.artifact_commit_status,
        contracts::ChatTurnArtifactCommitStatusView::Failed
    ) {
        turn.artifact_commit_failure_source = None;
    }
    turn.events = build_chat_turn_events(turn);
}

async fn hydrate_chat_message_view(
    state: &AppState,
    message: ChatMessage,
) -> std::result::Result<ChatMessageView, ApiError> {
    let llm_invocations = state
        .storage
        .llm_invocations()
        .list_by_chat_message(state.tenant_id, message.id)
        .await
        .map_err(ApiError::from_storage)?
        .into_iter()
        .map(to_llm_invocation_view)
        .collect();
    let tool_executions = state
        .storage
        .tool_executions()
        .list_by_chat_message(state.tenant_id, message.id)
        .await
        .map_err(ApiError::from_storage)?
        .into_iter()
        .map(to_tool_execution_view)
        .collect();

    Ok(to_chat_message_view(
        message,
        llm_invocations,
        tool_executions,
    ))
}

fn to_report_plan_ast_version_view(version: ReportPlanAstVersion) -> ReportPlanAstVersionView {
    ReportPlanAstVersionView {
        id: version.id,
        plan_id: version.plan_id,
        version_no: version.version_no,
        ast: version.ast,
        created_at: version.created_at,
    }
}

async fn hydrate_report_render_output_view(
    state: &AppState,
    output: ReportRenderOutput,
) -> std::result::Result<ReportRenderOutputView, ApiError> {
    let service_handoff = state
        .storage
        .workflow_executions()
        .get_by_id(state.tenant_id, output.execution_id)
        .await
        .map_err(ApiError::from_storage)?
        .as_ref()
        .and_then(workflow_execution_context_service_handoff);
    Ok(to_report_render_output_view(output, service_handoff))
}

fn to_report_render_output_view(
    output: ReportRenderOutput,
    service_handoff: Option<contracts::ManifestServiceHandoffView>,
) -> ReportRenderOutputView {
    let mut view = ReportRenderOutputView {
        id: output.id,
        execution_id: output.execution_id,
        plan_id: output.plan_id,
        dataset_id: output.dataset_id,
        ast_version_id: output.ast_version_id,
        surface: output.surface,
        status: contracts::ReportRenderOutputStatusView::from_domain(output.status),
        asset_manifest: output.asset_manifest,
        service_handoff,
        model_facing: None,
        created_at: output.created_at,
    };
    view.model_facing = Some(derive_report_render_output_model_facing_summary(&view));
    view
}

fn to_published_report_view(report: PublishedReport) -> PublishedReportView {
    PublishedReportView {
        id: report.id,
        dataset_id: report.dataset_id,
        plan_id: report.plan_id,
        slug: report.slug,
        current_version_id: report.current_version_id,
        created_at: report.created_at,
        updated_at: report.updated_at,
    }
}

fn to_published_report_version_view(version: PublishedReportVersion) -> PublishedReportVersionView {
    PublishedReportVersionView {
        id: version.id,
        report_id: version.report_id,
        version_no: version.version_no,
        surface: version.surface,
        asset_manifest: version.asset_manifest,
        created_at: version.created_at,
    }
}

async fn load_published_report_detail_with_state(
    state: &AppState,
    report_id: PublishedReportId,
    active_secret_binding_ids: &[SecretBindingId],
) -> std::result::Result<PublishedReportDetailView, ApiError> {
    let report = state
        .storage
        .published_reports()
        .get_by_id(state.tenant_id, report_id)
        .await
        .map_err(ApiError::from_storage)?
        .ok_or_else(|| {
            ApiError::not_found(
                "published_report_not_found",
                format!("published report {} was not found", report_id),
            )
        })?;
    load_visible_dataset(state, report.dataset_id, active_secret_binding_ids).await?;
    hydrate_published_report_detail_view(state, report).await
}

async fn load_published_report_detail_by_plan_with_state(
    state: &AppState,
    plan_id: ReportPlanId,
    active_secret_binding_ids: &[SecretBindingId],
) -> std::result::Result<PublishedReportDetailView, ApiError> {
    let plan = load_visible_report_plan(state, plan_id, active_secret_binding_ids).await?;
    let report = state
        .storage
        .published_reports()
        .get_by_plan(state.tenant_id, plan.id)
        .await
        .map_err(ApiError::from_storage)?
        .ok_or_else(|| {
            ApiError::not_found(
                "published_report_not_found",
                format!("report plan {} does not have a published report", plan_id),
            )
        })?;
    hydrate_published_report_detail_view(state, report).await
}

async fn hydrate_published_report_detail_view(
    state: &AppState,
    report: PublishedReport,
) -> std::result::Result<PublishedReportDetailView, ApiError> {
    let versions = state
        .storage
        .published_report_versions()
        .list_by_report(state.tenant_id, report.id)
        .await
        .map_err(ApiError::from_storage)?
        .into_iter()
        .map(to_published_report_version_view)
        .collect::<Vec<_>>();
    let current_version = report.current_version_id.and_then(|current_version_id| {
        versions
            .iter()
            .find(|version| version.id == current_version_id)
            .cloned()
    });

    Ok(PublishedReportDetailView {
        report: to_published_report_view(report),
        current_version,
        versions,
    })
}

fn build_published_report_slug(plan: &ReportPlan) -> String {
    let base = slugify_report_title(&plan.title);
    let suffix = plan
        .id
        .to_string()
        .chars()
        .filter(|value| *value != '-')
        .take(8)
        .collect::<String>();
    format!("{base}-{suffix}")
}

fn slugify_report_title(title: &str) -> String {
    let mut slug = String::new();
    let mut last_was_separator = false;

    for character in title.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
            last_was_separator = false;
            continue;
        }
        if slug.is_empty() || last_was_separator {
            continue;
        }
        slug.push('-');
        last_was_separator = true;
    }

    while slug.ends_with('-') {
        slug.pop();
    }

    if slug.is_empty() {
        "report".to_string()
    } else {
        slug
    }
}

fn build_published_report_version_manifest(
    render_output: &ReportRenderOutput,
    publish_note: Option<&str>,
    published_at: DateTime<Utc>,
) -> Value {
    let publish_metadata = json!({
        "published_at": published_at,
        "source_render_output_id": render_output.id,
        "source_execution_id": render_output.execution_id,
        "publish_note": publish_note,
    });

    match render_output.asset_manifest.clone() {
        Value::Object(mut manifest) => {
            manifest.insert("publish_metadata".to_string(), publish_metadata);
            Value::Object(manifest)
        }
        other => json!({
            "render_asset_manifest": other,
            "publish_metadata": publish_metadata,
        }),
    }
}

fn to_workflow_execution_view(execution: WorkflowExecution) -> WorkflowExecutionView {
    WorkflowExecutionView {
        id: execution.id,
        kind: execution.kind,
        status: execution.status,
        stage: execution.stage,
        updated_at: execution.updated_at,
    }
}

fn value_array(value: Value) -> Vec<Value> {
    match value {
        Value::Array(items) => items,
        _ => Vec::new(),
    }
}

fn to_assistant_run_view(run: AssistantRun) -> AssistantRunView {
    AssistantRunView {
        id: run.id,
        local_thread_id: run.local_thread_id,
        user_prompt: run.user_prompt,
        startup_briefing: run.startup_briefing,
        selected_scope: run.selected_scope,
        scope_candidates: value_array(run.scope_candidates),
        context_policy: run.context_policy,
        evidence_state: run.evidence_state,
        service_lane: run.service_lane,
        execution_trail: value_array(run.execution_trail),
        output_artifacts: value_array(run.output_artifacts),
        runtime: run.runtime_manifest,
        created_at: run.created_at,
        updated_at: run.updated_at,
    }
}

fn to_assistant_run_event_view(event: AssistantRunEvent) -> AssistantRunEventView {
    AssistantRunEventView {
        id: event.id,
        run_id: event.run_id,
        sequence_no: event.sequence_no,
        event_name: event.event_name,
        payload: event.payload,
        created_at: event.created_at,
    }
}

fn to_static_page_draft_view(draft: StaticPageDraft) -> StaticPageDraftView {
    StaticPageDraftView {
        id: draft.id,
        assistant_run_id: draft.assistant_run_id,
        title: draft.title,
        status: contracts::StaticPageDraftStatusView::from_domain(draft.status),
        selected_scope: draft.selected_scope,
        visibility_snapshot: draft.visibility_snapshot,
        source_refs: draft.source_refs,
        draft_payload: draft.draft_payload,
        created_at: draft.created_at,
        updated_at: draft.updated_at,
    }
}

fn to_static_page_image_job_view(job: StaticPageImageJob) -> StaticPageImageJobView {
    StaticPageImageJobView {
        id: job.id,
        draft_id: job.draft_id,
        assistant_run_id: job.assistant_run_id,
        status: contracts::StaticPageImageJobStatusView::from_domain(job.status),
        queue_position: job.queue_position,
        image_prompt_payload: job.image_prompt_payload,
        preview_asset_key: job.preview_asset_key,
        failure_reason: job.failure_reason,
        confirmed_at: job.confirmed_at,
        created_at: job.created_at,
        updated_at: job.updated_at,
    }
}

fn to_static_page_render_output_view(output: StaticPageRenderOutput) -> StaticPageRenderOutputView {
    StaticPageRenderOutputView {
        id: output.id,
        draft_id: output.draft_id,
        assistant_run_id: output.assistant_run_id,
        image_job_id: output.image_job_id,
        status: contracts::StaticPageRenderOutputStatusView::from_domain(output.status),
        html: output.html,
        asset_manifest: output.asset_manifest,
        created_at: output.created_at,
    }
}

fn to_conversation_memory_item_view(item: ConversationMemoryItem) -> ConversationMemoryItemView {
    ConversationMemoryItemView {
        id: item.id,
        local_thread_id: item.local_thread_id,
        role: item.role,
        item_kind: item.item_kind,
        summary: item.summary,
        source_message_refs: item.source_message_refs,
        artifact_refs: item.artifact_refs,
        metadata: item.metadata,
        created_at: item.created_at,
        updated_at: item.updated_at,
    }
}

fn to_workflow_event_view(event: WorkflowEventRecord) -> WorkflowEventView {
    WorkflowEventView {
        id: event.id,
        sequence_no: event.sequence_no,
        event_name: event.event_name,
        payload: event.payload,
        created_at: event.created_at,
    }
}

fn to_workflow_task_view(task: WorkflowTask) -> WorkflowTaskView {
    WorkflowTaskView {
        id: task.id,
        status: task.status,
        queue: task.queue,
        task_key: task.task_key,
        attempt: task.attempt,
        available_at: task.available_at,
        payload: task.payload,
    }
}

fn to_llm_invocation_view(llm_invocation: LlmInvocation) -> LlmInvocationView {
    LlmInvocationView {
        id: llm_invocation.id,
        execution_id: llm_invocation.execution_id,
        source_kind: match llm_invocation.source_kind {
            LlmInvocationSourceKind::DatasetOutput => {
                contracts::LlmInvocationSourceKindView::DatasetOutput
            }
            LlmInvocationSourceKind::ChatMessage => {
                contracts::LlmInvocationSourceKindView::ChatMessage
            }
            LlmInvocationSourceKind::WorkflowExecution => {
                contracts::LlmInvocationSourceKindView::WorkflowExecution
            }
        },
        dataset_output_id: llm_invocation.dataset_output_id,
        chat_message_id: llm_invocation.chat_message_id,
        sequence_no: llm_invocation.sequence_no,
        mode: match llm_invocation.mode {
            LlmInvocationMode::Placeholder => contracts::LlmInvocationModeView::Placeholder,
            LlmInvocationMode::Provider => contracts::LlmInvocationModeView::Provider,
        },
        provider: llm_invocation.provider,
        model: llm_invocation.model,
        request_id: llm_invocation.request_id,
        finish_reason: llm_invocation.finish_reason.map(|reason| match reason {
            LlmInvocationFinishReason::Stop => contracts::LlmInvocationFinishReasonView::Stop,
            LlmInvocationFinishReason::ToolCalls => {
                contracts::LlmInvocationFinishReasonView::ToolCalls
            }
            LlmInvocationFinishReason::Length => contracts::LlmInvocationFinishReasonView::Length,
            LlmInvocationFinishReason::ContentFilter => {
                contracts::LlmInvocationFinishReasonView::ContentFilter
            }
            LlmInvocationFinishReason::Error => contracts::LlmInvocationFinishReasonView::Error,
            LlmInvocationFinishReason::Other(value) => {
                contracts::LlmInvocationFinishReasonView::Other(value)
            }
        }),
        latency_ms: llm_invocation.latency_ms,
        usage: llm_invocation
            .usage
            .map(|usage| contracts::LlmTokenUsageView {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                total_tokens: usage.total_tokens,
            }),
        system_prompt_key: llm_invocation.system_prompt_key,
        system_prompt_version: llm_invocation.system_prompt_version,
        tool_trace_count: llm_invocation.tool_trace_count,
        created_at: llm_invocation.created_at,
    }
}

fn to_tool_execution_view(tool_execution: ToolExecution) -> ToolExecutionView {
    ToolExecutionView {
        id: tool_execution.id,
        execution_id: tool_execution.execution_id,
        source_kind: match tool_execution.source_kind {
            ToolExecutionSourceKind::DatasetOutput => {
                contracts::ToolExecutionSourceKindView::DatasetOutput
            }
            ToolExecutionSourceKind::ChatMessage => {
                contracts::ToolExecutionSourceKindView::ChatMessage
            }
            ToolExecutionSourceKind::WorkflowExecution => {
                contracts::ToolExecutionSourceKindView::WorkflowExecution
            }
        },
        dataset_output_id: tool_execution.dataset_output_id,
        chat_message_id: tool_execution.chat_message_id,
        sequence_no: tool_execution.sequence_no,
        call_id: tool_execution.call_id,
        tool_name: tool_execution.tool_name,
        tool: tool_execution
            .tool_snapshot
            .as_ref()
            .and_then(parse_tool_reference),
        status: match tool_execution.status {
            ToolExecutionStatus::Requested => contracts::ManifestToolCallStatusView::Requested,
            ToolExecutionStatus::Completed => contracts::ManifestToolCallStatusView::Completed,
            ToolExecutionStatus::Failed => contracts::ManifestToolCallStatusView::Failed,
        },
        arguments: tool_execution.arguments,
        result: tool_execution.result,
        created_at: tool_execution.created_at,
    }
}

fn summarize_execution_scope_runtime(
    llm_invocations: &[LlmInvocationView],
    tool_executions: &[ToolExecutionView],
) -> Option<contracts::WorkflowExecutionRuntimeSummaryView> {
    let execution_scope_invocations = llm_invocations
        .iter()
        .filter(|invocation| {
            invocation.source_kind == contracts::LlmInvocationSourceKindView::WorkflowExecution
        })
        .collect::<Vec<_>>();
    let execution_scope_tool_executions = tool_executions
        .iter()
        .filter(|execution| {
            execution.source_kind == contracts::ToolExecutionSourceKindView::WorkflowExecution
        })
        .collect::<Vec<_>>();

    if execution_scope_invocations.is_empty() && execution_scope_tool_executions.is_empty() {
        return None;
    }

    let latest_invocation = execution_scope_invocations.last().cloned().cloned();
    let failed_tool_execution_count = execution_scope_tool_executions
        .iter()
        .filter(|execution| execution.status == contracts::ManifestToolCallStatusView::Failed)
        .count();
    let latest_provider = latest_invocation
        .as_ref()
        .and_then(|invocation| invocation.provider.clone());
    let latest_model = latest_invocation
        .as_ref()
        .and_then(|invocation| invocation.model.clone());
    let latest_request_id = latest_invocation
        .as_ref()
        .and_then(|invocation| invocation.request_id.clone());
    let latest_finish_reason = latest_invocation
        .as_ref()
        .and_then(|invocation| invocation.finish_reason.clone());
    let latest_tool_trace_count = latest_invocation
        .as_ref()
        .and_then(|invocation| invocation.tool_trace_count);

    Some(contracts::WorkflowExecutionRuntimeSummaryView {
        llm_invocation_count: execution_scope_invocations.len(),
        tool_execution_count: execution_scope_tool_executions.len(),
        failed_tool_execution_count,
        latest_provider,
        latest_model,
        latest_request_id,
        latest_finish_reason,
        latest_tool_trace_count,
    })
}

fn validate_required(field: &'static str, value: &str) -> std::result::Result<(), ApiError> {
    if value.trim().is_empty() {
        return Err(ApiError::bad_request(
            "validation_error",
            format!("{field} must not be empty"),
        ));
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ChatSessionReportEntryUpdatePlan {
    RequestConfirmation(PreparedChatSessionReportEntry),
    StayMaterialService(PreparedChatSessionReportEntry),
    EnterReportService(PreparedChatSessionReportEntry),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreparedChatSessionReportEntry {
    requested_at: DateTime<Utc>,
    title: String,
    objective: String,
}

fn plan_chat_session_report_entry_update(
    session: &ChatSession,
    current_report_entry: Option<&contracts::ChatSessionReportEntryView>,
    request: &UpdateChatSessionReportEntryRequest,
    now: DateTime<Utc>,
) -> std::result::Result<ChatSessionReportEntryUpdatePlan, ApiError> {
    match request.action {
        contracts::ChatSessionReportEntryActionView::RequestConfirmation => {
            ensure_chat_session_report_entry_not_confirmed(
                current_report_entry,
                "chat session report entry has already been confirmed",
            )?;

            Ok(ChatSessionReportEntryUpdatePlan::RequestConfirmation(
                PreparedChatSessionReportEntry {
                    requested_at: current_report_entry
                        .and_then(|entry| entry.requested_at)
                        .unwrap_or(now),
                    title: trim_optional(request.title.clone())
                        .unwrap_or_else(|| derive_chat_session_report_plan_title(session)),
                    objective: trim_optional(request.objective.clone())
                        .unwrap_or_else(|| derive_chat_session_report_plan_objective(session)),
                },
            ))
        }
        contracts::ChatSessionReportEntryActionView::StayMaterialService => {
            ensure_chat_session_report_entry_not_confirmed(
                current_report_entry,
                "confirmed report service entry cannot be reset through this endpoint",
            )?;
            Ok(ChatSessionReportEntryUpdatePlan::StayMaterialService(
                PreparedChatSessionReportEntry {
                    requested_at: current_report_entry
                        .and_then(|entry| entry.requested_at)
                        .unwrap_or(now),
                    title: current_report_entry
                        .and_then(|entry| trim_optional(entry.suggested_title.clone()))
                        .unwrap_or_else(|| derive_chat_session_report_plan_title(session)),
                    objective: current_report_entry
                        .and_then(|entry| trim_optional(entry.suggested_objective.clone()))
                        .unwrap_or_else(|| derive_chat_session_report_plan_objective(session)),
                },
            ))
        }
        contracts::ChatSessionReportEntryActionView::EnterReportService => {
            ensure_chat_session_report_entry_not_confirmed(
                current_report_entry,
                "chat session report entry has already been confirmed",
            )?;

            let title = trim_optional(request.title.clone())
                .or_else(|| {
                    current_report_entry
                        .and_then(|entry| trim_optional(entry.suggested_title.clone()))
                })
                .unwrap_or_else(|| derive_chat_session_report_plan_title(session));
            let objective = trim_optional(request.objective.clone())
                .or_else(|| {
                    current_report_entry
                        .and_then(|entry| trim_optional(entry.suggested_objective.clone()))
                })
                .unwrap_or_else(|| derive_chat_session_report_plan_objective(session));
            validate_required("title", &title)?;
            validate_required("objective", &objective)?;

            Ok(ChatSessionReportEntryUpdatePlan::EnterReportService(
                PreparedChatSessionReportEntry {
                    requested_at: current_report_entry
                        .and_then(|entry| entry.requested_at)
                        .unwrap_or(now),
                    title,
                    objective,
                },
            ))
        }
    }
}

fn ensure_chat_session_report_entry_not_confirmed(
    current_report_entry: Option<&contracts::ChatSessionReportEntryView>,
    message: &str,
) -> std::result::Result<(), ApiError> {
    if matches!(
        current_report_entry.map(|entry| &entry.state),
        Some(contracts::ModelFacingReportEntryStateView::Confirmed)
    ) {
        return Err(ApiError::bad_request(
            "report_entry_already_confirmed",
            message.to_string(),
        ));
    }

    Ok(())
}

fn chat_session_manifest_object_mut(
    session_manifest: &mut Value,
) -> std::result::Result<&mut Map<String, Value>, ApiError> {
    session_manifest.as_object_mut().ok_or_else(|| {
        ApiError::internal(
            "chat_session_manifest_invalid",
            "chat session manifest must be a JSON object".to_string(),
        )
    })
}

fn write_chat_session_report_entry(
    session_manifest: &mut Value,
    state: contracts::ModelFacingReportEntryStateView,
    entry: &PreparedChatSessionReportEntry,
    resolved_at: Option<DateTime<Utc>>,
    resolved_action: Option<contracts::ChatSessionReportEntryResolutionView>,
    confirmed_report_plan_id: Option<ReportPlanId>,
) -> std::result::Result<(), ApiError> {
    chat_session_manifest_object_mut(session_manifest)?.insert(
        "report_entry".to_string(),
        json!({
            "state": format_model_facing_report_entry_state(&state),
            "requested_at": entry.requested_at,
            "resolved_at": resolved_at,
            "resolved_action": resolved_action.as_ref().map(format_chat_session_report_entry_resolution),
            "suggested_title": entry.title,
            "suggested_objective": entry.objective,
            "confirmed_report_plan_id": confirmed_report_plan_id,
        }),
    );
    Ok(())
}

fn confirmed_report_entry_service_handoff(
    entry: &PreparedChatSessionReportEntry,
    resolved_at: DateTime<Utc>,
) -> contracts::ManifestServiceHandoffView {
    contracts::ManifestServiceHandoffView {
        source: contracts::ManifestServiceHandoffSourceView::ChatSessionReportEntry,
        service_lane: contracts::ModelFacingServiceLaneView::ReportService,
        report_entry_state: contracts::ModelFacingReportEntryStateView::Confirmed,
        requested_at: Some(entry.requested_at),
        resolved_at: Some(resolved_at),
        resolved_action: Some(contracts::ChatSessionReportEntryResolutionView::EnterReportService),
        suggested_title: Some(entry.title.clone()),
        suggested_objective: Some(entry.objective.clone()),
        confirmed_report_plan_id: None,
    }
}

fn finalize_report_service_handoff(
    service_handoff: Option<contracts::ManifestServiceHandoffView>,
    report_plan_id: ReportPlanId,
) -> Option<contracts::ManifestServiceHandoffView> {
    service_handoff.map(|mut handoff| {
        if handoff.confirmed_report_plan_id.is_none()
            && handoff.report_entry_state == contracts::ModelFacingReportEntryStateView::Confirmed
        {
            handoff.confirmed_report_plan_id = Some(report_plan_id);
        }
        handoff
    })
}

fn trim_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim().to_string();
        (!trimmed.is_empty()).then_some(trimmed)
    })
}

fn derive_chat_session_title(prompt: &str) -> String {
    let normalized = prompt.split_whitespace().collect::<Vec<_>>().join(" ");
    let total_chars = normalized.chars().count();
    let mut title = normalized.chars().take(72).collect::<String>();
    if total_chars > 72 {
        title.push_str("...");
    }

    if title.is_empty() {
        "Untitled chat session".to_string()
    } else {
        title
    }
}

fn derive_chat_session_report_plan_title(session: &ChatSession) -> String {
    let base = session.title.trim();
    if base.is_empty() {
        return "Dataset Report".to_string();
    }

    let lowercase = base.to_ascii_lowercase();
    if lowercase.contains("report") {
        base.to_string()
    } else {
        format!("{base} Report")
    }
}

fn derive_chat_session_report_plan_objective(session: &ChatSession) -> String {
    let prompt = session
        .session_manifest
        .as_object()
        .and_then(|manifest| {
            manifest
                .get("last_prompt")
                .and_then(Value::as_str)
                .or_else(|| manifest.get("initial_prompt").and_then(Value::as_str))
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(session.title.trim());

    if prompt.is_empty() {
        "Turn the current dataset context into a report-ready output.".to_string()
    } else {
        format!("Turn the current dataset context into a report-ready output for: {prompt}")
    }
}

fn derive_static_page_draft_title(prompt: &str) -> String {
    let normalized = prompt.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return "静态页草稿".to_string();
    }
    let total_chars = normalized.chars().count();
    let mut title = normalized.chars().take(42).collect::<String>();
    if total_chars > 42 {
        title.push_str("...");
    }
    format!("静态页：{title}")
}

fn build_static_page_source_refs(run: &AssistantRun) -> Value {
    json!({
        "assistant_run_id": run.id,
        "local_thread_id": run.local_thread_id,
        "output_artifact_count": value_array(run.output_artifacts.clone()).len(),
    })
}

fn build_static_page_visibility_snapshot(run: &AssistantRun, selected_scope: &Value) -> Value {
    json!({
        "assistant_run_id": run.id,
        "selected_scope": selected_scope,
        "policy": "assistant_run_scope_snapshot",
        "created_from_evidence_state": run.evidence_state.get("status").and_then(Value::as_str).unwrap_or("unknown"),
    })
}

fn build_initial_static_page_draft_payload(run: &AssistantRun, prompt: &str) -> Value {
    let style_direction = "client-delivery";
    let visual_spec = build_static_page_visual_spec(style_direction);
    let render_spec = build_static_page_render_spec();
    let data_snapshot = json!({
        "version": 1,
        "source": "assistant_run",
        "selected_scope": run.selected_scope,
        "evidence_status": run.evidence_state.get("status").cloned().unwrap_or(Value::Null),
        "module_bindings": [],
    });
    let preview_contract = build_static_page_preview_contract(
        style_direction,
        &Value::Array(Vec::new()),
        &render_spec,
        None,
    );
    json!({
        "version": 1,
        "status": "draft",
        "title": derive_static_page_draft_title(prompt),
        "prompt": prompt,
        "modules": [],
        "mobileOrder": [],
        "styleDirection": style_direction,
        "style_direction": style_direction,
        "visualSpec": visual_spec.clone(),
        "visual_spec": visual_spec,
        "renderSpec": render_spec.clone(),
        "render_spec": render_spec,
        "dataSnapshot": data_snapshot.clone(),
        "data_snapshot": data_snapshot,
        "previewContract": preview_contract.clone(),
        "preview_contract": preview_contract,
        "data_bindings": [],
        "assistant_context": {
            "assistant_run_id": run.id,
            "selected_scope": run.selected_scope,
            "evidence_state": run.evidence_state,
        },
    })
}

fn build_static_page_image_prompt_payload(draft: &StaticPageDraft, prompt: Option<&str>) -> Value {
    let payload = &draft.draft_payload;
    let style_direction =
        static_page_payload_string(payload, &["styleDirection", "style_direction"])
            .unwrap_or_else(|| "client-delivery".to_string());
    let modules = static_page_payload_modules(payload);
    let visual_spec = static_page_payload_value(payload, &["visualSpec", "visual_spec"])
        .unwrap_or_else(|| build_static_page_visual_spec(&style_direction));
    let render_spec = static_page_payload_value(payload, &["renderSpec", "render_spec"])
        .unwrap_or_else(build_static_page_render_spec);
    let data_snapshot = static_page_payload_value(payload, &["dataSnapshot", "data_snapshot"])
        .unwrap_or_else(|| build_static_page_data_snapshot(payload, &draft.selected_scope));
    let preview_contract =
        static_page_payload_value(payload, &["previewContract", "preview_contract"])
            .unwrap_or_else(|| {
                build_static_page_preview_contract(&style_direction, &modules, &render_spec, None)
            });
    json!({
        "draft_id": draft.id,
        "assistant_run_id": draft.assistant_run_id,
        "title": draft.title,
        "prompt": prompt.map(str::trim).filter(|value| !value.is_empty()),
        "style_direction": style_direction,
        "visual_spec": visual_spec,
        "render_spec": render_spec,
        "data_snapshot": data_snapshot,
        "preview_contract": preview_contract,
        "design_contract": {
            "contract_source": "StaticPageDraft",
            "visual_source": "effect image is a preview contract, not final source code",
            "final_source": "draft_payload visual_spec/render_spec/modules/data_snapshot",
            "editable_core": "DOM text + SVG/chart components",
        },
        "selected_scope": draft.selected_scope,
        "visibility_snapshot": draft.visibility_snapshot,
        "modules": modules,
        "data_bindings": payload.get("data_bindings").cloned().unwrap_or_else(|| json!([])),
        "queue_copy": "资源正在排队，可以联系商务开通高级用户跳过等待。",
    })
}

fn build_static_page_visual_spec(style_direction: &str) -> Value {
    match style_direction {
        "decision-brief" => json!({
            "version": 1,
            "styleDirection": "decision-brief",
            "palette": {
                "background": "#0f172a",
                "surface": "rgba(255,255,255,0.08)",
                "text": "#f8fafc",
                "mutedText": "#cbd5e1",
                "accent": "#93c5fd",
                "chart": "#38bdf8"
            },
            "typography": {
                "headingFamily": "Aptos Display, ui-sans-serif, system-ui",
                "bodyFamily": "Aptos, ui-sans-serif, system-ui",
                "density": "compact"
            },
            "surface": {
                "radius": 26,
                "shadow": "deep",
                "decoration": "subtle-gradient"
            }
        }),
        "data-command" => json!({
            "version": 1,
            "styleDirection": "data-command",
            "palette": {
                "background": "#064e3b",
                "surface": "rgba(255,255,255,0.09)",
                "text": "#ecfeff",
                "mutedText": "#cbd5e1",
                "accent": "#5eead4",
                "chart": "#22d3ee"
            },
            "typography": {
                "headingFamily": "Aptos Display, ui-sans-serif, system-ui",
                "bodyFamily": "Aptos, ui-sans-serif, system-ui",
                "density": "dense"
            },
            "surface": {
                "radius": 22,
                "shadow": "glow",
                "decoration": "command-gradient"
            }
        }),
        _ => json!({
            "version": 1,
            "styleDirection": "client-delivery",
            "palette": {
                "background": "#f7f8fb",
                "surface": "rgba(255,255,255,0.76)",
                "text": "#101827",
                "mutedText": "#475569",
                "accent": "#2563eb",
                "chart": "#0ea5e9"
            },
            "typography": {
                "headingFamily": "Aptos Display, ui-sans-serif, system-ui",
                "bodyFamily": "Aptos, ui-sans-serif, system-ui",
                "density": "balanced"
            },
            "surface": {
                "radius": 26,
                "shadow": "soft",
                "decoration": "warm-gradient"
            }
        }),
    }
}

fn build_static_page_render_spec() -> Value {
    json!({
        "renderer": "static-page-renderer-v1",
        "layoutEngine": "css-grid-12",
        "desktopGrid": {
            "columns": 12,
            "rowHeight": 96
        },
        "mobileLayout": "single-column-sortable",
        "componentModel": "dom-text-svg-chart",
        "chartRuntime": "recharts-first-echarts-optional",
        "editableContent": ["title", "content", "dataBinding", "visualization", "layout"],
        "generationGuardrails": [
            "效果图必须服从模块网格布局和移动端顺序",
            "正文、指标、图表在最终静态页中必须是真 DOM 或 SVG，不允许只烘焙进图片",
            "复杂背景、纹理、装饰可以作为图片资产，核心数据表达必须可重新渲染",
            "避免 3D 透视、真实摄影 UI、不可复刻字体效果和过度复杂玻璃反射"
        ]
    })
}

fn build_static_page_data_snapshot(payload: &Value, selected_scope: &Value) -> Value {
    let module_bindings = static_page_payload_modules(payload)
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|module| {
            json!({
                "moduleId": module.get("id").cloned().unwrap_or(Value::Null),
                "title": module.get("title").cloned().unwrap_or(Value::Null),
                "binding": module
                    .get("dataBinding")
                    .or_else(|| module.get("data_binding"))
                    .cloned()
                    .unwrap_or(Value::Null),
                "visualizationType": module
                    .get("visualization")
                    .and_then(|visualization| visualization.get("type"))
                    .cloned()
                    .unwrap_or_else(|| json!("text-insight")),
            })
        })
        .collect::<Vec<_>>();
    json!({
        "version": 1,
        "source": "static_page_draft",
        "selected_scope": selected_scope,
        "module_bindings": module_bindings,
    })
}

fn build_static_page_preview_contract(
    style_direction: &str,
    modules: &Value,
    render_spec: &Value,
    patch: Option<Value>,
) -> Value {
    let contract_source = json!({
        "style_direction": style_direction,
        "modules": modules,
        "render_spec": render_spec,
    });
    let mut contract = json!({
        "version": 1,
        "kind": "static-page-preview-contract",
        "status": "not_requested",
        "draftFingerprint": static_page_design_fingerprint(&contract_source),
        "imageJobId": Value::Null,
        "assetKey": Value::Null,
        "confirmedAt": Value::Null,
        "renderExpectation": "final HTML/CSS/SVG should reproduce the confirmed preview without baking editable text or charts into the image",
    });
    if let Some(patch) = patch {
        merge_json_value(&mut contract, &patch);
        set_payload_string(
            &mut contract,
            "draftFingerprint",
            &static_page_design_fingerprint(&contract_source),
        );
    }
    contract
}

fn static_page_design_fingerprint(value: &Value) -> String {
    let serialized = serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string());
    let mut hash: u32 = 0;
    for byte in serialized.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u32);
    }
    format!("design-{hash:08x}")
}

fn static_page_payload_modules(payload: &Value) -> Value {
    payload
        .get("modules")
        .and_then(Value::as_array)
        .cloned()
        .map(Value::Array)
        .unwrap_or_else(|| Value::Array(Vec::new()))
}

fn static_page_payload_string(payload: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        payload
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    })
}

fn static_page_payload_value(payload: &Value, keys: &[&str]) -> Option<Value> {
    keys.iter().find_map(|key| payload.get(*key).cloned())
}

async fn load_static_page_draft_or_404(
    state: &AppState,
    draft_id: StaticPageDraftId,
) -> std::result::Result<StaticPageDraft, ApiError> {
    state
        .storage
        .static_page_drafts()
        .get_by_id(state.tenant_id, draft_id)
        .await
        .map_err(ApiError::from_storage)?
        .ok_or_else(|| {
            ApiError::not_found(
                "static_page_draft_not_found",
                format!("static page draft {} was not found", draft_id),
            )
        })
}

async fn load_static_page_image_job_or_404(
    state: &AppState,
    job_id: StaticPageImageJobId,
) -> std::result::Result<StaticPageImageJob, ApiError> {
    state
        .storage
        .static_page_image_jobs()
        .get_by_id(state.tenant_id, job_id)
        .await
        .map_err(ApiError::from_storage)?
        .ok_or_else(|| {
            ApiError::not_found(
                "static_page_image_job_not_found",
                format!("static page image job {} was not found", job_id),
            )
        })
}

async fn resolve_confirmed_static_page_image_job(
    state: &AppState,
    draft: &StaticPageDraft,
    requested_job_id: Option<StaticPageImageJobId>,
) -> std::result::Result<Option<StaticPageImageJob>, ApiError> {
    let job = if let Some(job_id) = requested_job_id {
        Some(load_static_page_image_job_or_404(state, job_id).await?)
    } else {
        state
            .storage
            .static_page_image_jobs()
            .list_by_draft(state.tenant_id, draft.id)
            .await
            .map_err(ApiError::from_storage)?
            .into_iter()
            .find(|job| matches!(job.status, StaticPageImageJobStatus::Confirmed))
    };
    let Some(job) = job else {
        return Err(ApiError::bad_request(
            "static_page_preview_not_confirmed",
            "confirm an effect preview before rendering the final static page".to_string(),
        ));
    };
    if job.draft_id != draft.id {
        return Err(ApiError::bad_request(
            "static_page_image_job_mismatch",
            "image job does not belong to this static page draft".to_string(),
        ));
    }
    if !matches!(job.status, StaticPageImageJobStatus::Confirmed) {
        return Err(ApiError::bad_request(
            "static_page_preview_not_confirmed",
            "confirm an effect preview before rendering the final static page".to_string(),
        ));
    }
    Ok(Some(job))
}

async fn append_static_page_draft_run_event(
    state: &AppState,
    draft: &StaticPageDraft,
    event_name: &str,
    payload: Value,
) -> std::result::Result<(), ApiError> {
    state
        .storage
        .assistant_runs()
        .append_event(
            state.tenant_id,
            draft.assistant_run_id,
            &NewAssistantRunEvent {
                event_name: event_name.to_string(),
                payload,
                created_at: Utc::now(),
            },
        )
        .await
        .map_err(ApiError::from_storage)?;
    Ok(())
}

async fn interpret_static_page_draft_intent_for_api(
    state: &AppState,
    draft: &StaticPageDraft,
    prompt: &str,
    draft_payload: &Value,
    messages: Vec<AssistantRunMessageView>,
) -> std::result::Result<StaticPageIntentOutcome, ApiError> {
    let run = state
        .storage
        .assistant_runs()
        .get_by_id(state.tenant_id, draft.assistant_run_id)
        .await
        .map_err(ApiError::from_storage)?
        .ok_or_else(|| {
            ApiError::not_found(
                "assistant_run_not_found",
                format!("assistant run {} was not found", draft.assistant_run_id),
            )
        })?;
    let runtime_request = StaticPageIntentRequest {
        prompt: prompt.to_string(),
        draft_payload: draft_payload.clone(),
        assistant_run_id: Some(draft.assistant_run_id.to_string()),
        startup_briefing: run.startup_briefing.clone(),
        selected_scope: draft.selected_scope.clone(),
        evidence_state: run.evidence_state.clone(),
        conversation_memory_refs: static_page_conversation_memory_refs(&run),
        messages: messages
            .into_iter()
            .map(|message| {
                json!({
                    "role": message.role.as_str(),
                    "content": message.content,
                })
            })
            .collect(),
    };
    let runtime_mode = std::env::var("STATIC_PAGE_INTENT_RUNTIME_MODE")
        .unwrap_or_else(|_| "deterministic".to_string());
    if runtime_mode != "provider" {
        return interpret_static_page_intent_deterministic(&runtime_request).map_err(|error| {
            ApiError::internal(
                "static_page_intent_runtime_failed",
                format!("static page deterministic intent failed: {error}"),
            )
        });
    }

    let runtime_provider = std::env::var("STATIC_PAGE_INTENT_RUNTIME_PROVIDER")
        .unwrap_or_else(|_| "static_page_intent_provider".to_string());
    let runtime_model = std::env::var("STATIC_PAGE_INTENT_RUNTIME_MODEL")
        .unwrap_or_else(|_| DEFAULT_STATIC_PAGE_INTENT_RUNTIME_MODEL.to_string());
    let provider_request = runtime_request.clone();
    let provider_result = tokio::task::spawn_blocking(move || {
        let provider = build_provider_from_env(
            "STATIC_PAGE_INTENT",
            "provider",
            runtime_provider,
            bootstrap_default_prompt_registry(),
        )?;
        interpret_static_page_intent_with_provider(
            &provider_request,
            provider.as_ref(),
            Some(&runtime_model),
        )
    })
    .await
    .map_err(|error| {
        ApiError::internal(
            "static_page_intent_join_failed",
            format!("static page intent worker join failed: {error}"),
        )
    })?;

    match provider_result {
        Ok(outcome) => Ok(outcome),
        Err(error) => {
            let mut fallback =
                interpret_static_page_intent_deterministic(&runtime_request).map_err(|fallback| {
                    ApiError::internal(
                        "static_page_intent_runtime_failed",
                        format!(
                            "static page provider failed ({error}); deterministic fallback also failed: {fallback}"
                        ),
                    )
                })?;
            fallback.runtime = json!({
                "source": "provider_fallback",
                "provider_failure": error.to_string(),
                "fallback": fallback.runtime,
            });
            Ok(fallback)
        }
    }
}

fn static_page_conversation_memory_refs(run: &AssistantRun) -> Vec<Value> {
    value_array(
        run.evidence_state
            .get("supplied_items")
            .cloned()
            .unwrap_or(Value::Null),
    )
    .into_iter()
    .filter(|item| item.get("type").and_then(Value::as_str) == Some("conversation_memory_item"))
    .collect()
}

fn validate_static_page_operations(
    operations: Vec<Value>,
) -> std::result::Result<Vec<Value>, ApiError> {
    sanitize_static_page_operations(operations)
        .map_err(|error| ApiError::bad_request("invalid_static_page_operation", error.to_string()))
}

fn summarize_static_page_operations(operations: &[Value]) -> String {
    if operations.is_empty() {
        return "未追加静态页操作。".to_string();
    }
    let mut types = BTreeMap::<String, usize>::new();
    for operation in operations {
        if let Some(operation_type) = static_page_operation_type(operation) {
            *types.entry(operation_type.to_string()).or_default() += 1;
        }
    }
    let labels = types
        .into_iter()
        .map(|(operation_type, count)| format!("{operation_type} x{count}"))
        .collect::<Vec<_>>()
        .join("，");
    format!("已追加静态页操作：{labels}。")
}

fn status_from_static_page_payload(payload: &Value) -> Option<StaticPageDraftStatus> {
    let status = payload
        .as_object()
        .and_then(|object| object.get("status"))
        .and_then(Value::as_str)
        .map(str::trim)?;
    match status {
        "draft" => Some(StaticPageDraftStatus::Draft),
        "planning" | "planned" => Some(StaticPageDraftStatus::Planned),
        "queued" => Some(StaticPageDraftStatus::Queued),
        "preview_ready" | "previewed" => Some(StaticPageDraftStatus::Previewed),
        "effect_confirmed" | "confirmed" => Some(StaticPageDraftStatus::Confirmed),
        "rendering" | "rendered" => Some(StaticPageDraftStatus::Rendered),
        "archived" => Some(StaticPageDraftStatus::Archived),
        _ => None,
    }
}

fn status_from_static_page_operations(operations: &[Value]) -> Option<StaticPageDraftStatus> {
    operations.iter().fold(None, |status, operation| {
        match static_page_operation_type(operation) {
            Some("queue_image_job") => Some(StaticPageDraftStatus::Queued),
            Some("mark_preview_ready") => Some(StaticPageDraftStatus::Previewed),
            Some("confirm_preview") => Some(StaticPageDraftStatus::Confirmed),
            Some("request_final_render") => Some(StaticPageDraftStatus::Rendered),
            Some(_) => Some(status.unwrap_or(StaticPageDraftStatus::Planned)),
            None => status,
        }
    })
}

fn static_page_operation_type(operation: &Value) -> Option<&str> {
    operation
        .as_object()
        .and_then(|object| object.get("type"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn apply_static_page_operations_to_payload(
    mut payload: Value,
    operations: &[Value],
    summary: Option<&str>,
) -> Value {
    ensure_json_object(&mut payload);
    for operation in operations {
        apply_static_page_operation_to_payload(&mut payload, operation);
    }
    if let Some(summary) = summary.filter(|value| !value.trim().is_empty()) {
        set_payload_string(&mut payload, "modelSummary", summary);
        set_payload_string(&mut payload, "model_summary", summary);
    }
    refresh_static_page_payload_design_contract(&mut payload);
    payload
}

fn apply_static_page_operation_to_payload(payload: &mut Value, operation: &Value) {
    let Some(operation_type) = static_page_operation_type(operation) else {
        return;
    };
    match operation_type {
        "change_style_direction" => {
            if let Some(style) = operation
                .get("styleDirection")
                .or_else(|| operation.get("style_direction"))
                .and_then(Value::as_str)
            {
                set_payload_string(payload, "styleDirection", style);
                set_payload_string(payload, "style_direction", style);
            }
        }
        "refresh_summary" => {
            if let Some(summary) = operation
                .get("modelSummary")
                .or_else(|| operation.get("model_summary"))
                .and_then(Value::as_str)
            {
                set_payload_string(payload, "modelSummary", summary);
                set_payload_string(payload, "model_summary", summary);
            }
        }
        "update_module" => {
            if let (Some(module_id), Some(patch)) = (
                static_page_operation_module_id(operation),
                operation.get("patch"),
            ) {
                merge_static_page_module(payload, module_id, patch);
            }
        }
        "change_visualization" => {
            if let (Some(module_id), Some(visualization_type)) = (
                static_page_operation_module_id(operation),
                operation.get("visualizationType").and_then(Value::as_str),
            ) {
                merge_static_page_module(
                    payload,
                    module_id,
                    &json!({
                        "visualization": {
                            "type": visualization_type,
                        },
                    }),
                );
            }
        }
        "change_data_binding" => {
            if let (Some(module_id), Some(data_binding)) = (
                static_page_operation_module_id(operation),
                operation.get("dataBinding"),
            ) {
                merge_static_page_module(
                    payload,
                    module_id,
                    &json!({
                        "dataBinding": data_binding,
                    }),
                );
            }
        }
        "add_module" => {
            if let Some(module) = operation.get("module") {
                push_static_page_module(payload, module.clone());
            }
        }
        "remove_module" => {
            if let Some(module_id) = static_page_operation_module_id(operation) {
                remove_static_page_module(payload, module_id);
            }
        }
        "move_module" | "resize_module" => {
            if let (Some(module_id), Some(layout)) = (
                static_page_operation_module_id(operation),
                operation.get("layout").or_else(|| operation.get("patch")),
            ) {
                merge_static_page_module(
                    payload,
                    module_id,
                    &json!({
                        "layout": layout,
                    }),
                );
            }
        }
        "reorder_modules" => {
            if let Some(order) = operation.get("order").and_then(Value::as_array) {
                set_payload_value(payload, "mobileOrder", Value::Array(order.clone()));
            }
        }
        "queue_image_job" => {
            set_payload_string(payload, "status", "queued");
            set_payload_value(
                payload,
                "imageJob",
                json!({
                    "id": operation.get("jobId").cloned().unwrap_or(Value::Null),
                    "status": "queued",
                    "queuePosition": operation.get("queuePosition").cloned().unwrap_or(Value::Null),
                    "queueMessage": operation
                        .get("queueMessage")
                        .and_then(Value::as_str)
                        .unwrap_or("资源正在排队，可以联系商务开通高级用户跳过等待。"),
                }),
            );
            set_payload_value(
                payload,
                "previewContract",
                json!({
                    "status": "queued",
                    "imageJobId": operation.get("jobId").cloned().unwrap_or(Value::Null),
                    "assetKey": Value::Null,
                    "queuePosition": operation.get("queuePosition").cloned().unwrap_or(Value::Null),
                    "confirmedAt": Value::Null,
                }),
            );
        }
        "mark_preview_ready" => {
            set_payload_string(payload, "status", "preview_ready");
            if let Some(preview) = operation.get("previewImage") {
                set_payload_value(payload, "previewImage", preview.clone());
                set_payload_value(
                    payload,
                    "previewContract",
                    json!({
                        "status": "preview_ready",
                        "imageJobId": preview.get("imageJobId").cloned().unwrap_or(Value::Null),
                        "assetKey": preview.get("assetKey").cloned().unwrap_or(Value::Null),
                        "queuePosition": Value::Null,
                    }),
                );
            }
        }
        "confirm_preview" => {
            set_payload_string(payload, "status", "effect_confirmed");
            if let Some(preview) = operation.get("previewImage") {
                set_payload_value(payload, "previewImage", preview.clone());
                set_payload_value(
                    payload,
                    "previewContract",
                    json!({
                        "status": "confirmed",
                        "imageJobId": preview.get("imageJobId").cloned().unwrap_or(Value::Null),
                        "assetKey": preview.get("assetKey").cloned().unwrap_or(Value::Null),
                        "queuePosition": Value::Null,
                        "confirmedAt": Utc::now(),
                    }),
                );
            }
        }
        "request_final_render" => {
            set_payload_string(payload, "status", "rendering");
            if let Some(final_page) = operation
                .get("finalPage")
                .or_else(|| operation.get("payload"))
            {
                set_payload_value(payload, "finalPage", final_page.clone());
            }
        }
        _ => {}
    }
}

fn refresh_static_page_payload_design_contract(payload: &mut Value) {
    ensure_json_object(payload);
    let style_direction =
        static_page_payload_string(payload, &["styleDirection", "style_direction"])
            .unwrap_or_else(|| "client-delivery".to_string());
    let modules = static_page_payload_modules(payload);
    let visual_spec = build_static_page_visual_spec(&style_direction);
    let render_spec = static_page_payload_value(payload, &["renderSpec", "render_spec"])
        .unwrap_or_else(build_static_page_render_spec);
    let selected_scope = payload
        .get("assistant_context")
        .and_then(|context| context.get("selected_scope"))
        .cloned()
        .or_else(|| payload.get("selected_scope").cloned())
        .unwrap_or(Value::Null);
    let data_snapshot = build_static_page_data_snapshot(payload, &selected_scope);
    let previous_contract =
        static_page_payload_value(payload, &["previewContract", "preview_contract"]);
    let preview_contract = build_static_page_preview_contract(
        &style_direction,
        &modules,
        &render_spec,
        previous_contract,
    );

    set_payload_string(payload, "styleDirection", &style_direction);
    set_payload_string(payload, "style_direction", &style_direction);
    set_payload_value(payload, "visualSpec", visual_spec.clone());
    set_payload_value(payload, "visual_spec", visual_spec);
    set_payload_value(payload, "renderSpec", render_spec.clone());
    set_payload_value(payload, "render_spec", render_spec);
    set_payload_value(payload, "dataSnapshot", data_snapshot.clone());
    set_payload_value(payload, "data_snapshot", data_snapshot);
    set_payload_value(payload, "previewContract", preview_contract.clone());
    set_payload_value(payload, "preview_contract", preview_contract);
}

fn append_static_page_operations_metadata(
    payload: &mut Value,
    operations: &[Value],
    prompt: Option<&str>,
    summary: &str,
) {
    ensure_json_object(payload);
    let Some(object) = payload.as_object_mut() else {
        return;
    };
    let entry = object
        .entry("operations".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    if !entry.is_array() {
        *entry = Value::Array(Vec::new());
    }
    if let Some(items) = entry.as_array_mut() {
        let applied_at = Utc::now().to_rfc3339();
        for operation in operations {
            let mut operation = operation.clone();
            if let Some(object) = operation.as_object_mut() {
                object
                    .entry("appliedAt".to_string())
                    .or_insert_with(|| json!(applied_at));
                if let Some(prompt) = prompt.map(str::trim).filter(|value| !value.is_empty()) {
                    object
                        .entry("prompt".to_string())
                        .or_insert_with(|| json!(prompt));
                }
            }
            items.push(operation);
        }
    }
    object.insert("lastOperationSummary".to_string(), json!(summary));
}

fn static_page_operation_module_id(operation: &Value) -> Option<&str> {
    operation
        .get("targetModuleId")
        .or_else(|| operation.get("moduleId"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn merge_static_page_module(payload: &mut Value, module_id: &str, patch: &Value) {
    let Some(modules) = payload
        .as_object_mut()
        .and_then(|object| object.get_mut("modules"))
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    let Some(module) = modules.iter_mut().find(|module| {
        module
            .as_object()
            .and_then(|object| object.get("id"))
            .and_then(Value::as_str)
            == Some(module_id)
    }) else {
        return;
    };
    merge_json_value(module, patch);
}

fn push_static_page_module(payload: &mut Value, module: Value) {
    ensure_json_object(payload);
    let Some(object) = payload.as_object_mut() else {
        return;
    };
    let modules = object
        .entry("modules".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    if !modules.is_array() {
        *modules = Value::Array(Vec::new());
    }
    if let Some(items) = modules.as_array_mut() {
        items.push(module);
    }
}

fn remove_static_page_module(payload: &mut Value, module_id: &str) {
    let Some(modules) = payload
        .as_object_mut()
        .and_then(|object| object.get_mut("modules"))
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    modules.retain(|module| {
        module
            .as_object()
            .and_then(|object| object.get("id"))
            .and_then(Value::as_str)
            != Some(module_id)
    });
}

fn merge_json_value(target: &mut Value, patch: &Value) {
    match (target, patch) {
        (Value::Object(target_object), Value::Object(patch_object)) => {
            for (key, value) in patch_object {
                match target_object.get_mut(key) {
                    Some(existing) => merge_json_value(existing, value),
                    None => {
                        target_object.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        (target, patch) => {
            *target = patch.clone();
        }
    }
}

fn ensure_json_object(value: &mut Value) {
    if !value.is_object() {
        *value = Value::Object(Map::new());
    }
}

fn set_payload_string(payload: &mut Value, key: &str, value: &str) {
    set_payload_value(payload, key, json!(value));
}

fn set_payload_value(payload: &mut Value, key: &str, value: Value) {
    ensure_json_object(payload);
    if let Some(object) = payload.as_object_mut() {
        object.insert(key.to_string(), value);
    }
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    payload: ApiErrorResponse,
}

impl ApiError {
    fn bad_request(code: &str, message: String) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            payload: ApiErrorResponse {
                code: code.to_string(),
                message,
            },
        }
    }

    fn not_found(code: &str, message: String) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            payload: ApiErrorResponse {
                code: code.to_string(),
                message,
            },
        }
    }

    fn internal(code: &str, message: String) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            payload: ApiErrorResponse {
                code: code.to_string(),
                message,
            },
        }
    }

    fn from_storage(error: anyhow::Error) -> Self {
        if let Some(sqlx::Error::Database(db_error)) = error.downcast_ref::<sqlx::Error>() {
            match db_error.code().as_deref() {
                Some("23505") => {
                    return Self {
                        status: StatusCode::CONFLICT,
                        payload: ApiErrorResponse {
                            code: "conflict".to_string(),
                            message: db_error.message().to_string(),
                        },
                    };
                }
                Some("23503") => {
                    return Self::bad_request(
                        "foreign_key_violation",
                        db_error.message().to_string(),
                    );
                }
                _ => {}
            }
        }

        tracing::error!(error = ?error, "platform-api storage operation failed");
        Self::internal("storage_error", error.to_string())
    }

    fn from_transition(error: workflow_engine::WorkflowTransitionError) -> Self {
        match error {
            workflow_engine::WorkflowTransitionError::UnsupportedSignal => Self::bad_request(
                "unsupported_signal",
                "the requested workflow signal is not supported".to_string(),
            ),
            workflow_engine::WorkflowTransitionError::InvalidTransition(message) => {
                Self::bad_request("invalid_transition", message)
            }
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(self.payload)).into_response()
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.payload.code, self.payload.message)
    }
}

impl std::error::Error for ApiError {}

#[cfg(test)]
mod tests {
    use super::*;
    use domain_model::{
        ChatMessageId, ChatSession, Dataset, DatasetId, DatasetLifecycle, DatasetOutput,
        DatasetOutputId, DatasetVisibility, DocumentChunk, DocumentChunkId, DocumentChunkState,
        LlmInvocation, LlmInvocationFinishReason, LlmInvocationId, LlmInvocationMode,
        LlmInvocationSourceKind, MemoryDirectory, MemoryDirectoryId, PublishedSurface, ReportPlan,
        ReportPlanAstVersionId, ReportPlanId, ReportPlanStatus, ReportRenderOutput,
        ReportRenderOutputId, ReportRenderOutputStatus, RetrievalEvidence, RetrievalEvidenceId,
        SecretBindingId, TenantId, ToolExecution, ToolExecutionId, ToolExecutionSourceKind,
        ToolExecutionStatus, WorkflowExecutionId, WorkflowStatus,
    };
    use event_bus::{workflow_execution_transition_subject, workflow_task_enqueued_subject};
    use test_fixtures::{
        local_postgres_storage, reset_local_postgres_storage, shared_local_postgres_test_lock,
    };
    use tool_registry::{ToolCliContract, ToolCliOutputMode, ToolDefinition, ToolInvocationMode};
    use tower::util::ServiceExt;

    #[test]
    fn assistant_run_provider_input_includes_briefing_scope_and_history() {
        let input = build_assistant_run_provider_input(&CreateAssistantRunRequest {
            prompt: "继续总结订单风险".to_string(),
            local_thread_id: Some("browser-thread-1".to_string()),
            startup_briefing: Some(json!({
                "productTruth": "智能数据工作台",
                "visibleDatasetCount": 2,
            })),
            selected_scope: Some(json!({
                "datasets": ["orders"],
            })),
            scope_candidates: vec![json!({
                "type": "dataset",
                "label": "订单",
            })],
            context_policy_hint: Some(json!({
                "history_policy": "intent_gated",
            })),
            current_artifact: None,
            messages: vec![
                AssistantRunMessageView {
                    role: ChatMessageRole::User,
                    content: "刚才先看销售".to_string(),
                },
                AssistantRunMessageView {
                    role: ChatMessageRole::Assistant,
                    content: "已预选订单数据集".to_string(),
                },
            ],
        });

        assert!(input.contains("智能数据工作台"));
        assert!(input.contains("范围候选"));
        assert!(input.contains("当前选中范围"));
        assert!(input.contains("assistant: 已预选订单数据集"));
        assert!(input.contains("用户问题：继续总结订单风险"));
    }

    #[tokio::test]
    async fn assistant_run_is_persisted_and_events_can_be_appended() {
        let _guard = shared_local_postgres_test_lock().lock().await;
        std::env::set_var("ASSISTANT_RUN_RUNTIME_MODE", "placeholder");
        let storage = match local_postgres_storage().await {
            Ok(storage) => storage,
            Err(reason) => {
                eprintln!("skipping assistant run persistence test: {reason}");
                return;
            }
        };
        reset_and_sync_test_storage(&storage).await;

        let tenant = storage
            .ensure_tenant(
                &format!("assistant-run-test-{}", Uuid::new_v4()),
                "Assistant Run Test",
            )
            .await
            .expect("tenant should exist");
        let state = AppState::new(
            storage,
            workflow_definitions::catalog(),
            tenant.id,
            EventBus::Disabled,
        );

        let (status, Json(response)) = create_assistant_run(
            State(state.clone()),
            HeaderMap::new(),
            Json(CreateAssistantRunRequest {
                prompt: "普通问答：今天先聊项目架构".to_string(),
                local_thread_id: Some("browser-thread-test".to_string()),
                startup_briefing: Some(json!({
                    "visibleDatasetCount": 0,
                    "capabilities": ["ordinary_chat", "static_page_plan"],
                })),
                selected_scope: Some(json!({
                    "mode": "ordinary_chat",
                    "selected": [],
                })),
                scope_candidates: vec![json!({
                    "type": "conversation_memory",
                    "label": "当前浏览器历史",
                    "confidence": 0.4,
                })],
                context_policy_hint: Some(json!({
                    "history_policy": "intent_gated",
                })),
                current_artifact: None,
                messages: Vec::new(),
            }),
        )
        .await
        .expect("assistant run should be created");

        assert_eq!(status, StatusCode::CREATED);
        assert!(!response.assistant_run_id.to_string().is_empty());
        assert_eq!(response.selected_scope["mode"], json!("ordinary_chat"));
        assert!(response.scope_candidates.is_empty());
        assert_eq!(response.execution_trail.len(), 4);

        let Json(detail) = get_assistant_run(
            State(state.clone()),
            Path(response.assistant_run_id.to_string()),
        )
        .await
        .expect("assistant run detail should load");
        assert_eq!(detail.run.id, response.assistant_run_id);
        assert_eq!(
            detail.run.local_thread_id.as_deref(),
            Some("browser-thread-test")
        );
        assert_eq!(detail.run.service_lane, "ordinary_chat");
        assert_eq!(detail.events.len(), 1);
        assert_eq!(detail.events[0].event_name, "assistant_run.completed");

        let (event_status, Json(event_response)) = append_assistant_run_event(
            State(state),
            Path(response.assistant_run_id.to_string()),
            Json(AppendAssistantRunEventRequest {
                event_name: "assistant_run.note".to_string(),
                payload: json!({
                    "label": "用户要求继续执行",
                }),
            }),
        )
        .await
        .expect("assistant run event should append");
        assert_eq!(event_status, StatusCode::CREATED);
        assert_eq!(event_response.event.sequence_no, 2);
        assert_eq!(event_response.event.event_name, "assistant_run.note");
    }

    #[tokio::test]
    async fn assistant_run_continue_appends_event_trail_and_output() {
        let _guard = shared_local_postgres_test_lock().lock().await;
        std::env::set_var("ASSISTANT_RUN_RUNTIME_MODE", "placeholder");
        let storage = match local_postgres_storage().await {
            Ok(storage) => storage,
            Err(reason) => {
                eprintln!("skipping assistant run continue test: {reason}");
                return;
            }
        };
        reset_and_sync_test_storage(&storage).await;

        let tenant = storage
            .ensure_tenant(
                &format!("assistant-run-continue-test-{}", Uuid::new_v4()),
                "Assistant Run Continue Test",
            )
            .await
            .expect("tenant should exist");
        let state = AppState::new(
            storage,
            workflow_definitions::catalog(),
            tenant.id,
            EventBus::Disabled,
        );

        let (_, Json(run_response)) = create_assistant_run(
            State(state.clone()),
            HeaderMap::new(),
            Json(CreateAssistantRunRequest {
                prompt: "先看一下当前项目".to_string(),
                local_thread_id: Some("assistant-run-continue-thread".to_string()),
                startup_briefing: Some(json!({"visibleDatasetCount": 0})),
                selected_scope: Some(json!({"mode": "ordinary_chat"})),
                scope_candidates: Vec::new(),
                context_policy_hint: None,
                current_artifact: None,
                messages: Vec::new(),
            }),
        )
        .await
        .expect("assistant run should be created");

        let (continue_status, Json(continue_response)) = continue_assistant_run(
            State(state.clone()),
            Path(run_response.assistant_run_id.to_string()),
            Json(ContinueAssistantRunRequest {
                prompt: Some("继续下一步，最多别超过 9 步".to_string()),
                max_steps: Some(9),
                current_artifact: Some(json!({"type": "none"})),
                messages: vec![AssistantRunMessageView {
                    role: ChatMessageRole::User,
                    content: "继续".to_string(),
                }],
            }),
        )
        .await
        .expect("assistant run should continue");

        assert_eq!(continue_status, StatusCode::CREATED);
        assert_eq!(
            continue_response.event.event_name,
            "assistant_run.continued"
        );
        assert_eq!(continue_response.event.sequence_no, 2);
        assert_eq!(continue_response.event.payload["max_steps"], json!(5));
        assert!(continue_response
            .assistant_message
            .content
            .contains("AssistantRun 继续执行占位回复"));
        assert!(continue_response.execution_trail.iter().any(|step| {
            step.get("label") == Some(&json!("继续执行"))
                && step.get("max_steps") == Some(&json!(5))
        }));
        assert_eq!(continue_response.output_artifacts.len(), 2);

        let Json(detail) = get_assistant_run(
            State(state),
            Path(run_response.assistant_run_id.to_string()),
        )
        .await
        .expect("assistant run detail should load");
        assert_eq!(detail.events.len(), 2);
        assert_eq!(detail.run.output_artifacts.len(), 2);
        assert!(detail.run.execution_trail.iter().any(|step| {
            step.get("label") == Some(&json!("继续执行"))
                && step.get("max_steps") == Some(&json!(5))
        }));
    }

    #[tokio::test]
    async fn static_page_draft_can_be_created_under_assistant_run() {
        let _guard = shared_local_postgres_test_lock().lock().await;
        std::env::set_var("ASSISTANT_RUN_RUNTIME_MODE", "placeholder");
        let storage = match local_postgres_storage().await {
            Ok(storage) => storage,
            Err(reason) => {
                eprintln!("skipping static page draft test: {reason}");
                return;
            }
        };
        reset_and_sync_test_storage(&storage).await;

        let tenant = storage
            .ensure_tenant(
                &format!("static-page-draft-test-{}", Uuid::new_v4()),
                "Static Page Draft Test",
            )
            .await
            .expect("tenant should exist");
        let state = AppState::new(
            storage,
            workflow_definitions::catalog(),
            tenant.id,
            EventBus::Disabled,
        );

        let (_, Json(run_response)) = create_assistant_run(
            State(state.clone()),
            HeaderMap::new(),
            Json(CreateAssistantRunRequest {
                prompt: "生成一页经营分析静态页".to_string(),
                local_thread_id: Some("static-page-draft-thread".to_string()),
                startup_briefing: Some(json!({"capabilities": ["static_page_plan"]})),
                selected_scope: Some(json!({"mode": "ordinary_chat"})),
                scope_candidates: Vec::new(),
                context_policy_hint: None,
                current_artifact: None,
                messages: Vec::new(),
            }),
        )
        .await
        .expect("assistant run should be created");

        let (draft_status, Json(draft_response)) = create_static_page_draft_for_assistant_run(
            State(state.clone()),
            Path(run_response.assistant_run_id.to_string()),
            Json(CreateStaticPageDraftRequest {
                title: Some("经营分析静态页".to_string()),
                prompt: Some("生成一页经营分析静态页".to_string()),
                selected_scope: None,
                visibility_snapshot: None,
                source_refs: Value::Null,
                draft_payload: json!({
                    "version": 1,
                    "status": "planning",
                    "styleDirection": "client-delivery",
                    "mobileOrder": ["hero", "kpi", "trend", "risk", "next-steps"],
                    "modules": [
                        { "id": "hero", "title": "核心判断", "content": "先给出主结论。" },
                        { "id": "kpi", "title": "关键指标", "content": "展示指标。" },
                        { "id": "trend", "title": "趋势变化", "content": "展示趋势。" },
                        { "id": "risk", "title": "风险与机会", "content": "展示风险。" },
                        { "id": "next-steps", "title": "建议动作", "content": "展示动作。" }
                    ]
                }),
            }),
        )
        .await
        .expect("static page draft should be created");

        assert_eq!(draft_status, StatusCode::CREATED);
        assert_eq!(draft_response.draft.title, "经营分析静态页");
        assert_eq!(
            draft_response.draft.status,
            contracts::StaticPageDraftStatusView::Planned
        );
        assert_eq!(
            draft_response.draft.assistant_run_id,
            run_response.assistant_run_id
        );
        assert_eq!(
            draft_response.draft.draft_payload["modules"][0]["title"],
            json!("核心判断")
        );

        let Json(local_thread_drafts) = list_static_page_drafts(
            State(state.clone()),
            Query(StaticPageDraftListQuery {
                local_thread_id: Some("static-page-draft-thread".to_string()),
                assistant_run_id: None,
                limit: Some(10),
            }),
        )
        .await
        .expect("static page drafts should list by local thread");
        assert_eq!(local_thread_drafts.len(), 1);
        assert_eq!(local_thread_drafts[0].id, draft_response.draft.id);

        let Json(run_drafts) = list_static_page_drafts(
            State(state.clone()),
            Query(StaticPageDraftListQuery {
                local_thread_id: None,
                assistant_run_id: Some(run_response.assistant_run_id.to_string()),
                limit: Some(10),
            }),
        )
        .await
        .expect("static page drafts should list by assistant run");
        assert_eq!(run_drafts.len(), 1);
        assert_eq!(
            run_drafts[0].assistant_run_id,
            run_response.assistant_run_id
        );

        let Json(loaded) = get_static_page_draft(
            State(state.clone()),
            Path(draft_response.draft.id.to_string()),
        )
        .await
        .expect("static page draft should load");
        assert_eq!(loaded.id, draft_response.draft.id);
        assert_eq!(
            loaded.visibility_snapshot["policy"],
            json!("assistant_run_scope_snapshot")
        );

        let (intent_status, Json(intent_response)) = apply_static_page_draft_intent(
            State(state.clone()),
            Path(draft_response.draft.id.to_string()),
            Json(ApplyStaticPageDraftIntentRequest {
                prompt: "给老板看，突出风险，趋势换成柱状图".to_string(),
                draft_payload: None,
                messages: Vec::new(),
            }),
        )
        .await
        .expect("static page intent should apply");
        assert_eq!(intent_status, StatusCode::CREATED);
        assert_eq!(
            intent_response.draft.status,
            contracts::StaticPageDraftStatusView::Planned
        );
        assert_eq!(
            intent_response.draft.draft_payload["styleDirection"],
            json!("decision-brief")
        );
        assert_eq!(
            intent_response.draft.draft_payload["modules"][3]["title"],
            json!("优先风险与机会")
        );
        assert!(intent_response
            .operations
            .iter()
            .any(|operation| operation["type"] == json!("change_visualization")));

        let (operation_status, Json(operation_response)) = append_static_page_draft_operations(
            State(state.clone()),
            Path(draft_response.draft.id.to_string()),
            Json(AppendStaticPageDraftOperationsRequest {
                prompt: Some("直接排队出效果图".to_string()),
                summary: None,
                operations: vec![json!({
                    "type": "queue_image_job",
                    "queuePosition": 2,
                })],
                draft_payload: None,
            }),
        )
        .await
        .expect("static page operations should append");
        assert_eq!(operation_status, StatusCode::CREATED);
        assert_eq!(
            operation_response.draft.status,
            contracts::StaticPageDraftStatusView::Queued
        );
        assert_eq!(
            operation_response.draft.draft_payload["imageJob"]["status"],
            json!("queued")
        );

        let invalid_operation = append_static_page_draft_operations(
            State(state.clone()),
            Path(draft_response.draft.id.to_string()),
            Json(AppendStaticPageDraftOperationsRequest {
                prompt: Some("执行未知操作".to_string()),
                summary: None,
                operations: vec![json!({
                    "type": "run_shell",
                })],
                draft_payload: None,
            }),
        )
        .await
        .expect_err("unknown static page operation should be rejected");
        assert_eq!(
            invalid_operation.payload.code,
            "invalid_static_page_operation"
        );

        let render_before_confirm = create_static_page_render(
            State(state.clone()),
            Path(draft_response.draft.id.to_string()),
            Json(CreateStaticPageRenderRequest { image_job_id: None }),
        )
        .await
        .expect_err("render should require confirmed preview");
        assert_eq!(
            render_before_confirm.payload.code,
            "static_page_preview_not_confirmed"
        );

        let (job_status, Json(job_response)) = create_static_page_image_job(
            State(state.clone()),
            Path(draft_response.draft.id.to_string()),
            Json(CreateStaticPageImageJobRequest {
                prompt: Some("生成一张经营分析效果图".to_string()),
                image_prompt_payload: Value::Null,
            }),
        )
        .await
        .expect("static page image job should be created");
        assert_eq!(job_status, StatusCode::CREATED);
        assert_eq!(
            job_response.image_job.status,
            contracts::StaticPageImageJobStatusView::Queued
        );
        assert_eq!(job_response.image_job.queue_position, Some(1));
        assert_eq!(
            job_response.image_job.image_prompt_payload["queue_copy"],
            json!("资源正在排队，可以联系商务开通高级用户跳过等待。")
        );
        assert_eq!(
            job_response.image_job.image_prompt_payload["design_contract"]["editable_core"],
            json!("DOM text + SVG/chart components")
        );
        assert_eq!(
            job_response.image_job.image_prompt_payload["render_spec"]["componentModel"],
            json!("dom-text-svg-chart")
        );
        let image_workflows = state
            .storage
            .workflow_executions()
            .list_by_tenant(state.tenant_id)
            .await
            .expect("workflow executions should list");
        let image_workflow = image_workflows
            .iter()
            .find(|execution| execution.kind == WorkflowKind::StaticPageImageGeneration)
            .expect("static page image generation workflow should be created");
        assert_eq!(image_workflow.status, WorkflowStatus::Running);
        assert_eq!(
            image_workflow.context["static_page_image_job_id"],
            json!(job_response.image_job.id.to_string())
        );
        let image_tasks = state
            .storage
            .workflow_tasks()
            .list_by_execution(image_workflow.id)
            .await
            .expect("static page image workflow tasks should list");
        assert_eq!(image_tasks.len(), 1);
        assert_eq!(image_tasks[0].queue, "static_page");
        assert_eq!(image_tasks[0].task_key, "generate_static_page_image");

        let Json(loaded_job) = get_static_page_image_job(
            State(state.clone()),
            Path(job_response.image_job.id.to_string()),
        )
        .await
        .expect("static page image job should load");
        assert_eq!(loaded_job.id, job_response.image_job.id);

        let Json(image_jobs) = list_static_page_image_jobs(
            State(state.clone()),
            Path(draft_response.draft.id.to_string()),
        )
        .await
        .expect("static page image jobs should list");
        assert_eq!(image_jobs.len(), 1);
        assert_eq!(image_jobs[0].id, job_response.image_job.id);

        let Json(confirmed) = confirm_static_page_image_job(
            State(state.clone()),
            Path(job_response.image_job.id.to_string()),
            Json(ConfirmStaticPageImageJobRequest {
                preview_asset_key: Some("previews/static-page-1.png".to_string()),
            }),
        )
        .await
        .expect("static page image job should confirm");
        assert_eq!(
            confirmed.image_job.status,
            contracts::StaticPageImageJobStatusView::Confirmed
        );
        assert_eq!(
            confirmed.draft.status,
            contracts::StaticPageDraftStatusView::Confirmed
        );
        assert_eq!(
            confirmed.draft.draft_payload["previewImage"]["assetKey"],
            json!("previews/static-page-1.png")
        );
        assert_eq!(
            confirmed.draft.draft_payload["previewContract"]["status"],
            json!("confirmed")
        );

        let (render_status, Json(render_response)) = create_static_page_render(
            State(state.clone()),
            Path(draft_response.draft.id.to_string()),
            Json(CreateStaticPageRenderRequest {
                image_job_id: Some(job_response.image_job.id),
            }),
        )
        .await
        .expect("static page render should be created");
        assert_eq!(render_status, StatusCode::CREATED);
        assert_eq!(
            render_response.render_output.status,
            contracts::StaticPageRenderOutputStatusView::Rendered
        );
        assert!(render_response.render_output.html.contains("核心判断"));
        assert!(render_response
            .render_output
            .html
            .contains("previews/static-page-1.png"));
        assert_eq!(
            render_response.render_output.asset_manifest["design_contract"]["preview_role"],
            json!("visual_contract")
        );
        assert_eq!(
            render_response.render_output.asset_manifest["render_spec"]["componentModel"],
            json!("dom-text-svg-chart")
        );
        assert_eq!(
            render_response.draft.status,
            contracts::StaticPageDraftStatusView::Rendered
        );

        let Json(render_outputs) = list_static_page_render_outputs(
            State(state.clone()),
            Path(draft_response.draft.id.to_string()),
        )
        .await
        .expect("static page render outputs should list");
        assert_eq!(render_outputs.len(), 1);
        assert_eq!(render_outputs[0].id, render_response.render_output.id);

        let Json(detail) = get_assistant_run(
            State(state),
            Path(run_response.assistant_run_id.to_string()),
        )
        .await
        .expect("assistant run detail should load");
        assert!(detail
            .events
            .iter()
            .any(|event| event.event_name == "static_page_draft.created"));
        assert!(detail
            .events
            .iter()
            .any(|event| event.event_name == "static_page_draft.intent_applied"));
        assert!(detail
            .events
            .iter()
            .any(|event| event.event_name == "static_page_draft.operations_appended"));
        assert!(detail
            .events
            .iter()
            .any(|event| event.event_name == "static_page_image_job.created"));
        assert!(detail
            .events
            .iter()
            .any(|event| event.event_name == "static_page_image_job.confirmed"));
        assert!(detail
            .events
            .iter()
            .any(|event| event.event_name == "static_page_render.created"));
    }

    #[tokio::test]
    async fn conversation_memory_items_can_be_listed_for_assistant_run() {
        let _guard = shared_local_postgres_test_lock().lock().await;
        std::env::set_var("ASSISTANT_RUN_RUNTIME_MODE", "placeholder");
        let storage = match local_postgres_storage().await {
            Ok(storage) => storage,
            Err(reason) => {
                eprintln!("skipping conversation memory route test: {reason}");
                return;
            }
        };
        reset_and_sync_test_storage(&storage).await;

        let tenant = storage
            .ensure_tenant(
                &format!("conversation-memory-test-{}", Uuid::new_v4()),
                "Conversation Memory Test",
            )
            .await
            .expect("tenant should exist");
        let state = AppState::new(
            storage,
            workflow_definitions::catalog(),
            tenant.id,
            EventBus::Disabled,
        );

        let (memory_status, Json(memory_item)) = create_conversation_memory_item(
            State(state.clone()),
            Json(CreateConversationMemoryItemRequest {
                local_thread_id: "browser-thread-memory".to_string(),
                role: ChatMessageRole::User,
                item_kind: "user_statement".to_string(),
                summary: "用户刚才关注订单风险和客服投诉。".to_string(),
                source_message_refs: json!(["local-message-1"]),
                artifact_refs: json!([]),
                metadata: json!({"source": "browser_summary"}),
            }),
        )
        .await
        .expect("conversation memory should be created");
        assert_eq!(memory_status, StatusCode::CREATED);
        assert_eq!(memory_item.item_kind, "user_statement");

        let (run_status, Json(run_response)) = create_assistant_run(
            State(state.clone()),
            HeaderMap::new(),
            Json(CreateAssistantRunRequest {
                prompt: "继续刚才的订单风险".to_string(),
                local_thread_id: Some("browser-thread-memory".to_string()),
                startup_briefing: Some(json!({"visibleDatasetCount": 0})),
                selected_scope: Some(json!({"mode": "ordinary_chat"})),
                scope_candidates: Vec::new(),
                context_policy_hint: None,
                current_artifact: None,
                messages: Vec::new(),
            }),
        )
        .await
        .expect("assistant run should be created");
        assert_eq!(run_status, StatusCode::CREATED);
        assert_eq!(
            run_response.selected_scope["conversation_memory"],
            json!(["local-thread"])
        );
        assert_eq!(run_response.evidence_state["status"], json!("supplied"));
        assert_eq!(
            run_response.evidence_state["supplied_items"][0]["type"],
            json!("conversation_memory_item")
        );
        assert_eq!(
            run_response.evidence_state["supplied_items"][0]["summary"],
            json!("用户刚才关注订单风险和客服投诉。")
        );

        let Json(candidates) = list_assistant_run_conversation_memory_candidates(
            State(state.clone()),
            Path(run_response.assistant_run_id.to_string()),
            Query(ConversationMemoryQuery {
                local_thread_id: None,
                query: Some("订单风险".to_string()),
                limit: Some(10),
            }),
        )
        .await
        .expect("conversation memory candidates should list");
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].id, memory_item.id);

        let Json(thread_items) = list_conversation_memory_items(
            State(state),
            Query(ConversationMemoryQuery {
                local_thread_id: Some("browser-thread-memory".to_string()),
                query: None,
                limit: Some(10),
            }),
        )
        .await
        .expect("thread conversation memory should list");
        assert_eq!(thread_items.len(), 1);
        assert_eq!(thread_items[0].summary, "用户刚才关注订单风险和客服投诉。");
    }

    #[tokio::test]
    async fn assistant_run_scope_planner_only_uses_visible_datasets() {
        let _guard = shared_local_postgres_test_lock().lock().await;
        std::env::set_var("ASSISTANT_RUN_RUNTIME_MODE", "placeholder");
        let storage = match local_postgres_storage().await {
            Ok(storage) => storage,
            Err(reason) => {
                eprintln!("skipping assistant scope visibility test: {reason}");
                return;
            }
        };
        reset_and_sync_test_storage(&storage).await;

        let tenant = storage
            .ensure_tenant(
                &format!("assistant-scope-visibility-test-{}", Uuid::new_v4()),
                "Assistant Scope Visibility Test",
            )
            .await
            .expect("tenant should exist");
        let state = AppState::new(
            storage,
            workflow_definitions::catalog(),
            tenant.id,
            EventBus::Disabled,
        );
        let private_secret_binding_id = SecretBindingId::new();
        let private_dataset = state
            .storage
            .datasets()
            .create_with_metadata(
                state.tenant_id,
                NewDataset {
                    key: format!("private-orders-{}", Uuid::new_v4()),
                    title: "私密订单".to_string(),
                    description: Some("只能由匹配本地密钥看到的订单数据集。".to_string()),
                },
                json!({
                    "visibility": DatasetVisibility::Private.as_str(),
                    "default_secret_binding_ids": [private_secret_binding_id],
                }),
            )
            .await
            .expect("private dataset should be created");

        let (_, Json(without_secret)) = create_assistant_run(
            State(state.clone()),
            HeaderMap::new(),
            Json(CreateAssistantRunRequest {
                prompt: "帮我继续看私密订单风险".to_string(),
                local_thread_id: Some("scope-visibility-thread".to_string()),
                startup_briefing: Some(json!({})),
                selected_scope: None,
                scope_candidates: vec![json!({
                    "type": "dataset",
                    "id": private_dataset.id,
                    "label": private_dataset.title,
                    "source": "untrusted_frontend",
                })],
                context_policy_hint: None,
                current_artifact: None,
                messages: Vec::new(),
            }),
        )
        .await
        .expect("assistant run should be created without secret");
        assert!(!without_secret
            .scope_candidates
            .iter()
            .any(|candidate| candidate.get("id") == Some(&json!(private_dataset.id))));

        let mut headers = HeaderMap::new();
        headers.insert(
            ACTIVE_SECRET_BINDING_IDS_HEADER,
            axum::http::HeaderValue::from_str(&private_secret_binding_id.to_string())
                .expect("secret header should parse"),
        );
        let (_, Json(with_secret)) = create_assistant_run(
            State(state),
            headers,
            Json(CreateAssistantRunRequest {
                prompt: "帮我继续看私密订单风险".to_string(),
                local_thread_id: Some("scope-visibility-thread".to_string()),
                startup_briefing: Some(json!({})),
                selected_scope: None,
                scope_candidates: Vec::new(),
                context_policy_hint: None,
                current_artifact: None,
                messages: Vec::new(),
            }),
        )
        .await
        .expect("assistant run should be created with secret");
        assert!(with_secret
            .scope_candidates
            .iter()
            .any(|candidate| candidate.get("id") == Some(&json!(private_dataset.id))));
    }

    #[tokio::test]
    async fn assistant_run_supplies_ranked_retrieval_evidence_for_selected_scope() {
        let _guard = shared_local_postgres_test_lock().lock().await;
        std::env::set_var("ASSISTANT_RUN_RUNTIME_MODE", "placeholder");
        std::env::remove_var("ASSISTANT_RUN_EVIDENCE_LIMIT");
        let storage = match local_postgres_storage().await {
            Ok(storage) => storage,
            Err(reason) => {
                eprintln!("skipping assistant run evidence supply test: {reason}");
                return;
            }
        };
        reset_and_sync_test_storage(&storage).await;

        let tenant = storage
            .ensure_tenant(
                &format!("assistant-evidence-supply-test-{}", Uuid::new_v4()),
                "Assistant Evidence Supply Test",
            )
            .await
            .expect("tenant should exist");
        let state = AppState::new(
            storage,
            workflow_definitions::catalog(),
            tenant.id,
            EventBus::Disabled,
        );
        let dataset = state
            .storage
            .datasets()
            .create(
                state.tenant_id,
                NewDataset {
                    key: format!("orders-evidence-{}", Uuid::new_v4()),
                    title: "订单风险".to_string(),
                    description: Some("订单履约和延期风险资料。".to_string()),
                },
            )
            .await
            .expect("dataset should be created");
        let document = state
            .storage
            .documents()
            .create(
                state.tenant_id,
                NewDocument {
                    dataset_id: dataset.id,
                    title: "Order Risk Notes".to_string(),
                    object_key: "documents/order-risk-notes.md".to_string(),
                    content_type: "text/markdown".to_string(),
                    secret_binding_ids: Vec::new(),
                    metadata: json!({}),
                },
            )
            .await
            .expect("document should be created");
        let now = Utc::now();
        let evidence_execution = create_test_workflow_execution(
            &state.storage,
            state.tenant_id,
            dataset.id,
            WorkflowKind::UploadIngest,
        )
        .await;
        let chunks = state
            .storage
            .document_chunks()
            .replace_for_document(
                state.tenant_id,
                document.id,
                &[
                    storage::NewDocumentChunk {
                        dataset_id: dataset.id,
                        document_id: document.id,
                        chunk_index: 0,
                        content: "Order delay risk rises when warehouse handoff exceeds two days."
                            .to_string(),
                        token_count: 12,
                        metadata: json!({"section": "risk"}),
                        created_at: now,
                    },
                    storage::NewDocumentChunk {
                        dataset_id: dataset.id,
                        document_id: document.id,
                        chunk_index: 1,
                        content: "Customer newsletter engagement improved last month.".to_string(),
                        token_count: 8,
                        metadata: json!({"section": "newsletter"}),
                        created_at: now,
                    },
                ],
            )
            .await
            .expect("document chunks should be created");
        let evidences = state
            .storage
            .retrieval_evidences()
            .create_many(
                state.tenant_id,
                &[
                    storage::NewRetrievalEvidence {
                        execution_id: evidence_execution.id,
                        dataset_id: dataset.id,
                        document_id: document.id,
                        document_chunk_id: chunks[0].id,
                        chunk_index: chunks[0].chunk_index,
                        source_locator: "documents/order-risk-notes.md#chunk=0".to_string(),
                        content_excerpt:
                            "Order delay risk rises when warehouse handoff exceeds two days."
                                .to_string(),
                        summary: "Order delay risk evidence".to_string(),
                        payload_filter_key: "dataset/order-risk".to_string(),
                        embedding_model: "placeholder-minilm".to_string(),
                        recall_score: 0.30,
                        evidence_manifest: json!({
                            "embedding": {
                                "term_weights": {
                                    "order": 1.0,
                                    "delay": 1.0,
                                    "risk": 1.0
                                }
                            },
                            "recall": { "rank_hint": 2 }
                        }),
                        created_at: now,
                    },
                    storage::NewRetrievalEvidence {
                        execution_id: evidence_execution.id,
                        dataset_id: dataset.id,
                        document_id: document.id,
                        document_chunk_id: chunks[1].id,
                        chunk_index: chunks[1].chunk_index,
                        source_locator: "documents/order-risk-notes.md#chunk=1".to_string(),
                        content_excerpt: "Customer newsletter engagement improved last month."
                            .to_string(),
                        summary: "Newsletter engagement evidence".to_string(),
                        payload_filter_key: "dataset/newsletter".to_string(),
                        embedding_model: "placeholder-minilm".to_string(),
                        recall_score: 0.99,
                        evidence_manifest: json!({
                            "embedding": {
                                "term_weights": {
                                    "newsletter": 1.0,
                                    "engagement": 1.0
                                }
                            },
                            "recall": { "rank_hint": 1 }
                        }),
                        created_at: now,
                    },
                ],
            )
            .await
            .expect("retrieval evidence should be created");

        let (status, Json(response)) = create_assistant_run(
            State(state.clone()),
            HeaderMap::new(),
            Json(CreateAssistantRunRequest {
                prompt: "order delay risk summary".to_string(),
                local_thread_id: Some("assistant-evidence-thread".to_string()),
                startup_briefing: Some(json!({"visibleDatasetCount": 1})),
                selected_scope: Some(json!({
                    "mode": "user_selected",
                    "datasets": [dataset.id],
                })),
                scope_candidates: Vec::new(),
                context_policy_hint: None,
                current_artifact: None,
                messages: Vec::new(),
            }),
        )
        .await
        .expect("assistant run should be created");

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(response.selected_scope["mode"], json!("user_selected"));
        assert_eq!(response.evidence_state["status"], json!("supplied"));
        assert_eq!(
            response.evidence_state["supplied_items"][0]["retrieval_evidence_id"],
            json!(evidences[0].id)
        );
        assert!(response
            .assistant_message
            .content
            .contains("供料状态: 已检索 2 条供料项"));
        assert!(response.execution_trail.iter().any(|step| {
            step.get("label") == Some(&json!("检索供料证据"))
                && step.get("supplied_count") == Some(&json!(2))
        }));

        let Json(detail) =
            get_assistant_run(State(state), Path(response.assistant_run_id.to_string()))
                .await
                .expect("assistant run detail should load");
        assert_eq!(detail.run.evidence_state["status"], json!("supplied"));
        assert_eq!(
            detail.run.evidence_state["supplied_items"][0]["summary"],
            json!("Order delay risk evidence")
        );
    }

    fn test_dataset(
        visibility: DatasetVisibility,
        secret_binding_ids: Vec<SecretBindingId>,
    ) -> Dataset {
        Dataset {
            id: DatasetId::new(),
            tenant_id: TenantId::new(),
            key: format!("dataset-{}", Uuid::new_v4()),
            title: "Dataset Visibility Test".to_string(),
            description: None,
            lifecycle: DatasetLifecycle::Active,
            visibility,
            default_secret_binding_ids: secret_binding_ids,
            metadata: BTreeMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn dataset_visibility_allows_public_without_secret() {
        let dataset = test_dataset(DatasetVisibility::Public, Vec::new());

        assert!(dataset_is_visible(&dataset, &[]));
    }

    #[test]
    fn dataset_visibility_hides_private_without_matching_secret() {
        let secret_binding_id = SecretBindingId::new();
        let dataset = test_dataset(DatasetVisibility::Private, vec![secret_binding_id]);

        assert!(!dataset_is_visible(&dataset, &[]));
        assert!(!dataset_is_visible(&dataset, &[SecretBindingId::new()]));
        assert!(dataset_is_visible(&dataset, &[secret_binding_id]));
    }

    #[tokio::test]
    async fn create_dataset_secret_binding_marks_dataset_private() {
        let _guard = shared_local_postgres_test_lock().lock().await;
        let storage = match local_postgres_storage().await {
            Ok(storage) => storage,
            Err(reason) => {
                eprintln!("skipping dataset secret binding route test: {reason}");
                return;
            }
        };
        reset_and_sync_test_storage(&storage).await;

        let tenant = storage
            .ensure_tenant(
                &format!("dataset-secret-test-{}", Uuid::new_v4()),
                "Dataset Secret Test",
            )
            .await
            .expect("tenant should exist");
        let dataset = storage
            .datasets()
            .create(
                tenant.id,
                NewDataset {
                    key: format!("private-{}", Uuid::new_v4()),
                    title: "Private Dataset".to_string(),
                    description: None,
                },
            )
            .await
            .expect("dataset should be created");
        let state = AppState::new(
            storage,
            workflow_definitions::catalog(),
            tenant.id,
            EventBus::Disabled,
        );

        let (status, Json(response)) = create_dataset_secret_binding(
            State(state.clone()),
            HeaderMap::new(),
            Json(CreateDatasetSecretBindingRequest {
                dataset_id: dataset.id,
                fingerprint: "local-secret-fingerprint".to_string(),
                label: None,
            }),
        )
        .await
        .expect("secret binding should be created");

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(response.dataset.visibility, DatasetVisibility::Private);
        assert_eq!(
            response.secret_binding_id,
            response.dataset.secret_binding_ids[0]
        );

        let Json(without_secret) = list_datasets(State(state.clone()), HeaderMap::new())
            .await
            .expect("datasets should list");
        assert!(!without_secret
            .iter()
            .any(|item| item.id == response.dataset.id));

        let mut headers = HeaderMap::new();
        headers.insert(
            ACTIVE_SECRET_BINDING_IDS_HEADER,
            axum::http::HeaderValue::from_str(&response.secret_binding_id.to_string())
                .expect("secret binding header should parse"),
        );
        let Json(with_secret) = list_datasets(State(state), headers)
            .await
            .expect("datasets should list with secret");
        assert!(with_secret
            .iter()
            .any(|item| item.id == response.dataset.id));
    }

    #[tokio::test]
    async fn create_dataset_with_secret_fingerprint_returns_private_dataset() {
        let _guard = shared_local_postgres_test_lock().lock().await;
        let storage = match local_postgres_storage().await {
            Ok(storage) => storage,
            Err(reason) => {
                eprintln!("skipping private dataset create route test: {reason}");
                return;
            }
        };
        reset_and_sync_test_storage(&storage).await;

        let tenant = storage
            .ensure_tenant(
                &format!("private-create-test-{}", Uuid::new_v4()),
                "Private Create Test",
            )
            .await
            .expect("tenant should exist");
        let state = AppState::new(
            storage,
            workflow_definitions::catalog(),
            tenant.id,
            EventBus::Disabled,
        );

        let (status, Json(dataset)) = create_dataset(
            State(state),
            Json(CreateDatasetRequest {
                key: format!("secret-create-{}", Uuid::new_v4()),
                title: "Secret Created Dataset".to_string(),
                description: None,
                visibility: None,
                secret_binding_ids: Vec::new(),
                secret_fingerprint: Some("create-secret-fingerprint".to_string()),
                secret_label: Some("local-test".to_string()),
            }),
        )
        .await
        .expect("private dataset should be created");

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(dataset.visibility, DatasetVisibility::Private);
        assert_eq!(dataset.secret_binding_ids.len(), 1);
        assert_eq!(dataset.access_warning, None);
    }

    #[tokio::test]
    async fn resolve_dataset_secret_bindings_returns_matching_private_dataset() {
        let _guard = shared_local_postgres_test_lock().lock().await;
        let storage = match local_postgres_storage().await {
            Ok(storage) => storage,
            Err(reason) => {
                eprintln!("skipping dataset secret resolve route test: {reason}");
                return;
            }
        };
        reset_and_sync_test_storage(&storage).await;

        let tenant = storage
            .ensure_tenant(
                &format!("secret-resolve-test-{}", Uuid::new_v4()),
                "Secret Resolve Test",
            )
            .await
            .expect("tenant should exist");
        let state = AppState::new(
            storage,
            workflow_definitions::catalog(),
            tenant.id,
            EventBus::Disabled,
        );
        let (_, Json(dataset)) = create_dataset(
            State(state.clone()),
            Json(CreateDatasetRequest {
                key: format!("resolve-secret-{}", Uuid::new_v4()),
                title: "Resolvable Private Dataset".to_string(),
                description: None,
                visibility: None,
                secret_binding_ids: Vec::new(),
                secret_fingerprint: Some("resolve-fingerprint".to_string()),
                secret_label: Some("resolve-test".to_string()),
            }),
        )
        .await
        .expect("private dataset should be created");

        let Json(response) = resolve_dataset_secret_bindings(
            State(state),
            Json(ResolveDatasetSecretBindingsRequest {
                fingerprint: "resolve-fingerprint".to_string(),
            }),
        )
        .await
        .expect("secret should resolve");

        assert_eq!(response.secret_binding_ids, dataset.secret_binding_ids);
        assert_eq!(response.datasets.len(), 1);
        assert_eq!(response.datasets[0].id, dataset.id);
        assert_eq!(response.datasets[0].visibility, DatasetVisibility::Private);
    }

    fn sample_chat_session_for_report_entry(
        title: &str,
        session_manifest: Value,
        now: DateTime<Utc>,
    ) -> ChatSession {
        ChatSession {
            id: ChatSessionId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            execution_id: WorkflowExecutionId::new(),
            title: title.to_string(),
            latest_memory_directory_id: None,
            latest_dataset_output_id: None,
            session_manifest,
            created_at: now,
            updated_at: now,
        }
    }

    struct ReportEntryApiTestHarness {
        app: Router,
        storage: PgStorage,
        tenant_id: TenantId,
        session_id: ChatSessionId,
    }

    struct AppendTurnApiTestHarness {
        app: Router,
        storage: PgStorage,
        tenant_id: TenantId,
        dataset_id: DatasetId,
        session_id: ChatSessionId,
    }

    async fn build_report_entry_api_test_harness(
        report_entry: Option<Value>,
    ) -> Option<ReportEntryApiTestHarness> {
        let storage = match local_postgres_storage().await {
            Ok(storage) => storage,
            Err(reason) => {
                eprintln!("skipping report_entry route test: {reason}");
                return None;
            }
        };
        reset_and_sync_test_storage(&storage).await;

        let workflow_catalog = workflow_definitions::catalog();

        let tenant = storage
            .ensure_tenant(
                &format!("platform-api-test-{}", Uuid::new_v4()),
                "Platform API Test",
            )
            .await
            .expect("tenant should exist");
        let dataset = storage
            .datasets()
            .create(
                tenant.id,
                NewDataset {
                    key: format!("dataset-{}", Uuid::new_v4()),
                    title: "Report Entry Dataset".to_string(),
                    description: Some("Dataset used for platform-api route tests.".to_string()),
                },
            )
            .await
            .expect("dataset should be created");

        let now = Utc::now();
        let definition = workflow_catalog
            .find_definition(WorkflowKind::ChatSession)
            .expect("chat session workflow definition should exist");
        let execution = WorkflowExecution {
            id: WorkflowExecutionId::new(),
            tenant_id: tenant.id,
            dataset_id: Some(dataset.id),
            report_plan_id: None,
            kind: WorkflowKind::ChatSession,
            version: definition.version().to_string(),
            stage: "queued".to_string(),
            status: WorkflowStatus::Pending,
            attempt: 0,
            context: json!({}),
            created_at: now,
            updated_at: now,
        };
        storage
            .workflow_executions()
            .create(&execution)
            .await
            .expect("workflow execution should be created");

        let mut session_manifest = json!({
            "generator": "chat-session-workflow",
            "schema_version": "0.3.0",
            "status": "assistant_replied",
            "initial_prompt": "Summarize the dataset",
            "last_prompt": "Turn this into a report",
            "context_binding": "creation_time",
        });
        if let Some(report_entry) = report_entry {
            session_manifest
                .as_object_mut()
                .expect("session manifest should be an object")
                .insert("report_entry".to_string(), report_entry);
        }

        let session = storage
            .chat_sessions()
            .create(
                tenant.id,
                &NewChatSession {
                    id: ChatSessionId::new(),
                    execution_id: execution.id,
                    dataset_id: dataset.id,
                    title: "Route test chat session".to_string(),
                    latest_memory_directory_id: None,
                    latest_dataset_output_id: None,
                    session_manifest,
                    created_at: now,
                },
            )
            .await
            .expect("chat session should be created");

        let app = router(
            storage.clone(),
            workflow_catalog,
            tenant.id,
            EventBus::Disabled,
        );

        Some(ReportEntryApiTestHarness {
            app,
            storage,
            tenant_id: tenant.id,
            session_id: session.id,
        })
    }

    async fn build_append_turn_api_test_harness(
        report_entry: Option<Value>,
    ) -> Option<AppendTurnApiTestHarness> {
        let storage = match local_postgres_storage().await {
            Ok(storage) => storage,
            Err(reason) => {
                eprintln!("skipping append turn route test: {reason}");
                return None;
            }
        };
        reset_and_sync_test_storage(&storage).await;

        let tenant = storage
            .ensure_tenant(
                &format!("platform-api-test-{}", Uuid::new_v4()),
                "Platform API Test",
            )
            .await
            .expect("tenant should exist");
        let dataset = storage
            .datasets()
            .create(
                tenant.id,
                NewDataset {
                    key: format!("dataset-{}", Uuid::new_v4()),
                    title: "Append Turn Dataset".to_string(),
                    description: Some("Dataset used for append turn tests.".to_string()),
                },
            )
            .await
            .expect("dataset should be created");
        let base_execution = create_test_workflow_execution(
            &storage,
            tenant.id,
            dataset.id,
            WorkflowKind::ChatSession,
        )
        .await;
        let now = Utc::now();
        let mut session_manifest = json!({
            "generator": "chat-session-workflow",
            "schema_version": "0.3.0",
            "status": "assistant_replied",
            "initial_prompt": "Initial question",
            "last_prompt": "Initial question",
            "context_binding": "creation_time",
        });
        if let Some(report_entry) = report_entry {
            session_manifest
                .as_object_mut()
                .expect("session manifest should be an object")
                .insert("report_entry".to_string(), report_entry);
        }

        let session = storage
            .chat_sessions()
            .create(
                tenant.id,
                &NewChatSession {
                    id: ChatSessionId::new(),
                    execution_id: base_execution.id,
                    dataset_id: dataset.id,
                    title: "Append turn session".to_string(),
                    latest_memory_directory_id: None,
                    latest_dataset_output_id: None,
                    session_manifest,
                    created_at: now,
                },
            )
            .await
            .expect("chat session should be created");
        storage
            .chat_messages()
            .create(
                tenant.id,
                &NewChatMessage {
                    session_id: session.id,
                    role: ChatMessageRole::User,
                    turn_index: 0,
                    content: "Initial question".to_string(),
                    message_manifest: json!({ "source": "test_user_prompt" }),
                    created_at: now,
                },
            )
            .await
            .expect("initial user message should be created");
        storage
            .chat_messages()
            .create(
                tenant.id,
                &NewChatMessage {
                    session_id: session.id,
                    role: ChatMessageRole::Assistant,
                    turn_index: 1,
                    content: "Initial answer".to_string(),
                    message_manifest: json!({ "source": "test_assistant_reply" }),
                    created_at: now,
                },
            )
            .await
            .expect("initial assistant message should be created");

        let app = router(
            storage.clone(),
            workflow_definitions::catalog(),
            tenant.id,
            EventBus::Disabled,
        );

        Some(AppendTurnApiTestHarness {
            app,
            storage,
            tenant_id: tenant.id,
            dataset_id: dataset.id,
            session_id: session.id,
        })
    }

    async fn reset_and_sync_test_storage(storage: &PgStorage) {
        reset_local_postgres_storage(storage)
            .await
            .expect("test storage should reset");

        let workflow_catalog = workflow_definitions::catalog();
        storage
            .sync_workflow_definitions(&workflow_catalog.descriptors())
            .await
            .expect("workflow definitions should sync");
    }

    async fn create_test_workflow_execution(
        storage: &PgStorage,
        tenant_id: TenantId,
        dataset_id: DatasetId,
        kind: WorkflowKind,
    ) -> WorkflowExecution {
        let workflow_catalog = workflow_definitions::catalog();
        let definition = workflow_catalog
            .find_definition(kind.clone())
            .expect("workflow definition should exist");
        let now = Utc::now();
        let execution = WorkflowExecution {
            id: WorkflowExecutionId::new(),
            tenant_id,
            dataset_id: Some(dataset_id),
            report_plan_id: None,
            kind,
            version: definition.version().to_string(),
            stage: "queued".to_string(),
            status: WorkflowStatus::Pending,
            attempt: 0,
            context: json!({}),
            created_at: now,
            updated_at: now,
        };

        storage
            .workflow_executions()
            .create(&execution)
            .await
            .expect("workflow execution should be created");
        execution
    }

    async fn post_append_turn_request(
        app: Router,
        session_id: ChatSessionId,
        prompt: &str,
    ) -> axum::response::Response {
        let request = axum::http::Request::builder()
            .method("POST")
            .uri(format!("/v1/chat-sessions/{session_id}/turns"))
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(
                serde_json::to_vec(&AppendChatSessionTurnRequest {
                    prompt: prompt.to_string(),
                })
                .expect("request should serialize"),
            ))
            .expect("request should build");

        app.oneshot(request)
            .await
            .expect("request should return a response")
    }

    async fn complete_appended_turn(
        storage: &PgStorage,
        tenant_id: TenantId,
        session_id: ChatSessionId,
        response: &AppendChatSessionTurnResponse,
        assistant_content: &str,
    ) {
        let completed_at = Utc::now();
        let chat_turn_id = response
            .user_message
            .message_manifest
            .get("chat_turn_id")
            .and_then(Value::as_str)
            .expect("user message should carry chat_turn_id")
            .to_string();
        let assistant_message = storage
            .chat_messages()
            .create(
                tenant_id,
                &NewChatMessage {
                    session_id,
                    role: ChatMessageRole::Assistant,
                    turn_index: response.user_message.turn_index + 1,
                    content: assistant_content.to_string(),
                    message_manifest: json!({
                        "source": "test_assistant_reply",
                        "turn": {
                            "turn_id": chat_turn_id,
                            "status": "completed",
                            "assistant_message_id": Value::Null,
                        }
                    }),
                    created_at: completed_at,
                },
            )
            .await
            .expect("assistant message should be created");
        let session = storage
            .chat_sessions()
            .get_by_id(tenant_id, session_id)
            .await
            .expect("chat session should load")
            .expect("chat session should exist");
        let mut session_manifest = session
            .session_manifest
            .as_object()
            .cloned()
            .unwrap_or_default();
        let mut last_turn = session_manifest
            .get("last_turn")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        last_turn.insert("status".to_string(), json!("completed"));
        last_turn.insert(
            "assistant_message_id".to_string(),
            json!(assistant_message.id),
        );
        last_turn.insert(
            "assistant_message_persisted_at".to_string(),
            json!(completed_at),
        );
        last_turn.insert("completed_at".to_string(), json!(completed_at));
        session_manifest.insert("status".to_string(), json!("assistant_replied"));
        session_manifest.insert(
            "last_prompt".to_string(),
            json!(response.user_message.content.clone()),
        );
        session_manifest.insert("last_turn".to_string(), Value::Object(last_turn));

        storage
            .chat_sessions()
            .update_context(
                tenant_id,
                session_id,
                session.latest_memory_directory_id,
                session.latest_dataset_output_id,
                &Value::Object(session_manifest),
                completed_at,
            )
            .await
            .expect("chat session should be marked completed");
    }

    async fn post_report_entry_request(
        app: Router,
        session_id: ChatSessionId,
        request: &UpdateChatSessionReportEntryRequest,
    ) -> axum::response::Response {
        let request = axum::http::Request::builder()
            .method("POST")
            .uri(format!("/v1/chat-sessions/{session_id}/report-entry"))
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(
                serde_json::to_vec(request).expect("request should serialize"),
            ))
            .expect("request should build");

        app.oneshot(request)
            .await
            .expect("request should return a response")
    }

    async fn read_json_response<T: serde::de::DeserializeOwned>(
        response: axum::response::Response,
    ) -> T {
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body should load");
        serde_json::from_slice(&body).expect("response body should deserialize")
    }

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

    #[test]
    fn plan_chat_session_report_entry_update_builds_confirmation_request_from_defaults() {
        let now = Utc::now();
        let session = sample_chat_session_for_report_entry(
            "Materials review",
            json!({
                "status": "assistant_replied",
                "initial_prompt": "Review the materials",
                "last_prompt": "Turn this into a report",
            }),
            now,
        );

        let plan = plan_chat_session_report_entry_update(
            &session,
            None,
            &UpdateChatSessionReportEntryRequest {
                action: contracts::ChatSessionReportEntryActionView::RequestConfirmation,
                title: None,
                objective: None,
            },
            now,
        )
        .expect("confirmation request should be planned");

        assert_eq!(
            plan,
            ChatSessionReportEntryUpdatePlan::RequestConfirmation(
                PreparedChatSessionReportEntry {
                    requested_at: now,
                    title: "Materials review Report".to_string(),
                    objective: "Turn the current dataset context into a report-ready output for: Turn this into a report".to_string(),
                }
            )
        );
    }

    #[test]
    fn plan_chat_session_report_entry_update_reuses_suggested_values_for_report_entry() {
        let now = Utc::now();
        let requested_at = now - chrono::TimeDelta::minutes(5);
        let session = sample_chat_session_for_report_entry(
            "Materials review",
            json!({
                "status": "assistant_replied",
                "initial_prompt": "Review the materials",
            }),
            now,
        );
        let current_report_entry = contracts::ChatSessionReportEntryView {
            state: contracts::ModelFacingReportEntryStateView::ConfirmationRequired,
            requested_at: Some(requested_at),
            resolved_at: None,
            resolved_action: None,
            suggested_title: Some("Prepared report title".to_string()),
            suggested_objective: Some("Prepared report objective".to_string()),
            confirmed_report_plan_id: None,
        };

        let plan = plan_chat_session_report_entry_update(
            &session,
            Some(&current_report_entry),
            &UpdateChatSessionReportEntryRequest {
                action: contracts::ChatSessionReportEntryActionView::EnterReportService,
                title: None,
                objective: None,
            },
            now,
        )
        .expect("report entry should reuse suggested values");

        assert_eq!(
            plan,
            ChatSessionReportEntryUpdatePlan::EnterReportService(PreparedChatSessionReportEntry {
                requested_at,
                title: "Prepared report title".to_string(),
                objective: "Prepared report objective".to_string(),
            })
        );
    }

    #[test]
    fn plan_chat_session_report_entry_update_rejects_resetting_confirmed_entry() {
        let now = Utc::now();
        let session = sample_chat_session_for_report_entry(
            "Materials review",
            json!({
                "status": "assistant_replied",
            }),
            now,
        );
        let current_report_entry = contracts::ChatSessionReportEntryView {
            state: contracts::ModelFacingReportEntryStateView::Confirmed,
            requested_at: Some(now),
            resolved_at: Some(now),
            resolved_action: Some(
                contracts::ChatSessionReportEntryResolutionView::EnterReportService,
            ),
            suggested_title: Some("Prepared report title".to_string()),
            suggested_objective: Some("Prepared report objective".to_string()),
            confirmed_report_plan_id: Some(ReportPlanId::new()),
        };

        let error = plan_chat_session_report_entry_update(
            &session,
            Some(&current_report_entry),
            &UpdateChatSessionReportEntryRequest {
                action: contracts::ChatSessionReportEntryActionView::StayMaterialService,
                title: None,
                objective: None,
            },
            now,
        )
        .expect_err("confirmed entry should not be reset");

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.payload.code, "report_entry_already_confirmed");
        assert!(error
            .payload
            .message
            .contains("confirmed report service entry cannot be reset"));
    }

    #[test]
    fn plan_chat_session_report_entry_update_preserves_decline_history_context() {
        let now = Utc::now();
        let requested_at = now - chrono::TimeDelta::minutes(5);
        let session = sample_chat_session_for_report_entry(
            "Materials review",
            json!({
                "status": "assistant_replied",
            }),
            now,
        );
        let current_report_entry = contracts::ChatSessionReportEntryView {
            state: contracts::ModelFacingReportEntryStateView::ConfirmationRequired,
            requested_at: Some(requested_at),
            resolved_at: None,
            resolved_action: None,
            suggested_title: Some("Prepared report title".to_string()),
            suggested_objective: Some("Prepared report objective".to_string()),
            confirmed_report_plan_id: None,
        };

        let plan = plan_chat_session_report_entry_update(
            &session,
            Some(&current_report_entry),
            &UpdateChatSessionReportEntryRequest {
                action: contracts::ChatSessionReportEntryActionView::StayMaterialService,
                title: None,
                objective: None,
            },
            now,
        )
        .expect("decline action should preserve report entry context");

        assert_eq!(
            plan,
            ChatSessionReportEntryUpdatePlan::StayMaterialService(PreparedChatSessionReportEntry {
                requested_at,
                title: "Prepared report title".to_string(),
                objective: "Prepared report objective".to_string(),
            })
        );
    }

    #[test]
    fn write_chat_session_report_entry_persists_confirmed_state_fields() {
        let now = Utc::now();
        let requested_at = now - chrono::TimeDelta::minutes(5);
        let report_plan_id = ReportPlanId::new();
        let mut session_manifest = json!({
            "status": "assistant_replied",
            "generator": "chat-session-workflow",
        });

        write_chat_session_report_entry(
            &mut session_manifest,
            contracts::ModelFacingReportEntryStateView::Confirmed,
            &PreparedChatSessionReportEntry {
                requested_at,
                title: "Prepared report title".to_string(),
                objective: "Prepared report objective".to_string(),
            },
            Some(now),
            Some(contracts::ChatSessionReportEntryResolutionView::EnterReportService),
            Some(report_plan_id),
        )
        .expect("confirmed report entry should be persisted");

        assert_eq!(
            session_manifest["report_entry"]["state"],
            json!("confirmed")
        );
        assert_eq!(
            session_manifest["report_entry"]["suggested_title"],
            json!("Prepared report title")
        );
        assert_eq!(
            session_manifest["report_entry"]["suggested_objective"],
            json!("Prepared report objective")
        );
        assert_eq!(
            session_manifest["report_entry"]["confirmed_report_plan_id"],
            json!(report_plan_id)
        );
        assert_eq!(
            session_manifest["report_entry"]["resolved_action"],
            json!("enter_report_service")
        );
    }

    #[test]
    fn write_chat_session_report_entry_persists_decline_history_fields() {
        let now = Utc::now();
        let requested_at = now - chrono::TimeDelta::minutes(5);
        let mut session_manifest = json!({
            "status": "assistant_replied",
            "generator": "chat-session-workflow",
        });

        write_chat_session_report_entry(
            &mut session_manifest,
            contracts::ModelFacingReportEntryStateView::NotApplicable,
            &PreparedChatSessionReportEntry {
                requested_at,
                title: "Prepared report title".to_string(),
                objective: "Prepared report objective".to_string(),
            },
            Some(now),
            Some(contracts::ChatSessionReportEntryResolutionView::StayMaterialService),
            None,
        )
        .expect("declined report entry should be persisted");

        assert_eq!(
            session_manifest["report_entry"]["state"],
            json!("not_applicable")
        );
        assert_eq!(
            session_manifest["report_entry"]["resolved_action"],
            json!("stay_material_service")
        );
        assert_eq!(
            session_manifest["report_entry"]["confirmed_report_plan_id"],
            Value::Null
        );
    }

    #[test]
    fn model_facing_signal_formatters_use_protocol_strings() {
        assert_eq!(
            format_chat_turn_status(&contracts::ChatTurnStatusView::Completed),
            "completed"
        );
        assert_eq!(
            format_chat_turn_artifact_commit_status(
                &contracts::ChatTurnArtifactCommitStatusView::NotReady
            ),
            "not_ready"
        );
        assert_eq!(
            format_model_facing_report_entry_state(
                &contracts::ModelFacingReportEntryStateView::ConfirmationRequired
            ),
            "confirmation_required"
        );
    }

    #[tokio::test]
    async fn append_chat_session_turn_creates_user_message_and_new_execution() {
        let _guard = shared_local_postgres_test_lock().lock().await;
        let Some(harness) = build_append_turn_api_test_harness(None).await else {
            return;
        };

        let response =
            post_append_turn_request(harness.app, harness.session_id, "Follow-up question").await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let payload: AppendChatSessionTurnResponse = read_json_response(response).await;

        assert_eq!(payload.chat_session.id, harness.session_id);
        assert_eq!(
            payload.chat_session.session_manifest["initial_prompt"],
            json!("Initial question")
        );
        assert_eq!(
            payload.chat_session.session_manifest["last_prompt"],
            json!("Follow-up question")
        );
        assert_eq!(payload.user_message.session_id, harness.session_id);
        assert_eq!(payload.user_message.role, ChatMessageRole::User);
        assert_eq!(payload.user_message.turn_index, 2);
        assert_eq!(payload.user_message.content, "Follow-up question");
        assert_eq!(payload.workflow_execution.kind, WorkflowKind::ChatSession);
        assert_eq!(payload.workflow_execution.status, WorkflowStatus::Pending);

        let persisted_execution = harness
            .storage
            .workflow_executions()
            .get_by_id(harness.tenant_id, payload.workflow_execution.id)
            .await
            .expect("workflow execution should load")
            .expect("workflow execution should exist");
        assert_eq!(persisted_execution.dataset_id, Some(harness.dataset_id));
        assert_eq!(
            persisted_execution.context["chat_session_id"],
            json!(harness.session_id)
        );
        assert_eq!(
            persisted_execution.context["prompt"],
            json!("Follow-up question")
        );

        let messages = harness
            .storage
            .chat_messages()
            .list_by_session(harness.tenant_id, harness.session_id)
            .await
            .expect("chat messages should list");
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[2].id, payload.user_message.id);
    }

    #[tokio::test]
    async fn append_chat_session_turn_supports_three_completed_followups() {
        let _guard = shared_local_postgres_test_lock().lock().await;
        let Some(harness) = build_append_turn_api_test_harness(None).await else {
            return;
        };

        for (index, prompt) in ["Second question", "Third question", "Fourth question"]
            .into_iter()
            .enumerate()
        {
            let response =
                post_append_turn_request(harness.app.clone(), harness.session_id, prompt).await;
            assert_eq!(response.status(), StatusCode::CREATED);
            let payload: AppendChatSessionTurnResponse = read_json_response(response).await;
            assert_eq!(payload.user_message.turn_index, 2 + (index as i32 * 2));
            assert_eq!(payload.user_message.content, prompt);

            complete_appended_turn(
                &harness.storage,
                harness.tenant_id,
                harness.session_id,
                &payload,
                &format!("Answer for {prompt}"),
            )
            .await;
        }

        let messages = harness
            .storage
            .chat_messages()
            .list_by_session(harness.tenant_id, harness.session_id)
            .await
            .expect("chat messages should list");
        assert_eq!(messages.len(), 8);
        assert_eq!(
            messages
                .iter()
                .map(|message| message.turn_index)
                .collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 4, 5, 6, 7]
        );

        let session = harness
            .storage
            .chat_sessions()
            .get_by_id(harness.tenant_id, harness.session_id)
            .await
            .expect("chat session should load")
            .expect("chat session should exist");
        assert_eq!(
            session.session_manifest["status"],
            json!("assistant_replied")
        );
        assert_eq!(
            session.session_manifest["last_prompt"],
            json!("Fourth question")
        );
    }

    #[tokio::test]
    async fn append_chat_session_turn_rejects_when_prior_turn_is_in_progress() {
        let _guard = shared_local_postgres_test_lock().lock().await;
        let Some(harness) = build_append_turn_api_test_harness(None).await else {
            return;
        };

        let first_response =
            post_append_turn_request(harness.app.clone(), harness.session_id, "Second question")
                .await;
        assert_eq!(first_response.status(), StatusCode::CREATED);

        let second_response =
            post_append_turn_request(harness.app, harness.session_id, "Third question").await;
        assert_eq!(second_response.status(), StatusCode::BAD_REQUEST);
        let payload: ApiErrorResponse = read_json_response(second_response).await;
        assert_eq!(payload.code, "chat_session_turn_in_progress");

        let messages = harness
            .storage
            .chat_messages()
            .list_by_session(harness.tenant_id, harness.session_id)
            .await
            .expect("chat messages should list");
        assert_eq!(messages.len(), 3);
    }

    #[tokio::test]
    async fn report_entry_gate_survives_append_turn_and_can_enter_report_service() {
        let _guard = shared_local_postgres_test_lock().lock().await;
        let requested_at = Utc::now();
        let Some(harness) = build_append_turn_api_test_harness(Some(json!({
            "state": "confirmation_required",
            "requested_at": requested_at,
            "resolved_at": Value::Null,
            "resolved_action": Value::Null,
            "suggested_title": "Prepared report title",
            "suggested_objective": "Prepared report objective",
            "confirmed_report_plan_id": Value::Null,
        })))
        .await
        else {
            return;
        };

        let append_response = post_append_turn_request(
            harness.app.clone(),
            harness.session_id,
            "Clarify report scope",
        )
        .await;
        assert_eq!(append_response.status(), StatusCode::CREATED);
        let append_payload: AppendChatSessionTurnResponse =
            read_json_response(append_response).await;
        assert_eq!(
            append_payload
                .chat_session
                .session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.report_entry.as_ref())
                .map(|entry| entry.state.clone()),
            Some(contracts::ModelFacingReportEntryStateView::ConfirmationRequired)
        );

        complete_appended_turn(
            &harness.storage,
            harness.tenant_id,
            harness.session_id,
            &append_payload,
            "Report scope is ready for confirmation.",
        )
        .await;

        let report_response = post_report_entry_request(
            harness.app,
            harness.session_id,
            &UpdateChatSessionReportEntryRequest {
                action: contracts::ChatSessionReportEntryActionView::EnterReportService,
                title: None,
                objective: None,
            },
        )
        .await;
        assert_eq!(report_response.status(), StatusCode::OK);
        let report_payload: UpdateChatSessionReportEntryResponse =
            read_json_response(report_response).await;
        assert!(report_payload.report_plan.is_some());
        assert!(report_payload.workflow_execution.is_some());
        assert_eq!(
            report_payload
                .chat_session
                .session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.report_entry.as_ref())
                .map(|entry| entry.state.clone()),
            Some(contracts::ModelFacingReportEntryStateView::Confirmed)
        );
    }

    #[tokio::test]
    async fn report_entry_route_request_confirmation_persists_pending_gate() {
        let _guard = shared_local_postgres_test_lock().lock().await;
        let Some(harness) = build_report_entry_api_test_harness(None).await else {
            return;
        };

        let response = post_report_entry_request(
            harness.app,
            harness.session_id,
            &UpdateChatSessionReportEntryRequest {
                action: contracts::ChatSessionReportEntryActionView::RequestConfirmation,
                title: None,
                objective: None,
            },
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let payload: UpdateChatSessionReportEntryResponse = read_json_response(response).await;
        assert!(payload.report_plan.is_none());
        assert!(payload.workflow_execution.is_none());
        assert_eq!(
            payload
                .chat_session
                .session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.report_entry.as_ref())
                .map(|entry| entry.state.clone()),
            Some(contracts::ModelFacingReportEntryStateView::ConfirmationRequired)
        );
        assert_eq!(
            payload
                .chat_session
                .session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.report_entry.as_ref())
                .and_then(|entry| entry.resolved_action.clone()),
            None
        );
        assert_eq!(
            payload
                .chat_session
                .model_facing
                .as_ref()
                .map(|summary| summary.continuation_state.clone()),
            Some(contracts::ModelFacingContinuationStateView::NeedsUserConfirmation)
        );
    }

    #[tokio::test]
    async fn report_entry_route_stay_material_service_persists_decline_history() {
        let _guard = shared_local_postgres_test_lock().lock().await;
        let now = Utc::now();
        let Some(harness) = build_report_entry_api_test_harness(Some(json!({
            "state": "confirmation_required",
            "requested_at": now,
            "resolved_at": Value::Null,
            "resolved_action": Value::Null,
            "suggested_title": "Prepared report title",
            "suggested_objective": "Prepared report objective",
            "confirmed_report_plan_id": Value::Null,
        })))
        .await
        else {
            return;
        };

        let response = post_report_entry_request(
            harness.app,
            harness.session_id,
            &UpdateChatSessionReportEntryRequest {
                action: contracts::ChatSessionReportEntryActionView::StayMaterialService,
                title: None,
                objective: None,
            },
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let payload: UpdateChatSessionReportEntryResponse = read_json_response(response).await;
        assert!(payload.report_plan.is_none());
        assert!(payload.workflow_execution.is_none());
        assert_eq!(
            payload
                .chat_session
                .session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.report_entry.as_ref())
                .map(|entry| entry.state.clone()),
            Some(contracts::ModelFacingReportEntryStateView::NotApplicable)
        );
        assert_eq!(
            payload
                .chat_session
                .session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.report_entry.as_ref())
                .and_then(|entry| entry.resolved_action.clone()),
            Some(contracts::ChatSessionReportEntryResolutionView::StayMaterialService)
        );
        assert_eq!(
            payload
                .chat_session
                .model_facing
                .as_ref()
                .map(|summary| summary.report_entry_state.clone()),
            Some(contracts::ModelFacingReportEntryStateView::NotApplicable)
        );

        let persisted_session = harness
            .storage
            .chat_sessions()
            .get_by_id(harness.tenant_id, harness.session_id)
            .await
            .expect("persisted session should load")
            .expect("persisted session should exist");
        let persisted_manifest =
            parse_chat_session_manifest(&persisted_session.session_manifest).expect("manifest");
        assert_eq!(
            persisted_manifest
                .report_entry
                .as_ref()
                .and_then(|entry| entry.resolved_action.clone()),
            Some(contracts::ChatSessionReportEntryResolutionView::StayMaterialService)
        );
    }

    #[tokio::test]
    async fn report_entry_route_enter_report_service_creates_plan_and_execution() {
        let _guard = shared_local_postgres_test_lock().lock().await;
        let now = Utc::now();
        let Some(harness) = build_report_entry_api_test_harness(Some(json!({
            "state": "confirmation_required",
            "requested_at": now,
            "resolved_at": Value::Null,
            "resolved_action": Value::Null,
            "suggested_title": "Prepared report title",
            "suggested_objective": "Prepared report objective",
            "confirmed_report_plan_id": Value::Null,
        })))
        .await
        else {
            return;
        };

        let response = post_report_entry_request(
            harness.app,
            harness.session_id,
            &UpdateChatSessionReportEntryRequest {
                action: contracts::ChatSessionReportEntryActionView::EnterReportService,
                title: None,
                objective: None,
            },
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let payload: UpdateChatSessionReportEntryResponse = read_json_response(response).await;
        let report_plan = payload
            .report_plan
            .as_ref()
            .expect("report plan should be created");
        let workflow_execution = payload
            .workflow_execution
            .as_ref()
            .expect("workflow execution should be created");
        assert_eq!(
            payload
                .chat_session
                .session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.report_entry.as_ref())
                .map(|entry| entry.state.clone()),
            Some(contracts::ModelFacingReportEntryStateView::Confirmed)
        );
        assert_eq!(
            payload
                .chat_session
                .session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.report_entry.as_ref())
                .and_then(|entry| entry.resolved_action.clone()),
            Some(contracts::ChatSessionReportEntryResolutionView::EnterReportService)
        );
        assert_eq!(
            payload
                .chat_session
                .session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.report_entry.as_ref())
                .and_then(|entry| entry.confirmed_report_plan_id),
            Some(report_plan.id)
        );
        assert_eq!(
            payload
                .chat_session
                .model_facing
                .as_ref()
                .map(|summary| summary.report_entry_state.clone()),
            Some(contracts::ModelFacingReportEntryStateView::Confirmed)
        );
        assert_eq!(
            payload
                .chat_session
                .model_facing
                .as_ref()
                .map(|summary| summary.service_lane.clone()),
            Some(contracts::ModelFacingServiceLaneView::ReportService)
        );

        let plans = harness
            .storage
            .report_plans()
            .list_by_tenant(harness.tenant_id)
            .await
            .expect("report plans should list");
        assert!(plans.iter().any(|plan| plan.id == report_plan.id));

        let executions = harness
            .storage
            .workflow_executions()
            .list_by_tenant(harness.tenant_id)
            .await
            .expect("workflow executions should list");
        assert!(executions
            .iter()
            .any(|execution| execution.id == workflow_execution.id));
    }

    #[tokio::test]
    async fn report_entry_route_rejects_confirmed_entry_mutation() {
        let _guard = shared_local_postgres_test_lock().lock().await;
        let now = Utc::now();
        let report_plan_id = ReportPlanId::new();
        let Some(harness) = build_report_entry_api_test_harness(Some(json!({
            "state": "confirmed",
            "requested_at": now,
            "resolved_at": now,
            "resolved_action": "enter_report_service",
            "suggested_title": "Prepared report title",
            "suggested_objective": "Prepared report objective",
            "confirmed_report_plan_id": report_plan_id,
        })))
        .await
        else {
            return;
        };

        let response = post_report_entry_request(
            harness.app,
            harness.session_id,
            &UpdateChatSessionReportEntryRequest {
                action: contracts::ChatSessionReportEntryActionView::RequestConfirmation,
                title: None,
                objective: None,
            },
        )
        .await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let payload: ApiErrorResponse = read_json_response(response).await;
        assert_eq!(payload.code, "report_entry_already_confirmed");
        assert!(payload.message.contains("already been confirmed"));
    }

    #[tokio::test]
    async fn request_report_render_creates_execution_for_planned_report() {
        let _guard = shared_local_postgres_test_lock().lock().await;
        let storage = match local_postgres_storage().await {
            Ok(storage) => storage,
            Err(reason) => {
                eprintln!("skipping request_report_render test: {reason}");
                return;
            }
        };
        reset_and_sync_test_storage(&storage).await;

        let tenant = storage
            .ensure_tenant(
                &format!("platform-api-test-{}", Uuid::new_v4()),
                "Platform API Test",
            )
            .await
            .expect("tenant should exist");
        let dataset = storage
            .datasets()
            .create(
                tenant.id,
                NewDataset {
                    key: format!("dataset-{}", Uuid::new_v4()),
                    title: "Report Render Dataset".to_string(),
                    description: Some(
                        "Dataset used for report render host-surface tests.".to_string(),
                    ),
                },
            )
            .await
            .expect("dataset should be created");
        let plan = storage
            .report_plans()
            .create(
                tenant.id,
                NewReportPlan {
                    dataset_id: dataset.id,
                    title: "Quarterly Report".to_string(),
                    objective: "Summarize the current dataset".to_string(),
                    theme_key: "default-local".to_string(),
                },
            )
            .await
            .expect("report plan should be created");

        let now = Utc::now();
        let ast_version = storage
            .report_plan_ast_versions()
            .create_next_version(
                tenant.id,
                plan.id,
                &storage::NewReportPlanAstVersion {
                    ast: json!({
                        "sections": [
                            { "title": "Overview" }
                        ]
                    }),
                    created_at: now,
                },
            )
            .await
            .expect("report plan ast version should be created");
        storage
            .report_plans()
            .mark_planned(tenant.id, plan.id, ast_version.id, &ast_version.ast, now)
            .await
            .expect("report plan should be marked planned");

        let response = request_report_render(
            storage.clone(),
            tenant.id,
            plan.id,
            CreateReportRenderRequest {
                surface: PublishedSurface::Pc,
            },
        )
        .await
        .expect("planned report should create render execution");

        assert_eq!(response.requested_ast_version_id, ast_version.id);
        assert_eq!(response.surface, PublishedSurface::Pc);
        assert_eq!(response.workflow_execution.kind, WorkflowKind::ReportRender);
        assert_eq!(response.workflow_execution.status, WorkflowStatus::Pending);
        assert_eq!(response.workflow_execution.stage, "queued");

        let persisted_execution = storage
            .workflow_executions()
            .get_by_id(tenant.id, response.workflow_execution.id)
            .await
            .expect("workflow execution should load")
            .expect("workflow execution should exist");
        assert_eq!(persisted_execution.report_plan_id, Some(plan.id));
        assert_eq!(
            persisted_execution.context["report_plan_ast_version_id"],
            json!(ast_version.id)
        );
        assert_eq!(persisted_execution.context["surface"], json!("pc"));
    }

    #[tokio::test]
    async fn request_report_plan_continue_creates_execution_for_existing_draft_plan() {
        let _guard = shared_local_postgres_test_lock().lock().await;
        let storage = match local_postgres_storage().await {
            Ok(storage) => storage,
            Err(reason) => {
                eprintln!("skipping request_report_plan_continue test: {reason}");
                return;
            }
        };
        reset_local_postgres_storage(&storage)
            .await
            .expect("test storage should reset");

        let workflow_catalog = workflow_definitions::catalog();
        storage
            .sync_workflow_definitions(&workflow_catalog.descriptors())
            .await
            .expect("workflow definitions should sync");

        let tenant = storage
            .ensure_tenant(
                &format!("platform-api-test-{}", Uuid::new_v4()),
                "Platform API Test",
            )
            .await
            .expect("tenant should exist");
        let dataset = storage
            .datasets()
            .create(
                tenant.id,
                NewDataset {
                    key: format!("dataset-{}", Uuid::new_v4()),
                    title: "Report Plan Continue Dataset".to_string(),
                    description: Some(
                        "Dataset used for report plan continue host-surface tests.".to_string(),
                    ),
                },
            )
            .await
            .expect("dataset should be created");
        let plan = storage
            .report_plans()
            .create(
                tenant.id,
                NewReportPlan {
                    dataset_id: dataset.id,
                    title: "Draft Report".to_string(),
                    objective: "Turn the dataset into a report plan".to_string(),
                    theme_key: "default-local".to_string(),
                },
            )
            .await
            .expect("report plan should be created");

        let response = request_report_plan_continue(storage.clone(), tenant.id, plan.id)
            .await
            .expect("draft report plan should create a planning execution");

        assert_eq!(response.plan.id, plan.id);
        assert_eq!(response.workflow_execution.kind, WorkflowKind::ReportPlan);
        assert_eq!(response.workflow_execution.status, WorkflowStatus::Pending);
        assert_eq!(response.workflow_execution.stage, "queued");

        let persisted_execution = storage
            .workflow_executions()
            .get_by_id(tenant.id, response.workflow_execution.id)
            .await
            .expect("workflow execution should load")
            .expect("workflow execution should exist");
        assert_eq!(persisted_execution.report_plan_id, Some(plan.id));
        assert_eq!(persisted_execution.kind, WorkflowKind::ReportPlan);
    }

    #[tokio::test]
    async fn request_report_publish_persists_version_and_marks_plan_published() {
        let _guard = shared_local_postgres_test_lock().lock().await;
        let storage = match local_postgres_storage().await {
            Ok(storage) => storage,
            Err(reason) => {
                eprintln!("skipping request_report_publish test: {reason}");
                return;
            }
        };
        reset_local_postgres_storage(&storage)
            .await
            .expect("test storage should reset");

        let workflow_catalog = workflow_definitions::catalog();
        storage
            .sync_workflow_definitions(&workflow_catalog.descriptors())
            .await
            .expect("workflow definitions should sync");

        let tenant = storage
            .ensure_tenant(
                &format!("platform-api-test-{}", Uuid::new_v4()),
                "Platform API Test",
            )
            .await
            .expect("tenant should exist");
        let dataset = storage
            .datasets()
            .create(
                tenant.id,
                NewDataset {
                    key: format!("dataset-{}", Uuid::new_v4()),
                    title: "Report Publish Dataset".to_string(),
                    description: Some(
                        "Dataset used for report publish host-surface tests.".to_string(),
                    ),
                },
            )
            .await
            .expect("dataset should be created");
        let plan = storage
            .report_plans()
            .create(
                tenant.id,
                NewReportPlan {
                    dataset_id: dataset.id,
                    title: "Quarterly Report".to_string(),
                    objective: "Summarize the current dataset".to_string(),
                    theme_key: "default-local".to_string(),
                },
            )
            .await
            .expect("report plan should be created");

        let now = Utc::now();
        let ast_version = storage
            .report_plan_ast_versions()
            .create_next_version(
                tenant.id,
                plan.id,
                &storage::NewReportPlanAstVersion {
                    ast: json!({
                        "sections": [
                            { "title": "Overview" }
                        ]
                    }),
                    created_at: now,
                },
            )
            .await
            .expect("report plan ast version should be created");
        storage
            .report_plans()
            .mark_planned(tenant.id, plan.id, ast_version.id, &ast_version.ast, now)
            .await
            .expect("report plan should be marked planned");

        let report_render_definition = workflow_catalog
            .find_definition(WorkflowKind::ReportRender)
            .expect("report render workflow definition should exist");
        let render_execution = WorkflowExecution {
            id: WorkflowExecutionId::new(),
            tenant_id: tenant.id,
            dataset_id: Some(dataset.id),
            report_plan_id: Some(plan.id),
            kind: WorkflowKind::ReportRender,
            version: report_render_definition.version().to_string(),
            stage: "completed".to_string(),
            status: WorkflowStatus::Succeeded,
            attempt: 0,
            context: json!({
                "report_plan_ast_version_id": ast_version.id,
                "surface": "pc"
            }),
            created_at: now,
            updated_at: now,
        };
        storage
            .workflow_executions()
            .create(&render_execution)
            .await
            .expect("report render execution should be created");
        let render_output = storage
            .report_render_outputs()
            .create(
                tenant.id,
                &storage::NewReportRenderOutput {
                    execution_id: render_execution.id,
                    plan_id: plan.id,
                    dataset_id: dataset.id,
                    ast_version_id: ast_version.id,
                    surface: PublishedSurface::Pc,
                    status: ReportRenderOutputStatus::Rendered,
                    asset_manifest: json!({
                        "kind": "html",
                        "path": "reports/quarterly/pc.html"
                    }),
                    created_at: now,
                },
            )
            .await
            .expect("report render output should be created");

        let first_publish = request_report_publish(
            storage.clone(),
            tenant.id,
            plan.id,
            PublishReportRequest {
                surface: PublishedSurface::Pc,
                publish_note: Some("initial release".to_string()),
            },
        )
        .await
        .expect("report publish should succeed");

        assert_eq!(first_publish.report.plan_id, plan.id);
        assert_eq!(first_publish.report.dataset_id, dataset.id);
        assert_eq!(first_publish.version.version_no, 1);
        assert_eq!(first_publish.version.surface, PublishedSurface::Pc);
        assert_eq!(
            first_publish.version.asset_manifest["path"],
            json!("reports/quarterly/pc.html")
        );
        assert_eq!(
            first_publish.version.asset_manifest["publish_metadata"]["source_render_output_id"],
            json!(render_output.id)
        );
        assert_eq!(
            first_publish.version.asset_manifest["publish_metadata"]["publish_note"],
            json!("initial release")
        );

        let second_publish = request_report_publish(
            storage.clone(),
            tenant.id,
            plan.id,
            PublishReportRequest {
                surface: PublishedSurface::Pc,
                publish_note: Some("republish".to_string()),
            },
        )
        .await
        .expect("republishing should create a new version");

        assert_eq!(second_publish.report.id, first_publish.report.id);
        assert_eq!(second_publish.version.version_no, 2);
        assert_eq!(
            second_publish.version.asset_manifest["publish_metadata"]["publish_note"],
            json!("republish")
        );

        let persisted_report = storage
            .published_reports()
            .get_by_plan(tenant.id, plan.id)
            .await
            .expect("published report should load")
            .expect("published report should exist");
        assert_eq!(
            persisted_report.current_version_id,
            Some(second_publish.version.id)
        );

        let persisted_plan = storage
            .report_plans()
            .get_by_id(tenant.id, plan.id)
            .await
            .expect("report plan should load")
            .expect("report plan should exist");
        assert_eq!(persisted_plan.status, ReportPlanStatus::Published);
    }

    #[tokio::test]
    async fn load_published_report_by_plan_returns_current_version_and_desc_versions() {
        let _guard = shared_local_postgres_test_lock().lock().await;
        let storage = match local_postgres_storage().await {
            Ok(storage) => storage,
            Err(reason) => {
                eprintln!("skipping load_published_report_by_plan test: {reason}");
                return;
            }
        };
        reset_local_postgres_storage(&storage)
            .await
            .expect("test storage should reset");

        let workflow_catalog = workflow_definitions::catalog();
        storage
            .sync_workflow_definitions(&workflow_catalog.descriptors())
            .await
            .expect("workflow definitions should sync");

        let tenant = storage
            .ensure_tenant(
                &format!("platform-api-test-{}", Uuid::new_v4()),
                "Platform API Test",
            )
            .await
            .expect("tenant should exist");
        let dataset = storage
            .datasets()
            .create(
                tenant.id,
                NewDataset {
                    key: format!("dataset-{}", Uuid::new_v4()),
                    title: "Published Report Read Dataset".to_string(),
                    description: Some(
                        "Dataset used for published report read host-surface tests.".to_string(),
                    ),
                },
            )
            .await
            .expect("dataset should be created");
        let plan = storage
            .report_plans()
            .create(
                tenant.id,
                NewReportPlan {
                    dataset_id: dataset.id,
                    title: "Executive Brief".to_string(),
                    objective: "Summarize the current dataset".to_string(),
                    theme_key: "default-local".to_string(),
                },
            )
            .await
            .expect("report plan should be created");

        let now = Utc::now();
        let ast_version = storage
            .report_plan_ast_versions()
            .create_next_version(
                tenant.id,
                plan.id,
                &storage::NewReportPlanAstVersion {
                    ast: json!({
                        "sections": [{ "title": "Overview" }]
                    }),
                    created_at: now,
                },
            )
            .await
            .expect("report plan ast version should be created");
        storage
            .report_plans()
            .mark_planned(tenant.id, plan.id, ast_version.id, &ast_version.ast, now)
            .await
            .expect("report plan should be marked planned");

        let report_render_definition = workflow_catalog
            .find_definition(WorkflowKind::ReportRender)
            .expect("report render workflow definition should exist");
        let render_execution = WorkflowExecution {
            id: WorkflowExecutionId::new(),
            tenant_id: tenant.id,
            dataset_id: Some(dataset.id),
            report_plan_id: Some(plan.id),
            kind: WorkflowKind::ReportRender,
            version: report_render_definition.version().to_string(),
            stage: "completed".to_string(),
            status: WorkflowStatus::Succeeded,
            attempt: 0,
            context: json!({
                "report_plan_ast_version_id": ast_version.id,
                "surface": "mobile"
            }),
            created_at: now,
            updated_at: now,
        };
        storage
            .workflow_executions()
            .create(&render_execution)
            .await
            .expect("report render execution should be created");
        storage
            .report_render_outputs()
            .create(
                tenant.id,
                &storage::NewReportRenderOutput {
                    execution_id: render_execution.id,
                    plan_id: plan.id,
                    dataset_id: dataset.id,
                    ast_version_id: ast_version.id,
                    surface: PublishedSurface::Mobile,
                    status: ReportRenderOutputStatus::Rendered,
                    asset_manifest: json!({
                        "kind": "html",
                        "path": "reports/executive/mobile.html"
                    }),
                    created_at: now,
                },
            )
            .await
            .expect("report render output should be created");

        let first_publish = request_report_publish(
            storage.clone(),
            tenant.id,
            plan.id,
            PublishReportRequest {
                surface: PublishedSurface::Mobile,
                publish_note: Some("v1".to_string()),
            },
        )
        .await
        .expect("first publish should succeed");
        let second_publish = request_report_publish(
            storage.clone(),
            tenant.id,
            plan.id,
            PublishReportRequest {
                surface: PublishedSurface::Mobile,
                publish_note: Some("v2".to_string()),
            },
        )
        .await
        .expect("second publish should succeed");

        let detail = load_published_report_by_plan(storage.clone(), tenant.id, plan.id)
            .await
            .expect("published report detail should load");

        assert_eq!(detail.report.id, first_publish.report.id);
        assert_eq!(
            detail.current_version.as_ref().map(|version| version.id),
            Some(second_publish.version.id)
        );
        assert_eq!(detail.versions.len(), 2);
        assert_eq!(detail.versions[0].id, second_publish.version.id);
        assert_eq!(detail.versions[1].id, first_publish.version.id);
        assert_eq!(
            detail.versions[0].asset_manifest["publish_metadata"]["publish_note"],
            json!("v2")
        );
    }

    #[tokio::test]
    async fn request_memory_directory_refresh_creates_execution_for_dataset() {
        let _guard = shared_local_postgres_test_lock().lock().await;
        let storage = match local_postgres_storage().await {
            Ok(storage) => storage,
            Err(reason) => {
                eprintln!("skipping request_memory_directory_refresh test: {reason}");
                return;
            }
        };
        reset_local_postgres_storage(&storage)
            .await
            .expect("test storage should reset");

        let workflow_catalog = workflow_definitions::catalog();
        storage
            .sync_workflow_definitions(&workflow_catalog.descriptors())
            .await
            .expect("workflow definitions should sync");

        let tenant = storage
            .ensure_tenant(
                &format!("platform-api-test-{}", Uuid::new_v4()),
                "Platform API Test",
            )
            .await
            .expect("tenant should exist");
        let dataset = storage
            .datasets()
            .create(
                tenant.id,
                NewDataset {
                    key: format!("dataset-{}", Uuid::new_v4()),
                    title: "Memory Directory Dataset".to_string(),
                    description: Some(
                        "Dataset used for memory directory host-surface tests.".to_string(),
                    ),
                },
            )
            .await
            .expect("dataset should be created");

        let response = request_memory_directory_refresh(storage.clone(), tenant.id, dataset.id)
            .await
            .expect("dataset should create memory directory execution");

        assert_eq!(
            response.workflow_execution.kind,
            WorkflowKind::MemoryDirectory
        );
        assert_eq!(response.workflow_execution.status, WorkflowStatus::Pending);
        assert_eq!(response.workflow_execution.stage, "queued");

        let persisted_execution = storage
            .workflow_executions()
            .get_by_id(tenant.id, response.workflow_execution.id)
            .await
            .expect("workflow execution should load")
            .expect("workflow execution should exist");
        assert_eq!(persisted_execution.dataset_id, Some(dataset.id));
        assert_eq!(persisted_execution.report_plan_id, None);
    }

    #[tokio::test]
    async fn load_document_detail_returns_document_chunks_and_retrieval_evidences() {
        let _guard = shared_local_postgres_test_lock().lock().await;
        let storage = match local_postgres_storage().await {
            Ok(storage) => storage,
            Err(reason) => {
                eprintln!("skipping load_document_detail test: {reason}");
                return;
            }
        };
        reset_and_sync_test_storage(&storage).await;

        let tenant = storage
            .ensure_tenant(
                &format!("platform-api-test-{}", Uuid::new_v4()),
                "Platform API Test",
            )
            .await
            .expect("tenant should exist");
        let dataset = storage
            .datasets()
            .create(
                tenant.id,
                NewDataset {
                    key: format!("dataset-{}", Uuid::new_v4()),
                    title: "Document Detail Dataset".to_string(),
                    description: Some(
                        "Dataset used for document detail host-surface tests.".to_string(),
                    ),
                },
            )
            .await
            .expect("dataset should be created");
        let document = storage
            .documents()
            .create(
                tenant.id,
                NewDocument {
                    dataset_id: dataset.id,
                    title: "Q1 Notes".to_string(),
                    object_key: "documents/q1-notes.md".to_string(),
                    content_type: "text/markdown".to_string(),
                    secret_binding_ids: Vec::new(),
                    metadata: json!({}),
                },
            )
            .await
            .expect("document should be created");
        let now = Utc::now();
        let evidence_execution = create_test_workflow_execution(
            &storage,
            tenant.id,
            dataset.id,
            WorkflowKind::UploadIngest,
        )
        .await;
        let chunks = storage
            .document_chunks()
            .replace_for_document(
                tenant.id,
                document.id,
                &[storage::NewDocumentChunk {
                    dataset_id: dataset.id,
                    document_id: document.id,
                    chunk_index: 0,
                    content: "Quarterly revenue increased by 12%.".to_string(),
                    token_count: 8,
                    metadata: json!({ "section": "summary" }),
                    created_at: now,
                }],
            )
            .await
            .expect("document chunks should be created");
        storage
            .retrieval_evidences()
            .create_many(
                tenant.id,
                &[storage::NewRetrievalEvidence {
                    execution_id: evidence_execution.id,
                    dataset_id: dataset.id,
                    document_id: document.id,
                    document_chunk_id: chunks[0].id,
                    chunk_index: chunks[0].chunk_index,
                    source_locator: "documents/q1-notes.md#chunk=0".to_string(),
                    content_excerpt: "Quarterly revenue increased by 12%.".to_string(),
                    summary: "Revenue growth summary".to_string(),
                    payload_filter_key: "dataset/q1-notes".to_string(),
                    embedding_model: "placeholder-minilm".to_string(),
                    recall_score: 0.92,
                    evidence_manifest: json!({ "rank": 1 }),
                    created_at: now,
                }],
            )
            .await
            .expect("retrieval evidences should be created");

        let detail = load_document_detail(storage, tenant.id, document.id)
            .await
            .expect("document detail should load");

        assert_eq!(detail.document.id, document.id);
        assert_eq!(detail.document.title, "Q1 Notes");
        assert_eq!(detail.chunks.len(), 1);
        assert_eq!(
            detail.chunks[0].content,
            "Quarterly revenue increased by 12%."
        );
        assert_eq!(detail.retrieval_evidences.len(), 1);
        assert_eq!(detail.retrieval_evidences[0].document_id, document.id);
        assert_eq!(detail.retrieval_evidences[0].chunk_index, 0);
        assert_eq!(
            detail
                .model_facing
                .as_ref()
                .map(|summary| summary.evidence_state.clone()),
            Some(contracts::ModelFacingEvidenceStateView::LiveDetail)
        );
        assert_eq!(
            detail
                .model_facing
                .as_ref()
                .and_then(|summary| summary.recommended_next_action.clone()),
            Some(contracts::ModelFacingNextActionView::AnswerDirectly)
        );
    }

    #[tokio::test]
    async fn compare_documents_returns_multiple_document_details() {
        let _guard = shared_local_postgres_test_lock().lock().await;
        let storage = match local_postgres_storage().await {
            Ok(storage) => storage,
            Err(reason) => {
                eprintln!("skipping compare_documents test: {reason}");
                return;
            }
        };
        reset_and_sync_test_storage(&storage).await;

        let tenant = storage
            .ensure_tenant(
                &format!("platform-api-test-{}", Uuid::new_v4()),
                "Platform API Test",
            )
            .await
            .expect("tenant should exist");
        let dataset = storage
            .datasets()
            .create(
                tenant.id,
                NewDataset {
                    key: format!("dataset-{}", Uuid::new_v4()),
                    title: "Document Compare Dataset".to_string(),
                    description: Some(
                        "Dataset used for document compare host-surface tests.".to_string(),
                    ),
                },
            )
            .await
            .expect("dataset should be created");
        let document_a = storage
            .documents()
            .create(
                tenant.id,
                NewDocument {
                    dataset_id: dataset.id,
                    title: "Contract A".to_string(),
                    object_key: "documents/contract-a.md".to_string(),
                    content_type: "text/markdown".to_string(),
                    secret_binding_ids: Vec::new(),
                    metadata: json!({}),
                },
            )
            .await
            .expect("document A should be created");
        let document_b = storage
            .documents()
            .create(
                tenant.id,
                NewDocument {
                    dataset_id: dataset.id,
                    title: "Contract B".to_string(),
                    object_key: "documents/contract-b.md".to_string(),
                    content_type: "text/markdown".to_string(),
                    secret_binding_ids: Vec::new(),
                    metadata: json!({}),
                },
            )
            .await
            .expect("document B should be created");
        let now = Utc::now();
        let evidence_execution = create_test_workflow_execution(
            &storage,
            tenant.id,
            dataset.id,
            WorkflowKind::UploadIngest,
        )
        .await;
        let chunks_a = storage
            .document_chunks()
            .replace_for_document(
                tenant.id,
                document_a.id,
                &[storage::NewDocumentChunk {
                    dataset_id: dataset.id,
                    document_id: document_a.id,
                    chunk_index: 0,
                    content: "Contract A requires a 30-day notice.".to_string(),
                    token_count: 7,
                    metadata: json!({ "section": "notice" }),
                    created_at: now,
                }],
            )
            .await
            .expect("document A chunks should be created");
        let chunks_b = storage
            .document_chunks()
            .replace_for_document(
                tenant.id,
                document_b.id,
                &[storage::NewDocumentChunk {
                    dataset_id: dataset.id,
                    document_id: document_b.id,
                    chunk_index: 0,
                    content: "Contract B requires a 60-day notice.".to_string(),
                    token_count: 7,
                    metadata: json!({ "section": "notice" }),
                    created_at: now,
                }],
            )
            .await
            .expect("document B chunks should be created");
        storage
            .retrieval_evidences()
            .create_many(
                tenant.id,
                &[
                    storage::NewRetrievalEvidence {
                        execution_id: evidence_execution.id,
                        dataset_id: dataset.id,
                        document_id: document_a.id,
                        document_chunk_id: chunks_a[0].id,
                        chunk_index: chunks_a[0].chunk_index,
                        source_locator: "documents/contract-a.md#chunk=0".to_string(),
                        content_excerpt: "Contract A requires a 30-day notice.".to_string(),
                        summary: "Contract A notice period".to_string(),
                        payload_filter_key: "dataset/contract-a".to_string(),
                        embedding_model: "placeholder-minilm".to_string(),
                        recall_score: 0.89,
                        evidence_manifest: json!({ "rank": 1 }),
                        created_at: now,
                    },
                    storage::NewRetrievalEvidence {
                        execution_id: evidence_execution.id,
                        dataset_id: dataset.id,
                        document_id: document_b.id,
                        document_chunk_id: chunks_b[0].id,
                        chunk_index: chunks_b[0].chunk_index,
                        source_locator: "documents/contract-b.md#chunk=0".to_string(),
                        content_excerpt: "Contract B requires a 60-day notice.".to_string(),
                        summary: "Contract B notice period".to_string(),
                        payload_filter_key: "dataset/contract-b".to_string(),
                        embedding_model: "placeholder-minilm".to_string(),
                        recall_score: 0.91,
                        evidence_manifest: json!({ "rank": 1 }),
                        created_at: now,
                    },
                ],
            )
            .await
            .expect("retrieval evidences should be created");

        let comparison = compare_documents(
            storage,
            tenant.id,
            CompareDocumentsRequest {
                document_ids: vec![document_b.id, document_a.id],
            },
        )
        .await
        .expect("document comparison should load");

        assert_eq!(comparison.documents.len(), 2);
        assert_eq!(comparison.documents[0].document.id, document_b.id);
        assert_eq!(comparison.documents[0].document.title, "Contract B");
        assert_eq!(comparison.documents[0].chunks.len(), 1);
        assert_eq!(comparison.documents[0].retrieval_evidences.len(), 1);
        assert_eq!(
            comparison.documents[0]
                .model_facing
                .as_ref()
                .map(|summary| summary.evidence_state.clone()),
            Some(contracts::ModelFacingEvidenceStateView::LiveDetail)
        );
        assert_eq!(comparison.documents[1].document.id, document_a.id);
        assert_eq!(comparison.documents[1].document.title, "Contract A");
        assert_eq!(comparison.documents[1].chunks.len(), 1);
        assert_eq!(comparison.documents[1].retrieval_evidences.len(), 1);
        assert_eq!(
            comparison
                .model_facing
                .as_ref()
                .map(|summary| summary.evidence_state.clone()),
            Some(contracts::ModelFacingEvidenceStateView::Mixed)
        );
        assert_eq!(
            comparison
                .model_facing
                .as_ref()
                .and_then(|summary| summary.recommended_next_action.clone()),
            Some(contracts::ModelFacingNextActionView::AnswerDirectly)
        );
        assert!(comparison
            .model_facing
            .as_ref()
            .expect("compare documents should expose model_facing summary")
            .allowed_tool_keys
            .contains(&"document.read_detail".to_string()));
    }

    #[tokio::test]
    async fn create_dataset_output_binds_query_ranked_retrieval_evidences() {
        let _guard = shared_local_postgres_test_lock().lock().await;
        let storage = match local_postgres_storage().await {
            Ok(storage) => storage,
            Err(reason) => {
                eprintln!("skipping create_dataset_output retrieval ranking test: {reason}");
                return;
            }
        };
        reset_and_sync_test_storage(&storage).await;

        let tenant = storage
            .ensure_tenant(
                &format!("platform-api-test-{}", Uuid::new_v4()),
                "Platform API Test",
            )
            .await
            .expect("tenant should exist");
        let dataset = storage
            .datasets()
            .create(
                tenant.id,
                NewDataset {
                    key: format!("dataset-{}", Uuid::new_v4()),
                    title: "Dataset Output Retrieval Ranking".to_string(),
                    description: Some(
                        "Dataset used to verify prompt-ranked retrieval evidence binding."
                            .to_string(),
                    ),
                },
            )
            .await
            .expect("dataset should be created");
        let revenue_document = storage
            .documents()
            .create(
                tenant.id,
                NewDocument {
                    dataset_id: dataset.id,
                    title: "Revenue Notes".to_string(),
                    object_key: "documents/revenue.md".to_string(),
                    content_type: "text/markdown".to_string(),
                    secret_binding_ids: Vec::new(),
                    metadata: json!({}),
                },
            )
            .await
            .expect("revenue document should be created");
        let roadmap_document = storage
            .documents()
            .create(
                tenant.id,
                NewDocument {
                    dataset_id: dataset.id,
                    title: "Roadmap Notes".to_string(),
                    object_key: "documents/roadmap.md".to_string(),
                    content_type: "text/markdown".to_string(),
                    secret_binding_ids: Vec::new(),
                    metadata: json!({}),
                },
            )
            .await
            .expect("roadmap document should be created");
        let now = Utc::now();
        let evidence_execution = create_test_workflow_execution(
            &storage,
            tenant.id,
            dataset.id,
            WorkflowKind::UploadIngest,
        )
        .await;
        let revenue_chunks = storage
            .document_chunks()
            .replace_for_document(
                tenant.id,
                revenue_document.id,
                &[storage::NewDocumentChunk {
                    dataset_id: dataset.id,
                    document_id: revenue_document.id,
                    chunk_index: 0,
                    content: "Revenue and margin expanded across enterprise sales.".to_string(),
                    token_count: 8,
                    metadata: json!({ "section": "finance" }),
                    created_at: now,
                }],
            )
            .await
            .expect("revenue chunks should be created");
        let roadmap_chunks = storage
            .document_chunks()
            .replace_for_document(
                tenant.id,
                roadmap_document.id,
                &[storage::NewDocumentChunk {
                    dataset_id: dataset.id,
                    document_id: roadmap_document.id,
                    chunk_index: 0,
                    content: "Mobile navigation and design system roadmap updates.".to_string(),
                    token_count: 8,
                    metadata: json!({ "section": "product" }),
                    created_at: now,
                }],
            )
            .await
            .expect("roadmap chunks should be created");
        let retrieval_evidences = storage
            .retrieval_evidences()
            .create_many(
                tenant.id,
                &[
                    storage::NewRetrievalEvidence {
                        execution_id: evidence_execution.id,
                        dataset_id: dataset.id,
                        document_id: roadmap_document.id,
                        document_chunk_id: roadmap_chunks[0].id,
                        chunk_index: roadmap_chunks[0].chunk_index,
                        source_locator: "documents/roadmap.md#chunk=0".to_string(),
                        content_excerpt: "Mobile navigation and design system roadmap updates."
                            .to_string(),
                        summary: "Roadmap note".to_string(),
                        payload_filter_key: "dataset/demo".to_string(),
                        embedding_model: "local-lexical-v1".to_string(),
                        recall_score: 0.99,
                        evidence_manifest: json!({
                            "embedding": {
                                "status": "indexed",
                                "model": "local-lexical-v1",
                                "token_count": 8,
                                "term_weights": {
                                    "roadmap": 1.6,
                                    "mobile": 1.5,
                                    "navigation": 1.4
                                }
                            },
                            "recall": {
                                "status": "ready",
                                "score": 0.99,
                                "rank_hint": 1
                            },
                            "evidence": {
                                "document_chunk_id": roadmap_chunks[0].id,
                                "payload_filter_key": "dataset/demo",
                                "source_locator": "documents/roadmap.md#chunk=0"
                            }
                        }),
                        created_at: now,
                    },
                    storage::NewRetrievalEvidence {
                        execution_id: evidence_execution.id,
                        dataset_id: dataset.id,
                        document_id: revenue_document.id,
                        document_chunk_id: revenue_chunks[0].id,
                        chunk_index: revenue_chunks[0].chunk_index,
                        source_locator: "documents/revenue.md#chunk=0".to_string(),
                        content_excerpt: "Revenue and margin expanded across enterprise sales."
                            .to_string(),
                        summary: "Revenue note".to_string(),
                        payload_filter_key: "dataset/demo".to_string(),
                        embedding_model: "local-lexical-v1".to_string(),
                        recall_score: 0.61,
                        evidence_manifest: json!({
                            "embedding": {
                                "status": "indexed",
                                "model": "local-lexical-v1",
                                "token_count": 8,
                                "term_weights": {
                                    "revenue": 1.8,
                                    "margin": 1.6,
                                    "sales": 1.1
                                }
                            },
                            "recall": {
                                "status": "ready",
                                "score": 0.61,
                                "rank_hint": 2
                            },
                            "evidence": {
                                "document_chunk_id": revenue_chunks[0].id,
                                "payload_filter_key": "dataset/demo",
                                "source_locator": "documents/revenue.md#chunk=0"
                            }
                        }),
                        created_at: now,
                    },
                ],
            )
            .await
            .expect("retrieval evidences should be created");
        let state = AppState::new(
            storage.clone(),
            workflow_definitions::catalog(),
            tenant.id,
            EventBus::Disabled,
        );

        let (status, Json(response)) = create_dataset_output(
            State(state),
            HeaderMap::new(),
            Path(dataset.id.to_string()),
            Json(CreateDatasetOutputRequest {
                prompt: "Summarize revenue and margin changes".to_string(),
                chat_session_id: None,
            }),
        )
        .await
        .expect("dataset output request should succeed");

        assert_eq!(status, StatusCode::CREATED);

        let execution = storage
            .workflow_executions()
            .get_by_id(tenant.id, response.workflow_execution.id)
            .await
            .expect("workflow execution should load")
            .expect("workflow execution should exist");
        let bound_retrieval_evidence_ids = execution.context["retrieval_evidence_ids"]
            .as_array()
            .expect("retrieval evidence ids should be present")
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>();

        assert_eq!(bound_retrieval_evidence_ids.len(), 2);
        assert_eq!(
            bound_retrieval_evidence_ids[0],
            retrieval_evidences[1].id.to_string()
        );
        assert_eq!(
            bound_retrieval_evidence_ids[1],
            retrieval_evidences[0].id.to_string()
        );
    }

    #[tokio::test]
    async fn search_dataset_retrieval_returns_ranked_hits_with_document_ids() {
        let _guard = shared_local_postgres_test_lock().lock().await;
        let storage = match local_postgres_storage().await {
            Ok(storage) => storage,
            Err(reason) => {
                eprintln!("skipping retrieval search test: {reason}");
                return;
            }
        };
        reset_and_sync_test_storage(&storage).await;

        let tenant = storage
            .ensure_tenant(
                &format!("platform-api-test-{}", Uuid::new_v4()),
                "Platform API Test",
            )
            .await
            .expect("tenant should exist");
        let dataset = storage
            .datasets()
            .create(
                tenant.id,
                NewDataset {
                    key: format!("dataset-{}", Uuid::new_v4()),
                    title: "Retrieval Search Dataset".to_string(),
                    description: Some(
                        "Dataset used to verify retrieval.search host surface.".to_string(),
                    ),
                },
            )
            .await
            .expect("dataset should be created");
        let revenue_document = storage
            .documents()
            .create(
                tenant.id,
                NewDocument {
                    dataset_id: dataset.id,
                    title: "Revenue Memo".to_string(),
                    object_key: "documents/revenue-memo.md".to_string(),
                    content_type: "text/markdown".to_string(),
                    secret_binding_ids: Vec::new(),
                    metadata: json!({}),
                },
            )
            .await
            .expect("revenue document should be created");
        let roadmap_document = storage
            .documents()
            .create(
                tenant.id,
                NewDocument {
                    dataset_id: dataset.id,
                    title: "Roadmap Memo".to_string(),
                    object_key: "documents/roadmap-memo.md".to_string(),
                    content_type: "text/markdown".to_string(),
                    secret_binding_ids: Vec::new(),
                    metadata: json!({}),
                },
            )
            .await
            .expect("roadmap document should be created");
        let now = Utc::now();
        let evidence_execution = create_test_workflow_execution(
            &storage,
            tenant.id,
            dataset.id,
            WorkflowKind::UploadIngest,
        )
        .await;
        let revenue_chunks = storage
            .document_chunks()
            .replace_for_document(
                tenant.id,
                revenue_document.id,
                &[storage::NewDocumentChunk {
                    dataset_id: dataset.id,
                    document_id: revenue_document.id,
                    chunk_index: 0,
                    content: "Revenue margin improved across enterprise subscriptions.".to_string(),
                    token_count: 7,
                    metadata: json!({ "section": "finance" }),
                    created_at: now,
                }],
            )
            .await
            .expect("revenue chunks should be created");
        let roadmap_chunks = storage
            .document_chunks()
            .replace_for_document(
                tenant.id,
                roadmap_document.id,
                &[storage::NewDocumentChunk {
                    dataset_id: dataset.id,
                    document_id: roadmap_document.id,
                    chunk_index: 0,
                    content: "Roadmap covers mobile navigation and design system cleanup."
                        .to_string(),
                    token_count: 9,
                    metadata: json!({ "section": "product" }),
                    created_at: now,
                }],
            )
            .await
            .expect("roadmap chunks should be created");
        storage
            .retrieval_evidences()
            .create_many(
                tenant.id,
                &[
                    storage::NewRetrievalEvidence {
                        execution_id: evidence_execution.id,
                        dataset_id: dataset.id,
                        document_id: roadmap_document.id,
                        document_chunk_id: roadmap_chunks[0].id,
                        chunk_index: roadmap_chunks[0].chunk_index,
                        source_locator: "documents/roadmap-memo.md#chunk=0".to_string(),
                        content_excerpt:
                            "Roadmap covers mobile navigation and design system cleanup."
                                .to_string(),
                        summary: "Roadmap memo".to_string(),
                        payload_filter_key: "dataset/retrieval-search".to_string(),
                        embedding_model: "local-lexical-v1".to_string(),
                        recall_score: 0.97,
                        evidence_manifest: json!({
                            "schema_version": "0.4.0",
                            "embedding": {
                                "status": "indexed",
                                "model": "local-lexical-v1",
                                "token_count": 9,
                                "term_weights": {
                                    "roadmap": 1.7,
                                    "mobile": 1.4,
                                    "navigation": 1.3
                                }
                            },
                            "recall": {
                                "status": "ready",
                                "score": 0.97,
                                "rank_hint": 1
                            },
                            "evidence": {
                                "document_chunk_id": roadmap_chunks[0].id,
                                "payload_filter_key": "dataset/retrieval-search",
                                "source_locator": "documents/roadmap-memo.md#chunk=0"
                            }
                        }),
                        created_at: now,
                    },
                    storage::NewRetrievalEvidence {
                        execution_id: evidence_execution.id,
                        dataset_id: dataset.id,
                        document_id: revenue_document.id,
                        document_chunk_id: revenue_chunks[0].id,
                        chunk_index: revenue_chunks[0].chunk_index,
                        source_locator: "documents/revenue-memo.md#chunk=0".to_string(),
                        content_excerpt: "Revenue margin improved across enterprise subscriptions."
                            .to_string(),
                        summary: "Revenue memo".to_string(),
                        payload_filter_key: "dataset/retrieval-search".to_string(),
                        embedding_model: "local-lexical-v1".to_string(),
                        recall_score: 0.63,
                        evidence_manifest: json!({
                            "schema_version": "0.4.0",
                            "embedding": {
                                "status": "indexed",
                                "model": "local-lexical-v1",
                                "token_count": 7,
                                "term_weights": {
                                    "revenue": 1.8,
                                    "margin": 1.6,
                                    "enterprise": 1.2
                                }
                            },
                            "recall": {
                                "status": "ready",
                                "score": 0.63,
                                "rank_hint": 2
                            },
                            "evidence": {
                                "document_chunk_id": revenue_chunks[0].id,
                                "payload_filter_key": "dataset/retrieval-search",
                                "source_locator": "documents/revenue-memo.md#chunk=0"
                            }
                        }),
                        created_at: now,
                    },
                ],
            )
            .await
            .expect("retrieval evidences should be created");

        let response = search_dataset_retrieval(
            storage,
            tenant.id,
            dataset.id,
            "Summarize revenue margin changes".to_string(),
            Some(2),
        )
        .await
        .expect("retrieval search should load hits");

        assert_eq!(response.hits.len(), 2);
        assert_eq!(response.hits[0].document_id, revenue_document.id);
        assert_eq!(
            response.hits[0].source_locator,
            "documents/revenue-memo.md#chunk=0"
        );
        assert!(response.hits[0].score >= response.hits[1].score);
        assert_eq!(response.hits[1].document_id, roadmap_document.id);
    }

    #[tokio::test]
    async fn request_workflow_retry_restarts_failed_execution_and_enqueues_task() {
        let _guard = shared_local_postgres_test_lock().lock().await;
        let storage = match local_postgres_storage().await {
            Ok(storage) => storage,
            Err(reason) => {
                eprintln!("skipping request_workflow_retry test: {reason}");
                return;
            }
        };
        reset_and_sync_test_storage(&storage).await;

        let tenant = storage
            .ensure_tenant(
                &format!("platform-api-test-{}", Uuid::new_v4()),
                "Platform API Test",
            )
            .await
            .expect("tenant should exist");
        let dataset = storage
            .datasets()
            .create(
                tenant.id,
                NewDataset {
                    key: format!("dataset-{}", Uuid::new_v4()),
                    title: "Retry Workflow Dataset".to_string(),
                    description: Some(
                        "Dataset used for workflow retry host-surface tests.".to_string(),
                    ),
                },
            )
            .await
            .expect("dataset should be created");
        let state = AppState::new(
            storage.clone(),
            workflow_definitions::catalog(),
            tenant.id,
            EventBus::Disabled,
        );
        let execution = build_initial_memory_directory_execution(&state, dataset.id)
            .expect("initial memory directory execution should build");
        storage
            .workflow_executions()
            .create(&execution)
            .await
            .expect("workflow execution should be created");

        let started = apply_workflow_signal(&state, execution.id, WorkflowSignal::Start)
            .await
            .expect("workflow should start");
        let failed = apply_workflow_signal(
            &state,
            execution.id,
            WorkflowSignal::StepFailed {
                task_key: started.enqueued_tasks[0].task_key.clone(),
                error: "transient worker error".to_string(),
            },
        )
        .await
        .expect("workflow should fail");
        assert_eq!(failed.execution.status, WorkflowStatus::Failed);

        let response = request_workflow_retry(
            storage.clone(),
            tenant.id,
            execution.id,
            RetryWorkflowExecutionRequest {
                reason: "retry after transient worker error".to_string(),
            },
        )
        .await
        .expect("workflow retry should succeed");

        assert_eq!(
            response.retry_transition.persisted_event.event_name,
            "workflow.retry_requested"
        );
        assert_eq!(
            response.retry_transition.execution.status,
            WorkflowStatus::Pending
        );
        assert_eq!(response.retry_transition.execution.stage, "queued");
        assert!(response.retry_transition.enqueued_tasks.is_empty());

        let restart = response
            .restart_transition
            .expect("retry should automatically restart the workflow");
        assert_eq!(restart.persisted_event.event_name, "workflow.started");
        assert_eq!(restart.execution.status, WorkflowStatus::Running);
        assert_eq!(restart.enqueued_tasks.len(), 1);

        let persisted_execution = storage
            .workflow_executions()
            .get_by_id(tenant.id, execution.id)
            .await
            .expect("workflow execution should load")
            .expect("workflow execution should exist");
        assert_eq!(persisted_execution.status, WorkflowStatus::Running);
        assert_eq!(persisted_execution.attempt, 2);
        assert_eq!(persisted_execution.context["retries_remaining"], json!(2));
        assert_eq!(
            persisted_execution.context["retry_reason"],
            json!("retry after transient worker error")
        );
    }

    #[tokio::test]
    async fn publish_workflow_transition_events_emits_execution_and_task_notifications() {
        let now = Utc::now();
        let event_bus = EventBus::in_memory();
        let execution = WorkflowExecution {
            id: WorkflowExecutionId::new(),
            tenant_id: TenantId::new(),
            dataset_id: Some(DatasetId::new()),
            report_plan_id: Some(ReportPlanId::new()),
            kind: WorkflowKind::ReportPlan,
            version: "0.1.0".to_string(),
            stage: "plan_report_ast".to_string(),
            status: WorkflowStatus::Running,
            attempt: 1,
            context: json!({}),
            created_at: now,
            updated_at: now,
        };
        let persisted_event = WorkflowEventRecord {
            id: domain_model::WorkflowEventId::new(),
            execution_id: execution.id,
            sequence_no: 2,
            event_name: "workflow.started".to_string(),
            payload: json!({ "task_key": "plan_report_ast" }),
            created_at: now,
        };
        let task = WorkflowTask {
            id: domain_model::WorkflowTaskId::new(),
            tenant_id: execution.tenant_id,
            execution_id: execution.id,
            queue: "report".to_string(),
            task_key: "plan_report_ast".to_string(),
            payload: json!({ "execution_id": execution.id }),
            status: domain_model::WorkflowTaskStatus::Queued,
            attempt: 0,
            max_attempts: 3,
            available_at: now,
            claimed_at: None,
            finished_at: None,
            error: None,
            created_at: now,
            updated_at: now,
        };

        publish_workflow_transition_events(
            &event_bus,
            &execution,
            &persisted_event,
            std::slice::from_ref(&task),
        )
        .await;

        let published = event_bus.published_events();
        assert_eq!(published.len(), 2);
        assert_eq!(
            published[0].subject,
            workflow_execution_transition_subject(execution.kind.as_str())
        );
        assert_eq!(
            published[0].payload["event_name"],
            json!("workflow.started")
        );
        assert_eq!(
            published[1].subject,
            workflow_task_enqueued_subject("report", "plan_report_ast")
        );
        assert_eq!(published[1].payload["task_id"], json!(task.id));
    }

    #[test]
    fn to_tool_definition_view_exposes_cli_contract() {
        let tool = ToolDefinition::cli(
            "weather.lookup",
            "Weather Lookup",
            "session",
            json!({
                "type": "object",
                "properties": {
                    "city": { "type": "string" }
                },
                "required": ["city"]
            }),
            json!({
                "type": "object",
                "properties": {
                    "summary": { "type": "string" }
                },
                "required": ["summary"]
            }),
            ToolCliContract {
                argv: vec!["cargo".to_string(), "run".to_string()],
                env_allowlist: vec!["WEATHER_API_KEY".to_string()],
                output_mode: ToolCliOutputMode::Json,
                timeout_ms: Some(10_000),
            },
        );

        let view = to_tool_definition_view(&tool);

        assert_eq!(view.key, "weather.lookup");
        assert_eq!(view.invocation_mode, contracts::ToolInvocationModeView::Cli);
        assert_eq!(
            view.cli.as_ref().map(|cli| cli.output_mode.clone()),
            Some(contracts::ToolCliOutputModeView::Json)
        );
        assert_eq!(
            view.cli.as_ref().map(|cli| cli.env_allowlist.clone()),
            Some(vec!["WEATHER_API_KEY".to_string()])
        );
    }

    #[test]
    fn to_tool_definition_view_marks_internal_tools() {
        let tool = ToolDefinition::internal(
            "memory.refresh",
            "Memory Refresh",
            "dataset",
            json!({ "type": "object" }),
            json!({ "type": "object" }),
        );

        let view = to_tool_definition_view(&tool);

        assert_eq!(
            view.invocation_mode,
            contracts::ToolInvocationModeView::Internal
        );
        assert!(view.cli.is_none());
        assert_eq!(tool.invocation_mode, ToolInvocationMode::Internal);
    }

    #[test]
    fn to_document_summary_exposes_typed_lifecycle() {
        let now = Utc::now();
        let document = Document {
            id: DocumentId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            title: "Quarterly Source".to_string(),
            object_key: "documents/q1.pdf".to_string(),
            content_type: "application/pdf".to_string(),
            lifecycle: domain_model::DocumentLifecycle::Indexed,
            secret_binding_ids: vec![SecretBindingId::new()],
            metadata: std::collections::BTreeMap::new(),
            created_at: now,
            updated_at: now,
        };

        let summary = to_document_summary(document);

        assert_eq!(summary.lifecycle, contracts::DocumentLifecycleView::Indexed);
        assert_eq!(summary.content_type, "application/pdf");
        assert_eq!(summary.secret_binding_ids.len(), 1);
    }

    #[test]
    fn parse_dataset_id_rejects_invalid_uuid() {
        let error = parse_dataset_id("not-a-uuid").expect_err("invalid dataset ids should fail");

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.payload.code, "invalid_dataset_id");
    }

    #[test]
    fn parse_dataset_output_id_rejects_invalid_uuid() {
        let error =
            parse_dataset_output_id("not-a-uuid").expect_err("invalid output ids should fail");

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.payload.code, "invalid_dataset_output_id");
    }

    #[test]
    fn to_memory_directory_view_exposes_counts() {
        let now = Utc::now();
        let dataset_id = DatasetId::new();
        let document_id = DocumentId::new();
        let directory = MemoryDirectory {
            id: MemoryDirectoryId::new(),
            tenant_id: TenantId::new(),
            dataset_id,
            execution_id: WorkflowExecutionId::new(),
            version_no: 3,
            directory_nodes: 3,
            refreshed_chunks: 12,
            directory_manifest: json!({
                "schema_version": "0.1.0",
                "generator": "memory-worker",
                "dataset_id": dataset_id,
                "version_no": 3,
                "include_directory": true,
                "root": {
                    "kind": "dataset",
                    "scope": "dataset",
                    "version_no": 3,
                    "title": "Dataset Memory",
                    "document_id": null,
                    "lifecycle": null,
                    "chunk_count": null,
                    "children": [
                        {
                            "kind": "document",
                            "title": "Alpha",
                            "document_id": document_id,
                            "lifecycle": "indexed",
                            "chunk_count": 12,
                            "children": [],
                        }
                    ],
                }
            }),
            created_at: now,
        };

        let view = to_memory_directory_view(directory);

        assert_eq!(view.version_no, 3);
        assert_eq!(view.directory_nodes, 3);
        assert_eq!(view.refreshed_chunks, 12);
        assert_eq!(
            view.directory_manifest["root"]["title"],
            json!("Dataset Memory")
        );
        assert_eq!(view.directory_manifest["version_no"], json!(3));
        assert_eq!(view.directory_manifest["root"]["version_no"], json!(3));
        assert_eq!(
            view.directory_tree.as_ref().map(|tree| tree.version_no),
            Some(3)
        );
        assert_eq!(
            view.directory_tree
                .as_ref()
                .map(|tree| (tree.root.scope.clone(), tree.root.version_no)),
            Some((
                Some(contracts::MemoryDirectoryNodeScopeView::Dataset),
                Some(3)
            ))
        );
        assert_eq!(
            view.directory_tree
                .as_ref()
                .and_then(|tree| tree.root.children.first())
                .and_then(|node| node.document.as_ref())
                .map(|document| (
                    document.id,
                    document.lifecycle.clone(),
                    document.chunk_count
                )),
            Some((document_id, contracts::DocumentLifecycleView::Indexed, 12))
        );
        assert_eq!(
            view.directory_tree
                .as_ref()
                .map(|tree| tree.root.children.first().map(|node| node.title.clone()))
                .flatten()
                .as_deref(),
            Some("Alpha")
        );
    }

    #[test]
    fn to_document_chunk_view_exposes_typed_state() {
        let now = Utc::now();
        let chunk = DocumentChunk {
            id: DocumentChunkId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            document_id: DocumentId::new(),
            chunk_index: 0,
            content: "alpha beta gamma".to_string(),
            token_count: 3,
            state: DocumentChunkState::Extracted,
            metadata: std::collections::BTreeMap::from_iter([(
                "source".to_string(),
                json!("placeholder"),
            )]),
            created_at: now,
            updated_at: now,
        };

        let view = to_document_chunk_view(chunk);

        assert_eq!(view.state, contracts::DocumentChunkStateView::Extracted);
        assert_eq!(view.content, "alpha beta gamma");
        assert_eq!(view.metadata["source"], json!("placeholder"));
    }

    #[test]
    fn to_report_plan_summary_exposes_typed_status() {
        let now = Utc::now();
        let ast_version_id = ReportPlanAstVersionId::new();
        let plan = ReportPlan {
            id: ReportPlanId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            title: "Quarterly Report".to_string(),
            objective: "Summarize product and market signals".to_string(),
            status: ReportPlanStatus::Planned,
            theme_key: "executive-default".to_string(),
            current_ast_version_id: Some(ast_version_id),
            modules: Vec::new(),
            created_at: now,
            updated_at: now,
        };

        let summary = to_report_plan_summary(plan, None);

        assert_eq!(summary.status, contracts::ReportPlanStatusView::Planned);
        assert_eq!(summary.theme_key, "executive-default");
        assert_eq!(summary.current_ast_version_id, Some(ast_version_id));
        assert_eq!(
            summary
                .model_facing
                .as_ref()
                .map(|model| &model.capability_class),
            Some(&contracts::ModelFacingCapabilityClassView::ReportPlanning)
        );
        assert_eq!(
            summary
                .model_facing
                .as_ref()
                .map(|model| &model.service_lane),
            Some(&contracts::ModelFacingServiceLaneView::ReportService)
        );
        assert_eq!(
            summary
                .model_facing
                .as_ref()
                .map(|model| &model.report_entry_state),
            Some(&contracts::ModelFacingReportEntryStateView::Confirmed)
        );
        assert_eq!(
            summary
                .model_facing
                .as_ref()
                .map(|model| &model.evidence_state),
            Some(&contracts::ModelFacingEvidenceStateView::Mixed)
        );
        assert_eq!(
            summary
                .model_facing
                .as_ref()
                .map(|model| &model.continuation_state),
            Some(&contracts::ModelFacingContinuationStateView::NeedsPlatformContinuation)
        );
        assert_eq!(
            summary
                .model_facing
                .as_ref()
                .and_then(|model| model.recommended_next_action.clone()),
            Some(contracts::ModelFacingNextActionView::GenerateReportOutput)
        );
        assert_eq!(
            summary
                .model_facing
                .as_ref()
                .and_then(|model| model.recommended_tool_key.clone()),
            Some("report.render".to_string())
        );
        assert!(summary
            .model_facing
            .as_ref()
            .expect("report plan should expose model_facing summary")
            .allowed_next_actions
            .contains(&contracts::ModelFacingNextActionView::GenerateReportOutput));
        assert!(summary
            .model_facing
            .as_ref()
            .expect("report plan should expose model_facing summary")
            .allowed_tool_keys
            .contains(&"report.render".to_string()));
    }

    #[test]
    fn derive_report_plan_model_facing_summary_marks_missing_ast_as_degraded() {
        let summary = ReportPlanSummary {
            id: ReportPlanId::new(),
            dataset_id: DatasetId::new(),
            title: "Quarterly Report".to_string(),
            objective: "Summarize product and market signals".to_string(),
            status: contracts::ReportPlanStatusView::Planned,
            theme_key: "executive-default".to_string(),
            current_ast_version_id: None,
            service_handoff: None,
            model_facing: None,
        };

        let model_facing = derive_report_plan_model_facing_summary(&summary);

        assert_eq!(
            model_facing.capability_class,
            contracts::ModelFacingCapabilityClassView::ReportPlanning
        );
        assert_eq!(
            model_facing.service_lane,
            contracts::ModelFacingServiceLaneView::ReportService
        );
        assert_eq!(
            model_facing.report_entry_state,
            contracts::ModelFacingReportEntryStateView::Confirmed
        );
        assert_eq!(
            model_facing.evidence_state,
            contracts::ModelFacingEvidenceStateView::Degraded
        );
        assert_eq!(
            model_facing.continuation_state,
            contracts::ModelFacingContinuationStateView::RetryRequired
        );
        assert_eq!(
            model_facing.recommended_next_action,
            Some(contracts::ModelFacingNextActionView::RetryExecution)
        );
        assert_eq!(
            model_facing.allowed_next_actions,
            vec![contracts::ModelFacingNextActionView::RetryExecution]
        );
        assert_eq!(
            model_facing.recommended_tool_key,
            Some("workflow.retry".to_string())
        );
        assert_eq!(
            model_facing.allowed_tool_keys,
            vec!["workflow.retry".to_string()]
        );
    }

    #[test]
    fn derive_report_plan_model_facing_summary_exposes_service_handoff_signals() {
        let report_plan_id = ReportPlanId::new();
        let summary = ReportPlanSummary {
            id: report_plan_id,
            dataset_id: DatasetId::new(),
            title: "Quarterly Report".to_string(),
            objective: "Summarize product and market signals".to_string(),
            status: contracts::ReportPlanStatusView::Planned,
            theme_key: "executive-default".to_string(),
            current_ast_version_id: Some(ReportPlanAstVersionId::new()),
            service_handoff: Some(contracts::ManifestServiceHandoffView {
                source: contracts::ManifestServiceHandoffSourceView::ChatSessionReportEntry,
                service_lane: contracts::ModelFacingServiceLaneView::ReportService,
                report_entry_state: contracts::ModelFacingReportEntryStateView::Confirmed,
                requested_at: Some(Utc::now()),
                resolved_at: Some(Utc::now()),
                resolved_action: Some(
                    contracts::ChatSessionReportEntryResolutionView::EnterReportService,
                ),
                suggested_title: Some("Quarterly Report".to_string()),
                suggested_objective: Some("Summarize product and market signals".to_string()),
                confirmed_report_plan_id: Some(report_plan_id),
            }),
            model_facing: None,
        };

        let model_facing = derive_report_plan_model_facing_summary(&summary);

        assert!(model_facing
            .signals
            .iter()
            .any(|signal| signal == "service_handoff_source=chat_session_report_entry"));
        assert!(model_facing
            .signals
            .iter()
            .any(|signal| *signal == format!("confirmed_report_plan_id={report_plan_id}")));
    }

    #[test]
    fn to_retrieval_evidence_view_exposes_recall_metadata() {
        let now = Utc::now();
        let dataset_id = DatasetId::new();
        let document_id = DocumentId::new();
        let document_chunk_id = DocumentChunkId::new();
        let evidence = RetrievalEvidence {
            id: RetrievalEvidenceId::new(),
            tenant_id: TenantId::new(),
            dataset_id,
            execution_id: WorkflowExecutionId::new(),
            document_id,
            document_chunk_id,
            chunk_index: 4,
            source_locator: "documents/demo.pdf#chunk=4".to_string(),
            content_excerpt: "alpha beta gamma".to_string(),
            summary: "demo summary".to_string(),
            payload_filter_key: "dataset/demo".to_string(),
            embedding_model: "placeholder-embedding-v1".to_string(),
            recall_score: 0.91,
            evidence_manifest: json!({
                "schema_version": "0.2.0",
                "embedding": {
                    "status": "indexed",
                    "model": "placeholder-embedding-v1",
                    "token_count": 42,
                },
                "recall": {
                    "status": "ready",
                    "score": 0.91,
                    "rank_hint": 1,
                },
                "evidence": {
                    "document_chunk_id": document_chunk_id,
                    "payload_filter_key": "dataset/demo",
                    "source_locator": "documents/demo.pdf#chunk=4",
                },
            }),
            created_at: now,
        };

        let view = to_retrieval_evidence_view(evidence);

        assert_eq!(view.chunk_index, 4);
        assert_eq!(view.payload_filter_key, "dataset/demo");
        assert_eq!(view.embedding_model, "placeholder-embedding-v1");
        assert_eq!(view.recall_score, 0.91);
        assert_eq!(
            view.evidence_manifest_view
                .as_ref()
                .map(|manifest| manifest.generator.as_str()),
            Some("retrieval-worker")
        );
        assert_eq!(
            view.evidence_manifest_view
                .as_ref()
                .map(|manifest| manifest.dataset_id),
            Some(dataset_id)
        );
        assert_eq!(
            view.evidence_manifest_view
                .as_ref()
                .map(|manifest| manifest.document_id),
            Some(document_id)
        );
        assert_eq!(
            view.evidence_manifest_view
                .as_ref()
                .map(|manifest| manifest.document_chunk_id),
            Some(document_chunk_id)
        );
        assert_eq!(
            view.evidence_manifest_view
                .as_ref()
                .map(|manifest| manifest.embedding.token_count),
            Some(42)
        );
        assert_eq!(
            view.evidence_manifest_view
                .as_ref()
                .map(|manifest| manifest.recall.rank_hint),
            Some(1)
        );
    }

    #[test]
    fn select_retrieval_evidence_ids_for_prompt_prefers_query_overlap() {
        let now = Utc::now();
        let revenue_id = RetrievalEvidenceId::new();
        let roadmap_id = RetrievalEvidenceId::new();
        let evidences = vec![
            RetrievalEvidence {
                id: roadmap_id,
                tenant_id: TenantId::new(),
                dataset_id: DatasetId::new(),
                execution_id: WorkflowExecutionId::new(),
                document_id: DocumentId::new(),
                document_chunk_id: DocumentChunkId::new(),
                chunk_index: 0,
                source_locator: "documents/roadmap.md#chunk=0".to_string(),
                content_excerpt: "Roadmap covers mobile navigation and design system polish."
                    .to_string(),
                summary: "Roadmap notes".to_string(),
                payload_filter_key: "dataset/demo".to_string(),
                embedding_model: "local-lexical-v1".to_string(),
                recall_score: 0.98,
                evidence_manifest: json!({
                    "embedding": {
                        "status": "indexed",
                        "model": "local-lexical-v1",
                        "token_count": 9,
                        "term_weights": {
                            "roadmap": 1.7,
                            "mobile": 1.4,
                            "navigation": 1.3,
                        }
                    },
                    "recall": {
                        "status": "ready",
                        "score": 0.98,
                        "rank_hint": 1
                    },
                    "evidence": {
                        "document_chunk_id": DocumentChunkId::new(),
                        "payload_filter_key": "dataset/demo",
                        "source_locator": "documents/roadmap.md#chunk=0"
                    }
                }),
                created_at: now,
            },
            RetrievalEvidence {
                id: revenue_id,
                tenant_id: TenantId::new(),
                dataset_id: DatasetId::new(),
                execution_id: WorkflowExecutionId::new(),
                document_id: DocumentId::new(),
                document_chunk_id: DocumentChunkId::new(),
                chunk_index: 1,
                source_locator: "documents/finance.md#chunk=1".to_string(),
                content_excerpt: "Revenue grew while operating margin expanded.".to_string(),
                summary: "Finance notes".to_string(),
                payload_filter_key: "dataset/demo".to_string(),
                embedding_model: "local-lexical-v1".to_string(),
                recall_score: 0.62,
                evidence_manifest: json!({
                    "embedding": {
                        "status": "indexed",
                        "model": "local-lexical-v1",
                        "token_count": 8,
                        "term_weights": {
                            "revenue": 1.8,
                            "margin": 1.5,
                            "operating": 1.2,
                        }
                    },
                    "recall": {
                        "status": "ready",
                        "score": 0.62,
                        "rank_hint": 2
                    },
                    "evidence": {
                        "document_chunk_id": DocumentChunkId::new(),
                        "payload_filter_key": "dataset/demo",
                        "source_locator": "documents/finance.md#chunk=1"
                    }
                }),
                created_at: now,
            },
        ];

        let selected = select_retrieval_evidence_ids_for_prompt(
            &evidences,
            "Summarize revenue and margin changes",
            2,
        );

        assert_eq!(selected, vec![revenue_id, roadmap_id]);
    }

    #[test]
    fn to_report_render_output_view_exposes_typed_status() {
        let now = Utc::now();
        let output = ReportRenderOutput {
            id: ReportRenderOutputId::new(),
            tenant_id: TenantId::new(),
            execution_id: WorkflowExecutionId::new(),
            plan_id: ReportPlanId::new(),
            dataset_id: DatasetId::new(),
            ast_version_id: ReportPlanAstVersionId::new(),
            surface: PublishedSurface::Mobile,
            status: ReportRenderOutputStatus::Rendered,
            asset_manifest: json!({
                "kind": "placeholder_asset",
                "path": "reports/demo/mobile.html",
            }),
            created_at: now,
        };

        let view = to_report_render_output_view(output, None);

        assert_eq!(view.surface, PublishedSurface::Mobile);
        assert_eq!(
            view.status,
            contracts::ReportRenderOutputStatusView::Rendered
        );
        assert_eq!(view.asset_manifest["kind"], json!("placeholder_asset"));
        assert_eq!(
            view.model_facing
                .as_ref()
                .map(|model| &model.capability_class),
            Some(&contracts::ModelFacingCapabilityClassView::ReportGenerationAndEditing)
        );
        assert_eq!(
            view.model_facing.as_ref().map(|model| &model.service_lane),
            Some(&contracts::ModelFacingServiceLaneView::ReportService)
        );
        assert_eq!(
            view.model_facing
                .as_ref()
                .map(|model| &model.report_entry_state),
            Some(&contracts::ModelFacingReportEntryStateView::Confirmed)
        );
        assert_eq!(
            view.model_facing
                .as_ref()
                .map(|model| &model.evidence_state),
            Some(&contracts::ModelFacingEvidenceStateView::Mixed)
        );
        assert_eq!(
            view.model_facing
                .as_ref()
                .map(|model| &model.continuation_state),
            Some(&contracts::ModelFacingContinuationStateView::NeedsPlatformContinuation)
        );
        assert_eq!(
            view.model_facing
                .as_ref()
                .and_then(|model| model.recommended_next_action.clone()),
            Some(contracts::ModelFacingNextActionView::PublishReport)
        );
        assert!(view
            .model_facing
            .as_ref()
            .expect("report render output should expose model_facing summary")
            .allowed_next_actions
            .contains(&contracts::ModelFacingNextActionView::PublishReport));
        assert!(!view
            .model_facing
            .as_ref()
            .expect("report render output should expose model_facing summary")
            .allowed_next_actions
            .contains(&contracts::ModelFacingNextActionView::GenerateReportOutput));
        assert_eq!(
            view.model_facing
                .as_ref()
                .and_then(|model| model.recommended_tool_key.clone()),
            Some("report.publish".to_string())
        );
        assert!(view
            .model_facing
            .as_ref()
            .expect("report render output should expose model_facing summary")
            .allowed_tool_keys
            .contains(&"report.publish".to_string()));
    }

    #[test]
    fn parse_published_report_id_rejects_invalid_uuid() {
        let error = parse_published_report_id("not-a-uuid")
            .expect_err("invalid published report ids should fail");

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.payload.code, "invalid_published_report_id");
    }

    #[test]
    fn derive_report_render_output_model_facing_summary_marks_failed_output_as_degraded() {
        let output = ReportRenderOutputView {
            id: ReportRenderOutputId::new(),
            execution_id: WorkflowExecutionId::new(),
            plan_id: ReportPlanId::new(),
            dataset_id: DatasetId::new(),
            ast_version_id: ReportPlanAstVersionId::new(),
            surface: PublishedSurface::Pc,
            status: contracts::ReportRenderOutputStatusView::Failed,
            asset_manifest: json!({ "kind": "placeholder_asset" }),
            service_handoff: None,
            model_facing: None,
            created_at: Utc::now(),
        };

        let model_facing = derive_report_render_output_model_facing_summary(&output);

        assert_eq!(
            model_facing.capability_class,
            contracts::ModelFacingCapabilityClassView::ReportGenerationAndEditing
        );
        assert_eq!(
            model_facing.service_lane,
            contracts::ModelFacingServiceLaneView::ReportService
        );
        assert_eq!(
            model_facing.report_entry_state,
            contracts::ModelFacingReportEntryStateView::Confirmed
        );
        assert_eq!(
            model_facing.evidence_state,
            contracts::ModelFacingEvidenceStateView::Degraded
        );
        assert_eq!(
            model_facing.continuation_state,
            contracts::ModelFacingContinuationStateView::RetryRequired
        );
        assert_eq!(
            model_facing.recommended_next_action,
            Some(contracts::ModelFacingNextActionView::RetryExecution)
        );
        assert_eq!(
            model_facing.allowed_next_actions,
            vec![contracts::ModelFacingNextActionView::RetryExecution]
        );
        assert_eq!(
            model_facing.recommended_tool_key,
            Some("workflow.retry".to_string())
        );
        assert_eq!(
            model_facing.allowed_tool_keys,
            vec!["workflow.retry".to_string()]
        );
    }

    #[test]
    fn derive_report_render_output_model_facing_summary_exposes_service_handoff_signals() {
        let report_plan_id = ReportPlanId::new();
        let output = ReportRenderOutputView {
            id: ReportRenderOutputId::new(),
            execution_id: WorkflowExecutionId::new(),
            plan_id: report_plan_id,
            dataset_id: DatasetId::new(),
            ast_version_id: ReportPlanAstVersionId::new(),
            surface: PublishedSurface::Pc,
            status: contracts::ReportRenderOutputStatusView::Rendered,
            asset_manifest: json!({ "kind": "placeholder_asset", "path": "reports/demo/pc.html" }),
            service_handoff: Some(contracts::ManifestServiceHandoffView {
                source: contracts::ManifestServiceHandoffSourceView::ChatSessionReportEntry,
                service_lane: contracts::ModelFacingServiceLaneView::ReportService,
                report_entry_state: contracts::ModelFacingReportEntryStateView::Confirmed,
                requested_at: Some(Utc::now()),
                resolved_at: Some(Utc::now()),
                resolved_action: Some(
                    contracts::ChatSessionReportEntryResolutionView::EnterReportService,
                ),
                suggested_title: Some("Quarterly Report".to_string()),
                suggested_objective: Some("Summarize product and market signals".to_string()),
                confirmed_report_plan_id: Some(report_plan_id),
            }),
            model_facing: None,
            created_at: Utc::now(),
        };

        let model_facing = derive_report_render_output_model_facing_summary(&output);

        assert!(model_facing
            .signals
            .iter()
            .any(|signal| signal == "service_handoff_source=chat_session_report_entry"));
        assert!(model_facing
            .signals
            .iter()
            .any(|signal| *signal == format!("confirmed_report_plan_id={report_plan_id}")));
    }

    #[test]
    fn to_dataset_output_view_exposes_retrieval_evidence_ids() {
        let now = Utc::now();
        let dataset_id = DatasetId::new();
        let evidence_ids = vec![RetrievalEvidenceId::new(), RetrievalEvidenceId::new()];
        let memory_directory = MemoryDirectoryView {
            id: MemoryDirectoryId::new(),
            dataset_id,
            execution_id: WorkflowExecutionId::new(),
            version_no: 2,
            directory_nodes: 3,
            refreshed_chunks: 8,
            directory_manifest: json!({
                "schema_version": "0.1.0",
                "generator": "memory-worker",
                "dataset_id": dataset_id,
                "version_no": 2,
                "include_directory": true,
                "root": {
                    "kind": "dataset",
                    "version_no": 2,
                    "scope": "dataset",
                    "title": "Dataset Memory",
                    "document_id": null,
                    "lifecycle": null,
                    "chunk_count": null,
                    "children": []
                }
            }),
            directory_tree: Some(contracts::MemoryDirectoryManifestView {
                schema_version: "0.1.0".to_string(),
                generator: "memory-worker".to_string(),
                dataset_id,
                version_no: 2,
                include_directory: true,
                root: contracts::MemoryDirectoryNodeView {
                    kind: contracts::MemoryDirectoryNodeKind::Dataset,
                    title: "Dataset Memory".to_string(),
                    scope: Some(contracts::MemoryDirectoryNodeScopeView::Dataset),
                    version_no: Some(2),
                    document: None,
                    children: Vec::new(),
                },
            }),
            created_at: now,
        };
        let retrieval_evidences = evidence_ids
            .iter()
            .enumerate()
            .map(|(index, id)| RetrievalEvidenceView {
                id: *id,
                dataset_id: DatasetId::new(),
                document_id: DocumentId::new(),
                document_chunk_id: DocumentChunkId::new(),
                execution_id: WorkflowExecutionId::new(),
                chunk_index: index as i32,
                source_locator: format!("documents/demo.md#chunk={index}"),
                content_excerpt: format!("excerpt-{index}"),
                summary: format!("summary-{index}"),
                payload_filter_key: "dataset/demo".to_string(),
                embedding_model: "placeholder-minilm".to_string(),
                recall_score: 0.9 - (index as f64 * 0.1),
                evidence_manifest: json!({ "rank": index + 1 }),
                evidence_manifest_view: None,
                created_at: now,
            })
            .collect::<Vec<_>>();
        let output = DatasetOutput {
            id: DatasetOutputId::new(),
            tenant_id: TenantId::new(),
            execution_id: WorkflowExecutionId::new(),
            dataset_id,
            prompt: "Summarize".to_string(),
            output_text: "Placeholder output".to_string(),
            memory_directory_id: Some(memory_directory.id),
            retrieval_evidence_ids: evidence_ids.clone(),
            output_manifest: json!({
                "generator": "dataset-output-worker",
                "schema_version": "0.4.0",
                "dataset_id": dataset_id,
                "prompt": "Summarize",
                "indexed_document_count": 2,
                "refreshed_chunks": 8,
                "memory_directory_id": memory_directory.id,
                "memory_directory_version_no": 2,
                "retrieval_evidence_count": evidence_ids.len(),
                "retrieval_evidence_ids": evidence_ids,
                "output": {
                    "format": "markdown",
                    "sections": [{
                        "section_key": "dataset_summary",
                        "kind": "summary",
                        "title": "Dataset Summary",
                        "content": "Placeholder output",
                        "retrieval_evidence_ids": evidence_ids,
                    }]
                },
                "service_handoff": {
                    "source": "chat_session_report_entry",
                    "service_lane": "material_service",
                    "report_entry_state": "not_applicable",
                    "requested_at": null,
                    "resolved_at": null,
                    "resolved_action": null,
                    "suggested_title": null,
                    "suggested_objective": null,
                    "confirmed_report_plan_id": null
                },
                "tool_trace": [{
                    "call_id": "call_dataset_output_1",
                    "tool_name": "retrieval.search",
                    "tool": {
                        "key": "retrieval.search",
                        "title": "Retrieval Search",
                        "scope_policy": "dataset_or_session",
                        "invocation_mode": "cli",
                        "cli": {
                            "argv": [
                                "cargo",
                                "run",
                                "-p",
                                "platform-api",
                                "--bin",
                                "retrieval-search-cli",
                                "--",
                                "search"
                            ],
                            "env_allowlist": [
                                "PLATFORM_DATABASE_URL",
                                "PLATFORM_TENANT_KEY",
                                "PLATFORM_TENANT_NAME"
                            ],
                            "output_mode": "json",
                            "timeout_ms": 30000
                        }
                    },
                    "status": "completed",
                    "arguments": { "query": "Summarize" },
                    "result": { "hits": 2 }
                }],
                "context_binding": "creation_time",
                "runtime": {
                    "mode": "placeholder",
                    "provider": "placeholder",
                    "model": "placeholder-dataset-output-v1",
                    "system_prompt_key": "dataset_output.placeholder",
                    "system_prompt_version": "v1"
                }
            }),
            created_at: now,
        };

        let view = to_dataset_output_view(
            output,
            Some(memory_directory),
            retrieval_evidences,
            Vec::new(),
            Vec::new(),
        );

        assert_eq!(view.retrieval_evidence_ids, evidence_ids);
        assert!(view.llm_invocations.is_empty());
        assert!(view.tool_executions.is_empty());
        assert_eq!(
            view.memory_directory
                .as_ref()
                .map(|directory| directory.version_no),
            Some(2)
        );
        assert_eq!(
            view.memory_directory
                .as_ref()
                .and_then(|directory| directory.directory_tree.as_ref())
                .map(|tree| tree.version_no),
            Some(2)
        );
        assert_eq!(
            view.memory_directory
                .as_ref()
                .and_then(|directory| directory.directory_tree.as_ref())
                .map(|tree| tree.root.kind.clone()),
            Some(contracts::MemoryDirectoryNodeKind::Dataset)
        );
        assert_eq!(view.retrieval_evidences.len(), 2);
        assert_eq!(
            view.retrieval_evidences[0].evidence_manifest["rank"],
            json!(1)
        );
        assert_eq!(view.output_manifest["retrieval_evidence_count"], json!(2));
        assert_eq!(
            view.output_manifest_view
                .as_ref()
                .map(|manifest| manifest.retrieval_evidence_count),
            Some(2)
        );
        assert_eq!(
            view.output_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.output.as_ref())
                .map(|output| output.format.clone()),
            Some(contracts::DatasetOutputFormatView::Markdown)
        );
        assert_eq!(
            view.output_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.output.as_ref())
                .map(|output| output.sections.len()),
            Some(1)
        );
        assert_eq!(
            view.output_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.output.as_ref())
                .and_then(|output| output.sections.first())
                .map(|section| section.kind.clone()),
            Some(contracts::DatasetOutputSectionKindView::Summary)
        );
        assert_eq!(
            view.output_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.output.as_ref())
                .and_then(|output| output.sections.first())
                .map(|section| section.retrieval_evidence_ids.clone()),
            Some(evidence_ids.clone())
        );
        assert_eq!(
            view.output_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.service_handoff.as_ref())
                .map(|handoff| handoff.source.clone()),
            Some(contracts::ManifestServiceHandoffSourceView::ChatSessionReportEntry)
        );
        assert_eq!(
            view.output_manifest_view
                .as_ref()
                .map(|manifest| manifest.tool_trace.len()),
            Some(1)
        );
        assert_eq!(
            view.output_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.tool_trace.first())
                .and_then(|call| call.tool.as_ref())
                .map(|tool| tool.key.as_str()),
            Some("retrieval.search")
        );
        assert_eq!(
            view.output_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.tool_trace.first())
                .and_then(|call| call.tool.as_ref())
                .map(|tool| tool.invocation_mode.clone()),
            Some(contracts::ToolInvocationModeView::Cli)
        );
        assert_eq!(
            view.output_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.tool_trace.first())
                .and_then(|call| call.tool.as_ref())
                .and_then(|tool| tool.cli.as_ref())
                .map(|cli| cli.output_mode.clone()),
            Some(contracts::ToolCliOutputModeView::Json)
        );
        assert_eq!(
            view.output_manifest_view
                .as_ref()
                .map(|manifest| manifest.context_binding.clone()),
            Some(contracts::ManifestContextBindingView::CreationTime)
        );
        assert_eq!(
            view.output_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .map(|runtime| runtime.mode.clone()),
            Some(contracts::ManifestRuntimeModeView::Placeholder)
        );
        assert_eq!(
            view.output_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .and_then(|runtime| runtime.provider.as_deref()),
            Some("placeholder")
        );
        assert_eq!(
            view.output_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .and_then(|runtime| runtime.model.as_deref()),
            Some("placeholder-dataset-output-v1")
        );
        assert_eq!(
            view.output_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .and_then(|runtime| runtime.system_prompt_key.as_deref()),
            Some("dataset_output.placeholder")
        );
        assert_eq!(
            view.output_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .and_then(|runtime| runtime.system_prompt_version.as_deref()),
            Some("v1")
        );
    }

    #[test]
    fn to_dataset_output_view_prefers_llm_invocation_runtime_over_manifest_runtime() {
        let now = Utc::now();
        let dataset_id = DatasetId::new();
        let output_id = DatasetOutputId::new();
        let execution_id = WorkflowExecutionId::new();
        let output = DatasetOutput {
            id: output_id,
            tenant_id: TenantId::new(),
            execution_id,
            dataset_id,
            prompt: "Summarize".to_string(),
            output_text: "Fresh output".to_string(),
            memory_directory_id: None,
            retrieval_evidence_ids: Vec::new(),
            output_manifest: json!({
                "generator": "dataset-output-worker",
                "schema_version": "0.4.0",
                "dataset_id": dataset_id,
                "prompt": "Summarize",
                "indexed_document_count": 1,
                "refreshed_chunks": 1,
                "memory_directory_id": null,
                "memory_directory_version_no": null,
                "retrieval_evidence_count": 0,
                "retrieval_evidence_ids": [],
                "output": {
                    "format": "markdown",
                    "sections": [{
                        "section_key": "dataset_summary",
                        "kind": "summary",
                        "title": "Dataset Summary",
                        "content": "Fresh output",
                        "retrieval_evidence_ids": []
                    }]
                },
                "tool_trace": [],
                "context_binding": "creation_time",
                "runtime": {
                    "mode": "placeholder",
                    "provider": "placeholder",
                    "model": "placeholder-dataset-output-v1",
                    "request_id": "req_dataset_output_stale",
                    "finish_reason": "stop",
                    "latency_ms": 1,
                    "usage": {
                        "input_tokens": 1,
                        "output_tokens": 1,
                        "total_tokens": 2
                    },
                    "system_prompt_key": "dataset_output.placeholder",
                    "system_prompt_version": "v1",
                    "tool_trace_count": 0
                }
            }),
            created_at: now,
        };
        let llm_invocations = vec![contracts::LlmInvocationView {
            id: domain_model::LlmInvocationId::new(),
            execution_id,
            source_kind: contracts::LlmInvocationSourceKindView::DatasetOutput,
            dataset_output_id: Some(output_id),
            chat_message_id: None,
            sequence_no: 1,
            mode: contracts::LlmInvocationModeView::Provider,
            provider: Some("openai".to_string()),
            model: Some("gpt-5.4".to_string()),
            request_id: Some("req_dataset_output_fresh".to_string()),
            finish_reason: Some(contracts::LlmInvocationFinishReasonView::ToolCalls),
            latency_ms: Some(321),
            usage: Some(contracts::LlmTokenUsageView {
                input_tokens: 144,
                output_tokens: 55,
                total_tokens: 199,
            }),
            system_prompt_key: Some("dataset_output.live".to_string()),
            system_prompt_version: Some("v2".to_string()),
            tool_trace_count: Some(3),
            created_at: now,
        }];

        let view = to_dataset_output_view(output, None, Vec::new(), llm_invocations, Vec::new());

        assert_eq!(
            view.output_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .map(|runtime| runtime.mode.clone()),
            Some(contracts::ManifestRuntimeModeView::Provider)
        );
        assert_eq!(
            view.output_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .and_then(|runtime| runtime.provider.as_deref()),
            Some("openai")
        );
        assert_eq!(
            view.output_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .and_then(|runtime| runtime.request_id.as_deref()),
            Some("req_dataset_output_fresh")
        );
        assert_eq!(
            view.output_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .and_then(|runtime| runtime.finish_reason.clone()),
            Some(contracts::ManifestFinishReasonView::ToolCalls)
        );
        assert_eq!(
            view.output_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .and_then(|runtime| runtime.latency_ms),
            Some(321)
        );
        assert_eq!(
            view.output_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .and_then(|runtime| runtime.usage.as_ref())
                .map(|usage| usage.total_tokens),
            Some(199)
        );
        assert_eq!(
            view.output_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .and_then(|runtime| runtime.system_prompt_key.as_deref()),
            Some("dataset_output.live")
        );
        assert_eq!(
            view.output_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .map(|runtime| runtime.tool_trace_count),
            Some(Some(3))
        );
    }

    #[test]
    fn to_dataset_output_view_prefers_tool_executions_over_manifest_tool_trace() {
        let now = Utc::now();
        let output = DatasetOutput {
            id: DatasetOutputId::new(),
            tenant_id: TenantId::new(),
            execution_id: WorkflowExecutionId::new(),
            dataset_id: DatasetId::new(),
            prompt: "Summarize".to_string(),
            output_text: "Fresh output".to_string(),
            memory_directory_id: None,
            retrieval_evidence_ids: Vec::new(),
            output_manifest: json!({
                "generator": "dataset-output-worker",
                "schema_version": "0.4.0",
                "dataset_id": DatasetId::new(),
                "prompt": "Summarize",
                "indexed_document_count": 1,
                "refreshed_chunks": 1,
                "memory_directory_id": null,
                "memory_directory_version_no": null,
                "retrieval_evidence_count": 0,
                "retrieval_evidence_ids": [],
                "output": {
                    "format": "markdown",
                    "sections": [{
                        "section_key": "dataset_summary",
                        "kind": "summary",
                        "title": "Dataset Summary",
                        "content": "Fresh output",
                        "retrieval_evidence_ids": []
                    }]
                },
                "tool_trace": [{
                    "call_id": "call_stale",
                    "tool_name": "stale.tool",
                    "status": "failed",
                    "arguments": { "query": "stale" },
                    "result": { "error": "stale" }
                }],
                "context_binding": "creation_time",
                "runtime": {
                    "mode": "placeholder",
                    "provider": "placeholder",
                    "model": "placeholder-dataset-output-v1",
                    "request_id": "req_dataset_output_stale",
                    "finish_reason": "error",
                    "latency_ms": 1,
                    "tool_trace_count": 99
                }
            }),
            created_at: now,
        };
        let tool_executions = vec![
            contracts::ToolExecutionView {
                id: ToolExecutionId::new(),
                execution_id: output.execution_id,
                source_kind: contracts::ToolExecutionSourceKindView::DatasetOutput,
                dataset_output_id: Some(output.id),
                chat_message_id: None,
                sequence_no: 2,
                call_id: Some("call_two".to_string()),
                tool_name: "retrieval.search".to_string(),
                tool: Some(contracts::ToolReferenceView {
                    key: "retrieval.search".to_string(),
                    title: "Retrieval Search".to_string(),
                    scope_policy: "dataset_or_session".to_string(),
                    invocation_mode: contracts::ToolInvocationModeView::Cli,
                    cli: Some(contracts::ToolCliContractView {
                        argv: vec!["cargo".to_string(), "run".to_string()],
                        env_allowlist: vec!["PLATFORM_DATABASE_URL".to_string()],
                        output_mode: contracts::ToolCliOutputModeView::Json,
                        timeout_ms: Some(30_000),
                    }),
                }),
                status: contracts::ManifestToolCallStatusView::Completed,
                arguments: Some(json!({ "query": "fresh" })),
                result: Some(json!({ "hits": 2 })),
                created_at: now,
            },
            contracts::ToolExecutionView {
                id: ToolExecutionId::new(),
                execution_id: output.execution_id,
                source_kind: contracts::ToolExecutionSourceKindView::DatasetOutput,
                dataset_output_id: Some(output.id),
                chat_message_id: None,
                sequence_no: 1,
                call_id: Some("call_one".to_string()),
                tool_name: "retrieval.prepare".to_string(),
                tool: Some(contracts::ToolReferenceView {
                    key: "retrieval.prepare".to_string(),
                    title: "Retrieval Prepare".to_string(),
                    scope_policy: "dataset".to_string(),
                    invocation_mode: contracts::ToolInvocationModeView::Cli,
                    cli: None,
                }),
                status: contracts::ManifestToolCallStatusView::Requested,
                arguments: Some(json!({ "dataset_id": "demo" })),
                result: None,
                created_at: now,
            },
        ];

        let view = to_dataset_output_view(output, None, Vec::new(), Vec::new(), tool_executions);

        assert_eq!(
            view.output_manifest_view
                .as_ref()
                .map(|manifest| manifest.tool_trace.len()),
            Some(2)
        );
        assert_eq!(
            view.output_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.tool_trace.first())
                .and_then(|call| call.call_id.as_deref()),
            Some("call_one")
        );
        assert_eq!(
            view.output_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.tool_trace.first())
                .map(|call| call.tool_name.as_str()),
            Some("retrieval.prepare")
        );
        assert_eq!(
            view.output_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.tool_trace.first())
                .map(|call| call.status.clone()),
            Some(contracts::ManifestToolCallStatusView::Requested)
        );
        assert_eq!(
            view.output_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.tool_trace.get(1))
                .and_then(|call| call.call_id.as_deref()),
            Some("call_two")
        );
        assert_eq!(
            view.output_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.tool_trace.get(1))
                .and_then(|call| call.tool.as_ref())
                .map(|tool| tool.key.as_str()),
            Some("retrieval.search")
        );
        assert_eq!(
            view.output_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .map(|runtime| runtime.tool_trace_count),
            Some(Some(2))
        );
    }

    #[test]
    fn to_chat_session_view_embeds_latest_dataset_output() {
        let now = Utc::now();
        let dataset_id = DatasetId::new();
        let latest_memory_directory = MemoryDirectoryView {
            id: MemoryDirectoryId::new(),
            dataset_id,
            execution_id: WorkflowExecutionId::new(),
            version_no: 1,
            directory_nodes: 2,
            refreshed_chunks: 4,
            directory_manifest: json!({
                "schema_version": "0.1.0",
                "generator": "memory-worker",
                "dataset_id": dataset_id,
                "version_no": 1,
                "include_directory": true,
                "root": {
                    "kind": "dataset",
                    "version_no": 1,
                    "scope": "dataset",
                    "title": "Dataset Memory",
                    "document_id": null,
                    "lifecycle": null,
                    "chunk_count": null,
                    "children": []
                }
            }),
            directory_tree: Some(contracts::MemoryDirectoryManifestView {
                schema_version: "0.1.0".to_string(),
                generator: "memory-worker".to_string(),
                dataset_id,
                version_no: 1,
                include_directory: true,
                root: contracts::MemoryDirectoryNodeView {
                    kind: contracts::MemoryDirectoryNodeKind::Dataset,
                    title: "Dataset Memory".to_string(),
                    scope: Some(contracts::MemoryDirectoryNodeScopeView::Dataset),
                    version_no: Some(1),
                    document: None,
                    children: Vec::new(),
                },
            }),
            created_at: now,
        };
        let latest_output = DatasetOutputView {
            id: DatasetOutputId::new(),
            dataset_id: DatasetId::new(),
            execution_id: WorkflowExecutionId::new(),
            prompt: "Summarize".to_string(),
            output_text: "Placeholder output".to_string(),
            memory_directory_id: Some(latest_memory_directory.id),
            memory_directory: Some(latest_memory_directory.clone()),
            retrieval_evidence_ids: vec![RetrievalEvidenceId::new()],
            retrieval_evidences: vec![RetrievalEvidenceView {
                id: RetrievalEvidenceId::new(),
                dataset_id: DatasetId::new(),
                document_id: DocumentId::new(),
                document_chunk_id: DocumentChunkId::new(),
                execution_id: WorkflowExecutionId::new(),
                chunk_index: 0,
                source_locator: "documents/demo.md#chunk=0".to_string(),
                content_excerpt: "alpha beta gamma".to_string(),
                summary: "demo summary".to_string(),
                payload_filter_key: "dataset/demo".to_string(),
                embedding_model: "placeholder-minilm".to_string(),
                recall_score: 0.91,
                evidence_manifest: json!({ "rank": 1 }),
                evidence_manifest_view: None,
                created_at: now,
            }],
            llm_invocations: Vec::new(),
            tool_executions: Vec::new(),
            output_manifest: json!({
                "generator": "dataset-output-worker",
                "schema_version": "0.4.0",
                "dataset_id": dataset_id,
                "prompt": "Summarize",
                "indexed_document_count": 1,
                "refreshed_chunks": 4,
                "memory_directory_id": latest_memory_directory.id,
                "memory_directory_version_no": 1,
                "retrieval_evidence_count": 1,
                "retrieval_evidence_ids": [],
                "output": {
                    "format": "markdown",
                    "sections": [{
                        "section_key": "dataset_summary",
                        "kind": "summary",
                        "title": "Dataset Summary",
                        "content": "Placeholder output",
                        "retrieval_evidence_ids": []
                    }]
                },
                "context_binding": "creation_time",
                "runtime": {
                    "mode": "placeholder",
                    "provider": "placeholder",
                    "model": "placeholder-dataset-output-v1",
                    "request_id": "req_dataset_output_placeholder",
                    "finish_reason": "stop",
                    "latency_ms": 0,
                    "usage": {
                        "input_tokens": 11,
                        "output_tokens": 11,
                        "total_tokens": 22
                    },
                    "system_prompt_key": "dataset_output.placeholder",
                    "system_prompt_version": "v1"
                }
            }),
            output_manifest_view: Some(contracts::DatasetOutputManifestView {
                generator: "dataset-output-worker".to_string(),
                schema_version: "0.4.0".to_string(),
                dataset_id,
                prompt: "Summarize".to_string(),
                indexed_document_count: 1,
                refreshed_chunks: 4,
                memory_directory_id: Some(latest_memory_directory.id),
                memory_directory_version_no: Some(1),
                retrieval_evidence_count: 1,
                retrieval_evidence_ids: Vec::new(),
                output: Some(contracts::DatasetOutputContentView {
                    format: contracts::DatasetOutputFormatView::Markdown,
                    sections: vec![contracts::DatasetOutputSectionView {
                        section_key: "dataset_summary".to_string(),
                        kind: contracts::DatasetOutputSectionKindView::Summary,
                        title: "Dataset Summary".to_string(),
                        content: "Placeholder output".to_string(),
                        retrieval_evidence_ids: Vec::new(),
                    }],
                }),
                service_handoff: None,
                tool_trace: Vec::new(),
                context_binding: contracts::ManifestContextBindingView::CreationTime,
                runtime: Some(contracts::ManifestRuntimeView {
                    mode: contracts::ManifestRuntimeModeView::Placeholder,
                    provider: Some("placeholder".to_string()),
                    model: Some("placeholder-dataset-output-v1".to_string()),
                    request_id: Some("req_dataset_output_placeholder".to_string()),
                    finish_reason: Some(contracts::ManifestFinishReasonView::Stop),
                    provider_failure: None,
                    latency_ms: Some(0),
                    usage: Some(contracts::ManifestTokenUsageView {
                        input_tokens: 11,
                        output_tokens: 11,
                        total_tokens: 22,
                    }),
                    system_prompt_key: Some("dataset_output.placeholder".to_string()),
                    system_prompt_version: Some("v1".to_string()),
                    tool_trace_count: None,
                }),
            }),
            model_facing: None,
            created_at: now,
        };
        let session = ChatSession {
            id: ChatSessionId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            execution_id: WorkflowExecutionId::new(),
            title: "Demo chat".to_string(),
            latest_memory_directory_id: Some(latest_memory_directory.id),
            latest_dataset_output_id: Some(latest_output.id),
            session_manifest: json!({
                "generator": "chat-session-workflow",
                "schema_version": "0.3.0",
                "status": "pending_assistant_reply",
                "initial_prompt": "Summarize",
                "last_prompt": "Summarize",
                "last_turn_kind": "placeholder_orchestration",
                "context_binding": "creation_time",
                "latest_memory_directory_id": latest_memory_directory.id,
                "latest_memory_directory_version_no": 1,
                "latest_dataset_output_id": latest_output.id,
                "last_turn": {
                    "turn_id": "turn_pending_demo",
                    "status": "pending",
                    "stream_mode": "buffered",
                    "provider_request_id": Value::Null,
                    "assistant_message_id": Value::Null,
                    "tool_trace_count": 0,
                    "started_at": now,
                    "completed_at": Value::Null
                },
                "runtime": {
                    "mode": "placeholder",
                    "provider": "placeholder",
                    "model": "placeholder-chat-session-v1",
                    "request_id": "req_chat_placeholder_stale",
                    "finish_reason": "stop",
                    "latency_ms": 1,
                    "usage": {
                        "input_tokens": 12,
                        "output_tokens": 12,
                        "total_tokens": 24
                    },
                    "system_prompt_key": "chat_session.placeholder",
                    "system_prompt_version": "v1",
                    "tool_trace_count": 0
                }
            }),
            created_at: now,
            updated_at: now,
        };

        let view = to_chat_session_view(
            session,
            Some(latest_memory_directory),
            Some(latest_output),
            None,
        );

        assert_eq!(
            view.latest_dataset_output
                .as_ref()
                .map(|output| output.retrieval_evidences.len()),
            Some(1)
        );
        assert_eq!(
            view.latest_memory_directory
                .as_ref()
                .map(|directory| directory.version_no),
            Some(1)
        );
        assert_eq!(
            view.latest_memory_directory
                .as_ref()
                .and_then(|directory| directory.directory_tree.as_ref())
                .map(|tree| tree.version_no),
            Some(1)
        );
        assert_eq!(
            view.latest_memory_directory
                .as_ref()
                .and_then(|directory| directory.directory_tree.as_ref())
                .map(|tree| tree.root.title.as_str()),
            Some("Dataset Memory")
        );
        assert_eq!(
            view.latest_dataset_output
                .as_ref()
                .map(|output| output.output_manifest["retrieval_evidence_count"].clone()),
            Some(json!(1))
        );
        assert_eq!(
            view.latest_dataset_output
                .as_ref()
                .map(|output| output.llm_invocations.len()),
            Some(0)
        );
        assert_eq!(
            view.latest_dataset_output
                .as_ref()
                .map(|output| output.tool_executions.len()),
            Some(0)
        );
        assert_eq!(
            view.latest_dataset_output
                .as_ref()
                .and_then(|output| output.output_manifest_view.as_ref())
                .map(|manifest| manifest
                    .runtime
                    .as_ref()
                    .map(|runtime| runtime.mode.clone())),
            Some(Some(contracts::ManifestRuntimeModeView::Placeholder))
        );
        assert_eq!(view.latest_assistant_message_id, None);
        assert!(view.latest_assistant_message.is_none());
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .map(|manifest| manifest.status.clone()),
            Some(contracts::ChatSessionManifestStatusView::PendingAssistantReply)
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.turn_id.as_str()),
            Some("turn_pending_demo")
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.status.clone()),
            Some(contracts::ChatTurnStatusView::Pending)
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.provider_status.clone()),
            Some(contracts::ChatTurnProviderStatusView::Pending)
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.events.len()),
            Some(1)
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .and_then(|turn| turn.events.first())
                .map(|event| event.kind.clone()),
            Some(contracts::ChatTurnEventKindView::TurnStarted)
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.stream_mode.clone()),
            Some(contracts::ChatTurnStreamModeView::Buffered)
        );
        assert!(view
            .session_manifest_view
            .as_ref()
            .and_then(|manifest| manifest.runtime.as_ref())
            .is_none());
    }

    #[test]
    fn pending_chat_session_view_preserves_provider_responded_state_before_message_commit() {
        let now = Utc::now();
        let requested_at = now + chrono::TimeDelta::milliseconds(25);
        let responded_at = requested_at + chrono::TimeDelta::milliseconds(140);
        let session = ChatSession {
            id: ChatSessionId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            execution_id: WorkflowExecutionId::new(),
            title: "Pending response-ready chat".to_string(),
            latest_memory_directory_id: None,
            latest_dataset_output_id: None,
            session_manifest: json!({
                "generator": "chat-session-workflow",
                "schema_version": "0.3.0",
                "status": "pending_assistant_reply",
                "initial_prompt": "Summarize the dataset",
                "last_prompt": "Summarize the dataset",
                "last_turn_kind": "placeholder_orchestration",
                "context_binding": "creation_time",
                "last_turn": {
                    "turn_id": "turn_response_ready",
                    "status": "pending",
                    "stream_mode": "streaming",
                    "provider_status": "responded",
                    "provider_request_id": "req_response_ready",
                    "provider_requested_at": requested_at,
                    "provider_responded_at": responded_at,
                    "finish_reason": "tool_calls",
                    "assistant_message_id": Value::Null,
                    "tool_trace_count": 1,
                    "tool_status_summary": {
                        "requested_count": 0,
                        "completed_count": 1,
                        "failed_count": 0
                    },
                    "events": [
                        {
                            "kind": "turn_started",
                            "at": now,
                            "provider_request_id": Value::Null,
                            "finish_reason": Value::Null,
                            "tool_trace_count": Value::Null
                        },
                        {
                            "kind": "provider_requested",
                            "at": requested_at,
                            "provider_request_id": "req_response_ready",
                            "finish_reason": Value::Null,
                            "tool_trace_count": Value::Null
                        },
                        {
                            "kind": "provider_responded",
                            "at": responded_at,
                            "provider_request_id": "req_response_ready",
                            "finish_reason": "tool_calls",
                            "tool_trace_count": 1
                        },
                        {
                            "kind": "tool_calls_emitted",
                            "at": responded_at,
                            "provider_request_id": "req_response_ready",
                            "finish_reason": "tool_calls",
                            "tool_trace_count": 1
                        }
                    ],
                    "started_at": now,
                    "completed_at": Value::Null
                },
                "runtime": {
                    "mode": "provider",
                    "provider": "stale-provider",
                    "model": "stale-model",
                    "request_id": "req_stale",
                    "finish_reason": "error",
                    "latency_ms": 999,
                    "tool_trace_count": 99
                }
            }),
            created_at: now,
            updated_at: responded_at,
        };

        let mut view = to_chat_session_view(session, None, None, None);
        view.model_facing = Some(derive_chat_session_model_facing_summary(&view));

        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .map(|manifest| manifest.status.clone()),
            Some(contracts::ChatSessionManifestStatusView::PendingAssistantReply)
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.status.clone()),
            Some(contracts::ChatTurnStatusView::Pending)
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.provider_status.clone()),
            Some(contracts::ChatTurnProviderStatusView::Responded)
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .and_then(|turn| turn.finish_reason.clone()),
            Some(contracts::ManifestFinishReasonView::ToolCalls)
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.completed_at),
            Some(None)
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.provider_requested_at),
            Some(Some(requested_at))
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.provider_responded_at),
            Some(Some(responded_at))
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .and_then(|turn| turn.tool_status_summary.as_ref())
                .map(|summary| (
                    summary.requested_count,
                    summary.completed_count,
                    summary.failed_count
                )),
            Some((0, 1, 0))
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.events.len()),
            Some(4)
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .and_then(|turn| turn.events.get(2))
                .map(|event| event.kind.clone()),
            Some(contracts::ChatTurnEventKindView::ProviderResponded)
        );
        assert!(view
            .session_manifest_view
            .as_ref()
            .and_then(|manifest| manifest.runtime.as_ref())
            .is_none());
    }

    #[test]
    fn pending_chat_session_view_preserves_failed_turn_state_for_polling_clients() {
        let now = Utc::now();
        let requested_at = now + chrono::TimeDelta::milliseconds(25);
        let failed_at = requested_at + chrono::TimeDelta::milliseconds(180);
        let session = ChatSession {
            id: ChatSessionId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            execution_id: WorkflowExecutionId::new(),
            title: "Failed chat".to_string(),
            latest_memory_directory_id: None,
            latest_dataset_output_id: None,
            session_manifest: json!({
                "generator": "chat-session-workflow",
                "schema_version": "0.3.0",
                "status": "pending_assistant_reply",
                "initial_prompt": "Summarize the dataset",
                "last_prompt": "Summarize the dataset",
                "last_turn_kind": "placeholder_orchestration",
                "context_binding": "creation_time",
                "last_turn": {
                    "turn_id": "turn_failed",
                    "status": "failed",
                    "stream_mode": "buffered",
                    "provider_status": "failed",
                    "provider_request_id": Value::Null,
                    "finish_reason": "error",
                    "assistant_message_id": Value::Null,
                    "tool_trace_count": 0,
                    "events": [
                        {
                            "kind": "turn_started",
                            "at": now,
                            "provider_request_id": Value::Null,
                            "finish_reason": Value::Null,
                            "tool_trace_count": Value::Null
                        },
                        {
                            "kind": "provider_requested",
                            "at": requested_at,
                            "provider_request_id": Value::Null,
                            "finish_reason": Value::Null,
                            "tool_trace_count": Value::Null
                        },
                        {
                            "kind": "turn_failed",
                            "at": failed_at,
                            "provider_request_id": Value::Null,
                            "finish_reason": "error",
                            "tool_trace_count": 0
                        }
                    ],
                    "started_at": now,
                    "completed_at": failed_at
                },
                "runtime": {
                    "mode": "provider",
                    "provider": "stale-provider",
                    "model": "stale-model",
                    "request_id": "req_stale",
                    "finish_reason": "error",
                    "latency_ms": 999,
                    "tool_trace_count": 99
                }
            }),
            created_at: now,
            updated_at: failed_at,
        };

        let mut view = to_chat_session_view(session, None, None, None);
        view.model_facing = Some(derive_chat_session_model_facing_summary(&view));

        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .map(|manifest| manifest.status.clone()),
            Some(contracts::ChatSessionManifestStatusView::PendingAssistantReply)
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.status.clone()),
            Some(contracts::ChatTurnStatusView::Failed)
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.provider_status.clone()),
            Some(contracts::ChatTurnProviderStatusView::Failed)
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .and_then(|turn| turn.finish_reason.clone()),
            Some(contracts::ManifestFinishReasonView::Error)
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.completed_at),
            Some(Some(failed_at))
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.events.len()),
            Some(3)
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .and_then(|turn| turn.events.last())
                .map(|event| event.kind.clone()),
            Some(contracts::ChatTurnEventKindView::TurnFailed)
        );
        assert!(view
            .session_manifest_view
            .as_ref()
            .and_then(|manifest| manifest.runtime.as_ref())
            .is_none());
    }

    #[test]
    fn to_chat_session_view_exposes_pending_report_entry_confirmation_gate() {
        let now = Utc::now();
        let session = ChatSession {
            id: ChatSessionId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            execution_id: WorkflowExecutionId::new(),
            title: "Pending report entry".to_string(),
            latest_memory_directory_id: None,
            latest_dataset_output_id: None,
            session_manifest: json!({
                "generator": "chat-session-workflow",
                "schema_version": "0.3.0",
                "status": "assistant_replied",
                "initial_prompt": "Summarize the dataset",
                "last_prompt": "Turn this into a report",
                "context_binding": "creation_time",
                "report_entry": {
                    "state": "confirmation_required",
                    "requested_at": now,
                    "resolved_at": Value::Null,
                    "suggested_title": "Dataset Report",
                    "suggested_objective": "Turn the current dataset context into a report-ready output."
                }
            }),
            created_at: now,
            updated_at: now,
        };

        let mut view = to_chat_session_view(session, None, None, None);
        view.model_facing = Some(derive_chat_session_model_facing_summary(&view));

        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.report_entry.as_ref())
                .map(|entry| entry.state.clone()),
            Some(contracts::ModelFacingReportEntryStateView::ConfirmationRequired)
        );
        assert_eq!(
            view.model_facing
                .as_ref()
                .map(|summary| summary.capability_class.clone()),
            Some(contracts::ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis)
        );
        assert_eq!(
            view.model_facing
                .as_ref()
                .map(|summary| summary.service_lane.clone()),
            Some(contracts::ModelFacingServiceLaneView::MaterialService)
        );
        assert_eq!(
            view.model_facing
                .as_ref()
                .map(|summary| summary.report_entry_state.clone()),
            Some(contracts::ModelFacingReportEntryStateView::ConfirmationRequired)
        );
        assert_eq!(
            view.model_facing
                .as_ref()
                .map(|summary| summary.continuation_state.clone()),
            Some(contracts::ModelFacingContinuationStateView::NeedsUserConfirmation)
        );
        assert_eq!(
            view.model_facing
                .as_ref()
                .and_then(|summary| summary.recommended_next_action.clone()),
            Some(contracts::ModelFacingNextActionView::RequestReportEntryConfirmation)
        );
    }

    #[test]
    fn to_chat_session_view_exposes_confirmed_report_entry_as_report_planning() {
        let now = Utc::now();
        let report_plan_id = ReportPlanId::new();
        let session = ChatSession {
            id: ChatSessionId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            execution_id: WorkflowExecutionId::new(),
            title: "Confirmed report entry".to_string(),
            latest_memory_directory_id: None,
            latest_dataset_output_id: None,
            session_manifest: json!({
                "generator": "chat-session-workflow",
                "schema_version": "0.3.0",
                "status": "assistant_replied",
                "initial_prompt": "Summarize the dataset",
                "last_prompt": "Turn this into a report",
                "context_binding": "creation_time",
                "report_entry": {
                    "state": "confirmed",
                    "requested_at": now,
                    "resolved_at": now,
                    "suggested_title": "Dataset Report",
                    "suggested_objective": "Turn the current dataset context into a report-ready output.",
                    "confirmed_report_plan_id": report_plan_id.to_string()
                }
            }),
            created_at: now,
            updated_at: now,
        };

        let mut view = to_chat_session_view(session, None, None, None);
        view.model_facing = Some(derive_chat_session_model_facing_summary(&view));

        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.report_entry.as_ref())
                .map(|entry| entry.state.clone()),
            Some(contracts::ModelFacingReportEntryStateView::Confirmed)
        );
        assert_eq!(
            view.model_facing
                .as_ref()
                .map(|summary| summary.capability_class.clone()),
            Some(contracts::ModelFacingCapabilityClassView::ReportPlanning)
        );
        assert_eq!(
            view.model_facing
                .as_ref()
                .map(|summary| summary.service_lane.clone()),
            Some(contracts::ModelFacingServiceLaneView::ReportService)
        );
        assert_eq!(
            view.model_facing
                .as_ref()
                .map(|summary| summary.report_entry_state.clone()),
            Some(contracts::ModelFacingReportEntryStateView::Confirmed)
        );
        assert_eq!(
            view.model_facing
                .as_ref()
                .and_then(|summary| summary.recommended_next_action.clone()),
            Some(contracts::ModelFacingNextActionView::ContinueReportPlanning)
        );
        assert_eq!(
            view.model_facing
                .as_ref()
                .and_then(|summary| summary.recommended_tool_key.clone()),
            Some("report.plan".to_string())
        );
    }

    #[test]
    fn to_chat_session_view_preserves_declined_report_entry_history() {
        let now = Utc::now();
        let requested_at = now - chrono::TimeDelta::minutes(5);
        let session = ChatSession {
            id: ChatSessionId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            execution_id: WorkflowExecutionId::new(),
            title: "Declined report entry".to_string(),
            latest_memory_directory_id: None,
            latest_dataset_output_id: None,
            session_manifest: json!({
                "generator": "chat-session-workflow",
                "schema_version": "0.3.0",
                "status": "assistant_replied",
                "initial_prompt": "Summarize the dataset",
                "last_prompt": "Turn this into a report",
                "context_binding": "creation_time",
                "report_entry": {
                    "state": "not_applicable",
                    "requested_at": requested_at,
                    "resolved_at": now,
                    "resolved_action": "stay_material_service",
                    "suggested_title": "Dataset Report",
                    "suggested_objective": "Turn the current dataset context into a report-ready output."
                }
            }),
            created_at: now,
            updated_at: now,
        };

        let mut view = to_chat_session_view(session, None, None, None);
        view.model_facing = Some(derive_chat_session_model_facing_summary(&view));

        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.report_entry.as_ref())
                .and_then(|entry| entry.resolved_action.clone()),
            Some(contracts::ChatSessionReportEntryResolutionView::StayMaterialService)
        );
        assert_eq!(
            view.model_facing
                .as_ref()
                .map(|summary| summary.service_lane.clone()),
            Some(contracts::ModelFacingServiceLaneView::MaterialService)
        );
        assert_eq!(
            view.model_facing
                .as_ref()
                .map(|summary| summary.report_entry_state.clone()),
            Some(contracts::ModelFacingReportEntryStateView::NotApplicable)
        );
        assert!(view
            .model_facing
            .as_ref()
            .map(|summary| summary
                .signals
                .iter()
                .any(|signal| signal == "report_entry_resolved_action=stay_material_service"))
            .unwrap_or(false));
    }

    #[test]
    fn pending_chat_session_view_infers_artifact_commit_failure_after_provider_response() {
        let now = Utc::now();
        let requested_at = now + chrono::TimeDelta::milliseconds(25);
        let failed_at = requested_at + chrono::TimeDelta::milliseconds(180);
        let session = ChatSession {
            id: ChatSessionId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            execution_id: WorkflowExecutionId::new(),
            title: "Failed chat after provider response".to_string(),
            latest_memory_directory_id: None,
            latest_dataset_output_id: None,
            session_manifest: json!({
                "generator": "chat-session-workflow",
                "schema_version": "0.3.0",
                "status": "pending_assistant_reply",
                "initial_prompt": "Summarize the dataset",
                "last_prompt": "Summarize the dataset",
                "last_turn_kind": "placeholder_orchestration",
                "context_binding": "creation_time",
                "last_turn": {
                    "turn_id": "turn_commit_failed",
                    "status": "failed",
                    "stream_mode": "buffered",
                    "artifact_commit_failure_source": "assistant_message_create",
                    "provider_request_id": "req_commit_failed",
                    "provider_requested_at": requested_at,
                    "provider_responded_at": failed_at,
                    "finish_reason": "stop",
                    "assistant_message_id": Value::Null,
                    "tool_trace_count": 0,
                    "started_at": now,
                    "completed_at": failed_at
                }
            }),
            created_at: now,
            updated_at: failed_at,
        };

        let view = to_chat_session_view(session, None, None, None);

        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.provider_status.clone()),
            Some(contracts::ChatTurnProviderStatusView::Responded)
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.artifact_commit_status.clone()),
            Some(contracts::ChatTurnArtifactCommitStatusView::Failed)
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .and_then(|turn| turn.artifact_commit_failure_source.clone()),
            Some(contracts::ChatTurnArtifactCommitFailureSourceView::AssistantMessageCreate)
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.artifact_commit_ready_at),
            Some(Some(failed_at))
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.events.len()),
            Some(6)
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .and_then(|turn| turn.events.get(4))
                .map(|event| event.kind.clone()),
            Some(contracts::ChatTurnEventKindView::ArtifactCommitFailed)
        );
    }

    #[test]
    fn pending_chat_session_view_parses_response_ready_session_update_failure_source() {
        let now = Utc::now();
        let requested_at = now + chrono::TimeDelta::milliseconds(25);
        let failed_at = requested_at + chrono::TimeDelta::milliseconds(180);
        let session = ChatSession {
            id: ChatSessionId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            execution_id: WorkflowExecutionId::new(),
            title: "Failed chat before assistant create".to_string(),
            latest_memory_directory_id: None,
            latest_dataset_output_id: None,
            session_manifest: json!({
                "generator": "chat-session-workflow",
                "schema_version": "0.3.0",
                "status": "pending_assistant_reply",
                "initial_prompt": "Summarize the dataset",
                "last_prompt": "Summarize the dataset",
                "last_turn_kind": "placeholder_orchestration",
                "context_binding": "creation_time",
                "last_turn": {
                    "turn_id": "turn_response_ready_failed",
                    "status": "failed",
                    "stream_mode": "buffered",
                    "artifact_commit_failure_source": "response_ready_session_update",
                    "provider_request_id": "req_response_ready_failed",
                    "provider_requested_at": requested_at,
                    "provider_responded_at": failed_at,
                    "finish_reason": "stop",
                    "assistant_message_id": Value::Null,
                    "tool_trace_count": 1,
                    "started_at": now,
                    "completed_at": failed_at
                }
            }),
            created_at: now,
            updated_at: failed_at,
        };

        let view = to_chat_session_view(session, None, None, None);

        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .and_then(|turn| turn.artifact_commit_failure_source.clone()),
            Some(contracts::ChatTurnArtifactCommitFailureSourceView::ResponseReadySessionUpdate)
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.artifact_commit_status.clone()),
            Some(contracts::ChatTurnArtifactCommitStatusView::Failed)
        );
    }

    #[test]
    fn pending_chat_session_view_parses_workflow_event_recovery_persist_failure_source() {
        let now = Utc::now();
        let requested_at = now + chrono::TimeDelta::milliseconds(25);
        let failed_at = requested_at + chrono::TimeDelta::milliseconds(180);
        let session = ChatSession {
            id: ChatSessionId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            execution_id: WorkflowExecutionId::new(),
            title: "Failed chat before response ready session update".to_string(),
            latest_memory_directory_id: None,
            latest_dataset_output_id: None,
            session_manifest: json!({
                "generator": "chat-session-workflow",
                "schema_version": "0.3.0",
                "status": "pending_assistant_reply",
                "initial_prompt": "Summarize the dataset",
                "last_prompt": "Summarize the dataset",
                "last_turn_kind": "placeholder_orchestration",
                "context_binding": "creation_time",
                "last_turn": {
                    "turn_id": "turn_event_checkpoint_failed",
                    "status": "failed",
                    "stream_mode": "buffered",
                    "artifact_commit_failure_source": "workflow_event_recovery_persist",
                    "provider_request_id": "req_event_checkpoint_failed",
                    "provider_requested_at": requested_at,
                    "provider_responded_at": failed_at,
                    "finish_reason": "stop",
                    "assistant_message_id": Value::Null,
                    "tool_trace_count": 1,
                    "started_at": now,
                    "completed_at": failed_at
                }
            }),
            created_at: now,
            updated_at: failed_at,
        };

        let view = to_chat_session_view(session, None, None, None);

        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .and_then(|turn| turn.artifact_commit_failure_source.clone()),
            Some(contracts::ChatTurnArtifactCommitFailureSourceView::WorkflowEventRecoveryPersist)
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.artifact_commit_status.clone()),
            Some(contracts::ChatTurnArtifactCommitStatusView::Failed)
        );
    }

    #[test]
    fn chat_session_view_supports_completed_manifest_without_runtime_duplicates() {
        let now = Utc::now();
        let session_id = ChatSessionId::new();
        let assistant_message_id = ChatMessageId::new();
        let session = ChatSession {
            id: session_id,
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            execution_id: WorkflowExecutionId::new(),
            title: "Completed chat".to_string(),
            latest_memory_directory_id: None,
            latest_dataset_output_id: None,
            session_manifest: json!({
                "generator": "chat-session-workflow",
                "schema_version": "0.3.0",
                "status": "assistant_replied",
                "initial_prompt": "Summarize the dataset",
                "last_prompt": "Summarize the dataset",
                "last_turn_kind": "placeholder_orchestration",
                "context_binding": "creation_time"
            }),
            created_at: now,
            updated_at: now,
        };
        let latest_assistant_message = ChatMessageView {
            id: assistant_message_id,
            session_id,
            role: ChatMessageRole::Assistant,
            turn_index: 1,
            content: "Assistant reply".to_string(),
            llm_invocations: Vec::new(),
            tool_executions: Vec::new(),
            message_manifest: json!({
                "generator": "chat-session-worker",
                "schema_version": "0.5.0",
                "dataset_id": session.dataset_id,
                "prompt": "Summarize the dataset",
                "indexed_document_count": 1,
                "refreshed_chunks": 0,
                "prior_message_count": 1,
                "output": {
                    "format": "markdown",
                    "sections": [{
                        "section_key": "assistant_reply",
                        "kind": "reply",
                        "title": "Assistant Reply",
                        "content": "Assistant reply",
                        "retrieval_evidence_ids": []
                    }]
                },
                "tool_trace": [],
                "turn": {
                    "turn_id": "turn_completed",
                    "status": "completed",
                    "stream_mode": "buffered",
                    "provider_request_id": "req_completed",
                    "provider_requested_at": now,
                    "provider_responded_at": now,
                    "assistant_message_id": assistant_message_id,
                    "tool_trace_count": 0,
                    "started_at": now,
                    "completed_at": now
                },
                "context_binding": "creation_time",
                "runtime": {
                    "mode": "placeholder",
                    "provider": "placeholder",
                    "model": "placeholder-chat-session-v1",
                    "request_id": "req_completed",
                    "finish_reason": "stop",
                    "latency_ms": 1,
                    "usage": {
                        "input_tokens": 10,
                        "output_tokens": 5,
                        "total_tokens": 15
                    },
                    "tool_trace_count": 0
                }
            }),
            message_manifest_view: None,
            model_facing: None,
            created_at: now,
        };

        let view = to_chat_session_view(session, None, None, Some(latest_assistant_message));

        assert_eq!(view.latest_assistant_message_id, Some(assistant_message_id));
        assert_eq!(
            view.latest_assistant_message
                .as_ref()
                .map(|message| message.content.as_str()),
            Some("Assistant reply")
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .map(|manifest| manifest.status.clone()),
            Some(contracts::ChatSessionManifestStatusView::AssistantReplied)
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.turn_id.as_str()),
            Some("turn_completed")
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref()),
            None
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .and_then(|turn| turn.assistant_message_id),
            Some(assistant_message_id)
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.provider_requested_at),
            Some(Some(now))
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.provider_responded_at),
            Some(Some(now))
        );
    }

    #[test]
    fn completed_chat_session_view_prefers_latest_message_turn_over_stale_session_summary() {
        let now = Utc::now();
        let session_id = ChatSessionId::new();
        let assistant_message_id = ChatMessageId::new();
        let session = ChatSession {
            id: session_id,
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            execution_id: WorkflowExecutionId::new(),
            title: "Completed chat".to_string(),
            latest_memory_directory_id: None,
            latest_dataset_output_id: None,
            session_manifest: json!({
                "generator": "chat-session-workflow",
                "schema_version": "0.3.0",
                "status": "assistant_replied",
                "initial_prompt": "Summarize the dataset",
                "last_prompt": "Summarize the dataset",
                "last_turn_kind": "placeholder_orchestration",
                "context_binding": "creation_time",
                "last_turn": {
                    "turn_id": "turn_stale",
                    "status": "completed",
                    "stream_mode": "buffered",
                    "provider_request_id": "req_stale",
                    "assistant_message_id": Value::Null,
                    "tool_trace_count": 99,
                    "started_at": now,
                    "completed_at": now
                },
                "runtime": {
                    "mode": "placeholder",
                    "provider": "stale-provider",
                    "model": "stale-model",
                    "request_id": "req_stale",
                    "finish_reason": "error",
                    "latency_ms": 999,
                    "tool_trace_count": 99
                }
            }),
            created_at: now,
            updated_at: now,
        };
        let latest_assistant_message = ChatMessageView {
            id: assistant_message_id,
            session_id,
            role: ChatMessageRole::Assistant,
            turn_index: 1,
            content: "Assistant reply".to_string(),
            llm_invocations: Vec::new(),
            tool_executions: Vec::new(),
            message_manifest: json!({
                "generator": "chat-session-worker",
                "schema_version": "0.5.0",
                "dataset_id": session.dataset_id,
                "prompt": "Summarize the dataset",
                "indexed_document_count": 1,
                "refreshed_chunks": 0,
                "prior_message_count": 1,
                "output": {
                    "format": "markdown",
                    "sections": [{
                        "section_key": "assistant_reply",
                        "kind": "reply",
                        "title": "Assistant Reply",
                        "content": "Assistant reply",
                        "retrieval_evidence_ids": []
                    }]
                },
                "tool_trace": [],
                "turn": {
                    "turn_id": "turn_fresh",
                    "status": "completed",
                    "stream_mode": "streaming",
                    "provider_request_id": "req_fresh",
                    "provider_requested_at": now,
                    "provider_responded_at": now,
                    "assistant_message_id": assistant_message_id,
                    "tool_trace_count": 1,
                    "started_at": now,
                    "completed_at": now
                },
                "context_binding": "creation_time",
                "runtime": {
                    "mode": "provider",
                    "provider": "openai",
                    "model": "gpt-5.4",
                    "request_id": "req_fresh",
                    "finish_reason": "tool_calls",
                    "latency_ms": 123,
                    "usage": {
                        "input_tokens": 12,
                        "output_tokens": 8,
                        "total_tokens": 20
                    },
                    "tool_trace_count": 1
                }
            }),
            message_manifest_view: None,
            model_facing: None,
            created_at: now,
        };

        let view = to_chat_session_view(session, None, None, Some(latest_assistant_message));

        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.turn_id.as_str()),
            Some("turn_fresh")
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.stream_mode.clone()),
            Some(contracts::ChatTurnStreamModeView::Streaming)
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.stream_status.clone()),
            Some(contracts::ChatTurnStreamStatusView::Completed)
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.provider_status.clone()),
            Some(contracts::ChatTurnProviderStatusView::Responded)
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .and_then(|turn| turn.finish_reason.clone()),
            Some(contracts::ManifestFinishReasonView::ToolCalls)
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.provider_requested_at),
            Some(Some(now))
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.provider_responded_at),
            Some(Some(now))
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.first_token_at),
            Some(Some(now))
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.stream_completed_at),
            Some(Some(now))
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.artifact_commit_status.clone()),
            Some(contracts::ChatTurnArtifactCommitStatusView::Completed)
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.artifact_commit_ready_at),
            Some(Some(now))
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.events.len()),
            Some(9)
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref()),
            None
        );
    }

    #[test]
    fn completed_chat_session_view_hydrates_latest_message_from_llm_invocations_when_manifest_view_absent(
    ) {
        let now = Utc::now();
        let session_id = ChatSessionId::new();
        let assistant_message_id = ChatMessageId::new();
        let execution_id = WorkflowExecutionId::new();
        let session = ChatSession {
            id: session_id,
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            execution_id,
            title: "Completed chat".to_string(),
            latest_memory_directory_id: None,
            latest_dataset_output_id: None,
            session_manifest: json!({
                "generator": "chat-session-workflow",
                "schema_version": "0.3.0",
                "status": "assistant_replied",
                "initial_prompt": "Summarize the dataset",
                "last_prompt": "Summarize the dataset",
                "last_turn_kind": "placeholder_orchestration",
                "context_binding": "creation_time",
                "last_turn": {
                    "turn_id": "turn_stale",
                    "status": "completed",
                    "stream_mode": "buffered",
                    "provider_request_id": "req_stale_session",
                    "assistant_message_id": null,
                    "tool_trace_count": 99,
                    "started_at": now,
                    "completed_at": now
                },
                "runtime": {
                    "mode": "placeholder",
                    "provider": "stale-session-provider",
                    "model": "stale-session-model",
                    "request_id": "req_stale_session",
                    "finish_reason": "error",
                    "latency_ms": 999,
                    "tool_trace_count": 99
                }
            }),
            created_at: now,
            updated_at: now,
        };
        let latest_assistant_message = ChatMessageView {
            id: assistant_message_id,
            session_id,
            role: ChatMessageRole::Assistant,
            turn_index: 1,
            content: "Assistant reply".to_string(),
            llm_invocations: vec![contracts::LlmInvocationView {
                id: domain_model::LlmInvocationId::new(),
                execution_id,
                source_kind: contracts::LlmInvocationSourceKindView::ChatMessage,
                dataset_output_id: None,
                chat_message_id: Some(assistant_message_id),
                sequence_no: 1,
                mode: contracts::LlmInvocationModeView::Provider,
                provider: Some("openai".to_string()),
                model: Some("gpt-5.4".to_string()),
                request_id: Some("req_fresh_invocation".to_string()),
                finish_reason: Some(contracts::LlmInvocationFinishReasonView::ToolCalls),
                latency_ms: Some(123),
                usage: Some(contracts::LlmTokenUsageView {
                    input_tokens: 12,
                    output_tokens: 8,
                    total_tokens: 20,
                }),
                system_prompt_key: Some("chat_session.live".to_string()),
                system_prompt_version: Some("v2".to_string()),
                tool_trace_count: Some(2),
                created_at: now,
            }],
            tool_executions: Vec::new(),
            message_manifest: json!({
                "generator": "chat-session-worker",
                "schema_version": "0.5.0",
                "dataset_id": session.dataset_id,
                "prompt": "Summarize the dataset",
                "indexed_document_count": 1,
                "refreshed_chunks": 0,
                "prior_message_count": 1,
                "output": {
                    "format": "markdown",
                    "sections": [{
                        "section_key": "assistant_reply",
                        "kind": "reply",
                        "title": "Assistant Reply",
                        "content": "Assistant reply",
                        "retrieval_evidence_ids": []
                    }]
                },
                "tool_trace": [],
                "turn": {
                    "turn_id": "turn_fresh",
                    "status": "completed",
                    "stream_mode": "streaming",
                    "provider_request_id": "req_stale_message",
                    "provider_requested_at": now,
                    "provider_responded_at": now,
                    "assistant_message_id": assistant_message_id,
                    "tool_trace_count": 0,
                    "started_at": now,
                    "completed_at": now
                },
                "context_binding": "creation_time",
                "runtime": {
                    "mode": "placeholder",
                    "provider": "stale-message-provider",
                    "model": "stale-message-model",
                    "request_id": "req_stale_message",
                    "finish_reason": "error",
                    "latency_ms": 999,
                    "tool_trace_count": 0
                }
            }),
            message_manifest_view: None,
            model_facing: None,
            created_at: now,
        };

        let view = to_chat_session_view(session, None, None, Some(latest_assistant_message));

        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.turn_id.as_str()),
            Some("turn_fresh")
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .and_then(|turn| turn.provider_request_id.as_deref()),
            Some("req_fresh_invocation")
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.tool_trace_count),
            Some(2)
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.provider_requested_at),
            Some(Some(now))
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.last_turn.as_ref())
                .map(|turn| turn.provider_responded_at),
            Some(Some(now))
        );
        assert_eq!(
            view.session_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref()),
            None
        );
    }

    #[test]
    fn latest_assistant_message_from_views_prefers_last_assistant() {
        let session_id = ChatSessionId::new();
        let first_assistant_id = ChatMessageId::new();
        let last_assistant_id = ChatMessageId::new();
        let messages = vec![
            ChatMessageView {
                id: ChatMessageId::new(),
                session_id,
                role: ChatMessageRole::User,
                turn_index: 0,
                content: "question".to_string(),
                llm_invocations: Vec::new(),
                tool_executions: Vec::new(),
                message_manifest: json!({}),
                message_manifest_view: None,
                model_facing: None,
                created_at: Utc::now(),
            },
            ChatMessageView {
                id: first_assistant_id,
                session_id,
                role: ChatMessageRole::Assistant,
                turn_index: 1,
                content: "first".to_string(),
                llm_invocations: Vec::new(),
                tool_executions: Vec::new(),
                message_manifest: json!({}),
                message_manifest_view: None,
                model_facing: None,
                created_at: Utc::now(),
            },
            ChatMessageView {
                id: last_assistant_id,
                session_id,
                role: ChatMessageRole::Assistant,
                turn_index: 2,
                content: "last".to_string(),
                llm_invocations: Vec::new(),
                tool_executions: Vec::new(),
                message_manifest: json!({}),
                message_manifest_view: None,
                model_facing: None,
                created_at: Utc::now(),
            },
        ];

        let latest = latest_assistant_message_from_views(&messages);

        assert_eq!(
            latest.as_ref().map(|message| message.id),
            Some(last_assistant_id)
        );
        assert_eq!(
            latest.as_ref().map(|message| message.content.as_str()),
            Some("last")
        );
    }

    #[test]
    fn to_chat_message_view_parses_placeholder_runtime_manifest() {
        let now = Utc::now();
        let dataset_id = DatasetId::new();
        let memory_directory_id = MemoryDirectoryId::new();
        let output_id = DatasetOutputId::new();
        let evidence_id = RetrievalEvidenceId::new();
        let message = ChatMessage {
            id: ChatMessageId::new(),
            tenant_id: TenantId::new(),
            session_id: ChatSessionId::new(),
            role: ChatMessageRole::Assistant,
            turn_index: 1,
            content: "Placeholder answer".to_string(),
            message_manifest: json!({
                "generator": "chat-session-worker",
                "schema_version": "0.4.0",
                "dataset_id": dataset_id,
                "prompt": "Summarize",
                "indexed_document_count": 3,
                "refreshed_chunks": 5,
                "prior_message_count": 1,
                "latest_memory_directory_id": memory_directory_id,
                "latest_memory_directory_version_no": 2,
                "latest_dataset_output_id": output_id,
                "output": {
                    "format": "markdown",
                    "sections": [
                        {
                            "section_key": "assistant_reply",
                            "kind": "reply",
                            "title": "Assistant Reply",
                            "content": "Placeholder answer",
                            "retrieval_evidence_ids": [evidence_id]
                        }
                    ]
                },
                "turn": {
                    "turn_id": "turn_completed_placeholder",
                    "status": "completed",
                    "stream_mode": "buffered",
                    "provider_request_id": "req_chat_message_placeholder",
                    "assistant_message_id": Value::Null,
                    "tool_trace_count": 0,
                    "started_at": now,
                    "completed_at": now
                },
                "context_binding": "creation_time",
                "runtime": {
                    "mode": "placeholder",
                    "provider": "placeholder",
                    "model": "placeholder-chat-session-v1",
                    "request_id": "req_chat_message_placeholder",
                    "finish_reason": "stop",
                    "latency_ms": 2,
                    "usage": {
                        "input_tokens": 10,
                        "output_tokens": 10,
                        "total_tokens": 20
                    },
                    "system_prompt_key": "chat_session.placeholder",
                    "system_prompt_version": "v1",
                    "tool_trace_count": 0
                }
            }),
            created_at: now,
        };

        let view = to_chat_message_view(message, Vec::new(), Vec::new());
        assert!(view.llm_invocations.is_empty());
        assert!(view.tool_executions.is_empty());

        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .map(|manifest| manifest.context_binding.clone()),
            Some(contracts::ManifestContextBindingView::CreationTime)
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .map(|runtime| runtime.mode.clone()),
            Some(contracts::ManifestRuntimeModeView::Placeholder)
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .and_then(|runtime| runtime.request_id.as_deref()),
            Some("req_chat_message_placeholder")
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .and_then(|runtime| runtime.finish_reason.clone()),
            Some(contracts::ManifestFinishReasonView::Stop)
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .and_then(|runtime| runtime.latency_ms),
            Some(2)
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .and_then(|runtime| runtime.usage.as_ref())
                .map(|usage| usage.total_tokens),
            Some(20)
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .map(|runtime| runtime.tool_trace_count),
            Some(Some(0))
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .and_then(|runtime| runtime.provider.as_deref()),
            Some("placeholder")
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .and_then(|runtime| runtime.model.as_deref()),
            Some("placeholder-chat-session-v1")
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .and_then(|runtime| runtime.system_prompt_key.as_deref()),
            Some("chat_session.placeholder")
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .and_then(|runtime| runtime.system_prompt_version.as_deref()),
            Some("v1")
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .map(|manifest| manifest.tool_trace.len()),
            Some(0)
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.turn.as_ref())
                .map(|turn| turn.turn_id.as_str()),
            Some("turn_completed_placeholder")
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.turn.as_ref())
                .map(|turn| turn.status.clone()),
            Some(contracts::ChatTurnStatusView::Completed)
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.turn.as_ref())
                .map(|turn| turn.stream_mode.clone()),
            Some(contracts::ChatTurnStreamModeView::Buffered)
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .map(|manifest| manifest.latest_dataset_output_id),
            Some(Some(output_id))
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.output.as_ref())
                .map(|output| output.format.clone()),
            Some(contracts::ChatMessageOutputFormatView::Markdown)
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.output.as_ref())
                .and_then(|output| output.sections.first())
                .map(|section| section.kind.clone()),
            Some(contracts::ChatMessageSectionKindView::Reply)
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.output.as_ref())
                .and_then(|output| output.sections.first())
                .map(|section| section.content.as_str()),
            Some("Placeholder answer")
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.output.as_ref())
                .and_then(|output| output.sections.first())
                .map(|section| section.retrieval_evidence_ids.clone()),
            Some(vec![evidence_id])
        );
        assert_eq!(
            view.model_facing
                .as_ref()
                .map(|summary| summary.capability_class.clone()),
            Some(contracts::ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis)
        );
        assert_eq!(
            view.model_facing
                .as_ref()
                .map(|summary| summary.evidence_state.clone()),
            Some(contracts::ModelFacingEvidenceStateView::Mixed)
        );
        assert_eq!(
            view.model_facing
                .as_ref()
                .map(|summary| summary.continuation_state.clone()),
            Some(contracts::ModelFacingContinuationStateView::ReadyToAnswer)
        );
        assert_eq!(
            view.model_facing
                .as_ref()
                .and_then(|summary| summary.recommended_next_action.clone()),
            Some(contracts::ModelFacingNextActionView::AnswerDirectly)
        );
        assert!(view
            .model_facing
            .as_ref()
            .expect("assistant message should expose model_facing summary")
            .allowed_tool_keys
            .contains(&"document.compare".to_string()));
        assert!(view
            .model_facing
            .as_ref()
            .expect("assistant message should expose model_facing summary")
            .signals
            .iter()
            .any(|signal| signal == "document_focus=multi_document"));
    }

    #[test]
    fn to_chat_message_view_exposes_pending_report_entry_handoff() {
        let now = Utc::now();
        let dataset_id = DatasetId::new();
        let message = ChatMessage {
            id: ChatMessageId::new(),
            tenant_id: TenantId::new(),
            session_id: ChatSessionId::new(),
            role: ChatMessageRole::Assistant,
            turn_index: 1,
            content: "I can turn this into a report when you confirm.".to_string(),
            message_manifest: json!({
                "generator": "chat-session-worker",
                "schema_version": "0.5.0",
                "dataset_id": dataset_id,
                "prompt": "Turn this into a report",
                "indexed_document_count": 2,
                "refreshed_chunks": 3,
                "prior_message_count": 1,
                "latest_memory_directory_id": null,
                "latest_memory_directory_version_no": null,
                "latest_dataset_output_id": null,
                "output": {
                    "format": "markdown",
                    "sections": [{
                        "section_key": "assistant_reply",
                        "kind": "reply",
                        "title": "Assistant Reply",
                        "content": "I can turn this into a report when you confirm.",
                        "retrieval_evidence_ids": []
                    }]
                },
                "service_handoff": {
                    "source": "chat_session_report_entry",
                    "service_lane": "material_service",
                    "report_entry_state": "confirmation_required",
                    "requested_at": now,
                    "resolved_at": null,
                    "resolved_action": null,
                    "suggested_title": "Dataset Report",
                    "suggested_objective": "Turn the current dataset context into a report-ready output.",
                    "confirmed_report_plan_id": null
                },
                "turn": {
                    "turn_id": "turn_pending_handoff",
                    "status": "completed",
                    "stream_mode": "buffered",
                    "provider_request_id": "req_pending_handoff",
                    "assistant_message_id": Value::Null,
                    "tool_trace_count": 0,
                    "started_at": now,
                    "completed_at": now
                },
                "context_binding": "creation_time"
            }),
            created_at: now,
        };

        let view = to_chat_message_view(message, Vec::new(), Vec::new());

        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.service_handoff.as_ref())
                .map(|handoff| handoff.report_entry_state.clone()),
            Some(contracts::ModelFacingReportEntryStateView::ConfirmationRequired)
        );
        assert_eq!(
            view.model_facing
                .as_ref()
                .map(|summary| summary.capability_class.clone()),
            Some(contracts::ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis)
        );
        assert_eq!(
            view.model_facing
                .as_ref()
                .map(|summary| summary.service_lane.clone()),
            Some(contracts::ModelFacingServiceLaneView::MaterialService)
        );
        assert_eq!(
            view.model_facing
                .as_ref()
                .map(|summary| summary.report_entry_state.clone()),
            Some(contracts::ModelFacingReportEntryStateView::ConfirmationRequired)
        );
        assert_eq!(
            view.model_facing
                .as_ref()
                .map(|summary| summary.continuation_state.clone()),
            Some(contracts::ModelFacingContinuationStateView::NeedsUserConfirmation)
        );
        assert_eq!(
            view.model_facing
                .as_ref()
                .and_then(|summary| summary.recommended_next_action.clone()),
            Some(contracts::ModelFacingNextActionView::RequestReportEntryConfirmation)
        );
        assert!(view
            .model_facing
            .as_ref()
            .map(|summary| summary
                .signals
                .iter()
                .any(|signal| signal == "service_handoff_source=chat_session_report_entry"))
            .unwrap_or(false));
    }

    #[test]
    fn to_chat_message_view_exposes_confirmed_report_service_handoff() {
        let now = Utc::now();
        let dataset_id = DatasetId::new();
        let report_plan_id = ReportPlanId::new();
        let message = ChatMessage {
            id: ChatMessageId::new(),
            tenant_id: TenantId::new(),
            session_id: ChatSessionId::new(),
            role: ChatMessageRole::Assistant,
            turn_index: 1,
            content: "Report planning is ready to continue.".to_string(),
            message_manifest: json!({
                "generator": "chat-session-worker",
                "schema_version": "0.5.0",
                "dataset_id": dataset_id,
                "prompt": "Continue the report plan",
                "indexed_document_count": 2,
                "refreshed_chunks": 3,
                "prior_message_count": 1,
                "latest_memory_directory_id": null,
                "latest_memory_directory_version_no": null,
                "latest_dataset_output_id": null,
                "output": {
                    "format": "markdown",
                    "sections": [{
                        "section_key": "assistant_reply",
                        "kind": "reply",
                        "title": "Assistant Reply",
                        "content": "Report planning is ready to continue.",
                        "retrieval_evidence_ids": []
                    }]
                },
                "service_handoff": {
                    "source": "chat_session_report_entry",
                    "service_lane": "report_service",
                    "report_entry_state": "confirmed",
                    "requested_at": now,
                    "resolved_at": now,
                    "resolved_action": "enter_report_service",
                    "suggested_title": "Dataset Report",
                    "suggested_objective": "Turn the current dataset context into a report-ready output.",
                    "confirmed_report_plan_id": report_plan_id
                },
                "turn": {
                    "turn_id": "turn_confirmed_handoff",
                    "status": "completed",
                    "stream_mode": "buffered",
                    "provider_request_id": "req_confirmed_handoff",
                    "assistant_message_id": Value::Null,
                    "tool_trace_count": 0,
                    "started_at": now,
                    "completed_at": now
                },
                "context_binding": "creation_time"
            }),
            created_at: now,
        };

        let view = to_chat_message_view(message, Vec::new(), Vec::new());

        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.service_handoff.as_ref())
                .map(|handoff| handoff.service_lane.clone()),
            Some(contracts::ModelFacingServiceLaneView::ReportService)
        );
        assert_eq!(
            view.model_facing
                .as_ref()
                .map(|summary| summary.capability_class.clone()),
            Some(contracts::ModelFacingCapabilityClassView::ReportPlanning)
        );
        assert_eq!(
            view.model_facing
                .as_ref()
                .map(|summary| summary.service_lane.clone()),
            Some(contracts::ModelFacingServiceLaneView::ReportService)
        );
        assert_eq!(
            view.model_facing
                .as_ref()
                .map(|summary| summary.report_entry_state.clone()),
            Some(contracts::ModelFacingReportEntryStateView::Confirmed)
        );
        assert_eq!(
            view.model_facing
                .as_ref()
                .map(|summary| summary.continuation_state.clone()),
            Some(contracts::ModelFacingContinuationStateView::NeedsPlatformContinuation)
        );
        assert_eq!(
            view.model_facing
                .as_ref()
                .and_then(|summary| summary.recommended_next_action.clone()),
            Some(contracts::ModelFacingNextActionView::ContinueReportPlanning)
        );
        assert_eq!(
            view.model_facing
                .as_ref()
                .and_then(|summary| summary.recommended_tool_key.clone()),
            Some("report.plan".to_string())
        );
        assert!(view
            .model_facing
            .as_ref()
            .map(|summary| summary
                .signals
                .iter()
                .any(|signal| *signal == format!("confirmed_report_plan_id={report_plan_id}")))
            .unwrap_or(false));
    }

    #[test]
    fn to_chat_message_view_prefers_llm_invocation_runtime_over_manifest_runtime() {
        let now = Utc::now();
        let dataset_id = DatasetId::new();
        let message_id = ChatMessageId::new();
        let execution_id = WorkflowExecutionId::new();
        let message = ChatMessage {
            id: message_id,
            tenant_id: TenantId::new(),
            session_id: ChatSessionId::new(),
            role: ChatMessageRole::Assistant,
            turn_index: 1,
            content: "Fresh answer".to_string(),
            message_manifest: json!({
                "generator": "chat-session-worker",
                "schema_version": "0.4.0",
                "dataset_id": dataset_id,
                "prompt": "Summarize",
                "indexed_document_count": 1,
                "refreshed_chunks": 2,
                "prior_message_count": 0,
                "latest_memory_directory_id": null,
                "latest_memory_directory_version_no": null,
                "latest_dataset_output_id": null,
                "output": {
                    "format": "markdown",
                    "sections": [{
                        "section_key": "assistant_reply",
                        "kind": "reply",
                        "title": "Assistant Reply",
                        "content": "Fresh answer",
                        "retrieval_evidence_ids": []
                    }]
                },
                "turn": {
                    "turn_id": "turn_runtime_fresh",
                    "status": "completed",
                    "stream_mode": "buffered",
                    "provider_request_id": "req_chat_message_stale",
                    "assistant_message_id": message_id,
                    "tool_trace_count": 0,
                    "started_at": now,
                    "completed_at": now
                },
                "context_binding": "creation_time",
                "runtime": {
                    "mode": "placeholder",
                    "provider": "placeholder",
                    "model": "placeholder-chat-session-v1",
                    "request_id": "req_chat_message_stale",
                    "finish_reason": "stop",
                    "latency_ms": 2,
                    "usage": {
                        "input_tokens": 10,
                        "output_tokens": 10,
                        "total_tokens": 20
                    },
                    "system_prompt_key": "chat_session.placeholder",
                    "system_prompt_version": "v1",
                    "tool_trace_count": 0
                }
            }),
            created_at: now,
        };
        let llm_invocations = vec![contracts::LlmInvocationView {
            id: domain_model::LlmInvocationId::new(),
            execution_id,
            source_kind: contracts::LlmInvocationSourceKindView::ChatMessage,
            dataset_output_id: None,
            chat_message_id: Some(message_id),
            sequence_no: 1,
            mode: contracts::LlmInvocationModeView::Provider,
            provider: Some("anthropic".to_string()),
            model: Some("claude-sonnet-4.5".to_string()),
            request_id: Some("req_chat_message_fresh".to_string()),
            finish_reason: Some(contracts::LlmInvocationFinishReasonView::Length),
            latency_ms: Some(654),
            usage: Some(contracts::LlmTokenUsageView {
                input_tokens: 377,
                output_tokens: 88,
                total_tokens: 465,
            }),
            system_prompt_key: Some("chat_session.live".to_string()),
            system_prompt_version: Some("v3".to_string()),
            tool_trace_count: Some(2),
            created_at: now,
        }];

        let view = to_chat_message_view(message, llm_invocations, Vec::new());

        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .map(|runtime| runtime.mode.clone()),
            Some(contracts::ManifestRuntimeModeView::Provider)
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .and_then(|runtime| runtime.provider.as_deref()),
            Some("anthropic")
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .and_then(|runtime| runtime.request_id.as_deref()),
            Some("req_chat_message_fresh")
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .and_then(|runtime| runtime.finish_reason.clone()),
            Some(contracts::ManifestFinishReasonView::Length)
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .and_then(|runtime| runtime.latency_ms),
            Some(654)
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .and_then(|runtime| runtime.usage.as_ref())
                .map(|usage| usage.total_tokens),
            Some(465)
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .and_then(|runtime| runtime.system_prompt_key.as_deref()),
            Some("chat_session.live")
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .map(|runtime| runtime.tool_trace_count),
            Some(Some(2))
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.turn.as_ref())
                .and_then(|turn| turn.provider_request_id.as_deref()),
            Some("req_chat_message_fresh")
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.turn.as_ref())
                .map(|turn| turn.tool_trace_count),
            Some(2)
        );
    }

    #[test]
    fn to_chat_message_view_prefers_tool_executions_over_manifest_tool_trace() {
        let now = Utc::now();
        let message_id = ChatMessageId::new();
        let message = ChatMessage {
            id: message_id,
            tenant_id: TenantId::new(),
            session_id: ChatSessionId::new(),
            role: ChatMessageRole::Assistant,
            turn_index: 1,
            content: "Fresh answer".to_string(),
            message_manifest: json!({
                "generator": "chat-session-worker",
                "schema_version": "0.4.0",
                "dataset_id": DatasetId::new(),
                "prompt": "Summarize",
                "indexed_document_count": 1,
                "refreshed_chunks": 2,
                "prior_message_count": 0,
                "latest_memory_directory_id": null,
                "latest_memory_directory_version_no": null,
                "latest_dataset_output_id": null,
                "output": {
                    "format": "markdown",
                    "sections": [{
                        "section_key": "assistant_reply",
                        "kind": "reply",
                        "title": "Assistant Reply",
                        "content": "Fresh answer",
                        "retrieval_evidence_ids": []
                    }]
                },
                "tool_trace": [{
                    "call_id": "call_stale",
                    "tool_name": "stale.tool",
                    "status": "failed",
                    "arguments": { "city": "stale" },
                    "result": { "error": "stale" }
                }],
                "turn": {
                    "turn_id": "turn_tool_trace",
                    "status": "completed",
                    "stream_mode": "buffered",
                    "provider_request_id": null,
                    "assistant_message_id": message_id,
                    "tool_trace_count": 99,
                    "started_at": now,
                    "completed_at": now
                },
                "context_binding": "creation_time",
                "runtime": {
                    "mode": "placeholder",
                    "provider": "placeholder",
                    "model": "placeholder-chat-session-v1",
                    "request_id": "req_chat_message_stale",
                    "finish_reason": "error",
                    "latency_ms": 2,
                    "tool_trace_count": 99
                }
            }),
            created_at: now,
        };
        let tool_executions = vec![
            contracts::ToolExecutionView {
                id: ToolExecutionId::new(),
                execution_id: WorkflowExecutionId::new(),
                source_kind: contracts::ToolExecutionSourceKindView::ChatMessage,
                dataset_output_id: None,
                chat_message_id: Some(message_id),
                sequence_no: 2,
                call_id: Some("call_two".to_string()),
                tool_name: "weather.lookup".to_string(),
                tool: Some(contracts::ToolReferenceView {
                    key: "weather.lookup".to_string(),
                    title: "Weather Lookup".to_string(),
                    scope_policy: "session".to_string(),
                    invocation_mode: contracts::ToolInvocationModeView::Cli,
                    cli: Some(contracts::ToolCliContractView {
                        argv: vec!["cargo".to_string(), "run".to_string()],
                        env_allowlist: vec!["WEATHER_API_KEY".to_string()],
                        output_mode: contracts::ToolCliOutputModeView::Json,
                        timeout_ms: Some(15_000),
                    }),
                }),
                status: contracts::ManifestToolCallStatusView::Completed,
                arguments: Some(json!({ "city": "Shanghai" })),
                result: Some(json!({ "summary": "sunny" })),
                created_at: now,
            },
            contracts::ToolExecutionView {
                id: ToolExecutionId::new(),
                execution_id: WorkflowExecutionId::new(),
                source_kind: contracts::ToolExecutionSourceKindView::ChatMessage,
                dataset_output_id: None,
                chat_message_id: Some(message_id),
                sequence_no: 1,
                call_id: Some("call_one".to_string()),
                tool_name: "weather.prepare".to_string(),
                tool: Some(contracts::ToolReferenceView {
                    key: "weather.prepare".to_string(),
                    title: "Weather Prepare".to_string(),
                    scope_policy: "session".to_string(),
                    invocation_mode: contracts::ToolInvocationModeView::Cli,
                    cli: None,
                }),
                status: contracts::ManifestToolCallStatusView::Requested,
                arguments: Some(json!({ "city": "Shanghai" })),
                result: None,
                created_at: now,
            },
        ];

        let view = to_chat_message_view(message, Vec::new(), tool_executions);

        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .map(|manifest| manifest.tool_trace.len()),
            Some(2)
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.tool_trace.first())
                .and_then(|call| call.call_id.as_deref()),
            Some("call_one")
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.tool_trace.first())
                .map(|call| call.tool_name.as_str()),
            Some("weather.prepare")
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.tool_trace.first())
                .map(|call| call.status.clone()),
            Some(contracts::ManifestToolCallStatusView::Requested)
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.tool_trace.get(1))
                .and_then(|call| call.call_id.as_deref()),
            Some("call_two")
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.tool_trace.get(1))
                .and_then(|call| call.tool.as_ref())
                .map(|tool| tool.key.as_str()),
            Some("weather.lookup")
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .map(|runtime| runtime.tool_trace_count),
            Some(Some(2))
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.turn.as_ref())
                .and_then(|turn| turn.tool_status_summary.as_ref())
                .map(|summary| (
                    summary.requested_count,
                    summary.completed_count,
                    summary.failed_count
                )),
            Some((1, 1, 0))
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.turn.as_ref())
                .map(|turn| turn.tool_trace_count),
            Some(2)
        );
    }

    #[test]
    fn to_chat_message_view_parses_structured_tool_trace() {
        let now = Utc::now();
        let evidence_ids = vec![RetrievalEvidenceId::new(), RetrievalEvidenceId::new()];
        let message = ChatMessage {
            id: ChatMessageId::new(),
            tenant_id: TenantId::new(),
            session_id: ChatSessionId::new(),
            role: ChatMessageRole::Assistant,
            turn_index: 2,
            content: "Used a tool".to_string(),
            message_manifest: json!({
                "generator": "chat-session-worker",
                "schema_version": "0.4.0",
                "dataset_id": DatasetId::new(),
                "prompt": "Look up weather",
                "indexed_document_count": 0,
                "refreshed_chunks": 0,
                "prior_message_count": 2,
                "output": {
                    "format": "markdown",
                    "sections": [
                        {
                            "section_key": "assistant_reply",
                            "kind": "reply",
                            "title": "Assistant Reply",
                            "content": "Used a tool",
                            "retrieval_evidence_ids": evidence_ids
                        }
                    ]
                },
                "tool_trace": [
                    {
                        "call_id": "call_123",
                        "tool_name": "weather.lookup",
                        "tool": {
                            "key": "weather.lookup",
                            "title": "Weather Lookup",
                            "scope_policy": "session",
                            "invocation_mode": "cli",
                            "cli": {
                                "argv": ["cargo", "run", "-p", "weather-tool"],
                                "env_allowlist": ["WEATHER_API_KEY"],
                                "output_mode": "json",
                                "timeout_ms": 15000
                            }
                        },
                        "status": "completed",
                        "arguments": { "city": "Shanghai" },
                        "result": { "summary": "sunny" }
                    }
                ],
                "turn": {
                    "turn_id": "turn_openai_weather",
                    "status": "completed",
                    "stream_mode": "buffered",
                    "provider_request_id": "req_openai_weather",
                    "assistant_message_id": "1f0cd0c4-4d24-4445-9da2-a72b4cfcc5bb",
                    "tool_trace_count": 1,
                    "started_at": now,
                    "completed_at": now
                },
                "context_binding": "creation_time",
                "runtime": {
                    "mode": "provider",
                    "provider": "openai",
                    "model": "gpt-5.4",
                    "request_id": "req_openai_weather",
                    "finish_reason": "tool_calls",
                    "latency_ms": 321,
                    "usage": {
                        "input_tokens": 18,
                        "output_tokens": 7,
                        "total_tokens": 25
                    },
                    "system_prompt_key": "chat_session.weather",
                    "system_prompt_version": "2026-04-19",
                    "tool_trace_count": 1
                }
            }),
            created_at: now,
        };

        let view = to_chat_message_view(message, Vec::new(), Vec::new());
        assert!(view.llm_invocations.is_empty());
        assert!(view.tool_executions.is_empty());

        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .map(|manifest| manifest.tool_trace.len()),
            Some(1)
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.tool_trace.first())
                .map(|call| call.tool_name.as_str()),
            Some("weather.lookup")
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.tool_trace.first())
                .and_then(|call| call.tool.as_ref())
                .map(|tool| tool.key.as_str()),
            Some("weather.lookup")
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.tool_trace.first())
                .and_then(|call| call.tool.as_ref())
                .map(|tool| tool.title.as_str()),
            Some("Weather Lookup")
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.tool_trace.first())
                .and_then(|call| call.tool.as_ref())
                .and_then(|tool| tool.cli.as_ref())
                .map(|cli| cli.output_mode.clone()),
            Some(contracts::ToolCliOutputModeView::Json)
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.tool_trace.first())
                .map(|call| call.status.clone()),
            Some(contracts::ManifestToolCallStatusView::Completed)
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .and_then(|runtime| runtime.provider.as_deref()),
            Some("openai")
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .and_then(|runtime| runtime.request_id.as_deref()),
            Some("req_openai_weather")
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .and_then(|runtime| runtime.finish_reason.clone()),
            Some(contracts::ManifestFinishReasonView::ToolCalls)
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .and_then(|runtime| runtime.latency_ms),
            Some(321)
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .and_then(|runtime| runtime.usage.as_ref())
                .map(|usage| usage.total_tokens),
            Some(25)
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .and_then(|runtime| runtime.system_prompt_key.as_deref()),
            Some("chat_session.weather")
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.runtime.as_ref())
                .and_then(|runtime| runtime.system_prompt_version.as_deref()),
            Some("2026-04-19")
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.turn.as_ref())
                .map(|turn| turn.turn_id.as_str()),
            Some("turn_openai_weather")
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.turn.as_ref())
                .and_then(|turn| turn.assistant_message_id),
            Some(ChatMessageId::from(
                Uuid::parse_str("1f0cd0c4-4d24-4445-9da2-a72b4cfcc5bb").unwrap()
            ))
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.output.as_ref())
                .and_then(|output| output.sections.first())
                .map(|section| section.kind.clone()),
            Some(contracts::ChatMessageSectionKindView::Reply)
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.output.as_ref())
                .and_then(|output| output.sections.first())
                .map(|section| section.content.as_str()),
            Some("Used a tool")
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.output.as_ref())
                .and_then(|output| output.sections.first())
                .map(|section| section.retrieval_evidence_ids.clone()),
            Some(evidence_ids)
        );
    }

    #[test]
    fn to_chat_message_view_preserves_assistant_message_persisted_event() {
        let now = Utc::now();
        let message_id = ChatMessageId::new();
        let message = ChatMessage {
            id: message_id,
            tenant_id: TenantId::new(),
            session_id: ChatSessionId::new(),
            role: ChatMessageRole::Assistant,
            turn_index: 1,
            content: "Persisted answer".to_string(),
            message_manifest: json!({
                "generator": "chat-session-worker",
                "schema_version": "0.5.0",
                "dataset_id": DatasetId::new(),
                "prompt": "Summarize",
                "indexed_document_count": 1,
                "refreshed_chunks": 0,
                "prior_message_count": 0,
                "output": {
                    "format": "markdown",
                    "sections": [{
                        "section_key": "assistant_reply",
                        "kind": "reply",
                        "title": "Assistant Reply",
                        "content": "Persisted answer",
                        "retrieval_evidence_ids": []
                    }]
                },
                "turn": {
                    "turn_id": "turn_persisted",
                    "status": "completed",
                    "stream_mode": "buffered",
                    "provider_status": "responded",
                    "provider_request_id": "req_persisted",
                    "provider_requested_at": now,
                    "provider_responded_at": now,
                    "finish_reason": "stop",
                    "assistant_message_id": message_id,
                    "assistant_message_persisted_at": now,
                    "tool_trace_count": 0,
                    "events": [
                        {
                            "kind": "turn_started",
                            "at": now,
                            "provider_request_id": Value::Null,
                            "finish_reason": Value::Null,
                            "tool_trace_count": Value::Null
                        },
                        {
                            "kind": "provider_requested",
                            "at": now,
                            "provider_request_id": "req_persisted",
                            "finish_reason": Value::Null,
                            "tool_trace_count": Value::Null
                        },
                        {
                            "kind": "provider_responded",
                            "at": now,
                            "provider_request_id": "req_persisted",
                            "finish_reason": "stop",
                            "tool_trace_count": 0
                        },
                        {
                            "kind": "assistant_message_persisted",
                            "at": now,
                            "provider_request_id": "req_persisted",
                            "finish_reason": "stop",
                            "tool_trace_count": 0
                        },
                        {
                            "kind": "turn_completed",
                            "at": now,
                            "provider_request_id": "req_persisted",
                            "finish_reason": "stop",
                            "tool_trace_count": 0
                        }
                    ],
                    "started_at": now,
                    "completed_at": now
                },
                "context_binding": "creation_time",
                "runtime": {
                    "mode": "provider",
                    "provider": "openai",
                    "model": "gpt-5.4",
                    "request_id": "req_persisted",
                    "finish_reason": "stop",
                    "latency_ms": 20,
                    "tool_trace_count": 0
                }
            }),
            created_at: now,
        };

        let view = to_chat_message_view(message, Vec::new(), Vec::new());

        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.turn.as_ref())
                .map(|turn| turn.assistant_message_persisted_at),
            Some(Some(now))
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.turn.as_ref())
                .map(|turn| turn.artifact_commit_status.clone()),
            Some(contracts::ChatTurnArtifactCommitStatusView::Completed)
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.turn.as_ref())
                .map(|turn| turn.artifact_commit_ready_at),
            Some(Some(now))
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.turn.as_ref())
                .map(|turn| turn.events.len()),
            Some(6)
        );
        assert_eq!(
            view.message_manifest_view
                .as_ref()
                .and_then(|manifest| manifest.turn.as_ref())
                .and_then(|turn| turn.events.get(4))
                .map(|event| event.kind.clone()),
            Some(contracts::ChatTurnEventKindView::AssistantMessagePersisted)
        );
    }

    #[test]
    fn to_tool_execution_view_parses_tool_snapshot() {
        let tool_execution = ToolExecution {
            id: ToolExecutionId::new(),
            tenant_id: TenantId::new(),
            execution_id: WorkflowExecutionId::new(),
            source_kind: ToolExecutionSourceKind::ChatMessage,
            dataset_output_id: None,
            chat_message_id: Some(ChatMessageId::new()),
            sequence_no: 0,
            call_id: Some("call_weather".to_string()),
            tool_name: "weather.lookup".to_string(),
            tool_snapshot: Some(json!({
                "key": "weather.lookup",
                "title": "Weather Lookup",
                "scope_policy": "session",
                "invocation_mode": "cli",
                "cli": {
                    "argv": ["cargo", "run", "-p", "weather-tool"],
                    "env_allowlist": ["WEATHER_API_KEY"],
                    "output_mode": "json",
                    "timeout_ms": 15000
                }
            })),
            status: ToolExecutionStatus::Completed,
            arguments: Some(json!({ "city": "Shanghai" })),
            result: Some(json!({ "summary": "sunny" })),
            created_at: Utc::now(),
        };

        let view = to_tool_execution_view(tool_execution);

        assert_eq!(
            view.source_kind,
            contracts::ToolExecutionSourceKindView::ChatMessage
        );
        assert_eq!(view.call_id.as_deref(), Some("call_weather"));
        assert_eq!(view.tool_name, "weather.lookup");
        assert_eq!(
            view.status,
            contracts::ManifestToolCallStatusView::Completed
        );
        assert_eq!(
            view.tool.as_ref().map(|tool| tool.key.as_str()),
            Some("weather.lookup")
        );
        assert_eq!(
            view.tool
                .as_ref()
                .and_then(|tool| tool.cli.as_ref())
                .map(|cli| cli.output_mode.clone()),
            Some(contracts::ToolCliOutputModeView::Json)
        );
    }

    #[test]
    fn to_llm_invocation_view_supports_workflow_execution_source_kind() {
        let invocation = LlmInvocation {
            id: LlmInvocationId::new(),
            tenant_id: TenantId::new(),
            execution_id: WorkflowExecutionId::new(),
            source_kind: LlmInvocationSourceKind::WorkflowExecution,
            dataset_output_id: None,
            chat_message_id: None,
            sequence_no: 0,
            mode: LlmInvocationMode::Provider,
            provider: Some("openai".to_string()),
            model: Some("gpt-5.4".to_string()),
            request_id: Some("req_execution_scope".to_string()),
            finish_reason: Some(LlmInvocationFinishReason::Error),
            latency_ms: Some(42),
            usage: None,
            system_prompt_key: Some("dataset_output.live".to_string()),
            system_prompt_version: Some("v2".to_string()),
            tool_trace_count: Some(1),
            created_at: Utc::now(),
        };

        let view = to_llm_invocation_view(invocation);

        assert_eq!(
            view.source_kind,
            contracts::LlmInvocationSourceKindView::WorkflowExecution
        );
        assert_eq!(view.dataset_output_id, None);
        assert_eq!(view.chat_message_id, None);
        assert_eq!(
            view.finish_reason,
            Some(contracts::LlmInvocationFinishReasonView::Error)
        );
    }

    #[test]
    fn summarize_execution_scope_runtime_prefers_workflow_execution_records() {
        let execution_id = WorkflowExecutionId::new();
        let summary = summarize_execution_scope_runtime(
            &[
                contracts::LlmInvocationView {
                    id: domain_model::LlmInvocationId::new(),
                    execution_id,
                    source_kind: contracts::LlmInvocationSourceKindView::DatasetOutput,
                    dataset_output_id: Some(DatasetOutputId::new()),
                    chat_message_id: None,
                    sequence_no: 0,
                    mode: contracts::LlmInvocationModeView::Provider,
                    provider: Some("ignore-me".to_string()),
                    model: Some("old-model".to_string()),
                    request_id: Some("req_old".to_string()),
                    finish_reason: Some(contracts::LlmInvocationFinishReasonView::Stop),
                    latency_ms: Some(1),
                    usage: None,
                    system_prompt_key: None,
                    system_prompt_version: None,
                    tool_trace_count: Some(0),
                    created_at: Utc::now(),
                },
                contracts::LlmInvocationView {
                    id: domain_model::LlmInvocationId::new(),
                    execution_id,
                    source_kind: contracts::LlmInvocationSourceKindView::WorkflowExecution,
                    dataset_output_id: None,
                    chat_message_id: None,
                    sequence_no: 1,
                    mode: contracts::LlmInvocationModeView::Provider,
                    provider: Some("openai".to_string()),
                    model: Some("gpt-5.4".to_string()),
                    request_id: Some("req_execution_scope".to_string()),
                    finish_reason: Some(contracts::LlmInvocationFinishReasonView::Error),
                    latency_ms: Some(42),
                    usage: None,
                    system_prompt_key: None,
                    system_prompt_version: None,
                    tool_trace_count: Some(2),
                    created_at: Utc::now(),
                },
            ],
            &[
                contracts::ToolExecutionView {
                    id: ToolExecutionId::new(),
                    execution_id,
                    source_kind: contracts::ToolExecutionSourceKindView::WorkflowExecution,
                    dataset_output_id: None,
                    chat_message_id: None,
                    sequence_no: 0,
                    call_id: Some("call_failed".to_string()),
                    tool_name: "weather.lookup".to_string(),
                    tool: None,
                    status: contracts::ManifestToolCallStatusView::Failed,
                    arguments: None,
                    result: None,
                    created_at: Utc::now(),
                },
                contracts::ToolExecutionView {
                    id: ToolExecutionId::new(),
                    execution_id,
                    source_kind: contracts::ToolExecutionSourceKindView::WorkflowExecution,
                    dataset_output_id: None,
                    chat_message_id: None,
                    sequence_no: 1,
                    call_id: Some("call_completed".to_string()),
                    tool_name: "retrieval.search".to_string(),
                    tool: None,
                    status: contracts::ManifestToolCallStatusView::Completed,
                    arguments: None,
                    result: None,
                    created_at: Utc::now(),
                },
            ],
        )
        .expect("workflow execution summary should exist");

        assert_eq!(summary.llm_invocation_count, 1);
        assert_eq!(summary.tool_execution_count, 2);
        assert_eq!(summary.failed_tool_execution_count, 1);
        assert_eq!(summary.latest_provider.as_deref(), Some("openai"));
        assert_eq!(summary.latest_model.as_deref(), Some("gpt-5.4"));
        assert_eq!(
            summary.latest_request_id.as_deref(),
            Some("req_execution_scope")
        );
        assert_eq!(
            summary.latest_finish_reason,
            Some(contracts::LlmInvocationFinishReasonView::Error)
        );
        assert_eq!(summary.latest_tool_trace_count, Some(2));
    }

    #[test]
    fn derive_model_facing_summary_marks_chat_runtime_as_mixed_when_memory_and_evidence_exist() {
        let now = Utc::now();
        let inspect = WorkflowRuntimeInspectView {
            execution: WorkflowExecutionView {
                id: WorkflowExecutionId::new(),
                kind: domain_model::WorkflowKind::ChatSession,
                status: WorkflowStatus::Succeeded,
                stage: "completed".to_string(),
                updated_at: now,
            },
            execution_scope_runtime: None,
            dataset_output: None,
            chat_session: Some(ChatSessionView {
                id: domain_model::ChatSessionId::new(),
                dataset_id: DatasetId::new(),
                execution_id: WorkflowExecutionId::new(),
                title: "Chat".to_string(),
                latest_memory_directory_id: Some(MemoryDirectoryId::new()),
                latest_memory_directory: None,
                latest_dataset_output_id: None,
                latest_dataset_output: None,
                latest_assistant_message_id: Some(ChatMessageId::new()),
                latest_assistant_message: Some(ChatMessageView {
                    id: ChatMessageId::new(),
                    session_id: domain_model::ChatSessionId::new(),
                    role: domain_model::ChatMessageRole::Assistant,
                    turn_index: 1,
                    content: "Answer".to_string(),
                    llm_invocations: Vec::new(),
                    tool_executions: Vec::new(),
                    message_manifest: json!({}),
                    message_manifest_view: Some(contracts::ChatMessageManifestView {
                        generator: "chat-session-worker".to_string(),
                        schema_version: "0.5.0".to_string(),
                        dataset_id: DatasetId::new(),
                        prompt: "Prompt".to_string(),
                        indexed_document_count: 2,
                        refreshed_chunks: 2,
                        prior_message_count: 0,
                        latest_memory_directory_id: Some(MemoryDirectoryId::new()),
                        latest_memory_directory_version_no: Some(1),
                        latest_dataset_output_id: None,
                        output: Some(contracts::ChatMessageOutputView {
                            format: contracts::ChatMessageOutputFormatView::Markdown,
                            sections: vec![contracts::ChatMessageSectionView {
                                section_key: "reply".to_string(),
                                kind: contracts::ChatMessageSectionKindView::Reply,
                                title: "Reply".to_string(),
                                content: "Answer".to_string(),
                                retrieval_evidence_ids: vec![
                                    RetrievalEvidenceId::new(),
                                    RetrievalEvidenceId::new(),
                                ],
                            }],
                        }),
                        service_handoff: None,
                        tool_trace: Vec::new(),
                        turn: Some(contracts::ChatTurnRuntimeView {
                            turn_id: "turn_1".to_string(),
                            status: contracts::ChatTurnStatusView::Completed,
                            stream_mode: contracts::ChatTurnStreamModeView::Buffered,
                            stream_status: contracts::ChatTurnStreamStatusView::NotRequested,
                            provider_status: contracts::ChatTurnProviderStatusView::Responded,
                            provider_failure: Some(contracts::ManifestProviderFailureView {
                                kind: contracts::ManifestProviderFailureKindView::FinishReasonError,
                                message: "openai returned finish_reason=error".to_string(),
                            }),
                            artifact_commit_status:
                                contracts::ChatTurnArtifactCommitStatusView::Completed,
                            tool_loop_status: contracts::ChatTurnToolLoopStatusView::NotRequested,
                            provider_request_id: Some("req_1".to_string()),
                            provider_requested_at: Some(now),
                            provider_responded_at: Some(now),
                            first_token_at: None,
                            stream_completed_at: None,
                            artifact_commit_ready_at: Some(now),
                            artifact_commit_failure_source: None,
                            tool_calls_emitted_at: None,
                            tool_loop_settled_at: Some(now),
                            finish_reason: Some(contracts::ManifestFinishReasonView::Stop),
                            assistant_message_id: None,
                            assistant_message_persisted_at: Some(now),
                            tool_trace_count: 0,
                            tool_status_summary: None,
                            events: Vec::new(),
                            started_at: now,
                            completed_at: Some(now),
                        }),
                        context_binding: contracts::ManifestContextBindingView::CreationTime,
                        runtime: None,
                    }),
                    model_facing: None,
                    created_at: now,
                }),
                session_manifest: json!({}),
                session_manifest_view: None,
                model_facing: None,
                created_at: now,
                updated_at: now,
            }),
            report_plan: None,
            report_render_output: None,
            chat_messages: Vec::new(),
            llm_invocations: Vec::new(),
            tool_executions: Vec::new(),
            model_facing: None,
            pretty_summaries: Vec::new(),
        };

        let summary = derive_model_facing_summary(&inspect);

        assert_eq!(
            summary.capability_class,
            contracts::ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis
        );
        assert_eq!(
            summary.service_lane,
            contracts::ModelFacingServiceLaneView::MaterialService
        );
        assert_eq!(
            summary.report_entry_state,
            contracts::ModelFacingReportEntryStateView::NotApplicable
        );
        assert_eq!(
            summary.evidence_state,
            contracts::ModelFacingEvidenceStateView::Mixed
        );
        assert_eq!(
            summary.continuation_state,
            contracts::ModelFacingContinuationStateView::ReadyToAnswer
        );
        assert_eq!(
            summary.recommended_next_action,
            Some(contracts::ModelFacingNextActionView::AnswerDirectly)
        );
        assert!(summary
            .allowed_next_actions
            .contains(&contracts::ModelFacingNextActionView::AnswerDirectly));
        assert!(summary
            .allowed_next_actions
            .contains(&contracts::ModelFacingNextActionView::ReadDocumentDetail));
        assert!(summary
            .allowed_tool_keys
            .contains(&"document.read_detail".to_string()));
        assert!(summary
            .allowed_next_actions
            .contains(&contracts::ModelFacingNextActionView::CompareDocuments));
        assert!(summary
            .allowed_tool_keys
            .contains(&"document.compare".to_string()));
        assert!(summary
            .signals
            .iter()
            .any(|signal| signal == "retrieval_evidence_count=2"));
    }

    #[test]
    fn derive_document_detail_model_facing_summary_marks_failed_retrieval_as_degraded() {
        let now = Utc::now();
        let detail = DocumentDetailView {
            document: DocumentSummary {
                id: DocumentId::new(),
                dataset_id: DatasetId::new(),
                title: "Broken Detail".to_string(),
                object_key: "documents/broken.md".to_string(),
                content_type: "text/markdown".to_string(),
                lifecycle: contracts::DocumentLifecycleView::Indexed,
                secret_binding_ids: Vec::new(),
                created_at: now,
                updated_at: now,
            },
            chunks: vec![DocumentChunkView {
                id: DocumentChunkId::new(),
                document_id: DocumentId::new(),
                chunk_index: 0,
                token_count: 12,
                state: contracts::DocumentChunkStateView::Indexed,
                content: "Broken retrieval detail".to_string(),
                metadata: json!({}),
                created_at: now,
                updated_at: now,
            }],
            retrieval_evidences: vec![RetrievalEvidenceView {
                id: RetrievalEvidenceId::new(),
                dataset_id: DatasetId::new(),
                document_id: DocumentId::new(),
                document_chunk_id: DocumentChunkId::new(),
                execution_id: WorkflowExecutionId::new(),
                chunk_index: 0,
                source_locator: "documents/broken.md#chunk=0".to_string(),
                content_excerpt: "Broken retrieval detail".to_string(),
                summary: "Broken retrieval detail".to_string(),
                payload_filter_key: "dataset/broken".to_string(),
                embedding_model: "local_lexical".to_string(),
                recall_score: 0.0,
                evidence_manifest: json!({
                    "embedding": { "status": "indexed", "model": "local_lexical", "token_count": 12 },
                    "recall": { "status": "failed", "score": 0.0, "rank_hint": 1 },
                    "evidence": {
                        "document_chunk_id": DocumentChunkId::new(),
                        "payload_filter_key": "dataset/broken",
                        "source_locator": "documents/broken.md#chunk=0"
                    }
                }),
                evidence_manifest_view: Some(contracts::RetrievalEvidenceManifestView {
                    schema_version: "0.4.0".to_string(),
                    generator: "retrieval-worker".to_string(),
                    dataset_id: DatasetId::new(),
                    document_id: DocumentId::new(),
                    document_chunk_id: DocumentChunkId::new(),
                    chunk_index: 0,
                    indexed_at: now,
                    embedding: contracts::RetrievalEmbeddingManifestView {
                        status: contracts::RetrievalEmbeddingStatusView::Indexed,
                        model: "local_lexical".to_string(),
                        token_count: 12,
                    },
                    recall: contracts::RetrievalRecallManifestView {
                        status: contracts::RetrievalRecallStatusView::Failed,
                        score: 0.0,
                        rank_hint: 1,
                    },
                    evidence: contracts::RetrievalEvidenceLocatorManifestView {
                        document_chunk_id: DocumentChunkId::new(),
                        payload_filter_key: "dataset/broken".to_string(),
                        source_locator: "documents/broken.md#chunk=0".to_string(),
                    },
                }),
                created_at: now,
            }],
            model_facing: None,
        };

        let summary = derive_document_detail_model_facing_summary(&detail);

        assert_eq!(
            summary.capability_class,
            contracts::ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis
        );
        assert_eq!(
            summary.evidence_state,
            contracts::ModelFacingEvidenceStateView::Degraded
        );
        assert_eq!(
            summary.continuation_state,
            contracts::ModelFacingContinuationStateView::RetryRequired
        );
        assert_eq!(
            summary.recommended_next_action,
            Some(contracts::ModelFacingNextActionView::RetryExecution)
        );
        assert!(summary
            .signals
            .iter()
            .any(|signal| signal == "failed_retrieval_evidence_count=1"));
    }

    #[test]
    fn derive_compare_documents_model_facing_summary_marks_multi_document_detail_as_mixed() {
        let now = Utc::now();
        let comparison = CompareDocumentsView {
            documents: vec![
                DocumentDetailView {
                    document: DocumentSummary {
                        id: DocumentId::new(),
                        dataset_id: DatasetId::new(),
                        title: "Doc A".to_string(),
                        object_key: "documents/a.md".to_string(),
                        content_type: "text/markdown".to_string(),
                        lifecycle: contracts::DocumentLifecycleView::Indexed,
                        secret_binding_ids: Vec::new(),
                        created_at: now,
                        updated_at: now,
                    },
                    chunks: vec![DocumentChunkView {
                        id: DocumentChunkId::new(),
                        document_id: DocumentId::new(),
                        chunk_index: 0,
                        token_count: 8,
                        state: contracts::DocumentChunkStateView::Indexed,
                        content: "Document A detail".to_string(),
                        metadata: json!({}),
                        created_at: now,
                        updated_at: now,
                    }],
                    retrieval_evidences: vec![RetrievalEvidenceView {
                        id: RetrievalEvidenceId::new(),
                        dataset_id: DatasetId::new(),
                        document_id: DocumentId::new(),
                        document_chunk_id: DocumentChunkId::new(),
                        execution_id: WorkflowExecutionId::new(),
                        chunk_index: 0,
                        source_locator: "documents/a.md#chunk=0".to_string(),
                        content_excerpt: "Document A detail".to_string(),
                        summary: "Document A".to_string(),
                        payload_filter_key: "dataset/a".to_string(),
                        embedding_model: "local_lexical".to_string(),
                        recall_score: 0.9,
                        evidence_manifest: json!({}),
                        evidence_manifest_view: None,
                        created_at: now,
                    }],
                    model_facing: None,
                },
                DocumentDetailView {
                    document: DocumentSummary {
                        id: DocumentId::new(),
                        dataset_id: DatasetId::new(),
                        title: "Doc B".to_string(),
                        object_key: "documents/b.md".to_string(),
                        content_type: "text/markdown".to_string(),
                        lifecycle: contracts::DocumentLifecycleView::Indexed,
                        secret_binding_ids: Vec::new(),
                        created_at: now,
                        updated_at: now,
                    },
                    chunks: vec![DocumentChunkView {
                        id: DocumentChunkId::new(),
                        document_id: DocumentId::new(),
                        chunk_index: 0,
                        token_count: 8,
                        state: contracts::DocumentChunkStateView::Indexed,
                        content: "Document B detail".to_string(),
                        metadata: json!({}),
                        created_at: now,
                        updated_at: now,
                    }],
                    retrieval_evidences: vec![RetrievalEvidenceView {
                        id: RetrievalEvidenceId::new(),
                        dataset_id: DatasetId::new(),
                        document_id: DocumentId::new(),
                        document_chunk_id: DocumentChunkId::new(),
                        execution_id: WorkflowExecutionId::new(),
                        chunk_index: 0,
                        source_locator: "documents/b.md#chunk=0".to_string(),
                        content_excerpt: "Document B detail".to_string(),
                        summary: "Document B".to_string(),
                        payload_filter_key: "dataset/b".to_string(),
                        embedding_model: "local_lexical".to_string(),
                        recall_score: 0.88,
                        evidence_manifest: json!({}),
                        evidence_manifest_view: None,
                        created_at: now,
                    }],
                    model_facing: None,
                },
            ],
            model_facing: None,
        };

        let summary = derive_compare_documents_model_facing_summary(&comparison);

        assert_eq!(
            summary.capability_class,
            contracts::ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis
        );
        assert_eq!(
            summary.evidence_state,
            contracts::ModelFacingEvidenceStateView::Mixed
        );
        assert_eq!(
            summary.continuation_state,
            contracts::ModelFacingContinuationStateView::ReadyToAnswer
        );
        assert_eq!(
            summary.recommended_next_action,
            Some(contracts::ModelFacingNextActionView::AnswerDirectly)
        );
        assert!(summary
            .allowed_next_actions
            .contains(&contracts::ModelFacingNextActionView::ReadDocumentDetail));
        assert!(summary
            .allowed_tool_keys
            .contains(&"document.read_detail".to_string()));
        assert!(summary
            .signals
            .iter()
            .any(|signal| signal == "distinct_document_count=2"));
    }

    #[test]
    fn derive_dataset_output_model_facing_summary_marks_retrieval_only_output_as_evidence_retrieval(
    ) {
        let now = Utc::now();
        let document_id = DocumentId::new();
        let output = DatasetOutputView {
            id: DatasetOutputId::new(),
            dataset_id: DatasetId::new(),
            execution_id: WorkflowExecutionId::new(),
            prompt: "Find relevant evidence".to_string(),
            output_text: String::new(),
            memory_directory_id: None,
            memory_directory: None,
            retrieval_evidence_ids: vec![RetrievalEvidenceId::new()],
            retrieval_evidences: vec![RetrievalEvidenceView {
                id: RetrievalEvidenceId::new(),
                dataset_id: DatasetId::new(),
                document_id,
                document_chunk_id: DocumentChunkId::new(),
                execution_id: WorkflowExecutionId::new(),
                chunk_index: 0,
                source_locator: "documents/demo.md#chunk=0".to_string(),
                content_excerpt: "detail evidence".to_string(),
                summary: "detail evidence".to_string(),
                payload_filter_key: "dataset/demo".to_string(),
                embedding_model: "placeholder".to_string(),
                recall_score: 0.88,
                evidence_manifest: json!({}),
                evidence_manifest_view: None,
                created_at: now,
            }],
            llm_invocations: Vec::new(),
            tool_executions: Vec::new(),
            output_manifest: json!({}),
            output_manifest_view: Some(contracts::DatasetOutputManifestView {
                generator: "dataset-output-worker".to_string(),
                schema_version: "0.5.0".to_string(),
                dataset_id: DatasetId::new(),
                prompt: "Find relevant evidence".to_string(),
                indexed_document_count: 1,
                refreshed_chunks: 1,
                memory_directory_id: None,
                memory_directory_version_no: None,
                retrieval_evidence_count: 1,
                retrieval_evidence_ids: Vec::new(),
                output: None,
                service_handoff: None,
                tool_trace: Vec::new(),
                context_binding: contracts::ManifestContextBindingView::CreationTime,
                runtime: None,
            }),
            model_facing: None,
            created_at: now,
        };

        let summary = derive_dataset_output_model_facing_summary(&output);

        assert_eq!(
            summary.capability_class,
            contracts::ModelFacingCapabilityClassView::EvidenceRetrieval
        );
        assert_eq!(
            summary.service_lane,
            contracts::ModelFacingServiceLaneView::MaterialService
        );
        assert_eq!(
            summary.report_entry_state,
            contracts::ModelFacingReportEntryStateView::NotApplicable
        );
        assert_eq!(
            summary.evidence_state,
            contracts::ModelFacingEvidenceStateView::SupplyOnly
        );
        assert_eq!(
            summary.continuation_state,
            contracts::ModelFacingContinuationStateView::NeedsPlatformContinuation
        );
        assert_eq!(
            summary.recommended_next_action,
            Some(contracts::ModelFacingNextActionView::ReadDocumentDetail)
        );
        assert_eq!(
            summary.recommended_tool_key,
            Some("document.read_detail".to_string())
        );
        assert!(summary
            .allowed_next_actions
            .contains(&contracts::ModelFacingNextActionView::ReadDocumentDetail));
        assert!(summary
            .allowed_tool_keys
            .contains(&"document.read_detail".to_string()));
        assert!(!summary
            .allowed_next_actions
            .contains(&contracts::ModelFacingNextActionView::AnswerDirectly));
        assert!(summary
            .signals
            .iter()
            .any(|signal| signal == "document_focus=single_document"));
    }

    #[test]
    fn derive_dataset_output_model_facing_summary_exposes_compare_documents_tool_key_for_multi_document_retrieval(
    ) {
        let now = Utc::now();
        let dataset_id = DatasetId::new();
        let document_a_id = DocumentId::new();
        let document_b_id = DocumentId::new();
        let output = DatasetOutputView {
            id: DatasetOutputId::new(),
            dataset_id,
            execution_id: WorkflowExecutionId::new(),
            prompt: "Compare the two contracts".to_string(),
            output_text: String::new(),
            memory_directory_id: None,
            memory_directory: None,
            retrieval_evidence_ids: vec![RetrievalEvidenceId::new(), RetrievalEvidenceId::new()],
            retrieval_evidences: vec![
                RetrievalEvidenceView {
                    id: RetrievalEvidenceId::new(),
                    dataset_id,
                    document_id: document_a_id,
                    document_chunk_id: DocumentChunkId::new(),
                    execution_id: WorkflowExecutionId::new(),
                    chunk_index: 0,
                    source_locator: "documents/contract-a.md#chunk=0".to_string(),
                    content_excerpt: "Contract A requires a 30-day notice.".to_string(),
                    summary: "Contract A notice period".to_string(),
                    payload_filter_key: "dataset/contract-a".to_string(),
                    embedding_model: "placeholder".to_string(),
                    recall_score: 0.86,
                    evidence_manifest: json!({}),
                    evidence_manifest_view: None,
                    created_at: now,
                },
                RetrievalEvidenceView {
                    id: RetrievalEvidenceId::new(),
                    dataset_id,
                    document_id: document_b_id,
                    document_chunk_id: DocumentChunkId::new(),
                    execution_id: WorkflowExecutionId::new(),
                    chunk_index: 0,
                    source_locator: "documents/contract-b.md#chunk=0".to_string(),
                    content_excerpt: "Contract B requires a 60-day notice.".to_string(),
                    summary: "Contract B notice period".to_string(),
                    payload_filter_key: "dataset/contract-b".to_string(),
                    embedding_model: "placeholder".to_string(),
                    recall_score: 0.9,
                    evidence_manifest: json!({}),
                    evidence_manifest_view: None,
                    created_at: now,
                },
            ],
            llm_invocations: Vec::new(),
            tool_executions: Vec::new(),
            output_manifest: json!({}),
            output_manifest_view: Some(contracts::DatasetOutputManifestView {
                generator: "dataset-output-worker".to_string(),
                schema_version: "0.5.0".to_string(),
                dataset_id,
                prompt: "Compare the two contracts".to_string(),
                indexed_document_count: 2,
                refreshed_chunks: 2,
                memory_directory_id: None,
                memory_directory_version_no: None,
                retrieval_evidence_count: 2,
                retrieval_evidence_ids: Vec::new(),
                output: None,
                service_handoff: None,
                tool_trace: Vec::new(),
                context_binding: contracts::ManifestContextBindingView::CreationTime,
                runtime: None,
            }),
            model_facing: None,
            created_at: now,
        };

        let summary = derive_dataset_output_model_facing_summary(&output);

        assert_eq!(
            summary.capability_class,
            contracts::ModelFacingCapabilityClassView::EvidenceRetrieval
        );
        assert_eq!(
            summary.continuation_state,
            contracts::ModelFacingContinuationStateView::NeedsPlatformContinuation
        );
        assert_eq!(
            summary.recommended_next_action,
            Some(contracts::ModelFacingNextActionView::CompareDocuments)
        );
        assert_eq!(
            summary.recommended_tool_key,
            Some("document.compare".to_string())
        );
        assert!(summary
            .allowed_next_actions
            .contains(&contracts::ModelFacingNextActionView::CompareDocuments));
        assert!(summary
            .allowed_next_actions
            .contains(&contracts::ModelFacingNextActionView::ReadDocumentDetail));
        assert!(summary
            .allowed_tool_keys
            .contains(&"document.compare".to_string()));
        assert!(summary
            .allowed_tool_keys
            .contains(&"document.read_detail".to_string()));
        assert!(summary
            .signals
            .iter()
            .any(|signal| signal == "document_focus=multi_document"));
    }

    #[test]
    fn derive_dataset_output_model_facing_summary_exposes_refresh_directory_tool_key() {
        let now = Utc::now();
        let memory_directory_id = MemoryDirectoryId::new();
        let output = DatasetOutputView {
            id: DatasetOutputId::new(),
            dataset_id: DatasetId::new(),
            execution_id: WorkflowExecutionId::new(),
            prompt: "Show me the dataset structure".to_string(),
            output_text: String::new(),
            memory_directory_id: Some(memory_directory_id),
            memory_directory: None,
            retrieval_evidence_ids: Vec::new(),
            retrieval_evidences: Vec::new(),
            llm_invocations: Vec::new(),
            tool_executions: Vec::new(),
            output_manifest: json!({}),
            output_manifest_view: Some(contracts::DatasetOutputManifestView {
                generator: "dataset-output-worker".to_string(),
                schema_version: "0.5.0".to_string(),
                dataset_id: DatasetId::new(),
                prompt: "Show me the dataset structure".to_string(),
                indexed_document_count: 0,
                refreshed_chunks: 0,
                memory_directory_id: Some(memory_directory_id),
                memory_directory_version_no: Some(1),
                retrieval_evidence_count: 0,
                retrieval_evidence_ids: Vec::new(),
                output: None,
                service_handoff: None,
                tool_trace: Vec::new(),
                context_binding: contracts::ManifestContextBindingView::CreationTime,
                runtime: None,
            }),
            model_facing: None,
            created_at: now,
        };

        let summary = derive_dataset_output_model_facing_summary(&output);

        assert_eq!(
            summary.capability_class,
            contracts::ModelFacingCapabilityClassView::DatasetDirectoryAwareness
        );
        assert_eq!(
            summary.recommended_next_action,
            Some(contracts::ModelFacingNextActionView::AnswerDirectly)
        );
        assert_eq!(summary.recommended_tool_key, None);
        assert!(summary
            .allowed_next_actions
            .contains(&contracts::ModelFacingNextActionView::RefreshDirectory));
        assert!(summary
            .allowed_tool_keys
            .contains(&"memory_directory.refresh".to_string()));
        assert!(summary
            .signals
            .iter()
            .any(|signal| signal == "has_memory_directory=true"));
    }

    #[test]
    fn derive_dataset_output_model_facing_summary_marks_single_document_answer_as_live_detail() {
        let now = Utc::now();
        let dataset_id = DatasetId::new();
        let document_id = DocumentId::new();
        let output = DatasetOutputView {
            id: DatasetOutputId::new(),
            dataset_id,
            execution_id: WorkflowExecutionId::new(),
            prompt: "Explain clause 4".to_string(),
            output_text: "Clause 4 requires written approval.".to_string(),
            memory_directory_id: None,
            memory_directory: None,
            retrieval_evidence_ids: vec![RetrievalEvidenceId::new()],
            retrieval_evidences: vec![RetrievalEvidenceView {
                id: RetrievalEvidenceId::new(),
                dataset_id,
                document_id,
                document_chunk_id: DocumentChunkId::new(),
                execution_id: WorkflowExecutionId::new(),
                chunk_index: 0,
                source_locator: "documents/contract.md#chunk=0".to_string(),
                content_excerpt: "Clause 4 requires written approval.".to_string(),
                summary: "Clause 4 approval rule".to_string(),
                payload_filter_key: "dataset/contract".to_string(),
                embedding_model: "placeholder".to_string(),
                recall_score: 0.93,
                evidence_manifest: json!({}),
                evidence_manifest_view: None,
                created_at: now,
            }],
            llm_invocations: Vec::new(),
            tool_executions: Vec::new(),
            output_manifest: json!({}),
            output_manifest_view: Some(contracts::DatasetOutputManifestView {
                generator: "dataset-output-worker".to_string(),
                schema_version: "0.5.0".to_string(),
                dataset_id,
                prompt: "Explain clause 4".to_string(),
                indexed_document_count: 1,
                refreshed_chunks: 1,
                memory_directory_id: None,
                memory_directory_version_no: None,
                retrieval_evidence_count: 1,
                retrieval_evidence_ids: Vec::new(),
                output: Some(contracts::DatasetOutputContentView {
                    format: contracts::DatasetOutputFormatView::Markdown,
                    sections: vec![contracts::DatasetOutputSectionView {
                        section_key: "summary".to_string(),
                        kind: contracts::DatasetOutputSectionKindView::Summary,
                        title: "Summary".to_string(),
                        content: "Clause 4 requires written approval.".to_string(),
                        retrieval_evidence_ids: Vec::new(),
                    }],
                }),
                service_handoff: None,
                tool_trace: Vec::new(),
                context_binding: contracts::ManifestContextBindingView::CreationTime,
                runtime: None,
            }),
            model_facing: None,
            created_at: now,
        };

        let summary = derive_dataset_output_model_facing_summary(&output);

        assert_eq!(
            summary.capability_class,
            contracts::ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis
        );
        assert_eq!(
            summary.service_lane,
            contracts::ModelFacingServiceLaneView::MaterialService
        );
        assert_eq!(
            summary.report_entry_state,
            contracts::ModelFacingReportEntryStateView::NotApplicable
        );
        assert_eq!(
            summary.evidence_state,
            contracts::ModelFacingEvidenceStateView::LiveDetail
        );
        assert_eq!(
            summary.continuation_state,
            contracts::ModelFacingContinuationStateView::ReadyToAnswer
        );
        assert_eq!(
            summary.recommended_next_action,
            Some(contracts::ModelFacingNextActionView::AnswerDirectly)
        );
        assert_eq!(summary.recommended_tool_key, None);
        assert!(summary
            .allowed_next_actions
            .contains(&contracts::ModelFacingNextActionView::AnswerDirectly));
        assert!(summary
            .allowed_next_actions
            .contains(&contracts::ModelFacingNextActionView::ReadDocumentDetail));
        assert!(summary
            .allowed_tool_keys
            .contains(&"document.read_detail".to_string()));
        assert!(!summary
            .allowed_next_actions
            .contains(&contracts::ModelFacingNextActionView::CompareDocuments));
        assert!(summary
            .signals
            .iter()
            .any(|signal| signal == "distinct_document_count=1"));
    }

    #[test]
    fn derive_dataset_output_model_facing_summary_exposes_pending_report_entry_handoff() {
        let now = Utc::now();
        let output = DatasetOutputView {
            id: DatasetOutputId::new(),
            dataset_id: DatasetId::new(),
            execution_id: WorkflowExecutionId::new(),
            prompt: "Turn this into a report".to_string(),
            output_text: "I can prepare a report outline once confirmed.".to_string(),
            memory_directory_id: None,
            memory_directory: None,
            retrieval_evidence_ids: Vec::new(),
            retrieval_evidences: Vec::new(),
            llm_invocations: Vec::new(),
            tool_executions: Vec::new(),
            output_manifest: json!({}),
            output_manifest_view: Some(contracts::DatasetOutputManifestView {
                generator: "dataset-output-worker".to_string(),
                schema_version: "0.5.0".to_string(),
                dataset_id: DatasetId::new(),
                prompt: "Turn this into a report".to_string(),
                indexed_document_count: 1,
                refreshed_chunks: 0,
                memory_directory_id: None,
                memory_directory_version_no: None,
                retrieval_evidence_count: 0,
                retrieval_evidence_ids: Vec::new(),
                output: Some(contracts::DatasetOutputContentView {
                    format: contracts::DatasetOutputFormatView::Markdown,
                    sections: vec![contracts::DatasetOutputSectionView {
                        section_key: "summary".to_string(),
                        kind: contracts::DatasetOutputSectionKindView::Summary,
                        title: "Summary".to_string(),
                        content: "I can prepare a report outline once confirmed.".to_string(),
                        retrieval_evidence_ids: Vec::new(),
                    }],
                }),
                service_handoff: Some(contracts::ManifestServiceHandoffView {
                    source: contracts::ManifestServiceHandoffSourceView::ChatSessionReportEntry,
                    service_lane: contracts::ModelFacingServiceLaneView::MaterialService,
                    report_entry_state:
                        contracts::ModelFacingReportEntryStateView::ConfirmationRequired,
                    requested_at: Some(now),
                    resolved_at: None,
                    resolved_action: None,
                    suggested_title: Some("Dataset Report".to_string()),
                    suggested_objective: Some(
                        "Turn the current dataset context into a report-ready output.".to_string(),
                    ),
                    confirmed_report_plan_id: None,
                }),
                tool_trace: Vec::new(),
                context_binding: contracts::ManifestContextBindingView::CreationTime,
                runtime: None,
            }),
            model_facing: None,
            created_at: now,
        };

        let summary = derive_dataset_output_model_facing_summary(&output);

        assert_eq!(
            summary.capability_class,
            contracts::ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis
        );
        assert_eq!(
            summary.service_lane,
            contracts::ModelFacingServiceLaneView::MaterialService
        );
        assert_eq!(
            summary.report_entry_state,
            contracts::ModelFacingReportEntryStateView::ConfirmationRequired
        );
        assert_eq!(
            summary.continuation_state,
            contracts::ModelFacingContinuationStateView::NeedsUserConfirmation
        );
        assert_eq!(
            summary.recommended_next_action,
            Some(contracts::ModelFacingNextActionView::RequestReportEntryConfirmation)
        );
        assert_eq!(
            summary.recommended_tool_key,
            Some("chat_session.report_entry".to_string())
        );
        assert!(summary
            .signals
            .iter()
            .any(|signal| signal == "service_handoff_source=chat_session_report_entry"));
        assert!(summary
            .allowed_tool_keys
            .contains(&"chat_session.report_entry".to_string()));
    }

    #[test]
    fn derive_dataset_output_model_facing_summary_exposes_confirmed_report_service_handoff() {
        let now = Utc::now();
        let report_plan_id = ReportPlanId::new();
        let output = DatasetOutputView {
            id: DatasetOutputId::new(),
            dataset_id: DatasetId::new(),
            execution_id: WorkflowExecutionId::new(),
            prompt: "Continue the report".to_string(),
            output_text: "Report planning is ready to continue.".to_string(),
            memory_directory_id: None,
            memory_directory: None,
            retrieval_evidence_ids: Vec::new(),
            retrieval_evidences: Vec::new(),
            llm_invocations: Vec::new(),
            tool_executions: Vec::new(),
            output_manifest: json!({}),
            output_manifest_view: Some(contracts::DatasetOutputManifestView {
                generator: "dataset-output-worker".to_string(),
                schema_version: "0.5.0".to_string(),
                dataset_id: DatasetId::new(),
                prompt: "Continue the report".to_string(),
                indexed_document_count: 1,
                refreshed_chunks: 0,
                memory_directory_id: None,
                memory_directory_version_no: None,
                retrieval_evidence_count: 0,
                retrieval_evidence_ids: Vec::new(),
                output: Some(contracts::DatasetOutputContentView {
                    format: contracts::DatasetOutputFormatView::Markdown,
                    sections: vec![contracts::DatasetOutputSectionView {
                        section_key: "summary".to_string(),
                        kind: contracts::DatasetOutputSectionKindView::Summary,
                        title: "Summary".to_string(),
                        content: "Report planning is ready to continue.".to_string(),
                        retrieval_evidence_ids: Vec::new(),
                    }],
                }),
                service_handoff: Some(contracts::ManifestServiceHandoffView {
                    source: contracts::ManifestServiceHandoffSourceView::ChatSessionReportEntry,
                    service_lane: contracts::ModelFacingServiceLaneView::ReportService,
                    report_entry_state: contracts::ModelFacingReportEntryStateView::Confirmed,
                    requested_at: Some(now),
                    resolved_at: Some(now),
                    resolved_action: Some(
                        contracts::ChatSessionReportEntryResolutionView::EnterReportService,
                    ),
                    suggested_title: Some("Dataset Report".to_string()),
                    suggested_objective: Some(
                        "Turn the current dataset context into a report-ready output.".to_string(),
                    ),
                    confirmed_report_plan_id: Some(report_plan_id),
                }),
                tool_trace: Vec::new(),
                context_binding: contracts::ManifestContextBindingView::CreationTime,
                runtime: None,
            }),
            model_facing: None,
            created_at: now,
        };

        let summary = derive_dataset_output_model_facing_summary(&output);

        assert_eq!(
            summary.capability_class,
            contracts::ModelFacingCapabilityClassView::ReportPlanning
        );
        assert_eq!(
            summary.service_lane,
            contracts::ModelFacingServiceLaneView::ReportService
        );
        assert_eq!(
            summary.report_entry_state,
            contracts::ModelFacingReportEntryStateView::Confirmed
        );
        assert_eq!(
            summary.continuation_state,
            contracts::ModelFacingContinuationStateView::NeedsPlatformContinuation
        );
        assert_eq!(
            summary.recommended_next_action,
            Some(contracts::ModelFacingNextActionView::ContinueReportPlanning)
        );
        assert_eq!(
            summary.recommended_tool_key,
            Some("report.plan".to_string())
        );
        assert!(summary
            .allowed_tool_keys
            .contains(&"report.plan".to_string()));
        assert!(summary
            .signals
            .iter()
            .any(|signal| *signal == format!("confirmed_report_plan_id={report_plan_id}")));
    }

    #[test]
    fn derive_model_facing_summary_prefers_report_plan_provenance() {
        let now = Utc::now();
        let report_plan_id = ReportPlanId::new();
        let inspect = WorkflowRuntimeInspectView {
            execution: WorkflowExecutionView {
                id: WorkflowExecutionId::new(),
                kind: domain_model::WorkflowKind::ReportPlan,
                status: WorkflowStatus::Succeeded,
                stage: "completed".to_string(),
                updated_at: now,
            },
            execution_scope_runtime: None,
            dataset_output: None,
            chat_session: None,
            report_plan: Some(ReportPlanSummary {
                id: report_plan_id,
                dataset_id: DatasetId::new(),
                title: "Quarterly Report".to_string(),
                objective: "Summarize product and market signals".to_string(),
                status: contracts::ReportPlanStatusView::Planned,
                theme_key: "executive-default".to_string(),
                current_ast_version_id: Some(ReportPlanAstVersionId::new()),
                service_handoff: Some(contracts::ManifestServiceHandoffView {
                    source: contracts::ManifestServiceHandoffSourceView::ChatSessionReportEntry,
                    service_lane: contracts::ModelFacingServiceLaneView::ReportService,
                    report_entry_state: contracts::ModelFacingReportEntryStateView::Confirmed,
                    requested_at: Some(now),
                    resolved_at: Some(now),
                    resolved_action: Some(
                        contracts::ChatSessionReportEntryResolutionView::EnterReportService,
                    ),
                    suggested_title: Some("Quarterly Report".to_string()),
                    suggested_objective: Some("Summarize product and market signals".to_string()),
                    confirmed_report_plan_id: Some(report_plan_id),
                }),
                model_facing: None,
            }),
            report_render_output: None,
            chat_messages: Vec::new(),
            llm_invocations: Vec::new(),
            tool_executions: Vec::new(),
            model_facing: None,
            pretty_summaries: Vec::new(),
        };

        let summary = derive_model_facing_summary(&inspect);

        assert_eq!(
            summary.capability_class,
            contracts::ModelFacingCapabilityClassView::ReportPlanning
        );
        assert_eq!(
            summary.service_lane,
            contracts::ModelFacingServiceLaneView::ReportService
        );
        assert_eq!(
            summary.report_entry_state,
            contracts::ModelFacingReportEntryStateView::Confirmed
        );
        assert!(summary
            .signals
            .iter()
            .any(|signal| signal == "service_handoff_source=chat_session_report_entry"));
        assert!(summary
            .signals
            .iter()
            .any(|signal| *signal == format!("confirmed_report_plan_id={report_plan_id}")));
    }

    #[test]
    fn render_latest_assistant_turn_summary_includes_tool_status_summary() {
        let now = Utc::now();
        let assistant_message_id = ChatMessageId::new();
        let inspect = WorkflowRuntimeInspectView {
            execution: WorkflowExecutionView {
                id: WorkflowExecutionId::new(),
                kind: domain_model::WorkflowKind::ChatSession,
                status: WorkflowStatus::Succeeded,
                stage: "completed".to_string(),
                updated_at: now,
            },
            execution_scope_runtime: None,
            dataset_output: None,
            chat_session: Some(ChatSessionView {
                id: domain_model::ChatSessionId::new(),
                dataset_id: DatasetId::new(),
                execution_id: WorkflowExecutionId::new(),
                title: "Chat".to_string(),
                latest_memory_directory_id: None,
                latest_memory_directory: None,
                latest_dataset_output_id: None,
                latest_dataset_output: None,
                latest_assistant_message_id: Some(assistant_message_id),
                latest_assistant_message: Some(ChatMessageView {
                    id: assistant_message_id,
                    session_id: domain_model::ChatSessionId::new(),
                    role: domain_model::ChatMessageRole::Assistant,
                    turn_index: 1,
                    content: "Answer".to_string(),
                    llm_invocations: Vec::new(),
                    tool_executions: Vec::new(),
                    message_manifest: json!({}),
                    message_manifest_view: Some(contracts::ChatMessageManifestView {
                        generator: "chat-session-worker".to_string(),
                        schema_version: "0.5.0".to_string(),
                        dataset_id: DatasetId::new(),
                        prompt: "Prompt".to_string(),
                        indexed_document_count: 0,
                        refreshed_chunks: 0,
                        prior_message_count: 0,
                        latest_memory_directory_id: None,
                        latest_memory_directory_version_no: None,
                        latest_dataset_output_id: None,
                        output: None,
                        service_handoff: None,
                        tool_trace: Vec::new(),
                        turn: Some(contracts::ChatTurnRuntimeView {
                            turn_id: "turn_1".to_string(),
                            status: contracts::ChatTurnStatusView::Completed,
                            stream_mode: contracts::ChatTurnStreamModeView::Buffered,
                            stream_status: contracts::ChatTurnStreamStatusView::NotRequested,
                            provider_status: contracts::ChatTurnProviderStatusView::Responded,
                            provider_failure: Some(contracts::ManifestProviderFailureView {
                                kind: contracts::ManifestProviderFailureKindView::FinishReasonError,
                                message: "openai returned finish_reason=error".to_string(),
                            }),
                            artifact_commit_status:
                                contracts::ChatTurnArtifactCommitStatusView::Completed,
                            tool_loop_status: contracts::ChatTurnToolLoopStatusView::Pending,
                            provider_request_id: Some("req_1".to_string()),
                            provider_requested_at: Some(now),
                            provider_responded_at: Some(now),
                            artifact_commit_ready_at: Some(now),
                            artifact_commit_failure_source: None,
                            first_token_at: None,
                            stream_completed_at: None,
                            tool_calls_emitted_at: Some(now),
                            tool_loop_settled_at: None,
                            finish_reason: Some(contracts::ManifestFinishReasonView::ToolCalls),
                            assistant_message_id: Some(assistant_message_id),
                            assistant_message_persisted_at: Some(now),
                            tool_trace_count: 3,
                            tool_status_summary: Some(contracts::ChatTurnToolStatusSummaryView {
                                requested_count: 1,
                                completed_count: 1,
                                failed_count: 1,
                            }),
                            events: Vec::new(),
                            started_at: now,
                            completed_at: Some(now),
                        }),
                        context_binding: contracts::ManifestContextBindingView::CreationTime,
                        runtime: None,
                    }),
                    model_facing: None,
                    created_at: now,
                }),
                session_manifest: json!({}),
                session_manifest_view: None,
                model_facing: None,
                created_at: now,
                updated_at: now,
            }),
            report_plan: None,
            report_render_output: None,
            chat_messages: Vec::new(),
            llm_invocations: Vec::new(),
            tool_executions: Vec::new(),
            model_facing: None,
            pretty_summaries: Vec::new(),
        };

        let summary = render_latest_assistant_turn_summary(&inspect)
            .expect("latest assistant turn summary should render");

        assert!(summary.contains("Latest Assistant Turn"));
        assert!(summary.contains("assistant_message_id:"));
        assert!(summary.contains("turn_id: turn_1"));
        assert!(summary.contains("provider_request_id: req_1"));
        assert!(summary.contains("finish_reason: tool_calls"));
        assert!(summary.contains("provider_failure_kind: finish_reason_error"));
        assert!(summary.contains("provider_failure_message: openai returned finish_reason=error"));
        assert!(summary.contains("stream_status: NotRequested"));
        assert!(summary.contains("tool_loop_status: Pending"));
        assert!(summary.contains("tool_calls_emitted_at:"));
        assert!(summary.contains("tool_status_summary: requested=1, completed=1, failed=1"));
    }

    #[test]
    fn render_dataset_output_runtime_summary_includes_tool_status_summary() {
        let now = Utc::now();
        let inspect = WorkflowRuntimeInspectView {
            execution: WorkflowExecutionView {
                id: WorkflowExecutionId::new(),
                kind: domain_model::WorkflowKind::DatasetOutput,
                status: WorkflowStatus::Succeeded,
                stage: "completed".to_string(),
                updated_at: now,
            },
            execution_scope_runtime: None,
            dataset_output: Some(contracts::DatasetOutputView {
                id: DatasetOutputId::new(),
                dataset_id: DatasetId::new(),
                execution_id: WorkflowExecutionId::new(),
                prompt: "Summarize".to_string(),
                output_text: "Summary".to_string(),
                memory_directory_id: None,
                memory_directory: None,
                retrieval_evidence_ids: vec![
                    RetrievalEvidenceId::new(),
                    RetrievalEvidenceId::new(),
                ],
                retrieval_evidences: Vec::new(),
                llm_invocations: vec![contracts::LlmInvocationView {
                    id: LlmInvocationId::new(),
                    execution_id: WorkflowExecutionId::new(),
                    source_kind: contracts::LlmInvocationSourceKindView::DatasetOutput,
                    dataset_output_id: None,
                    chat_message_id: None,
                    sequence_no: 1,
                    mode: contracts::LlmInvocationModeView::Provider,
                    provider: Some("openai".to_string()),
                    model: Some("gpt-5.4".to_string()),
                    request_id: Some("req_dataset_output".to_string()),
                    finish_reason: Some(contracts::LlmInvocationFinishReasonView::Stop),
                    latency_ms: Some(50),
                    usage: Some(contracts::LlmTokenUsageView {
                        input_tokens: 10,
                        output_tokens: 20,
                        total_tokens: 30,
                    }),
                    system_prompt_key: None,
                    system_prompt_version: None,
                    tool_trace_count: Some(2),
                    created_at: now,
                }],
                tool_executions: vec![
                    contracts::ToolExecutionView {
                        id: ToolExecutionId::new(),
                        execution_id: WorkflowExecutionId::new(),
                        source_kind: contracts::ToolExecutionSourceKindView::DatasetOutput,
                        dataset_output_id: None,
                        chat_message_id: None,
                        sequence_no: 1,
                        call_id: Some("call_requested".to_string()),
                        tool_name: "weather.prepare".to_string(),
                        tool: None,
                        status: contracts::ManifestToolCallStatusView::Requested,
                        arguments: None,
                        result: None,
                        created_at: now,
                    },
                    contracts::ToolExecutionView {
                        id: ToolExecutionId::new(),
                        execution_id: WorkflowExecutionId::new(),
                        source_kind: contracts::ToolExecutionSourceKindView::DatasetOutput,
                        dataset_output_id: None,
                        chat_message_id: None,
                        sequence_no: 2,
                        call_id: Some("call_completed".to_string()),
                        tool_name: "weather.lookup".to_string(),
                        tool: None,
                        status: contracts::ManifestToolCallStatusView::Completed,
                        arguments: None,
                        result: None,
                        created_at: now,
                    },
                    contracts::ToolExecutionView {
                        id: ToolExecutionId::new(),
                        execution_id: WorkflowExecutionId::new(),
                        source_kind: contracts::ToolExecutionSourceKindView::DatasetOutput,
                        dataset_output_id: None,
                        chat_message_id: None,
                        sequence_no: 3,
                        call_id: Some("call_failed".to_string()),
                        tool_name: "weather.publish".to_string(),
                        tool: None,
                        status: contracts::ManifestToolCallStatusView::Failed,
                        arguments: None,
                        result: None,
                        created_at: now,
                    },
                ],
                output_manifest: json!({}),
                output_manifest_view: Some(contracts::DatasetOutputManifestView {
                    generator: "dataset-output-worker".to_string(),
                    schema_version: "0.5.0".to_string(),
                    dataset_id: DatasetId::new(),
                    prompt: "Summarize".to_string(),
                    indexed_document_count: 1,
                    refreshed_chunks: 0,
                    memory_directory_id: None,
                    memory_directory_version_no: None,
                    retrieval_evidence_count: 2,
                    retrieval_evidence_ids: Vec::new(),
                    output: Some(contracts::DatasetOutputContentView {
                        format: contracts::DatasetOutputFormatView::Markdown,
                        sections: vec![contracts::DatasetOutputSectionView {
                            section_key: "summary".to_string(),
                            kind: contracts::DatasetOutputSectionKindView::Summary,
                            title: "Summary".to_string(),
                            content: "Summary".to_string(),
                            retrieval_evidence_ids: Vec::new(),
                        }],
                    }),
                    service_handoff: None,
                    tool_trace: Vec::new(),
                    context_binding: contracts::ManifestContextBindingView::CreationTime,
                    runtime: Some(contracts::ManifestRuntimeView {
                        mode: contracts::ManifestRuntimeModeView::Provider,
                        provider: Some("openai".to_string()),
                        model: Some("gpt-5.4".to_string()),
                        request_id: Some("req_dataset_output".to_string()),
                        finish_reason: Some(contracts::ManifestFinishReasonView::Stop),
                        provider_failure: Some(contracts::ManifestProviderFailureView {
                            kind: contracts::ManifestProviderFailureKindView::HttpStatus,
                            message: "openai returned HTTP 502".to_string(),
                        }),
                        latency_ms: Some(50),
                        usage: Some(contracts::ManifestTokenUsageView {
                            input_tokens: 10,
                            output_tokens: 20,
                            total_tokens: 30,
                        }),
                        system_prompt_key: None,
                        system_prompt_version: None,
                        tool_trace_count: Some(2),
                    }),
                }),
                model_facing: None,
                created_at: now,
            }),
            chat_session: None,
            report_plan: None,
            report_render_output: None,
            chat_messages: Vec::new(),
            llm_invocations: Vec::new(),
            tool_executions: Vec::new(),
            model_facing: None,
            pretty_summaries: Vec::new(),
        };

        let summary = render_dataset_output_runtime_summary(&inspect)
            .expect("dataset output summary should render");

        assert!(summary.contains("Dataset Output Runtime"));
        assert!(summary.contains("provider: openai"));
        assert!(summary.contains("model: gpt-5.4"));
        assert!(summary.contains("request_id: req_dataset_output"));
        assert!(summary.contains("finish_reason: stop"));
        assert!(summary.contains("provider_failure_kind: http_status"));
        assert!(summary.contains("provider_failure_message: openai returned HTTP 502"));
        assert!(summary.contains("token_usage: input=10, output=20, total=30"));
        assert!(summary.contains("tool_status_summary: requested=1, completed=1, failed=1"));
    }

    #[test]
    fn render_workflow_runtime_pretty_summaries_include_report_blocks() {
        let now = Utc::now();
        let report_plan_id = ReportPlanId::new();
        let inspect = WorkflowRuntimeInspectView {
            execution: WorkflowExecutionView {
                id: WorkflowExecutionId::new(),
                kind: domain_model::WorkflowKind::ReportRender,
                status: WorkflowStatus::Succeeded,
                stage: "rendered".to_string(),
                updated_at: now,
            },
            execution_scope_runtime: None,
            dataset_output: None,
            chat_session: None,
            report_plan: Some(ReportPlanSummary {
                id: report_plan_id,
                dataset_id: DatasetId::new(),
                title: "Quarterly Report".to_string(),
                objective: "Summarize product and market signals".to_string(),
                status: contracts::ReportPlanStatusView::Rendered,
                theme_key: "executive-default".to_string(),
                current_ast_version_id: Some(ReportPlanAstVersionId::new()),
                service_handoff: Some(contracts::ManifestServiceHandoffView {
                    source: contracts::ManifestServiceHandoffSourceView::ChatSessionReportEntry,
                    service_lane: contracts::ModelFacingServiceLaneView::ReportService,
                    report_entry_state: contracts::ModelFacingReportEntryStateView::Confirmed,
                    requested_at: Some(now),
                    resolved_at: Some(now),
                    resolved_action: Some(
                        contracts::ChatSessionReportEntryResolutionView::EnterReportService,
                    ),
                    suggested_title: Some("Quarterly Report".to_string()),
                    suggested_objective: Some("Summarize product and market signals".to_string()),
                    confirmed_report_plan_id: Some(report_plan_id),
                }),
                model_facing: None,
            }),
            report_render_output: Some(ReportRenderOutputView {
                id: ReportRenderOutputId::new(),
                execution_id: WorkflowExecutionId::new(),
                plan_id: report_plan_id,
                dataset_id: DatasetId::new(),
                ast_version_id: ReportPlanAstVersionId::new(),
                surface: PublishedSurface::Pc,
                status: contracts::ReportRenderOutputStatusView::Rendered,
                asset_manifest: json!({
                    "kind": "placeholder_asset",
                    "path": "reports/demo/pc.html"
                }),
                service_handoff: Some(contracts::ManifestServiceHandoffView {
                    source: contracts::ManifestServiceHandoffSourceView::ChatSessionReportEntry,
                    service_lane: contracts::ModelFacingServiceLaneView::ReportService,
                    report_entry_state: contracts::ModelFacingReportEntryStateView::Confirmed,
                    requested_at: Some(now),
                    resolved_at: Some(now),
                    resolved_action: Some(
                        contracts::ChatSessionReportEntryResolutionView::EnterReportService,
                    ),
                    suggested_title: Some("Quarterly Report".to_string()),
                    suggested_objective: Some("Summarize product and market signals".to_string()),
                    confirmed_report_plan_id: Some(report_plan_id),
                }),
                model_facing: None,
                created_at: now,
            }),
            chat_messages: Vec::new(),
            llm_invocations: Vec::new(),
            tool_executions: Vec::new(),
            model_facing: Some(contracts::WorkflowModelFacingSummaryView {
                capability_class:
                    contracts::ModelFacingCapabilityClassView::ReportGenerationAndEditing,
                service_lane: contracts::ModelFacingServiceLaneView::ReportService,
                report_entry_state: contracts::ModelFacingReportEntryStateView::Confirmed,
                evidence_state: contracts::ModelFacingEvidenceStateView::Mixed,
                continuation_state:
                    contracts::ModelFacingContinuationStateView::NeedsPlatformContinuation,
                recommended_next_action: Some(contracts::ModelFacingNextActionView::AnswerDirectly),
                allowed_next_actions: vec![contracts::ModelFacingNextActionView::AnswerDirectly],
                recommended_tool_key: None,
                allowed_tool_keys: Vec::new(),
                signals: vec!["workflow_kind=report_render_workflow".to_string()],
            }),
            pretty_summaries: Vec::new(),
        };

        let summaries = render_workflow_runtime_pretty_summaries(&inspect);

        assert!(summaries
            .iter()
            .any(|summary| summary.starts_with("Report Plan Runtime")));
        assert!(summaries
            .iter()
            .any(|summary| summary.starts_with("Report Render Runtime")));
    }

    #[test]
    fn render_workflow_runtime_pretty_summaries_returns_expected_order() {
        let now = Utc::now();
        let inspect = WorkflowRuntimeInspectView {
            execution: WorkflowExecutionView {
                id: WorkflowExecutionId::new(),
                kind: domain_model::WorkflowKind::ChatSession,
                status: WorkflowStatus::Succeeded,
                stage: "completed".to_string(),
                updated_at: now,
            },
            execution_scope_runtime: Some(contracts::WorkflowExecutionRuntimeSummaryView {
                llm_invocation_count: 1,
                tool_execution_count: 2,
                failed_tool_execution_count: 1,
                latest_provider: Some("openai".to_string()),
                latest_model: Some("gpt-5.4".to_string()),
                latest_request_id: Some("req_execution".to_string()),
                latest_finish_reason: Some(contracts::LlmInvocationFinishReasonView::Stop),
                latest_tool_trace_count: Some(1),
            }),
            dataset_output: Some(contracts::DatasetOutputView {
                id: DatasetOutputId::new(),
                dataset_id: DatasetId::new(),
                execution_id: WorkflowExecutionId::new(),
                prompt: "Summarize".to_string(),
                output_text: "Summary".to_string(),
                memory_directory_id: None,
                memory_directory: None,
                retrieval_evidence_ids: Vec::new(),
                retrieval_evidences: Vec::new(),
                llm_invocations: Vec::new(),
                tool_executions: Vec::new(),
                output_manifest: json!({}),
                output_manifest_view: Some(contracts::DatasetOutputManifestView {
                    generator: "dataset-output-worker".to_string(),
                    schema_version: "0.5.0".to_string(),
                    dataset_id: DatasetId::new(),
                    prompt: "Summarize".to_string(),
                    indexed_document_count: 0,
                    refreshed_chunks: 0,
                    memory_directory_id: None,
                    memory_directory_version_no: None,
                    retrieval_evidence_count: 0,
                    retrieval_evidence_ids: Vec::new(),
                    output: None,
                    service_handoff: None,
                    tool_trace: Vec::new(),
                    context_binding: contracts::ManifestContextBindingView::CreationTime,
                    runtime: Some(contracts::ManifestRuntimeView {
                        mode: contracts::ManifestRuntimeModeView::Provider,
                        provider: Some("openai".to_string()),
                        model: Some("gpt-5.4".to_string()),
                        request_id: Some("req_dataset".to_string()),
                        finish_reason: Some(contracts::ManifestFinishReasonView::Stop),
                        provider_failure: None,
                        latency_ms: Some(10),
                        usage: None,
                        system_prompt_key: None,
                        system_prompt_version: None,
                        tool_trace_count: Some(0),
                    }),
                }),
                model_facing: None,
                created_at: now,
            }),
            chat_session: Some(ChatSessionView {
                id: domain_model::ChatSessionId::new(),
                dataset_id: DatasetId::new(),
                execution_id: WorkflowExecutionId::new(),
                title: "Chat".to_string(),
                latest_memory_directory_id: None,
                latest_memory_directory: None,
                latest_dataset_output_id: None,
                latest_dataset_output: None,
                latest_assistant_message_id: Some(ChatMessageId::new()),
                latest_assistant_message: Some(ChatMessageView {
                    id: ChatMessageId::new(),
                    session_id: domain_model::ChatSessionId::new(),
                    role: domain_model::ChatMessageRole::Assistant,
                    turn_index: 1,
                    content: "Answer".to_string(),
                    llm_invocations: Vec::new(),
                    tool_executions: Vec::new(),
                    message_manifest: json!({}),
                    message_manifest_view: Some(contracts::ChatMessageManifestView {
                        generator: "chat-session-worker".to_string(),
                        schema_version: "0.5.0".to_string(),
                        dataset_id: DatasetId::new(),
                        prompt: "Prompt".to_string(),
                        indexed_document_count: 0,
                        refreshed_chunks: 0,
                        prior_message_count: 0,
                        latest_memory_directory_id: None,
                        latest_memory_directory_version_no: None,
                        latest_dataset_output_id: None,
                        output: None,
                        service_handoff: None,
                        tool_trace: Vec::new(),
                        turn: Some(contracts::ChatTurnRuntimeView {
                            turn_id: "turn_1".to_string(),
                            status: contracts::ChatTurnStatusView::Completed,
                            stream_mode: contracts::ChatTurnStreamModeView::Buffered,
                            stream_status: contracts::ChatTurnStreamStatusView::NotRequested,
                            provider_status: contracts::ChatTurnProviderStatusView::Responded,
                            provider_failure: None,
                            artifact_commit_status:
                                contracts::ChatTurnArtifactCommitStatusView::Completed,
                            tool_loop_status: contracts::ChatTurnToolLoopStatusView::NotRequested,
                            provider_request_id: None,
                            provider_requested_at: Some(now),
                            provider_responded_at: Some(now),
                            artifact_commit_ready_at: Some(now),
                            artifact_commit_failure_source: None,
                            first_token_at: None,
                            stream_completed_at: None,
                            tool_calls_emitted_at: None,
                            tool_loop_settled_at: None,
                            finish_reason: Some(contracts::ManifestFinishReasonView::Stop),
                            assistant_message_id: None,
                            assistant_message_persisted_at: Some(now),
                            tool_trace_count: 0,
                            tool_status_summary: None,
                            events: Vec::new(),
                            started_at: now,
                            completed_at: Some(now),
                        }),
                        context_binding: contracts::ManifestContextBindingView::CreationTime,
                        runtime: None,
                    }),
                    model_facing: None,
                    created_at: now,
                }),
                session_manifest: json!({}),
                session_manifest_view: None,
                model_facing: None,
                created_at: now,
                updated_at: now,
            }),
            report_plan: None,
            report_render_output: None,
            chat_messages: Vec::new(),
            llm_invocations: Vec::new(),
            tool_executions: Vec::new(),
            model_facing: Some(contracts::WorkflowModelFacingSummaryView {
                capability_class:
                    contracts::ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis,
                service_lane: contracts::ModelFacingServiceLaneView::MaterialService,
                report_entry_state: contracts::ModelFacingReportEntryStateView::NotApplicable,
                evidence_state: contracts::ModelFacingEvidenceStateView::CatalogMemory,
                continuation_state: contracts::ModelFacingContinuationStateView::ReadyToAnswer,
                recommended_next_action: Some(contracts::ModelFacingNextActionView::AnswerDirectly),
                allowed_next_actions: vec![contracts::ModelFacingNextActionView::AnswerDirectly],
                recommended_tool_key: None,
                allowed_tool_keys: Vec::new(),
                signals: vec!["workflow_kind=chat_session_workflow".to_string()],
            }),
            pretty_summaries: Vec::new(),
        };

        let summaries = render_workflow_runtime_pretty_summaries(&inspect);

        assert_eq!(summaries.len(), 4);
        assert!(summaries[0].starts_with("Model-facing Runtime"));
        assert!(summaries[0].contains("service_lane"));
        assert!(summaries[0].contains("report_entry_state"));
        assert!(summaries[0].contains("continuation_state"));
        assert!(summaries[0].contains("recommended_next_action"));
        assert!(summaries[1].starts_with("Execution-scope Runtime"));
        assert!(summaries[2].starts_with("Dataset Output Runtime"));
        assert!(summaries[3].starts_with("Latest Assistant Turn"));
    }

    #[test]
    fn build_model_facing_summary_marks_runtime_waits_before_more_actions() {
        let summary = build_model_facing_summary(
            contracts::ModelFacingCapabilityClassView::MaterialExplanationAndSynthesis,
            contracts::ModelFacingEvidenceStateView::Mixed,
            vec![
                contracts::ModelFacingNextActionView::FinalizeArtifactCommit,
                contracts::ModelFacingNextActionView::WaitForToolLoop,
                contracts::ModelFacingNextActionView::AnswerDirectly,
            ],
            vec!["workflow_kind=chat_session".to_string()],
        );

        assert_eq!(
            summary.continuation_state,
            contracts::ModelFacingContinuationStateView::WaitingForRuntime
        );
        assert_eq!(
            summary.recommended_next_action,
            Some(contracts::ModelFacingNextActionView::WaitForToolLoop)
        );
    }

    #[test]
    fn to_tool_execution_view_supports_workflow_execution_source_kind() {
        let tool_execution = ToolExecution {
            id: ToolExecutionId::new(),
            tenant_id: TenantId::new(),
            execution_id: WorkflowExecutionId::new(),
            source_kind: ToolExecutionSourceKind::WorkflowExecution,
            dataset_output_id: None,
            chat_message_id: None,
            sequence_no: 0,
            call_id: Some("call_execution_scope".to_string()),
            tool_name: "weather.lookup".to_string(),
            tool_snapshot: None,
            status: ToolExecutionStatus::Failed,
            arguments: Some(json!({ "city": "Shanghai" })),
            result: Some(json!({ "error": "timeout" })),
            created_at: Utc::now(),
        };

        let view = to_tool_execution_view(tool_execution);

        assert_eq!(
            view.source_kind,
            contracts::ToolExecutionSourceKindView::WorkflowExecution
        );
        assert_eq!(view.dataset_output_id, None);
        assert_eq!(view.chat_message_id, None);
        assert_eq!(view.status, contracts::ManifestToolCallStatusView::Failed);
    }
}
