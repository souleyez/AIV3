use anyhow::{anyhow, Result};
use chrono::Utc;
use domain_model::{Document, DocumentChunk, DocumentLifecycle, WorkflowTask};
use event_bus::{workflow_task_enqueued_subject, EventBus, EventSubscription};
use retrieval_worker::{
    LocalLexicalRetrievalIndexer, RetrievalChunkInput, RetrievalIndexJob, RetrievalIndexer,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use storage::{NewRetrievalEvidence, PgStorage, DEFAULT_LOCAL_DATABASE_URL};
use tokio::time::Duration;
use workflow_engine::{WorkflowCatalog, WorkflowSignal};

const DEFAULT_QUEUE: &str = "retrieval";
const EXTERNAL_SOURCE_INDEX_TASK_KEY: &str = "index_external_retrieval";
const POST_INGEST_FACT_INDEX_TASK_KEY: &str = "cleanup_document_facts";
const DEFAULT_POLL_INTERVAL_MS: u64 = 1_000;

#[tokio::main]
async fn main() -> Result<()> {
    observability::install("retrieval_worker")?;

    let database_url = std::env::var("PLATFORM_DATABASE_URL")
        .unwrap_or_else(|_| DEFAULT_LOCAL_DATABASE_URL.to_string());
    let queue = std::env::var("RETRIEVAL_QUEUE").unwrap_or_else(|_| DEFAULT_QUEUE.to_string());
    let task_key = optional_env("RETRIEVAL_TASK_KEY");
    let poll_interval = std::env::var("RETRIEVAL_POLL_INTERVAL_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_POLL_INTERVAL_MS);

    let storage = PgStorage::connect(&database_url).await?;
    let workflow_catalog = workflow_definitions::catalog();
    let event_bus = EventBus::connect_from_env_or_disabled("PLATFORM_NATS_URL").await;
    let indexer = LocalLexicalRetrievalIndexer;
    let wake_subject = task_key
        .as_deref()
        .map(|task_key| workflow_task_enqueued_subject(&queue, task_key))
        .unwrap_or_else(|| format!("workflow.task.enqueued.{queue}.>"));
    let mut task_waker = event_bus
        .subscribe_queue_or_disabled(
            &wake_subject,
            Some(&format!(
                "retrieval_worker.{queue}.{}",
                task_key.as_deref().unwrap_or("*")
            )),
        )
        .await;

    tracing::info!(
        %queue,
        task_key = task_key.as_deref().unwrap_or("*"),
        %wake_subject,
        event_bus_enabled = event_bus.is_enabled(),
        poll_interval_ms = poll_interval,
        %database_url,
        "retrieval-worker polling started"
    );

    loop {
        match storage
            .workflow_tasks()
            .claim_next_available(&queue, task_key.as_deref(), Utc::now())
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
    if task.task_key == EXTERNAL_SOURCE_INDEX_TASK_KEY {
        return process_external_source_index_task(
            storage,
            workflow_catalog,
            event_bus,
            indexer,
            task,
        )
        .await;
    }
    if task.task_key == POST_INGEST_FACT_INDEX_TASK_KEY {
        return process_post_ingest_fact_index_task(storage, task).await;
    }

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
            chunks: chunks
                .iter()
                .map(|chunk| RetrievalChunkInput {
                    chunk_index: chunk.chunk_index,
                    content: retrieval_index_text(&document, chunk),
                    token_count: chunk.token_count.max(0) as usize,
                })
                .collect(),
        });
        let embedding_model = outcome.embedding_model.clone();
        let payload_filter_key = outcome.payload_filter_key.clone();
        let chunk_profiles = BTreeMap::from_iter(
            outcome
                .chunk_profiles
                .iter()
                .cloned()
                .map(|profile| (profile.chunk_index, profile)),
        );
        let mut new_retrieval_evidences = Vec::with_capacity(chunks.len());
        for chunk in &chunks {
            let profile = chunk_profiles.get(&chunk.chunk_index).ok_or_else(|| {
                anyhow!(
                    "retrieval lexical profile missing for document {} chunk {}",
                    document_id,
                    chunk.chunk_index
                )
            })?;
            let source_locator = retrieval_source_locator(document_id, chunk);
            let section_title_hints = document_chunk_section_title_hints(chunk);
            let noun_terms = document_chunk_noun_terms(chunk);
            let mut evidence_manifest = json!({
                "schema_version": "0.4.0",
                "generator": "retrieval-worker",
                "dataset_id": dataset_id,
                "document_id": document_id,
                "document_chunk_id": chunk.id,
                "chunk_index": chunk.chunk_index,
                "indexed_at": indexed_at,
                "embedding": {
                    "status": "indexed",
                    "model": &embedding_model,
                    "token_count": profile.token_count,
                    "strategy": "local_lexical_v1",
                    "signature_terms": profile.signature_terms,
                    "term_weights": profile.term_weights,
                    "vector_norm": profile.vector_norm,
                },
                "recall": {
                    "status": "ready",
                    "score": profile.recall_score,
                    "rank_hint": profile.rank_hint,
                },
                "evidence": {
                    "document_chunk_id": chunk.id,
                    "payload_filter_key": &payload_filter_key,
                    "source_locator": source_locator.clone(),
                    "section_title_hints": &section_title_hints,
                    "noun_terms": &noun_terms,
                },
            });
            if let Some(media_manifest) = retrieval_media_manifest(chunk) {
                if let Some(object) = evidence_manifest.as_object_mut() {
                    object.insert("media".to_string(), media_manifest);
                }
            }
            if let Some(external_acl) = retrieval_external_acl_manifest(&document, chunk) {
                if let Some(object) = evidence_manifest.as_object_mut() {
                    object.insert("external_acl".to_string(), external_acl);
                }
            }
            new_retrieval_evidences.push(NewRetrievalEvidence {
                execution_id: task.execution_id,
                dataset_id,
                document_id,
                document_chunk_id: chunk.id,
                chunk_index: chunk.chunk_index,
                source_locator,
                content_excerpt: excerpt(&chunk.content, 240),
                summary: retrieval_evidence_summary(
                    &document,
                    chunk,
                    &section_title_hints,
                    "lexical retrieval recall",
                ),
                payload_filter_key: payload_filter_key.clone(),
                embedding_model: embedding_model.clone(),
                recall_score: profile.recall_score,
                evidence_manifest,
                created_at: indexed_at,
            });
        }
        let retrieval_evidences = storage
            .retrieval_evidences()
            .create_many(task.tenant_id, &new_retrieval_evidences)
            .await?;
        let indexed_chunks = storage
            .document_chunks()
            .mark_indexed(
                task.tenant_id,
                document_id,
                &json!({
                    "retrieval": {
                        "indexer": "local_lexical",
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
                        "indexer": "local_lexical",
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
                        "indexer": "local_lexical",
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

async fn process_post_ingest_fact_index_task(
    storage: &PgStorage,
    task: WorkflowTask,
) -> Result<()> {
    let process_result: Result<usize> = async {
        let dataset_id = task_payload_uuid(&task.payload, "dataset_id")
            .map(domain_model::DatasetId)
            .ok_or_else(|| anyhow!("fact index cleanup task missing dataset_id"))?;
        let document_id = task_payload_uuid(&task.payload, "document_id")
            .map(domain_model::DocumentId)
            .ok_or_else(|| anyhow!("fact index cleanup task missing document_id"))?;
        let document = storage
            .documents()
            .get_by_id(task.tenant_id, document_id)
            .await?
            .ok_or_else(|| anyhow!("document {} not found", document_id))?;
        if document.dataset_id != dataset_id {
            return Err(anyhow!(
                "fact index cleanup task dataset_id {} does not match document {} dataset_id {}",
                dataset_id,
                document_id,
                document.dataset_id
            ));
        }
        let chunks = storage
            .document_chunks()
            .list_by_document(task.tenant_id, document_id)
            .await?;
        let indexed_at = Utc::now();
        let facts = platform_api::fact_index::build_document_fact_candidates(
            &document, &chunks, indexed_at,
        );
        let persisted_facts = storage
            .document_facts()
            .replace_document_facts(task.tenant_id, document_id, &facts)
            .await?;
        let snapshot = platform_api::fact_index::rebuild_dataset_entity_rows_snapshot(
            storage,
            task.tenant_id,
            dataset_id,
            50,
            indexed_at,
        )
        .await?;

        storage
            .documents()
            .update_state(
                task.tenant_id,
                document_id,
                document.lifecycle.clone(),
                Some(&document.title),
                &json!({
                    "fact_index": {
                        "status": if chunks.is_empty() { "no_chunks" } else { "indexed" },
                        "indexer": "post_ingest_cleanup_v1",
                        "indexed_at": indexed_at,
                        "fact_count": persisted_facts.len(),
                        "chunk_count": chunks.len(),
                        "source_task_id": task.id,
                        "snapshot_kind": snapshot.snapshot_kind,
                        "snapshot_key": snapshot.snapshot_key,
                        "snapshot_source_fact_count": snapshot.source_fact_count,
                        "snapshot_source_document_count": snapshot.source_document_count,
                    }
                }),
                indexed_at,
            )
            .await?;

        Ok(persisted_facts.len())
    }
    .await;

    match process_result {
        Ok(fact_count) => {
            storage
                .workflow_tasks()
                .mark_succeeded(task.id, Utc::now())
                .await?;
            tracing::info!(
                task_id = %task.id,
                execution_id = %task.execution_id,
                fact_count,
                "post-ingest fact index cleanup task completed"
            );
            Ok(())
        }
        Err(error) => {
            let error_message = error.to_string();
            storage
                .workflow_tasks()
                .mark_failed(task.id, &error_message, Utc::now())
                .await?;
            Err(error)
        }
    }
}

#[derive(Clone, Debug)]
struct ExternalIndexedDocumentSummary {
    document_id: domain_model::DocumentId,
    indexed_chunk_count: usize,
    retrieval_evidence_count: usize,
}

async fn process_external_source_index_task(
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
    let document_ids = external_index_document_ids(&execution.context, &task.payload)?;

    let process_result: Result<Value> = async {
        let mut summaries = Vec::with_capacity(document_ids.len());
        for document_id in document_ids {
            summaries.push(
                index_external_document(storage, indexer, &task, dataset_id, document_id).await?,
            );
        }
        let indexed_chunk_count = summaries
            .iter()
            .map(|summary| summary.indexed_chunk_count)
            .sum::<usize>();
        let retrieval_evidence_count = summaries
            .iter()
            .map(|summary| summary.retrieval_evidence_count)
            .sum::<usize>();
        let signal_output = json!({
            "source_id": context_string(&execution.context, "source_id")?,
            "external_sync_run_id": context_string(&execution.context, "external_sync_run_id")?,
            "document_count": summaries.len(),
            "indexed_chunk_count": indexed_chunk_count,
            "retrieval_evidence_count": retrieval_evidence_count,
            "document_ids": summaries
                .iter()
                .map(|summary| json!(summary.document_id))
                .collect::<Vec<_>>(),
        });
        update_external_sync_run_index_counts(storage, task.tenant_id, &signal_output).await?;
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
                "external source retrieval index task completed"
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
                    "retrieval worker failed to send external source step_failed signal"
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

async fn index_external_document(
    storage: &PgStorage,
    indexer: &impl RetrievalIndexer,
    task: &domain_model::WorkflowTask,
    dataset_id: domain_model::DatasetId,
    document_id: domain_model::DocumentId,
) -> Result<ExternalIndexedDocumentSummary> {
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

    let indexed_at = Utc::now();
    let outcome = indexer.index(&RetrievalIndexJob {
        dataset_id,
        document_id,
        chunks: chunks
            .iter()
            .map(|chunk| RetrievalChunkInput {
                chunk_index: chunk.chunk_index,
                content: retrieval_index_text(&document, chunk),
                token_count: chunk.token_count.max(0) as usize,
            })
            .collect(),
    });
    let embedding_model = outcome.embedding_model.clone();
    let payload_filter_key = outcome.payload_filter_key.clone();
    let chunk_profiles = BTreeMap::from_iter(
        outcome
            .chunk_profiles
            .iter()
            .cloned()
            .map(|profile| (profile.chunk_index, profile)),
    );
    let mut new_retrieval_evidences = Vec::with_capacity(chunks.len());
    for chunk in &chunks {
        let profile = chunk_profiles.get(&chunk.chunk_index).ok_or_else(|| {
            anyhow!(
                "retrieval lexical profile missing for document {} chunk {}",
                document_id,
                chunk.chunk_index
            )
        })?;
        let source_locator = retrieval_source_locator(document_id, chunk);
        let section_title_hints = document_chunk_section_title_hints(chunk);
        let noun_terms = document_chunk_noun_terms(chunk);
        let mut evidence_manifest = json!({
            "schema_version": "0.4.0",
            "generator": "retrieval-worker",
            "dataset_id": dataset_id,
            "document_id": document_id,
            "document_chunk_id": chunk.id,
            "chunk_index": chunk.chunk_index,
            "indexed_at": indexed_at,
            "embedding": {
                "status": "indexed",
                "model": &embedding_model,
                "token_count": profile.token_count,
                "strategy": "local_lexical_v1",
                "signature_terms": profile.signature_terms,
                "term_weights": profile.term_weights,
                "vector_norm": profile.vector_norm,
            },
            "recall": {
                "status": "ready",
                "score": profile.recall_score,
                "rank_hint": profile.rank_hint,
            },
            "evidence": {
                "document_chunk_id": chunk.id,
                "payload_filter_key": &payload_filter_key,
                "source_locator": source_locator.clone(),
                "section_title_hints": &section_title_hints,
                "noun_terms": &noun_terms,
            },
        });
        if let Some(media_manifest) = retrieval_media_manifest(chunk) {
            if let Some(object) = evidence_manifest.as_object_mut() {
                object.insert("media".to_string(), media_manifest);
            }
        }
        if let Some(external_acl) = retrieval_external_acl_manifest(&document, chunk) {
            if let Some(object) = evidence_manifest.as_object_mut() {
                object.insert("external_acl".to_string(), external_acl);
            }
        }
        new_retrieval_evidences.push(NewRetrievalEvidence {
            execution_id: task.execution_id,
            dataset_id,
            document_id,
            document_chunk_id: chunk.id,
            chunk_index: chunk.chunk_index,
            source_locator,
            content_excerpt: excerpt(&chunk.content, 240),
            summary: retrieval_evidence_summary(
                &document,
                chunk,
                &section_title_hints,
                "external source retrieval recall",
            ),
            payload_filter_key: payload_filter_key.clone(),
            embedding_model: embedding_model.clone(),
            recall_score: profile.recall_score,
            evidence_manifest,
            created_at: indexed_at,
        });
    }
    let retrieval_evidences = storage
        .retrieval_evidences()
        .create_many(task.tenant_id, &new_retrieval_evidences)
        .await?;
    let indexed_chunks = storage
        .document_chunks()
        .mark_indexed(
            task.tenant_id,
            document_id,
            &json!({
                "retrieval": {
                    "indexer": "local_lexical",
                    "indexed_at": indexed_at,
                    "embedding_model": &embedding_model,
                    "payload_filter_key": &payload_filter_key,
                    "retrieval_evidence_count": retrieval_evidences.len(),
                }
            }),
            indexed_at,
        )
        .await?;
    storage
        .documents()
        .update_state(
            task.tenant_id,
            document_id,
            DocumentLifecycle::Indexed,
            Some(&document.title),
            &json!({
                "retrieval": {
                    "indexer": "local_lexical",
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

    Ok(ExternalIndexedDocumentSummary {
        document_id,
        indexed_chunk_count: indexed_chunks.len(),
        retrieval_evidence_count: retrieval_evidences.len(),
    })
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

fn retrieval_index_text(document: &Document, chunk: &DocumentChunk) -> String {
    let section_title_hints = document_chunk_section_title_hints(chunk).join("\n");
    let noun_terms = document_chunk_noun_terms(chunk).join("\n");
    [
        document.title.trim(),
        document.object_key.trim(),
        section_title_hints.trim(),
        noun_terms.trim(),
        chunk.content.trim(),
    ]
    .into_iter()
    .filter(|value| !value.is_empty())
    .collect::<Vec<_>>()
    .join("\n")
}

fn retrieval_evidence_summary(
    document: &Document,
    chunk: &DocumentChunk,
    section_title_hints: &[String],
    recall_kind: &str,
) -> String {
    let section = section_title_hints
        .first()
        .map(|title| format!(" section {title}"))
        .unwrap_or_default();
    format!(
        "{} chunk {}{} indexed for {}.",
        document.title, chunk.chunk_index, section, recall_kind
    )
}

fn document_chunk_section_title_hints(chunk: &DocumentChunk) -> Vec<String> {
    let mut hints = Vec::new();
    for key in [
        "section_title_hints",
        "sectionTitleHints",
        "section_titles",
        "sectionTitles",
        "heading_hints",
        "headingHints",
    ] {
        if let Some(value) = chunk.metadata.get(key) {
            collect_string_list(value, &mut hints);
        }
    }
    if let Some(value) = chunk
        .metadata
        .get("parse_metadata")
        .and_then(|value| value.get("section_title_hints"))
    {
        collect_string_list(value, &mut hints);
    }
    if hints.is_empty() {
        for hint in infer_section_title_hints_from_text(&chunk.content, 6) {
            push_string_hint(&mut hints, hint);
        }
    }
    hints.truncate(6);
    hints
}

fn document_chunk_noun_terms(chunk: &DocumentChunk) -> Vec<String> {
    let mut terms = Vec::new();
    if let Some(value) = chunk
        .metadata
        .get("understanding")
        .and_then(|value| value.get("noun_terms").or_else(|| value.get("nounTerms")))
    {
        collect_string_list(value, &mut terms);
    }
    for key in ["noun_terms", "nounTerms", "term_hints", "termHints"] {
        if let Some(value) = chunk.metadata.get(key) {
            collect_string_list(value, &mut terms);
        }
    }
    terms.truncate(64);
    terms
}

fn collect_string_list(value: &Value, output: &mut Vec<String>) {
    match value {
        Value::String(text) => {
            push_string_hint(output, text);
        }
        Value::Array(items) => {
            for item in items {
                collect_string_list(item, output);
            }
        }
        _ => {}
    }
}

fn push_string_hint(output: &mut Vec<String>, text: impl AsRef<str>) {
    let normalized = text.as_ref().trim().chars().take(80).collect::<String>();
    if !normalized.is_empty() && !output.iter().any(|existing| existing == &normalized) {
        output.push(normalized);
    }
}

fn infer_section_title_hints_from_text(text: &str, limit: usize) -> Vec<String> {
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

fn retrieval_source_locator(
    document_id: domain_model::DocumentId,
    chunk: &DocumentChunk,
) -> String {
    let base = format!("document://{document_id}/chunks/{}", chunk.chunk_index);
    chunk_media_metadata(chunk)
        .and_then(media_first_timestamp_window)
        .and_then(|window| media_locator_suffix(window.start_seconds, window.end_seconds))
        .map(|suffix| format!("{base}{suffix}"))
        .unwrap_or(base)
}

#[derive(Clone, Copy, Debug)]
struct MediaTimestampWindow {
    kind: &'static str,
    start_seconds: Option<f64>,
    end_seconds: Option<f64>,
}

fn retrieval_media_manifest(chunk: &DocumentChunk) -> Option<Value> {
    let media = chunk_media_metadata(chunk)?;
    let first_window = media_first_timestamp_window(media);
    let timestamp_window = first_window.map(|window| {
        json!({
            "kind": window.kind,
            "start_seconds": window.start_seconds,
            "end_seconds": window.end_seconds,
        })
    });
    Some(json!({
        "kind": media
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("unknown"),
        "parse_status": media
            .get("parse_status")
            .and_then(Value::as_str)
            .unwrap_or("unknown"),
        "has_timestamped_evidence": first_window.is_some(),
        "timestamp_window": timestamp_window,
        "transcript_segment_count": media_array_len(media, "transcript_segments"),
        "scene_count": media_array_len(media, "scenes"),
        "keyframe_ocr_snippet_count": media_array_len(media, "keyframe_ocr_snippets"),
        "provider_evidence_count": media_array_len(media, "provider_evidence"),
    }))
}

fn retrieval_external_acl_manifest(document: &Document, chunk: &DocumentChunk) -> Option<Value> {
    chunk
        .metadata
        .get("external_acl")
        .or_else(|| document.metadata.get("external_acl"))
        .or_else(|| document.metadata.get("external_source"))
        .and_then(Value::as_object)
        .map(|external_acl| {
            json!({
                "source_id": external_acl.get("source_id").and_then(Value::as_str),
                "document_external_id": external_acl
                    .get("document_external_id")
                    .and_then(Value::as_str),
                "revision_external_id": external_acl
                    .get("revision_external_id")
                    .and_then(Value::as_str),
                "acl_hash": external_acl.get("acl_hash").and_then(Value::as_str),
            })
        })
        .filter(|value| {
            value.get("source_id").and_then(Value::as_str).is_some()
                && value
                    .get("document_external_id")
                    .and_then(Value::as_str)
                    .is_some()
        })
}

fn external_index_document_ids(
    context: &Value,
    task_payload: &Value,
) -> Result<Vec<domain_model::DocumentId>> {
    let candidates = [
        context.pointer("/last_output/document_ids"),
        context.pointer("/last_output/ingested_document_ids"),
        task_payload.get("document_ids"),
        task_payload.get("ingested_document_ids"),
    ];
    let values = candidates
        .into_iter()
        .flatten()
        .find_map(Value::as_array)
        .ok_or_else(|| anyhow!("external retrieval index requires document_ids array"))?;
    if values.is_empty() {
        return Err(anyhow!(
            "external retrieval index requires at least one document id"
        ));
    }

    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| anyhow!("external document id must be a string"))
                .and_then(|raw| {
                    raw.parse::<uuid::Uuid>()
                        .map(domain_model::DocumentId)
                        .map_err(|error| anyhow!("invalid external document id {raw}: {error}"))
                })
        })
        .collect()
}

async fn update_external_sync_run_index_counts(
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
        "chunks_indexed": signal_output
            .get("indexed_chunk_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        "retrieval_evidences_indexed": signal_output
            .get("retrieval_evidence_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
    }))
    .bind(Utc::now())
    .execute(storage.pool())
    .await?;

    Ok(())
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

fn chunk_media_metadata(chunk: &DocumentChunk) -> Option<&Value> {
    chunk
        .metadata
        .get("parse_metadata")
        .and_then(|metadata| metadata.pointer("/media"))
        .or_else(|| chunk.metadata.get("media"))
        .filter(|value| value.is_object())
}

fn media_array_len(media: &Value, key: &str) -> usize {
    media
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0)
}

fn media_first_timestamp_window(media: &Value) -> Option<MediaTimestampWindow> {
    for (key, kind) in [
        ("transcript_segments", "transcript"),
        ("scenes", "scene"),
        ("keyframe_ocr_snippets", "keyframe_ocr"),
    ] {
        let Some(items) = media.get(key).and_then(Value::as_array) else {
            continue;
        };
        for item in items {
            let start_seconds = media_numeric_field(
                item,
                &[
                    "start_seconds",
                    "start",
                    "timestamp_seconds",
                    "timestamp",
                    "representative_seconds",
                ],
            );
            let end_seconds = media_numeric_field(item, &["end_seconds", "end"]);
            if start_seconds.is_some() || end_seconds.is_some() {
                return Some(MediaTimestampWindow {
                    kind,
                    start_seconds,
                    end_seconds,
                });
            }
        }
    }
    None
}

fn media_numeric_field(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|key| {
        value.get(*key).and_then(|item| {
            item.as_f64()
                .or_else(|| item.as_str().and_then(|text| text.parse::<f64>().ok()))
        })
    })
}

fn media_locator_suffix(start_seconds: Option<f64>, end_seconds: Option<f64>) -> Option<String> {
    let start_seconds = start_seconds?.max(0.0);
    let suffix = match end_seconds {
        Some(end_seconds) => format!("#t={start_seconds:.3}-{:.3}", end_seconds.max(0.0)),
        None => format!("#t={start_seconds:.3}"),
    };
    Some(suffix)
}

fn optional_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && value != "*")
}

fn task_payload_uuid(payload: &Value, key: &str) -> Option<uuid::Uuid> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .and_then(|raw| raw.parse::<uuid::Uuid>().ok())
}

async fn wait_for_next_task_signal(task_waker: &mut EventSubscription, poll_interval_ms: u64) {
    if let Some(event) = task_waker
        .wait_for_event(Duration::from_millis(poll_interval_ms))
        .await
    {
        tracing::debug!(subject = %event.subject, "retrieval worker received task wake signal");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain_model::{
        DatasetId, DocumentChunkId, DocumentChunkState, DocumentId, DocumentLifecycle, TenantId,
    };

    fn media_chunk(metadata: Value) -> DocumentChunk {
        let now = Utc::now();
        DocumentChunk {
            id: DocumentChunkId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            document_id: DocumentId::new(),
            chunk_index: 0,
            content: "Transcript segments:\n[00:01.500 - 00:02.750] 客户询问订单状态".to_string(),
            token_count: 18,
            state: DocumentChunkState::Extracted,
            metadata: BTreeMap::from_iter([("parse_metadata".to_string(), metadata)]),
            created_at: now,
            updated_at: now,
        }
    }

    fn document_with_metadata(metadata: Value) -> Document {
        let now = Utc::now();
        Document {
            id: DocumentId::new(),
            tenant_id: TenantId::new(),
            dataset_id: DatasetId::new(),
            owner_user_id: None,
            title: "External Doc".to_string(),
            object_key: "external/src/doc".to_string(),
            content_type: "text/markdown".to_string(),
            lifecycle: DocumentLifecycle::Extracted,
            secret_binding_ids: Vec::new(),
            metadata: metadata
                .as_object()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .collect(),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn retrieval_source_locator_adds_first_media_timestamp_window() {
        let document_id = DocumentId::new();
        let chunk = media_chunk(json!({
            "media": {
                "kind": "audio",
                "parse_status": "transcribed",
                "transcript_segments": [{
                    "start_seconds": 1.5,
                    "end_seconds": 2.75,
                    "text": "客户询问订单状态"
                }]
            }
        }));

        let locator = retrieval_source_locator(document_id, &chunk);

        assert_eq!(
            locator,
            format!("document://{document_id}/chunks/0#t=1.500-2.750")
        );
    }

    #[test]
    fn retrieval_media_manifest_summarizes_timestamped_evidence() {
        let chunk = media_chunk(json!({
            "media": {
                "kind": "video",
                "parse_status": "enriched_partial",
                "transcript_segments": [],
                "scenes": [{
                    "start_seconds": 0,
                    "end_seconds": 12,
                    "summary": "门店入口画面"
                }],
                "keyframe_ocr_snippets": [{
                    "timestamp_seconds": 6,
                    "text": "今日客流 2180"
                }],
                "provider_evidence": [{
                    "provider": "minimax",
                    "capability": "native_video_understanding",
                    "supported": false
                }]
            }
        }));

        let manifest = retrieval_media_manifest(&chunk).expect("media manifest should exist");

        assert_eq!(manifest["kind"], json!("video"));
        assert_eq!(manifest["has_timestamped_evidence"], json!(true));
        assert_eq!(manifest["timestamp_window"]["kind"], json!("scene"));
        assert_eq!(manifest["scene_count"], json!(1));
        assert_eq!(manifest["keyframe_ocr_snippet_count"], json!(1));
        assert_eq!(manifest["provider_evidence_count"], json!(1));
    }

    #[test]
    fn retrieval_index_text_includes_section_title_hints() {
        let document = document_with_metadata(json!({}));
        let mut chunk = media_chunk(json!({}));
        chunk.content = "审批正文说明。".to_string();
        chunk.metadata.insert(
            "section_title_hints".to_string(),
            json!(["固定资产申请", "审批流程"]),
        );

        let indexed_text = retrieval_index_text(&document, &chunk);

        assert!(indexed_text.contains("External Doc"));
        assert!(indexed_text.contains("固定资产申请"));
        assert!(indexed_text.contains("审批正文说明"));
    }

    #[test]
    fn retrieval_index_text_includes_chunk_noun_terms() {
        let document = document_with_metadata(json!({}));
        let mut chunk = media_chunk(json!({}));
        chunk.metadata.insert(
            "understanding".to_string(),
            json!({"noun_terms": ["订单延期风险", "供应商确认"]}),
        );
        chunk.content = "仓库交接超过两天需要提醒。".to_string();

        let indexed_text = retrieval_index_text(&document, &chunk);
        let noun_terms = document_chunk_noun_terms(&chunk);

        assert!(indexed_text.contains("订单延期风险"));
        assert!(indexed_text.contains("供应商确认"));
        assert_eq!(noun_terms, vec!["订单延期风险", "供应商确认"]);
    }

    #[test]
    fn retrieval_evidence_summary_names_first_section_title_hint() {
        let document = document_with_metadata(json!({}));
        let mut chunk = media_chunk(json!({}));
        chunk
            .metadata
            .insert("section_title_hints".to_string(), json!(["固定资产申请"]));
        let hints = document_chunk_section_title_hints(&chunk);

        let summary =
            retrieval_evidence_summary(&document, &chunk, &hints, "lexical retrieval recall");

        assert!(summary.contains("section 固定资产申请"));
    }

    #[test]
    fn post_ingest_fact_candidates_keep_normalized_names_and_sources() {
        let document = document_with_metadata(json!({
            "ingest": { "parse_method": "pdf-paddleocr" }
        }));
        let mut chunk = media_chunk(json!({}));
        chunk.dataset_id = document.dataset_id;
        chunk.document_id = document.id;
        chunk.content =
            "## 工作经历\n北京星河科技有限公司 2020年 任后端工程师，负责智能梯控系统。".to_string();
        chunk
            .metadata
            .insert("section_title_hints".to_string(), json!(["工作经历"]));
        chunk.metadata.insert(
            "understanding".to_string(),
            json!({
                "noun_terms": [
                    "北京星河科技有限公司",
                    "后端工程师",
                    "智能梯控系统"
                ]
            }),
        );

        let facts = platform_api::fact_index::build_document_fact_candidates(
            &document,
            &[chunk],
            Utc::now(),
        );

        assert!(facts.iter().any(|fact| fact.fact_type == "section"
            && fact.normalized_name == "工作经历"
            && fact.source_kind == "section_title_hint"));
        assert!(facts.iter().any(|fact| fact.fact_type == "organization"
            && fact.normalized_name == "北京星河科技有限公司"));
        assert!(facts
            .iter()
            .any(|fact| fact.fact_type == "role_position" && fact.normalized_name == "后端工程师"));
        assert!(facts
            .iter()
            .any(|fact| fact.fact_type == "project_product_system"
                && fact.normalized_name == "智能梯控系统"));
        assert!(facts
            .iter()
            .any(|fact| fact.fact_type == "date_period" && fact.normalized_name == "2020年"));
        assert!(facts.iter().all(|fact| fact
            .source_locator
            .as_deref()
            .unwrap_or_default()
            .contains("/chunks/")));
        assert!(facts
            .iter()
            .all(|fact| fact.parse_version.as_deref() == Some("pdf-paddleocr")));
    }

    #[test]
    fn retrieval_external_acl_manifest_prefers_chunk_acl_ref() {
        let document = document_with_metadata(json!({
            "external_source": {
                "source_id": "src-docs",
                "document_external_id": "doc-from-document"
            }
        }));
        let chunk = media_chunk(json!({}));
        let mut chunk = chunk;
        chunk.metadata.insert(
            "external_acl".to_string(),
            json!({
                "source_id": "src-docs",
                "document_external_id": "doc-from-chunk",
                "revision_external_id": "rev-3",
                "acl_hash": "acl-3"
            }),
        );

        let manifest =
            retrieval_external_acl_manifest(&document, &chunk).expect("external ACL ref");

        assert_eq!(manifest["source_id"], json!("src-docs"));
        assert_eq!(manifest["document_external_id"], json!("doc-from-chunk"));
        assert_eq!(manifest["revision_external_id"], json!("rev-3"));
        assert_eq!(manifest["acl_hash"], json!("acl-3"));
    }

    #[test]
    fn external_index_document_ids_parse_workflow_last_output() {
        let first = DocumentId::new();
        let second = DocumentId::new();
        let ids = external_index_document_ids(
            &json!({
                "last_output": {
                    "document_ids": [first.to_string(), second.to_string()]
                }
            }),
            &json!({}),
        )
        .expect("external document ids should parse");

        assert_eq!(ids, vec![first, second]);
    }
}
