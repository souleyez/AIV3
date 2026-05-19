use anyhow::{anyhow, Result};
use chrono::Utc;
use domain_model::{Document, DocumentLifecycle, WorkflowStatus};
use event_bus::{workflow_task_enqueued_subject, EventBus, EventSubscription};
use ingest_worker::{
    split_text_chunks, IngestJob, IngestOutcome, IngestProcessor, LocalIngestProcessor,
};
use serde_json::{json, Value};
use storage::{NewDocument, NewDocumentChunk, PgStorage, DEFAULT_LOCAL_DATABASE_URL};
use tokio::time::Duration;
use workflow_engine::{WorkflowCatalog, WorkflowSignal};

const DEFAULT_QUEUE: &str = "ingest";
const DEFAULT_WAKE_TASK_KEY: &str = "ingest_uploaded_document";
const EXTERNAL_SOURCE_INGEST_TASK_KEY: &str = "ingest_external_content";
const DEFAULT_POLL_INTERVAL_MS: u64 = 1_000;
const DEFAULT_AUTO_REPARSE_MAX_ATTEMPTS: usize = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
struct AutoReparseDecision {
    should_retry: bool,
    attempt_count: usize,
    max_attempts: usize,
    status: &'static str,
    reason: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UploadedDocumentTaskOutcome {
    Completed,
    DeferredForReparse,
}

#[tokio::main]
async fn main() -> Result<()> {
    observability::install("ingest_worker")?;

    let database_url = std::env::var("PLATFORM_DATABASE_URL")
        .unwrap_or_else(|_| DEFAULT_LOCAL_DATABASE_URL.to_string());
    let queue = std::env::var("INGEST_QUEUE").unwrap_or_else(|_| DEFAULT_QUEUE.to_string());
    let task_key = optional_env("INGEST_TASK_KEY");
    let poll_interval = std::env::var("INGEST_POLL_INTERVAL_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_POLL_INTERVAL_MS);

    let storage = PgStorage::connect(&database_url).await?;
    let workflow_catalog = workflow_definitions::catalog();
    let event_bus = EventBus::connect_from_env_or_disabled("PLATFORM_NATS_URL").await;
    let processor = LocalIngestProcessor;
    let wake_task_key = task_key.as_deref().unwrap_or(DEFAULT_WAKE_TASK_KEY);
    let wake_subject = workflow_task_enqueued_subject(&queue, wake_task_key);
    let mut task_waker = event_bus
        .subscribe_queue_or_disabled(
            &wake_subject,
            Some(&format!("ingest_worker.{queue}.{wake_task_key}")),
        )
        .await;

    tracing::info!(
        %queue,
        task_key = task_key.as_deref().unwrap_or("*"),
        %wake_subject,
        event_bus_enabled = event_bus.is_enabled(),
        poll_interval_ms = poll_interval,
        %database_url,
        "ingest-worker polling started"
    );

    loop {
        match storage
            .workflow_tasks()
            .claim_next_available(&queue, task_key.as_deref(), Utc::now())
            .await
        {
            Ok(Some(task)) => {
                if let Err(error) =
                    process_task(&storage, &workflow_catalog, &event_bus, &processor, task).await
                {
                    tracing::error!(error = ?error, "ingest task processing failed");
                }
            }
            Ok(None) => {
                wait_for_next_task_signal(&mut task_waker, poll_interval).await;
            }
            Err(error) => {
                tracing::error!(error = ?error, "ingest worker failed to claim task");
                wait_for_next_task_signal(&mut task_waker, poll_interval).await;
            }
        }
    }
}

async fn process_task(
    storage: &PgStorage,
    workflow_catalog: &WorkflowCatalog,
    event_bus: &EventBus,
    processor: &impl IngestProcessor,
    task: domain_model::WorkflowTask,
) -> Result<()> {
    if task.task_key == EXTERNAL_SOURCE_INGEST_TASK_KEY {
        return process_external_source_ingest_task(storage, workflow_catalog, event_bus, task)
            .await;
    }

    process_uploaded_document_task(storage, workflow_catalog, event_bus, processor, task).await
}

async fn process_uploaded_document_task(
    storage: &PgStorage,
    workflow_catalog: &WorkflowCatalog,
    event_bus: &EventBus,
    processor: &impl IngestProcessor,
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
    let document_id = context_uuid_string(&execution.context, "document_id")?
        .map(domain_model::DocumentId)
        .ok_or_else(|| anyhow!("workflow execution {} missing document_id", execution.id))?;
    let document = storage
        .documents()
        .get_by_id(task.tenant_id, document_id)
        .await?
        .ok_or_else(|| anyhow!("document {} not found", document_id))?;

    let job = IngestJob {
        dataset_id,
        document_id,
        title: document.title.clone(),
        object_key: document.object_key.clone(),
        content_type: document.content_type.clone(),
    };
    let current_title = document.title.clone();
    let current_content_type = document.content_type.clone();
    let current_object_key = document.object_key.clone();

    let process_result: Result<UploadedDocumentTaskOutcome> = async {
        let outcome = processor.process(&job);
        let chunk_count = outcome.chunk_count();
        let parse_status = outcome.parse_status();
        let parse_quality_status = outcome.parse_quality_status();
        let cloud_structured_provider = outcome.cloud_structured_provider();
        let auto_reparse_decision =
            auto_reparse_decision(&document, &parse_status, parse_quality_status.as_deref());
        let chunks = build_document_chunks(dataset_id, document_id, &outcome);
        storage
            .document_chunks()
            .replace_for_document(task.tenant_id, document_id, &chunks)
            .await?;
        let extracted_at = Utc::now();
        let mut ingest_metadata = json!({
            "processor": if outcome.used_placeholder { "placeholder" } else { "local_parser" },
            "parse_method": outcome.parse_method.clone(),
            "parse_status": parse_status,
            "parse_quality_status": parse_quality_status,
            "cloud_structured_provider": cloud_structured_provider,
            "parse_metadata": outcome.metadata.clone(),
            "chunk_count": chunk_count,
            "extracted_chars": outcome.extracted_chars,
            "content_type": current_content_type,
            "object_key": current_object_key,
            "extracted_at": extracted_at,
        });
        if let Value::Object(metadata) = &mut ingest_metadata {
            if let Some(decision) = auto_reparse_decision.as_ref() {
                metadata.insert(
                    "auto_reparse".to_string(),
                    auto_reparse_metadata(decision, extracted_at),
                );
            } else if let Some(success_metadata) =
                auto_reparse_success_metadata(&document, extracted_at)
            {
                metadata.insert("auto_reparse".to_string(), success_metadata);
            }
        }
        let metadata_updates = json!({
            "parse_status": parse_status,
            "ingest": ingest_metadata
        });
        if let Some(decision) = auto_reparse_decision.as_ref() {
            let updated_document = storage
                .documents()
                .update_state(
                    task.tenant_id,
                    document_id,
                    DocumentLifecycle::Failed,
                    Some(&current_title),
                    &metadata_updates,
                    Utc::now(),
                )
                .await?;

            platform_api::apply_workflow_signal_with_dependencies(
                storage,
                workflow_catalog,
                event_bus,
                task.tenant_id,
                task.execution_id,
                WorkflowSignal::StepFailed {
                    task_key: task.task_key.clone(),
                    error: decision.reason.clone(),
                },
            )
            .await?;
            storage
                .workflow_tasks()
                .mark_failed(task.id, &decision.reason, Utc::now())
                .await?;

            let retry_started = if decision.should_retry {
                dispatch_auto_reparse_retry(
                    storage,
                    workflow_catalog,
                    event_bus,
                    &task,
                    decision.reason.clone(),
                )
                .await
            } else {
                false
            };

            tracing::warn!(
                task_id = %task.id,
                execution_id = %task.execution_id,
                document_id = %updated_document.id,
                dataset_id = %updated_document.dataset_id,
                parse_status = %parse_status,
                auto_reparse_status = %decision.status,
                auto_reparse_attempt = decision.attempt_count,
                auto_reparse_max_attempts = decision.max_attempts,
                retry_started,
                "ingest parse quality degraded; workflow deferred for reparse"
            );
            return Ok(UploadedDocumentTaskOutcome::DeferredForReparse);
        }

        let updated_document = storage
            .documents()
            .update_state(
                task.tenant_id,
                document_id,
                DocumentLifecycle::Extracted,
                outcome.inferred_title.as_deref(),
                &metadata_updates,
                Utc::now(),
            )
            .await?;
        let signal_output = json!({
            "document_id": updated_document.id,
            "dataset_id": updated_document.dataset_id,
            "content_type": updated_document.content_type,
            "chunk_count": chunk_count,
            "parse_method": outcome.parse_method.clone(),
            "parse_status": parse_status,
            "parse_quality_status": parse_quality_status,
            "cloud_structured_provider": cloud_structured_provider,
            "extracted_chars": outcome.extracted_chars,
            "lifecycle": updated_document.lifecycle.as_str(),
            "title": updated_document.title,
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

        Ok(UploadedDocumentTaskOutcome::Completed)
    }
    .await;

    match process_result {
        Ok(UploadedDocumentTaskOutcome::Completed) => {}
        Ok(UploadedDocumentTaskOutcome::DeferredForReparse) => return Ok(()),
        Err(error) => {
            let error_message = error.to_string();
            let _ = storage
                .documents()
                .update_state(
                    task.tenant_id,
                    document_id,
                    DocumentLifecycle::Failed,
                    Some(&current_title),
                    &json!({
                        "parse_status": "failed",
                        "ingest": {
                            "processor": "placeholder",
                            "parse_status": "failed",
                            "failed_at": Utc::now(),
                            "last_error": error_message,
                        }
                    }),
                    Utc::now(),
                )
                .await;

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
                    "ingest worker failed to send workflow step_failed signal"
                );
            }

            storage
                .workflow_tasks()
                .mark_failed(task.id, &error_message, Utc::now())
                .await?;
            return Err(error);
        }
    }

    storage
        .workflow_tasks()
        .mark_succeeded(task.id, Utc::now())
        .await?;

    tracing::info!(
        task_id = %task.id,
        execution_id = %task.execution_id,
        document_id = %document_id,
        dataset_id = %dataset_id,
        "ingest task completed"
    );

    Ok(())
}

fn auto_reparse_decision(
    document: &Document,
    parse_status: &str,
    parse_quality_status: Option<&str>,
) -> Option<AutoReparseDecision> {
    auto_reparse_decision_for_attempts(
        parse_status,
        parse_quality_status,
        document_auto_reparse_attempt_count(document),
        ingest_auto_reparse_enabled(),
        ingest_auto_reparse_max_attempts(),
    )
}

fn auto_reparse_decision_for_attempts(
    parse_status: &str,
    parse_quality_status: Option<&str>,
    current_attempt_count: usize,
    enabled: bool,
    max_attempts: usize,
) -> Option<AutoReparseDecision> {
    if !parse_status_requires_auto_reparse(parse_status) {
        return None;
    }

    let should_retry = enabled && current_attempt_count < max_attempts;
    let attempt_count = if should_retry {
        current_attempt_count.saturating_add(1)
    } else {
        current_attempt_count
    };
    let status = if should_retry {
        "queued"
    } else if enabled {
        "exhausted"
    } else {
        "disabled"
    };
    let quality_status = parse_quality_status
        .filter(|status| !status.trim().is_empty())
        .unwrap_or("unknown");
    let reason = if should_retry {
        format!(
            "document parse quality requires reparse: parse_status={parse_status}, parse_quality_status={quality_status}, attempt={attempt_count}/{max_attempts}"
        )
    } else {
        format!(
            "document parse quality requires reparse but auto reparse is {status}: parse_status={parse_status}, parse_quality_status={quality_status}, attempts={attempt_count}/{max_attempts}"
        )
    };

    Some(AutoReparseDecision {
        should_retry,
        attempt_count,
        max_attempts,
        status,
        reason,
    })
}

fn parse_status_requires_auto_reparse(parse_status: &str) -> bool {
    matches!(parse_status, "parse_degraded" | "placeholder")
}

fn ingest_auto_reparse_enabled() -> bool {
    std::env::var("INGEST_AUTO_REPARSE_ENABLED")
        .ok()
        .and_then(|value| parse_bool_env_value(&value))
        .unwrap_or(true)
}

fn ingest_auto_reparse_max_attempts() -> usize {
    std::env::var("INGEST_AUTO_REPARSE_MAX_ATTEMPTS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(DEFAULT_AUTO_REPARSE_MAX_ATTEMPTS)
}

fn parse_bool_env_value(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn document_auto_reparse_attempt_count(document: &Document) -> usize {
    document
        .metadata
        .get("ingest")
        .and_then(|metadata| metadata.get("auto_reparse"))
        .and_then(|metadata| metadata.get("attempt_count"))
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(0)
}

fn auto_reparse_metadata(
    decision: &AutoReparseDecision,
    updated_at: chrono::DateTime<Utc>,
) -> Value {
    json!({
        "status": decision.status,
        "reason": decision.reason,
        "attempt_count": decision.attempt_count,
        "max_attempts": decision.max_attempts,
        "updated_at": updated_at,
    })
}

fn auto_reparse_success_metadata(
    document: &Document,
    updated_at: chrono::DateTime<Utc>,
) -> Option<Value> {
    let attempt_count = document_auto_reparse_attempt_count(document);
    if attempt_count == 0 {
        return None;
    }

    Some(json!({
        "status": "succeeded",
        "reason": "document parse succeeded after auto reparse",
        "attempt_count": attempt_count,
        "max_attempts": ingest_auto_reparse_max_attempts(),
        "updated_at": updated_at,
    }))
}

async fn dispatch_auto_reparse_retry(
    storage: &PgStorage,
    workflow_catalog: &WorkflowCatalog,
    event_bus: &EventBus,
    task: &domain_model::WorkflowTask,
    reason: String,
) -> bool {
    let retry_transition = match platform_api::apply_workflow_signal_with_dependencies(
        storage,
        workflow_catalog,
        event_bus,
        task.tenant_id,
        task.execution_id,
        WorkflowSignal::RetryRequested {
            reason: reason.clone(),
        },
    )
    .await
    {
        Ok(transition) => transition,
        Err(error) => {
            tracing::error!(
                error = ?error,
                task_id = %task.id,
                execution_id = %task.execution_id,
                "ingest worker failed to request auto reparse retry"
            );
            return false;
        }
    };

    if retry_transition.execution.status != WorkflowStatus::Pending {
        tracing::warn!(
            task_id = %task.id,
            execution_id = %task.execution_id,
            status = %retry_transition.execution.status.as_str(),
            "ingest auto reparse retry did not return workflow to pending"
        );
        return false;
    }

    match platform_api::apply_workflow_signal_with_dependencies(
        storage,
        workflow_catalog,
        event_bus,
        task.tenant_id,
        task.execution_id,
        WorkflowSignal::Start,
    )
    .await
    {
        Ok(_) => true,
        Err(error) => {
            tracing::error!(
                error = ?error,
                task_id = %task.id,
                execution_id = %task.execution_id,
                "ingest worker failed to restart workflow for auto reparse"
            );
            false
        }
    }
}

#[derive(Clone, Debug)]
struct ExternalSourceDocumentInput {
    document_external_id: String,
    revision_external_id: Option<String>,
    title: String,
    content_type: String,
    body: String,
    metadata: Value,
    acl_snapshot: Option<Value>,
    acl_hash: Option<String>,
}

async fn process_external_source_ingest_task(
    storage: &PgStorage,
    workflow_catalog: &WorkflowCatalog,
    event_bus: &EventBus,
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
    let source_id = match context_string(&execution.context, "source_id")? {
        Some(source_id) => Some(source_id),
        None => context_string(&task.payload, "source_id")?,
    }
    .ok_or_else(|| anyhow!("workflow execution {} missing source_id", execution.id))?;
    let sync_run_id = match context_string(&execution.context, "external_sync_run_id")? {
        Some(sync_run_id) => Some(sync_run_id),
        None => context_string(&task.payload, "external_sync_run_id")?,
    };
    let documents = external_source_documents_from_context(&execution.context, &task.payload)?;

    let process_result: Result<Value> = async {
        let mut document_summaries = Vec::with_capacity(documents.len());
        let mut total_chunks = 0usize;
        for input in documents {
            let document = upsert_external_source_document(
                storage,
                task.tenant_id,
                dataset_id,
                &source_id,
                sync_run_id.as_deref(),
                &input,
            )
            .await?;
            if let Some(acl_snapshot) = input.acl_snapshot.as_ref() {
                upsert_external_permission_snapshot(
                    storage,
                    task.tenant_id,
                    &source_id,
                    &input,
                    acl_snapshot,
                )
                .await?;
            }

            let chunks =
                build_external_source_document_chunks(dataset_id, document.id, &source_id, &input);
            total_chunks += chunks.len();
            storage
                .document_chunks()
                .replace_for_document(task.tenant_id, document.id, &chunks)
                .await?;
            let updated_document = storage
                .documents()
                .update_state(
                    task.tenant_id,
                    document.id,
                    DocumentLifecycle::Extracted,
                    Some(&input.title),
                    &json!({
                        "ingest": {
                            "processor": "external_source_inline",
                            "parse_method": "external_source_inline",
                            "parse_status": "parsed",
                            "chunk_count": chunks.len(),
                            "extracted_chars": input.body.chars().count(),
                            "content_type": input.content_type,
                            "object_key": document.object_key,
                            "extracted_at": Utc::now(),
                        },
                        "parse_status": "parsed",
                        "external_source": external_source_ref(&source_id, sync_run_id.as_deref(), &input),
                    }),
                    Utc::now(),
                )
                .await?;
            document_summaries.push(json!({
                "document_id": updated_document.id,
                "document_external_id": input.document_external_id,
                "revision_external_id": input.revision_external_id,
                "chunk_count": chunks.len(),
                "lifecycle": updated_document.lifecycle.as_str(),
            }));
        }

        let signal_output = json!({
            "source_id": source_id,
            "external_sync_run_id": sync_run_id,
            "document_count": document_summaries.len(),
            "chunk_count": total_chunks,
            "document_ids": document_summaries
                .iter()
                .filter_map(|summary| summary.get("document_id").cloned())
                .collect::<Vec<_>>(),
            "external_documents": document_summaries,
        });
        update_external_sync_run_counts(storage, task.tenant_id, &signal_output).await?;

        Ok(signal_output)
    }
    .await;

    match process_result {
        Ok(signal_output) => {
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
            storage
                .workflow_tasks()
                .mark_succeeded(task.id, Utc::now())
                .await?;
            tracing::info!(
                task_id = %task.id,
                execution_id = %task.execution_id,
                source_id = %source_id,
                "external source ingest task completed"
            );
            Ok(())
        }
        Err(error) => {
            let error_message = error.to_string();
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
                    "ingest worker failed to send external source step_failed signal"
                );
            }
            storage
                .workflow_tasks()
                .mark_failed(task.id, &error_message, Utc::now())
                .await?;
            Err(error)
        }
    }
}

fn build_document_chunks(
    dataset_id: domain_model::DatasetId,
    document_id: domain_model::DocumentId,
    outcome: &IngestOutcome,
) -> Vec<NewDocumentChunk> {
    let created_at = Utc::now();
    let section_title_hints_by_chunk = section_title_hints_for_chunk_sequence(&outcome.chunks, 6);

    outcome
        .chunks
        .iter()
        .enumerate()
        .map(|(index, content)| {
            let section_title_hints = section_title_hints_by_chunk
                .get(index)
                .cloned()
                .unwrap_or_default();
            NewDocumentChunk {
                dataset_id,
                document_id,
                chunk_index: index as i32,
                content: content.clone(),
                token_count: estimate_token_count(content),
                metadata: json!({
                    "extractor": if outcome.used_placeholder { "placeholder" } else { "local_parser" },
                    "parse_method": outcome.parse_method.clone(),
                    "parse_status": outcome.parse_status(),
                    "parse_metadata": outcome.metadata.clone(),
                    "section_title_hints": section_title_hints,
                    "source": "upload_ingest_workflow",
                }),
                created_at,
            }
        })
        .collect()
}

fn build_external_source_document_chunks(
    dataset_id: domain_model::DatasetId,
    document_id: domain_model::DocumentId,
    source_id: &str,
    input: &ExternalSourceDocumentInput,
) -> Vec<NewDocumentChunk> {
    let created_at = Utc::now();
    let external_source = external_source_ref(source_id, None, input);
    let external_acl = external_acl_ref(source_id, input);
    let chunks = split_text_chunks(&input.body, 1_800);
    let section_title_hints_by_chunk = section_title_hints_for_chunk_sequence(&chunks, 6);

    chunks
        .into_iter()
        .enumerate()
        .map(|(index, content)| {
            let section_title_hints = section_title_hints_by_chunk
                .get(index)
                .cloned()
                .unwrap_or_default();
            NewDocumentChunk {
                dataset_id,
                document_id,
                chunk_index: index as i32,
                token_count: estimate_token_count(&content),
                content,
                metadata: json!({
                    "extractor": "external_source_inline",
                    "parse_method": "external_source_inline",
                    "parse_status": "parsed",
                    "source": "external_source_sync_workflow",
                    "external_source": external_source,
                    "external_acl": external_acl,
                    "section_title_hints": section_title_hints,
                    "parse_metadata": input.metadata.clone(),
                }),
                created_at,
            }
        })
        .collect()
}

async fn upsert_external_source_document(
    storage: &PgStorage,
    tenant_id: domain_model::TenantId,
    dataset_id: domain_model::DatasetId,
    source_id: &str,
    sync_run_id: Option<&str>,
    input: &ExternalSourceDocumentInput,
) -> Result<Document> {
    if let Some(document) =
        find_external_source_document(storage, tenant_id, dataset_id, source_id, input).await?
    {
        return Ok(document);
    }

    storage
        .documents()
        .create(
            tenant_id,
            NewDocument {
                dataset_id,
                title: input.title.clone(),
                object_key: external_source_object_key(source_id, input),
                content_type: input.content_type.clone(),
                secret_binding_ids: Vec::new(),
                owner_user_id: None,
                metadata: json!({
                    "external_source": external_source_ref(source_id, sync_run_id, input),
                    "external_metadata": input.metadata.clone(),
                }),
            },
        )
        .await
}

async fn find_external_source_document(
    storage: &PgStorage,
    tenant_id: domain_model::TenantId,
    dataset_id: domain_model::DatasetId,
    source_id: &str,
    input: &ExternalSourceDocumentInput,
) -> Result<Option<Document>> {
    let documents = storage
        .documents()
        .list_by_dataset(tenant_id, dataset_id)
        .await?;
    Ok(documents.into_iter().find(|document| {
        document
            .metadata
            .get("external_source")
            .and_then(Value::as_object)
            .is_some_and(|metadata| {
                metadata.get("source_id").and_then(Value::as_str) == Some(source_id)
                    && metadata.get("document_external_id").and_then(Value::as_str)
                        == Some(input.document_external_id.as_str())
            })
    }))
}

async fn upsert_external_permission_snapshot(
    storage: &PgStorage,
    tenant_id: domain_model::TenantId,
    source_id: &str,
    input: &ExternalSourceDocumentInput,
    acl_snapshot: &Value,
) -> Result<()> {
    let now = Utc::now();
    let updated = sqlx::query(
        r#"
        update external_permission_snapshots
        set acl_snapshot = $5,
            acl_hash = $6,
            captured_at = $7
        where tenant_id = $1
          and source_id = $2
          and document_external_id = $3
          and coalesce(revision_external_id, '') = coalesce($4::text, '')
        "#,
    )
    .bind(tenant_id.0)
    .bind(source_id)
    .bind(&input.document_external_id)
    .bind(&input.revision_external_id)
    .bind(acl_snapshot)
    .bind(&input.acl_hash)
    .bind(now)
    .execute(storage.pool())
    .await?;

    if updated.rows_affected() == 0 {
        sqlx::query(
            r#"
            insert into external_permission_snapshots (
                tenant_id,
                source_id,
                document_external_id,
                revision_external_id,
                acl_snapshot,
                acl_hash,
                captured_at
            )
            values ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(tenant_id.0)
        .bind(source_id)
        .bind(&input.document_external_id)
        .bind(&input.revision_external_id)
        .bind(acl_snapshot)
        .bind(&input.acl_hash)
        .bind(now)
        .execute(storage.pool())
        .await?;
    }

    Ok(())
}

async fn update_external_sync_run_counts(
    storage: &PgStorage,
    tenant_id: domain_model::TenantId,
    signal_output: &Value,
) -> Result<()> {
    let Some(sync_run_id) = signal_output
        .get("external_sync_run_id")
        .and_then(Value::as_str)
        .and_then(|raw| uuid::Uuid::parse_str(raw).ok())
    else {
        return Ok(());
    };

    sqlx::query(
        r#"
        update external_sync_runs
        set counts = counts || $3,
            updated_at = $4
        where tenant_id = $1 and id = $2
        "#,
    )
    .bind(tenant_id.0)
    .bind(sync_run_id)
    .bind(json!({
        "documents_ingested": signal_output
            .get("document_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        "chunks_ingested": signal_output
            .get("chunk_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
    }))
    .bind(Utc::now())
    .execute(storage.pool())
    .await?;

    Ok(())
}

fn external_source_documents_from_context(
    context: &Value,
    task_payload: &Value,
) -> Result<Vec<ExternalSourceDocumentInput>> {
    let candidates = [
        context.pointer("/last_output/external_documents"),
        context.pointer("/last_output/documents"),
        task_payload.get("external_documents"),
        task_payload.get("documents"),
    ];
    let documents = candidates
        .into_iter()
        .flatten()
        .find_map(Value::as_array)
        .ok_or_else(|| anyhow!("external source ingest requires external_documents array"))?;

    if documents.is_empty() {
        return Err(anyhow!(
            "external source ingest requires at least one external document"
        ));
    }

    documents
        .iter()
        .map(external_source_document_from_value)
        .collect()
}

fn external_source_document_from_value(value: &Value) -> Result<ExternalSourceDocumentInput> {
    let object = value
        .as_object()
        .ok_or_else(|| anyhow!("external document must be a JSON object"))?;
    let document_external_id = string_field(value, &["document_external_id", "documentExternalId"])
        .ok_or_else(|| anyhow!("external document missing document_external_id"))?;
    let body = string_field(value, &["body", "content", "text"])
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            anyhow!("external document {document_external_id} missing non-empty body")
        })?;
    let title = string_field(value, &["title", "name"])
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| document_external_id.clone());
    let content_type = string_field(value, &["content_type", "contentType"])
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "text/markdown".to_string());
    let revision_external_id = string_field(value, &["revision_external_id", "revisionExternalId"]);
    let metadata = object
        .get("metadata")
        .filter(|value| value.is_object())
        .cloned()
        .unwrap_or_else(|| json!({}));
    let acl_snapshot = object
        .get("acl_snapshot")
        .or_else(|| object.get("aclSnapshot"))
        .or_else(|| object.get("acl"))
        .filter(|value| value.is_object())
        .cloned();
    let acl_hash = string_field(value, &["acl_hash", "aclHash"]);

    Ok(ExternalSourceDocumentInput {
        document_external_id,
        revision_external_id,
        title,
        content_type,
        body,
        metadata,
        acl_snapshot,
        acl_hash,
    })
}

fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(ToString::to_string)
}

fn context_string(value: &Value, key: &str) -> Result<Option<String>> {
    match value {
        Value::Object(map) => Ok(map
            .get(key)
            .and_then(Value::as_str)
            .map(ToString::to_string)),
        Value::Null => Ok(None),
        _ => Err(anyhow!("workflow execution context must be a JSON object")),
    }
}

fn external_source_ref(
    source_id: &str,
    sync_run_id: Option<&str>,
    input: &ExternalSourceDocumentInput,
) -> Value {
    json!({
        "source_id": source_id,
        "document_external_id": input.document_external_id,
        "revision_external_id": input.revision_external_id,
        "external_sync_run_id": sync_run_id,
        "synced_at": Utc::now(),
    })
}

fn external_acl_ref(source_id: &str, input: &ExternalSourceDocumentInput) -> Value {
    json!({
        "source_id": source_id,
        "document_external_id": input.document_external_id,
        "revision_external_id": input.revision_external_id,
        "acl_hash": input.acl_hash,
    })
}

fn external_source_object_key(source_id: &str, input: &ExternalSourceDocumentInput) -> String {
    match input.revision_external_id.as_deref() {
        Some(revision) if !revision.is_empty() => {
            format!(
                "external/{}/{}/{}",
                source_id, input.document_external_id, revision
            )
        }
        _ => format!("external/{}/{}", source_id, input.document_external_id),
    }
}

fn estimate_token_count(content: &str) -> i32 {
    let estimated = (content.chars().count() / 4).max(1);
    estimated.try_into().unwrap_or(i32::MAX)
}

fn section_title_hints_from_text(text: &str, limit: usize) -> Vec<String> {
    let mut hints = Vec::new();
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if let Some(title) = normalize_section_title_hint(line) {
            if !hints.contains(&title) {
                hints.push(title);
            }
            if hints.len() >= limit {
                break;
            }
        }
    }
    hints
}

fn section_title_hints_for_chunk_sequence(chunks: &[String], limit: usize) -> Vec<Vec<String>> {
    let mut active_hints = Vec::new();
    chunks
        .iter()
        .map(|chunk| {
            let direct_hints = section_title_hints_from_text(chunk, limit);
            if direct_hints.is_empty() {
                active_hints.clone()
            } else {
                active_hints = direct_hints;
                active_hints.clone()
            }
        })
        .collect()
}

fn normalize_section_title_hint(line: &str) -> Option<String> {
    let trimmed = line.trim().trim_matches(|ch: char| ch == '*' || ch == '`');
    if trimmed.is_empty() {
        return None;
    }
    let candidate = trimmed
        .strip_prefix('#')
        .map(|value| value.trim_start_matches('#').trim())
        .or_else(|| {
            if let Some((index, _)) = trimmed
                .char_indices()
                .find(|(_, value)| value.is_whitespace())
            {
                let marker = trimmed[..index].trim();
                if marker.chars().count() <= 12 && looks_like_heading_marker(marker) {
                    return Some(trimmed[index..].trim());
                }
            }
            let marker_end = trimmed
                .char_indices()
                .find_map(|(index, value)| {
                    if matches!(value, '、' | '.' | '．' | ')' | '）' | ':' | '：') {
                        Some(index + value.len_utf8())
                    } else {
                        None
                    }
                })
                .filter(|index| *index <= 12)?;
            let marker = trimmed[..marker_end].trim();
            looks_like_heading_marker(marker).then(|| trimmed[marker_end..].trim())
        })
        .or_else(|| {
            (trimmed.starts_with('第')
                && trimmed
                    .chars()
                    .take(8)
                    .any(|value| value == '章' || value == '节'))
            .then_some(trimmed)
        })
        .or_else(|| looks_like_standalone_heading(trimmed).then_some(trimmed))?;
    let normalized = candidate
        .trim_matches(|ch: char| ch.is_whitespace() || "#*-_".contains(ch))
        .chars()
        .take(80)
        .collect::<String>();
    (!normalized.is_empty()).then_some(normalized)
}

fn looks_like_heading_marker(value: &str) -> bool {
    value.chars().any(|ch| ch.is_ascii_digit())
        || value.chars().any(|ch| "一二三四五六七八九十".contains(ch))
}

fn looks_like_standalone_heading(value: &str) -> bool {
    let char_count = value.chars().count();
    char_count >= 2
        && char_count <= 32
        && !value.ends_with('。')
        && !value.ends_with('！')
        && !value.ends_with('？')
        && !value.ends_with(';')
        && !value.ends_with('；')
        && !value.contains('|')
}

fn context_uuid_string(value: &Value, key: &str) -> Result<Option<uuid::Uuid>> {
    match value {
        Value::Object(map) => match map.get(key).and_then(Value::as_str) {
            Some(raw) => Ok(Some(uuid::Uuid::parse_str(raw)?)),
            None => Ok(None),
        },
        Value::Null => Ok(None),
        _ => Err(anyhow!("workflow execution context must be a JSON object")),
    }
}

fn optional_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && value != "*")
}

async fn wait_for_next_task_signal(task_waker: &mut EventSubscription, poll_interval_ms: u64) {
    if let Some(event) = task_waker
        .wait_for_event(Duration::from_millis(poll_interval_ms))
        .await
    {
        tracing::debug!(subject = %event.subject, "ingest worker received task wake signal");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain_model::{
        DatasetId, DocumentId, WorkflowEventId, WorkflowEventRecord, WorkflowExecution,
        WorkflowExecutionId, WorkflowKind, WorkflowStatus, WorkflowTaskStatus,
    };
    use storage::{NewDataset, NewDocument};
    use test_fixtures::{
        local_postgres_storage, reset_local_postgres_storage, shared_local_postgres_test_lock,
    };

    #[derive(Clone, Debug)]
    struct StaticIngestProcessor {
        outcome: IngestOutcome,
    }

    impl IngestProcessor for StaticIngestProcessor {
        fn process(&self, _job: &IngestJob) -> IngestOutcome {
            self.outcome.clone()
        }
    }

    #[test]
    fn auto_reparse_decision_ignores_healthy_parse() {
        let decision =
            auto_reparse_decision_for_attempts("parsed", Some("usable_text"), 0, true, 1);

        assert_eq!(decision, None);
    }

    #[test]
    fn auto_reparse_decision_queues_degraded_parse_once() {
        let decision = auto_reparse_decision_for_attempts(
            "parse_degraded",
            Some("low_text_coverage"),
            0,
            true,
            1,
        )
        .expect("degraded parse should request reparse");

        assert!(decision.should_retry);
        assert_eq!(decision.status, "queued");
        assert_eq!(decision.attempt_count, 1);
        assert_eq!(decision.max_attempts, 1);
        assert!(decision.reason.contains("parse_status=parse_degraded"));
        assert!(decision.reason.contains("attempt=1/1"));
    }

    #[test]
    fn auto_reparse_decision_exhausts_after_attempt_limit() {
        let decision = auto_reparse_decision_for_attempts(
            "parse_degraded",
            Some("low_text_coverage"),
            1,
            true,
            1,
        )
        .expect("degraded parse should be classified");

        assert!(!decision.should_retry);
        assert_eq!(decision.status, "exhausted");
        assert_eq!(decision.attempt_count, 1);
        assert!(decision.reason.contains("attempts=1/1"));
    }

    #[test]
    fn auto_reparse_decision_can_be_disabled() {
        let decision = auto_reparse_decision_for_attempts("placeholder", None, 0, false, 1)
            .expect("placeholder parse should be classified");

        assert!(!decision.should_retry);
        assert_eq!(decision.status, "disabled");
        assert_eq!(decision.attempt_count, 0);
        assert!(decision.reason.contains("auto reparse is disabled"));
    }

    #[tokio::test]
    async fn degraded_uploaded_document_is_failed_and_requeued_for_auto_reparse() {
        let _guard = shared_local_postgres_test_lock().lock().await;
        let storage = match local_postgres_storage().await {
            Ok(storage) => storage,
            Err(reason) => {
                eprintln!("skipping ingest auto reparse workflow test: {reason}");
                return;
            }
        };
        reset_local_postgres_storage(&storage)
            .await
            .expect("test storage should reset");

        let tenant = storage
            .ensure_tenant(
                &format!("ingest-auto-reparse-test-{}", uuid::Uuid::new_v4()),
                "Ingest Auto Reparse Test",
            )
            .await
            .expect("tenant should exist");
        let dataset = storage
            .datasets()
            .create(
                tenant.id,
                NewDataset {
                    key: format!("auto-reparse-{}", uuid::Uuid::new_v4()),
                    title: "Auto reparse dataset".to_string(),
                    description: Some("Documents that need a parse retry.".to_string()),
                    owner_user_id: None,
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
                    title: "One character customer PDF".to_string(),
                    object_key: "documents/one-character.pdf".to_string(),
                    content_type: "application/pdf".to_string(),
                    secret_binding_ids: Vec::new(),
                    owner_user_id: None,
                    metadata: json!({}),
                },
            )
            .await
            .expect("document should be created");
        let workflow_catalog = workflow_definitions::catalog();
        let execution = test_upload_ingest_execution(tenant.id, &workflow_catalog, &document);
        let initial_event = test_upload_ingest_event(&execution, &document);
        storage
            .workflow_executions()
            .create_with_initial_event(&execution, &initial_event)
            .await
            .expect("workflow execution should be created");
        platform_api::apply_workflow_signal_with_dependencies(
            &storage,
            &workflow_catalog,
            &EventBus::Disabled,
            tenant.id,
            execution.id,
            WorkflowSignal::Start,
        )
        .await
        .expect("workflow should start");
        let first_task = storage
            .workflow_tasks()
            .list_by_execution(execution.id)
            .await
            .expect("tasks should load")
            .into_iter()
            .next()
            .expect("start should enqueue ingest task");
        let processor = StaticIngestProcessor {
            outcome: IngestOutcome {
                chunks: vec!["PDF parse quality warning: extracted text was too short.".to_string()],
                inferred_title: None,
                parse_method: "pdf-paddleocr+low-quality".to_string(),
                extracted_chars: 1,
                used_placeholder: false,
                metadata: json!({
                    "parse_quality": {
                        "kind": "pdf_text_extraction",
                        "status": "low_text_coverage_fallback_unavailable",
                        "text_chars": 1,
                        "min_usable_text_chars": 32,
                        "fallback_status": "unavailable"
                    }
                }),
            },
        };

        process_uploaded_document_task(
            &storage,
            &workflow_catalog,
            &EventBus::Disabled,
            &processor,
            first_task,
        )
        .await
        .expect("degraded parse should be handled as controlled reparse");

        let updated_document = storage
            .documents()
            .get_by_id(tenant.id, document.id)
            .await
            .expect("document should load")
            .expect("document should exist");
        assert_eq!(updated_document.lifecycle, DocumentLifecycle::Failed);
        assert_eq!(
            updated_document.metadata.get("parse_status"),
            Some(&json!("parse_degraded"))
        );
        assert_eq!(
            updated_document
                .metadata
                .get("ingest")
                .and_then(|value| value.pointer("/auto_reparse/status")),
            Some(&json!("queued"))
        );
        assert_eq!(
            updated_document
                .metadata
                .get("ingest")
                .and_then(|value| value.pointer("/auto_reparse/attempt_count")),
            Some(&json!(1))
        );

        let updated_execution = storage
            .workflow_executions()
            .get_by_id(tenant.id, execution.id)
            .await
            .expect("execution should load")
            .expect("execution should exist");
        assert_eq!(updated_execution.status, WorkflowStatus::Running);
        assert_eq!(updated_execution.stage, DEFAULT_WAKE_TASK_KEY);
        let tasks = storage
            .workflow_tasks()
            .list_by_execution(execution.id)
            .await
            .expect("tasks should load");
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].status, WorkflowTaskStatus::Failed);
        assert_eq!(tasks[1].status, WorkflowTaskStatus::Queued);
        assert_eq!(tasks[1].task_key, DEFAULT_WAKE_TASK_KEY);
    }

    fn test_upload_ingest_execution(
        tenant_id: domain_model::TenantId,
        workflow_catalog: &WorkflowCatalog,
        document: &Document,
    ) -> WorkflowExecution {
        let definition = workflow_catalog
            .find_definition(WorkflowKind::UploadIngest)
            .expect("upload ingest workflow definition should exist");
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

        WorkflowExecution {
            id: execution_id,
            tenant_id,
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
        }
    }

    fn test_upload_ingest_event(
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

    #[test]
    fn external_source_documents_parse_from_workflow_last_output() {
        let context = json!({
            "last_output": {
                "external_documents": [{
                    "document_external_id": "doc-001",
                    "revision_external_id": "rev-7",
                    "title": "采购制度",
                    "content_type": "text/markdown",
                    "body": "第一章 采购审批。\n\n第二章 供应商准入。",
                    "metadata": {
                        "path": "/wiki/purchase"
                    },
                    "acl_snapshot": {
                        "allowed_group_external_ids": ["procurement"]
                    },
                    "acl_hash": "acl-001"
                }]
            }
        });

        let documents = external_source_documents_from_context(&context, &json!({}))
            .expect("external documents should parse");

        assert_eq!(documents.len(), 1);
        assert_eq!(documents[0].document_external_id, "doc-001");
        assert_eq!(documents[0].revision_external_id.as_deref(), Some("rev-7"));
        assert_eq!(documents[0].title, "采购制度");
        assert_eq!(documents[0].acl_hash.as_deref(), Some("acl-001"));
        assert_eq!(
            documents[0]
                .acl_snapshot
                .as_ref()
                .and_then(|value| value.get("allowed_group_external_ids"))
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(1)
        );
    }

    #[test]
    fn external_source_chunks_carry_acl_reference_for_retrieval_gate() {
        let input = external_source_document_from_value(&json!({
            "document_external_id": "doc-002",
            "revision_external_id": "rev-8",
            "title": "订单风险制度",
            "body": "## 固定资产申请\n\n订单延期超过两天需要赔付提醒。",
            "acl_hash": "acl-002"
        }))
        .expect("external document should parse");

        let chunks = build_external_source_document_chunks(
            DatasetId::new(),
            DocumentId::new(),
            "src-docs",
            &input,
        );

        assert_eq!(chunks.len(), 1);
        assert_eq!(
            chunks[0].metadata["source"],
            json!("external_source_sync_workflow")
        );
        assert_eq!(
            chunks[0].metadata["external_acl"]["source_id"],
            json!("src-docs")
        );
        assert_eq!(
            chunks[0].metadata["external_acl"]["document_external_id"],
            json!("doc-002")
        );
        assert_eq!(
            chunks[0].metadata["external_acl"]["revision_external_id"],
            json!("rev-8")
        );
        assert_eq!(
            chunks[0].metadata["section_title_hints"],
            json!(["固定资产申请"])
        );
    }

    #[test]
    fn section_title_hints_detect_common_heading_shapes() {
        let hints = section_title_hints_from_text(
            "# 固定资产申请\n正文第一句。\n\n1.2 审批流程\n审批说明。\n\n这里是一句普通正文。",
            8,
        );

        assert_eq!(hints, vec!["固定资产申请", "审批流程"]);
    }

    #[test]
    fn build_document_chunks_inherit_section_title_hints_across_chunk_sequence() {
        let outcome = IngestOutcome {
            chunks: vec![
                "# 固定资产申请\n提交前需要填写申请单。".to_string(),
                "审批通过后由行政登记资产编号。".to_string(),
                "## 报废处理\n报废前需要主管确认。".to_string(),
                "财务完成残值核算。".to_string(),
            ],
            inferred_title: None,
            parse_method: "local-text.md".to_string(),
            extracted_chars: 64,
            used_placeholder: false,
            metadata: json!({}),
        };

        let chunks = build_document_chunks(
            domain_model::DatasetId::new(),
            domain_model::DocumentId::new(),
            &outcome,
        );

        assert_eq!(
            chunks[0].metadata["section_title_hints"],
            json!(["固定资产申请"])
        );
        assert_eq!(chunks[0].metadata["parse_status"], json!("parsed"));
        assert_eq!(
            chunks[1].metadata["section_title_hints"],
            json!(["固定资产申请"])
        );
        assert_eq!(
            chunks[2].metadata["section_title_hints"],
            json!(["报废处理"])
        );
        assert_eq!(
            chunks[3].metadata["section_title_hints"],
            json!(["报废处理"])
        );
    }
}
