use anyhow::{anyhow, Result};
use chrono::Utc;
use domain_model::{
    AssistantRunId, StaticPageDraft, StaticPageDraftStatus, StaticPageImageJob,
    StaticPageImageJobStatus, StaticPageRenderOutput, StaticPageRenderOutputId,
    StaticPageRenderOutputStatus, TenantId, WorkflowKind,
};
use event_bus::{workflow_task_enqueued_subject, EventBus, EventSubscription};
use reqwest::blocking::Client;
use serde_json::{json, Value};
use static_page_renderer::{render_static_page, StaticPageRenderRequest};
use static_page_worker::{
    context_uuid, extract_first_image_artifact, merge_orchestrator_state,
    normalize_artifact_asset_key, poll_static_page_visual_task, submit_static_page_visual_task,
    task_failure_message, CodexOrchestratorConfig,
};
use storage::{NewAssistantRunEvent, PgStorage, DEFAULT_LOCAL_DATABASE_URL};
use tokio::time::Duration;
use workflow_engine::{WorkflowCatalog, WorkflowSignal};

const DEFAULT_QUEUE: &str = "static_page";
const DEFAULT_IMAGE_TASK_KEY: &str = "generate_static_page_image";
const DEFAULT_POLL_INTERVAL_MS: u64 = 1_000;
const DEFAULT_ORCHESTRATOR_POLL_INTERVAL_MS: u64 = 5_000;
const DEFAULT_ORCHESTRATOR_MAX_POLLS: u32 = 120;

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
    let orchestrator_config = CodexOrchestratorConfig::from_env()?;
    let http_client = Client::builder().build()?;

    tracing::info!(
        %queue,
        task_key = task_key.as_deref().unwrap_or("*"),
        %wake_subject,
        event_bus_enabled = event_bus.is_enabled(),
        poll_interval_ms = poll_interval,
        orchestrator_poll_interval_ms = orchestrator_poll_interval,
        orchestrator_max_polls,
        orchestrator_base_url = %orchestrator_config.base_url,
        orchestrator_runtime_target = %orchestrator_config.runtime_target_id,
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
                    &orchestrator_config,
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
    orchestrator_config: &CodexOrchestratorConfig,
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
        if let Ok(mut job) = storage
            .static_page_image_jobs()
            .get_by_id(task.tenant_id, job_id)
            .await
        {
            if let Some(mut job) = job.take() {
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
                    }),
                )
                .await;
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
    let process_result: Result<()> = async {
        let mut output = storage
            .static_page_render_outputs()
            .get_by_id(task.tenant_id, output_id)
            .await?
            .ok_or_else(|| anyhow!("static page render output {output_id} not found"))?;
        mark_render_output_rendering(storage, &mut output, execution).await?;

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

        output.status = StaticPageRenderOutputStatus::Rendered;
        output.html = rendered.html;
        output.asset_manifest = merge_render_state(
            &rendered.asset_manifest,
            json!({
                "status": "rendered",
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

        Ok(())
    }
    .await;

    if let Err(error) = process_result {
        let error_message = error.to_string();
        if let Ok(Some(mut output)) = storage
            .static_page_render_outputs()
            .get_by_id(task.tenant_id, output_id)
            .await
        {
            let _ =
                mark_render_output_failed(storage, &mut output, execution, &error_message).await;
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

    storage
        .workflow_tasks()
        .mark_succeeded(task.id, Utc::now())
        .await?;

    tracing::info!(
        task_id = %task.id,
        execution_id = %task.execution_id,
        static_page_render_output_id = %output_id,
        "static page render task completed"
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
        let task = poll_static_page_visual_task(http_client, orchestrator_config, task_id)?;
        match task.status.as_str() {
            "completed" => return Ok(task),
            "failed" | "cancelled" => return Err(anyhow!(task_failure_message(&task))),
            "queued" | "submitted" | "running" => {
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

async fn mark_render_output_rendering(
    storage: &PgStorage,
    output: &mut StaticPageRenderOutput,
    execution: &domain_model::WorkflowExecution,
) -> Result<()> {
    output.status = StaticPageRenderOutputStatus::Rendering;
    output.asset_manifest = merge_render_state(
        &output.asset_manifest,
        json!({
            "status": "rendering",
            "workflowExecutionId": execution.id,
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
    error_message: &str,
) -> Result<()> {
    output.status = StaticPageRenderOutputStatus::Failed;
    output.asset_manifest = merge_render_state(
        &output.asset_manifest,
        json!({
            "status": "failed",
            "workflowExecutionId": execution.id,
            "error": error_message,
            "updatedAt": Utc::now(),
        }),
    );
    *output = storage
        .static_page_render_outputs()
        .update(output.tenant_id, output)
        .await?;
    Ok(())
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
    object.insert("workflow".to_string(), state);
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
            "queueMessage": "效果图已生成，等待客户确认。",
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
        "renderExpectation": "final HTML/CSS/SVG should reproduce the confirmed preview without baking editable text or charts into the image",
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
        "renderExpectation": "final HTML/CSS/SVG should reproduce the confirmed preview without baking editable text or charts into the image",
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
