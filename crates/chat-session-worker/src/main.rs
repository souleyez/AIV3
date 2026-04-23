use anyhow::{anyhow, Result};
use chat_session_worker::{
    chat_runtime_error_message, finalize_assistant_message_manifest,
    render_assistant_message_manifest, render_completed_session_manifest,
    render_failed_session_manifest, render_in_flight_session_manifest,
    render_response_ready_session_manifest, ChatSessionJob, ChatSessionOrchestrator,
    PlaceholderChatSessionOrchestrator,
};
use chrono::Utc;
use domain_model::{
    ChatMessage, ChatMessageRole, ChatSessionId, DatasetOutputId, DocumentLifecycle,
    MemoryDirectoryId, WorkflowEventRecord, WorkflowStatus,
};
use event_bus::{workflow_task_enqueued_subject, EventBus, EventSubscription};
use llm_gateway::{
    build_provider_from_env, render_runtime_manifest, LlmFinishReason, LlmRuntimeMetadata,
    LlmRuntimeMode, LlmTokenUsage, LlmToolCall, LlmToolCallStatus,
};
use prompt_registry::bootstrap_default_prompt_registry;
use storage::{
    LlmInvocationRecordInput, NewChatMessage, PgStorage, ToolExecutionRecordInput,
    DEFAULT_LOCAL_DATABASE_URL,
};
use tokio::time::Duration;
use tool_registry::find_default_tool_snapshot_value;
use uuid::Uuid;
use workflow_engine::{WorkflowCatalog, WorkflowSignal};

const DEFAULT_QUEUE: &str = "chat_session";
const DEFAULT_TASK_KEY: &str = "orchestrate_chat_session";
const DEFAULT_POLL_INTERVAL_MS: u64 = 1_000;
const DEFAULT_RUNTIME_MODE: &str = "placeholder";
const DEFAULT_RUNTIME_PROVIDER: &str = "placeholder";
const DEFAULT_RUNTIME_MODEL: &str = "placeholder-chat-session-v1";
const DEFAULT_RUNTIME_STREAM_MODE: &str = "buffered";
const CHAT_TURN_RECOVERY_PAYLOAD_KEY: &str = "_chat_turn_recovery";
const CHAT_TURN_RECOVERY_EVENT_NAME: &str = "chat_turn_recovery_checkpoint";

#[derive(Debug)]
struct ChatSessionTaskError {
    source: anyhow::Error,
    failed_runtime: Option<LlmRuntimeMetadata>,
    failed_tool_calls: Vec<LlmToolCall>,
    failed_assistant_message_content: Option<String>,
    artifact_commit_failure_source: Option<ArtifactCommitFailureSource>,
    preserve_successful_turn: bool,
}

#[derive(Clone, Copy, Debug)]
enum ArtifactCommitFailureSource {
    WorkflowEventRecoveryPersist,
    ResponseReadySessionUpdate,
    AssistantMessageCreate,
    AssistantMessageManifestUpdate,
    LlmInvocationPersist,
    ToolExecutionPersist,
    SessionContextUpdate,
}

impl ArtifactCommitFailureSource {
    fn as_manifest_str(self) -> &'static str {
        match self {
            Self::WorkflowEventRecoveryPersist => "workflow_event_recovery_persist",
            Self::ResponseReadySessionUpdate => "response_ready_session_update",
            Self::AssistantMessageCreate => "assistant_message_create",
            Self::AssistantMessageManifestUpdate => "assistant_message_manifest_update",
            Self::LlmInvocationPersist => "llm_invocation_persist",
            Self::ToolExecutionPersist => "tool_execution_persist",
            Self::SessionContextUpdate => "session_context_update",
        }
    }
}

#[derive(Clone, Debug)]
struct RecoveredAssistantArtifact {
    message: ChatMessage,
    runtime: LlmRuntimeMetadata,
    tool_calls: Vec<LlmToolCall>,
}

#[derive(Clone, Debug)]
struct RecoveredPendingAssistantTurn {
    assistant_message_content: String,
    runtime: LlmRuntimeMetadata,
    tool_calls: Vec<LlmToolCall>,
    captured_at: chrono::DateTime<Utc>,
}

impl ChatSessionTaskError {
    fn artifact_commit(
        source: anyhow::Error,
        failed_runtime: LlmRuntimeMetadata,
        failed_tool_calls: Vec<LlmToolCall>,
        failed_assistant_message_content: Option<String>,
        artifact_commit_failure_source: ArtifactCommitFailureSource,
    ) -> Self {
        Self {
            source,
            failed_runtime: Some(failed_runtime),
            failed_tool_calls,
            failed_assistant_message_content,
            artifact_commit_failure_source: Some(artifact_commit_failure_source),
            preserve_successful_turn: false,
        }
    }

    fn post_commit(
        source: anyhow::Error,
        failed_runtime: LlmRuntimeMetadata,
        failed_tool_calls: Vec<LlmToolCall>,
        failed_assistant_message_content: Option<String>,
    ) -> Self {
        Self {
            source,
            failed_runtime: Some(failed_runtime),
            failed_tool_calls,
            failed_assistant_message_content,
            artifact_commit_failure_source: None,
            preserve_successful_turn: true,
        }
    }
}

impl From<anyhow::Error> for ChatSessionTaskError {
    fn from(source: anyhow::Error) -> Self {
        Self {
            source,
            failed_runtime: None,
            failed_tool_calls: Vec::new(),
            failed_assistant_message_content: None,
            artifact_commit_failure_source: None,
            preserve_successful_turn: false,
        }
    }
}

impl From<platform_api::ApiError> for ChatSessionTaskError {
    fn from(source: platform_api::ApiError) -> Self {
        Self {
            source: source.into(),
            failed_runtime: None,
            failed_tool_calls: Vec::new(),
            failed_assistant_message_content: None,
            artifact_commit_failure_source: None,
            preserve_successful_turn: false,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    observability::install("chat_session_worker")?;

    let database_url = std::env::var("PLATFORM_DATABASE_URL")
        .unwrap_or_else(|_| DEFAULT_LOCAL_DATABASE_URL.to_string());
    let queue = std::env::var("CHAT_SESSION_QUEUE").unwrap_or_else(|_| DEFAULT_QUEUE.to_string());
    let task_key =
        std::env::var("CHAT_SESSION_TASK_KEY").unwrap_or_else(|_| DEFAULT_TASK_KEY.to_string());
    let poll_interval = std::env::var("CHAT_SESSION_POLL_INTERVAL_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_POLL_INTERVAL_MS);
    let runtime_mode = std::env::var("CHAT_SESSION_RUNTIME_MODE")
        .unwrap_or_else(|_| DEFAULT_RUNTIME_MODE.to_string());
    let runtime_provider = std::env::var("CHAT_SESSION_RUNTIME_PROVIDER")
        .unwrap_or_else(|_| DEFAULT_RUNTIME_PROVIDER.to_string());
    let runtime_model = std::env::var("CHAT_SESSION_RUNTIME_MODEL")
        .unwrap_or_else(|_| DEFAULT_RUNTIME_MODEL.to_string());
    let runtime_stream_mode = parse_turn_stream_mode(
        &std::env::var("CHAT_SESSION_RUNTIME_STREAM_MODE")
            .unwrap_or_else(|_| DEFAULT_RUNTIME_STREAM_MODE.to_string()),
    )?;

    let storage = PgStorage::connect(&database_url).await?;
    let workflow_catalog = workflow_definitions::catalog();
    let event_bus = EventBus::connect_from_env_or_disabled("PLATFORM_NATS_URL").await;
    let prompt_registry = bootstrap_default_prompt_registry();
    let provider = build_provider_from_env(
        "CHAT_SESSION",
        &runtime_mode,
        runtime_provider,
        prompt_registry,
    )?;
    let orchestrator = PlaceholderChatSessionOrchestrator::new(provider, runtime_model);
    let wake_subject = workflow_task_enqueued_subject(&queue, &task_key);
    let mut task_waker = event_bus
        .subscribe_queue_or_disabled(
            &wake_subject,
            Some(&format!("chat_session_worker.{queue}.{task_key}")),
        )
        .await;

    tracing::info!(
        %queue,
        %task_key,
        %wake_subject,
        event_bus_enabled = event_bus.is_enabled(),
        poll_interval_ms = poll_interval,
        %runtime_mode,
        %runtime_stream_mode,
        %database_url,
        "chat-session-worker polling started"
    );

    loop {
        match storage
            .workflow_tasks()
            .claim_next_available(&queue, Some(&task_key), Utc::now())
            .await
        {
            Ok(Some(task)) => {
                if let Err(error) = process_task(
                    &storage,
                    &workflow_catalog,
                    &event_bus,
                    &orchestrator,
                    &runtime_stream_mode,
                    task,
                )
                .await
                {
                    tracing::error!(error = ?error, "chat session task processing failed");
                }
            }
            Ok(None) => {
                wait_for_next_task_signal(&mut task_waker, poll_interval).await;
            }
            Err(error) => {
                tracing::error!(error = ?error, "chat session worker failed to claim task");
                wait_for_next_task_signal(&mut task_waker, poll_interval).await;
            }
        }
    }
}

async fn process_task(
    storage: &PgStorage,
    workflow_catalog: &WorkflowCatalog,
    event_bus: &EventBus,
    orchestrator: &impl ChatSessionOrchestrator,
    runtime_stream_mode: &str,
    task: domain_model::WorkflowTask,
) -> Result<()> {
    let execution = storage
        .workflow_executions()
        .get_by_id(task.tenant_id, task.execution_id)
        .await?
        .ok_or_else(|| anyhow!("workflow execution {} not found", task.execution_id))?;
    if matches!(execution.status, WorkflowStatus::Succeeded) {
        tracing::info!(
            task_id = %task.id,
            execution_id = %task.execution_id,
            "chat session worker skipping duplicate run for already succeeded execution"
        );
        storage
            .workflow_tasks()
            .mark_succeeded(task.id, Utc::now())
            .await?;
        return Ok(());
    }
    let dataset_id = execution
        .dataset_id
        .ok_or_else(|| anyhow!("workflow execution {} has no dataset id", execution.id))?;
    let prompt = execution
        .context
        .get("prompt")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("workflow execution {} missing prompt", execution.id))?
        .to_string();
    let session = match context_uuid(&execution.context, "chat_session_id").map(ChatSessionId) {
        Some(chat_session_id) => storage
            .chat_sessions()
            .get_by_id(task.tenant_id, chat_session_id)
            .await?
            .ok_or_else(|| anyhow!("chat session {} not found", chat_session_id))?,
        None => storage
            .chat_sessions()
            .get_by_execution_id(task.tenant_id, task.execution_id)
            .await?
            .ok_or_else(|| anyhow!("chat session for execution {} not found", task.execution_id))?,
    };
    let existing_messages = storage
        .chat_messages()
        .list_by_session(task.tenant_id, session.id)
        .await?;
    let indexed_documents = storage
        .documents()
        .list_by_dataset(task.tenant_id, dataset_id)
        .await?
        .into_iter()
        .filter(|document| document.lifecycle == DocumentLifecycle::Indexed)
        .count();
    let bound_memory_directory_id =
        context_uuid(&execution.context, "memory_directory_id").map(MemoryDirectoryId);
    let bound_memory_directory_version_no =
        context_i32(&execution.context, "memory_directory_version_no");
    let bound_dataset_output_id =
        context_uuid(&execution.context, "dataset_output_id").map(DatasetOutputId);
    let turn_id = context_string(&execution.context, "chat_turn_id")
        .ok_or_else(|| anyhow!("workflow execution {} missing chat_turn_id", execution.id))?;
    let latest_memory_directory = match bound_memory_directory_id {
        Some(memory_directory_id) => {
            storage
                .memory_directories()
                .get_by_id(task.tenant_id, memory_directory_id)
                .await?
        }
        None => storage
            .memory_directories()
            .list_by_dataset(task.tenant_id, dataset_id)
            .await?
            .into_iter()
            .next(),
    };
    let latest_dataset_output = match bound_dataset_output_id {
        Some(dataset_output_id) => {
            storage
                .dataset_outputs()
                .get_by_id(task.tenant_id, dataset_output_id)
                .await?
        }
        None => storage
            .dataset_outputs()
            .list_by_dataset(task.tenant_id, dataset_id)
            .await?
            .into_iter()
            .next(),
    };
    let job = ChatSessionJob {
        dataset_id,
        initial_prompt: session
            .session_manifest
            .get("initial_prompt")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| prompt.clone()),
        prompt: prompt.clone(),
        indexed_document_count: indexed_documents,
        refreshed_chunks: latest_memory_directory
            .as_ref()
            .map(|directory| directory.refreshed_chunks as usize)
            .unwrap_or(0),
        prior_message_count: existing_messages.len(),
        latest_memory_directory_id: latest_memory_directory.as_ref().map(|entry| entry.id),
        latest_memory_directory_version_no: latest_memory_directory
            .as_ref()
            .map(|entry| entry.version_no)
            .or(bound_memory_directory_version_no),
        latest_dataset_output_id: latest_dataset_output.as_ref().map(|entry| entry.id),
        latest_dataset_output_retrieval_evidence_ids: latest_dataset_output
            .as_ref()
            .map(|entry| entry.retrieval_evidence_ids.clone())
            .unwrap_or_default(),
        service_handoff: service_handoff_from_session_manifest(&session.session_manifest),
        turn_stream_mode: runtime_stream_mode.to_string(),
        turn_id,
        turn_started_at: execution.created_at,
    };
    let requested_at = Utc::now();

    let process_result: std::result::Result<(), ChatSessionTaskError> = async {
        let recovered_assistant =
            find_recoverable_assistant_message(&existing_messages, &job.turn_id)
                .map_err(ChatSessionTaskError::from)?;
        let recovered_pending_turn = if recovered_assistant.is_none() {
            if let Some(recovered) =
                find_recoverable_session_turn(&session.session_manifest, &job.turn_id)
                    .map_err(ChatSessionTaskError::from)?
            {
                Some(recovered)
            } else {
                if let Some(recovered) = find_recoverable_task_turn(&task.payload, &job.turn_id)
                    .map_err(ChatSessionTaskError::from)?
                {
                    Some(recovered)
                } else {
                    let recovery_events = storage
                        .workflow_events()
                        .list_by_execution(task.execution_id)
                        .await
                        .map_err(ChatSessionTaskError::from)?;
                    find_recoverable_event_turn(&recovery_events, &job.turn_id)
                        .map_err(ChatSessionTaskError::from)?
                }
            }
        } else {
            None
        };
        if recovered_assistant.is_none() && recovered_pending_turn.is_none() {
            let in_flight_session_manifest = render_in_flight_session_manifest(&job, requested_at);
            storage
                .chat_sessions()
                .update_context(
                    task.tenant_id,
                    session.id,
                    latest_memory_directory.as_ref().map(|entry| entry.id),
                    latest_dataset_output.as_ref().map(|entry| entry.id),
                    &in_flight_session_manifest,
                    requested_at,
                )
                .await?;
        }
        let (assistant_message, runtime, tool_calls, session_manifest) =
            if let Some(recovered) = recovered_assistant {
                let RecoveredAssistantArtifact {
                    message,
                    runtime,
                    tool_calls,
                } = recovered;
                tracing::info!(
                    task_id = %task.id,
                    execution_id = %task.execution_id,
                    assistant_message_id = %message.id,
                    "chat session worker resuming artifact commit from existing assistant message"
                );
                (
                    resume_assistant_message_manifest(
                        storage,
                        task.tenant_id,
                        RecoveredAssistantArtifact {
                            message,
                            runtime: runtime.clone(),
                            tool_calls: tool_calls.clone(),
                        },
                    )
                    .await
                    .map_err(ChatSessionTaskError::from)?,
                    runtime,
                    tool_calls,
                    render_completed_session_manifest(&job),
                )
            } else if let Some(recovered) = recovered_pending_turn {
                tracing::info!(
                    task_id = %task.id,
                    execution_id = %task.execution_id,
                    turn_id = %job.turn_id,
                    "chat session worker recreating assistant artifact from recoverable session manifest"
                );
                let message_manifest = render_assistant_message_manifest(
                    &job,
                    &recovered.assistant_message_content,
                    &recovered.runtime,
                    &recovered.tool_calls,
                    requested_at,
                    recovered.captured_at,
                );
                let assistant_message = storage
                    .chat_messages()
                    .create(
                        task.tenant_id,
                        &NewChatMessage {
                            session_id: session.id,
                            role: domain_model::ChatMessageRole::Assistant,
                            turn_index: existing_messages.len() as i32,
                            content: recovered.assistant_message_content.clone(),
                            message_manifest,
                            created_at: recovered.captured_at,
                        },
                    )
                    .await
                    .map_err(|error| {
                        ChatSessionTaskError::artifact_commit(
                            error.into(),
                            recovered.runtime.clone(),
                            recovered.tool_calls.clone(),
                            Some(recovered.assistant_message_content.clone()),
                            ArtifactCommitFailureSource::AssistantMessageCreate,
                        )
                    })?;
                let assistant_message_manifest = finalize_assistant_message_manifest(
                    &assistant_message.message_manifest,
                    assistant_message.id,
                    Utc::now(),
                );
                let assistant_message = storage
                    .chat_messages()
                    .update_manifest(
                        task.tenant_id,
                        assistant_message.id,
                        &assistant_message_manifest,
                    )
                    .await
                    .map_err(|error| {
                        ChatSessionTaskError::artifact_commit(
                            error.into(),
                            recovered.runtime.clone(),
                            recovered.tool_calls.clone(),
                            Some(recovered.assistant_message_content.clone()),
                            ArtifactCommitFailureSource::AssistantMessageManifestUpdate,
                        )
                    })?;
                (
                    assistant_message,
                    recovered.runtime,
                    recovered.tool_calls,
                    render_completed_session_manifest(&job),
                )
            } else {
                let outcome = orchestrator.generate(&job, requested_at)?;
                if let Some(error_message) = chat_runtime_error_message(&outcome.runtime) {
                    return Err(ChatSessionTaskError {
                        source: anyhow!(error_message),
                        failed_runtime: Some(outcome.runtime.clone()),
                        failed_tool_calls: outcome.tool_calls.clone(),
                        failed_assistant_message_content: Some(outcome.assistant_message.clone()),
                        artifact_commit_failure_source: None,
                        preserve_successful_turn: false,
                    });
                }
                let response_ready_at = Utc::now();
                if let Err(error) = storage
                    .workflow_tasks()
                    .update_payload(
                        task.id,
                        &payload_with_turn_recovery(
                            &task.payload,
                            &job.turn_id,
                            &outcome.assistant_message,
                            &outcome.runtime,
                            &outcome.tool_calls,
                            response_ready_at,
                        ),
                        response_ready_at,
                    )
                    .await
                {
                    tracing::warn!(
                        error = ?error,
                        task_id = %task.id,
                        execution_id = %task.execution_id,
                        "chat session worker failed to persist task recovery checkpoint"
                    );
                }
                storage
                    .workflow_events()
                    .create(
                        task.execution_id,
                        CHAT_TURN_RECOVERY_EVENT_NAME,
                        &render_turn_recovery_payload(
                            &job.turn_id,
                            &outcome.assistant_message,
                            &outcome.runtime,
                            &outcome.tool_calls,
                            response_ready_at,
                        ),
                        response_ready_at,
                    )
                    .await
                    .map_err(|error| {
                        ChatSessionTaskError::artifact_commit(
                            error.into(),
                            outcome.runtime.clone(),
                            outcome.tool_calls.clone(),
                            Some(outcome.assistant_message.clone()),
                            ArtifactCommitFailureSource::WorkflowEventRecoveryPersist,
                        )
                    })?;
                let response_ready_session_manifest = render_response_ready_session_manifest(
                    &job,
                    &outcome.assistant_message,
                    &outcome.runtime,
                    &outcome.tool_calls,
                    requested_at,
                    response_ready_at,
                );
                storage
                    .chat_sessions()
                    .update_context(
                        task.tenant_id,
                        session.id,
                        latest_memory_directory.as_ref().map(|entry| entry.id),
                        latest_dataset_output.as_ref().map(|entry| entry.id),
                        &response_ready_session_manifest,
                        response_ready_at,
                    )
                    .await
                    .map_err(|error| {
                        ChatSessionTaskError::artifact_commit(
                            error.into(),
                            outcome.runtime.clone(),
                            outcome.tool_calls.clone(),
                            Some(outcome.assistant_message.clone()),
                            ArtifactCommitFailureSource::ResponseReadySessionUpdate,
                        )
                    })?;
                let session_manifest = outcome.session_manifest.clone();
                let assistant_message = storage
                    .chat_messages()
                    .create(
                        task.tenant_id,
                        &NewChatMessage {
                            session_id: session.id,
                            role: domain_model::ChatMessageRole::Assistant,
                            turn_index: existing_messages.len() as i32,
                            content: outcome.assistant_message.clone(),
                            message_manifest: outcome.message_manifest.clone(),
                            created_at: outcome.completed_at,
                        },
                    )
                    .await
                    .map_err(|error| {
                        ChatSessionTaskError::artifact_commit(
                            error.into(),
                            outcome.runtime.clone(),
                            outcome.tool_calls.clone(),
                            Some(outcome.assistant_message.clone()),
                            ArtifactCommitFailureSource::AssistantMessageCreate,
                        )
                    })?;
                let assistant_message_manifest = finalize_assistant_message_manifest(
                    &assistant_message.message_manifest,
                    assistant_message.id,
                    Utc::now(),
                );
                let assistant_message = storage
                    .chat_messages()
                    .update_manifest(
                        task.tenant_id,
                        assistant_message.id,
                        &assistant_message_manifest,
                    )
                    .await
                    .map_err(|error| {
                        ChatSessionTaskError::artifact_commit(
                            error.into(),
                            outcome.runtime.clone(),
                            outcome.tool_calls.clone(),
                            Some(outcome.assistant_message.clone()),
                            ArtifactCommitFailureSource::AssistantMessageManifestUpdate,
                        )
                    })?;
                (
                    assistant_message,
                    outcome.runtime,
                    outcome.tool_calls,
                    session_manifest,
                )
            };
        storage
            .llm_invocations()
            .replace_for_chat_message_records(
                task.tenant_id,
                task.execution_id,
                assistant_message.id,
                &[llm_invocation_record_from_runtime(&runtime)],
                assistant_message.created_at,
            )
            .await
            .map_err(|error| {
                ChatSessionTaskError::artifact_commit(
                    error.into(),
                    runtime.clone(),
                    tool_calls.clone(),
                    Some(assistant_message.content.clone()),
                    ArtifactCommitFailureSource::LlmInvocationPersist,
                )
            })?;
        storage
            .tool_executions()
            .replace_for_chat_message_records(
                task.tenant_id,
                task.execution_id,
                assistant_message.id,
                &tool_execution_records_from_tool_calls(&tool_calls),
                assistant_message.created_at,
            )
            .await
            .map_err(|error| {
                ChatSessionTaskError::artifact_commit(
                    error.into(),
                    runtime.clone(),
                    tool_calls.clone(),
                    Some(assistant_message.content.clone()),
                    ArtifactCommitFailureSource::ToolExecutionPersist,
                )
            })?;
        storage
            .chat_sessions()
            .update_context(
                task.tenant_id,
                session.id,
                latest_memory_directory.as_ref().map(|entry| entry.id),
                latest_dataset_output.as_ref().map(|entry| entry.id),
                &session_manifest,
                assistant_message.created_at,
            )
            .await
            .map_err(|error| {
                ChatSessionTaskError::artifact_commit(
                    error.into(),
                    runtime.clone(),
                    tool_calls.clone(),
                    Some(assistant_message.content.clone()),
                    ArtifactCommitFailureSource::SessionContextUpdate,
                )
            })?;
        let signal_output = serde_json::json!({
            "dataset_id": dataset_id,
            "chat_session_id": session.id,
            "assistant_message_id": assistant_message.id,
            "latest_memory_directory_id": latest_memory_directory.as_ref().map(|entry| entry.id),
            "latest_memory_directory_version_no": latest_memory_directory
                .as_ref()
                .map(|entry| entry.version_no)
                .or(bound_memory_directory_version_no),
            "latest_dataset_output_id": latest_dataset_output.as_ref().map(|entry| entry.id),
        });

        platform_api::apply_workflow_signal_with_dependencies(
            storage,
            workflow_catalog,
            event_bus,
            task.tenant_id,
            task.execution_id,
            WorkflowSignal::StepCompleted {
                task_key: task.task_key.clone(),
                output: Some(signal_output),
            },
        )
        .await
        .map_err(|error| {
            ChatSessionTaskError::post_commit(
                error.into(),
                runtime.clone(),
                tool_calls.clone(),
                Some(assistant_message.content.clone()),
            )
        })?;

        Ok(())
    }
    .await;

    if let Err(error) = process_result {
        let error_message = error.source.to_string();
        let failed_at = Utc::now();
        let failed_session_manifest = (!error.preserve_successful_turn).then(|| {
            render_failed_session_manifest(
                &job,
                requested_at,
                failed_at,
                error.failed_assistant_message_content.as_deref(),
                error.failed_runtime.as_ref(),
                &error.failed_tool_calls,
                error
                    .artifact_commit_failure_source
                    .map(ArtifactCommitFailureSource::as_manifest_str),
            )
        });
        if let Some(runtime) = error.failed_runtime.as_ref() {
            if let Err(record_error) = storage
                .llm_invocations()
                .replace_for_execution_records(
                    task.tenant_id,
                    task.execution_id,
                    &[llm_invocation_record_from_runtime(runtime)],
                    failed_at,
                )
                .await
            {
                tracing::error!(
                    error = ?record_error,
                    task_id = %task.id,
                    "chat session worker failed to persist execution-scoped llm invocation"
                );
            }
        }
        if !error.failed_tool_calls.is_empty() {
            if let Err(record_error) = storage
                .tool_executions()
                .replace_for_execution_records(
                    task.tenant_id,
                    task.execution_id,
                    &tool_execution_records_from_tool_calls(&error.failed_tool_calls),
                    failed_at,
                )
                .await
            {
                tracing::error!(
                    error = ?record_error,
                    task_id = %task.id,
                    "chat session worker failed to persist execution-scoped tool executions"
                );
            }
        }
        if let Some(failed_session_manifest) = failed_session_manifest.as_ref() {
            if let Err(update_error) = storage
                .chat_sessions()
                .update_context(
                    task.tenant_id,
                    session.id,
                    latest_memory_directory.as_ref().map(|entry| entry.id),
                    latest_dataset_output.as_ref().map(|entry| entry.id),
                    failed_session_manifest,
                    failed_at,
                )
                .await
            {
                tracing::error!(
                    error = ?update_error,
                    task_id = %task.id,
                    "chat session worker failed to persist failed session manifest"
                );
            }
        } else if error.preserve_successful_turn {
            tracing::warn!(
                task_id = %task.id,
                session_id = %session.id,
                "chat session worker skipped failed session manifest overwrite after post-commit error"
            );
        }
        if let Err(signal_error) = platform_api::apply_workflow_signal_with_dependencies(
            storage,
            workflow_catalog,
            event_bus,
            task.tenant_id,
            task.execution_id,
            WorkflowSignal::StepFailed {
                task_key: task.task_key.clone(),
                error: error_message.clone(),
            },
        )
        .await
        {
            tracing::error!(
                error = ?signal_error,
                task_id = %task.id,
                "chat session worker failed to send workflow step_failed signal"
            );
        }

        storage
            .workflow_tasks()
            .mark_failed(task.id, &error_message, Utc::now())
            .await?;
        return Err(error.source);
    }

    if let Err(error) = storage
        .workflow_tasks()
        .update_payload(
            task.id,
            &payload_without_turn_recovery(&task.payload),
            Utc::now(),
        )
        .await
    {
        tracing::warn!(
            error = ?error,
            task_id = %task.id,
            execution_id = %task.execution_id,
            "chat session worker failed to clear task recovery checkpoint"
        );
    }

    storage
        .workflow_tasks()
        .mark_succeeded(task.id, Utc::now())
        .await?;

    tracing::info!(
        task_id = %task.id,
        execution_id = %task.execution_id,
        dataset_id = %dataset_id,
        session_id = %session.id,
        "chat session task completed"
    );

    Ok(())
}

async fn wait_for_next_task_signal(task_waker: &mut EventSubscription, poll_interval_ms: u64) {
    if let Some(event) = task_waker
        .wait_for_event(Duration::from_millis(poll_interval_ms))
        .await
    {
        tracing::debug!(subject = %event.subject, "chat session worker received task wake signal");
    }
}

fn context_i32(value: &serde_json::Value, key: &str) -> Option<i32> {
    match value {
        serde_json::Value::Object(map) => map
            .get(key)
            .and_then(serde_json::Value::as_i64)
            .and_then(|value| i32::try_from(value).ok()),
        _ => None,
    }
}

fn context_uuid(value: &serde_json::Value, key: &str) -> Option<Uuid> {
    match value {
        serde_json::Value::Object(map) => map
            .get(key)
            .and_then(serde_json::Value::as_str)
            .and_then(|raw| Uuid::parse_str(raw).ok()),
        _ => None,
    }
}

fn context_string(value: &serde_json::Value, key: &str) -> Option<String> {
    match value {
        serde_json::Value::Object(map) => map
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        _ => None,
    }
}

fn service_handoff_from_session_manifest(
    session_manifest: &serde_json::Value,
) -> Option<contracts::ManifestServiceHandoffView> {
    let report_entry = serde_json::from_value::<contracts::ChatSessionReportEntryView>(
        session_manifest.get("report_entry")?.clone(),
    )
    .ok()?;

    let service_lane = match report_entry.state {
        contracts::ModelFacingReportEntryStateView::Confirmed => {
            contracts::ModelFacingServiceLaneView::ReportService
        }
        contracts::ModelFacingReportEntryStateView::NotApplicable
        | contracts::ModelFacingReportEntryStateView::ConfirmationRequired => {
            contracts::ModelFacingServiceLaneView::MaterialService
        }
    };

    Some(contracts::ManifestServiceHandoffView {
        source: contracts::ManifestServiceHandoffSourceView::ChatSessionReportEntry,
        service_lane,
        report_entry_state: report_entry.state,
        requested_at: report_entry.requested_at,
        resolved_at: report_entry.resolved_at,
        resolved_action: report_entry.resolved_action,
        suggested_title: report_entry.suggested_title,
        suggested_objective: report_entry.suggested_objective,
        confirmed_report_plan_id: report_entry.confirmed_report_plan_id,
    })
}

fn payload_with_turn_recovery(
    payload: &serde_json::Value,
    turn_id: &str,
    assistant_message_content: &str,
    runtime: &LlmRuntimeMetadata,
    tool_calls: &[LlmToolCall],
    captured_at: chrono::DateTime<Utc>,
) -> serde_json::Value {
    let mut payload = payload.as_object().cloned().unwrap_or_default();
    payload.insert(
        CHAT_TURN_RECOVERY_PAYLOAD_KEY.to_string(),
        render_turn_recovery_payload(
            turn_id,
            assistant_message_content,
            runtime,
            tool_calls,
            captured_at,
        ),
    );
    serde_json::Value::Object(payload)
}

fn payload_without_turn_recovery(payload: &serde_json::Value) -> serde_json::Value {
    let Some(mut object) = payload.as_object().cloned() else {
        return payload.clone();
    };
    object.remove(CHAT_TURN_RECOVERY_PAYLOAD_KEY);
    serde_json::Value::Object(object)
}

fn render_turn_recovery_payload(
    turn_id: &str,
    assistant_message_content: &str,
    runtime: &LlmRuntimeMetadata,
    tool_calls: &[LlmToolCall],
    captured_at: chrono::DateTime<Utc>,
) -> serde_json::Value {
    serde_json::json!({
        "turn_id": turn_id,
        "assistant_message_content": assistant_message_content,
        "runtime": render_runtime_manifest(runtime),
        "tool_trace": tool_calls,
        "captured_at": captured_at,
    })
}

fn find_recoverable_assistant_message(
    messages: &[ChatMessage],
    turn_id: &str,
) -> Result<Option<RecoveredAssistantArtifact>> {
    let Some(message) = messages.iter().rev().find(|message| {
        matches!(message.role, ChatMessageRole::Assistant)
            && message
                .message_manifest
                .get("turn")
                .and_then(serde_json::Value::as_object)
                .and_then(|turn| turn.get("turn_id"))
                .and_then(serde_json::Value::as_str)
                == Some(turn_id)
    }) else {
        return Ok(None);
    };

    let runtime = parse_runtime_from_message_manifest(&message.message_manifest)?;
    let tool_calls = parse_tool_calls_from_message_manifest(&message.message_manifest)?;
    if runtime.tool_trace_count != tool_calls.len() {
        return Err(anyhow!(
            "assistant message {} tool_trace_count={} does not match tool_trace entries={}",
            message.id,
            runtime.tool_trace_count,
            tool_calls.len()
        ));
    }

    Ok(Some(RecoveredAssistantArtifact {
        message: message.clone(),
        runtime,
        tool_calls,
    }))
}

fn find_recoverable_session_turn(
    session_manifest: &serde_json::Value,
    turn_id: &str,
) -> Result<Option<RecoveredPendingAssistantTurn>> {
    let Some(turn) = session_manifest
        .get("last_turn")
        .and_then(serde_json::Value::as_object)
    else {
        return Ok(None);
    };
    if turn.get("turn_id").and_then(serde_json::Value::as_str) != Some(turn_id) {
        return Ok(None);
    }
    let Some(recovery) = turn.get("recovery").and_then(serde_json::Value::as_object) else {
        return Ok(None);
    };
    let assistant_message_content = recovery
        .get("assistant_message_content")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| anyhow!("session recovery payload missing assistant_message_content"))?;
    let runtime = parse_runtime_manifest_value(
        recovery
            .get("runtime")
            .ok_or_else(|| anyhow!("session recovery payload missing runtime"))?,
    )?;
    let tool_calls = parse_tool_trace_value(
        recovery
            .get("tool_trace")
            .ok_or_else(|| anyhow!("session recovery payload missing tool_trace"))?,
    )?;
    if runtime.tool_trace_count != tool_calls.len() {
        return Err(anyhow!(
            "session turn {} tool_trace_count={} does not match tool_trace entries={}",
            turn_id,
            runtime.tool_trace_count,
            tool_calls.len()
        ));
    }
    let captured_at = recovery
        .get("captured_at")
        .cloned()
        .map(serde_json::from_value::<chrono::DateTime<Utc>>)
        .transpose()
        .map_err(|error| anyhow!("session recovery payload has invalid captured_at: {error}"))?
        .unwrap_or_else(Utc::now);

    Ok(Some(RecoveredPendingAssistantTurn {
        assistant_message_content,
        runtime,
        tool_calls,
        captured_at,
    }))
}

fn find_recoverable_task_turn(
    task_payload: &serde_json::Value,
    turn_id: &str,
) -> Result<Option<RecoveredPendingAssistantTurn>> {
    let Some(recovery) = task_payload
        .get(CHAT_TURN_RECOVERY_PAYLOAD_KEY)
        .and_then(serde_json::Value::as_object)
    else {
        return Ok(None);
    };
    if recovery.get("turn_id").and_then(serde_json::Value::as_str) != Some(turn_id) {
        return Ok(None);
    }
    let assistant_message_content = recovery
        .get("assistant_message_content")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| anyhow!("task recovery payload missing assistant_message_content"))?;
    let runtime = parse_runtime_manifest_value(
        recovery
            .get("runtime")
            .ok_or_else(|| anyhow!("task recovery payload missing runtime"))?,
    )?;
    let tool_calls = parse_tool_trace_value(
        recovery
            .get("tool_trace")
            .ok_or_else(|| anyhow!("task recovery payload missing tool_trace"))?,
    )?;
    if runtime.tool_trace_count != tool_calls.len() {
        return Err(anyhow!(
            "task turn {} tool_trace_count={} does not match tool_trace entries={}",
            turn_id,
            runtime.tool_trace_count,
            tool_calls.len()
        ));
    }
    let captured_at = recovery
        .get("captured_at")
        .cloned()
        .map(serde_json::from_value::<chrono::DateTime<Utc>>)
        .transpose()
        .map_err(|error| anyhow!("task recovery payload has invalid captured_at: {error}"))?
        .unwrap_or_else(Utc::now);

    Ok(Some(RecoveredPendingAssistantTurn {
        assistant_message_content,
        runtime,
        tool_calls,
        captured_at,
    }))
}

fn find_recoverable_event_turn(
    events: &[WorkflowEventRecord],
    turn_id: &str,
) -> Result<Option<RecoveredPendingAssistantTurn>> {
    let Some(event) = events.iter().rev().find(|event| {
        event.event_name == CHAT_TURN_RECOVERY_EVENT_NAME
            && event
                .payload
                .get("turn_id")
                .and_then(serde_json::Value::as_str)
                == Some(turn_id)
    }) else {
        return Ok(None);
    };
    let assistant_message_content = event
        .payload
        .get("assistant_message_content")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            anyhow!("workflow event recovery payload missing assistant_message_content")
        })?;
    let runtime = parse_runtime_manifest_value(
        event
            .payload
            .get("runtime")
            .ok_or_else(|| anyhow!("workflow event recovery payload missing runtime"))?,
    )?;
    let tool_calls = parse_tool_trace_value(
        event
            .payload
            .get("tool_trace")
            .ok_or_else(|| anyhow!("workflow event recovery payload missing tool_trace"))?,
    )?;
    if runtime.tool_trace_count != tool_calls.len() {
        return Err(anyhow!(
            "workflow event turn {} tool_trace_count={} does not match tool_trace entries={}",
            turn_id,
            runtime.tool_trace_count,
            tool_calls.len()
        ));
    }

    Ok(Some(RecoveredPendingAssistantTurn {
        assistant_message_content,
        runtime,
        tool_calls,
        captured_at: event.created_at,
    }))
}

async fn resume_assistant_message_manifest(
    storage: &PgStorage,
    tenant_id: domain_model::TenantId,
    recovered: RecoveredAssistantArtifact,
) -> Result<ChatMessage> {
    let turn = recovered
        .message
        .message_manifest
        .get("turn")
        .and_then(serde_json::Value::as_object);
    let needs_finalize = turn
        .and_then(|turn| turn.get("assistant_message_persisted_at"))
        .map(serde_json::Value::is_null)
        .unwrap_or(true)
        || turn
            .and_then(|turn| turn.get("assistant_message_id"))
            .map(serde_json::Value::is_null)
            .unwrap_or(true);
    if !needs_finalize {
        return Ok(recovered.message);
    }

    let assistant_message_manifest = finalize_assistant_message_manifest(
        &recovered.message.message_manifest,
        recovered.message.id,
        Utc::now(),
    );
    storage
        .chat_messages()
        .update_manifest(tenant_id, recovered.message.id, &assistant_message_manifest)
        .await
        .map_err(Into::into)
}

fn parse_runtime_from_message_manifest(manifest: &serde_json::Value) -> Result<LlmRuntimeMetadata> {
    parse_runtime_manifest_value(
        manifest
            .get("runtime")
            .ok_or_else(|| anyhow!("assistant message manifest missing runtime object"))?,
    )
}

fn parse_runtime_manifest_value(runtime: &serde_json::Value) -> Result<LlmRuntimeMetadata> {
    let runtime = runtime
        .as_object()
        .ok_or_else(|| anyhow!("runtime payload is not an object"))?;
    let mode = match runtime
        .get("mode")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("provider")
    {
        "placeholder" => LlmRuntimeMode::Placeholder,
        "provider" => LlmRuntimeMode::Provider,
        other => return Err(anyhow!("unknown runtime mode {other}")),
    };
    let provider = runtime
        .get("provider")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("assistant message runtime missing provider"))?
        .to_string();
    let model = runtime
        .get("model")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("assistant message runtime missing model"))?
        .to_string();
    let finish_reason = runtime
        .get("finish_reason")
        .and_then(serde_json::Value::as_str)
        .map(LlmFinishReason::from_wire_value);
    let usage = if let Some(usage) = runtime.get("usage").and_then(serde_json::Value::as_object) {
        Some(LlmTokenUsage {
            input_tokens: usage
                .get("input_tokens")
                .and_then(serde_json::Value::as_u64)
                .ok_or_else(|| anyhow!("assistant runtime usage missing input_tokens"))?
                as usize,
            output_tokens: usage
                .get("output_tokens")
                .and_then(serde_json::Value::as_u64)
                .ok_or_else(|| anyhow!("assistant runtime usage missing output_tokens"))?
                as usize,
            total_tokens: usage
                .get("total_tokens")
                .and_then(serde_json::Value::as_u64)
                .ok_or_else(|| anyhow!("assistant runtime usage missing total_tokens"))?
                as usize,
        })
    } else {
        None
    };
    let tool_trace_count = runtime
        .get("tool_trace_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0) as usize;

    Ok(LlmRuntimeMetadata {
        mode,
        provider,
        model,
        request_id: runtime
            .get("request_id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        finish_reason,
        latency_ms: runtime
            .get("latency_ms")
            .and_then(serde_json::Value::as_u64),
        usage,
        system_prompt_key: runtime
            .get("system_prompt_key")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        system_prompt_version: runtime
            .get("system_prompt_version")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        tool_trace_count,
    })
}

fn parse_tool_calls_from_message_manifest(
    manifest: &serde_json::Value,
) -> Result<Vec<LlmToolCall>> {
    let Some(tool_trace) = manifest.get("tool_trace") else {
        return Ok(Vec::new());
    };
    parse_tool_trace_value(tool_trace)
        .map_err(|error| anyhow!("assistant message manifest has invalid tool_trace: {error}"))
}

fn parse_tool_trace_value(tool_trace: &serde_json::Value) -> Result<Vec<LlmToolCall>> {
    serde_json::from_value(tool_trace.clone())
        .map_err(|error| anyhow!("invalid tool_trace payload: {error}"))
}

fn llm_invocation_record_from_runtime(runtime: &LlmRuntimeMetadata) -> LlmInvocationRecordInput {
    LlmInvocationRecordInput {
        mode: match runtime.mode {
            llm_gateway::LlmRuntimeMode::Placeholder => {
                domain_model::LlmInvocationMode::Placeholder
            }
            llm_gateway::LlmRuntimeMode::Provider => domain_model::LlmInvocationMode::Provider,
        },
        provider: Some(runtime.provider.clone()),
        model: Some(runtime.model.clone()),
        request_id: runtime.request_id.clone(),
        finish_reason: runtime.finish_reason.as_ref().map(|reason| match reason {
            llm_gateway::LlmFinishReason::Stop => domain_model::LlmInvocationFinishReason::Stop,
            llm_gateway::LlmFinishReason::ToolCalls => {
                domain_model::LlmInvocationFinishReason::ToolCalls
            }
            llm_gateway::LlmFinishReason::Length => domain_model::LlmInvocationFinishReason::Length,
            llm_gateway::LlmFinishReason::ContentFilter => {
                domain_model::LlmInvocationFinishReason::ContentFilter
            }
            llm_gateway::LlmFinishReason::Error => domain_model::LlmInvocationFinishReason::Error,
            llm_gateway::LlmFinishReason::Other(value) => {
                domain_model::LlmInvocationFinishReason::Other(value.clone())
            }
        }),
        latency_ms: runtime.latency_ms,
        usage: runtime
            .usage
            .as_ref()
            .map(|usage| domain_model::LlmTokenUsage {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                total_tokens: usage.total_tokens,
            }),
        system_prompt_key: runtime.system_prompt_key.clone(),
        system_prompt_version: runtime.system_prompt_version.clone(),
        tool_trace_count: Some(runtime.tool_trace_count),
    }
}

fn tool_execution_records_from_tool_calls(
    tool_calls: &[LlmToolCall],
) -> Vec<ToolExecutionRecordInput> {
    tool_calls
        .iter()
        .map(|tool_call| ToolExecutionRecordInput {
            call_id: tool_call.call_id.clone(),
            tool_name: tool_call.tool_name.clone(),
            tool_snapshot: find_default_tool_snapshot_value(&tool_call.tool_name),
            status: match tool_call.status {
                LlmToolCallStatus::Requested => domain_model::ToolExecutionStatus::Requested,
                LlmToolCallStatus::Completed => domain_model::ToolExecutionStatus::Completed,
                LlmToolCallStatus::Failed => domain_model::ToolExecutionStatus::Failed,
            },
            arguments: tool_call.arguments.clone(),
            result: tool_call.result.clone(),
        })
        .collect()
}

fn parse_turn_stream_mode(value: &str) -> Result<String> {
    match value {
        "buffered" | "streaming" => Ok(value.to_string()),
        other => Err(anyhow!(
            "unsupported CHAT_SESSION_RUNTIME_STREAM_MODE {other}; expected buffered or streaming"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        find_recoverable_assistant_message, find_recoverable_event_turn,
        find_recoverable_session_turn, find_recoverable_task_turn, payload_with_turn_recovery,
        payload_without_turn_recovery, render_turn_recovery_payload,
        service_handoff_from_session_manifest,
    };
    use chrono::Utc;
    use domain_model::{
        ChatMessage, ChatMessageId, ChatMessageRole, ChatSessionId, TenantId, WorkflowEventId,
        WorkflowEventRecord, WorkflowExecutionId,
    };
    use llm_gateway::{
        LlmFinishReason, LlmRuntimeMetadata, LlmRuntimeMode, LlmTokenUsage, LlmToolCall,
        LlmToolCallStatus,
    };
    use serde_json::json;

    #[test]
    fn service_handoff_from_session_manifest_captures_confirmed_report_entry() {
        let now = Utc::now();
        let report_plan_id = domain_model::ReportPlanId::new();
        let handoff = service_handoff_from_session_manifest(&json!({
            "report_entry": {
                "state": "confirmed",
                "requested_at": now,
                "resolved_at": now,
                "resolved_action": "enter_report_service",
                "suggested_title": "Dataset Report",
                "suggested_objective": "Turn the current dataset context into a report-ready output.",
                "confirmed_report_plan_id": report_plan_id,
            }
        }))
        .expect("service handoff should parse");

        assert_eq!(
            handoff.source,
            contracts::ManifestServiceHandoffSourceView::ChatSessionReportEntry
        );
        assert_eq!(
            handoff.service_lane,
            contracts::ModelFacingServiceLaneView::ReportService
        );
        assert_eq!(
            handoff.report_entry_state,
            contracts::ModelFacingReportEntryStateView::Confirmed
        );
        assert_eq!(
            handoff.resolved_action,
            Some(contracts::ChatSessionReportEntryResolutionView::EnterReportService)
        );
        assert_eq!(handoff.confirmed_report_plan_id, Some(report_plan_id));
    }

    #[test]
    fn find_recoverable_assistant_message_returns_latest_matching_turn() {
        let turn_id = "turn-recover";
        let older = build_chat_message(
            ChatMessageRole::Assistant,
            "older",
            "other-turn",
            build_runtime("req-older", 0),
            Vec::new(),
        );
        let newer = build_chat_message(
            ChatMessageRole::Assistant,
            "newer",
            turn_id,
            build_runtime("req-newer", 1),
            vec![build_tool_call("weather.lookup")],
        );
        let newest = build_chat_message(
            ChatMessageRole::Assistant,
            "newest",
            turn_id,
            build_runtime("req-newest", 1),
            vec![build_tool_call("memory.append")],
        );

        let recovered =
            find_recoverable_assistant_message(&[older, newer, newest.clone()], turn_id)
                .expect("recovery should succeed")
                .expect("matching assistant message should exist");

        assert_eq!(recovered.message.id, newest.id);
        assert_eq!(recovered.runtime.request_id.as_deref(), Some("req-newest"));
        assert_eq!(recovered.tool_calls.len(), 1);
        assert_eq!(recovered.tool_calls[0].tool_name, "memory.append");
    }

    #[test]
    fn find_recoverable_assistant_message_rejects_tool_trace_count_mismatch() {
        let message = build_chat_message(
            ChatMessageRole::Assistant,
            "broken",
            "turn-broken",
            build_runtime("req-broken", 2),
            vec![build_tool_call("weather.lookup")],
        );

        let error = find_recoverable_assistant_message(&[message], "turn-broken")
            .expect_err("mismatched tool trace should fail");

        assert!(error
            .to_string()
            .contains("tool_trace_count=2 does not match tool_trace entries=1"));
    }

    #[test]
    fn find_recoverable_session_turn_returns_response_ready_payload() {
        let session_manifest = json!({
            "last_turn": {
                "turn_id": "turn-session-recover",
                "recovery": {
                    "assistant_message_content": "Recovered from session",
                    "runtime": llm_gateway::render_runtime_manifest(&build_runtime("req-session", 1)),
                    "tool_trace": [build_tool_call("weather.lookup")],
                    "captured_at": Utc::now(),
                }
            }
        });

        let recovered = find_recoverable_session_turn(&session_manifest, "turn-session-recover")
            .expect("session recovery should succeed")
            .expect("session recovery payload should exist");

        assert_eq!(
            recovered.assistant_message_content,
            "Recovered from session"
        );
        assert_eq!(recovered.runtime.request_id.as_deref(), Some("req-session"));
        assert_eq!(recovered.tool_calls.len(), 1);
        assert_eq!(recovered.tool_calls[0].tool_name, "weather.lookup");
    }

    #[test]
    fn find_recoverable_session_turn_rejects_tool_trace_count_mismatch() {
        let session_manifest = json!({
            "last_turn": {
                "turn_id": "turn-session-broken",
                "recovery": {
                    "assistant_message_content": "Recovered from session",
                    "runtime": llm_gateway::render_runtime_manifest(&build_runtime("req-session-broken", 2)),
                    "tool_trace": [build_tool_call("weather.lookup")],
                    "captured_at": Utc::now(),
                }
            }
        });

        let error = find_recoverable_session_turn(&session_manifest, "turn-session-broken")
            .expect_err("mismatched session recovery payload should fail");

        assert!(error
            .to_string()
            .contains("tool_trace_count=2 does not match tool_trace entries=1"));
    }

    #[test]
    fn find_recoverable_task_turn_returns_payload_checkpoint() {
        let payload = payload_with_turn_recovery(
            &json!({ "execution_id": "exec-1" }),
            "turn-task-recover",
            "Recovered from task payload",
            &build_runtime("req-task", 1),
            &[build_tool_call("weather.lookup")],
            Utc::now(),
        );

        let recovered = find_recoverable_task_turn(&payload, "turn-task-recover")
            .expect("task recovery should succeed")
            .expect("task recovery payload should exist");

        assert_eq!(
            recovered.assistant_message_content,
            "Recovered from task payload"
        );
        assert_eq!(recovered.runtime.request_id.as_deref(), Some("req-task"));
        assert_eq!(recovered.tool_calls.len(), 1);
    }

    #[test]
    fn find_recoverable_task_turn_rejects_tool_trace_count_mismatch() {
        let payload = payload_with_turn_recovery(
            &json!({}),
            "turn-task-broken",
            "Recovered from task payload",
            &build_runtime("req-task-broken", 2),
            &[build_tool_call("weather.lookup")],
            Utc::now(),
        );

        let error = find_recoverable_task_turn(&payload, "turn-task-broken")
            .expect_err("mismatched task recovery payload should fail");

        assert!(error
            .to_string()
            .contains("tool_trace_count=2 does not match tool_trace entries=1"));
    }

    #[test]
    fn payload_without_turn_recovery_removes_only_checkpoint_key() {
        let payload = payload_with_turn_recovery(
            &json!({ "execution_id": "exec-1", "kind": "chat_session" }),
            "turn-task-clear",
            "Recovered from task payload",
            &build_runtime("req-task-clear", 0),
            &[],
            Utc::now(),
        );

        let cleared = payload_without_turn_recovery(&payload);

        assert_eq!(cleared["execution_id"], json!("exec-1"));
        assert_eq!(cleared["kind"], json!("chat_session"));
        assert!(cleared.get("_chat_turn_recovery").is_none());
    }

    #[test]
    fn find_recoverable_event_turn_returns_latest_event_checkpoint() {
        let older = WorkflowEventRecord {
            id: WorkflowEventId::new(),
            execution_id: WorkflowExecutionId::new(),
            sequence_no: 1,
            event_name: "chat_turn_recovery_checkpoint".to_string(),
            payload: render_turn_recovery_payload(
                "turn-event-recover",
                "Older recovery",
                &build_runtime("req-event-older", 1),
                &[build_tool_call("weather.lookup")],
                Utc::now(),
            ),
            created_at: Utc::now(),
        };
        let newer = WorkflowEventRecord {
            id: WorkflowEventId::new(),
            execution_id: older.execution_id,
            sequence_no: 2,
            event_name: "chat_turn_recovery_checkpoint".to_string(),
            payload: render_turn_recovery_payload(
                "turn-event-recover",
                "Newer recovery",
                &build_runtime("req-event-newer", 1),
                &[build_tool_call("memory.append")],
                Utc::now(),
            ),
            created_at: Utc::now(),
        };

        let recovered = find_recoverable_event_turn(&[older, newer], "turn-event-recover")
            .expect("event recovery should succeed")
            .expect("event recovery payload should exist");

        assert_eq!(recovered.assistant_message_content, "Newer recovery");
        assert_eq!(
            recovered.runtime.request_id.as_deref(),
            Some("req-event-newer")
        );
        assert_eq!(recovered.tool_calls[0].tool_name, "memory.append");
    }

    #[test]
    fn find_recoverable_event_turn_rejects_tool_trace_count_mismatch() {
        let event = WorkflowEventRecord {
            id: WorkflowEventId::new(),
            execution_id: WorkflowExecutionId::new(),
            sequence_no: 1,
            event_name: "chat_turn_recovery_checkpoint".to_string(),
            payload: render_turn_recovery_payload(
                "turn-event-broken",
                "Broken recovery",
                &build_runtime("req-event-broken", 2),
                &[build_tool_call("weather.lookup")],
                Utc::now(),
            ),
            created_at: Utc::now(),
        };

        let error = find_recoverable_event_turn(&[event], "turn-event-broken")
            .expect_err("mismatched event recovery payload should fail");

        assert!(error
            .to_string()
            .contains("tool_trace_count=2 does not match tool_trace entries=1"));
    }

    fn build_chat_message(
        role: ChatMessageRole,
        content: &str,
        turn_id: &str,
        runtime: LlmRuntimeMetadata,
        tool_trace: Vec<LlmToolCall>,
    ) -> ChatMessage {
        ChatMessage {
            id: ChatMessageId::new(),
            tenant_id: TenantId::new(),
            session_id: ChatSessionId::new(),
            role,
            turn_index: 0,
            content: content.to_string(),
            message_manifest: json!({
                "turn": {
                    "turn_id": turn_id,
                    "assistant_message_id": null,
                    "assistant_message_persisted_at": null,
                },
                "runtime": llm_gateway::render_runtime_manifest(&runtime),
                "tool_trace": tool_trace,
            }),
            created_at: Utc::now(),
        }
    }

    fn build_runtime(request_id: &str, tool_trace_count: usize) -> LlmRuntimeMetadata {
        LlmRuntimeMetadata {
            mode: LlmRuntimeMode::Provider,
            provider: "openai".to_string(),
            model: "gpt-test".to_string(),
            request_id: Some(request_id.to_string()),
            finish_reason: Some(LlmFinishReason::ToolCalls),
            latency_ms: Some(42),
            usage: Some(LlmTokenUsage {
                input_tokens: 10,
                output_tokens: 20,
                total_tokens: 30,
            }),
            system_prompt_key: Some("chat.session.default".to_string()),
            system_prompt_version: Some("v1".to_string()),
            tool_trace_count,
        }
    }

    fn build_tool_call(tool_name: &str) -> LlmToolCall {
        LlmToolCall {
            call_id: Some(format!("call-{tool_name}")),
            tool_name: tool_name.to_string(),
            status: LlmToolCallStatus::Completed,
            arguments: Some(json!({ "query": "status" })),
            result: Some(json!({ "ok": true })),
        }
    }
}
