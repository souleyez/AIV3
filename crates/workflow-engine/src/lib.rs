use chrono::{DateTime, Utc};
use domain_model::{WorkflowExecutionId, WorkflowKind, WorkflowStatus};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::sync::Arc;
use thiserror::Error;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum WorkflowSignalKind {
    Start,
    StepCompleted,
    StepFailed,
    RetryRequested,
    CancelRequested,
    PublishRequested,
}

impl WorkflowSignalKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::StepCompleted => "step_completed",
            Self::StepFailed => "step_failed",
            Self::RetryRequested => "retry_requested",
            Self::CancelRequested => "cancel_requested",
            Self::PublishRequested => "publish_requested",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum WorkflowSignal {
    Start,
    StepCompleted {
        task_key: String,
        output: Option<Value>,
    },
    StepFailed {
        task_key: String,
        error: String,
    },
    RetryRequested {
        reason: String,
    },
    CancelRequested {
        reason: String,
    },
    PublishRequested {
        note: Option<String>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowRuntimeState {
    pub execution_id: WorkflowExecutionId,
    pub kind: WorkflowKind,
    pub version: String,
    pub stage: String,
    pub status: WorkflowStatus,
    pub retries_remaining: u32,
    pub context: Map<String, Value>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowEvent {
    pub name: String,
    pub detail: Value,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowTaskRequest {
    pub queue: String,
    pub task_key: String,
    pub payload: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowTransition {
    pub next_state: WorkflowRuntimeState,
    pub persisted_event: WorkflowEvent,
    pub enqueued_tasks: Vec<WorkflowTaskRequest>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowDefinitionSummary {
    pub kind: WorkflowKind,
    pub version: String,
    pub summary: String,
    pub accepted_signals: Vec<WorkflowSignalKind>,
}

#[derive(Debug, Error)]
pub enum WorkflowTransitionError {
    #[error("unsupported signal for current workflow stage")]
    UnsupportedSignal,
    #[error("invalid transition: {0}")]
    InvalidTransition(String),
}

pub trait WorkflowDefinition: Send + Sync {
    fn kind(&self) -> WorkflowKind;
    fn version(&self) -> &'static str;
    fn summary(&self) -> &'static str;
    fn accepted_signals(&self) -> &'static [WorkflowSignalKind];
    fn initial_state(
        &self,
        execution_id: WorkflowExecutionId,
        now: DateTime<Utc>,
    ) -> WorkflowRuntimeState;
    fn transition(
        &self,
        state: &WorkflowRuntimeState,
        signal: WorkflowSignal,
        now: DateTime<Utc>,
    ) -> Result<WorkflowTransition, WorkflowTransitionError>;

    fn descriptor(&self) -> WorkflowDefinitionSummary {
        WorkflowDefinitionSummary {
            kind: self.kind(),
            version: self.version().to_string(),
            summary: self.summary().to_string(),
            accepted_signals: self.accepted_signals().to_vec(),
        }
    }
}

pub type DynWorkflowDefinition = Arc<dyn WorkflowDefinition>;

#[derive(Clone, Default)]
pub struct WorkflowCatalog {
    definitions: Vec<DynWorkflowDefinition>,
}

impl WorkflowCatalog {
    pub fn new(definitions: Vec<DynWorkflowDefinition>) -> Self {
        Self { definitions }
    }

    pub fn definitions(&self) -> &[DynWorkflowDefinition] {
        &self.definitions
    }

    pub fn descriptors(&self) -> Vec<WorkflowDefinitionSummary> {
        self.definitions
            .iter()
            .map(|definition| definition.descriptor())
            .collect()
    }

    pub fn find_definition(&self, kind: WorkflowKind) -> Option<DynWorkflowDefinition> {
        self.definitions
            .iter()
            .find(|definition| definition.kind() == kind)
            .cloned()
    }
}
