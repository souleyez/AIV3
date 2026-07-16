use anyhow::{anyhow, Result};
use chrono::Utc;
use dataset_output_worker::{
    dataset_runtime_error_message, DatasetOutputGenerator, DatasetOutputJob,
    PlaceholderDatasetOutputGenerator,
};
use domain_model::{
    ChatSessionId, DocumentId, DocumentLifecycle, MemoryDirectory, MemoryDirectoryId,
    RetrievalEvidenceId, UserId,
};
use event_bus::{workflow_task_enqueued_subject, EventBus, EventSubscription};
use llm_gateway::{
    build_provider_from_env, LlmProviderError, LlmRuntimeMetadata, LlmToolCall, LlmToolCallStatus,
};
use prompt_registry::bootstrap_default_prompt_registry;
use std::collections::HashSet;
use storage::{
    LlmInvocationRecordInput, NewDatasetOutput, PgStorage, ToolExecutionRecordInput,
    DEFAULT_LOCAL_DATABASE_URL,
};
use tokio::time::Duration;
use tool_registry::find_default_tool_snapshot_value;
use uuid::Uuid;
use workflow_engine::{WorkflowCatalog, WorkflowSignal};

const DEFAULT_QUEUE: &str = "dataset_output";
const DEFAULT_TASK_KEY: &str = "produce_dataset_output";
const DEFAULT_POLL_INTERVAL_MS: u64 = 1_000;
const DEFAULT_RUNTIME_MODE: &str = "placeholder";
const DEFAULT_RUNTIME_PROVIDER: &str = "placeholder";
const DEFAULT_RUNTIME_MODEL: &str = "placeholder-dataset-output-v1";
const RETRIEVAL_SEARCH_LIMIT: usize = 8;

#[derive(Debug)]
struct DatasetOutputTaskError {
    source: anyhow::Error,
    failed_runtime: Option<LlmRuntimeMetadata>,
    failed_tool_calls: Vec<LlmToolCall>,
}

impl From<anyhow::Error> for DatasetOutputTaskError {
    fn from(source: anyhow::Error) -> Self {
        let failed_runtime = source
            .downcast_ref::<LlmProviderError>()
            .map(|error| error.runtime().clone());
        Self {
            source,
            failed_runtime,
            failed_tool_calls: Vec::new(),
        }
    }
}

impl From<platform_api::ApiError> for DatasetOutputTaskError {
    fn from(source: platform_api::ApiError) -> Self {
        Self {
            source: source.into(),
            failed_runtime: None,
            failed_tool_calls: Vec::new(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    observability::install("dataset_output_worker")?;

    let database_url = std::env::var("PLATFORM_DATABASE_URL")
        .unwrap_or_else(|_| DEFAULT_LOCAL_DATABASE_URL.to_string());
    let queue = std::env::var("DATASET_OUTPUT_QUEUE").unwrap_or_else(|_| DEFAULT_QUEUE.to_string());
    let task_key =
        std::env::var("DATASET_OUTPUT_TASK_KEY").unwrap_or_else(|_| DEFAULT_TASK_KEY.to_string());
    let poll_interval = std::env::var("DATASET_OUTPUT_POLL_INTERVAL_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_POLL_INTERVAL_MS);
    let runtime_mode = std::env::var("DATASET_OUTPUT_RUNTIME_MODE")
        .unwrap_or_else(|_| DEFAULT_RUNTIME_MODE.to_string());
    let runtime_provider = std::env::var("DATASET_OUTPUT_RUNTIME_PROVIDER")
        .unwrap_or_else(|_| DEFAULT_RUNTIME_PROVIDER.to_string());
    let runtime_model = std::env::var("DATASET_OUTPUT_RUNTIME_MODEL")
        .unwrap_or_else(|_| DEFAULT_RUNTIME_MODEL.to_string());

    let storage = PgStorage::connect_with_configured_max_connections(
        &database_url,
        "DATASET_OUTPUT_DATABASE_MAX_CONNECTIONS",
    )
    .await?;
    let workflow_catalog = workflow_definitions::catalog();
    let event_bus = EventBus::connect_from_env_or_disabled("PLATFORM_NATS_URL").await;
    let prompt_registry = bootstrap_default_prompt_registry();
    let provider = build_provider_from_env(
        "DATASET_OUTPUT",
        &runtime_mode,
        runtime_provider,
        prompt_registry,
    )?;
    let generator = PlaceholderDatasetOutputGenerator::new(provider, runtime_model);
    let wake_subject = workflow_task_enqueued_subject(&queue, &task_key);
    let mut task_waker = event_bus
        .subscribe_queue_or_disabled(
            &wake_subject,
            Some(&format!("dataset_output_worker.{queue}.{task_key}")),
        )
        .await;

    tracing::info!(
        %queue,
        %task_key,
        %wake_subject,
        event_bus_enabled = event_bus.is_enabled(),
        poll_interval_ms = poll_interval,
        %runtime_mode,
        database_endpoint = %observability::redact_connection_endpoint(&database_url),
        "dataset-output-worker polling started"
    );

    loop {
        match storage
            .workflow_tasks()
            .claim_next_available(&queue, Some(&task_key), Utc::now())
            .await
        {
            Ok(Some(task)) => {
                if let Err(error) =
                    process_task(&storage, &workflow_catalog, &event_bus, &generator, task).await
                {
                    tracing::error!(error = ?error, "dataset output task processing failed");
                }
            }
            Ok(None) => {
                wait_for_next_task_signal(&mut task_waker, poll_interval).await;
            }
            Err(error) => {
                tracing::error!(error = ?error, "dataset output worker failed to claim task");
                wait_for_next_task_signal(&mut task_waker, poll_interval).await;
            }
        }
    }
}

async fn process_task(
    storage: &PgStorage,
    workflow_catalog: &WorkflowCatalog,
    event_bus: &EventBus,
    generator: &impl DatasetOutputGenerator,
    task: domain_model::WorkflowTask,
) -> Result<()> {
    let execution = storage
        .workflow_executions()
        .get_by_id(task.tenant_id, task.execution_id)
        .await?
        .ok_or_else(|| anyhow!("workflow execution {} not found", task.execution_id))?;
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
    let context_owner_user_id = context_uuid(&execution.context, "owner_user_id").map(UserId);
    let bound_chat_session_id =
        context_uuid(&execution.context, "chat_session_id").map(ChatSessionId);
    let bound_chat_session = match bound_chat_session_id {
        Some(chat_session_id) => {
            let session = storage
                .chat_sessions()
                .get_by_id(task.tenant_id, chat_session_id)
                .await?
                .ok_or_else(|| {
                    anyhow!(
                        "chat session {} referenced by workflow execution {} was not found",
                        chat_session_id,
                        execution.id
                    )
                })?;
            if session.dataset_id != dataset_id {
                return Err(anyhow!(
                    "chat session {} belongs to dataset {}, expected {}",
                    chat_session_id,
                    session.dataset_id,
                    dataset_id
                ));
            }
            Some(session)
        }
        None => None,
    };
    let owner_user_id = bound_chat_session
        .as_ref()
        .and_then(|session| session.user_id)
        .or(context_owner_user_id);
    let visible_document_ids =
        visible_document_ids_for_owner(storage, task.tenant_id, dataset_id, owner_user_id).await?;
    let indexed_documents = storage
        .documents()
        .list_by_dataset(task.tenant_id, dataset_id)
        .await?
        .into_iter()
        .filter(|document| document.lifecycle == DocumentLifecycle::Indexed)
        .filter(|document| visible_document_ids.contains(&document.id))
        .count();
    let bound_memory_directory_id =
        context_uuid(&execution.context, "memory_directory_id").map(MemoryDirectoryId);
    let bound_memory_directory_version_no =
        context_i32(&execution.context, "memory_directory_version_no");
    let bound_retrieval_evidence_ids =
        context_uuid_array(&execution.context, "retrieval_evidence_ids")?
            .into_iter()
            .map(RetrievalEvidenceId)
            .collect::<Vec<_>>();
    let (retrieval_search_evidence_ids, retrieval_search_tool_call) =
        run_retrieval_search_tool(storage, task.tenant_id, dataset_id, &prompt, owner_user_id)
            .await;
    let selected_retrieval_evidence_ids = if bound_retrieval_evidence_ids.is_empty() {
        retrieval_search_evidence_ids
    } else {
        bound_retrieval_evidence_ids
    };
    let bound_memory_directory = match bound_memory_directory_id {
        Some(memory_directory_id) => match storage
            .memory_directories()
            .get_by_id(task.tenant_id, memory_directory_id)
            .await?
        {
            Some(directory)
                if memory_directory_matches_owner_scope(
                    &directory,
                    &visible_document_ids,
                    owner_user_id,
                ) =>
            {
                Some(directory)
            }
            Some(_) => {
                return Err(anyhow!(
                    "memory directory {} is not visible for workflow execution {}",
                    memory_directory_id,
                    execution.id
                ));
            }
            None => None,
        },
        None => storage
            .memory_directories()
            .list_by_dataset(task.tenant_id, dataset_id)
            .await?
            .into_iter()
            .find(|directory| {
                memory_directory_matches_owner_scope(
                    directory,
                    &visible_document_ids,
                    owner_user_id,
                )
            }),
    };
    let retrieval_evidences = if selected_retrieval_evidence_ids.is_empty() {
        Vec::new()
    } else {
        storage
            .retrieval_evidences()
            .list_by_ids(task.tenant_id, &selected_retrieval_evidence_ids)
            .await?
            .into_iter()
            .filter(|evidence| visible_document_ids.contains(&evidence.document_id))
            .collect()
    };

    let process_result: std::result::Result<(), DatasetOutputTaskError> = async {
        let job = DatasetOutputJob {
            dataset_id,
            prompt: prompt.clone(),
            indexed_document_count: indexed_documents,
            refreshed_chunks: bound_memory_directory
                .as_ref()
                .map(|directory| directory.refreshed_chunks as usize)
                .unwrap_or(0),
            memory_directory_id: bound_memory_directory
                .as_ref()
                .map(|directory| directory.id),
            memory_directory_version_no: bound_memory_directory
                .as_ref()
                .map(|directory| directory.version_no)
                .or(bound_memory_directory_version_no),
            retrieval_evidence_ids: retrieval_evidences
                .iter()
                .map(|evidence| evidence.id)
                .collect(),
            tool_calls: vec![retrieval_search_tool_call.clone()],
            service_handoff: bound_chat_session.as_ref().and_then(|session| {
                service_handoff_from_session_manifest(&session.session_manifest)
            }),
        };
        let outcome = generator
            .generate(&job)
            .map_err(|source| dataset_task_error_with_tool_calls(source, &job.tool_calls))?;
        if let Some(error_message) = dataset_runtime_error_message(&outcome.runtime) {
            return Err(DatasetOutputTaskError {
                source: anyhow!(error_message),
                failed_runtime: Some(outcome.runtime.clone()),
                failed_tool_calls: outcome.tool_calls.clone(),
            });
        }
        let output = storage
            .dataset_outputs()
            .create(
                task.tenant_id,
                &NewDatasetOutput {
                    execution_id: task.execution_id,
                    dataset_id,
                    owner_user_id,
                    prompt: prompt.clone(),
                    output_text: outcome.output_text.clone(),
                    memory_directory_id: bound_memory_directory
                        .as_ref()
                        .map(|directory| directory.id),
                    retrieval_evidence_ids: retrieval_evidences
                        .iter()
                        .map(|evidence| evidence.id)
                        .collect(),
                    output_manifest: outcome.output_manifest.clone(),
                    created_at: Utc::now(),
                },
            )
            .await?;
        storage
            .llm_invocations()
            .replace_for_dataset_output_records(
                task.tenant_id,
                task.execution_id,
                output.id,
                &[llm_invocation_record_from_runtime(&outcome.runtime)],
                output.created_at,
            )
            .await?;
        storage
            .tool_executions()
            .replace_for_dataset_output_records(
                task.tenant_id,
                task.execution_id,
                output.id,
                &tool_execution_records_from_tool_calls(&outcome.tool_calls),
                output.created_at,
            )
            .await?;
        let signal_output = serde_json::json!({
            "dataset_id": dataset_id,
            "dataset_output_id": output.id,
            "memory_directory_id": output.memory_directory_id,
            "memory_directory_version_no": bound_memory_directory
                .as_ref()
                .map(|directory| directory.version_no)
                .or(bound_memory_directory_version_no),
            "retrieval_evidence_ids": output.retrieval_evidence_ids,
            "prompt": output.prompt,
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
        .await?;

        Ok(())
    }
    .await;

    if let Err(error) = process_result {
        let error_message = error.source.to_string();
        let failed_at = Utc::now();
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
                    "dataset output worker failed to persist execution-scoped llm invocation"
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
                    "dataset output worker failed to persist execution-scoped tool executions"
                );
            }
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
                "dataset output worker failed to send workflow step_failed signal"
            );
        }

        storage
            .workflow_tasks()
            .mark_failed(task.id, &error_message, Utc::now())
            .await?;
        return Err(error.source);
    }

    storage
        .workflow_tasks()
        .mark_succeeded(task.id, Utc::now())
        .await?;

    tracing::info!(
        task_id = %task.id,
        execution_id = %task.execution_id,
        dataset_id = %dataset_id,
        "dataset output task completed"
    );

    Ok(())
}

async fn visible_document_ids_for_owner(
    storage: &PgStorage,
    tenant_id: domain_model::TenantId,
    dataset_id: domain_model::DatasetId,
    current_user_id: Option<UserId>,
) -> Result<HashSet<DocumentId>> {
    Ok(storage
        .documents()
        .list_by_dataset(tenant_id, dataset_id)
        .await?
        .into_iter()
        .filter(|document| {
            document.owner_user_id.is_none() || document.owner_user_id == current_user_id
        })
        .map(|document| document.id)
        .collect())
}

fn memory_directory_matches_owner_scope(
    directory: &MemoryDirectory,
    visible_document_ids: &HashSet<DocumentId>,
    current_user_id: Option<UserId>,
) -> bool {
    (directory.owner_user_id.is_none() || directory.owner_user_id == current_user_id)
        && memory_directory_source_document_ids(directory)
            .into_iter()
            .all(|document_id| visible_document_ids.contains(&document_id))
}

fn memory_directory_source_document_ids(directory: &MemoryDirectory) -> Vec<DocumentId> {
    if !directory.source_document_ids.is_empty() {
        return directory.source_document_ids.clone();
    }

    let mut ids = Vec::new();
    collect_memory_manifest_document_ids(&directory.directory_manifest, &mut ids);
    ids
}

fn collect_memory_manifest_document_ids(value: &serde_json::Value, ids: &mut Vec<DocumentId>) {
    match value {
        serde_json::Value::Object(object) => {
            if let Some(document_id) = object
                .get("document_id")
                .and_then(serde_json::Value::as_str)
                .and_then(|value| Uuid::parse_str(value).ok())
                .map(DocumentId)
            {
                push_unique_document_id(ids, document_id);
            }
            if let Some(source_ids) = object
                .get("source_document_ids")
                .and_then(serde_json::Value::as_array)
            {
                for source_id in source_ids {
                    if let Some(document_id) = source_id
                        .as_str()
                        .and_then(|value| Uuid::parse_str(value).ok())
                        .map(DocumentId)
                    {
                        push_unique_document_id(ids, document_id);
                    }
                }
            }
            for child in object.values() {
                collect_memory_manifest_document_ids(child, ids);
            }
        }
        serde_json::Value::Array(entries) => {
            for entry in entries {
                collect_memory_manifest_document_ids(entry, ids);
            }
        }
        _ => {}
    }
}

fn push_unique_document_id(ids: &mut Vec<DocumentId>, document_id: DocumentId) {
    if !ids.iter().any(|existing| *existing == document_id) {
        ids.push(document_id);
    }
}

async fn run_retrieval_search_tool(
    storage: &PgStorage,
    tenant_id: domain_model::TenantId,
    dataset_id: domain_model::DatasetId,
    prompt: &str,
    current_user_id: Option<UserId>,
) -> (Vec<RetrievalEvidenceId>, LlmToolCall) {
    let arguments = serde_json::json!({
        "dataset_id": dataset_id,
        "query": prompt,
        "limit": RETRIEVAL_SEARCH_LIMIT,
    });
    match platform_api::search_dataset_retrieval_for_user(
        storage.clone(),
        tenant_id,
        dataset_id,
        prompt.to_string(),
        Some(RETRIEVAL_SEARCH_LIMIT),
        current_user_id,
    )
    .await
    {
        Ok(response) => {
            let evidence_ids = response
                .hits
                .iter()
                .map(|hit| hit.retrieval_evidence_id)
                .collect();
            (
                evidence_ids,
                retrieval_search_tool_call(
                    LlmToolCallStatus::Completed,
                    arguments,
                    serde_json::to_value(&response).unwrap_or_else(|error| {
                        serde_json::json!({
                            "error": format!("failed to serialize retrieval.search response: {error}")
                        })
                    }),
                ),
            )
        }
        Err(error) => {
            tracing::warn!(
                error = ?error,
                dataset_id = %dataset_id,
                "dataset output worker retrieval.search tool call failed"
            );
            (
                Vec::new(),
                retrieval_search_tool_call(
                    LlmToolCallStatus::Failed,
                    arguments,
                    serde_json::json!({ "error": error.to_string() }),
                ),
            )
        }
    }
}

fn retrieval_search_tool_call(
    status: LlmToolCallStatus,
    arguments: serde_json::Value,
    result: serde_json::Value,
) -> LlmToolCall {
    LlmToolCall {
        call_id: Some(format!("host_retrieval_search_{}", Uuid::new_v4())),
        tool_name: "retrieval.search".to_string(),
        status,
        arguments: Some(arguments),
        result: Some(result),
    }
}

fn dataset_task_error_with_tool_calls(
    source: anyhow::Error,
    tool_calls: &[LlmToolCall],
) -> DatasetOutputTaskError {
    let mut error = DatasetOutputTaskError::from(source);
    if !tool_calls.is_empty() {
        if let Some(runtime) = error.failed_runtime.as_mut() {
            runtime.tool_trace_count = tool_calls.len();
        }
        error.failed_tool_calls = tool_calls.to_vec();
    }
    error
}

async fn wait_for_next_task_signal(task_waker: &mut EventSubscription, poll_interval_ms: u64) {
    if let Some(event) = task_waker
        .wait_for_event(Duration::from_millis(poll_interval_ms))
        .await
    {
        tracing::debug!(subject = %event.subject, "dataset output worker received task wake signal");
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

fn context_uuid_array(value: &serde_json::Value, key: &str) -> Result<Vec<Uuid>> {
    let Some(entries) = value
        .as_object()
        .and_then(|map| map.get(key))
        .and_then(serde_json::Value::as_array)
    else {
        return Ok(Vec::new());
    };

    entries
        .iter()
        .map(|entry| {
            let raw = entry.as_str().ok_or_else(|| {
                anyhow!("execution context field {key} must contain UUID strings")
            })?;
            Uuid::parse_str(raw).map_err(|error| {
                anyhow!("invalid UUID {raw} in execution context field {key}: {error}")
            })
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::{
        dataset_task_error_with_tool_calls, retrieval_search_tool_call,
        service_handoff_from_session_manifest,
    };
    use chrono::Utc;
    use serde_json::json;

    #[test]
    fn service_handoff_from_session_manifest_captures_confirmation_required_gate() {
        let now = Utc::now();
        let handoff = service_handoff_from_session_manifest(&json!({
            "report_entry": {
                "state": "confirmation_required",
                "requested_at": now,
                "resolved_at": null,
                "resolved_action": null,
                "suggested_title": "Dataset Report",
                "suggested_objective": "Turn the current dataset context into a report-ready output.",
                "confirmed_report_plan_id": null
            }
        }))
        .expect("service handoff should parse");

        assert_eq!(
            handoff.source,
            contracts::ManifestServiceHandoffSourceView::ChatSessionReportEntry
        );
        assert_eq!(
            handoff.service_lane,
            contracts::ModelFacingServiceLaneView::MaterialService
        );
        assert_eq!(
            handoff.report_entry_state,
            contracts::ModelFacingReportEntryStateView::ConfirmationRequired
        );
        assert_eq!(handoff.requested_at, Some(now));
    }

    #[test]
    fn retrieval_search_tool_call_records_result_payload() {
        let call = retrieval_search_tool_call(
            llm_gateway::LlmToolCallStatus::Completed,
            json!({
                "dataset_id": domain_model::DatasetId::new(),
                "query": "revenue margin",
                "limit": 8
            }),
            json!({
                "hits": [{
                    "retrieval_evidence_id": domain_model::RetrievalEvidenceId::new(),
                    "document_id": domain_model::DocumentId::new(),
                    "score": 1.25,
                    "summary": "Revenue notes",
                    "source_locator": "documents/finance.md#chunk=1"
                }]
            }),
        );

        assert_eq!(call.tool_name, "retrieval.search");
        assert_eq!(call.status, llm_gateway::LlmToolCallStatus::Completed);
        assert_eq!(
            call.result.as_ref().unwrap()["hits"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn provider_failure_preserves_precomputed_tool_calls() {
        let tool_call = retrieval_search_tool_call(
            llm_gateway::LlmToolCallStatus::Completed,
            json!({ "query": "risk controls" }),
            json!({ "hits": [] }),
        );
        let runtime = llm_gateway::LlmRuntimeMetadata {
            mode: llm_gateway::LlmRuntimeMode::Provider,
            provider: "openai".to_string(),
            model: "gpt-5.4".to_string(),
            lane: Some(llm_gateway::MODEL_LANE_DATASET_OUTPUT.to_string()),
            request_id: Some("req_failed".to_string()),
            finish_reason: Some(llm_gateway::LlmFinishReason::Error),
            provider_failure: Some(llm_gateway::LlmProviderFailure {
                kind: llm_gateway::LlmProviderFailureKind::RequestTimeout,
                message: "request timed out".to_string(),
            }),
            latency_ms: Some(30_000),
            usage: None,
            system_prompt_key: None,
            system_prompt_version: None,
            tool_trace_count: 0,
        };

        let error = dataset_task_error_with_tool_calls(
            llm_gateway::LlmProviderError::new(runtime, "provider failed").into(),
            &[tool_call],
        );

        assert_eq!(error.failed_tool_calls.len(), 1);
        assert_eq!(
            error
                .failed_runtime
                .as_ref()
                .map(|runtime| runtime.tool_trace_count),
            Some(1)
        );
    }
}
