use chrono::{DateTime, Utc};
use domain_model::{WorkflowExecutionId, WorkflowKind, WorkflowStatus};
use serde_json::{json, Map, Value};
use std::sync::Arc;
use workflow_engine::{
    DynWorkflowDefinition, WorkflowCatalog, WorkflowDefinition, WorkflowRuntimeState,
    WorkflowSignal, WorkflowSignalKind, WorkflowTaskRequest, WorkflowTransition,
    WorkflowTransitionError,
};

const STANDARD_SIGNALS: &[WorkflowSignalKind] = &[
    WorkflowSignalKind::Start,
    WorkflowSignalKind::StepCompleted,
    WorkflowSignalKind::StepFailed,
    WorkflowSignalKind::RetryRequested,
    WorkflowSignalKind::CancelRequested,
];

#[derive(Clone)]
struct WorkflowStepSpec {
    queue: &'static str,
    task_key: &'static str,
}

#[derive(Clone)]
struct LinearWorkflowDefinition {
    kind: WorkflowKind,
    summary: &'static str,
    queue: &'static str,
    task_key: &'static str,
    success_stage: &'static str,
    next_step: Option<WorkflowStepSpec>,
}

impl LinearWorkflowDefinition {
    fn pending_state(
        &self,
        execution_id: WorkflowExecutionId,
        now: DateTime<Utc>,
    ) -> WorkflowRuntimeState {
        let mut context = Map::new();
        context.insert("queue".to_string(), Value::String(self.queue.to_string()));
        context.insert(
            "task_key".to_string(),
            Value::String(self.task_key.to_string()),
        );

        WorkflowRuntimeState {
            execution_id,
            kind: self.kind.clone(),
            version: self.version().to_string(),
            stage: "queued".to_string(),
            status: WorkflowStatus::Pending,
            retries_remaining: 3,
            context,
            updated_at: now,
        }
    }
}

impl WorkflowDefinition for LinearWorkflowDefinition {
    fn kind(&self) -> WorkflowKind {
        self.kind.clone()
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn summary(&self) -> &'static str {
        self.summary
    }

    fn accepted_signals(&self) -> &'static [WorkflowSignalKind] {
        STANDARD_SIGNALS
    }

    fn initial_state(
        &self,
        execution_id: WorkflowExecutionId,
        now: DateTime<Utc>,
    ) -> WorkflowRuntimeState {
        self.pending_state(execution_id, now)
    }

    fn transition(
        &self,
        state: &WorkflowRuntimeState,
        signal: WorkflowSignal,
        now: DateTime<Utc>,
    ) -> Result<WorkflowTransition, WorkflowTransitionError> {
        match signal {
            WorkflowSignal::Start if state.status == WorkflowStatus::Pending => {
                let mut next_state = state.clone();
                next_state.stage = self.task_key.to_string();
                next_state.status = WorkflowStatus::Running;
                next_state.updated_at = now;

                Ok(WorkflowTransition {
                    next_state,
                    persisted_event: workflow_event(
                        "workflow.started",
                        json!({ "task_key": self.task_key, "queue": self.queue }),
                        now,
                    ),
                    enqueued_tasks: vec![WorkflowTaskRequest {
                        queue: self.queue.to_string(),
                        task_key: self.task_key.to_string(),
                        payload: json!({ "execution_id": state.execution_id, "kind": self.kind.as_str() }),
                    }],
                })
            }
            WorkflowSignal::StepCompleted { task_key, output }
                if state.status == WorkflowStatus::Running =>
            {
                if state.stage == self.task_key && task_key == self.task_key {
                    let mut next_state = state.clone();
                    let mut enqueued_tasks = Vec::new();

                    if let Some(next_step) = &self.next_step {
                        next_state.stage = next_step.task_key.to_string();
                        next_state.status = WorkflowStatus::Running;
                        enqueued_tasks.push(WorkflowTaskRequest {
                            queue: next_step.queue.to_string(),
                            task_key: next_step.task_key.to_string(),
                            payload: json!({
                                "execution_id": state.execution_id,
                                "kind": self.kind.as_str(),
                            }),
                        });
                    } else {
                        next_state.stage = self.success_stage.to_string();
                        next_state.status = WorkflowStatus::Succeeded;
                    }
                    next_state.updated_at = now;

                    if let Some(output) = output {
                        next_state.context.insert("last_output".to_string(), output);
                    }

                    Ok(WorkflowTransition {
                        next_state,
                        persisted_event: workflow_event(
                            "workflow.step_completed",
                            json!({ "task_key": task_key }),
                            now,
                        ),
                        enqueued_tasks,
                    })
                } else if self
                    .next_step
                    .as_ref()
                    .map(|step| state.stage == step.task_key && task_key == step.task_key)
                    .unwrap_or(false)
                {
                    let mut next_state = state.clone();
                    next_state.stage = self.success_stage.to_string();
                    next_state.status = WorkflowStatus::Succeeded;
                    next_state.updated_at = now;

                    if let Some(output) = output {
                        next_state.context.insert("last_output".to_string(), output);
                    }

                    Ok(WorkflowTransition {
                        next_state,
                        persisted_event: workflow_event(
                            "workflow.step_completed",
                            json!({ "task_key": task_key }),
                            now,
                        ),
                        enqueued_tasks: Vec::new(),
                    })
                } else {
                    Err(WorkflowTransitionError::InvalidTransition(format!(
                        "workflow={} stage={} task_key={task_key}",
                        self.kind.as_str(),
                        state.stage
                    )))
                }
            }
            WorkflowSignal::StepFailed { task_key, error }
                if state.status == WorkflowStatus::Running =>
            {
                let mut next_state = state.clone();
                next_state.stage = format!("{task_key}:failed");
                next_state.status = WorkflowStatus::Failed;
                next_state.updated_at = now;
                next_state
                    .context
                    .insert("last_error".to_string(), Value::String(error.clone()));

                Ok(WorkflowTransition {
                    next_state,
                    persisted_event: workflow_event(
                        "workflow.step_failed",
                        json!({ "task_key": task_key, "error": error }),
                        now,
                    ),
                    enqueued_tasks: Vec::new(),
                })
            }
            WorkflowSignal::RetryRequested { reason } if state.status == WorkflowStatus::Failed => {
                if state.retries_remaining == 0 {
                    let mut next_state = state.clone();
                    next_state.stage = "dead_lettered".to_string();
                    next_state.status = WorkflowStatus::DeadLettered;
                    next_state.updated_at = now;

                    return Ok(WorkflowTransition {
                        next_state,
                        persisted_event: workflow_event(
                            "workflow.dead_lettered",
                            json!({ "reason": reason }),
                            now,
                        ),
                        enqueued_tasks: Vec::new(),
                    });
                }

                let mut next_state = state.clone();
                next_state.stage = "queued".to_string();
                next_state.status = WorkflowStatus::Pending;
                next_state.retries_remaining -= 1;
                next_state.updated_at = now;
                next_state
                    .context
                    .insert("retry_reason".to_string(), Value::String(reason.clone()));

                Ok(WorkflowTransition {
                    next_state,
                    persisted_event: workflow_event(
                        "workflow.retry_requested",
                        json!({ "reason": reason }),
                        now,
                    ),
                    enqueued_tasks: Vec::new(),
                })
            }
            WorkflowSignal::CancelRequested { reason } => {
                let mut next_state = state.clone();
                next_state.stage = "cancelled".to_string();
                next_state.status = WorkflowStatus::Cancelled;
                next_state.updated_at = now;
                next_state
                    .context
                    .insert("cancel_reason".to_string(), Value::String(reason.clone()));

                Ok(WorkflowTransition {
                    next_state,
                    persisted_event: workflow_event(
                        "workflow.cancel_requested",
                        json!({ "reason": reason }),
                        now,
                    ),
                    enqueued_tasks: Vec::new(),
                })
            }
            WorkflowSignal::PublishRequested { .. } => {
                Err(WorkflowTransitionError::UnsupportedSignal)
            }
            _ => Err(WorkflowTransitionError::InvalidTransition(format!(
                "workflow={} stage={} status={:?}",
                self.kind.as_str(),
                state.stage,
                state.status
            ))),
        }
    }
}

pub fn registry() -> Vec<DynWorkflowDefinition> {
    vec![
        Arc::new(LinearWorkflowDefinition {
            kind: WorkflowKind::ChatSession,
            summary:
                "Drive multi-turn chat session orchestration with tool traces and scoped recall.",
            queue: "chat_session",
            task_key: "orchestrate_chat_session",
            success_stage: "chat_session_completed",
            next_step: None,
        }),
        Arc::new(LinearWorkflowDefinition {
            kind: WorkflowKind::DatasetOutput,
            summary: "Produce dataset-scoped output artifacts and persistent conversation answers.",
            queue: "dataset_output",
            task_key: "produce_dataset_output",
            success_stage: "dataset_output_completed",
            next_step: None,
        }),
        Arc::new(LinearWorkflowDefinition {
            kind: WorkflowKind::MemoryDirectory,
            summary: "Refresh long-term memory directories and evidence indices.",
            queue: "memory",
            task_key: "refresh_memory_directory",
            success_stage: "memory_directory_completed",
            next_step: None,
        }),
        Arc::new(LinearWorkflowDefinition {
            kind: WorkflowKind::UploadIngest,
            summary:
                "Receive documents, extract structure, chunk content, and attach scope bindings.",
            queue: "ingest",
            task_key: "ingest_uploaded_document",
            success_stage: "upload_ingest_completed",
            next_step: Some(WorkflowStepSpec {
                queue: "retrieval",
                task_key: "index_retrieval_artifacts",
            }),
        }),
        Arc::new(LinearWorkflowDefinition {
            kind: WorkflowKind::ReportPlan,
            summary:
                "Produce report plan AST and editable module graph drafts from dataset context.",
            queue: "report",
            task_key: "plan_report_ast",
            success_stage: "report_plan_completed",
            next_step: None,
        }),
        Arc::new(LinearWorkflowDefinition {
            kind: WorkflowKind::ReportRender,
            summary: "Render report plans into publishable PC and mobile assets.",
            queue: "report",
            task_key: "render_report_plan",
            success_stage: "report_render_completed",
            next_step: None,
        }),
        Arc::new(LinearWorkflowDefinition {
            kind: WorkflowKind::StaticPageImageGeneration,
            summary: "Generate static-page visual preview images through the Codex orchestrator.",
            queue: "static_page",
            task_key: "generate_static_page_image",
            success_stage: "static_page_image_generated",
            next_step: None,
        }),
        Arc::new(LinearWorkflowDefinition {
            kind: WorkflowKind::StaticPageRender,
            summary: "Render confirmed static-page drafts into durable HTML and asset manifests.",
            queue: "static_page",
            task_key: "render_static_page",
            success_stage: "static_page_render_completed",
            next_step: None,
        }),
        Arc::new(LinearWorkflowDefinition {
            kind: WorkflowKind::CodexHostTask,
            summary: "Queue a V3-audited external Codex Host task with isolated task memory.",
            queue: "codex_host",
            task_key: "run_codex_host_task",
            success_stage: "codex_host_task_completed",
            next_step: None,
        }),
    ]
}

pub fn catalog() -> WorkflowCatalog {
    WorkflowCatalog::new(registry())
}

fn workflow_event(
    name: &str,
    detail: Value,
    occurred_at: DateTime<Utc>,
) -> workflow_engine::WorkflowEvent {
    workflow_engine::WorkflowEvent {
        name: name.to_string(),
        detail,
        occurred_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_contains_expected_workflows() {
        let entries = registry();
        let names: Vec<_> = entries.iter().map(|entry| entry.kind().as_str()).collect();

        assert_eq!(entries.len(), 9);
        assert!(names.contains(&"chat_session_workflow"));
        assert!(names.contains(&"report_render_workflow"));
        assert!(names.contains(&"static_page_image_generation_workflow"));
        assert!(names.contains(&"static_page_render_workflow"));
        assert!(names.contains(&"codex_host_task_workflow"));
    }

    #[test]
    fn codex_host_task_starts_on_dedicated_queue() {
        let definition = registry()
            .into_iter()
            .find(|entry| entry.kind() == WorkflowKind::CodexHostTask)
            .expect("codex host workflow exists");
        let now = Utc::now();
        let pending = definition.initial_state(WorkflowExecutionId::new(), now);
        let running = definition
            .transition(&pending, WorkflowSignal::Start, now)
            .expect("start");

        assert_eq!(
            definition.summary(),
            "Queue a V3-audited external Codex Host task with isolated task memory."
        );
        assert_eq!(running.next_state.status, WorkflowStatus::Running);
        assert_eq!(running.next_state.stage, "run_codex_host_task");
        assert_eq!(running.enqueued_tasks.len(), 1);
        assert_eq!(running.enqueued_tasks[0].queue, "codex_host");
        assert_eq!(running.enqueued_tasks[0].task_key, "run_codex_host_task");
    }

    #[test]
    fn upload_ingest_moves_from_pending_to_success() {
        let definition = registry()
            .into_iter()
            .find(|entry| entry.kind() == WorkflowKind::UploadIngest)
            .expect("upload ingest workflow exists");
        let now = Utc::now();
        let pending = definition.initial_state(WorkflowExecutionId::new(), now);
        let running = definition
            .transition(&pending, WorkflowSignal::Start, now)
            .expect("start");
        let done = definition
            .transition(
                &running.next_state,
                WorkflowSignal::StepCompleted {
                    task_key: "ingest_uploaded_document".to_string(),
                    output: None,
                },
                now,
            )
            .expect("first step completes");
        let finished = definition
            .transition(
                &done.next_state,
                WorkflowSignal::StepCompleted {
                    task_key: "index_retrieval_artifacts".to_string(),
                    output: None,
                },
                now,
            )
            .expect("retrieval step completes");

        assert_eq!(done.next_state.status, WorkflowStatus::Running);
        assert_eq!(done.next_state.stage, "index_retrieval_artifacts");
        assert_eq!(done.enqueued_tasks.len(), 1);
        assert_eq!(done.enqueued_tasks[0].queue, "retrieval");
        assert_eq!(done.enqueued_tasks[0].task_key, "index_retrieval_artifacts");
        assert_eq!(finished.next_state.status, WorkflowStatus::Succeeded);
        assert_eq!(finished.next_state.stage, "upload_ingest_completed");
    }
}
