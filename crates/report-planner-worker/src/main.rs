use anyhow::{anyhow, Result};
use chrono::Utc;
use event_bus::{workflow_task_enqueued_subject, EventBus, EventSubscription};
use serde_json::{json, Value};
use storage::{NewReportPlanAstVersion, PgStorage, DEFAULT_LOCAL_DATABASE_URL};
use tokio::time::Duration;
use workflow_engine::{WorkflowCatalog, WorkflowSignal};

const DEFAULT_QUEUE: &str = "report";
const DEFAULT_TASK_KEY: &str = "plan_report_ast";
const DEFAULT_POLL_INTERVAL_MS: u64 = 1_000;

#[tokio::main]
async fn main() -> Result<()> {
    observability::install("report_planner_worker")?;

    let database_url = std::env::var("PLATFORM_DATABASE_URL")
        .unwrap_or_else(|_| DEFAULT_LOCAL_DATABASE_URL.to_string());
    let queue = std::env::var("REPORT_PLANNER_QUEUE").unwrap_or_else(|_| DEFAULT_QUEUE.to_string());
    let task_key =
        std::env::var("REPORT_PLANNER_TASK_KEY").unwrap_or_else(|_| DEFAULT_TASK_KEY.to_string());
    let poll_interval = std::env::var("REPORT_PLANNER_POLL_INTERVAL_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_POLL_INTERVAL_MS);

    let storage = PgStorage::connect(&database_url).await?;
    let workflow_catalog = workflow_definitions::catalog();
    let event_bus = EventBus::connect_from_env_or_disabled("PLATFORM_NATS_URL").await;
    let wake_subject = workflow_task_enqueued_subject(&queue, &task_key);
    let mut task_waker = event_bus
        .subscribe_queue_or_disabled(
            &wake_subject,
            Some(&format!("report_planner_worker.{queue}.{task_key}")),
        )
        .await;

    tracing::info!(
        %queue,
        %task_key,
        %wake_subject,
        event_bus_enabled = event_bus.is_enabled(),
        poll_interval_ms = poll_interval,
        %database_url,
        "report-planner-worker polling started"
    );

    loop {
        match storage
            .workflow_tasks()
            .claim_next_available(&queue, Some(&task_key), Utc::now())
            .await
        {
            Ok(Some(task)) => {
                if let Err(error) =
                    process_task(&storage, &workflow_catalog, &event_bus, task).await
                {
                    tracing::error!(error = ?error, "report planner task processing failed");
                }
            }
            Ok(None) => {
                wait_for_next_task_signal(&mut task_waker, poll_interval).await;
            }
            Err(error) => {
                tracing::error!(error = ?error, "report planner failed to claim task");
                wait_for_next_task_signal(&mut task_waker, poll_interval).await;
            }
        }
    }
}

async fn process_task(
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
    let report_plan_id = execution
        .report_plan_id
        .ok_or_else(|| anyhow!("workflow execution {} has no report plan id", execution.id))?;
    let report_plan = storage
        .report_plans()
        .get_by_id(task.tenant_id, report_plan_id)
        .await?
        .ok_or_else(|| anyhow!("report plan {} not found", report_plan_id))?;

    let ast = build_placeholder_ast(&report_plan);

    let process_result: Result<()> = async {
        let ast_version = storage
            .report_plan_ast_versions()
            .create_next_version(
                task.tenant_id,
                report_plan.id,
                &NewReportPlanAstVersion {
                    ast: ast.clone(),
                    created_at: Utc::now(),
                },
            )
            .await?;
        let outcome = json!({
            "report_plan_id": report_plan.id,
            "planner": "report-planner-worker",
            "ast_version_id": ast_version.id,
            "ast_version_no": ast_version.version_no,
            "module_count": ast
                .get("modules")
                .and_then(Value::as_array)
                .map(|modules| modules.len())
                .unwrap_or(0),
        });

        storage
            .report_plans()
            .mark_planned(
                task.tenant_id,
                report_plan.id,
                ast_version.id,
                &ast,
                Utc::now(),
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
                output: Some(outcome),
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
                "report planner failed to send workflow step_failed signal"
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
        report_plan_id = %report_plan.id,
        "report planner task completed"
    );

    Ok(())
}

fn build_placeholder_ast(plan: &domain_model::ReportPlan) -> Value {
    json!({
        "schema_version": "0.1.0",
        "planner": "report-planner-worker",
        "plan_id": plan.id,
        "dataset_id": plan.dataset_id,
        "title": plan.title,
        "objective": plan.objective,
        "modules": [
            {
                "kind": "hero",
                "title": "Executive Summary",
                "binding_slot": "summary.hero",
                "notes": "Topline summary generated from dataset context and planner skeleton."
            },
            {
                "kind": "timeline",
                "title": "Evidence Timeline",
                "binding_slot": "summary.timeline",
                "notes": "Key evidence and milestones to be expanded by report runtime."
            },
            {
                "kind": "evidence_list",
                "title": "Supporting Evidence",
                "binding_slot": "summary.evidence",
                "notes": "Primary supporting evidence placeholders for future retrieval binding."
            }
        ]
    })
}

async fn wait_for_next_task_signal(task_waker: &mut EventSubscription, poll_interval_ms: u64) {
    if let Some(event) = task_waker
        .wait_for_event(Duration::from_millis(poll_interval_ms))
        .await
    {
        tracing::debug!(subject = %event.subject, "report planner received task wake signal");
    }
}
