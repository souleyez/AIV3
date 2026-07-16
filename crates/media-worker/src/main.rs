use anyhow::{anyhow, Result};
use chrono::Utc;
use domain_model::{
    AssistantRunId, DatasetId, DocumentId, WorkflowEventId, WorkflowEventRecord, WorkflowExecution,
    WorkflowExecutionId, WorkflowKind, WorkflowTask,
};
use event_bus::{workflow_task_enqueued_subject, EventBus, EventSubscription};
use media_worker::{
    extract_video_ppt_output_with_artifacts, frame_extraction_config_from_env,
    merge_video_extraction_durable_published_version_ref, merge_video_extraction_output_artifacts,
    register_video_asset_output, resolve_video_source_output,
    run_video_frame_extraction_if_enabled, video_extraction_completion_audit_from_output,
    video_extraction_completion_follow_up_from_output,
    video_extraction_durable_published_version_manifest,
    video_extraction_html_artifact_from_output, video_extraction_model_completion_dispatch_request,
    write_video_extraction_text_artifacts_if_available, FrameExtractionConfig,
    MediaWorkflowTaskKind,
};
use serde_json::{json, Value};
use storage::{
    NewAssistantRunEvent, NewHtmlArtifact, NewPublishedVideoPptVersion, PgStorage,
    DEFAULT_LOCAL_DATABASE_URL,
};
use tokio::time::Duration;
use uuid::Uuid;
use workflow_engine::{WorkflowCatalog, WorkflowSignal};

const DEFAULT_QUEUE: &str = "media";
const DEFAULT_WAKE_TASK_KEY: &str = "resolve_video_source";
const DEFAULT_POLL_INTERVAL_MS: u64 = 1_000;

#[tokio::main]
async fn main() -> Result<()> {
    observability::install("media_worker")?;

    let database_url = std::env::var("PLATFORM_DATABASE_URL")
        .unwrap_or_else(|_| DEFAULT_LOCAL_DATABASE_URL.to_string());
    let queue = std::env::var("MEDIA_QUEUE").unwrap_or_else(|_| DEFAULT_QUEUE.to_string());
    let task_key = optional_env("MEDIA_TASK_KEY");
    let poll_interval = std::env::var("MEDIA_POLL_INTERVAL_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_POLL_INTERVAL_MS);

    let storage = PgStorage::connect_with_configured_max_connections(
        &database_url,
        "MEDIA_WORKER_DATABASE_MAX_CONNECTIONS",
    )
    .await?;
    let workflow_catalog = workflow_definitions::catalog();
    let event_bus = EventBus::connect_from_env_or_disabled("PLATFORM_NATS_URL").await;
    let frame_extraction_config = frame_extraction_config_from_env();
    let wake_task_key = task_key.as_deref().unwrap_or(DEFAULT_WAKE_TASK_KEY);
    let wake_subject = workflow_task_enqueued_subject(&queue, wake_task_key);
    let mut task_waker = event_bus
        .subscribe_queue_or_disabled(
            &wake_subject,
            Some(&format!("media_worker.{queue}.{wake_task_key}")),
        )
        .await;

    tracing::info!(
        %queue,
        task_key = task_key.as_deref().unwrap_or("*"),
        %wake_subject,
        event_bus_enabled = event_bus.is_enabled(),
        poll_interval_ms = poll_interval,
        frame_extraction_enabled = frame_extraction_config.enabled,
        frame_extraction_interval_seconds = frame_extraction_config.interval_seconds,
        database_endpoint = %observability::redact_connection_endpoint(&database_url),
        "media-worker polling started"
    );

    loop {
        match storage
            .workflow_tasks()
            .claim_next_available(&queue, task_key.as_deref(), Utc::now())
            .await
        {
            Ok(Some(task)) => {
                if let Err(error) = process_task(
                    &storage,
                    &workflow_catalog,
                    &event_bus,
                    &frame_extraction_config,
                    task,
                )
                .await
                {
                    tracing::error!(error = ?error, "media task processing failed");
                }
            }
            Ok(None) => {
                wait_for_next_task_signal(&mut task_waker, poll_interval).await;
            }
            Err(error) => {
                tracing::error!(error = ?error, "media worker failed to claim task");
                wait_for_next_task_signal(&mut task_waker, poll_interval).await;
            }
        }
    }
}

async fn process_task(
    storage: &PgStorage,
    workflow_catalog: &WorkflowCatalog,
    event_bus: &EventBus,
    frame_extraction_config: &FrameExtractionConfig,
    task: WorkflowTask,
) -> Result<()> {
    let process_result: Result<()> = async {
        let execution = storage
            .workflow_executions()
            .get_by_id(task.tenant_id, task.execution_id)
            .await?
            .ok_or_else(|| anyhow!("workflow execution {} not found", task.execution_id))?;
        if execution.kind != WorkflowKind::VideoExtraction {
            return Err(anyhow!(
                "workflow execution {} has unexpected kind {}",
                execution.id,
                execution.kind.as_str()
            ));
        }

        let task_kind = MediaWorkflowTaskKind::from_task_key(&task.task_key)
            .ok_or_else(|| anyhow!("unsupported media task key {}", task.task_key))?;
        let output = match task_kind {
            MediaWorkflowTaskKind::ResolveVideoSource => {
                resolve_video_source_output(&task, &execution.context)
            }
            MediaWorkflowTaskKind::RegisterVideoAsset => {
                let document_id = context_uuid(&execution.context, "document_id")?;
                let document = storage
                    .documents()
                    .get_by_id(task.tenant_id, domain_model::DocumentId(document_id))
                    .await?
                    .ok_or_else(|| anyhow!("document {} not found", document_id))?;
                register_video_asset_output(&document)
            }
            MediaWorkflowTaskKind::ExtractVideoPpt => {
                let document_id = context_uuid(&execution.context, "document_id")?;
                let document_id = domain_model::DocumentId(document_id);
                let document = storage
                    .documents()
                    .get_by_id(task.tenant_id, document_id)
                    .await?
                    .ok_or_else(|| anyhow!("document {} not found", document_id))?;
                let chunks = storage
                    .document_chunks()
                    .list_by_document(task.tenant_id, document_id)
                    .await?;
                let frame_extraction =
                    run_video_frame_extraction_if_enabled(&document, frame_extraction_config);
                let generated_artifacts = write_video_extraction_text_artifacts_if_available(
                    &document,
                    &chunks,
                    &frame_extraction,
                    &frame_extraction_config.output_root,
                );
                extract_video_ppt_output_with_artifacts(
                    &document,
                    &chunks,
                    frame_extraction,
                    generated_artifacts,
                )
            }
        };

        platform_api::apply_workflow_signal_with_dependencies(
            storage,
            workflow_catalog,
            event_bus,
            task.tenant_id,
            task.execution_id,
            WorkflowSignal::StepCompleted {
                task_key: task.task_key.clone(),
                output: Some(output.clone()),
            },
        )
        .await?;

        if task_kind == MediaWorkflowTaskKind::ExtractVideoPpt {
            if let Err(error) = append_video_extraction_assistant_event(
                storage,
                workflow_catalog,
                event_bus,
                &execution,
                &output,
            )
            .await
            {
                tracing::warn!(
                    error = ?error,
                    execution_id = %execution.id,
                    "media worker could not append assistant video extraction event"
                );
            }
        }

        Ok(())
    }
    .await;

    if let Err(error) = process_result {
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
                "media worker failed to send workflow step_failed signal"
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
        task_key = %task.task_key,
        "media task completed"
    );

    Ok(())
}

async fn append_video_extraction_assistant_event(
    storage: &PgStorage,
    workflow_catalog: &WorkflowCatalog,
    event_bus: &EventBus,
    execution: &WorkflowExecution,
    output: &Value,
) -> Result<()> {
    let Some(raw_run_id) = execution
        .context
        .get("assistant_run_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(());
    };
    let run_id = raw_run_id
        .parse::<Uuid>()
        .map(AssistantRunId)
        .map_err(|error| anyhow!("invalid assistant_run_id in video workflow context: {error}"))?;
    let run = storage
        .assistant_runs()
        .get_by_id(execution.tenant_id, run_id)
        .await?
        .ok_or_else(|| anyhow!("assistant run {run_id} not found"))?;
    let local_thread_id = execution
        .context
        .get("local_thread_id")
        .and_then(Value::as_str)
        .or(run.local_thread_id.as_deref());
    let html_artifacts =
        video_extraction_html_artifact_from_output(&run_id.to_string(), local_thread_id, output)
            .into_iter()
            .collect::<Vec<_>>();
    let completion_follow_up =
        video_extraction_completion_follow_up_from_output(output, &html_artifacts);
    let model_completion_turn_request = completion_follow_up
        .as_ref()
        .and_then(|follow_up| follow_up.get("model_follow_up"))
        .cloned()
        .unwrap_or(Value::Null);
    let completion_audit = video_extraction_completion_audit_from_output(output);
    let workflow_execution_id = execution.id.to_string();
    let model_completion_turn_dispatch_request =
        video_extraction_model_completion_dispatch_request(
            &run_id.to_string(),
            Some(&workflow_execution_id),
            Some(execution.kind.as_str()),
            output,
            completion_follow_up.as_ref(),
        )
        .unwrap_or(Value::Null);

    storage
        .assistant_runs()
        .append_event(
            execution.tenant_id,
            run_id,
            &NewAssistantRunEvent {
                event_name: "video_extraction.workflow_completed".to_string(),
                payload: serde_json::json!({
                    "status": output.get("status").and_then(Value::as_str).unwrap_or("partial"),
                    "workflow_execution_id": execution.id.to_string(),
                    "workflow_kind": execution.kind.as_str(),
                    "document_id": output.get("document_id").cloned().unwrap_or(Value::Null),
                    "dataset_id": output.get("dataset_id").cloned().unwrap_or(Value::Null),
                    "output": output,
                    "html_artifacts": html_artifacts,
                    "completion_follow_up": completion_follow_up,
                    "model_completion_turn_request": model_completion_turn_request,
                    "model_completion_turn_dispatch_request": model_completion_turn_dispatch_request.clone(),
                    "completion_audit": completion_audit,
                    "no_host_composed_answer": true,
                }),
                created_at: Utc::now(),
            },
        )
        .await?;
    if !model_completion_turn_dispatch_request.is_null() {
        storage
            .assistant_runs()
            .append_event(
                execution.tenant_id,
                run_id,
                &NewAssistantRunEvent {
                    event_name: "assistant_run.model_completion_turn_requested".to_string(),
                    payload: model_completion_turn_dispatch_request.clone(),
                    created_at: Utc::now(),
                },
            )
            .await?;
    }
    persist_video_extraction_html_artifacts(storage, &run, &html_artifacts).await?;
    let durable_published_version = persist_video_ppt_published_version(
        storage,
        execution,
        run_id,
        &workflow_execution_id,
        output,
    )
    .await?;
    let mut output_artifacts = merge_video_extraction_output_artifacts(
        &run.output_artifacts,
        &run_id.to_string(),
        local_thread_id,
        output,
        &html_artifacts,
    );
    if let Some(durable_published_version) = durable_published_version.as_ref() {
        if let Some(document_id) = output.get("document_id").and_then(Value::as_str) {
            output_artifacts = merge_video_extraction_durable_published_version_ref(
                &output_artifacts,
                &run_id.to_string(),
                document_id,
                durable_published_version,
            );
        }
        storage
            .assistant_runs()
            .append_event(
                execution.tenant_id,
                run_id,
                &NewAssistantRunEvent {
                    event_name: "video_extraction.published_version_persisted".to_string(),
                    payload: json!({
                        "durable_published_version": durable_published_version,
                        "workflow_execution_id": execution.id.to_string(),
                        "workflow_kind": execution.kind.as_str(),
                        "no_host_composed_answer": true,
                    }),
                    created_at: Utc::now(),
                },
            )
            .await?;
    }
    storage
        .assistant_runs()
        .attach_output_artifacts(execution.tenant_id, run_id, &output_artifacts)
        .await?;
    if !model_completion_turn_dispatch_request.is_null() {
        if let Some(enqueued) = enqueue_assistant_run_model_completion_turn(
            storage,
            workflow_catalog,
            event_bus,
            execution,
            run_id,
            local_thread_id,
            &model_completion_turn_dispatch_request,
        )
        .await?
        {
            storage
                .assistant_runs()
                .append_event(
                    execution.tenant_id,
                    run_id,
                    &NewAssistantRunEvent {
                        event_name: "assistant_run.model_completion_turn_enqueued".to_string(),
                        payload: enqueued,
                        created_at: Utc::now(),
                    },
                )
                .await?;
        }
    }

    Ok(())
}

async fn persist_video_ppt_published_version(
    storage: &PgStorage,
    execution: &WorkflowExecution,
    run_id: AssistantRunId,
    workflow_execution_id: &str,
    output: &Value,
) -> Result<Option<Value>> {
    let Some(manifest) = video_extraction_durable_published_version_manifest(
        &run_id.to_string(),
        workflow_execution_id,
        output,
    ) else {
        return Ok(None);
    };
    let document_id = manifest_uuid(&manifest, "document_id")?;
    let dataset_id = manifest_uuid(&manifest, "dataset_id")?;
    let package_key = manifest
        .get("package_key")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("durable video PPT manifest missing package_key"))?
        .to_string();
    let version_fingerprint = manifest
        .get("version_fingerprint")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("durable video PPT manifest missing version_fingerprint"))?
        .to_string();
    let lifecycle_state = manifest
        .get("lifecycle_state")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("published_version_ready")
        .to_string();
    let (package, version) = storage
        .published_video_ppt_versions()
        .create_next_version(
            execution.tenant_id,
            &NewPublishedVideoPptVersion {
                assistant_run_id: run_id,
                document_id: DocumentId(document_id),
                dataset_id: DatasetId(dataset_id),
                package_key,
                version_fingerprint,
                lifecycle_state,
                artifact_manifest: manifest,
                created_at: Utc::now(),
            },
        )
        .await?;

    Ok(Some(json!({
        "type": "video_ppt_published_version",
        "package_id": package.id.to_string(),
        "version_id": version.id.to_string(),
        "version_no": version.version_no,
        "version_fingerprint": version.version_fingerprint,
        "lifecycle_state": version.lifecycle_state,
        "package_key": package.package_key,
        "assistant_run_id": package.assistant_run_id.to_string(),
        "document_id": package.document_id.to_string(),
        "dataset_id": package.dataset_id.to_string(),
        "current_version_id": package.current_version_id.map(|id| id.to_string()),
        "manifest_type": version
            .artifact_manifest
            .get("manifest_type")
            .and_then(Value::as_str)
            .unwrap_or("v3.video_ppt_durable_published_version.v1"),
        "durable_history_status": "promoted_to_storage",
        "created_at": version.created_at.to_rfc3339(),
        "no_host_composed_answer": true,
    })))
}

async fn enqueue_assistant_run_model_completion_turn(
    storage: &PgStorage,
    workflow_catalog: &WorkflowCatalog,
    event_bus: &EventBus,
    source_execution: &WorkflowExecution,
    run_id: AssistantRunId,
    local_thread_id: Option<&str>,
    dispatch_request: &Value,
) -> Result<Option<Value>> {
    let idempotency_key = dispatch_request
        .get("idempotency_key")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("model completion dispatch request missing idempotency_key"))?;
    if existing_model_completion_turn_queue_event(
        storage,
        source_execution.tenant_id,
        run_id,
        idempotency_key,
    )
    .await?
    .is_some()
    {
        tracing::info!(
            assistant_run_id = %run_id,
            %idempotency_key,
            "assistant run model-completion turn is already queued or consumed"
        );
        return Ok(None);
    }

    let definition = workflow_catalog
        .find_definition(WorkflowKind::AssistantRunModelCompletion)
        .ok_or_else(|| anyhow!("assistant run model-completion workflow is not registered"))?;
    let now = Utc::now();
    let execution_id = WorkflowExecutionId::new();
    let runtime_state = definition.initial_state(execution_id, now);
    let mut context = runtime_state.context;
    context.insert("assistant_run_id".to_string(), json!(run_id.to_string()));
    if let Some(local_thread_id) = local_thread_id {
        context.insert("local_thread_id".to_string(), json!(local_thread_id));
    }
    context.insert(
        "source_workflow_execution_id".to_string(),
        json!(source_execution.id.to_string()),
    );
    context.insert(
        "source_workflow_kind".to_string(),
        json!(source_execution.kind.as_str()),
    );
    context.insert("idempotency_key".to_string(), json!(idempotency_key));
    context.insert(
        "entrypoint".to_string(),
        dispatch_request
            .get("entrypoint")
            .cloned()
            .unwrap_or_else(|| json!("assistant_run_background_completion")),
    );
    context.insert(
        "document_id".to_string(),
        dispatch_request
            .get("document_id")
            .cloned()
            .unwrap_or(Value::Null),
    );
    context.insert("dispatch_request".to_string(), dispatch_request.clone());
    context.insert("no_host_composed_answer".to_string(), json!(true));
    context.insert(
        "retries_remaining".to_string(),
        json!(runtime_state.retries_remaining),
    );

    let workflow_execution = WorkflowExecution {
        id: execution_id,
        tenant_id: source_execution.tenant_id,
        dataset_id: source_execution.dataset_id,
        report_plan_id: source_execution.report_plan_id,
        kind: runtime_state.kind,
        version: runtime_state.version,
        stage: runtime_state.stage,
        status: runtime_state.status,
        attempt: 0,
        context: Value::Object(context),
        created_at: now,
        updated_at: runtime_state.updated_at,
    };
    let initial_event = WorkflowEventRecord {
        id: WorkflowEventId::new(),
        execution_id,
        sequence_no: 1,
        event_name: "workflow.created".to_string(),
        payload: json!({
            "reason": "assistant_run_model_completion_turn_requested",
            "assistant_run_id": run_id,
            "idempotency_key": idempotency_key,
            "source_workflow_execution_id": source_execution.id,
            "source_workflow_kind": source_execution.kind.as_str(),
            "no_host_composed_answer": true,
        }),
        created_at: now,
    };
    storage
        .workflow_executions()
        .create_with_initial_event(&workflow_execution, &initial_event)
        .await?;
    let started = platform_api::apply_workflow_signal_with_dependencies(
        storage,
        workflow_catalog,
        event_bus,
        source_execution.tenant_id,
        workflow_execution.id,
        WorkflowSignal::Start,
    )
    .await
    .map_err(|error| anyhow!(error.to_string()))?;

    Ok(Some(json!({
        "status": "enqueued",
        "assistant_run_id": run_id,
        "idempotency_key": idempotency_key,
        "workflow_execution_id": workflow_execution.id,
        "workflow_kind": WorkflowKind::AssistantRunModelCompletion.as_str(),
        "workflow_task_id": started.enqueued_tasks.first().map(|task| task.id),
        "queue": started
            .enqueued_tasks
            .first()
            .map(|task| task.queue.clone())
            .unwrap_or_else(|| "assistant_run".to_string()),
        "task_key": started
            .enqueued_tasks
            .first()
            .map(|task| task.task_key.clone())
            .unwrap_or_else(|| "consume_model_completion_turn".to_string()),
        "source_workflow_execution_id": source_execution.id,
        "source_workflow_kind": source_execution.kind.as_str(),
        "no_host_composed_answer": true,
    })))
}

async fn existing_model_completion_turn_queue_event(
    storage: &PgStorage,
    tenant_id: domain_model::TenantId,
    run_id: AssistantRunId,
    idempotency_key: &str,
) -> Result<Option<Value>> {
    let events = storage
        .assistant_runs()
        .list_events(tenant_id, run_id)
        .await?;
    Ok(events.iter().rev().find_map(|event| {
        let event_key = event
            .payload
            .get("idempotency_key")
            .and_then(Value::as_str)?;
        if event_key != idempotency_key {
            return None;
        }
        match event.event_name.as_str() {
            "assistant_run.model_completion_turn_enqueued"
            | "assistant_run.model_completion_turn_consumed" => Some(json!({
                "status": "already_enqueued",
                "assistant_run_id": run_id,
                "idempotency_key": idempotency_key,
                "existing_event_id": event.id,
                "existing_sequence_no": event.sequence_no,
                "existing_event_name": event.event_name.clone(),
                "existing_workflow_execution_id": event
                    .payload
                    .get("workflow_execution_id")
                    .cloned()
                    .unwrap_or(Value::Null),
                "no_host_composed_answer": true,
            })),
            _ => None,
        }
    }))
}

async fn persist_video_extraction_html_artifacts(
    storage: &PgStorage,
    run: &domain_model::AssistantRun,
    html_artifacts: &[Value],
) -> Result<()> {
    for artifact in html_artifacts {
        let Some(id) = artifact.get("id").and_then(Value::as_str) else {
            continue;
        };
        storage
            .html_artifacts()
            .upsert(
                run.tenant_id,
                &NewHtmlArtifact {
                    id: id.to_string(),
                    owner_user_id: run.user_id,
                    assistant_run_id: Some(run.id),
                    local_thread_id: run.local_thread_id.clone(),
                    source_type: artifact
                        .get("source_type")
                        .and_then(Value::as_str)
                        .unwrap_or("video_extraction")
                        .to_string(),
                    template_id: artifact
                        .get("template_id")
                        .and_then(Value::as_str)
                        .unwrap_or("video_extraction_summary")
                        .to_string(),
                    interaction_mode: artifact
                        .get("interaction_mode")
                        .and_then(Value::as_str)
                        .unwrap_or("read_only")
                        .to_string(),
                    manifest: artifact.clone(),
                    created_at: Utc::now(),
                },
            )
            .await?;
    }

    Ok(())
}

fn context_uuid(value: &Value, key: &str) -> Result<Uuid> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("workflow context missing {key}"))?
        .parse::<Uuid>()
        .map_err(|error| anyhow!("invalid workflow context {key}: {error}"))
}

fn manifest_uuid(value: &Value, key: &str) -> Result<Uuid> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("durable video PPT manifest missing {key}"))?
        .parse::<Uuid>()
        .map_err(|error| anyhow!("invalid durable video PPT manifest {key}: {error}"))
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
        tracing::debug!(subject = %event.subject, "media worker woke from event bus");
    }
}
