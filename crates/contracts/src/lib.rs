use chrono::{DateTime, Utc};
use domain_model::{
    ChatMessageId, ChatMessageRole, ChatSessionId, DatasetId, DatasetLifecycle, DatasetOutputId,
    DocumentChunkId, DocumentId, LlmInvocationId, MemoryDirectoryId, PublishedReportId,
    PublishedReportVersionId, PublishedSurface, ReportPlanAstVersionId, ReportPlanId,
    ReportRenderOutputId, RetrievalEvidenceId, SecretBindingId, ToolExecutionId, WorkflowEventId,
    WorkflowExecutionId, WorkflowKind, WorkflowStatus, WorkflowTaskId, WorkflowTaskStatus,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub service: String,
    pub status: String,
    pub version: String,
    pub checked_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiErrorResponse {
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowDefinitionView {
    pub kind: WorkflowKind,
    pub version: String,
    pub summary: String,
    pub accepted_signals: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolInvocationModeView {
    Cli,
    Internal,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolCliOutputModeView {
    Json,
    Text,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolCliContractView {
    pub argv: Vec<String>,
    pub env_allowlist: Vec<String>,
    pub output_mode: ToolCliOutputModeView,
    pub timeout_ms: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolDefinitionView {
    pub key: String,
    pub title: String,
    pub scope_policy: String,
    pub input_schema: Value,
    pub output_schema: Value,
    pub invocation_mode: ToolInvocationModeView,
    pub cli: Option<ToolCliContractView>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolReferenceView {
    pub key: String,
    pub title: String,
    pub scope_policy: String,
    pub invocation_mode: ToolInvocationModeView,
    pub cli: Option<ToolCliContractView>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowExecutionView {
    pub id: WorkflowExecutionId,
    pub kind: WorkflowKind,
    pub status: WorkflowStatus,
    pub stage: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowEventView {
    pub id: WorkflowEventId,
    pub sequence_no: i64,
    pub event_name: String,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowTaskView {
    pub id: WorkflowTaskId,
    pub status: WorkflowTaskStatus,
    pub queue: String,
    pub task_key: String,
    pub attempt: u32,
    pub available_at: DateTime<Utc>,
    pub payload: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolExecutionSourceKindView {
    DatasetOutput,
    ChatMessage,
    WorkflowExecution,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolExecutionView {
    pub id: ToolExecutionId,
    pub execution_id: WorkflowExecutionId,
    pub source_kind: ToolExecutionSourceKindView,
    pub dataset_output_id: Option<DatasetOutputId>,
    pub chat_message_id: Option<ChatMessageId>,
    pub sequence_no: i32,
    pub call_id: Option<String>,
    pub tool_name: String,
    pub tool: Option<ToolReferenceView>,
    pub status: ManifestToolCallStatusView,
    pub arguments: Option<Value>,
    pub result: Option<Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdvanceWorkflowExecutionResponse {
    pub execution: WorkflowExecutionView,
    pub persisted_event: WorkflowEventView,
    pub enqueued_tasks: Vec<WorkflowTaskView>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RetryWorkflowExecutionRequest {
    pub reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RetryWorkflowExecutionResponse {
    pub retry_transition: AdvanceWorkflowExecutionResponse,
    pub restart_transition: Option<AdvanceWorkflowExecutionResponse>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowSignalRequest {
    pub kind: WorkflowSignalKindView,
    pub task_key: Option<String>,
    pub output: Option<Value>,
    pub error: Option<String>,
    pub reason: Option<String>,
    pub note: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkflowSignalKindView {
    Start,
    StepCompleted,
    StepFailed,
    RetryRequested,
    CancelRequested,
    PublishRequested,
    Other(String),
}

impl WorkflowSignalKindView {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Start => "start",
            Self::StepCompleted => "step_completed",
            Self::StepFailed => "step_failed",
            Self::RetryRequested => "retry_requested",
            Self::CancelRequested => "cancel_requested",
            Self::PublishRequested => "publish_requested",
            Self::Other(value) => value.as_str(),
        }
    }
}

impl Serialize for WorkflowSignalKindView {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for WorkflowSignalKindView {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "start" => Self::Start,
            "step_completed" => Self::StepCompleted,
            "step_failed" => Self::StepFailed,
            "retry_requested" => Self::RetryRequested,
            "cancel_requested" => Self::CancelRequested,
            "publish_requested" => Self::PublishRequested,
            _ => Self::Other(value),
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateDatasetRequest {
    pub key: String,
    pub title: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DatasetSummary {
    pub id: DatasetId,
    pub key: String,
    pub title: String,
    pub lifecycle: DatasetLifecycle,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentSummary {
    pub id: DocumentId,
    pub dataset_id: DatasetId,
    pub title: String,
    pub object_key: String,
    pub content_type: String,
    pub lifecycle: DocumentLifecycleView,
    pub secret_binding_ids: Vec<SecretBindingId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentChunkView {
    pub id: DocumentChunkId,
    pub document_id: DocumentId,
    pub chunk_index: i32,
    pub token_count: i32,
    pub state: DocumentChunkStateView,
    pub content: String,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DocumentChunkStateView {
    Extracted,
    Indexed,
}

impl DocumentChunkStateView {
    pub fn from_domain(value: domain_model::DocumentChunkState) -> Self {
        match value {
            domain_model::DocumentChunkState::Extracted => Self::Extracted,
            domain_model::DocumentChunkState::Indexed => Self::Indexed,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalEmbeddingStatusView {
    Pending,
    Indexed,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalRecallStatusView {
    Pending,
    Ready,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetrievalEmbeddingManifestView {
    pub status: RetrievalEmbeddingStatusView,
    pub model: String,
    pub token_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RetrievalRecallManifestView {
    pub status: RetrievalRecallStatusView,
    pub score: f64,
    pub rank_hint: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetrievalEvidenceLocatorManifestView {
    pub document_chunk_id: DocumentChunkId,
    pub payload_filter_key: String,
    pub source_locator: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RetrievalEvidenceManifestView {
    pub schema_version: String,
    pub generator: String,
    pub dataset_id: DatasetId,
    pub document_id: DocumentId,
    pub document_chunk_id: DocumentChunkId,
    pub chunk_index: i32,
    pub indexed_at: DateTime<Utc>,
    pub embedding: RetrievalEmbeddingManifestView,
    pub recall: RetrievalRecallManifestView,
    pub evidence: RetrievalEvidenceLocatorManifestView,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RetrievalEvidenceView {
    pub id: RetrievalEvidenceId,
    pub dataset_id: DatasetId,
    pub document_id: DocumentId,
    pub document_chunk_id: DocumentChunkId,
    pub execution_id: WorkflowExecutionId,
    pub chunk_index: i32,
    pub source_locator: String,
    pub content_excerpt: String,
    pub summary: String,
    pub payload_filter_key: String,
    pub embedding_model: String,
    pub recall_score: f64,
    pub evidence_manifest: Value,
    pub evidence_manifest_view: Option<RetrievalEvidenceManifestView>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentDetailView {
    pub document: DocumentSummary,
    pub chunks: Vec<DocumentChunkView>,
    pub retrieval_evidences: Vec<RetrievalEvidenceView>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompareDocumentsRequest {
    pub document_ids: Vec<DocumentId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompareDocumentsView {
    pub documents: Vec<DocumentDetailView>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DocumentLifecycleView {
    Received,
    Extracted,
    Indexed,
    Failed,
}

impl DocumentLifecycleView {
    pub fn from_domain(value: domain_model::DocumentLifecycle) -> Self {
        match value {
            domain_model::DocumentLifecycle::Received => Self::Received,
            domain_model::DocumentLifecycle::Extracted => Self::Extracted,
            domain_model::DocumentLifecycle::Indexed => Self::Indexed,
            domain_model::DocumentLifecycle::Failed => Self::Failed,
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "received" => Some(Self::Received),
            "extracted" => Some(Self::Extracted),
            "indexed" => Some(Self::Indexed),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryDirectoryManifestView {
    pub schema_version: String,
    pub generator: String,
    pub dataset_id: DatasetId,
    pub version_no: i32,
    pub include_directory: bool,
    pub root: MemoryDirectoryNodeView,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryDirectoryNodeKind {
    Dataset,
    Document,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryDirectoryNodeScopeView {
    Dataset,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryDirectoryDocumentView {
    pub id: DocumentId,
    pub lifecycle: DocumentLifecycleView,
    pub chunk_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryDirectoryNodeView {
    pub kind: MemoryDirectoryNodeKind,
    pub title: String,
    pub scope: Option<MemoryDirectoryNodeScopeView>,
    pub version_no: Option<i32>,
    pub document: Option<MemoryDirectoryDocumentView>,
    pub children: Vec<MemoryDirectoryNodeView>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryDirectoryView {
    pub id: MemoryDirectoryId,
    pub dataset_id: DatasetId,
    pub execution_id: WorkflowExecutionId,
    pub version_no: i32,
    pub directory_nodes: i32,
    pub refreshed_chunks: i32,
    pub directory_manifest: Value,
    pub directory_tree: Option<MemoryDirectoryManifestView>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ManifestContextBindingView {
    CreationTime,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ManifestRuntimeModeView {
    Placeholder,
    Provider,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManifestFinishReasonView {
    Stop,
    ToolCalls,
    Length,
    ContentFilter,
    Error,
    Other(String),
}

impl ManifestFinishReasonView {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Stop => "stop",
            Self::ToolCalls => "tool_calls",
            Self::Length => "length",
            Self::ContentFilter => "content_filter",
            Self::Error => "error",
            Self::Other(value) => value.as_str(),
        }
    }
}

impl Serialize for ManifestFinishReasonView {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ManifestFinishReasonView {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "stop" => Self::Stop,
            "tool_calls" => Self::ToolCalls,
            "length" => Self::Length,
            "content_filter" => Self::ContentFilter,
            "error" => Self::Error,
            _ => Self::Other(value),
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestTokenUsageView {
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub total_tokens: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LlmInvocationSourceKindView {
    DatasetOutput,
    ChatMessage,
    WorkflowExecution,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LlmInvocationModeView {
    Placeholder,
    Provider,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LlmInvocationFinishReasonView {
    Stop,
    ToolCalls,
    Length,
    ContentFilter,
    Error,
    Other(String),
}

impl LlmInvocationFinishReasonView {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Stop => "stop",
            Self::ToolCalls => "tool_calls",
            Self::Length => "length",
            Self::ContentFilter => "content_filter",
            Self::Error => "error",
            Self::Other(value) => value.as_str(),
        }
    }
}

impl Serialize for LlmInvocationFinishReasonView {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for LlmInvocationFinishReasonView {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "stop" => Self::Stop,
            "tool_calls" => Self::ToolCalls,
            "length" => Self::Length,
            "content_filter" => Self::ContentFilter,
            "error" => Self::Error,
            _ => Self::Other(value),
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LlmTokenUsageView {
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub total_tokens: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LlmInvocationView {
    pub id: LlmInvocationId,
    pub execution_id: WorkflowExecutionId,
    pub source_kind: LlmInvocationSourceKindView,
    pub dataset_output_id: Option<DatasetOutputId>,
    pub chat_message_id: Option<ChatMessageId>,
    pub sequence_no: i32,
    pub mode: LlmInvocationModeView,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub request_id: Option<String>,
    pub finish_reason: Option<LlmInvocationFinishReasonView>,
    pub latency_ms: Option<u64>,
    pub usage: Option<LlmTokenUsageView>,
    pub system_prompt_key: Option<String>,
    pub system_prompt_version: Option<String>,
    pub tool_trace_count: Option<usize>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestRuntimeView {
    pub mode: ManifestRuntimeModeView,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub request_id: Option<String>,
    pub finish_reason: Option<ManifestFinishReasonView>,
    pub latency_ms: Option<u64>,
    pub usage: Option<ManifestTokenUsageView>,
    pub system_prompt_key: Option<String>,
    pub system_prompt_version: Option<String>,
    pub tool_trace_count: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ManifestToolCallStatusView {
    Requested,
    Completed,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestToolCallView {
    pub call_id: Option<String>,
    pub tool_name: String,
    pub tool: Option<ToolReferenceView>,
    pub status: ManifestToolCallStatusView,
    pub arguments: Option<Value>,
    pub result: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DatasetOutputFormatView {
    Markdown,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DatasetOutputSectionKindView {
    Summary,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DatasetOutputSectionView {
    pub section_key: String,
    pub kind: DatasetOutputSectionKindView,
    pub title: String,
    pub content: String,
    pub retrieval_evidence_ids: Vec<RetrievalEvidenceId>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DatasetOutputContentView {
    pub format: DatasetOutputFormatView,
    pub sections: Vec<DatasetOutputSectionView>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DatasetOutputManifestView {
    pub generator: String,
    pub schema_version: String,
    pub dataset_id: DatasetId,
    pub prompt: String,
    pub indexed_document_count: usize,
    pub refreshed_chunks: usize,
    pub memory_directory_id: Option<MemoryDirectoryId>,
    pub memory_directory_version_no: Option<i32>,
    pub retrieval_evidence_count: usize,
    pub retrieval_evidence_ids: Vec<RetrievalEvidenceId>,
    pub output: Option<DatasetOutputContentView>,
    #[serde(default)]
    pub service_handoff: Option<ManifestServiceHandoffView>,
    pub tool_trace: Vec<ManifestToolCallView>,
    pub context_binding: ManifestContextBindingView,
    pub runtime: Option<ManifestRuntimeView>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChatMessageOutputFormatView {
    Markdown,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChatMessageSectionKindView {
    Reply,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatMessageSectionView {
    pub section_key: String,
    pub kind: ChatMessageSectionKindView,
    pub title: String,
    pub content: String,
    pub retrieval_evidence_ids: Vec<RetrievalEvidenceId>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatMessageOutputView {
    pub format: ChatMessageOutputFormatView,
    pub sections: Vec<ChatMessageSectionView>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DatasetOutputView {
    pub id: DatasetOutputId,
    pub dataset_id: DatasetId,
    pub execution_id: WorkflowExecutionId,
    pub prompt: String,
    pub output_text: String,
    pub memory_directory_id: Option<MemoryDirectoryId>,
    pub memory_directory: Option<MemoryDirectoryView>,
    pub retrieval_evidence_ids: Vec<RetrievalEvidenceId>,
    pub retrieval_evidences: Vec<RetrievalEvidenceView>,
    pub llm_invocations: Vec<LlmInvocationView>,
    pub tool_executions: Vec<ToolExecutionView>,
    pub output_manifest: Value,
    pub output_manifest_view: Option<DatasetOutputManifestView>,
    #[serde(default)]
    pub model_facing: Option<WorkflowModelFacingSummaryView>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChatSessionManifestStatusView {
    PendingAssistantReply,
    AssistantReplied,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChatSessionTurnKindView {
    PlaceholderOrchestration,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChatTurnStatusView {
    Pending,
    Completed,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChatTurnStreamModeView {
    Buffered,
    Streaming,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChatTurnStreamStatusView {
    NotRequested,
    Pending,
    Completed,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChatTurnArtifactCommitStatusView {
    NotReady,
    Pending,
    Failed,
    Completed,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChatTurnArtifactCommitFailureSourceView {
    WorkflowEventRecoveryPersist,
    ResponseReadySessionUpdate,
    AssistantMessageCreate,
    AssistantMessageManifestUpdate,
    LlmInvocationPersist,
    ToolExecutionPersist,
    SessionContextUpdate,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChatTurnProviderStatusView {
    Pending,
    Responded,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChatTurnToolLoopStatusView {
    NotRequested,
    Pending,
    Completed,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChatTurnEventKindView {
    TurnStarted,
    ProviderRequested,
    ProviderResponded,
    FirstTokenEmitted,
    StreamCompleted,
    StreamFailed,
    ArtifactCommitReady,
    ArtifactCommitFailed,
    ToolCallsEmitted,
    ToolLoopCompleted,
    ToolLoopFailed,
    AssistantMessagePersisted,
    TurnCompleted,
    TurnFailed,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatTurnEventView {
    pub kind: ChatTurnEventKindView,
    pub at: DateTime<Utc>,
    pub provider_request_id: Option<String>,
    pub finish_reason: Option<ManifestFinishReasonView>,
    pub tool_trace_count: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatTurnToolStatusSummaryView {
    pub requested_count: usize,
    pub completed_count: usize,
    pub failed_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatTurnRuntimeView {
    pub turn_id: String,
    pub status: ChatTurnStatusView,
    pub stream_mode: ChatTurnStreamModeView,
    pub stream_status: ChatTurnStreamStatusView,
    pub artifact_commit_status: ChatTurnArtifactCommitStatusView,
    pub provider_status: ChatTurnProviderStatusView,
    pub tool_loop_status: ChatTurnToolLoopStatusView,
    pub provider_request_id: Option<String>,
    pub provider_requested_at: Option<DateTime<Utc>>,
    pub provider_responded_at: Option<DateTime<Utc>>,
    pub first_token_at: Option<DateTime<Utc>>,
    pub stream_completed_at: Option<DateTime<Utc>>,
    pub artifact_commit_ready_at: Option<DateTime<Utc>>,
    pub artifact_commit_failure_source: Option<ChatTurnArtifactCommitFailureSourceView>,
    pub tool_calls_emitted_at: Option<DateTime<Utc>>,
    pub tool_loop_settled_at: Option<DateTime<Utc>>,
    pub finish_reason: Option<ManifestFinishReasonView>,
    pub assistant_message_id: Option<ChatMessageId>,
    pub assistant_message_persisted_at: Option<DateTime<Utc>>,
    pub tool_trace_count: usize,
    pub tool_status_summary: Option<ChatTurnToolStatusSummaryView>,
    pub events: Vec<ChatTurnEventView>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChatSessionReportEntryResolutionView {
    StayMaterialService,
    EnterReportService,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ManifestServiceHandoffSourceView {
    ChatSessionReportEntry,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestServiceHandoffView {
    pub source: ManifestServiceHandoffSourceView,
    pub service_lane: ModelFacingServiceLaneView,
    pub report_entry_state: ModelFacingReportEntryStateView,
    pub requested_at: Option<DateTime<Utc>>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolved_action: Option<ChatSessionReportEntryResolutionView>,
    pub suggested_title: Option<String>,
    pub suggested_objective: Option<String>,
    pub confirmed_report_plan_id: Option<ReportPlanId>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatSessionReportEntryView {
    pub state: ModelFacingReportEntryStateView,
    pub requested_at: Option<DateTime<Utc>>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolved_action: Option<ChatSessionReportEntryResolutionView>,
    pub suggested_title: Option<String>,
    pub suggested_objective: Option<String>,
    pub confirmed_report_plan_id: Option<ReportPlanId>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatSessionManifestView {
    pub generator: Option<String>,
    pub schema_version: Option<String>,
    pub status: ChatSessionManifestStatusView,
    pub initial_prompt: Option<String>,
    pub last_prompt: Option<String>,
    pub last_turn_kind: Option<ChatSessionTurnKindView>,
    pub context_binding: Option<ManifestContextBindingView>,
    pub latest_memory_directory_id: Option<MemoryDirectoryId>,
    pub latest_memory_directory_version_no: Option<i32>,
    pub latest_dataset_output_id: Option<DatasetOutputId>,
    pub report_entry: Option<ChatSessionReportEntryView>,
    pub last_turn: Option<ChatTurnRuntimeView>,
    pub runtime: Option<ManifestRuntimeView>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatSessionView {
    pub id: ChatSessionId,
    pub dataset_id: DatasetId,
    pub execution_id: WorkflowExecutionId,
    pub title: String,
    pub latest_memory_directory_id: Option<MemoryDirectoryId>,
    pub latest_memory_directory: Option<MemoryDirectoryView>,
    pub latest_dataset_output_id: Option<DatasetOutputId>,
    pub latest_dataset_output: Option<DatasetOutputView>,
    pub latest_assistant_message_id: Option<ChatMessageId>,
    pub latest_assistant_message: Option<ChatMessageView>,
    pub session_manifest: Value,
    pub session_manifest_view: Option<ChatSessionManifestView>,
    #[serde(default)]
    pub model_facing: Option<WorkflowModelFacingSummaryView>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatMessageManifestView {
    pub generator: String,
    pub schema_version: String,
    pub dataset_id: DatasetId,
    pub prompt: String,
    pub indexed_document_count: usize,
    pub refreshed_chunks: usize,
    pub prior_message_count: usize,
    pub latest_memory_directory_id: Option<MemoryDirectoryId>,
    pub latest_memory_directory_version_no: Option<i32>,
    pub latest_dataset_output_id: Option<DatasetOutputId>,
    pub output: Option<ChatMessageOutputView>,
    #[serde(default)]
    pub service_handoff: Option<ManifestServiceHandoffView>,
    pub tool_trace: Vec<ManifestToolCallView>,
    pub turn: Option<ChatTurnRuntimeView>,
    pub context_binding: ManifestContextBindingView,
    pub runtime: Option<ManifestRuntimeView>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessageView {
    pub id: ChatMessageId,
    pub session_id: ChatSessionId,
    pub role: ChatMessageRole,
    pub turn_index: i32,
    pub content: String,
    pub llm_invocations: Vec<LlmInvocationView>,
    pub tool_executions: Vec<ToolExecutionView>,
    pub message_manifest: Value,
    pub message_manifest_view: Option<ChatMessageManifestView>,
    #[serde(default)]
    pub model_facing: Option<WorkflowModelFacingSummaryView>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReportPlanSummary {
    pub id: ReportPlanId,
    pub dataset_id: DatasetId,
    pub title: String,
    pub objective: String,
    pub status: ReportPlanStatusView,
    pub theme_key: String,
    pub current_ast_version_id: Option<ReportPlanAstVersionId>,
    #[serde(default)]
    pub service_handoff: Option<ManifestServiceHandoffView>,
    #[serde(default)]
    pub model_facing: Option<WorkflowModelFacingSummaryView>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReportPlanStatusView {
    Draft,
    Planned,
    Rendered,
    Published,
}

impl ReportPlanStatusView {
    pub fn from_domain(value: domain_model::ReportPlanStatus) -> Self {
        match value {
            domain_model::ReportPlanStatus::Draft => Self::Draft,
            domain_model::ReportPlanStatus::Planned => Self::Planned,
            domain_model::ReportPlanStatus::Rendered => Self::Rendered,
            domain_model::ReportPlanStatus::Published => Self::Published,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReportPlanAstVersionView {
    pub id: ReportPlanAstVersionId,
    pub plan_id: ReportPlanId,
    pub version_no: i32,
    pub ast: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateReportPlanResponse {
    pub plan: ReportPlanSummary,
    pub workflow_execution: WorkflowExecutionView,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateReportRenderRequest {
    pub surface: PublishedSurface,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateReportRenderResponse {
    pub workflow_execution: WorkflowExecutionView,
    pub requested_ast_version_id: ReportPlanAstVersionId,
    pub surface: PublishedSurface,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublishReportResponse {
    pub report: PublishedReportView,
    pub version: PublishedReportVersionView,
    pub source_render_output: ReportRenderOutputView,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublishedReportDetailView {
    pub report: PublishedReportView,
    pub current_version: Option<PublishedReportVersionView>,
    pub versions: Vec<PublishedReportVersionView>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegisterDocumentRequest {
    pub dataset_id: DatasetId,
    pub title: String,
    pub object_key: String,
    pub content_type: String,
    pub secret_binding_ids: Vec<SecretBindingId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegisterDocumentResponse {
    pub document: DocumentSummary,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateDocumentIngestResponse {
    pub document: DocumentSummary,
    pub workflow_execution: WorkflowExecutionView,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateMemoryDirectoryRefreshResponse {
    pub workflow_execution: WorkflowExecutionView,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateDatasetOutputRequest {
    pub prompt: String,
    #[serde(default)]
    pub chat_session_id: Option<ChatSessionId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateDatasetOutputResponse {
    pub workflow_execution: WorkflowExecutionView,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateChatSessionRequest {
    pub prompt: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateChatSessionResponse {
    pub chat_session: ChatSessionView,
    pub workflow_execution: WorkflowExecutionView,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppendChatSessionTurnRequest {
    pub prompt: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppendChatSessionTurnResponse {
    pub chat_session: ChatSessionView,
    pub user_message: ChatMessageView,
    pub workflow_execution: WorkflowExecutionView,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChatSessionReportEntryActionView {
    RequestConfirmation,
    StayMaterialService,
    EnterReportService,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateChatSessionReportEntryRequest {
    pub action: ChatSessionReportEntryActionView,
    pub title: Option<String>,
    pub objective: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateChatSessionReportEntryResponse {
    pub chat_session: ChatSessionView,
    pub report_plan: Option<ReportPlanSummary>,
    pub workflow_execution: Option<WorkflowExecutionView>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowRuntimeInspectView {
    pub execution: WorkflowExecutionView,
    pub execution_scope_runtime: Option<WorkflowExecutionRuntimeSummaryView>,
    pub dataset_output: Option<DatasetOutputView>,
    pub chat_session: Option<ChatSessionView>,
    #[serde(default)]
    pub report_plan: Option<ReportPlanSummary>,
    #[serde(default)]
    pub report_render_output: Option<ReportRenderOutputView>,
    pub chat_messages: Vec<ChatMessageView>,
    pub llm_invocations: Vec<LlmInvocationView>,
    pub tool_executions: Vec<ToolExecutionView>,
    #[serde(default)]
    pub model_facing: Option<WorkflowModelFacingSummaryView>,
    #[serde(default)]
    pub pretty_summaries: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowExecutionRuntimeSummaryView {
    pub llm_invocation_count: usize,
    pub tool_execution_count: usize,
    pub failed_tool_execution_count: usize,
    pub latest_provider: Option<String>,
    pub latest_model: Option<String>,
    pub latest_request_id: Option<String>,
    pub latest_finish_reason: Option<LlmInvocationFinishReasonView>,
    pub latest_tool_trace_count: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelFacingCapabilityClassView {
    DatasetDirectoryAwareness,
    EvidenceRetrieval,
    MaterialExplanationAndSynthesis,
    ReportPlanning,
    ReportGenerationAndEditing,
    ControlledPlatformAction,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelFacingEvidenceStateView {
    CatalogMemory,
    SupplyOnly,
    LiveDetail,
    Mixed,
    Degraded,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelFacingServiceLaneView {
    MaterialService,
    ReportService,
    ControlledPlatformAction,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelFacingReportEntryStateView {
    NotApplicable,
    ConfirmationRequired,
    Confirmed,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelFacingNextActionView {
    AnswerDirectly,
    ReadDocumentDetail,
    CompareDocuments,
    RequestReportEntryConfirmation,
    WaitForToolLoop,
    FinalizeArtifactCommit,
    RetryExecution,
    RefreshDirectory,
    ContinueReportPlanning,
    GenerateReportOutput,
    PublishReport,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelFacingContinuationStateView {
    ReadyToAnswer,
    NeedsPlatformContinuation,
    NeedsUserConfirmation,
    WaitingForRuntime,
    RetryRequired,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkflowModelFacingSummaryView {
    pub capability_class: ModelFacingCapabilityClassView,
    pub service_lane: ModelFacingServiceLaneView,
    pub report_entry_state: ModelFacingReportEntryStateView,
    pub evidence_state: ModelFacingEvidenceStateView,
    pub continuation_state: ModelFacingContinuationStateView,
    #[serde(default)]
    pub recommended_next_action: Option<ModelFacingNextActionView>,
    #[serde(default)]
    pub allowed_next_actions: Vec<ModelFacingNextActionView>,
    #[serde(default)]
    pub recommended_tool_key: Option<String>,
    #[serde(default)]
    pub allowed_tool_keys: Vec<String>,
    #[serde(default)]
    pub signals: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlanReportRequest {
    pub dataset_id: DatasetId,
    pub title: String,
    pub objective: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublishReportRequest {
    pub surface: PublishedSurface,
    pub publish_note: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventEnvelope<T> {
    pub name: String,
    pub occurred_at: DateTime<Utc>,
    pub payload: T,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentIngestRequested {
    pub document_id: DocumentId,
    pub dataset_id: DatasetId,
    pub content_type: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReportRenderRequested {
    pub plan_id: ReportPlanId,
    pub dataset_id: DatasetId,
    pub theme_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReportRenderOutputView {
    pub id: ReportRenderOutputId,
    pub execution_id: WorkflowExecutionId,
    pub plan_id: ReportPlanId,
    pub dataset_id: DatasetId,
    pub ast_version_id: ReportPlanAstVersionId,
    pub surface: PublishedSurface,
    pub status: ReportRenderOutputStatusView,
    pub asset_manifest: Value,
    #[serde(default)]
    pub service_handoff: Option<ManifestServiceHandoffView>,
    #[serde(default)]
    pub model_facing: Option<WorkflowModelFacingSummaryView>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublishedReportView {
    pub id: PublishedReportId,
    pub dataset_id: DatasetId,
    pub plan_id: ReportPlanId,
    pub slug: String,
    pub current_version_id: Option<PublishedReportVersionId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublishedReportVersionView {
    pub id: PublishedReportVersionId,
    pub report_id: PublishedReportId,
    pub version_no: i32,
    pub surface: PublishedSurface,
    pub asset_manifest: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReportRenderOutputStatusView {
    Rendered,
    Failed,
}

impl ReportRenderOutputStatusView {
    pub fn from_domain(value: domain_model::ReportRenderOutputStatus) -> Self {
        match value {
            domain_model::ReportRenderOutputStatus::Rendered => Self::Rendered,
            domain_model::ReportRenderOutputStatus::Failed => Self::Failed,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PromptDefinitionView {
    pub key: String,
    pub surface: String,
    pub active_version: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;

    #[test]
    fn workflow_runtime_inspect_view_defaults_pretty_summaries_when_missing() {
        let execution = serde_json::to_value(WorkflowExecutionView {
            id: WorkflowExecutionId::new(),
            kind: domain_model::WorkflowKind::ChatSession,
            status: domain_model::WorkflowStatus::Succeeded,
            stage: "completed".to_string(),
            updated_at: Utc::now(),
        })
        .expect("execution view should serialize");

        let view: WorkflowRuntimeInspectView = serde_json::from_value(json!({
            "execution": execution,
            "execution_scope_runtime": null,
            "dataset_output": null,
            "chat_session": null,
            "chat_messages": [],
            "llm_invocations": [],
            "tool_executions": []
        }))
        .expect("missing pretty_summaries should deserialize");

        assert!(view.pretty_summaries.is_empty());
    }
}
