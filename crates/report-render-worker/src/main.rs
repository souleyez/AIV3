use anyhow::{anyhow, Result};
use chrono::Utc;
use domain_model::{PublishedSurface, ReportRenderOutputStatus};
use event_bus::{workflow_task_enqueued_subject, EventBus, EventSubscription};
use report_runtime::{PlaceholderReportRuntime, ReportRenderRequest, ReportRuntime};
use serde_json::{json, Value};
use storage::{NewReportRenderOutput, PgStorage, DEFAULT_LOCAL_DATABASE_URL};
use tokio::time::Duration;
use workflow_engine::{WorkflowCatalog, WorkflowSignal};

const DEFAULT_QUEUE: &str = "report";
const DEFAULT_TASK_KEY: &str = "render_report_plan";
const DEFAULT_POLL_INTERVAL_MS: u64 = 1_000;

#[tokio::main]
async fn main() -> Result<()> {
    observability::install("report_render_worker")?;

    let database_url = std::env::var("PLATFORM_DATABASE_URL")
        .unwrap_or_else(|_| DEFAULT_LOCAL_DATABASE_URL.to_string());
    let queue = std::env::var("REPORT_RENDER_QUEUE").unwrap_or_else(|_| DEFAULT_QUEUE.to_string());
    let task_key =
        std::env::var("REPORT_RENDER_TASK_KEY").unwrap_or_else(|_| DEFAULT_TASK_KEY.to_string());
    let poll_interval = std::env::var("REPORT_RENDER_POLL_INTERVAL_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_POLL_INTERVAL_MS);

    let storage = PgStorage::connect(&database_url).await?;
    let workflow_catalog = workflow_definitions::catalog();
    let runtime = PlaceholderReportRuntime;
    let event_bus = EventBus::connect_from_env_or_disabled("PLATFORM_NATS_URL").await;
    let wake_subject = workflow_task_enqueued_subject(&queue, &task_key);
    let mut task_waker = event_bus
        .subscribe_queue_or_disabled(
            &wake_subject,
            Some(&format!("report_render_worker.{queue}.{task_key}")),
        )
        .await;

    tracing::info!(
        %queue,
        %task_key,
        %wake_subject,
        event_bus_enabled = event_bus.is_enabled(),
        poll_interval_ms = poll_interval,
        %database_url,
        "report-render-worker polling started"
    );

    loop {
        match storage
            .workflow_tasks()
            .claim_next_available(&queue, Some(&task_key), Utc::now())
            .await
        {
            Ok(Some(task)) => {
                if let Err(error) =
                    process_task(&storage, &workflow_catalog, &event_bus, &runtime, task).await
                {
                    tracing::error!(error = ?error, "report render task processing failed");
                }
            }
            Ok(None) => {
                wait_for_next_task_signal(&mut task_waker, poll_interval).await;
            }
            Err(error) => {
                tracing::error!(error = ?error, "report render failed to claim task");
                wait_for_next_task_signal(&mut task_waker, poll_interval).await;
            }
        }
    }
}

async fn process_task(
    storage: &PgStorage,
    workflow_catalog: &WorkflowCatalog,
    event_bus: &EventBus,
    runtime: &impl ReportRuntime,
    task: domain_model::WorkflowTask,
) -> Result<()> {
    let execution = storage
        .workflow_executions()
        .get_by_id(task.tenant_id, task.execution_id)
        .await?
        .ok_or_else(|| anyhow!("workflow execution {} not found", task.execution_id))?;
    let plan_id = execution
        .report_plan_id
        .ok_or_else(|| anyhow!("workflow execution {} has no report plan id", execution.id))?;
    let dataset_id = execution
        .dataset_id
        .ok_or_else(|| anyhow!("workflow execution {} has no dataset id", execution.id))?;
    let ast_version_id = context_uuid_string(&execution.context, "report_plan_ast_version_id")?
        .map(domain_model::ReportPlanAstVersionId)
        .ok_or_else(|| {
            anyhow!(
                "workflow execution {} missing report_plan_ast_version_id",
                execution.id
            )
        })?;
    let surface = context_string(&execution.context, "surface")?
        .and_then(|value| PublishedSurface::from_str(&value))
        .ok_or_else(|| anyhow!("workflow execution {} missing valid surface", execution.id))?;
    let theme_key = context_string(&execution.context, "theme_key")?
        .ok_or_else(|| anyhow!("workflow execution {} missing theme_key", execution.id))?;

    let plan = storage
        .report_plans()
        .get_by_id(task.tenant_id, plan_id)
        .await?
        .ok_or_else(|| anyhow!("report plan {} not found", plan_id))?;
    let ast_version = storage
        .report_plan_ast_versions()
        .get_by_id(task.tenant_id, ast_version_id)
        .await?
        .ok_or_else(|| anyhow!("report plan ast version {} not found", ast_version_id))?;

    let render_request = ReportRenderRequest {
        plan_id,
        dataset_id,
        ast_version_id,
        surface: surface.clone(),
        theme_key,
    };

    let process_result: Result<()> = async {
        let render_outcome = runtime.render(&render_request);
        let module_count = ast_version
            .ast
            .get("modules")
            .and_then(Value::as_array)
            .map(|modules| modules.len())
            .unwrap_or(0);

        let output = storage
            .report_render_outputs()
            .create(
                task.tenant_id,
                &NewReportRenderOutput {
                    execution_id: task.execution_id,
                    plan_id,
                    dataset_id,
                    ast_version_id,
                    surface: surface.clone(),
                    status: ReportRenderOutputStatus::Rendered,
                    asset_manifest: render_outcome.asset_manifest.clone(),
                    created_at: Utc::now(),
                },
            )
            .await?;

        let signal_output = json!({
            "report_plan_id": plan.id,
            "report_render_output_id": output.id,
            "surface": surface.as_str(),
            "asset_count": render_outcome.asset_count,
            "manifest_key": render_outcome.manifest_key,
            "ast_version_id": ast_version.id,
            "module_count": module_count,
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
                "report render failed to send workflow step_failed signal"
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
        report_plan_id = %plan.id,
        ast_version_id = %ast_version.id,
        surface = %surface.as_str(),
        "report render task completed"
    );

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

fn context_uuid_string(value: &Value, key: &str) -> Result<Option<uuid::Uuid>> {
    match context_string(value, key)? {
        Some(raw) => Ok(Some(uuid::Uuid::parse_str(&raw)?)),
        None => Ok(None),
    }
}

async fn wait_for_next_task_signal(task_waker: &mut EventSubscription, poll_interval_ms: u64) {
    if let Some(event) = task_waker
        .wait_for_event(Duration::from_millis(poll_interval_ms))
        .await
    {
        tracing::debug!(subject = %event.subject, "report render received task wake signal");
    }
}
