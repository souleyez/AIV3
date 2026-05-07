use chrono::{DateTime, Utc};
use domain_model::{
    AssistantRunEventId, AssistantRunId, AuthChallengePurpose, AuthSessionMethod, ChatMessageId,
    ChatMessageRole, ChatSessionId, ConversationMemoryItemId, DatasetId, DatasetLifecycle,
    DatasetOutputId, DatasetVisibility, DocumentChunkId, DocumentId, EmailVerificationChallengeId,
    LlmInvocationId, MemoryDirectoryId, PublishedReportId, PublishedReportVersionId,
    PublishedSurface, ReportPlanAstVersionId, ReportPlanId, ReportRenderOutputId,
    RetrievalEvidenceId, SecretBindingId, StaticPageDraftId, StaticPageImageJobId,
    StaticPageRenderOutputId, ToolExecutionId, UserId, UserSessionId, WorkflowEventId,
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthUserView {
    pub id: UserId,
    pub email: String,
    pub display_name: String,
    pub email_verified: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthSessionView {
    pub id: UserSessionId,
    pub user_id: UserId,
    pub email: String,
    pub auth_method: AuthSessionMethod,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthSessionResponse {
    #[serde(default)]
    pub user: Option<AuthUserView>,
    #[serde(default)]
    pub session: Option<AuthSessionView>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StartEmailAuthRequest {
    pub email: String,
    pub purpose: AuthChallengePurpose,
    #[serde(default)]
    pub device_fingerprint: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StartEmailAuthResponse {
    pub challenge_id: EmailVerificationChallengeId,
    pub email: String,
    pub purpose: AuthChallengePurpose,
    pub expires_at: DateTime<Utc>,
    #[serde(default)]
    pub resend_after_seconds: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerifyEmailAuthRequest {
    pub email: String,
    pub code: String,
    pub purpose: AuthChallengePurpose,
    #[serde(default)]
    pub device_fingerprint: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerifyEmailAuthResponse {
    pub user: AuthUserView,
    pub session: AuthSessionView,
    #[serde(default)]
    pub active_secret_binding_ids: Vec<SecretBindingId>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeyLoginRequest {
    pub email: String,
    pub local_key: String,
    #[serde(default)]
    pub device_fingerprint: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeyLoginResponse {
    pub user: AuthUserView,
    pub session: AuthSessionView,
    #[serde(default)]
    pub active_secret_binding_ids: Vec<SecretBindingId>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeyRotateRequest {
    pub new_local_key: String,
    #[serde(default)]
    pub current_local_key: Option<String>,
    #[serde(default)]
    pub email_verification_code: Option<String>,
    #[serde(default)]
    pub device_fingerprint: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeyRotateResponse {
    pub user: AuthUserView,
    pub primary_secret_fingerprint: String,
    #[serde(default)]
    pub active_secret_binding_ids: Vec<SecretBindingId>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BindEmailRequest {
    pub email: String,
    #[serde(default)]
    pub device_fingerprint: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BindEmailResponse {
    pub challenge_id: EmailVerificationChallengeId,
    pub email: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClaimLocalDataRequest {
    pub fingerprint: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClaimLocalDataResponse {
    pub user: AuthUserView,
    pub claimed_datasets: Vec<DatasetSummary>,
    pub active_secret_binding_ids: Vec<SecretBindingId>,
    pub skipped_owned_dataset_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LogoutResponse {
    pub revoked: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateDatasetRequest {
    pub key: String,
    pub title: String,
    pub description: Option<String>,
    #[serde(default)]
    pub visibility: Option<DatasetVisibility>,
    #[serde(default)]
    pub secret_binding_ids: Vec<SecretBindingId>,
    #[serde(default)]
    pub secret_fingerprint: Option<String>,
    #[serde(default)]
    pub secret_label: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DatasetSummary {
    pub id: DatasetId,
    pub key: String,
    pub title: String,
    pub lifecycle: DatasetLifecycle,
    pub visibility: DatasetVisibility,
    pub secret_binding_ids: Vec<SecretBindingId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_warning: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateDatasetSecretBindingRequest {
    pub dataset_id: DatasetId,
    pub fingerprint: String,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateDatasetSecretBindingResponse {
    pub dataset: DatasetSummary,
    pub secret_binding_id: SecretBindingId,
    pub active_secret_binding_ids: Vec<SecretBindingId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResolveDatasetSecretBindingsRequest {
    pub fingerprint: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResolveDatasetSecretBindingsResponse {
    pub secret_binding_ids: Vec<SecretBindingId>,
    pub datasets: Vec<DatasetSummary>,
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
pub struct RetrievalSearchHitView {
    pub retrieval_evidence_id: RetrievalEvidenceId,
    pub document_id: DocumentId,
    pub score: f64,
    pub summary: String,
    pub source_locator: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RetrievalSearchResponse {
    pub hits: Vec<RetrievalSearchHitView>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentDetailView {
    pub document: DocumentSummary,
    pub chunks: Vec<DocumentChunkView>,
    pub retrieval_evidences: Vec<RetrievalEvidenceView>,
    #[serde(default)]
    pub model_facing: Option<WorkflowModelFacingSummaryView>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompareDocumentsRequest {
    pub document_ids: Vec<DocumentId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompareDocumentsView {
    pub documents: Vec<DocumentDetailView>,
    #[serde(default)]
    pub model_facing: Option<WorkflowModelFacingSummaryView>,
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
    pub owner_user_id: Option<UserId>,
    pub source_document_ids: Vec<DocumentId>,
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
#[serde(rename_all = "snake_case")]
pub enum ManifestProviderFailureKindView {
    RequestFailed,
    RequestTimeout,
    HttpStatus,
    ResponseBodyReadFailed,
    InvalidJson,
    InvalidResponse,
    FinishReasonError,
}

impl ManifestProviderFailureKindView {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RequestFailed => "request_failed",
            Self::RequestTimeout => "request_timeout",
            Self::HttpStatus => "http_status",
            Self::ResponseBodyReadFailed => "response_body_read_failed",
            Self::InvalidJson => "invalid_json",
            Self::InvalidResponse => "invalid_response",
            Self::FinishReasonError => "finish_reason_error",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestProviderFailureView {
    pub kind: ManifestProviderFailureKindView,
    pub message: String,
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
    #[serde(default)]
    pub provider_failure: Option<ManifestProviderFailureView>,
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
    #[serde(default)]
    pub provider_failure: Option<ManifestProviderFailureView>,
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
    #[serde(default)]
    pub metadata: Value,
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
pub struct AssistantRunMessageView {
    pub role: ChatMessageRole,
    pub content: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateAssistantRunRequest {
    pub prompt: String,
    #[serde(default)]
    pub local_thread_id: Option<String>,
    #[serde(default)]
    pub startup_briefing: Option<Value>,
    #[serde(default)]
    pub selected_scope: Option<Value>,
    #[serde(default)]
    pub scope_candidates: Vec<Value>,
    #[serde(default)]
    pub context_policy_hint: Option<Value>,
    #[serde(default)]
    pub current_artifact: Option<Value>,
    #[serde(default)]
    pub messages: Vec<AssistantRunMessageView>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateAssistantRunResponse {
    pub assistant_run_id: AssistantRunId,
    pub assistant_message: AssistantRunMessageView,
    pub runtime: Value,
    pub selected_scope: Value,
    pub scope_candidates: Vec<Value>,
    pub evidence_state: Value,
    pub execution_trail: Vec<Value>,
    pub output_artifacts: Vec<Value>,
    pub required_confirmations: Vec<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContinueAssistantRunRequest {
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub max_steps: Option<usize>,
    #[serde(default)]
    pub current_artifact: Option<Value>,
    #[serde(default)]
    pub messages: Vec<AssistantRunMessageView>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContinueAssistantRunResponse {
    pub run: AssistantRunView,
    pub assistant_message: AssistantRunMessageView,
    pub runtime: Value,
    pub event: AssistantRunEventView,
    pub selected_scope: Value,
    pub evidence_state: Value,
    pub execution_trail: Vec<Value>,
    pub output_artifacts: Vec<Value>,
    pub required_confirmations: Vec<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssistantRunView {
    pub id: AssistantRunId,
    pub local_thread_id: Option<String>,
    pub user_prompt: String,
    pub startup_briefing: Value,
    pub selected_scope: Value,
    pub scope_candidates: Vec<Value>,
    pub context_policy: Value,
    pub evidence_state: Value,
    pub service_lane: String,
    pub execution_trail: Vec<Value>,
    pub output_artifacts: Vec<Value>,
    pub runtime: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssistantRunEventView {
    pub id: AssistantRunEventId,
    pub run_id: AssistantRunId,
    pub sequence_no: i32,
    pub event_name: String,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssistantRunDetailView {
    pub run: AssistantRunView,
    pub events: Vec<AssistantRunEventView>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppendAssistantRunEventRequest {
    pub event_name: String,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppendAssistantRunEventResponse {
    pub run: AssistantRunView,
    pub event: AssistantRunEventView,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateStaticPageDraftRequest {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub selected_scope: Option<Value>,
    #[serde(default)]
    pub visibility_snapshot: Option<Value>,
    #[serde(default)]
    pub source_refs: Value,
    #[serde(default)]
    pub draft_payload: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StaticPageDraftStatusView {
    Draft,
    Planned,
    Queued,
    Previewed,
    Confirmed,
    Rendered,
    Archived,
}

impl StaticPageDraftStatusView {
    pub fn from_domain(value: domain_model::StaticPageDraftStatus) -> Self {
        match value {
            domain_model::StaticPageDraftStatus::Draft => Self::Draft,
            domain_model::StaticPageDraftStatus::Planned => Self::Planned,
            domain_model::StaticPageDraftStatus::Queued => Self::Queued,
            domain_model::StaticPageDraftStatus::Previewed => Self::Previewed,
            domain_model::StaticPageDraftStatus::Confirmed => Self::Confirmed,
            domain_model::StaticPageDraftStatus::Rendered => Self::Rendered,
            domain_model::StaticPageDraftStatus::Archived => Self::Archived,
        }
    }

    pub fn to_domain(&self) -> domain_model::StaticPageDraftStatus {
        match self {
            Self::Draft => domain_model::StaticPageDraftStatus::Draft,
            Self::Planned => domain_model::StaticPageDraftStatus::Planned,
            Self::Queued => domain_model::StaticPageDraftStatus::Queued,
            Self::Previewed => domain_model::StaticPageDraftStatus::Previewed,
            Self::Confirmed => domain_model::StaticPageDraftStatus::Confirmed,
            Self::Rendered => domain_model::StaticPageDraftStatus::Rendered,
            Self::Archived => domain_model::StaticPageDraftStatus::Archived,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StaticPageDraftView {
    pub id: StaticPageDraftId,
    pub assistant_run_id: AssistantRunId,
    pub title: String,
    pub status: StaticPageDraftStatusView,
    pub selected_scope: Value,
    pub visibility_snapshot: Value,
    pub source_refs: Value,
    pub draft_payload: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateStaticPageDraftResponse {
    pub draft: StaticPageDraftView,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UpdateStaticPageDraftRequest {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub status: Option<StaticPageDraftStatusView>,
    #[serde(default)]
    pub selected_scope: Option<Value>,
    #[serde(default)]
    pub visibility_snapshot: Option<Value>,
    #[serde(default)]
    pub source_refs: Option<Value>,
    #[serde(default)]
    pub draft_payload: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateStaticPageDraftResponse {
    pub draft: StaticPageDraftView,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppendStaticPageDraftOperationsRequest {
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub operations: Vec<Value>,
    #[serde(default)]
    pub draft_payload: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppendStaticPageDraftOperationsResponse {
    pub draft: StaticPageDraftView,
    pub operations: Vec<Value>,
    pub summary: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApplyStaticPageDraftIntentRequest {
    pub prompt: String,
    #[serde(default)]
    pub draft_payload: Option<Value>,
    #[serde(default)]
    pub messages: Vec<AssistantRunMessageView>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApplyStaticPageDraftIntentResponse {
    pub draft: StaticPageDraftView,
    pub operations: Vec<Value>,
    pub summary: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateStaticPageImageJobRequest {
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub image_prompt_payload: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StaticPageImageJobStatusView {
    Queued,
    Running,
    PreviewReady,
    Failed,
    Confirmed,
}

impl StaticPageImageJobStatusView {
    pub fn from_domain(value: domain_model::StaticPageImageJobStatus) -> Self {
        match value {
            domain_model::StaticPageImageJobStatus::Queued => Self::Queued,
            domain_model::StaticPageImageJobStatus::Running => Self::Running,
            domain_model::StaticPageImageJobStatus::PreviewReady => Self::PreviewReady,
            domain_model::StaticPageImageJobStatus::Failed => Self::Failed,
            domain_model::StaticPageImageJobStatus::Confirmed => Self::Confirmed,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StaticPageImageJobView {
    pub id: StaticPageImageJobId,
    pub draft_id: StaticPageDraftId,
    pub assistant_run_id: AssistantRunId,
    pub status: StaticPageImageJobStatusView,
    pub queue_position: Option<i32>,
    pub image_prompt_payload: Value,
    pub preview_asset_key: Option<String>,
    pub failure_reason: Option<String>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateStaticPageImageJobResponse {
    pub image_job: StaticPageImageJobView,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfirmStaticPageImageJobRequest {
    #[serde(default)]
    pub preview_asset_key: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfirmStaticPageImageJobResponse {
    pub image_job: StaticPageImageJobView,
    pub draft: StaticPageDraftView,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateStaticPageRenderRequest {
    #[serde(default)]
    pub image_job_id: Option<StaticPageImageJobId>,
    #[serde(default)]
    pub background: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StaticPageRenderOutputStatusView {
    Queued,
    Rendering,
    Rendered,
    Failed,
    Cancelled,
}

impl StaticPageRenderOutputStatusView {
    pub fn from_domain(value: domain_model::StaticPageRenderOutputStatus) -> Self {
        match value {
            domain_model::StaticPageRenderOutputStatus::Queued => Self::Queued,
            domain_model::StaticPageRenderOutputStatus::Rendering => Self::Rendering,
            domain_model::StaticPageRenderOutputStatus::Rendered => Self::Rendered,
            domain_model::StaticPageRenderOutputStatus::Failed => Self::Failed,
            domain_model::StaticPageRenderOutputStatus::Cancelled => Self::Cancelled,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StaticPageRenderOutputView {
    pub id: StaticPageRenderOutputId,
    pub draft_id: StaticPageDraftId,
    pub assistant_run_id: AssistantRunId,
    pub image_job_id: Option<StaticPageImageJobId>,
    pub status: StaticPageRenderOutputStatusView,
    pub html: String,
    pub asset_manifest: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateStaticPageRenderResponse {
    pub render_output: StaticPageRenderOutputView,
    pub draft: StaticPageDraftView,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateConversationMemoryItemRequest {
    pub local_thread_id: String,
    pub role: ChatMessageRole,
    pub item_kind: String,
    pub summary: String,
    #[serde(default)]
    pub source_message_refs: Value,
    #[serde(default)]
    pub artifact_refs: Value,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConversationMemoryItemView {
    pub id: ConversationMemoryItemId,
    pub local_thread_id: String,
    pub role: ChatMessageRole,
    pub item_kind: String,
    pub summary: String,
    pub source_message_refs: Value,
    pub artifact_refs: Value,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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

    #[test]
    fn auth_requests_use_snake_case_wire_values() {
        let start: StartEmailAuthRequest = serde_json::from_value(json!({
            "email": "user@example.com",
            "purpose": "recover_key",
            "device_fingerprint": "browser-device"
        }))
        .expect("start email auth request should deserialize");

        assert_eq!(start.purpose, AuthChallengePurpose::RecoverKey);
        assert_eq!(start.device_fingerprint.as_deref(), Some("browser-device"));

        let session = AuthSessionView {
            id: UserSessionId::new(),
            user_id: UserId::new(),
            email: "user@example.com".to_string(),
            auth_method: AuthSessionMethod::EmailCode,
            created_at: Utc::now(),
            expires_at: Utc::now(),
        };
        let encoded = serde_json::to_value(&session).expect("session view should serialize");

        assert_eq!(encoded["auth_method"], "email_code");

        let claim: ClaimLocalDataRequest = serde_json::from_value(json!({
            "fingerprint": "local-secret-fingerprint"
        }))
        .expect("claim local data request should deserialize");
        assert_eq!(claim.fingerprint, "local-secret-fingerprint");

        let rotate: KeyRotateRequest = serde_json::from_value(json!({
            "new_local_key": "next-local-key",
            "device_fingerprint": "browser-device"
        }))
        .expect("rotate key request should deserialize");
        assert_eq!(rotate.new_local_key, "next-local-key");
        assert_eq!(rotate.device_fingerprint.as_deref(), Some("browser-device"));
    }
}
