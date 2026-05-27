use anyhow::{anyhow, Result};
use chrono::Utc;
use domain_model::{
    AssistantRunId, StaticPageDraft, StaticPageDraftStatus, StaticPageImageJob,
    StaticPageImageJobStatus, StaticPageRenderOutput, StaticPageRenderOutputId,
    StaticPageRenderOutputStatus, TenantId, WorkflowKind, WorkflowStatus,
};
use event_bus::{workflow_task_enqueued_subject, EventBus, EventSubscription};
use reqwest::blocking::Client;
use serde_json::{json, Value};
use static_page_renderer::{render_static_page, StaticPageRenderRequest};
use static_page_worker::{
    context_uuid, extract_first_image_artifact, merge_orchestrator_state,
    normalize_artifact_asset_key, poll_static_page_visual_task, submit_static_page_visual_task,
    task_failure_message, CodexOrchestratorConfig, StaticPageVisualArtifact,
};
use storage::{NewAssistantRunEvent, PgStorage, DEFAULT_LOCAL_DATABASE_URL};
use tokio::time::Duration;
use workflow_engine::{WorkflowCatalog, WorkflowSignal};

const DEFAULT_QUEUE: &str = "static_page";
const DEFAULT_IMAGE_TASK_KEY: &str = "generate_static_page_image";
const DEFAULT_POLL_INTERVAL_MS: u64 = 1_000;
const DEFAULT_ORCHESTRATOR_POLL_INTERVAL_MS: u64 = 5_000;
const DEFAULT_ORCHESTRATOR_MAX_POLLS: u32 = 360;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StaticPageRenderTaskOutcome {
    Rendered,
    Cancelled,
}

impl StaticPageRenderTaskOutcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Rendered => "rendered",
            Self::Cancelled => "cancelled",
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    observability::install("static_page_worker")?;

    let database_url = std::env::var("PLATFORM_DATABASE_URL")
        .unwrap_or_else(|_| DEFAULT_LOCAL_DATABASE_URL.to_string());
    let queue = std::env::var("STATIC_PAGE_QUEUE").unwrap_or_else(|_| DEFAULT_QUEUE.to_string());
    let task_key =
        optional_env("STATIC_PAGE_TASK_KEY").or_else(|| optional_env("STATIC_PAGE_IMAGE_TASK_KEY"));
    let poll_interval = env_u64(
        "STATIC_PAGE_WORKER_POLL_INTERVAL_MS",
        DEFAULT_POLL_INTERVAL_MS,
    );
    let orchestrator_poll_interval = env_u64(
        "STATIC_PAGE_ORCHESTRATOR_POLL_INTERVAL_MS",
        DEFAULT_ORCHESTRATOR_POLL_INTERVAL_MS,
    );
    let orchestrator_max_polls = env_u32(
        "STATIC_PAGE_ORCHESTRATOR_MAX_POLLS",
        DEFAULT_ORCHESTRATOR_MAX_POLLS,
    );

    let storage = PgStorage::connect(&database_url).await?;
    let workflow_catalog = workflow_definitions::catalog();
    let event_bus = EventBus::connect_from_env_or_disabled("PLATFORM_NATS_URL").await;
    let wake_task_key = task_key.as_deref().unwrap_or(DEFAULT_IMAGE_TASK_KEY);
    let wake_subject = workflow_task_enqueued_subject(&queue, wake_task_key);
    let mut task_waker = event_bus
        .subscribe_queue_or_disabled(
            &wake_subject,
            Some(&format!("static_page_worker.{queue}.{wake_task_key}")),
        )
        .await;
    let orchestrator_config = match CodexOrchestratorConfig::from_env() {
        Ok(config) => Some(config),
        Err(error) => {
            tracing::warn!(
                error = ?error,
                "static-page image generation is disabled until Codex orchestrator config is provided"
            );
            None
        }
    };
    let http_client = Client::builder().build()?;

    tracing::info!(
        %queue,
        task_key = task_key.as_deref().unwrap_or("*"),
        %wake_subject,
        event_bus_enabled = event_bus.is_enabled(),
        poll_interval_ms = poll_interval,
        orchestrator_poll_interval_ms = orchestrator_poll_interval,
        orchestrator_max_polls,
        orchestrator_configured = orchestrator_config.is_some(),
        orchestrator_base_url = orchestrator_config.as_ref().map(|config| config.base_url.as_str()).unwrap_or("not-configured"),
        orchestrator_runtime_target = orchestrator_config.as_ref().map(|config| config.runtime_target_id.as_str()).unwrap_or("not-configured"),
        "static-page-worker polling started"
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
                    &http_client,
                    orchestrator_config.as_ref(),
                    orchestrator_poll_interval,
                    orchestrator_max_polls,
                    task,
                )
                .await
                {
                    tracing::error!(error = ?error, "static page image task processing failed");
                }
            }
            Ok(None) => {
                wait_for_next_task_signal(&mut task_waker, poll_interval).await;
            }
            Err(error) => {
                tracing::error!(error = ?error, "static page image failed to claim task");
                wait_for_next_task_signal(&mut task_waker, poll_interval).await;
            }
        }
    }
}

async fn process_task(
    storage: &PgStorage,
    workflow_catalog: &WorkflowCatalog,
    event_bus: &EventBus,
    http_client: &Client,
    orchestrator_config: Option<&CodexOrchestratorConfig>,
    orchestrator_poll_interval_ms: u64,
    orchestrator_max_polls: u32,
    task: domain_model::WorkflowTask,
) -> Result<()> {
    let execution = storage
        .workflow_executions()
        .get_by_id(task.tenant_id, task.execution_id)
        .await?
        .ok_or_else(|| anyhow!("workflow execution {} not found", task.execution_id))?;
    match &execution.kind {
        WorkflowKind::StaticPageImageGeneration => {
            let orchestrator_config = orchestrator_config.ok_or_else(|| {
                anyhow!(
                    "CODEX_ORCHESTRATOR_ACCESS_KEY or CODEX_ORCHESTRATOR_KEY_FILE is required for static page image generation"
                )
            })?;
            process_static_page_image_task(
                storage,
                workflow_catalog,
                event_bus,
                http_client,
                orchestrator_config,
                orchestrator_poll_interval_ms,
                orchestrator_max_polls,
                &execution,
                &task,
            )
            .await
        }
        WorkflowKind::StaticPageRender => {
            process_static_page_render_task(storage, workflow_catalog, event_bus, &execution, &task)
                .await
        }
        _ => Err(anyhow!(
            "workflow execution {} has unexpected kind {}",
            execution.id,
            execution.kind.as_str()
        )),
    }
}

async fn process_static_page_image_task(
    storage: &PgStorage,
    workflow_catalog: &WorkflowCatalog,
    event_bus: &EventBus,
    http_client: &Client,
    orchestrator_config: &CodexOrchestratorConfig,
    orchestrator_poll_interval_ms: u64,
    orchestrator_max_polls: u32,
    execution: &domain_model::WorkflowExecution,
    task: &domain_model::WorkflowTask,
) -> Result<()> {
    let job_id = domain_model::StaticPageImageJobId(context_uuid(
        &execution.context,
        "static_page_image_job_id",
    )?);
    let process_result: Result<()> = async {
        let mut job = storage
            .static_page_image_jobs()
            .get_by_id(task.tenant_id, job_id)
            .await?
            .ok_or_else(|| anyhow!("static page image job {job_id} not found"))?;
        mark_job_running(storage, &mut job).await?;

        let submitted_task = submit_static_page_visual_task(
            http_client,
            orchestrator_config,
            job.id,
            &job.image_prompt_payload,
        )?;
        job.image_prompt_payload = merge_orchestrator_state(
            &job.image_prompt_payload,
            json!({
                "status": submitted_task.status,
                "taskId": submitted_task.id,
                "queuePosition": submitted_task.queue_position,
                "submittedAt": Utc::now(),
            }),
        );
        job.queue_position = submitted_task.queue_position;
        job = storage
            .static_page_image_jobs()
            .update(task.tenant_id, &job)
            .await?;

        let completed_task = poll_until_finished(
            http_client,
            orchestrator_config,
            &submitted_task.id,
            orchestrator_poll_interval_ms,
            orchestrator_max_polls,
            &mut job,
            storage,
        )
        .await?;
        let mut artifact = extract_first_image_artifact(&completed_task)?;
        artifact.asset_key =
            normalize_artifact_asset_key(&artifact.asset_key, &orchestrator_config.base_url);
        mark_job_preview_ready(storage, &job, &artifact.asset_key).await?;
        append_assistant_event(
            storage,
            job.tenant_id,
            job.assistant_run_id,
            "static_page_image_job.preview_ready",
            json!({
                "draft_id": job.draft_id,
                "image_job_id": job.id,
                "orchestrator_task_id": completed_task.id,
                "preview_asset_key": artifact.asset_key,
                "artifact": {
                    "name": artifact.name,
                    "mime_type": artifact.mime_type,
                    "width": artifact.width,
                    "height": artifact.height,
                },
                "artifact_manifest": static_page_image_preview_artifact_manifest(
                    &job,
                    &artifact,
                    &completed_task.id,
                ),
            }),
        )
        .await?;

        platform_api::apply_workflow_signal_with_dependencies(
            storage,
            workflow_catalog,
            event_bus,
            task.tenant_id,
            task.execution_id,
            WorkflowSignal::StepCompleted {
                task_key: task.task_key.clone(),
                output: Some(json!({
                    "static_page_image_job_id": job.id,
                    "preview_asset_key": artifact.asset_key,
                    "orchestrator_task_id": completed_task.id,
                })),
            },
        )
        .await?;

        Ok(())
    }
    .await;

    if let Err(error) = process_result {
        let error_message = error.to_string();
        let retryable = static_page_image_error_should_auto_retry(&error_message);
        if let Ok(mut job) = storage
            .static_page_image_jobs()
            .get_by_id(task.tenant_id, job_id)
            .await
        {
            if let Some(mut job) = job.take() {
                if retryable {
                    let _ = mark_job_retrying(storage, &mut job, &error_message).await;
                    let _ = append_assistant_event(
                        storage,
                        job.tenant_id,
                        job.assistant_run_id,
                        "static_page_image_job.retry_queued",
                        json!({
                            "draft_id": job.draft_id,
                            "image_job_id": job.id,
                            "error": error_message,
                            "retryable": true,
                            "status": "retry_queued",
                        }),
                    )
                    .await;
                } else {
                    let _ = mark_job_failed(storage, &mut job, &error_message).await;
                    let _ = append_assistant_event(
                        storage,
                        job.tenant_id,
                        job.assistant_run_id,
                        "static_page_image_job.failed",
                        json!({
                            "draft_id": job.draft_id,
                            "image_job_id": job.id,
                            "error": error_message,
                            "retryable": false,
                        }),
                    )
                    .await;
                }
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
                "static page image failed to send workflow step_failed signal"
            );
        }
        if retryable {
            if let Err(retry_error) = auto_retry_static_page_image_workflow(
                storage,
                workflow_catalog,
                event_bus,
                task,
                &error_message,
            )
            .await
            {
                tracing::warn!(
                    error = ?retry_error,
                    task_id = %task.id,
                    "static page image auto retry enqueue failed"
                );
            }
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
        static_page_image_job_id = %job_id,
        "static page image task completed"
    );

    Ok(())
}

async fn auto_retry_static_page_image_workflow(
    storage: &PgStorage,
    workflow_catalog: &WorkflowCatalog,
    event_bus: &EventBus,
    task: &domain_model::WorkflowTask,
    error_message: &str,
) -> Result<()> {
    let reason = format!(
        "auto retry after transient static-page image error: {}",
        bounded_text(error_message, 180)
    );
    let retry = platform_api::apply_workflow_signal_with_dependencies(
        storage,
        workflow_catalog,
        event_bus,
        task.tenant_id,
        task.execution_id,
        WorkflowSignal::RetryRequested { reason },
    )
    .await?;
    if retry.execution.status != WorkflowStatus::Pending {
        tracing::warn!(
            execution_id = %task.execution_id,
            status = ?retry.execution.status,
            stage = %retry.execution.stage,
            "static page image auto retry did not return to pending"
        );
        return Ok(());
    }
    platform_api::apply_workflow_signal_with_dependencies(
        storage,
        workflow_catalog,
        event_bus,
        task.tenant_id,
        task.execution_id,
        WorkflowSignal::Start,
    )
    .await?;
    Ok(())
}

async fn process_static_page_render_task(
    storage: &PgStorage,
    workflow_catalog: &WorkflowCatalog,
    event_bus: &EventBus,
    execution: &domain_model::WorkflowExecution,
    task: &domain_model::WorkflowTask,
) -> Result<()> {
    let output_id = StaticPageRenderOutputId(context_uuid(
        &execution.context,
        "static_page_render_output_id",
    )?);
    let process_result: Result<StaticPageRenderTaskOutcome> = async {
        let mut output = storage
            .static_page_render_outputs()
            .get_by_id(task.tenant_id, output_id)
            .await?
            .ok_or_else(|| anyhow!("static page render output {output_id} not found"))?;
        mark_render_output_rendering(storage, &mut output, execution, task).await?;

        let draft = storage
            .static_page_drafts()
            .get_by_id(task.tenant_id, output.draft_id)
            .await?
            .ok_or_else(|| anyhow!("static page draft {} not found", output.draft_id))?;
        let image_job = match output.image_job_id {
            Some(image_job_id) => Some(
                storage
                    .static_page_image_jobs()
                    .get_by_id(task.tenant_id, image_job_id)
                    .await?
                    .ok_or_else(|| anyhow!("static page image job {image_job_id} not found"))?,
            ),
            None => None,
        };
        if image_job
            .as_ref()
            .map(|job| job.status != StaticPageImageJobStatus::Confirmed)
            .unwrap_or(false)
        {
            return Err(anyhow!(
                "static page preview must be confirmed before final render"
            ));
        }
        if let Some(cancelled_execution) =
            load_cancelled_workflow_execution(storage, task.tenant_id, task.execution_id).await?
        {
            let reason = workflow_cancel_reason(
                &cancelled_execution,
                "workflow was cancelled before static page render",
            );
            mark_render_output_cancelled(storage, &mut output, &cancelled_execution, task, &reason)
                .await?;
            return Ok(StaticPageRenderTaskOutcome::Cancelled);
        }

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
        if let Some(cancelled_execution) =
            load_cancelled_workflow_execution(storage, task.tenant_id, task.execution_id).await?
        {
            let reason = workflow_cancel_reason(
                &cancelled_execution,
                "workflow was cancelled before static page render was saved",
            );
            mark_render_output_cancelled(storage, &mut output, &cancelled_execution, task, &reason)
                .await?;
            return Ok(StaticPageRenderTaskOutcome::Cancelled);
        }

        output.status = StaticPageRenderOutputStatus::Rendered;
        output.html = rendered.html;
        output.asset_manifest = merge_render_state(
            &rendered.asset_manifest,
            json!({
                "status": "rendered",
                "executionId": execution.id,
                "taskId": task.id,
                "workflowExecutionId": execution.id,
                "workflowTaskId": task.id,
                "renderedAt": Utc::now(),
            }),
        );
        output = storage
            .static_page_render_outputs()
            .update(task.tenant_id, &output)
            .await?;
        mark_draft_final_rendered(storage, &draft, &output).await?;
        append_assistant_event(
            storage,
            output.tenant_id,
            output.assistant_run_id,
            "static_page_render.created",
            json!({
                "draft_id": output.draft_id,
                "render_output_id": output.id,
                "image_job_id": output.image_job_id,
                "workflow_execution_id": execution.id,
            }),
        )
        .await?;

        platform_api::apply_workflow_signal_with_dependencies(
            storage,
            workflow_catalog,
            event_bus,
            task.tenant_id,
            task.execution_id,
            WorkflowSignal::StepCompleted {
                task_key: task.task_key.clone(),
                output: Some(json!({
                    "static_page_render_output_id": output.id,
                    "static_page_draft_id": output.draft_id,
                })),
            },
        )
        .await?;

        Ok(StaticPageRenderTaskOutcome::Rendered)
    }
    .await;

    let outcome = match process_result {
        Ok(outcome) => outcome,
        Err(error) => {
            let error_message = error.to_string();
            if let Ok(Some(mut output)) = storage
                .static_page_render_outputs()
                .get_by_id(task.tenant_id, output_id)
                .await
            {
                let _ = mark_render_output_failed(
                    storage,
                    &mut output,
                    execution,
                    task,
                    &error_message,
                )
                .await;
                let _ = append_assistant_event(
                    storage,
                    output.tenant_id,
                    output.assistant_run_id,
                    "static_page_render.failed",
                    json!({
                        "draft_id": output.draft_id,
                        "render_output_id": output.id,
                        "error": error_message,
                        "workflow_execution_id": execution.id,
                    }),
                )
                .await;
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
                    "static page render failed to send workflow step_failed signal"
                );
            }
            storage
                .workflow_tasks()
                .mark_failed(task.id, &error_message, Utc::now())
                .await?;
            return Err(error);
        }
    };

    storage
        .workflow_tasks()
        .mark_succeeded(task.id, Utc::now())
        .await?;

    tracing::info!(
        task_id = %task.id,
        execution_id = %task.execution_id,
        static_page_render_output_id = %output_id,
        outcome = outcome.as_str(),
        "static page render task settled"
    );

    Ok(())
}

async fn poll_until_finished(
    http_client: &Client,
    orchestrator_config: &CodexOrchestratorConfig,
    task_id: &str,
    poll_interval_ms: u64,
    max_polls: u32,
    job: &mut StaticPageImageJob,
    storage: &PgStorage,
) -> Result<static_page_worker::OrchestratorTaskView> {
    for _ in 0..max_polls {
        let task = match poll_static_page_visual_task(http_client, orchestrator_config, task_id) {
            Ok(task) => task,
            Err(error) if orchestrator_poll_error_is_transient(&error) => {
                job.status = StaticPageImageJobStatus::Running;
                job.queue_position = None;
                job.image_prompt_payload = merge_orchestrator_state(
                    &job.image_prompt_payload,
                    json!({
                        "status": "poll_retry",
                        "taskId": task_id,
                        "transientError": bounded_error_message(&error, 240),
                        "updatedAt": Utc::now(),
                    }),
                );
                *job = storage
                    .static_page_image_jobs()
                    .update(job.tenant_id, job)
                    .await?;
                tokio::time::sleep(Duration::from_millis(poll_interval_ms)).await;
                continue;
            }
            Err(error) => return Err(error),
        };
        match task.status.as_str() {
            "completed" => return Ok(task),
            "failed" | "cancelled" => return Err(anyhow!(task_failure_message(&task))),
            status if orchestrator_task_status_is_pending(status) => {
                job.status = StaticPageImageJobStatus::Running;
                job.queue_position = task.queue_position;
                job.image_prompt_payload = merge_orchestrator_state(
                    &job.image_prompt_payload,
                    json!({
                        "status": task.status,
                        "taskId": task.id,
                        "queuePosition": task.queue_position,
                        "updatedAt": Utc::now(),
                    }),
                );
                *job = storage
                    .static_page_image_jobs()
                    .update(job.tenant_id, job)
                    .await?;
            }
            other => return Err(anyhow!("orchestrator returned unknown task status {other}")),
        }
        tokio::time::sleep(Duration::from_millis(poll_interval_ms)).await;
    }
    Err(anyhow!(
        "orchestrator task {task_id} did not finish after {max_polls} polls"
    ))
}

fn orchestrator_poll_error_is_transient(error: &anyhow::Error) -> bool {
    let message = error.to_string().to_ascii_lowercase();
    message.contains("error decoding response body")
        || message.contains("eof while parsing")
        || message.contains("json decode failed")
        || message.contains("connection")
        || message.contains("timed out")
        || message.contains("status=502")
        || message.contains("status=503")
        || message.contains("status=504")
}

fn static_page_image_error_should_auto_retry(error_message: &str) -> bool {
    let message = error_message.to_ascii_lowercase();
    message.contains("did not finish after")
        || message.contains("error decoding response body")
        || message.contains("eof while parsing")
        || message.contains("json decode failed")
        || message.contains("connection")
        || message.contains("timed out")
        || message.contains("timeout")
        || message.contains("status=502")
        || message.contains("status=503")
        || message.contains("status=504")
}

fn bounded_error_message(error: &anyhow::Error, max_chars: usize) -> String {
    let text = error.to_string();
    bounded_text(&text, max_chars)
}

fn bounded_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut output = text.chars().take(max_chars).collect::<String>();
    output.push_str("...[truncated]");
    output
}

fn orchestrator_task_status_is_pending(status: &str) -> bool {
    matches!(
        status,
        "queued"
            | "submitted"
            | "running"
            | "pending"
            | "processing"
            | "in_progress"
            | "retry_wait"
            | "retrying"
            | "waking_runtime"
            | "waiting"
    )
}

async fn mark_job_running(storage: &PgStorage, job: &mut StaticPageImageJob) -> Result<()> {
    job.status = StaticPageImageJobStatus::Running;
    job.failure_reason = None;
    job.image_prompt_payload = merge_orchestrator_state(
        &job.image_prompt_payload,
        json!({
            "status": "submitting",
            "updatedAt": Utc::now(),
        }),
    );
    *job = storage
        .static_page_image_jobs()
        .update(job.tenant_id, job)
        .await?;
    Ok(())
}

async fn mark_job_preview_ready(
    storage: &PgStorage,
    job: &StaticPageImageJob,
    asset_key: &str,
) -> Result<()> {
    let mut next_job = job.clone();
    next_job.status = StaticPageImageJobStatus::PreviewReady;
    next_job.queue_position = None;
    next_job.preview_asset_key = Some(asset_key.to_string());
    next_job.failure_reason = None;
    next_job.image_prompt_payload = merge_orchestrator_state(
        &next_job.image_prompt_payload,
        json!({
            "status": "completed",
            "previewAssetKey": asset_key,
            "updatedAt": Utc::now(),
        }),
    );
    storage
        .static_page_image_jobs()
        .update(next_job.tenant_id, &next_job)
        .await?;

    if let Some(mut draft) = storage
        .static_page_drafts()
        .get_by_id(job.tenant_id, job.draft_id)
        .await?
    {
        draft.status = StaticPageDraftStatus::Previewed;
        draft.draft_payload = mark_draft_preview_ready(&draft.draft_payload, job, asset_key);
        storage
            .static_page_drafts()
            .update(draft.tenant_id, &draft)
            .await?;
    }

    Ok(())
}

async fn mark_job_failed(
    storage: &PgStorage,
    job: &mut StaticPageImageJob,
    error_message: &str,
) -> Result<()> {
    job.status = StaticPageImageJobStatus::Failed;
    job.queue_position = None;
    job.failure_reason = Some(error_message.to_string());
    job.image_prompt_payload = merge_orchestrator_state(
        &job.image_prompt_payload,
        json!({
            "status": "failed",
            "error": error_message,
            "updatedAt": Utc::now(),
        }),
    );
    *job = storage
        .static_page_image_jobs()
        .update(job.tenant_id, job)
        .await?;

    if let Some(mut draft) = storage
        .static_page_drafts()
        .get_by_id(job.tenant_id, job.draft_id)
        .await?
    {
        draft.status = StaticPageDraftStatus::Planned;
        draft.draft_payload = mark_draft_image_job_failed(&draft.draft_payload, job, error_message);
        storage
            .static_page_drafts()
            .update(draft.tenant_id, &draft)
            .await?;
    }

    Ok(())
}

async fn mark_job_retrying(
    storage: &PgStorage,
    job: &mut StaticPageImageJob,
    error_message: &str,
) -> Result<()> {
    job.status = StaticPageImageJobStatus::Running;
    job.queue_position = None;
    job.failure_reason = Some(error_message.to_string());
    job.image_prompt_payload = merge_orchestrator_state(
        &job.image_prompt_payload,
        json!({
            "status": "retry_queued",
            "retryable": true,
            "lastError": bounded_text(error_message, 240),
            "updatedAt": Utc::now(),
        }),
    );
    *job = storage
        .static_page_image_jobs()
        .update(job.tenant_id, job)
        .await?;

    if let Some(mut draft) = storage
        .static_page_drafts()
        .get_by_id(job.tenant_id, job.draft_id)
        .await?
    {
        draft.status = StaticPageDraftStatus::Queued;
        draft.draft_payload =
            mark_draft_image_job_retrying(&draft.draft_payload, job, error_message);
        storage
            .static_page_drafts()
            .update(draft.tenant_id, &draft)
            .await?;
    }

    Ok(())
}

async fn mark_render_output_rendering(
    storage: &PgStorage,
    output: &mut StaticPageRenderOutput,
    execution: &domain_model::WorkflowExecution,
    task: &domain_model::WorkflowTask,
) -> Result<()> {
    output.status = StaticPageRenderOutputStatus::Rendering;
    output.asset_manifest = merge_render_state(
        &output.asset_manifest,
        json!({
            "status": "rendering",
            "executionId": execution.id,
            "taskId": task.id,
            "workflowExecutionId": execution.id,
            "workflowTaskId": task.id,
            "updatedAt": Utc::now(),
        }),
    );
    *output = storage
        .static_page_render_outputs()
        .update(output.tenant_id, output)
        .await?;
    Ok(())
}

async fn mark_render_output_failed(
    storage: &PgStorage,
    output: &mut StaticPageRenderOutput,
    execution: &domain_model::WorkflowExecution,
    task: &domain_model::WorkflowTask,
    error_message: &str,
) -> Result<()> {
    output.status = StaticPageRenderOutputStatus::Failed;
    output.asset_manifest = merge_render_state(
        &output.asset_manifest,
        json!({
            "status": "failed",
            "executionId": execution.id,
            "taskId": task.id,
            "workflowExecutionId": execution.id,
            "workflowTaskId": task.id,
            "error": error_message,
            "lastError": {
                "message": error_message,
            },
            "updatedAt": Utc::now(),
        }),
    );
    *output = storage
        .static_page_render_outputs()
        .update(output.tenant_id, output)
        .await?;
    Ok(())
}

async fn mark_render_output_cancelled(
    storage: &PgStorage,
    output: &mut StaticPageRenderOutput,
    execution: &domain_model::WorkflowExecution,
    task: &domain_model::WorkflowTask,
    reason: &str,
) -> Result<()> {
    output.status = StaticPageRenderOutputStatus::Cancelled;
    output.asset_manifest = merge_render_state(
        &output.asset_manifest,
        json!({
            "status": "cancelled",
            "executionId": execution.id,
            "taskId": task.id,
            "workflowExecutionId": execution.id,
            "workflowTaskId": task.id,
            "cancelReason": reason,
            "updatedAt": Utc::now(),
        }),
    );
    *output = storage
        .static_page_render_outputs()
        .update(output.tenant_id, output)
        .await?;
    Ok(())
}

async fn load_cancelled_workflow_execution(
    storage: &PgStorage,
    tenant_id: TenantId,
    execution_id: domain_model::WorkflowExecutionId,
) -> Result<Option<domain_model::WorkflowExecution>> {
    let execution = storage
        .workflow_executions()
        .get_by_id(tenant_id, execution_id)
        .await?
        .ok_or_else(|| anyhow!("workflow execution {execution_id} not found"))?;
    Ok((execution.status == WorkflowStatus::Cancelled).then_some(execution))
}

fn workflow_cancel_reason(execution: &domain_model::WorkflowExecution, fallback: &str) -> String {
    execution
        .context
        .get("cancel_reason")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(fallback)
        .to_string()
}

async fn mark_draft_final_rendered(
    storage: &PgStorage,
    draft: &StaticPageDraft,
    output: &StaticPageRenderOutput,
) -> Result<()> {
    let mut next_draft = draft.clone();
    next_draft.status = StaticPageDraftStatus::Rendered;
    next_draft.draft_payload = mark_draft_final_rendered_payload(&draft.draft_payload, output);
    storage
        .static_page_drafts()
        .update(next_draft.tenant_id, &next_draft)
        .await?;
    Ok(())
}

fn mark_draft_final_rendered_payload(payload: &Value, output: &StaticPageRenderOutput) -> Value {
    let mut object = payload.as_object().cloned().unwrap_or_default();
    object.insert("status".to_string(), Value::String("rendered".to_string()));
    let final_page = json!({
        "status": "rendered",
        "renderer": "static-page-renderer-v1",
        "renderOutputId": output.id,
        "imageJobId": output.image_job_id,
        "assetManifest": output.asset_manifest,
    });
    object.insert("finalPage".to_string(), final_page.clone());
    object.insert("final_page".to_string(), final_page);
    Value::Object(object)
}

fn merge_render_state(manifest: &Value, state: Value) -> Value {
    let mut object = manifest.as_object().cloned().unwrap_or_default();
    let mut workflow = object
        .get("workflow")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if let Some(state_object) = state.as_object() {
        if let Some(status) = state_object.get("status").and_then(Value::as_str) {
            object.insert("status".to_string(), Value::String(status.to_string()));
        }
        for (key, value) in state_object {
            workflow.insert(key.clone(), value.clone());
        }
        if !workflow.contains_key("lastError") {
            if let Some(error) = state_object.get("error") {
                workflow.insert(
                    "lastError".to_string(),
                    match error.as_str() {
                        Some(message) => json!({ "message": message }),
                        None => error.clone(),
                    },
                );
            }
        }
    }
    object.insert("workflow".to_string(), Value::Object(workflow));
    Value::Object(object)
}

fn mark_draft_preview_ready(payload: &Value, job: &StaticPageImageJob, asset_key: &str) -> Value {
    let mut object = payload.as_object().cloned().unwrap_or_default();
    object.insert(
        "status".to_string(),
        Value::String("preview_ready".to_string()),
    );
    object.insert(
        "imageJob".to_string(),
        json!({
            "id": job.id,
            "status": "preview_ready",
            "queuePosition": null,
            "queueMessage": "效果图已生成，将自动继续制作页面。",
        }),
    );
    object.insert(
        "previewImage".to_string(),
        json!({
            "kind": "static-page-effect-preview",
            "assetKey": asset_key,
            "imageJobId": job.id,
            "title": "静态页效果图",
            "subtitle": "由 Codex 远程生图队列生成",
            "modules": [],
        }),
    );
    let preview_contract = json!({
        "version": 1,
        "kind": "static-page-preview-contract",
        "status": "preview_ready",
        "imageJobId": job.id,
        "assetKey": asset_key,
        "queuePosition": null,
        "renderExpectation": "final HTML/CSS/SVG should reproduce the generated preview without baking editable text or charts into the image",
    });
    object.insert("previewContract".to_string(), preview_contract.clone());
    object.insert("preview_contract".to_string(), preview_contract);
    Value::Object(object)
}

fn mark_draft_image_job_failed(
    payload: &Value,
    job: &StaticPageImageJob,
    error_message: &str,
) -> Value {
    let mut object = payload.as_object().cloned().unwrap_or_default();
    object.insert("status".to_string(), Value::String("planning".to_string()));
    object.insert(
        "imageJob".to_string(),
        json!({
            "id": job.id,
            "status": "failed",
            "queuePosition": null,
            "queueMessage": error_message,
        }),
    );
    let preview_contract = json!({
        "version": 1,
        "kind": "static-page-preview-contract",
        "status": "failed",
        "imageJobId": job.id,
        "assetKey": null,
        "queuePosition": null,
        "failureReason": error_message,
        "renderExpectation": "final HTML/CSS/SVG should reproduce the generated preview without baking editable text or charts into the image",
    });
    object.insert("previewContract".to_string(), preview_contract.clone());
    object.insert("preview_contract".to_string(), preview_contract);
    Value::Object(object)
}

fn mark_draft_image_job_retrying(
    payload: &Value,
    job: &StaticPageImageJob,
    error_message: &str,
) -> Value {
    let mut object = payload.as_object().cloned().unwrap_or_default();
    let last_error = bounded_text(error_message, 240);
    object.insert("status".to_string(), Value::String("queued".to_string()));
    object.insert(
        "imageJob".to_string(),
        json!({
            "id": job.id,
            "status": "retry_queued",
            "queuePosition": null,
            "queueMessage": "效果图生成遇到临时问题，已进入重试队列。",
            "retryable": true,
            "lastError": last_error,
        }),
    );
    let preview_contract = json!({
        "version": 1,
        "kind": "static-page-preview-contract",
        "status": "retry_queued",
        "imageJobId": job.id,
        "assetKey": null,
        "queuePosition": null,
        "retryable": true,
        "lastError": last_error,
        "renderExpectation": "final HTML/CSS/SVG should reproduce the generated preview without baking editable text or charts into the image",
    });
    object.insert("previewContract".to_string(), preview_contract.clone());
    object.insert("preview_contract".to_string(), preview_contract);
    Value::Object(object)
}

async fn append_assistant_event(
    storage: &PgStorage,
    tenant_id: TenantId,
    assistant_run_id: AssistantRunId,
    event_name: &str,
    payload: Value,
) -> Result<()> {
    storage
        .assistant_runs()
        .append_event(
            tenant_id,
            assistant_run_id,
            &NewAssistantRunEvent {
                event_name: event_name.to_string(),
                payload,
                created_at: Utc::now(),
            },
        )
        .await?;
    Ok(())
}

fn static_page_image_preview_artifact_manifest(
    job: &StaticPageImageJob,
    artifact: &StaticPageVisualArtifact,
    orchestrator_task_id: &str,
) -> Value {
    let primary_url = if artifact.asset_key.starts_with("http://")
        || artifact.asset_key.starts_with("https://")
    {
        Value::String(artifact.asset_key.clone())
    } else {
        Value::Null
    };
    let links = primary_url
        .as_str()
        .map(|url| json!([{"rel": "preview", "url": url}]))
        .unwrap_or_else(|| json!([]));
    let preview_asset_key_is_embedded =
        artifact.asset_key.starts_with("data:image/") || artifact.asset_key.starts_with("blob:");
    let preview_asset_key = if preview_asset_key_is_embedded {
        Value::Null
    } else {
        Value::String(artifact.asset_key.clone())
    };

    json!({
        "schema": "v3.output_artifact_manifest",
        "schema_version": 1,
        "artifact_type": "static_page_image_preview",
        "artifact_kind": "image2_visual_contract",
        "title": "static_page_image2_preview",
        "status": "preview_ready",
        "primary_url": primary_url,
        "links": links,
        "refs": {
            "draft_id": job.draft_id,
            "image_job_id": job.id,
            "orchestrator_task_id": orchestrator_task_id,
            "preview_asset_key": preview_asset_key,
            "preview_asset_key_redacted": preview_asset_key_is_embedded,
        },
        "safety": {
            "customer_visible": true,
            "credentials_exposed": false,
            "raw_logs_exposed": false,
            "generated_artifact_only": true,
            "overwrite_allowed": false,
        },
    })
}

async fn wait_for_next_task_signal(task_waker: &mut EventSubscription, poll_interval_ms: u64) {
    if let Some(event) = task_waker
        .wait_for_event(Duration::from_millis(poll_interval_ms))
        .await
    {
        tracing::debug!(subject = %event.subject, "static page worker received task wake signal");
    }
}

fn env_u64(key: &str, default_value: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default_value)
}

fn env_u32(key: &str, default_value: u32) -> u32 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(default_value)
}

fn optional_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain_model::{WorkflowExecution, WorkflowExecutionId, WorkflowTask, WorkflowTaskStatus};
    use storage::{
        NewAssistantRun, NewStaticPageDraft, NewStaticPageRenderOutput, NewWorkflowTask,
    };
    use test_fixtures::{
        local_postgres_storage, reset_local_postgres_storage, shared_local_postgres_test_lock,
    };

    #[test]
    fn orchestrator_retry_wait_is_treated_as_pending_status() {
        assert!(orchestrator_task_status_is_pending("retry_wait"));
        assert!(orchestrator_task_status_is_pending("waking_runtime"));
        assert!(orchestrator_task_status_is_pending("processing"));
        assert!(!orchestrator_task_status_is_pending("failed"));
        assert!(!orchestrator_task_status_is_pending("completed"));
    }

    #[test]
    fn orchestrator_poll_decode_errors_are_transient() {
        assert!(orchestrator_poll_error_is_transient(&anyhow!(
            "error decoding response body"
        )));
        assert!(orchestrator_poll_error_is_transient(&anyhow!(
            "orchestrator response JSON decode failed: status=200 body_chars=0 body_excerpt=\"\": EOF while parsing a value at line 1 column 0"
        )));
        assert!(!orchestrator_poll_error_is_transient(&anyhow!(
            "orchestrator task ended with status failed"
        )));
    }

    #[test]
    fn default_orchestrator_poll_window_allows_slow_cloudflare_tasks() {
        assert!(
            u64::from(DEFAULT_ORCHESTRATOR_MAX_POLLS) * DEFAULT_ORCHESTRATOR_POLL_INTERVAL_MS
                >= 30 * 60 * 1_000
        );
    }

    #[test]
    fn static_page_image_auto_retry_classifies_transient_errors() {
        assert!(static_page_image_error_should_auto_retry(
            "orchestrator task task_1 did not finish after 120 polls"
        ));
        assert!(static_page_image_error_should_auto_retry(
            "error decoding response body"
        ));
        assert!(static_page_image_error_should_auto_retry(
            "orchestrator poll failed: status=503"
        ));
        assert!(!static_page_image_error_should_auto_retry(
            "orchestrator task ended with status failed"
        ));
    }

    #[test]
    fn static_page_image_preview_manifest_exposes_safe_visual_contract_refs() {
        let now = Utc::now();
        let job = StaticPageImageJob {
            id: domain_model::StaticPageImageJobId::new(),
            tenant_id: TenantId::new(),
            draft_id: domain_model::StaticPageDraftId::new(),
            assistant_run_id: AssistantRunId::new(),
            status: StaticPageImageJobStatus::PreviewReady,
            queue_position: None,
            image_prompt_payload: json!({}),
            preview_asset_key: None,
            failure_reason: None,
            confirmed_at: None,
            created_at: now,
            updated_at: now,
        };
        let artifact = StaticPageVisualArtifact {
            asset_key: "https://souleye.cc/artifacts/preview.png".to_string(),
            name: Some("preview.png".to_string()),
            mime_type: Some("image/png".to_string()),
            width: Some(1024),
            height: Some(768),
        };

        let manifest = static_page_image_preview_artifact_manifest(&job, &artifact, "cf-task-001");

        assert_eq!(manifest["schema"], json!("v3.output_artifact_manifest"));
        assert_eq!(
            manifest["artifact_type"],
            json!("static_page_image_preview")
        );
        assert_eq!(manifest["artifact_kind"], json!("image2_visual_contract"));
        assert_eq!(
            manifest["primary_url"],
            json!("https://souleye.cc/artifacts/preview.png")
        );
        assert_eq!(
            manifest["links"][0],
            json!({"rel": "preview", "url": "https://souleye.cc/artifacts/preview.png"})
        );
        assert_eq!(manifest["refs"]["image_job_id"], json!(job.id));
        assert_eq!(
            manifest["refs"]["orchestrator_task_id"],
            json!("cf-task-001")
        );
        assert_eq!(manifest["safety"]["credentials_exposed"], json!(false));
    }

    #[test]
    fn static_page_image_preview_manifest_redacts_embedded_data_urls() {
        let now = Utc::now();
        let job = StaticPageImageJob {
            id: domain_model::StaticPageImageJobId::new(),
            tenant_id: TenantId::new(),
            draft_id: domain_model::StaticPageDraftId::new(),
            assistant_run_id: AssistantRunId::new(),
            status: StaticPageImageJobStatus::PreviewReady,
            queue_position: None,
            image_prompt_payload: json!({}),
            preview_asset_key: None,
            failure_reason: None,
            confirmed_at: None,
            created_at: now,
            updated_at: now,
        };
        let artifact = StaticPageVisualArtifact {
            asset_key: "data:image/png;base64,abc123".to_string(),
            name: None,
            mime_type: Some("image/png".to_string()),
            width: None,
            height: None,
        };

        let manifest = static_page_image_preview_artifact_manifest(&job, &artifact, "cf-task-002");

        assert_eq!(manifest["primary_url"], Value::Null);
        assert_eq!(manifest["links"], json!([]));
        assert_eq!(manifest["refs"]["preview_asset_key"], Value::Null);
        assert_eq!(manifest["refs"]["preview_asset_key_redacted"], json!(true));
        assert!(!manifest.to_string().contains("abc123"));
    }

    #[test]
    fn mark_draft_image_job_retrying_preserves_retry_status() {
        let now = Utc::now();
        let job = StaticPageImageJob {
            id: domain_model::StaticPageImageJobId::new(),
            tenant_id: TenantId::new(),
            draft_id: domain_model::StaticPageDraftId::new(),
            assistant_run_id: AssistantRunId::new(),
            status: StaticPageImageJobStatus::Running,
            queue_position: None,
            image_prompt_payload: json!({}),
            preview_asset_key: None,
            failure_reason: Some("previous transient failure".to_string()),
            confirmed_at: None,
            created_at: now,
            updated_at: now,
        };

        let payload = mark_draft_image_job_retrying(
            &json!({
                "version": 1,
                "modules": []
            }),
            &job,
            "orchestrator task task_1 did not finish after 120 polls",
        );

        assert_eq!(payload["status"], json!("queued"));
        assert_eq!(payload["imageJob"]["status"], json!("retry_queued"));
        assert_eq!(payload["imageJob"]["retryable"], json!(true));
        assert_eq!(payload["previewContract"]["status"], json!("retry_queued"));
        assert_eq!(payload["preview_contract"]["status"], json!("retry_queued"));
        assert_eq!(payload["previewContract"]["retryable"], json!(true));
    }

    #[test]
    fn merge_render_state_preserves_existing_workflow_fields() {
        let manifest = json!({
            "status": "queued",
            "workflow": {
                "executionId": "execution-1",
                "taskId": "task-1",
                "queuePosition": 2
            },
            "export_package": {
                "kind": "static-page-export-package"
            }
        });

        let merged = merge_render_state(
            &manifest,
            json!({
                "status": "rendering",
                "workflowExecutionId": "execution-1",
                "updatedAt": "2026-05-09T00:00:00Z"
            }),
        );

        assert_eq!(merged["status"], json!("rendering"));
        assert_eq!(merged["workflow"]["taskId"], json!("task-1"));
        assert_eq!(merged["workflow"]["queuePosition"], json!(2));
        assert_eq!(
            merged["workflow"]["workflowExecutionId"],
            json!("execution-1")
        );
        assert_eq!(
            merged["export_package"]["kind"],
            json!("static-page-export-package")
        );
    }

    #[test]
    fn merge_render_state_adds_frontend_readable_last_error() {
        let merged = merge_render_state(
            &json!({
                "status": "rendering",
                "workflow": {
                    "executionId": "execution-1",
                    "taskId": "task-1"
                }
            }),
            json!({
                "status": "failed",
                "error": "renderer failed to produce html"
            }),
        );

        assert_eq!(merged["status"], json!("failed"));
        assert_eq!(merged["workflow"]["taskId"], json!("task-1"));
        assert_eq!(
            merged["workflow"]["error"],
            json!("renderer failed to produce html")
        );
        assert_eq!(
            merged["workflow"]["lastError"]["message"],
            json!("renderer failed to produce html")
        );
    }

    #[test]
    fn merge_render_state_preserves_task_for_cancelled_outputs() {
        let merged = merge_render_state(
            &json!({
                "status": "rendering",
                "workflow": {
                    "executionId": "execution-1",
                    "taskId": "task-1"
                }
            }),
            json!({
                "status": "cancelled",
                "cancelReason": "customer changed render request"
            }),
        );

        assert_eq!(merged["status"], json!("cancelled"));
        assert_eq!(merged["workflow"]["taskId"], json!("task-1"));
        assert_eq!(
            merged["workflow"]["cancelReason"],
            json!("customer changed render request")
        );
    }

    async fn setup_render_task_fixture(
        storage: &PgStorage,
        workflow_status: WorkflowStatus,
    ) -> (
        domain_model::TenantId,
        domain_model::StaticPageDraftId,
        StaticPageRenderOutputId,
        WorkflowExecution,
        WorkflowTask,
    ) {
        let now = Utc::now();
        let tenant = storage
            .ensure_tenant(
                &format!("static-page-worker-test-{}", uuid::Uuid::new_v4()),
                "Static Page Worker Test",
            )
            .await
            .expect("tenant should exist");
        let run = storage
            .assistant_runs()
            .create(
                tenant.id,
                &NewAssistantRun {
                    user_id: None,
                    local_thread_id: Some("static-page-worker-test-thread".to_string()),
                    user_prompt: "生成一页经营分析静态页".to_string(),
                    startup_briefing: json!({}),
                    selected_scope: json!({"mode": "ordinary_chat"}),
                    scope_candidates: json!([]),
                    context_policy: json!({}),
                    evidence_state: json!({}),
                    service_lane: "placeholder".to_string(),
                    execution_trail: json!([]),
                    output_artifacts: json!([]),
                    runtime_manifest: json!({}),
                    created_at: now,
                },
            )
            .await
            .expect("assistant run should be created");
        let draft = storage
            .static_page_drafts()
            .create(
                tenant.id,
                &NewStaticPageDraft {
                    assistant_run_id: run.id,
                    owner_user_id: None,
                    title: "经营分析静态页".to_string(),
                    status: StaticPageDraftStatus::Confirmed,
                    selected_scope: json!({"mode": "ordinary_chat"}),
                    visibility_snapshot: json!({"policy": "test"}),
                    source_refs: json!([]),
                    draft_payload: json!({
                        "version": 1,
                        "status": "effect_confirmed",
                        "styleDirection": "decision-brief",
                        "previewContract": {
                            "status": "confirmed",
                            "assetKey": "previews/static-page-test.png"
                        },
                        "modelSummary": "核心增长来自高价值客户。",
                        "mobileOrder": ["trend", "hero"],
                        "modules": [{
                            "id": "hero",
                            "title": "核心判断",
                            "content": "增长放缓但结构改善。",
                            "dataBinding": { "label": "订单数据摘要" },
                            "visualization": { "type": "headline", "label": "关键结论" },
                            "layout": { "x": 0, "y": 0, "w": 5, "h": 3 }
                        }, {
                            "id": "trend",
                            "title": "趋势变化",
                            "content": "订单转化率连续三周回升。",
                            "dataBinding": {
                                "label": "订单趋势",
                                "values": [
                                    { "label": "一月", "value": 42 },
                                    { "label": "二月", "value": 58 },
                                    { "label": "三月", "value": 76 }
                                ]
                            },
                            "visualization": { "type": "bar-chart", "label": "分类对比柱状图" },
                            "layout": { "x": 5, "y": 0, "w": 7, "h": 3 }
                        }]
                    }),
                    created_at: now,
                },
            )
            .await
            .expect("static page draft should be created");
        let execution_id = WorkflowExecutionId::new();
        let output = storage
            .static_page_render_outputs()
            .create(
                tenant.id,
                &NewStaticPageRenderOutput {
                    draft_id: draft.id,
                    assistant_run_id: run.id,
                    owner_user_id: None,
                    image_job_id: None,
                    status: StaticPageRenderOutputStatus::Queued,
                    html: String::new(),
                    asset_manifest: json!({
                        "status": "queued",
                        "workflow": {
                            "executionId": execution_id
                        },
                        "export_package": {
                            "kind": "static-page-export-package"
                        }
                    }),
                    created_at: now,
                },
            )
            .await
            .expect("static page render output should be created");
        let (stage, context) = if workflow_status == WorkflowStatus::Cancelled {
            (
                "cancelled".to_string(),
                json!({
                    "queue": DEFAULT_QUEUE,
                    "task_key": "render_static_page",
                    "static_page_render_output_id": output.id,
                    "static_page_draft_id": draft.id,
                    "cancel_reason": "customer changed render request"
                }),
            )
        } else {
            (
                "render_static_page".to_string(),
                json!({
                    "queue": DEFAULT_QUEUE,
                    "task_key": "render_static_page",
                    "static_page_render_output_id": output.id,
                    "static_page_draft_id": draft.id
                }),
            )
        };
        let execution = WorkflowExecution {
            id: execution_id,
            tenant_id: tenant.id,
            dataset_id: None,
            report_plan_id: None,
            kind: WorkflowKind::StaticPageRender,
            version: "0.1.0".to_string(),
            stage,
            status: workflow_status,
            attempt: 1,
            context,
            created_at: now,
            updated_at: now,
        };
        storage
            .workflow_executions()
            .create(&execution)
            .await
            .expect("workflow execution should be created");
        let task = storage
            .workflow_tasks()
            .create(
                &execution,
                &NewWorkflowTask {
                    queue: DEFAULT_QUEUE.to_string(),
                    task_key: "render_static_page".to_string(),
                    payload: json!({
                        "execution_id": execution.id,
                        "kind": execution.kind.as_str()
                    }),
                    available_at: now,
                    max_attempts: 3,
                },
                now,
            )
            .await
            .expect("workflow task should be created");
        (tenant.id, draft.id, output.id, execution, task)
    }

    #[tokio::test]
    async fn render_task_writes_durable_output_and_completes_workflow() {
        let _guard = shared_local_postgres_test_lock().lock().await;
        let storage = match local_postgres_storage().await {
            Ok(storage) => storage,
            Err(reason) => {
                eprintln!("skipping static page render worker integration test: {reason}");
                return;
            }
        };
        reset_local_postgres_storage(&storage)
            .await
            .expect("storage should reset");
        let workflow_catalog = workflow_definitions::catalog();
        storage
            .sync_workflow_definitions(&workflow_catalog.descriptors())
            .await
            .expect("workflow definitions should sync");
        let (tenant_id, draft_id, output_id, execution, task) =
            setup_render_task_fixture(&storage, WorkflowStatus::Running).await;
        let event_bus = EventBus::Disabled;

        process_static_page_render_task(&storage, &workflow_catalog, &event_bus, &execution, &task)
            .await
            .expect("render task should complete");

        let output = storage
            .static_page_render_outputs()
            .get_by_id(tenant_id, output_id)
            .await
            .expect("render output should load")
            .expect("render output should exist");
        assert_eq!(output.status, StaticPageRenderOutputStatus::Rendered);
        assert!(output.html.contains("核心判断"));
        assert_eq!(output.asset_manifest["status"], json!("rendered"));
        assert_eq!(output.asset_manifest["workflow"]["taskId"], json!(task.id));
        assert_eq!(
            output.asset_manifest["export_package"]["kind"],
            json!("static-page-export-package")
        );

        let draft = storage
            .static_page_drafts()
            .get_by_id(tenant_id, draft_id)
            .await
            .expect("draft should load")
            .expect("draft should exist");
        assert_eq!(draft.status, StaticPageDraftStatus::Rendered);
        assert_eq!(
            draft.draft_payload["finalPage"]["status"],
            json!("rendered")
        );

        let stored_execution = storage
            .workflow_executions()
            .get_by_id(tenant_id, execution.id)
            .await
            .expect("workflow execution should load")
            .expect("workflow execution should exist");
        assert_eq!(stored_execution.status, WorkflowStatus::Succeeded);
        assert_eq!(
            stored_execution.context["last_output"]["static_page_render_output_id"],
            json!(output_id)
        );
        let tasks = storage
            .workflow_tasks()
            .list_by_execution(execution.id)
            .await
            .expect("workflow tasks should load");
        assert_eq!(tasks[0].status, WorkflowTaskStatus::Succeeded);
    }

    #[tokio::test]
    async fn render_task_does_not_overwrite_cancelled_workflow_output() {
        let _guard = shared_local_postgres_test_lock().lock().await;
        let storage = match local_postgres_storage().await {
            Ok(storage) => storage,
            Err(reason) => {
                eprintln!("skipping static page render cancellation test: {reason}");
                return;
            }
        };
        reset_local_postgres_storage(&storage)
            .await
            .expect("storage should reset");
        let workflow_catalog = workflow_definitions::catalog();
        storage
            .sync_workflow_definitions(&workflow_catalog.descriptors())
            .await
            .expect("workflow definitions should sync");
        let (tenant_id, draft_id, output_id, execution, task) =
            setup_render_task_fixture(&storage, WorkflowStatus::Cancelled).await;
        let event_bus = EventBus::Disabled;

        process_static_page_render_task(&storage, &workflow_catalog, &event_bus, &execution, &task)
            .await
            .expect("cancelled render task should settle without rendering");

        let output = storage
            .static_page_render_outputs()
            .get_by_id(tenant_id, output_id)
            .await
            .expect("render output should load")
            .expect("render output should exist");
        assert_eq!(output.status, StaticPageRenderOutputStatus::Cancelled);
        assert_eq!(output.html, "");
        assert_eq!(output.asset_manifest["status"], json!("cancelled"));
        assert_eq!(
            output.asset_manifest["workflow"]["cancelReason"],
            json!("customer changed render request")
        );

        let draft = storage
            .static_page_drafts()
            .get_by_id(tenant_id, draft_id)
            .await
            .expect("draft should load")
            .expect("draft should exist");
        assert_eq!(draft.status, StaticPageDraftStatus::Confirmed);
        assert!(draft.draft_payload.get("finalPage").is_none());

        let stored_execution = storage
            .workflow_executions()
            .get_by_id(tenant_id, execution.id)
            .await
            .expect("workflow execution should load")
            .expect("workflow execution should exist");
        assert_eq!(stored_execution.status, WorkflowStatus::Cancelled);
        let tasks = storage
            .workflow_tasks()
            .list_by_execution(execution.id)
            .await
            .expect("workflow tasks should load");
        assert_eq!(tasks[0].status, WorkflowTaskStatus::Succeeded);
    }
}
