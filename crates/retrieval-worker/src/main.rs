use anyhow::{anyhow, Result};
use chrono::Utc;
use domain_model::DocumentLifecycle;
use event_bus::{workflow_task_enqueued_subject, EventBus, EventSubscription};
use retrieval_worker::{PlaceholderRetrievalIndexer, RetrievalIndexJob, RetrievalIndexer};
use serde_json::json;
use storage::{NewRetrievalEvidence, PgStorage, DEFAULT_LOCAL_DATABASE_URL};
use tokio::time::Duration;
use workflow_engine::{WorkflowCatalog, WorkflowSignal};

const DEFAULT_QUEUE: &str = "retrieval";
const DEFAULT_TASK_KEY: &str = "index_retrieval_artifacts";
const DEFAULT_POLL_INTERVAL_MS: u64 = 1_000;

#[tokio::main]
async fn main() -> Result<()> {
    observability::install("retrieval_worker")?;

    let database_url = std::env::var("PLATFORM_DATABASE_URL")
        .unwrap_or_else(|_| DEFAULT_LOCAL_DATABASE_URL.to_string());
    let queue = std::env::var("RETRIEVAL_QUEUE").unwrap_or_else(|_| DEFAULT_QUEUE.to_string());
    let task_key =
        std::env::var("RETRIEVAL_TASK_KEY").unwrap_or_else(|_| DEFAULT_TASK_KEY.to_string());
    let poll_interval = std::env::var("RETRIEVAL_POLL_INTERVAL_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_POLL_INTERVAL_MS);

    let storage = PgStorage::connect(&database_url).await?;
    let workflow_catalog = workflow_definitions::catalog();
    let event_bus = EventBus::connect_from_env_or_disabled("PLATFORM_NATS_URL").await;
    let indexer = PlaceholderRetrievalIndexer;
    let wake_subject = workflow_task_enqueued_subject(&queue, &task_key);
    let mut task_waker = event_bus
        .subscribe_queue_or_disabled(
            &wake_subject,
            Some(&format!("retrieval_worker.{queue}.{task_key}")),
        )
        .await;

    tracing::info!(
        %queue,
        %task_key,
        %wake_subject,
        event_bus_enabled = event_bus.is_enabled(),
        poll_interval_ms = poll_interval,
        %database_url,
        "retrieval-worker polling started"
    );

    loop {
        match storage
            .workflow_tasks()
            .claim_next_available(&queue, Some(&task_key), Utc::now())
            .await
        {
            Ok(Some(task)) => {
                if let Err(error) =
                    process_task(&storage, &workflow_catalog, &event_bus, &indexer, task).await
                {
                    tracing::error!(error = ?error, "retrieval task processing failed");
                }
            }
            Ok(None) => {
                wait_for_next_task_signal(&mut task_waker, poll_interval).await;
            }
            Err(error) => {
                tracing::error!(error = ?error, "retrieval worker failed to claim task");
                wait_for_next_task_signal(&mut task_waker, poll_interval).await;
            }
        }
    }
}

async fn process_task(
    storage: &PgStorage,
    workflow_catalog: &WorkflowCatalog,
    event_bus: &EventBus,
    indexer: &impl RetrievalIndexer,
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
    let document_id = execution
        .context
        .get("document_id")
        .and_then(|value| value.as_str())
        .ok_or_else(|| anyhow!("workflow execution {} missing document_id", execution.id))
        .and_then(|raw| {
            raw.parse::<uuid::Uuid>()
                .map(domain_model::DocumentId)
                .map_err(|error| anyhow!("invalid document_id {raw}: {error}"))
        })?;
    let document = storage
        .documents()
        .get_by_id(task.tenant_id, document_id)
        .await?
        .ok_or_else(|| anyhow!("document {} not found", document_id))?;
    let chunks = storage
        .document_chunks()
        .list_by_document(task.tenant_id, document_id)
        .await?;
    if chunks.is_empty() {
        return Err(anyhow!(
            "document {} has no extracted chunks to index",
            document_id
        ));
    }

    let process_result: Result<()> = async {
        let indexed_at = Utc::now();
        let outcome = indexer.index(&RetrievalIndexJob {
            dataset_id,
            document_id,
            chunk_count: chunks.len(),
        });
        let embedding_model = outcome.embedding_model.clone();
        let payload_filter_key = outcome.payload_filter_key.clone();
        let retrieval_evidences = storage
            .retrieval_evidences()
            .create_many(
                task.tenant_id,
                &chunks
                    .iter()
                    .map(|chunk| NewRetrievalEvidence {
                        execution_id: task.execution_id,
                        dataset_id,
                        document_id,
                        document_chunk_id: chunk.id,
                        chunk_index: chunk.chunk_index,
                        source_locator: format!(
                            "document://{document_id}/chunks/{}",
                            chunk.chunk_index
                        ),
                        content_excerpt: excerpt(&chunk.content, 240),
                        summary: format!(
                            "{} chunk {} indexed for placeholder retrieval recall.",
                            document.title, chunk.chunk_index
                        ),
                        payload_filter_key: payload_filter_key.clone(),
                        embedding_model: embedding_model.clone(),
                        recall_score: placeholder_recall_score(chunk.chunk_index),
                        evidence_manifest: json!({
                            "schema_version": "0.3.0",
                            "generator": "retrieval-worker",
                            "dataset_id": dataset_id,
                            "document_id": document_id,
                            "document_chunk_id": chunk.id,
                            "chunk_index": chunk.chunk_index,
                            "indexed_at": indexed_at,
                            "embedding": {
                                "status": "indexed",
                                "model": &embedding_model,
                                "token_count": chunk.token_count,
                            },
                            "recall": {
                                "status": "ready",
                                "score": placeholder_recall_score(chunk.chunk_index),
                                "rank_hint": chunk.chunk_index + 1,
                            },
                            "evidence": {
                                "document_chunk_id": chunk.id,
                                "payload_filter_key": &payload_filter_key,
                                "source_locator": format!(
                                    "document://{document_id}/chunks/{}",
                                    chunk.chunk_index
                                ),
                            },
                        }),
                        created_at: indexed_at,
                    })
                    .collect::<Vec<_>>(),
            )
            .await?;
        let indexed_chunks = storage
            .document_chunks()
            .mark_indexed(
                task.tenant_id,
                document_id,
                &json!({
                    "retrieval": {
                        "indexer": "placeholder",
                        "indexed_at": indexed_at,
                        "embedding_model": &embedding_model,
                        "payload_filter_key": &payload_filter_key,
                        "retrieval_evidence_count": retrieval_evidences.len(),
                    }
                }),
                indexed_at,
            )
            .await?;
        let updated_document = storage
            .documents()
            .update_state(
                task.tenant_id,
                document_id,
                DocumentLifecycle::Indexed,
                Some(&document.title),
                &json!({
                    "retrieval": {
                        "indexer": "placeholder",
                        "indexed_at": indexed_at,
                        "embedding_model": &embedding_model,
                        "embedded_chunks": outcome.embedded_chunks,
                        "payload_filter_key": &payload_filter_key,
                        "retrieval_evidence_count": retrieval_evidences.len(),
                        "latest_execution_id": task.execution_id,
                    }
                }),
                indexed_at,
            )
            .await?;
        let signal_output = json!({
            "document_id": updated_document.id,
            "dataset_id": updated_document.dataset_id,
            "embedded_chunks": outcome.embedded_chunks,
            "indexed_chunk_count": indexed_chunks.len(),
            "retrieval_evidence_count": retrieval_evidences.len(),
            "payload_filter_key": payload_filter_key,
            "lifecycle": updated_document.lifecycle.as_str(),
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
        let error_message = error.to_string();
        let _ = storage
            .documents()
            .update_state(
                task.tenant_id,
                document_id,
                DocumentLifecycle::Failed,
                Some(&document.title),
                &json!({
                    "retrieval": {
                        "indexer": "placeholder",
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
                "retrieval worker failed to send workflow step_failed signal"
            );
        }

        storage
            .workflow_tasks()
            .mark_failed(task.id, &error_message, Utc::now())
            .await?;
        return Err(error);
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
        "retrieval task completed"
    );

    Ok(())
}

fn excerpt(content: &str, max_chars: usize) -> String {
    let trimmed = content.trim();
    let total_chars = trimmed.chars().count();
    let mut value = trimmed.chars().take(max_chars).collect::<String>();
    if total_chars > max_chars {
        value.push_str("...");
    }
    value
}

fn placeholder_recall_score(chunk_index: i32) -> f64 {
    let rank = (chunk_index.max(0) + 1) as f64;
    (1.0 / rank * 100.0).round() / 100.0
}

async fn wait_for_next_task_signal(task_waker: &mut EventSubscription, poll_interval_ms: u64) {
    if let Some(event) = task_waker
        .wait_for_event(Duration::from_millis(poll_interval_ms))
        .await
    {
        tracing::debug!(subject = %event.subject, "retrieval worker received task wake signal");
    }
}
