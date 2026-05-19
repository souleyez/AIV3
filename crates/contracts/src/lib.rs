use chrono::{DateTime, Utc};
use domain_model::{
    AssistantRunEventId, AssistantRunId, AuthAuditEventId, AuthAuditOutcome, AuthChallengePurpose,
    AuthSessionMethod, ChatMessageId, ChatMessageRole, ChatSessionId, ConversationMemoryItemId,
    DatasetId, DatasetLifecycle, DatasetOutputId, DatasetVisibility, DocumentChunkId, DocumentId,
    EmailVerificationChallengeId, LlmInvocationId, MemoryDirectoryId, PublishedReportId,
    PublishedReportVersionId, PublishedSurface, ReportPlanAstVersionId, ReportPlanId,
    ReportRenderOutputId, RetrievalEvidenceId, SecretBindingId, StaticPageDraftId,
    StaticPageImageJobId, StaticPageRenderOutputId, ToolExecutionId, UserId, UserSessionId,
    WorkflowEventId, WorkflowExecutionId, WorkflowKind, WorkflowStatus, WorkflowTaskId,
    WorkflowTaskStatus,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{json, Value};

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExternalChannelPlatformView {
    Feishu,
    Lark,
    WeCom,
    GenericChat,
    ThirdParty,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExternalMessageTypeView {
    Text,
    Image,
    File,
    Audio,
    Video,
    Card,
    Event,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExternalAttachmentRefView {
    pub attachment_external_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download_url_redacted: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExternalBotMessageView {
    pub platform: ExternalChannelPlatformView,
    pub tenant_external_id: String,
    pub bot_external_id: String,
    pub conversation_external_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_external_id: Option<String>,
    pub sender_external_id: String,
    pub message_external_id: String,
    pub message_type: ExternalMessageTypeView,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mention_external_user_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachment_refs: Vec<ExternalAttachmentRefView>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub available_document_external_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub available_document_source_id: Option<String>,
    pub idempotency_key: String,
    pub received_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExternalPrincipalTrustLevelView {
    Unresolved,
    Guest,
    ExternalUser,
    Employee,
    Manager,
    Admin,
    SystemOperator,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExternalPrincipalView {
    pub tenant_id: String,
    pub platform: ExternalChannelPlatformView,
    pub external_user_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub external_department_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub external_group_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub external_role_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v3_user_id: Option<String>,
    pub trust_level: ExternalPrincipalTrustLevelView,
    #[serde(default)]
    pub is_disabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ThirdPartyConnectorKindView {
    Document,
    UserDirectory,
    Artifact,
    ChatChannel,
    Action,
    Combined,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ThirdPartySyncModeView {
    Push,
    Pull,
    Hybrid,
    Manual,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ThirdPartyPermissionModeView {
    SourceAclSnapshot,
    SourceRuntimeCheck,
    V3Managed,
    TenantPublic,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationHealthStatusView {
    Unknown,
    Healthy,
    Degraded,
    Failing,
    Disabled,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct IntegrationHealthView {
    pub status: IntegrationHealthStatusView,
    pub checked_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_success_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_failure_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ThirdPartyKnowledgeSourceView {
    pub source_id: String,
    pub tenant_id: String,
    pub connector_kind: ThirdPartyConnectorKindView,
    pub base_url_redacted: String,
    pub sync_mode: ThirdPartySyncModeView,
    pub permission_mode: ThirdPartyPermissionModeView,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supported_capabilities: Vec<String>,
    pub health: IntegrationHealthView,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExternalDocumentAclSnapshotView {
    pub source_id: String,
    pub document_external_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_external_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_user_external_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_department_external_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_group_external_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_role_external_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub denied_user_external_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub denied_department_external_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub denied_group_external_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub denied_role_external_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acl_hash: Option<String>,
    pub captured_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExternalActionRiskLevelView {
    ReadOnly,
    LowRiskWrite,
    HighRiskWrite,
    CrossSystem,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExternalActionCapabilityView {
    ArtifactPublish,
    ArtifactRevoke,
    ArtifactStatus,
    BusinessAction,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExternalActionConfirmationModeView {
    NotRequired,
    OriginalChannel,
    TrustedCustomerPage,
    SourceSystem,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExternalActionPolicyView {
    pub capability: ExternalActionCapabilityView,
    pub action_type: String,
    pub target_system: String,
    pub risk_level: ExternalActionRiskLevelView,
    pub requires_confirmation: bool,
    pub confirmation_mode: ExternalActionConfirmationModeView,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExternalActionIntentView {
    pub action_id: String,
    pub tenant_id: String,
    pub requester: ExternalPrincipalView,
    pub risk_level: ExternalActionRiskLevelView,
    pub target_system: String,
    pub action_type: String,
    #[serde(default)]
    pub arguments_redacted: Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_evidence_refs: Vec<String>,
    pub requires_confirmation: bool,
    pub idempotency_key: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExternalActionConfirmationDecisionView {
    Approved,
    Rejected,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExternalActionConfirmationRequestView {
    pub assistant_run_id: AssistantRunId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_id: Option<String>,
    pub confirmation_external_id: String,
    pub sender_external_user_id: String,
    pub decision: ExternalActionConfirmationDecisionView,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    pub idempotency_key: String,
    pub confirmed_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExternalActionConfirmationResponseView {
    pub accepted: bool,
    pub assistant_run_id: AssistantRunId,
    pub action_id: String,
    pub confirmation_state: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ExternalActionResultCallbackRequestView {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_request_id: Option<String>,
    pub status: String,
    pub idempotency_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExternalActionResultCallbackResponseView {
    pub accepted: bool,
    pub action_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_request_id: Option<String>,
    pub status: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExternalBotReplyTypeView {
    TaskStatus,
    Text,
    Card,
    ArtifactLink,
    RequiresConfirmation,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExternalBotReplyView {
    pub target_conversation_external_id: String,
    pub reply_type: ExternalBotReplyTypeView,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card: Option<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifact_links: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_status: Option<String>,
    #[serde(default)]
    pub requires_confirmation: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirmation_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExternalChannelEventResponse {
    pub accepted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assistant_run_id: Option<AssistantRunId>,
    pub idempotency_key: String,
    pub reply: ExternalBotReplyView,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ExternalIntegrationSummaryView {
    pub integration_id: String,
    pub integration_kind: String,
    pub display_name: String,
    pub provider: String,
    pub status: String,
    pub health_status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_event_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sync_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_success_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_failure_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled_at: Option<DateTime<Utc>>,
    pub pending_action_count: i64,
    pub blocked_action_count: i64,
    pub failed_action_count: i64,
    pub dispatched_action_count: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_action_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub action_summary: Value,
    pub config_summary: Value,
    #[serde(default)]
    pub search_summary: Value,
    #[serde(default)]
    pub drift_summary: Value,
    #[serde(default)]
    pub artifact_summary: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ListExternalIntegrationsResponse {
    pub integrations: Vec<ExternalIntegrationSummaryView>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ExternalIntegrationAuditItemView {
    pub item_type: String,
    pub created_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assistant_run_id: Option<AssistantRunId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_kind: Option<String>,
    pub summary: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ExternalIntegrationAuditResponse {
    pub integration_id: String,
    pub items: Vec<ExternalIntegrationAuditItemView>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExternalIntegrationControlRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_kind: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExternalIntegrationControlResponse {
    pub accepted: bool,
    pub integration_id: String,
    pub integration_kind: String,
    pub action: String,
    pub status: String,
    pub message: String,
    pub affected_action_count: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow_execution: Option<WorkflowExecutionView>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enqueued_tasks: Vec<WorkflowTaskView>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateExternalSourceSyncRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dataset_id: Option<DatasetId>,
    #[serde(default)]
    pub checkpoint: Value,
    #[serde(default)]
    pub connector_context: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateExternalSourceSyncResponse {
    pub accepted: bool,
    pub source_id: String,
    pub sync_run_id: String,
    pub sync_kind: String,
    pub status: String,
    pub workflow_execution: WorkflowExecutionView,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enqueued_tasks: Vec<WorkflowTaskView>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateExternalDocumentParseRequest {
    pub source_id: String,
    pub dataset_id: DatasetId,
    pub document_external_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_external_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    pub content_url: String,
    #[serde(default)]
    pub metadata: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    #[serde(default)]
    pub allow_http_loopback: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExternalDocumentParseDocumentView {
    pub id: DocumentId,
    pub dataset_id: DatasetId,
    pub title: String,
    pub content_type: String,
    pub lifecycle: DocumentLifecycleView,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateExternalDocumentParseResponse {
    pub accepted: bool,
    pub source_id: String,
    pub document_external_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_external_id: Option<String>,
    pub document: ExternalDocumentParseDocumentView,
    pub workflow_execution: WorkflowExecutionView,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExternalDocumentParseDetailItemView {
    pub document_id: DocumentId,
    pub dataset_id: DatasetId,
    pub title: String,
    pub content_type: String,
    pub lifecycle: DocumentLifecycleView,
    pub source_id: String,
    pub document_external_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_external_id: Option<String>,
    pub chunk_count: usize,
    pub retrieval_evidence_count: usize,
    #[serde(default)]
    pub ingest: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetExternalDocumentParseDetailResponse {
    pub source_id: String,
    pub document_external_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle: Option<DocumentLifecycleView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunk_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retrieval_evidence_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ingest: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest: Option<ExternalDocumentParseDetailItemView>,
    pub documents: Vec<ExternalDocumentParseDetailItemView>,
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
pub struct CodexHostTaskMemoryPolicyView {
    pub kind: String,
    pub isolated: bool,
    pub promote_summary_to_conversation: bool,
    pub memory_space_id: Option<String>,
}

impl CodexHostTaskMemoryPolicyView {
    pub fn task_scoped(
        assistant_run_id: AssistantRunId,
        execution_id: WorkflowExecutionId,
    ) -> Self {
        Self {
            kind: "task".to_string(),
            isolated: true,
            promote_summary_to_conversation: false,
            memory_space_id: Some(format!(
                "codex-host-task:{}:{}",
                assistant_run_id, execution_id
            )),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexHostTaskSafetyPolicyView {
    pub user_cli_flags_allowed: bool,
    pub secrets_in_prompt_allowed: bool,
    pub raw_logs_require_redaction: bool,
    pub real_codex_exec_requires_host_allowlist: bool,
}

impl Default for CodexHostTaskSafetyPolicyView {
    fn default() -> Self {
        Self {
            user_cli_flags_allowed: false,
            secrets_in_prompt_allowed: false,
            raw_logs_require_redaction: true,
            real_codex_exec_requires_host_allowlist: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexHostTaskRequestView {
    pub assistant_run_id: AssistantRunId,
    pub capability: String,
    pub task: Option<String>,
    pub local_thread_id: Option<String>,
    pub task_memory_policy: CodexHostTaskMemoryPolicyView,
    pub safety: CodexHostTaskSafetyPolicyView,
}

impl CodexHostTaskRequestView {
    pub fn to_workflow_context(&self) -> Value {
        json!({
            "assistant_run_id": self.assistant_run_id.to_string(),
            "capability": self.capability.clone(),
            "task": self.task.clone(),
            "local_thread_id": self.local_thread_id.clone(),
            "task_memory_policy": self.task_memory_policy.clone(),
            "task_memory_space_id": self.task_memory_policy.memory_space_id.clone(),
            "safety": self.safety.clone(),
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexHostTaskProfileSummaryView {
    pub id: String,
    pub kind: String,
    pub model: Option<String>,
    pub provider_id: Option<String>,
    pub base_url_configured: bool,
    pub env_key: Option<String>,
    pub wire_api: Option<String>,
    pub allowed_capabilities: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexHostCommandPlanSummaryView {
    pub program: String,
    pub args_without_prompt: Vec<String>,
    pub prompt_chars: usize,
    pub sandbox: String,
    pub workspace_configured: bool,
    pub workspace_label: Option<String>,
    pub prompt_redacted: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexHostProcessOutputSummaryView {
    pub exit_code: Option<i32>,
    pub stdout_chars: usize,
    pub stderr_chars: usize,
    pub stdout_excerpt: String,
    pub stderr_excerpt: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HtmlArtifactSourceTypeView {
    CodexHost,
    StaticPage,
    Report,
    CodeReview,
    VideoExtraction,
    ExternalIntegration,
    Manual,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HtmlArtifactTemplateIdView {
    CodexExecutionReport,
    StaticPagePlanningHandoff,
    StaticPageDataQualityReport,
    ReportRenderSummary,
    CodeReviewSummary,
    VideoExtractionSummary,
    WechatVideoLoginHandoff,
    ThirdPartyHandoffDocument,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HtmlArtifactInteractionModeView {
    ReadOnly,
    JsonPatch,
    ActionIntent,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HtmlArtifactOwnerScopeView {
    #[serde(rename = "type")]
    pub scope_type: String,
    pub id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HtmlArtifactDataRefView {
    pub kind: String,
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HtmlArtifactProvenanceView {
    pub producer: String,
    pub reason: String,
    pub source_run_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HtmlArtifactManifestView {
    pub kind: String,
    pub version: u32,
    pub id: String,
    pub title: String,
    pub source_type: HtmlArtifactSourceTypeView,
    pub template_id: HtmlArtifactTemplateIdView,
    pub owner_scope: HtmlArtifactOwnerScopeView,
    #[serde(default)]
    pub data_refs: Vec<HtmlArtifactDataRefView>,
    pub provenance: HtmlArtifactProvenanceView,
    pub interaction_mode: HtmlArtifactInteractionModeView,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub payload: Value,
}

impl HtmlArtifactManifestView {
    pub fn codex_execution_report(
        assistant_run_id: &str,
        title: impl Into<String>,
        reason: impl Into<String>,
        payload: Value,
    ) -> Self {
        Self {
            kind: "html_artifact".to_string(),
            version: 1,
            id: format!("html-artifact-codex-{assistant_run_id}"),
            title: title.into(),
            source_type: HtmlArtifactSourceTypeView::CodexHost,
            template_id: HtmlArtifactTemplateIdView::CodexExecutionReport,
            owner_scope: HtmlArtifactOwnerScopeView {
                scope_type: "assistant_run".to_string(),
                id: assistant_run_id.to_string(),
            },
            data_refs: Vec::new(),
            provenance: HtmlArtifactProvenanceView {
                producer: "codex-host-agent".to_string(),
                reason: reason.into(),
                source_run_id: Some(assistant_run_id.to_string()),
            },
            interaction_mode: HtmlArtifactInteractionModeView::ReadOnly,
            created_at: Utc::now(),
            payload,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexHostTaskOutputView {
    pub mode: String,
    pub codex_invoked: bool,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_kind: Option<String>,
    pub assistant_run_id: String,
    pub capability: String,
    pub profile: Option<CodexHostTaskProfileSummaryView>,
    pub command_plan: Option<CodexHostCommandPlanSummaryView>,
    pub process: Option<CodexHostProcessOutputSummaryView>,
    pub task_chars: usize,
    pub local_thread_id: Option<String>,
    pub task_memory_isolated: bool,
    pub task_memory_space_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub html_artifacts: Vec<HtmlArtifactManifestView>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VideoExtractionSourceKindView {
    UploadedVideoFile,
    DirectVideoUrl,
    PublicPageResolvableVideo,
}

impl VideoExtractionSourceKindView {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UploadedVideoFile => "uploaded_video_file",
            Self::DirectVideoUrl => "direct_video_url",
            Self::PublicPageResolvableVideo => "public_page_resolvable_video",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct VideoExtractionSourceRefView {
    pub source_type: VideoExtractionSourceKindView,
    pub document_id: Option<DocumentId>,
    pub source_url: Option<String>,
    pub source_page_url: Option<String>,
    pub title_hint: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct VideoExtractionResolvedAssetRefView {
    pub document_id: DocumentId,
    pub dataset_id: DatasetId,
    pub title: String,
    pub content_type: String,
    pub asset_state: String,
    pub workflow_execution_id: Option<WorkflowExecutionId>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VideoExtractionArtifactKindView {
    TranscriptText,
    SlideImageCandidates,
    PptOutline,
    Pptx,
    Markdown,
    SourceText,
    TimestampMap,
    HtmlSummary,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct VideoExtractionArtifactRefView {
    pub artifact_kind: VideoExtractionArtifactKindView,
    pub artifact_id: String,
    pub title: String,
    pub format: String,
    pub uri: Option<String>,
    pub html_artifact_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VideoExtractionStageView {
    Queued,
    ResolvingSource,
    Registered,
    Parsing,
    ExtractingPpt,
    Completed,
    Failed,
    UnsupportedSource,
    Cancelled,
    DeadLettered,
}

impl VideoExtractionStageView {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::ResolvingSource => "resolving_source",
            Self::Registered => "registered",
            Self::Parsing => "parsing",
            Self::ExtractingPpt => "extracting_ppt",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::UnsupportedSource => "unsupported_source",
            Self::Cancelled => "cancelled",
            Self::DeadLettered => "dead_lettered",
        }
    }

    pub fn from_stage(value: &str) -> Option<Self> {
        match value {
            "queued" => Some(Self::Queued),
            "resolving_source" => Some(Self::ResolvingSource),
            "registered" => Some(Self::Registered),
            "parsing" => Some(Self::Parsing),
            "extracting_ppt" => Some(Self::ExtractingPpt),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "unsupported_source" => Some(Self::UnsupportedSource),
            "cancelled" => Some(Self::Cancelled),
            "dead_lettered" => Some(Self::DeadLettered),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct VideoExtractionRequestView {
    pub assistant_run_id: Option<AssistantRunId>,
    pub local_thread_id: Option<String>,
    pub source: VideoExtractionSourceRefView,
    pub target_dataset_id: Option<DatasetId>,
    pub requested_outputs: Vec<VideoExtractionArtifactKindView>,
    pub background_only: bool,
}

impl VideoExtractionRequestView {
    pub fn to_workflow_context(&self) -> Value {
        json!({
            "kind": WorkflowKind::VideoExtraction.as_str(),
            "assistant_run_id": self.assistant_run_id.map(|id| id.to_string()),
            "local_thread_id": self.local_thread_id.clone(),
            "source": self.source.clone(),
            "target_dataset_id": self.target_dataset_id.map(|id| id.to_string()),
            "requested_outputs": self.requested_outputs.clone(),
            "background_only": self.background_only,
            "queue": "media",
            "task_key": "resolve_video_source",
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct VideoExtractionStateView {
    pub workflow_execution_id: WorkflowExecutionId,
    pub stage: VideoExtractionStageView,
    pub status: WorkflowStatus,
    pub source: VideoExtractionSourceRefView,
    pub resolved_asset: Option<VideoExtractionResolvedAssetRefView>,
    pub artifacts: Vec<VideoExtractionArtifactRefView>,
    pub message: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssistantRunExecutorTransportView {
    Direct,
    CodexDryRun,
    CodexPlanOnly,
    CodexExecSchema,
    CodexSdkThread,
    CodexAppServer,
    CodexMcpServer,
}

impl AssistantRunExecutorTransportView {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::CodexDryRun => "codex_dry_run",
            Self::CodexPlanOnly => "codex_plan_only",
            Self::CodexExecSchema => "codex_exec_schema",
            Self::CodexSdkThread => "codex_sdk_thread",
            Self::CodexAppServer => "codex_app_server",
            Self::CodexMcpServer => "codex_mcp_server",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssistantRunCodexSafetyPolicyView {
    pub v3_validates_all_actions: bool,
    pub direct_database_access_allowed: bool,
    pub direct_queue_access_allowed: bool,
    pub direct_filesystem_access_allowed: bool,
    pub provider_keys_in_prompt_allowed: bool,
    pub raw_logs_require_redaction: bool,
    pub static_page_state_machine_owned_by_v3: bool,
}

impl Default for AssistantRunCodexSafetyPolicyView {
    fn default() -> Self {
        Self {
            v3_validates_all_actions: true,
            direct_database_access_allowed: false,
            direct_queue_access_allowed: false,
            direct_filesystem_access_allowed: false,
            provider_keys_in_prompt_allowed: false,
            raw_logs_require_redaction: true,
            static_page_state_machine_owned_by_v3: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssistantRunCodexContextBudgetItemView {
    pub category: String,
    pub item_count: usize,
    pub char_count: usize,
    pub soft_limit_chars: Option<usize>,
    pub pressure: String,
    pub trimmed_item_count: usize,
    #[serde(default)]
    pub note: Option<String>,
}

impl AssistantRunCodexContextBudgetItemView {
    pub fn new(
        category: impl Into<String>,
        item_count: usize,
        char_count: usize,
        soft_limit_chars: Option<usize>,
        trimmed_item_count: usize,
        note: Option<String>,
    ) -> Self {
        let pressure = match soft_limit_chars {
            Some(limit) if limit > 0 && char_count > limit => "over_limit",
            Some(limit) if limit > 0 && char_count * 10 >= limit * 7 => "attention",
            Some(_) => "ok",
            None => "unbounded",
        }
        .to_string();

        Self {
            category: category.into(),
            item_count,
            char_count,
            soft_limit_chars,
            pressure,
            trimmed_item_count,
            note,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssistantRunCodexContextBudgetView {
    pub quality_first: bool,
    pub max_prompt_chars: Option<usize>,
    #[serde(default)]
    pub estimated_prompt_chars: usize,
    #[serde(default)]
    pub budget_pressure: String,
    pub included_message_count: usize,
    pub selected_dataset_count: usize,
    pub evidence_item_count: usize,
    pub hidden_memory_item_count: usize,
    pub artifact_state_chars: usize,
    pub tool_output_chars: usize,
    pub trimmed_item_count: usize,
    #[serde(default)]
    pub items: Vec<AssistantRunCodexContextBudgetItemView>,
}

impl Default for AssistantRunCodexContextBudgetView {
    fn default() -> Self {
        Self {
            quality_first: true,
            max_prompt_chars: None,
            estimated_prompt_chars: 0,
            budget_pressure: "unbounded".to_string(),
            included_message_count: 0,
            selected_dataset_count: 0,
            evidence_item_count: 0,
            hidden_memory_item_count: 0,
            artifact_state_chars: 0,
            tool_output_chars: 0,
            trimmed_item_count: 0,
            items: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderShimHealthStatusView {
    Healthy,
    Degraded,
    Unavailable,
    Unknown,
}

impl ProviderShimHealthStatusView {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::Degraded => "degraded",
            Self::Unavailable => "unavailable",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderShimHealthView {
    pub status: ProviderShimHealthStatusView,
    pub process_reachable: bool,
    pub upstream_reachable: bool,
    pub checked_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub message: Option<String>,
}

impl ProviderShimHealthView {
    pub fn unknown(message: impl Into<String>) -> Self {
        Self {
            status: ProviderShimHealthStatusView::Unknown,
            process_reachable: false,
            upstream_reachable: false,
            checked_at: None,
            message: Some(message.into()),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderShimRateLimitHintsView {
    pub requests_per_minute: Option<u32>,
    pub tokens_per_minute: Option<u32>,
    pub concurrent_requests: Option<u32>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderShimCostHintsView {
    pub input_microusd_per_million_tokens: Option<u64>,
    pub output_microusd_per_million_tokens: Option<u64>,
    pub currency: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderShimRedactionPolicyView {
    pub redact_provider_errors: bool,
    pub redact_request_payloads: bool,
    pub redact_response_payloads: bool,
    pub max_error_chars: usize,
}

impl Default for ProviderShimRedactionPolicyView {
    fn default() -> Self {
        Self {
            redact_provider_errors: true,
            redact_request_payloads: true,
            redact_response_payloads: true,
            max_error_chars: 0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderShimProfileSnapshotView {
    pub profile_id: String,
    pub provider_id: String,
    pub model_id: String,
    pub wire_api: String,
    pub endpoint_scope: String,
    pub base_url_configured: bool,
    pub api_path: Option<String>,
    pub auth_env_key_name: Option<String>,
    pub auth_configured: bool,
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    pub rate_limit: ProviderShimRateLimitHintsView,
    pub cost: ProviderShimCostHintsView,
    pub redaction: ProviderShimRedactionPolicyView,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderShimUsageSummaryView {
    pub request_count: u64,
    pub failed_request_count: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub last_request_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderShimUsageEventView {
    pub request_id: Option<String>,
    pub assistant_run_id: Option<String>,
    pub workflow_execution_id: Option<String>,
    pub status: String,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub latency_ms: Option<u64>,
    pub provider_failure_kind: Option<String>,
    pub provider_failure_message: Option<String>,
    pub recorded_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderShimBalanceView {
    pub supported: bool,
    pub currency: Option<String>,
    pub amount_microunits: Option<i64>,
    pub checked_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderShimDebugTraceStatusView {
    pub enabled: bool,
    pub redacted: bool,
    pub storage: String,
    pub retained_trace_count: usize,
    pub latest_trace_id: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

impl Default for ProviderShimDebugTraceStatusView {
    fn default() -> Self {
        Self {
            enabled: false,
            redacted: true,
            storage: "disabled".to_string(),
            retained_trace_count: 0,
            latest_trace_id: None,
            note: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderShimContextBudgetReportView {
    pub quality_first: bool,
    pub max_prompt_chars: Option<usize>,
    pub estimated_prompt_chars: usize,
    pub budget_pressure: String,
    pub trimmed_item_count: usize,
    #[serde(default)]
    pub items: Vec<AssistantRunCodexContextBudgetItemView>,
}

impl From<AssistantRunCodexContextBudgetView> for ProviderShimContextBudgetReportView {
    fn from(value: AssistantRunCodexContextBudgetView) -> Self {
        Self {
            quality_first: value.quality_first,
            max_prompt_chars: value.max_prompt_chars,
            estimated_prompt_chars: value.estimated_prompt_chars,
            budget_pressure: value.budget_pressure,
            trimmed_item_count: value.trimmed_item_count,
            items: value.items,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderShimToolOutputBudgetView {
    pub largest_output_chars: usize,
    pub trimmed_output_count: usize,
    pub preserved_recent_output_count: usize,
    pub preserved_error_count: usize,
    pub preserved_evidence_ref_count: usize,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderShimLivenessEventView {
    pub event_type: String,
    pub status: String,
    pub retry_count: usize,
    pub action: Option<String>,
    pub occurred_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderShimObservabilitySnapshotView {
    pub schema_version: u32,
    pub health: ProviderShimHealthView,
    pub profile: ProviderShimProfileSnapshotView,
    pub usage_summary: ProviderShimUsageSummaryView,
    #[serde(default)]
    pub recent_usage_events: Vec<ProviderShimUsageEventView>,
    pub balance: ProviderShimBalanceView,
    pub debug_trace_status: ProviderShimDebugTraceStatusView,
    pub context_budget_report: Option<ProviderShimContextBudgetReportView>,
    pub tool_output_budget: ProviderShimToolOutputBudgetView,
    #[serde(default)]
    pub liveness_events: Vec<ProviderShimLivenessEventView>,
}

impl ProviderShimObservabilitySnapshotView {
    pub const SCHEMA_VERSION: u32 = 1;

    pub fn new(health: ProviderShimHealthView, profile: ProviderShimProfileSnapshotView) -> Self {
        Self {
            schema_version: Self::SCHEMA_VERSION,
            health,
            profile,
            usage_summary: ProviderShimUsageSummaryView::default(),
            recent_usage_events: Vec::new(),
            balance: ProviderShimBalanceView::default(),
            debug_trace_status: ProviderShimDebugTraceStatusView::default(),
            context_budget_report: None,
            tool_output_budget: ProviderShimToolOutputBudgetView::default(),
            liveness_events: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssistantRunCodexToolOutputPolicyView {
    pub max_total_chars: usize,
    pub max_item_chars: usize,
    pub preserve_recent_output_count: usize,
    pub preserve_error_fields: bool,
    pub preserve_evidence_refs: bool,
    pub preserve_media_timestamps: bool,
}

impl Default for AssistantRunCodexToolOutputPolicyView {
    fn default() -> Self {
        Self {
            max_total_chars: 80_000,
            max_item_chars: 16_000,
            preserve_recent_output_count: 3,
            preserve_error_fields: true,
            preserve_evidence_refs: true,
            preserve_media_timestamps: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AssistantRunCodexActionContractView {
    pub action_type: String,
    pub title: String,
    pub description: String,
    pub input_schema: Value,
    pub requires_v3_validation: bool,
    pub mutates_state: bool,
}

impl AssistantRunCodexActionContractView {
    pub fn new(
        action_type: impl Into<String>,
        title: impl Into<String>,
        description: impl Into<String>,
        input_schema: Value,
        mutates_state: bool,
    ) -> Self {
        Self {
            action_type: action_type.into(),
            title: title.into(),
            description: description.into(),
            input_schema,
            requires_v3_validation: true,
            mutates_state,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssistantRunCodexContextPackageView {
    pub schema_version: u32,
    pub assistant_run_id: AssistantRunId,
    pub local_thread_id: Option<String>,
    pub executor_transport: AssistantRunExecutorTransportView,
    pub user_prompt: String,
    #[serde(default)]
    pub messages: Vec<AssistantRunMessageView>,
    #[serde(default)]
    pub startup_briefing: Value,
    #[serde(default)]
    pub selected_scope: Value,
    #[serde(default)]
    pub inferred_scope_candidates: Vec<Value>,
    #[serde(default)]
    pub evidence_state: Value,
    #[serde(default)]
    pub supply_quality: Value,
    #[serde(default)]
    pub model_gateway: Value,
    #[serde(default)]
    pub hidden_memory_candidates: Vec<Value>,
    #[serde(default)]
    pub current_artifact: Option<Value>,
    #[serde(default)]
    pub available_actions: Vec<AssistantRunCodexActionContractView>,
    pub safety: AssistantRunCodexSafetyPolicyView,
    pub context_budget: AssistantRunCodexContextBudgetView,
    #[serde(default)]
    pub tool_output_policy: AssistantRunCodexToolOutputPolicyView,
}

impl AssistantRunCodexContextPackageView {
    pub const SCHEMA_VERSION: u32 = 1;

    pub fn new(assistant_run_id: AssistantRunId, user_prompt: impl Into<String>) -> Self {
        Self {
            schema_version: Self::SCHEMA_VERSION,
            assistant_run_id,
            local_thread_id: None,
            executor_transport: AssistantRunExecutorTransportView::CodexDryRun,
            user_prompt: user_prompt.into(),
            messages: Vec::new(),
            startup_briefing: Value::Null,
            selected_scope: Value::Null,
            inferred_scope_candidates: Vec::new(),
            evidence_state: Value::Null,
            supply_quality: Value::Null,
            model_gateway: Value::Null,
            hidden_memory_candidates: Vec::new(),
            current_artifact: None,
            available_actions: Vec::new(),
            safety: AssistantRunCodexSafetyPolicyView::default(),
            context_budget: AssistantRunCodexContextBudgetView::default(),
            tool_output_policy: AssistantRunCodexToolOutputPolicyView::default(),
        }
    }

    pub fn action_types(&self) -> Vec<String> {
        self.available_actions
            .iter()
            .map(|action| action.action_type.clone())
            .collect()
    }

    pub fn to_task_context(&self) -> serde_json::Result<Value> {
        serde_json::to_value(self)
    }
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthAuditEventView {
    pub id: AuthAuditEventId,
    pub event_name: String,
    pub outcome: AuthAuditOutcome,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub device_fingerprint: Option<String>,
    pub current_session: bool,
    pub details: Value,
    pub created_at: DateTime<Utc>,
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
    pub local_only: bool,
    #[serde(default)]
    pub local_thread_id: Option<String>,
    #[serde(default)]
    pub secret_binding_ids: Vec<SecretBindingId>,
    #[serde(default)]
    pub secret_fingerprint: Option<String>,
    #[serde(default)]
    pub secret_label: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateDatasetRequest {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lifecycle: Option<String>,
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
pub struct MediaTranscriptSegmentView {
    pub start_seconds: Option<f64>,
    pub end_seconds: Option<f64>,
    pub text: String,
    pub source: String,
    pub language: Option<String>,
    pub confidence: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MediaSceneView {
    pub start_seconds: Option<f64>,
    pub end_seconds: Option<f64>,
    pub representative_seconds: Option<f64>,
    pub summary: String,
    pub source: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MediaOcrSnippetView {
    pub timestamp_seconds: Option<f64>,
    pub text: String,
    pub source: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MediaProviderEvidenceView {
    pub provider: String,
    pub capability: String,
    pub status: String,
    pub supported: bool,
    pub detail: String,
    pub endpoint: Option<String>,
    pub model: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentMediaDetailView {
    pub document: DocumentSummary,
    pub media_kind: String,
    pub parse_status: String,
    pub transcript_segments: Vec<MediaTranscriptSegmentView>,
    pub scenes: Vec<MediaSceneView>,
    pub keyframe_ocr_snippets: Vec<MediaOcrSnippetView>,
    pub provider_evidence: Vec<MediaProviderEvidenceView>,
    pub raw_media_metadata: Value,
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
    Archived,
}

impl DocumentLifecycleView {
    pub fn from_domain(value: domain_model::DocumentLifecycle) -> Self {
        match value {
            domain_model::DocumentLifecycle::Received => Self::Received,
            domain_model::DocumentLifecycle::Extracted => Self::Extracted,
            domain_model::DocumentLifecycle::Indexed => Self::Indexed,
            domain_model::DocumentLifecycle::Failed => Self::Failed,
            domain_model::DocumentLifecycle::Archived => Self::Archived,
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "received" => Some(Self::Received),
            "extracted" => Some(Self::Extracted),
            "indexed" => Some(Self::Indexed),
            "failed" => Some(Self::Failed),
            "archived" => Some(Self::Archived),
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
pub struct UpdateDocumentRequest {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub lifecycle: Option<String>,
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
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub local_thread_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateChatSessionResponse {
    pub chat_session: ChatSessionView,
    pub workflow_execution: WorkflowExecutionView,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateChatSessionRequest {
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateChatSessionResponse {
    pub chat_session: ChatSessionView,
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
    #[serde(default)]
    pub diagnostics: Value,
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
    #[serde(default)]
    pub diagnostics: Value,
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
    #[serde(default)]
    pub diagnostics: Value,
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
pub struct SubmitHtmlArtifactEventRequest {
    #[serde(default)]
    pub assistant_run_id: Option<String>,
    #[serde(default)]
    pub local_thread_id: Option<String>,
    pub event_type: String,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubmitHtmlArtifactEventResponse {
    pub accepted: bool,
    pub artifact: HtmlArtifactManifestView,
    pub run: AssistantRunView,
    pub event: AssistantRunEventView,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateStaticPageDraftRequest {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default, alias = "templateReferenceId")]
    pub template_reference_id: Option<String>,
    #[serde(default)]
    pub selected_scope: Option<Value>,
    #[serde(default)]
    pub visibility_snapshot: Option<Value>,
    #[serde(default)]
    pub source_refs: Value,
    #[serde(default)]
    pub draft_payload: Value,
}

pub const STATIC_PAGE_DEFAULT_CHART_RUNTIME: &str = "deterministic";
pub const STATIC_PAGE_ADVANCED_CHART_RUNTIME: &str = "echarts";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StaticPageChartRuntimeView {
    Deterministic,
    Echarts,
}

impl StaticPageChartRuntimeView {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Deterministic => STATIC_PAGE_DEFAULT_CHART_RUNTIME,
            Self::Echarts => STATIC_PAGE_ADVANCED_CHART_RUNTIME,
        }
    }
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
    #[serde(default)]
    pub direct_html: bool,
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
    fn external_bot_message_contract_uses_channel_safe_wire_shape() {
        let received_at = Utc::now();
        let message: ExternalBotMessageView = serde_json::from_value(json!({
            "platform": "we_com",
            "tenant_external_id": "corp-001",
            "bot_external_id": "bot-qa",
            "conversation_external_id": "chat-risk-review",
            "sender_external_id": "user-a",
            "message_external_id": "msg-001",
            "message_type": "text",
            "text": "本周订单风险有哪些？",
            "attachment_refs": [
                {
                    "attachment_external_id": "file-001",
                    "filename": "orders.xlsx",
                    "content_type": "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                    "size_bytes": 2048,
                    "download_url_redacted": "https://wecom.example/download/[redacted]"
                }
            ],
            "idempotency_key": "wecom:corp-001:msg-001",
            "received_at": received_at
        }))
        .expect("external bot message should deserialize");

        assert_eq!(message.platform, ExternalChannelPlatformView::WeCom);
        assert_eq!(message.message_type, ExternalMessageTypeView::Text);
        assert!(message.mention_external_user_ids.is_empty());
        assert_eq!(
            message.attachment_refs[0].filename.as_deref(),
            Some("orders.xlsx")
        );

        let encoded = serde_json::to_value(&message).expect("message should serialize");
        assert_eq!(encoded["platform"], json!("we_com"));
        assert_eq!(encoded["message_type"], json!("text"));
        assert_eq!(
            encoded["attachment_refs"][0]["download_url_redacted"],
            json!("https://wecom.example/download/[redacted]")
        );
        assert!(encoded.get("thread_external_id").is_none());
    }

    #[test]
    fn external_principal_and_acl_snapshot_roundtrip_permission_shape() {
        let captured_at = Utc::now();
        let principal = ExternalPrincipalView {
            tenant_id: "tenant-001".to_string(),
            platform: ExternalChannelPlatformView::Feishu,
            external_user_id: "ou-user-a".to_string(),
            external_department_ids: vec!["dept-sales".to_string()],
            external_group_ids: vec!["group-risk".to_string()],
            external_role_ids: vec!["role-manager".to_string()],
            v3_user_id: Some("v3-user-a".to_string()),
            trust_level: ExternalPrincipalTrustLevelView::Manager,
            is_disabled: false,
        };
        let acl = ExternalDocumentAclSnapshotView {
            source_id: "src-feishu-docs".to_string(),
            document_external_id: "doc-risk-001".to_string(),
            revision_external_id: Some("rev-7".to_string()),
            allowed_user_external_ids: vec!["ou-user-a".to_string()],
            allowed_department_external_ids: vec!["dept-sales".to_string()],
            allowed_group_external_ids: vec!["group-risk".to_string()],
            allowed_role_external_ids: vec!["role-manager".to_string()],
            denied_user_external_ids: vec!["ou-user-blocked".to_string()],
            denied_department_external_ids: Vec::new(),
            denied_group_external_ids: Vec::new(),
            denied_role_external_ids: Vec::new(),
            acl_hash: Some("acl-hash-001".to_string()),
            captured_at,
        };

        let encoded_principal =
            serde_json::to_value(&principal).expect("principal should serialize");
        let encoded_acl = serde_json::to_value(&acl).expect("acl should serialize");

        assert_eq!(encoded_principal["platform"], json!("feishu"));
        assert_eq!(encoded_principal["trust_level"], json!("manager"));
        assert_eq!(encoded_acl["revision_external_id"], json!("rev-7"));
        assert_eq!(
            encoded_acl["denied_user_external_ids"],
            json!(["ou-user-blocked"])
        );

        let decoded_acl: ExternalDocumentAclSnapshotView =
            serde_json::from_value(encoded_acl).expect("acl should deserialize");
        assert_eq!(decoded_acl.captured_at, captured_at);
        assert_eq!(decoded_acl.acl_hash.as_deref(), Some("acl-hash-001"));
    }

    #[test]
    fn third_party_source_and_action_intent_keep_runtime_values_redacted() {
        let now = Utc::now();
        let source = ThirdPartyKnowledgeSourceView {
            source_id: "src-third-docs".to_string(),
            tenant_id: "tenant-001".to_string(),
            connector_kind: ThirdPartyConnectorKindView::Document,
            base_url_redacted: "https://docs.example.com/[tenant]/".to_string(),
            sync_mode: ThirdPartySyncModeView::Hybrid,
            permission_mode: ThirdPartyPermissionModeView::SourceAclSnapshot,
            supported_capabilities: vec![
                "documents.list".to_string(),
                "documents.fetch_acl".to_string(),
            ],
            health: IntegrationHealthView {
                status: IntegrationHealthStatusView::Degraded,
                checked_at: now,
                last_success_at: Some(now),
                last_failure_at: Some(now),
                failure_kind: Some("rate_limited".to_string()),
                message: Some("Retry scheduled; token value is not exposed".to_string()),
            },
        };
        let requester = ExternalPrincipalView {
            tenant_id: "tenant-001".to_string(),
            platform: ExternalChannelPlatformView::GenericChat,
            external_user_id: "user-portal-a".to_string(),
            external_department_ids: Vec::new(),
            external_group_ids: vec!["support".to_string()],
            external_role_ids: Vec::new(),
            v3_user_id: None,
            trust_level: ExternalPrincipalTrustLevelView::Employee,
            is_disabled: false,
        };
        let intent = ExternalActionIntentView {
            action_id: "act-001".to_string(),
            tenant_id: "tenant-001".to_string(),
            requester,
            risk_level: ExternalActionRiskLevelView::HighRiskWrite,
            target_system: "ticketing".to_string(),
            action_type: "ticket.update_priority".to_string(),
            arguments_redacted: json!({
                "ticket_id": "T-1001",
                "priority": "high",
                "auth_token": "[redacted]"
            }),
            source_evidence_refs: vec!["retrieval:chunk-001".to_string()],
            requires_confirmation: true,
            idempotency_key: "external-action:act-001".to_string(),
            created_at: now,
        };

        let encoded_source = serde_json::to_value(&source).expect("source should serialize");
        let encoded_intent = serde_json::to_value(&intent).expect("intent should serialize");
        let serialized_intent = encoded_intent.to_string();

        assert_eq!(encoded_source["connector_kind"], json!("document"));
        assert_eq!(encoded_source["sync_mode"], json!("hybrid"));
        assert_eq!(
            encoded_source["permission_mode"],
            json!("source_acl_snapshot")
        );
        assert_eq!(encoded_source["health"]["status"], json!("degraded"));
        assert_eq!(encoded_intent["risk_level"], json!("high_risk_write"));
        assert_eq!(encoded_intent["requires_confirmation"], json!(true));
        assert!(encoded_intent.get("arguments").is_none());
        assert!(serialized_intent.contains("arguments_redacted"));
        assert!(!serialized_intent.contains("sk-"));
        assert!(!serialized_intent.contains("raw_secret"));

        let decoded_intent: ExternalActionIntentView =
            serde_json::from_value(encoded_intent).expect("intent should deserialize");
        assert_eq!(
            decoded_intent.requester.platform,
            ExternalChannelPlatformView::GenericChat
        );
        assert_eq!(
            decoded_intent.risk_level,
            ExternalActionRiskLevelView::HighRiskWrite
        );
    }

    #[test]
    fn external_action_policy_serializes_risk_and_confirmation_modes() {
        let policies = vec![
            ExternalActionPolicyView {
                capability: ExternalActionCapabilityView::ArtifactStatus,
                action_type: "external_artifact.status".to_string(),
                target_system: "third_party_artifact_api".to_string(),
                risk_level: ExternalActionRiskLevelView::ReadOnly,
                requires_confirmation: false,
                confirmation_mode: ExternalActionConfirmationModeView::NotRequired,
            },
            ExternalActionPolicyView {
                capability: ExternalActionCapabilityView::ArtifactPublish,
                action_type: "external_artifact.publish".to_string(),
                target_system: "third_party_artifact_api".to_string(),
                risk_level: ExternalActionRiskLevelView::LowRiskWrite,
                requires_confirmation: false,
                confirmation_mode: ExternalActionConfirmationModeView::NotRequired,
            },
            ExternalActionPolicyView {
                capability: ExternalActionCapabilityView::ArtifactRevoke,
                action_type: "external_artifact.revoke".to_string(),
                target_system: "third_party_artifact_api".to_string(),
                risk_level: ExternalActionRiskLevelView::HighRiskWrite,
                requires_confirmation: true,
                confirmation_mode: ExternalActionConfirmationModeView::OriginalChannel,
            },
            ExternalActionPolicyView {
                capability: ExternalActionCapabilityView::BusinessAction,
                action_type: "external_business_action.invoke".to_string(),
                target_system: "third_party_business_api".to_string(),
                risk_level: ExternalActionRiskLevelView::CrossSystem,
                requires_confirmation: true,
                confirmation_mode: ExternalActionConfirmationModeView::TrustedCustomerPage,
            },
        ];

        let encoded = serde_json::to_value(&policies).expect("policies should serialize");

        assert_eq!(encoded[0]["capability"], json!("artifact_status"));
        assert_eq!(encoded[0]["risk_level"], json!("read_only"));
        assert_eq!(encoded[0]["confirmation_mode"], json!("not_required"));
        assert_eq!(encoded[1]["risk_level"], json!("low_risk_write"));
        assert_eq!(encoded[2]["requires_confirmation"], json!(true));
        assert_eq!(encoded[2]["confirmation_mode"], json!("original_channel"));
        assert_eq!(encoded[3]["risk_level"], json!("cross_system"));
        assert_eq!(
            encoded[3]["confirmation_mode"],
            json!("trusted_customer_page")
        );

        let decoded: Vec<ExternalActionPolicyView> =
            serde_json::from_value(encoded).expect("policies should deserialize");
        assert_eq!(decoded, policies);
    }

    #[test]
    fn external_action_confirmation_contract_serializes_decision_and_action_id() {
        let run_id = AssistantRunId::new();
        let confirmed_at = Utc::now();
        let request = ExternalActionConfirmationRequestView {
            assistant_run_id: run_id,
            action_id: Some("external-action-001".to_string()),
            confirmation_external_id: "confirm-001".to_string(),
            sender_external_user_id: "user-10001".to_string(),
            decision: ExternalActionConfirmationDecisionView::Approved,
            comment: Some("确认提交。".to_string()),
            idempotency_key: "generic_chat:tenant-ext-001:confirm-001".to_string(),
            confirmed_at,
        };

        let encoded = serde_json::to_value(&request).expect("request should serialize");

        assert_eq!(encoded["assistant_run_id"], json!(run_id.to_string()));
        assert_eq!(encoded["action_id"], json!("external-action-001"));
        assert_eq!(encoded["decision"], json!("approved"));
        assert_eq!(encoded["confirmed_at"], json!(confirmed_at));

        let decoded: ExternalActionConfirmationRequestView =
            serde_json::from_value(encoded).expect("request should deserialize");
        assert_eq!(decoded, request);

        let response = ExternalActionConfirmationResponseView {
            accepted: true,
            assistant_run_id: run_id,
            action_id: "external-action-001".to_string(),
            confirmation_state: "confirmed".to_string(),
            idempotency_key: "generic_chat:tenant-ext-001:confirm-001".to_string(),
        };
        let encoded_response = serde_json::to_value(&response).expect("response should serialize");
        assert_eq!(encoded_response["confirmation_state"], json!("confirmed"));
    }

    #[test]
    fn external_action_result_callback_view_serializes_redacted_envelope() {
        let completed_at = Utc::now();
        let request = ExternalActionResultCallbackRequestView {
            external_request_id: Some("gw-req-001".to_string()),
            status: "succeeded".to_string(),
            idempotency_key: "generic_chat:tenant-ext-001:result-001".to_string(),
            completed_at: Some(completed_at),
            code: Some("OK".to_string()),
            message: Some("done".to_string()),
            result: Some(json!({
                "artifact_id": "artifact-001"
            })),
        };

        let encoded = serde_json::to_value(&request).expect("request should serialize");

        assert_eq!(encoded["external_request_id"], json!("gw-req-001"));
        assert_eq!(encoded["status"], json!("succeeded"));
        assert_eq!(encoded["completed_at"], json!(completed_at));

        let decoded: ExternalActionResultCallbackRequestView =
            serde_json::from_value(encoded).expect("request should deserialize");
        assert_eq!(decoded, request);

        let response = ExternalActionResultCallbackResponseView {
            accepted: true,
            action_id: "external-action-001".to_string(),
            external_request_id: Some("gw-req-001".to_string()),
            status: "succeeded".to_string(),
            idempotency_key: "generic_chat:tenant-ext-001:result-001".to_string(),
        };
        let encoded_response = serde_json::to_value(&response).expect("response should serialize");
        assert_eq!(encoded_response["accepted"], json!(true));
        assert_eq!(encoded_response["status"], json!("succeeded"));
    }

    #[test]
    fn external_channel_event_response_uses_task_status_reply_envelope() {
        let run_id = AssistantRunId::new();
        let response = ExternalChannelEventResponse {
            accepted: true,
            assistant_run_id: Some(run_id),
            idempotency_key: "generic:tenant:msg-001".to_string(),
            reply: ExternalBotReplyView {
                target_conversation_external_id: "chat-001".to_string(),
                reply_type: ExternalBotReplyTypeView::TaskStatus,
                text: None,
                card: None,
                artifact_links: Vec::new(),
                task_status: Some("accepted".to_string()),
                requires_confirmation: false,
                action_id: None,
                confirmation_id: None,
            },
        };

        let encoded = serde_json::to_value(&response).expect("response should serialize");

        assert_eq!(encoded["accepted"], json!(true));
        assert_eq!(encoded["assistant_run_id"], json!(run_id.to_string()));
        assert_eq!(encoded["reply"]["reply_type"], json!("task_status"));
        assert_eq!(encoded["reply"]["task_status"], json!("accepted"));
        assert!(encoded["reply"].get("text").is_none());

        let decoded: ExternalChannelEventResponse =
            serde_json::from_value(encoded).expect("response should deserialize");
        assert_eq!(decoded.assistant_run_id, Some(run_id));
        assert_eq!(
            decoded.reply.reply_type,
            ExternalBotReplyTypeView::TaskStatus
        );
    }

    #[test]
    fn external_source_sync_request_defaults_optional_connector_context() {
        let dataset_id = DatasetId::new();
        let request: CreateExternalSourceSyncRequest = serde_json::from_value(json!({
            "sync_kind": "full",
            "dataset_id": dataset_id.to_string(),
            "checkpoint": {
                "cursor": "page-2"
            }
        }))
        .expect("external source sync request should deserialize");

        assert_eq!(request.sync_kind.as_deref(), Some("full"));
        assert_eq!(request.dataset_id, Some(dataset_id));
        assert_eq!(request.checkpoint["cursor"], json!("page-2"));
        assert_eq!(request.connector_context, Value::Null);

        let response = CreateExternalSourceSyncResponse {
            accepted: true,
            source_id: "src-third-docs".to_string(),
            sync_run_id: "sync-run-001".to_string(),
            sync_kind: "full".to_string(),
            status: "running".to_string(),
            workflow_execution: WorkflowExecutionView {
                id: WorkflowExecutionId::new(),
                kind: domain_model::WorkflowKind::ExternalSourceSync,
                status: domain_model::WorkflowStatus::Running,
                stage: "sync_users".to_string(),
                updated_at: Utc::now(),
            },
            enqueued_tasks: Vec::new(),
        };
        let encoded = serde_json::to_value(&response).expect("response should serialize");

        assert_eq!(encoded["accepted"], json!(true));
        assert_eq!(encoded["source_id"], json!("src-third-docs"));
        assert_eq!(
            encoded["workflow_execution"]["kind"],
            json!("ExternalSourceSync")
        );
        assert!(encoded.get("enqueued_tasks").is_none());
    }

    #[test]
    fn external_integration_control_response_omits_empty_workflow_fields() {
        let request: ExternalIntegrationControlRequest = serde_json::from_value(json!({
            "reason": "operator_retry",
            "sync_kind": "incremental"
        }))
        .expect("control request should deserialize");
        assert_eq!(request.reason.as_deref(), Some("operator_retry"));
        assert_eq!(request.sync_kind.as_deref(), Some("incremental"));

        let response = ExternalIntegrationControlResponse {
            accepted: true,
            integration_id: "src-docs".to_string(),
            integration_kind: "source".to_string(),
            action: "retry".to_string(),
            status: "running".to_string(),
            message: "external source retry enqueued".to_string(),
            affected_action_count: 0,
            sync_run_id: Some("sync-run-001".to_string()),
            workflow_execution: None,
            enqueued_tasks: Vec::new(),
        };
        let encoded = serde_json::to_value(response).expect("response should serialize");

        assert_eq!(encoded["accepted"], json!(true));
        assert_eq!(encoded["integration_kind"], json!("source"));
        assert_eq!(encoded["sync_run_id"], json!("sync-run-001"));
        assert!(encoded.get("workflow_execution").is_none());
        assert!(encoded.get("enqueued_tasks").is_none());
    }

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
    fn codex_host_task_request_context_uses_isolated_memory_space() {
        let assistant_run_id = AssistantRunId::new();
        let execution_id = WorkflowExecutionId::new();
        let memory_policy =
            CodexHostTaskMemoryPolicyView::task_scoped(assistant_run_id, execution_id);
        let expected_memory_space_id = format!("codex-host-task:{assistant_run_id}:{execution_id}");
        let request = CodexHostTaskRequestView {
            assistant_run_id,
            capability: "inspect_project".to_string(),
            task: Some("Summarize repository shape".to_string()),
            local_thread_id: Some("thread-a".to_string()),
            task_memory_policy: memory_policy,
            safety: CodexHostTaskSafetyPolicyView::default(),
        };

        let context = request.to_workflow_context();

        assert_eq!(
            context["assistant_run_id"],
            json!(assistant_run_id.to_string())
        );
        assert_eq!(context["capability"], json!("inspect_project"));
        assert_eq!(context["task_memory_policy"]["isolated"], json!(true));
        assert_eq!(
            context["task_memory_policy"]["promote_summary_to_conversation"],
            json!(false)
        );
        assert_eq!(
            context["task_memory_policy"]["memory_space_id"],
            json!(expected_memory_space_id)
        );
        assert_eq!(
            context["task_memory_space_id"],
            context["task_memory_policy"]["memory_space_id"]
        );
        assert_eq!(context["safety"]["user_cli_flags_allowed"], json!(false));
        assert_eq!(context["safety"]["secrets_in_prompt_allowed"], json!(false));
        assert_eq!(context["safety"]["raw_logs_require_redaction"], json!(true));
        assert_eq!(
            context["safety"]["real_codex_exec_requires_host_allowlist"],
            json!(true)
        );
    }

    #[test]
    fn assistant_run_codex_context_package_keeps_v3_as_control_plane() {
        let assistant_run_id = AssistantRunId::new();
        let mut package = AssistantRunCodexContextPackageView::new(
            assistant_run_id,
            "根据当前数据生成一页客户流失风险静态页",
        );
        package.local_thread_id = Some("browser-thread-1".to_string());
        package.messages = vec![AssistantRunMessageView {
            role: ChatMessageRole::User,
            content: "继续刚才那版，把风险模块放大一点".to_string(),
        }];
        package.startup_briefing = json!({
            "product": "AI数据智能助手",
            "capabilities": ["static_page_plan", "retrieval"]
        });
        package.selected_scope = json!({
            "datasets": [{"id": "dataset-1", "title": "客户服务记录"}]
        });
        package.evidence_state = json!({
            "mode": "supplied_by_v3",
            "items": [{"id": "evidence-1", "source": "chunk"}],
            "supply_quality": {
                "status": "grounded",
                "citationLocatorCount": 1
            }
        });
        package.supply_quality = package.evidence_state["supply_quality"].clone();
        package.model_gateway = json!({
            "lane": "codex_conversation",
            "selected_model": {
                "mode": "provider",
                "provider": "minimax",
                "model": "MiniMax-M2.7"
            },
            "profile": {
                "profile_id": "minimax-codex-shadow",
                "wire_api": "codex_compatible_shim",
                "capabilities": ["chat", "json", "tool_calling"]
            }
        });
        assert_eq!(
            AssistantRunCodexContextPackageView::new(assistant_run_id, "默认包").supply_quality,
            Value::Null
        );
        assert_eq!(
            AssistantRunCodexContextPackageView::new(assistant_run_id, "默认包").model_gateway,
            Value::Null
        );
        package.current_artifact = Some(json!({
            "kind": "static_page_draft",
            "draft_id": "draft-1",
            "preview_status": "stale"
        }));
        package.available_actions = vec![
            AssistantRunCodexActionContractView::new(
                "update_static_page_module",
                "更新静态页模块",
                "只能更新当前可见静态页草稿中的模块",
                json!({
                    "type": "object",
                    "properties": {
                        "draft_id": {"type": "string"},
                        "module_id": {"type": "string"}
                    },
                    "required": ["draft_id", "module_id"]
                }),
                true,
            ),
            AssistantRunCodexActionContractView::new(
                "retrieve_dataset_detail",
                "检索数据集详情",
                "从V3已经判定可见的数据集范围内补充证据",
                json!({"type": "object"}),
                false,
            ),
        ];
        package.context_budget = AssistantRunCodexContextBudgetView {
            included_message_count: 1,
            selected_dataset_count: 1,
            evidence_item_count: 1,
            estimated_prompt_chars: 3200,
            budget_pressure: "attention".to_string(),
            artifact_state_chars: package
                .current_artifact
                .as_ref()
                .map(|value| value.to_string().len())
                .unwrap_or_default(),
            items: vec![AssistantRunCodexContextBudgetItemView::new(
                "retrieval_evidence",
                1,
                2800,
                Some(3000),
                0,
                Some("test evidence budget".to_string()),
            )],
            ..AssistantRunCodexContextBudgetView::default()
        };

        let context = package
            .to_task_context()
            .expect("codex context package should serialize");

        assert_eq!(context["schema_version"], json!(1));
        assert_eq!(
            context["assistant_run_id"],
            json!(assistant_run_id.to_string())
        );
        assert_eq!(context["executor_transport"], json!("codex_dry_run"));
        assert_eq!(context["safety"]["v3_validates_all_actions"], json!(true));
        assert_eq!(
            context["safety"]["direct_database_access_allowed"],
            json!(false)
        );
        assert_eq!(
            context["safety"]["direct_queue_access_allowed"],
            json!(false)
        );
        assert_eq!(
            context["safety"]["static_page_state_machine_owned_by_v3"],
            json!(true)
        );
        assert_eq!(
            context["available_actions"][0]["requires_v3_validation"],
            json!(true)
        );
        assert_eq!(
            package.action_types(),
            vec![
                "update_static_page_module".to_string(),
                "retrieve_dataset_detail".to_string()
            ]
        );
        assert_eq!(context["context_budget"]["quality_first"], json!(true));
        assert_eq!(
            context["context_budget"]["estimated_prompt_chars"],
            json!(3200)
        );
        assert_eq!(
            context["context_budget"]["budget_pressure"],
            json!("attention")
        );
        assert_eq!(
            context["context_budget"]["items"][0]["category"],
            json!("retrieval_evidence")
        );
        assert_eq!(
            context["context_budget"]["items"][0]["pressure"],
            json!("attention")
        );
        assert_eq!(
            context["context_budget"]["selected_dataset_count"],
            json!(1)
        );
        assert_eq!(context["supply_quality"]["status"], json!("grounded"));
        assert_eq!(context["supply_quality"]["citationLocatorCount"], json!(1));
        assert_eq!(
            context["model_gateway"]["lane"],
            json!("codex_conversation")
        );
        assert_eq!(
            context["model_gateway"]["selected_model"]["provider"],
            json!("minimax")
        );
        assert_eq!(
            context["model_gateway"]["profile"]["wire_api"],
            json!("codex_compatible_shim")
        );
        assert_eq!(
            context["tool_output_policy"]["preserve_recent_output_count"],
            json!(3)
        );
        assert_eq!(
            context["tool_output_policy"]["preserve_evidence_refs"],
            json!(true)
        );
    }

    #[test]
    fn provider_shim_observability_snapshot_serializes_redacted_runtime_status() {
        let profile = ProviderShimProfileSnapshotView {
            profile_id: "minimax-private-experiment".to_string(),
            provider_id: "minimax".to_string(),
            model_id: "MiniMax-M2.7".to_string(),
            wire_api: "codex_compatible_shim".to_string(),
            endpoint_scope: "local_private".to_string(),
            base_url_configured: true,
            api_path: Some("/v1/responses".to_string()),
            auth_env_key_name: Some("MINIMAX_API_KEY".to_string()),
            auth_configured: true,
            timeout_ms: Some(45_000),
            capabilities: vec![
                "chat".to_string(),
                "json_mode".to_string(),
                "codex_compatible".to_string(),
            ],
            rate_limit: ProviderShimRateLimitHintsView {
                requests_per_minute: Some(60),
                tokens_per_minute: Some(120_000),
                concurrent_requests: Some(2),
            },
            cost: ProviderShimCostHintsView {
                input_microusd_per_million_tokens: Some(200_000),
                output_microusd_per_million_tokens: Some(1_000_000),
                currency: Some("USD".to_string()),
            },
            redaction: ProviderShimRedactionPolicyView {
                redact_provider_errors: true,
                redact_request_payloads: true,
                redact_response_payloads: true,
                max_error_chars: 900,
            },
        };
        let mut snapshot = ProviderShimObservabilitySnapshotView::new(
            ProviderShimHealthView {
                status: ProviderShimHealthStatusView::Healthy,
                process_reachable: true,
                upstream_reachable: true,
                checked_at: Some(Utc::now()),
                message: Some("local shim ready".to_string()),
            },
            profile,
        );
        snapshot.usage_summary = ProviderShimUsageSummaryView {
            request_count: 2,
            failed_request_count: 1,
            input_tokens: 1200,
            output_tokens: 340,
            total_tokens: 1540,
            last_request_id: Some("req-redacted".to_string()),
        };
        snapshot.context_budget_report = Some(
            AssistantRunCodexContextBudgetView {
                estimated_prompt_chars: 8000,
                budget_pressure: "attention".to_string(),
                trimmed_item_count: 1,
                items: vec![AssistantRunCodexContextBudgetItemView::new(
                    "tool_outputs",
                    3,
                    6000,
                    Some(7000),
                    1,
                    Some("bounded tool outputs".to_string()),
                )],
                ..AssistantRunCodexContextBudgetView::default()
            }
            .into(),
        );
        snapshot.liveness_events = vec![ProviderShimLivenessEventView {
            event_type: "tool_call_loop".to_string(),
            status: "continued".to_string(),
            retry_count: 1,
            action: Some("continue_after_incomplete_tool_call".to_string()),
            occurred_at: Some(Utc::now()),
            note: Some("prompt payload omitted".to_string()),
        }];

        let encoded = serde_json::to_value(&snapshot).expect("snapshot should serialize");
        let serialized = encoded.to_string();

        assert_eq!(encoded["schema_version"], json!(1));
        assert_eq!(encoded["health"]["status"], json!("healthy"));
        assert_eq!(encoded["profile"]["endpoint_scope"], json!("local_private"));
        assert_eq!(
            encoded["context_budget_report"]["items"][0]["category"],
            json!("tool_outputs")
        );
        assert_eq!(encoded["liveness_events"][0]["retry_count"], json!(1));
        assert!(!serialized.contains("sk-"));
        assert!(!serialized.contains("raw prompt"));
        assert!(!serialized.contains("api_key"));
    }

    #[test]
    fn html_artifact_manifest_uses_safe_snake_case_contract() {
        let manifest = HtmlArtifactManifestView::codex_execution_report(
            "run-1",
            "Codex Host 执行报告",
            "plan_only planned",
            json!({
                "mode": "plan_only",
                "status": "planned",
                "summary": "safe report"
            }),
        );

        let encoded = serde_json::to_value(&manifest).expect("manifest should serialize");

        assert_eq!(encoded["kind"], json!("html_artifact"));
        assert_eq!(encoded["version"], json!(1));
        assert_eq!(encoded["source_type"], json!("codex_host"));
        assert_eq!(encoded["template_id"], json!("codex_execution_report"));
        assert_eq!(encoded["interaction_mode"], json!("read_only"));
        assert_eq!(encoded["owner_scope"]["type"], json!("assistant_run"));
        assert_eq!(encoded["owner_scope"]["id"], json!("run-1"));
        assert_eq!(encoded["provenance"]["producer"], json!("codex-host-agent"));
        assert_eq!(encoded["payload"]["mode"], json!("plan_only"));

        let decoded: HtmlArtifactManifestView =
            serde_json::from_value(encoded).expect("manifest should deserialize");
        assert_eq!(
            decoded.template_id,
            HtmlArtifactTemplateIdView::CodexExecutionReport
        );
        assert_eq!(decoded.source_type, HtmlArtifactSourceTypeView::CodexHost);
    }

    #[test]
    fn video_extraction_request_context_uses_stable_wire_shape() {
        let assistant_run_id = AssistantRunId::new();
        let dataset_id = DatasetId::new();
        let document_id = DocumentId::new();
        let request = VideoExtractionRequestView {
            assistant_run_id: Some(assistant_run_id),
            local_thread_id: Some("browser-thread-a".to_string()),
            source: VideoExtractionSourceRefView {
                source_type: VideoExtractionSourceKindView::PublicPageResolvableVideo,
                document_id: Some(document_id),
                source_url: Some("https://example.com/video.mp4".to_string()),
                source_page_url: Some("https://example.com/watch".to_string()),
                title_hint: Some("Demo Video".to_string()),
            },
            target_dataset_id: Some(dataset_id),
            requested_outputs: vec![
                VideoExtractionArtifactKindView::TranscriptText,
                VideoExtractionArtifactKindView::Pptx,
                VideoExtractionArtifactKindView::HtmlSummary,
            ],
            background_only: true,
        };

        let encoded = serde_json::to_value(&request).expect("request should serialize");

        assert_eq!(
            encoded["source"]["source_type"],
            json!("public_page_resolvable_video")
        );
        assert_eq!(encoded["requested_outputs"][1], json!("pptx"));
        assert_eq!(encoded["background_only"], json!(true));

        let context = request.to_workflow_context();
        assert_eq!(context["kind"], json!("video_extraction_workflow"));
        assert_eq!(
            context["assistant_run_id"],
            json!(assistant_run_id.to_string())
        );
        assert_eq!(context["target_dataset_id"], json!(dataset_id.to_string()));
        assert_eq!(context["queue"], json!("media"));
        assert_eq!(context["task_key"], json!("resolve_video_source"));

        let decoded: VideoExtractionRequestView =
            serde_json::from_value(encoded).expect("request should deserialize");
        assert_eq!(
            decoded.source.source_type,
            VideoExtractionSourceKindView::PublicPageResolvableVideo
        );
        assert_eq!(
            decoded.requested_outputs,
            vec![
                VideoExtractionArtifactKindView::TranscriptText,
                VideoExtractionArtifactKindView::Pptx,
                VideoExtractionArtifactKindView::HtmlSummary,
            ]
        );
    }

    #[test]
    fn video_extraction_state_and_artifacts_use_workflow_stage_names() {
        let state = VideoExtractionStateView {
            workflow_execution_id: WorkflowExecutionId::new(),
            stage: VideoExtractionStageView::ExtractingPpt,
            status: WorkflowStatus::Running,
            source: VideoExtractionSourceRefView {
                source_type: VideoExtractionSourceKindView::UploadedVideoFile,
                document_id: Some(DocumentId::new()),
                source_url: None,
                source_page_url: None,
                title_hint: Some("Uploaded Video".to_string()),
            },
            resolved_asset: Some(VideoExtractionResolvedAssetRefView {
                document_id: DocumentId::new(),
                dataset_id: DatasetId::new(),
                title: "Uploaded Video".to_string(),
                content_type: "video/mp4".to_string(),
                asset_state: "parsed".to_string(),
                workflow_execution_id: Some(WorkflowExecutionId::new()),
            }),
            artifacts: vec![VideoExtractionArtifactRefView {
                artifact_kind: VideoExtractionArtifactKindView::Markdown,
                artifact_id: "video-artifact-md-1".to_string(),
                title: "原文和 PPT 大纲".to_string(),
                format: "text/markdown".to_string(),
                uri: Some("artifact://video-artifact-md-1".to_string()),
                html_artifact_id: None,
            }],
            message: Some("extracting PPT candidates".to_string()),
            updated_at: Utc::now(),
        };

        let encoded = serde_json::to_value(&state).expect("state should serialize");

        assert_eq!(encoded["stage"], json!("extracting_ppt"));
        assert_eq!(encoded["artifacts"][0]["artifact_kind"], json!("markdown"));
        assert_eq!(
            VideoExtractionStageView::from_stage("extracting_ppt"),
            Some(VideoExtractionStageView::ExtractingPpt)
        );
        assert_eq!(
            VideoExtractionStageView::ExtractingPpt.as_str(),
            "extracting_ppt"
        );
    }

    #[test]
    fn html_artifact_report_render_summary_uses_safe_wire_shape() {
        let manifest = HtmlArtifactManifestView {
            kind: "html_artifact".to_string(),
            version: 1,
            id: "html-report-render-output-1".to_string(),
            title: "Quarterly Report · Render Summary".to_string(),
            source_type: HtmlArtifactSourceTypeView::Report,
            template_id: HtmlArtifactTemplateIdView::ReportRenderSummary,
            owner_scope: HtmlArtifactOwnerScopeView {
                scope_type: "report_render_output".to_string(),
                id: "output-1".to_string(),
            },
            data_refs: vec![HtmlArtifactDataRefView {
                kind: "report_plan".to_string(),
                id: "plan-1".to_string(),
                label: "Report Plan".to_string(),
            }],
            provenance: HtmlArtifactProvenanceView {
                producer: "v3-report-runtime".to_string(),
                reason: "report render output summary".to_string(),
                source_run_id: None,
            },
            interaction_mode: HtmlArtifactInteractionModeView::ReadOnly,
            created_at: Utc::now(),
            payload: json!({
                "surface": "pc",
                "status": "rendered",
                "publishable": true
            }),
        };

        let encoded = serde_json::to_value(&manifest).expect("manifest should serialize");

        assert_eq!(encoded["source_type"], json!("report"));
        assert_eq!(encoded["template_id"], json!("report_render_summary"));
        assert_eq!(
            encoded["owner_scope"]["type"],
            json!("report_render_output")
        );

        let decoded: HtmlArtifactManifestView =
            serde_json::from_value(encoded).expect("manifest should deserialize");
        assert_eq!(
            decoded.template_id,
            HtmlArtifactTemplateIdView::ReportRenderSummary
        );
        assert_eq!(decoded.source_type, HtmlArtifactSourceTypeView::Report);
    }

    #[test]
    fn html_artifact_video_extraction_summary_uses_safe_wire_shape() {
        let manifest = HtmlArtifactManifestView {
            kind: "html_artifact".to_string(),
            version: 1,
            id: "html-video-extraction-doc-1".to_string(),
            title: "Video · Extraction Summary".to_string(),
            source_type: HtmlArtifactSourceTypeView::VideoExtraction,
            template_id: HtmlArtifactTemplateIdView::VideoExtractionSummary,
            owner_scope: HtmlArtifactOwnerScopeView {
                scope_type: "document".to_string(),
                id: "doc-1".to_string(),
            },
            data_refs: vec![HtmlArtifactDataRefView {
                kind: "document".to_string(),
                id: "doc-1".to_string(),
                label: "Video".to_string(),
            }],
            provenance: HtmlArtifactProvenanceView {
                producer: "v3-media-runtime".to_string(),
                reason: "video PPT/transcript extraction evidence summary".to_string(),
                source_run_id: None,
            },
            interaction_mode: HtmlArtifactInteractionModeView::ReadOnly,
            created_at: Utc::now(),
            payload: json!({
                "mediaKind": "video",
                "parseStatus": "transcribed",
                "evidenceStatus": "available"
            }),
        };

        let encoded = serde_json::to_value(&manifest).expect("manifest should serialize");

        assert_eq!(encoded["source_type"], json!("video_extraction"));
        assert_eq!(encoded["template_id"], json!("video_extraction_summary"));
        assert_eq!(encoded["owner_scope"]["type"], json!("document"));

        let decoded: HtmlArtifactManifestView =
            serde_json::from_value(encoded).expect("manifest should deserialize");
        assert_eq!(
            decoded.template_id,
            HtmlArtifactTemplateIdView::VideoExtractionSummary
        );
        assert_eq!(
            decoded.source_type,
            HtmlArtifactSourceTypeView::VideoExtraction
        );
    }

    #[test]
    fn html_artifact_third_party_handoff_document_uses_safe_wire_shape() {
        let manifest = HtmlArtifactManifestView {
            kind: "html_artifact".to_string(),
            version: 1,
            id: "html-artifact-third-party-handoff-document".to_string(),
            title: "V3 纯第三方模式对接文档".to_string(),
            source_type: HtmlArtifactSourceTypeView::ExternalIntegration,
            template_id: HtmlArtifactTemplateIdView::ThirdPartyHandoffDocument,
            owner_scope: HtmlArtifactOwnerScopeView {
                scope_type: "external_integration_handoff".to_string(),
                id: "pure-third-party".to_string(),
            },
            data_refs: vec![HtmlArtifactDataRefView {
                kind: "source_document".to_string(),
                id: "docs/pure-third-party-integration-guide.zh-CN.md".to_string(),
                label: "纯第三方模式 Markdown 源文档".to_string(),
            }],
            provenance: HtmlArtifactProvenanceView {
                producer: "v3-handoff-package-builder".to_string(),
                reason: "third-party handoff review artifact".to_string(),
                source_run_id: Some("abc1234".to_string()),
            },
            interaction_mode: HtmlArtifactInteractionModeView::ReadOnly,
            created_at: Utc::now(),
            payload: json!({
                "handoff": {
                    "status": "review_ready",
                    "defaultDomain": "v3.elepcloud.com"
                }
            }),
        };

        let encoded = serde_json::to_value(&manifest).expect("manifest should serialize");

        assert_eq!(encoded["source_type"], json!("external_integration"));
        assert_eq!(
            encoded["template_id"],
            json!("third_party_handoff_document")
        );
        assert_eq!(
            encoded["owner_scope"]["type"],
            json!("external_integration_handoff")
        );

        let decoded: HtmlArtifactManifestView =
            serde_json::from_value(encoded).expect("manifest should deserialize");
        assert_eq!(
            decoded.template_id,
            HtmlArtifactTemplateIdView::ThirdPartyHandoffDocument
        );
        assert_eq!(
            decoded.source_type,
            HtmlArtifactSourceTypeView::ExternalIntegration
        );
    }

    #[test]
    fn html_artifact_event_request_uses_safe_wire_shape() {
        let request: SubmitHtmlArtifactEventRequest = serde_json::from_value(json!({
            "assistant_run_id": "run-1",
            "event_type": "html_artifact.action_intent",
            "payload": {
                "action": "submit",
                "intent": "apply suggested change"
            }
        }))
        .expect("html artifact event request should deserialize");

        assert_eq!(request.assistant_run_id.as_deref(), Some("run-1"));
        assert_eq!(request.event_type, "html_artifact.action_intent");
        assert_eq!(request.payload["action"], json!("submit"));
        assert!(request.local_thread_id.is_none());
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
