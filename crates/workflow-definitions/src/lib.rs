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

#[derive(Clone)]
struct ExternalSourceSyncStep {
    stage: &'static str,
    queue: &'static str,
    task_key: &'static str,
}

struct ExternalSourceSyncWorkflowDefinition;

const EXTERNAL_SOURCE_SYNC_STEPS: &[ExternalSourceSyncStep] = &[
    ExternalSourceSyncStep {
        stage: "sync_users",
        queue: "external_source",
        task_key: "sync_external_users",
    },
    ExternalSourceSyncStep {
        stage: "sync_acl",
        queue: "external_source",
        task_key: "sync_external_acl",
    },
    ExternalSourceSyncStep {
        stage: "sync_metadata",
        queue: "external_source",
        task_key: "sync_external_metadata",
    },
    ExternalSourceSyncStep {
        stage: "fetch_content",
        queue: "external_source",
        task_key: "fetch_external_content",
    },
    ExternalSourceSyncStep {
        stage: "ingest",
        queue: "ingest",
        task_key: "ingest_external_content",
    },
    ExternalSourceSyncStep {
        stage: "index",
        queue: "retrieval",
        task_key: "index_external_retrieval",
    },
];

impl ExternalSourceSyncWorkflowDefinition {
    fn pending_state(
        execution_id: WorkflowExecutionId,
        now: DateTime<Utc>,
    ) -> WorkflowRuntimeState {
        let first_step = Self::first_step();
        let mut context = Map::new();
        context.insert(
            "queue".to_string(),
            Value::String(first_step.queue.to_string()),
        );
        context.insert(
            "task_key".to_string(),
            Value::String(first_step.task_key.to_string()),
        );
        context.insert(
            "states".to_string(),
            json!([
                "sync_users",
                "sync_acl",
                "sync_metadata",
                "fetch_content",
                "ingest",
                "index",
                "completed",
                "failed"
            ]),
        );
        context.insert(
            "permission_boundary".to_string(),
            Value::String("attach_external_acl_snapshot_before_index".to_string()),
        );

        WorkflowRuntimeState {
            execution_id,
            kind: WorkflowKind::ExternalSourceSync,
            version: "0.1.0".to_string(),
            stage: "queued".to_string(),
            status: WorkflowStatus::Pending,
            retries_remaining: 3,
            context,
            updated_at: now,
        }
    }

    fn first_step() -> &'static ExternalSourceSyncStep {
        EXTERNAL_SOURCE_SYNC_STEPS
            .first()
            .expect("external source sync has at least one step")
    }

    fn stage_after_completed_task(
        current_stage: &str,
        task_key: &str,
    ) -> Result<ExternalSourceSyncStageOutcome, WorkflowTransitionError> {
        let Some((index, _step)) = EXTERNAL_SOURCE_SYNC_STEPS
            .iter()
            .enumerate()
            .find(|(_, step)| step.stage == current_stage && step.task_key == task_key)
        else {
            return Err(WorkflowTransitionError::InvalidTransition(format!(
                "workflow={} stage={} task_key={task_key}",
                WorkflowKind::ExternalSourceSync.as_str(),
                current_stage
            )));
        };

        if let Some(next_step) = EXTERNAL_SOURCE_SYNC_STEPS.get(index + 1) {
            Ok(ExternalSourceSyncStageOutcome::Next(next_step))
        } else {
            Ok(ExternalSourceSyncStageOutcome::Completed)
        }
    }

    fn task_request(
        state: &WorkflowRuntimeState,
        step: &ExternalSourceSyncStep,
    ) -> WorkflowTaskRequest {
        let mut payload = Map::new();
        payload.insert("execution_id".to_string(), json!(state.execution_id));
        payload.insert(
            "kind".to_string(),
            Value::String(WorkflowKind::ExternalSourceSync.as_str().to_string()),
        );
        payload.insert("stage".to_string(), Value::String(step.stage.to_string()));
        for key in [
            "source_id",
            "external_sync_run_id",
            "sync_kind",
            "dataset_id",
            "connector_kind",
            "sync_mode",
            "permission_mode",
        ] {
            if let Some(value) = state.context.get(key) {
                payload.insert(key.to_string(), value.clone());
            }
        }

        WorkflowTaskRequest {
            queue: step.queue.to_string(),
            task_key: step.task_key.to_string(),
            payload: Value::Object(payload),
        }
    }
}

enum ExternalSourceSyncStageOutcome {
    Next(&'static ExternalSourceSyncStep),
    Completed,
}

impl WorkflowDefinition for ExternalSourceSyncWorkflowDefinition {
    fn kind(&self) -> WorkflowKind {
        WorkflowKind::ExternalSourceSync
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn summary(&self) -> &'static str {
        "Sync third-party users, ACLs, metadata, content, ingest output, and retrieval indexes."
    }

    fn accepted_signals(&self) -> &'static [WorkflowSignalKind] {
        STANDARD_SIGNALS
    }

    fn initial_state(
        &self,
        execution_id: WorkflowExecutionId,
        now: DateTime<Utc>,
    ) -> WorkflowRuntimeState {
        Self::pending_state(execution_id, now)
    }

    fn transition(
        &self,
        state: &WorkflowRuntimeState,
        signal: WorkflowSignal,
        now: DateTime<Utc>,
    ) -> Result<WorkflowTransition, WorkflowTransitionError> {
        match signal {
            WorkflowSignal::Start if state.status == WorkflowStatus::Pending => {
                let first_step = Self::first_step();
                let mut next_state = state.clone();
                next_state.stage = first_step.stage.to_string();
                next_state.status = WorkflowStatus::Running;
                next_state.updated_at = now;

                Ok(WorkflowTransition {
                    next_state,
                    persisted_event: workflow_event(
                        "workflow.started",
                        json!({ "task_key": first_step.task_key, "queue": first_step.queue }),
                        now,
                    ),
                    enqueued_tasks: vec![Self::task_request(state, first_step)],
                })
            }
            WorkflowSignal::StepCompleted { task_key, output }
                if state.status == WorkflowStatus::Running =>
            {
                let outcome = Self::stage_after_completed_task(&state.stage, &task_key)?;
                let mut next_state = state.clone();
                next_state.updated_at = now;
                if let Some(output) = output {
                    next_state.context.insert("last_output".to_string(), output);
                }

                match outcome {
                    ExternalSourceSyncStageOutcome::Next(next_step) => {
                        next_state.stage = next_step.stage.to_string();
                        next_state.status = WorkflowStatus::Running;
                        Ok(WorkflowTransition {
                            next_state,
                            persisted_event: workflow_event(
                                "workflow.step_completed",
                                json!({ "task_key": task_key, "next_stage": next_step.stage }),
                                now,
                            ),
                            enqueued_tasks: vec![Self::task_request(state, next_step)],
                        })
                    }
                    ExternalSourceSyncStageOutcome::Completed => {
                        next_state.stage = "completed".to_string();
                        next_state.status = WorkflowStatus::Succeeded;
                        Ok(WorkflowTransition {
                            next_state,
                            persisted_event: workflow_event(
                                "workflow.completed",
                                json!({ "task_key": task_key }),
                                now,
                            ),
                            enqueued_tasks: Vec::new(),
                        })
                    }
                }
            }
            WorkflowSignal::StepFailed { task_key, error }
                if state.status == WorkflowStatus::Running =>
            {
                let mut next_state = state.clone();
                next_state.stage = "failed".to_string();
                next_state.status = WorkflowStatus::Failed;
                next_state.updated_at = now;
                next_state
                    .context
                    .insert("last_error".to_string(), Value::String(error.clone()));
                next_state.context.insert(
                    "failed_task_key".to_string(),
                    Value::String(task_key.clone()),
                );

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
                WorkflowKind::ExternalSourceSync.as_str(),
                state.stage,
                state.status
            ))),
        }
    }
}

struct VideoExtractionWorkflowDefinition;

impl VideoExtractionWorkflowDefinition {
    fn pending_state(
        execution_id: WorkflowExecutionId,
        now: DateTime<Utc>,
    ) -> WorkflowRuntimeState {
        let mut context = Map::new();
        context.insert("queue".to_string(), Value::String("media".to_string()));
        context.insert(
            "task_key".to_string(),
            Value::String("resolve_video_source".to_string()),
        );
        context.insert(
            "states".to_string(),
            json!([
                "resolving_source",
                "registered",
                "parsing",
                "extracting_ppt",
                "completed",
                "failed",
                "unsupported_source"
            ]),
        );

        WorkflowRuntimeState {
            execution_id,
            kind: WorkflowKind::VideoExtraction,
            version: "0.1.0".to_string(),
            stage: "queued".to_string(),
            status: WorkflowStatus::Pending,
            retries_remaining: 3,
            context,
            updated_at: now,
        }
    }

    fn next_task_for_stage(stage: &str) -> Option<WorkflowStepSpec> {
        match stage {
            "registered" => Some(WorkflowStepSpec {
                queue: "media",
                task_key: "register_video_asset",
            }),
            "parsing" => Some(WorkflowStepSpec {
                queue: "ingest",
                task_key: "parse_video_media",
            }),
            "extracting_ppt" => Some(WorkflowStepSpec {
                queue: "media",
                task_key: "extract_video_ppt",
            }),
            _ => None,
        }
    }

    fn stage_after_completed_task(
        current_stage: &str,
        task_key: &str,
        output: Option<&Value>,
    ) -> Result<VideoExtractionStageOutcome, WorkflowTransitionError> {
        if video_extraction_output_is_unsupported(output) {
            return Ok(VideoExtractionStageOutcome::UnsupportedSource);
        }

        match (current_stage, task_key) {
            ("resolving_source", "resolve_video_source") => {
                Ok(VideoExtractionStageOutcome::Next("registered"))
            }
            ("registered", "register_video_asset") => {
                Ok(VideoExtractionStageOutcome::Next("parsing"))
            }
            ("parsing", "parse_video_media") => {
                Ok(VideoExtractionStageOutcome::Next("extracting_ppt"))
            }
            ("extracting_ppt", "extract_video_ppt") => Ok(VideoExtractionStageOutcome::Completed),
            _ => Err(WorkflowTransitionError::InvalidTransition(format!(
                "workflow={} stage={} task_key={task_key}",
                WorkflowKind::VideoExtraction.as_str(),
                current_stage
            ))),
        }
    }
}

enum VideoExtractionStageOutcome {
    Next(&'static str),
    Completed,
    UnsupportedSource,
}

impl WorkflowDefinition for VideoExtractionWorkflowDefinition {
    fn kind(&self) -> WorkflowKind {
        WorkflowKind::VideoExtraction
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn summary(&self) -> &'static str {
        "Resolve video sources, parse media evidence, and extract PPT/transcript deliverables."
    }

    fn accepted_signals(&self) -> &'static [WorkflowSignalKind] {
        STANDARD_SIGNALS
    }

    fn initial_state(
        &self,
        execution_id: WorkflowExecutionId,
        now: DateTime<Utc>,
    ) -> WorkflowRuntimeState {
        Self::pending_state(execution_id, now)
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
                next_state.stage = "resolving_source".to_string();
                next_state.status = WorkflowStatus::Running;
                next_state.updated_at = now;

                Ok(WorkflowTransition {
                    next_state,
                    persisted_event: workflow_event(
                        "workflow.started",
                        json!({ "task_key": "resolve_video_source", "queue": "media" }),
                        now,
                    ),
                    enqueued_tasks: vec![WorkflowTaskRequest {
                        queue: "media".to_string(),
                        task_key: "resolve_video_source".to_string(),
                        payload: json!({
                            "execution_id": state.execution_id,
                            "kind": WorkflowKind::VideoExtraction.as_str(),
                            "stage": "resolving_source"
                        }),
                    }],
                })
            }
            WorkflowSignal::StepCompleted { task_key, output }
                if state.status == WorkflowStatus::Running =>
            {
                let outcome =
                    Self::stage_after_completed_task(&state.stage, &task_key, output.as_ref())?;
                let mut next_state = state.clone();
                next_state.updated_at = now;
                if let Some(output) = output {
                    next_state.context.insert("last_output".to_string(), output);
                }

                match outcome {
                    VideoExtractionStageOutcome::Next(next_stage) => {
                        next_state.stage = next_stage.to_string();
                        next_state.status = WorkflowStatus::Running;
                        let next_task = Self::next_task_for_stage(next_stage).ok_or_else(|| {
                            WorkflowTransitionError::InvalidTransition(format!(
                                "missing video extraction task for stage {next_stage}"
                            ))
                        })?;
                        Ok(WorkflowTransition {
                            next_state,
                            persisted_event: workflow_event(
                                "workflow.step_completed",
                                json!({ "task_key": task_key, "next_stage": next_stage }),
                                now,
                            ),
                            enqueued_tasks: vec![WorkflowTaskRequest {
                                queue: next_task.queue.to_string(),
                                task_key: next_task.task_key.to_string(),
                                payload: json!({
                                    "execution_id": state.execution_id,
                                    "kind": WorkflowKind::VideoExtraction.as_str(),
                                    "stage": next_stage
                                }),
                            }],
                        })
                    }
                    VideoExtractionStageOutcome::Completed => {
                        next_state.stage = "completed".to_string();
                        next_state.status = WorkflowStatus::Succeeded;
                        Ok(WorkflowTransition {
                            next_state,
                            persisted_event: workflow_event(
                                "workflow.completed",
                                json!({ "task_key": task_key }),
                                now,
                            ),
                            enqueued_tasks: Vec::new(),
                        })
                    }
                    VideoExtractionStageOutcome::UnsupportedSource => {
                        next_state.stage = "unsupported_source".to_string();
                        next_state.status = WorkflowStatus::Failed;
                        Ok(WorkflowTransition {
                            next_state,
                            persisted_event: workflow_event(
                                "workflow.unsupported_source",
                                json!({ "task_key": task_key }),
                                now,
                            ),
                            enqueued_tasks: Vec::new(),
                        })
                    }
                }
            }
            WorkflowSignal::StepFailed { task_key, error }
                if state.status == WorkflowStatus::Running =>
            {
                let mut next_state = state.clone();
                next_state.stage = if error == "unsupported_source" {
                    "unsupported_source".to_string()
                } else {
                    "failed".to_string()
                };
                next_state.status = WorkflowStatus::Failed;
                next_state.updated_at = now;
                next_state
                    .context
                    .insert("last_error".to_string(), Value::String(error.clone()));
                next_state.context.insert(
                    "failed_task_key".to_string(),
                    Value::String(task_key.clone()),
                );

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

                let mut next_state = Self::pending_state(state.execution_id, now);
                next_state.retries_remaining = state.retries_remaining - 1;
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
                WorkflowKind::VideoExtraction.as_str(),
                state.stage,
                state.status
            ))),
        }
    }
}

fn video_extraction_output_is_unsupported(output: Option<&Value>) -> bool {
    output.is_some_and(|value| {
        value
            .get("status")
            .and_then(Value::as_str)
            .is_some_and(|status| status == "unsupported_source")
            || value
                .get("reason")
                .and_then(Value::as_str)
                .is_some_and(|reason| reason == "unsupported_source")
            || value
                .get("unsupported_source")
                .and_then(Value::as_bool)
                .unwrap_or(false)
    })
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
        Arc::new(ExternalSourceSyncWorkflowDefinition),
        Arc::new(VideoExtractionWorkflowDefinition),
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

        assert_eq!(entries.len(), 11);
        assert!(names.contains(&"chat_session_workflow"));
        assert!(names.contains(&"report_render_workflow"));
        assert!(names.contains(&"static_page_image_generation_workflow"));
        assert!(names.contains(&"static_page_render_workflow"));
        assert!(names.contains(&"codex_host_task_workflow"));
        assert!(names.contains(&"external_source_sync_workflow"));
        assert!(names.contains(&"video_extraction_workflow"));
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

    #[test]
    fn external_source_sync_walks_connector_ingest_and_index_stages() {
        let definition = registry()
            .into_iter()
            .find(|entry| entry.kind() == WorkflowKind::ExternalSourceSync)
            .expect("external source sync workflow exists");
        let now = Utc::now();
        let mut pending = definition.initial_state(WorkflowExecutionId::new(), now);
        pending
            .context
            .insert("source_id".to_string(), json!("src-docs"));
        pending
            .context
            .insert("external_sync_run_id".to_string(), json!("sync-run-001"));
        pending
            .context
            .insert("sync_kind".to_string(), json!("full"));

        let sync_users = definition
            .transition(&pending, WorkflowSignal::Start, now)
            .expect("external source sync starts");
        assert_eq!(
            definition.summary(),
            "Sync third-party users, ACLs, metadata, content, ingest output, and retrieval indexes."
        );
        assert_eq!(sync_users.next_state.status, WorkflowStatus::Running);
        assert_eq!(sync_users.next_state.stage, "sync_users");
        assert_eq!(sync_users.enqueued_tasks[0].queue, "external_source");
        assert_eq!(sync_users.enqueued_tasks[0].task_key, "sync_external_users");
        assert_eq!(
            sync_users.enqueued_tasks[0].payload["source_id"],
            json!("src-docs")
        );

        let expected_steps = [
            (
                "sync_external_users",
                "sync_acl",
                "external_source",
                "sync_external_acl",
            ),
            (
                "sync_external_acl",
                "sync_metadata",
                "external_source",
                "sync_external_metadata",
            ),
            (
                "sync_external_metadata",
                "fetch_content",
                "external_source",
                "fetch_external_content",
            ),
            (
                "fetch_external_content",
                "ingest",
                "ingest",
                "ingest_external_content",
            ),
            (
                "ingest_external_content",
                "index",
                "retrieval",
                "index_external_retrieval",
            ),
        ];

        let mut state = sync_users.next_state;
        for (completed_task, next_stage, next_queue, next_task) in expected_steps {
            let transition = definition
                .transition(
                    &state,
                    WorkflowSignal::StepCompleted {
                        task_key: completed_task.to_string(),
                        output: Some(json!({ "count": 1 })),
                    },
                    now,
                )
                .expect("external sync step completes");
            assert_eq!(transition.next_state.status, WorkflowStatus::Running);
            assert_eq!(transition.next_state.stage, next_stage);
            assert_eq!(transition.enqueued_tasks[0].queue, next_queue);
            assert_eq!(transition.enqueued_tasks[0].task_key, next_task);
            assert_eq!(
                transition.enqueued_tasks[0].payload["external_sync_run_id"],
                json!("sync-run-001")
            );
            state = transition.next_state;
        }

        let completed = definition
            .transition(
                &state,
                WorkflowSignal::StepCompleted {
                    task_key: "index_external_retrieval".to_string(),
                    output: Some(json!({ "indexed_count": 3 })),
                },
                now,
            )
            .expect("external sync completes");
        assert_eq!(completed.next_state.status, WorkflowStatus::Succeeded);
        assert_eq!(completed.next_state.stage, "completed");
        assert!(completed.enqueued_tasks.is_empty());
    }

    #[test]
    fn video_extraction_workflow_walks_expected_stages() {
        let definition = registry()
            .into_iter()
            .find(|entry| entry.kind() == WorkflowKind::VideoExtraction)
            .expect("video extraction workflow exists");
        let now = Utc::now();
        let pending = definition.initial_state(WorkflowExecutionId::new(), now);
        let resolving = definition
            .transition(&pending, WorkflowSignal::Start, now)
            .expect("video extraction starts");

        assert_eq!(
            definition.summary(),
            "Resolve video sources, parse media evidence, and extract PPT/transcript deliverables."
        );
        assert_eq!(resolving.next_state.status, WorkflowStatus::Running);
        assert_eq!(resolving.next_state.stage, "resolving_source");
        assert_eq!(resolving.enqueued_tasks[0].queue, "media");
        assert_eq!(resolving.enqueued_tasks[0].task_key, "resolve_video_source");

        let registered = definition
            .transition(
                &resolving.next_state,
                WorkflowSignal::StepCompleted {
                    task_key: "resolve_video_source".to_string(),
                    output: Some(json!({ "source_type": "direct_video_url" })),
                },
                now,
            )
            .expect("source resolves");
        assert_eq!(registered.next_state.stage, "registered");
        assert_eq!(
            registered.enqueued_tasks[0].task_key,
            "register_video_asset"
        );

        let parsing = definition
            .transition(
                &registered.next_state,
                WorkflowSignal::StepCompleted {
                    task_key: "register_video_asset".to_string(),
                    output: Some(json!({ "asset_state": "registered" })),
                },
                now,
            )
            .expect("asset registers");
        assert_eq!(parsing.next_state.stage, "parsing");
        assert_eq!(parsing.enqueued_tasks[0].queue, "ingest");
        assert_eq!(parsing.enqueued_tasks[0].task_key, "parse_video_media");

        let extracting = definition
            .transition(
                &parsing.next_state,
                WorkflowSignal::StepCompleted {
                    task_key: "parse_video_media".to_string(),
                    output: Some(json!({ "parse_status": "transcribed" })),
                },
                now,
            )
            .expect("media parses");
        assert_eq!(extracting.next_state.stage, "extracting_ppt");
        assert_eq!(extracting.enqueued_tasks[0].task_key, "extract_video_ppt");

        let completed = definition
            .transition(
                &extracting.next_state,
                WorkflowSignal::StepCompleted {
                    task_key: "extract_video_ppt".to_string(),
                    output: Some(json!({ "artifact_count": 2 })),
                },
                now,
            )
            .expect("ppt extraction completes");
        assert_eq!(completed.next_state.status, WorkflowStatus::Succeeded);
        assert_eq!(completed.next_state.stage, "completed");
        assert!(completed.enqueued_tasks.is_empty());
    }

    #[test]
    fn video_extraction_workflow_has_unsupported_source_branch() {
        let definition = registry()
            .into_iter()
            .find(|entry| entry.kind() == WorkflowKind::VideoExtraction)
            .expect("video extraction workflow exists");
        let now = Utc::now();
        let pending = definition.initial_state(WorkflowExecutionId::new(), now);
        let resolving = definition
            .transition(&pending, WorkflowSignal::Start, now)
            .expect("video extraction starts");
        let unsupported = definition
            .transition(
                &resolving.next_state,
                WorkflowSignal::StepCompleted {
                    task_key: "resolve_video_source".to_string(),
                    output: Some(json!({ "status": "unsupported_source" })),
                },
                now,
            )
            .expect("unsupported source is explicit");

        assert_eq!(unsupported.next_state.status, WorkflowStatus::Failed);
        assert_eq!(unsupported.next_state.stage, "unsupported_source");
        assert_eq!(
            unsupported.persisted_event.name,
            "workflow.unsupported_source"
        );
        assert!(unsupported.enqueued_tasks.is_empty());
    }

    #[test]
    fn static_page_render_retry_and_dead_letter_paths_are_explicit() {
        let definition = registry()
            .into_iter()
            .find(|entry| entry.kind() == WorkflowKind::StaticPageRender)
            .expect("static page render workflow exists");
        let now = Utc::now();
        let pending = definition.initial_state(WorkflowExecutionId::new(), now);
        let running = definition
            .transition(&pending, WorkflowSignal::Start, now)
            .expect("static page render starts");
        let failed = definition
            .transition(
                &running.next_state,
                WorkflowSignal::StepFailed {
                    task_key: "render_static_page".to_string(),
                    error: "renderer failed".to_string(),
                },
                now,
            )
            .expect("render step can fail");

        assert_eq!(running.enqueued_tasks[0].queue, "static_page");
        assert_eq!(running.enqueued_tasks[0].task_key, "render_static_page");
        assert_eq!(failed.next_state.status, WorkflowStatus::Failed);
        assert_eq!(failed.next_state.stage, "render_static_page:failed");
        assert_eq!(
            failed.next_state.context["last_error"],
            json!("renderer failed")
        );

        let retry = definition
            .transition(
                &failed.next_state,
                WorkflowSignal::RetryRequested {
                    reason: "user requested retry".to_string(),
                },
                now,
            )
            .expect("failed render can be retried");
        assert_eq!(retry.next_state.status, WorkflowStatus::Pending);
        assert_eq!(retry.next_state.stage, "queued");
        assert_eq!(retry.next_state.retries_remaining, 2);
        assert_eq!(
            retry.next_state.context["retry_reason"],
            json!("user requested retry")
        );
        assert!(retry.enqueued_tasks.is_empty());

        let restarted = definition
            .transition(&retry.next_state, WorkflowSignal::Start, now)
            .expect("retried render can restart");
        assert_eq!(restarted.next_state.status, WorkflowStatus::Running);
        assert_eq!(restarted.enqueued_tasks[0].queue, "static_page");
        assert_eq!(restarted.enqueued_tasks[0].task_key, "render_static_page");

        let mut exhausted = failed.next_state;
        exhausted.retries_remaining = 0;
        let dead_lettered = definition
            .transition(
                &exhausted,
                WorkflowSignal::RetryRequested {
                    reason: "no retries left".to_string(),
                },
                now,
            )
            .expect("exhausted render retry moves to dead letter");
        assert_eq!(
            dead_lettered.next_state.status,
            WorkflowStatus::DeadLettered
        );
        assert_eq!(dead_lettered.next_state.stage, "dead_lettered");
    }
}
