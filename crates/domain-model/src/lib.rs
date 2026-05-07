use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, fmt};
use uuid::Uuid;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(
            Clone,
            Copy,
            Debug,
            Default,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Hash,
            Serialize,
            Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl From<Uuid> for $name {
            fn from(value: Uuid) -> Self {
                Self(value)
            }
        }
    };
}

id_type!(TenantId);
id_type!(UserId);
id_type!(DatasetId);
id_type!(DocumentId);
id_type!(DocumentChunkId);
id_type!(SecretBindingId);
id_type!(SecretGrantId);
id_type!(EmailVerificationChallengeId);
id_type!(UserSessionId);
id_type!(AuthAuditEventId);
id_type!(WorkflowExecutionId);
id_type!(WorkflowEventId);
id_type!(WorkflowTaskId);
id_type!(ReportPlanId);
id_type!(ReportPlanAstVersionId);
id_type!(ReportRenderOutputId);
id_type!(MemoryDirectoryId);
id_type!(DatasetOutputId);
id_type!(ChatSessionId);
id_type!(ChatMessageId);
id_type!(ReportModuleId);
id_type!(PublishedReportId);
id_type!(PublishedReportVersionId);
id_type!(RetrievalEvidenceId);
id_type!(LlmInvocationId);
id_type!(ToolExecutionId);
id_type!(AssistantRunId);
id_type!(AssistantRunEventId);
id_type!(ConversationMemoryItemId);
id_type!(StaticPageDraftId);
id_type!(StaticPageImageJobId);
id_type!(StaticPageRenderOutputId);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum DatasetLifecycle {
    Draft,
    Active,
    Archived,
}

impl DatasetLifecycle {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Archived => "archived",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "draft" => Some(Self::Draft),
            "active" => Some(Self::Active),
            "archived" => Some(Self::Archived),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DatasetVisibility {
    Public,
    Private,
}

impl DatasetVisibility {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "public" => Some(Self::Public),
            "private" => Some(Self::Private),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum DocumentLifecycle {
    Received,
    Extracted,
    Indexed,
    Failed,
}

impl DocumentLifecycle {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Received => "received",
            Self::Extracted => "extracted",
            Self::Indexed => "indexed",
            Self::Failed => "failed",
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum DocumentChunkState {
    Extracted,
    Indexed,
}

impl DocumentChunkState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Extracted => "extracted",
            Self::Indexed => "indexed",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "extracted" => Some(Self::Extracted),
            "indexed" => Some(Self::Indexed),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum SecretScopeLevel {
    Dataset,
    Document,
}

impl SecretScopeLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Dataset => "dataset",
            Self::Document => "document",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "dataset" => Some(Self::Dataset),
            "document" => Some(Self::Document),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum SecretGrantState {
    Locked,
    Unlocked,
    Revoked,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthChallengePurpose {
    AccountCreate,
    Login,
    RecoverKey,
    BindEmail,
    RotateKey,
}

impl AuthChallengePurpose {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AccountCreate => "account_create",
            Self::Login => "login",
            Self::RecoverKey => "recover_key",
            Self::BindEmail => "bind_email",
            Self::RotateKey => "rotate_key",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "account_create" => Some(Self::AccountCreate),
            "login" => Some(Self::Login),
            "recover_key" => Some(Self::RecoverKey),
            "bind_email" => Some(Self::BindEmail),
            "rotate_key" => Some(Self::RotateKey),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthSessionMethod {
    EmailCode,
    EmailKey,
    LocalKey,
}

impl AuthSessionMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::EmailCode => "email_code",
            Self::EmailKey => "email_key",
            Self::LocalKey => "local_key",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "email_code" => Some(Self::EmailCode),
            "email_key" => Some(Self::EmailKey),
            "local_key" => Some(Self::LocalKey),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthAuditOutcome {
    Succeeded,
    Failed,
}

impl AuthAuditOutcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "succeeded" => Some(Self::Succeeded),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChatMessageRole {
    User,
    Assistant,
    Tool,
}

impl ChatMessageRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "user" => Some(Self::User),
            "assistant" => Some(Self::Assistant),
            "tool" => Some(Self::Tool),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ToolExecutionSourceKind {
    DatasetOutput,
    ChatMessage,
    WorkflowExecution,
}

impl ToolExecutionSourceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DatasetOutput => "dataset_output",
            Self::ChatMessage => "chat_message",
            Self::WorkflowExecution => "workflow_execution",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "dataset_output" => Some(Self::DatasetOutput),
            "chat_message" => Some(Self::ChatMessage),
            "workflow_execution" => Some(Self::WorkflowExecution),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ToolExecutionStatus {
    Requested,
    Completed,
    Failed,
}

impl ToolExecutionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Requested => "requested",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "requested" => Some(Self::Requested),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum LlmInvocationSourceKind {
    DatasetOutput,
    ChatMessage,
    WorkflowExecution,
}

impl LlmInvocationSourceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DatasetOutput => "dataset_output",
            Self::ChatMessage => "chat_message",
            Self::WorkflowExecution => "workflow_execution",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "dataset_output" => Some(Self::DatasetOutput),
            "chat_message" => Some(Self::ChatMessage),
            "workflow_execution" => Some(Self::WorkflowExecution),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum LlmInvocationMode {
    Placeholder,
    Provider,
}

impl LlmInvocationMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Placeholder => "placeholder",
            Self::Provider => "provider",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "placeholder" => Some(Self::Placeholder),
            "provider" => Some(Self::Provider),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum LlmInvocationFinishReason {
    Stop,
    ToolCalls,
    Length,
    ContentFilter,
    Error,
    Other(String),
}

impl LlmInvocationFinishReason {
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

    pub fn from_str(value: &str) -> Self {
        match value {
            "stop" => Self::Stop,
            "tool_calls" => Self::ToolCalls,
            "length" => Self::Length,
            "content_filter" => Self::ContentFilter,
            "error" => Self::Error,
            _ => Self::Other(value.to_string()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LlmTokenUsage {
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub total_tokens: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum WorkflowKind {
    ChatSession,
    DatasetOutput,
    MemoryDirectory,
    UploadIngest,
    ReportPlan,
    ReportRender,
    StaticPageImageGeneration,
    StaticPageRender,
    CodexHostTask,
}

impl WorkflowKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ChatSession => "chat_session_workflow",
            Self::DatasetOutput => "dataset_output_workflow",
            Self::MemoryDirectory => "memory_directory_workflow",
            Self::UploadIngest => "upload_ingest_workflow",
            Self::ReportPlan => "report_plan_workflow",
            Self::ReportRender => "report_render_workflow",
            Self::StaticPageImageGeneration => "static_page_image_generation_workflow",
            Self::StaticPageRender => "static_page_render_workflow",
            Self::CodexHostTask => "codex_host_task_workflow",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "chat_session_workflow" => Some(Self::ChatSession),
            "dataset_output_workflow" => Some(Self::DatasetOutput),
            "memory_directory_workflow" => Some(Self::MemoryDirectory),
            "upload_ingest_workflow" => Some(Self::UploadIngest),
            "report_plan_workflow" => Some(Self::ReportPlan),
            "report_render_workflow" => Some(Self::ReportRender),
            "static_page_image_generation_workflow" => Some(Self::StaticPageImageGeneration),
            "static_page_render_workflow" => Some(Self::StaticPageRender),
            "codex_host_task_workflow" => Some(Self::CodexHostTask),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum WorkflowStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    DeadLettered,
}

impl WorkflowStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::DeadLettered => "dead_lettered",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "running" => Some(Self::Running),
            "succeeded" => Some(Self::Succeeded),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            "dead_lettered" => Some(Self::DeadLettered),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum WorkflowTaskStatus {
    Queued,
    Claimed,
    Succeeded,
    Failed,
    Cancelled,
    DeadLettered,
}

impl WorkflowTaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Claimed => "claimed",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::DeadLettered => "dead_lettered",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "queued" => Some(Self::Queued),
            "claimed" => Some(Self::Claimed),
            "succeeded" => Some(Self::Succeeded),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            "dead_lettered" => Some(Self::DeadLettered),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReportPlanStatus {
    Draft,
    Planned,
    Rendered,
    Published,
}

impl ReportPlanStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Planned => "planned",
            Self::Rendered => "rendered",
            Self::Published => "published",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "draft" => Some(Self::Draft),
            "planned" => Some(Self::Planned),
            "rendered" => Some(Self::Rendered),
            "published" => Some(Self::Published),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReportModuleKind {
    Hero,
    NarrativeText,
    EvidenceList,
    MetricCard,
    Chart,
    Table,
    Timeline,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PublishedSurface {
    Pc,
    Mobile,
}

impl PublishedSurface {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pc => "pc",
            Self::Mobile => "mobile",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "pc" => Some(Self::Pc),
            "mobile" => Some(Self::Mobile),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AssistantRun {
    pub id: AssistantRunId,
    pub tenant_id: TenantId,
    pub user_id: Option<UserId>,
    pub local_thread_id: Option<String>,
    pub user_prompt: String,
    pub startup_briefing: Value,
    pub selected_scope: Value,
    pub scope_candidates: Value,
    pub context_policy: Value,
    pub evidence_state: Value,
    pub service_lane: String,
    pub execution_trail: Value,
    pub output_artifacts: Value,
    pub runtime_manifest: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AssistantRunEvent {
    pub id: AssistantRunEventId,
    pub tenant_id: TenantId,
    pub run_id: AssistantRunId,
    pub sequence_no: i32,
    pub event_name: String,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ConversationMemoryItem {
    pub id: ConversationMemoryItemId,
    pub tenant_id: TenantId,
    pub user_id: Option<UserId>,
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StaticPageDraftStatus {
    Draft,
    Planned,
    Queued,
    Previewed,
    Confirmed,
    Rendered,
    Archived,
}

impl StaticPageDraftStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Planned => "planned",
            Self::Queued => "queued",
            Self::Previewed => "previewed",
            Self::Confirmed => "confirmed",
            Self::Rendered => "rendered",
            Self::Archived => "archived",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "draft" => Some(Self::Draft),
            "planned" => Some(Self::Planned),
            "queued" => Some(Self::Queued),
            "previewed" => Some(Self::Previewed),
            "confirmed" => Some(Self::Confirmed),
            "rendered" => Some(Self::Rendered),
            "archived" => Some(Self::Archived),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct StaticPageDraft {
    pub id: StaticPageDraftId,
    pub tenant_id: TenantId,
    pub assistant_run_id: AssistantRunId,
    pub owner_user_id: Option<UserId>,
    pub title: String,
    pub status: StaticPageDraftStatus,
    pub selected_scope: Value,
    pub visibility_snapshot: Value,
    pub source_refs: Value,
    pub draft_payload: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StaticPageImageJobStatus {
    Queued,
    Running,
    PreviewReady,
    Failed,
    Confirmed,
}

impl StaticPageImageJobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::PreviewReady => "preview_ready",
            Self::Failed => "failed",
            Self::Confirmed => "confirmed",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "queued" => Some(Self::Queued),
            "running" => Some(Self::Running),
            "preview_ready" => Some(Self::PreviewReady),
            "failed" => Some(Self::Failed),
            "confirmed" => Some(Self::Confirmed),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct StaticPageImageJob {
    pub id: StaticPageImageJobId,
    pub tenant_id: TenantId,
    pub draft_id: StaticPageDraftId,
    pub assistant_run_id: AssistantRunId,
    pub status: StaticPageImageJobStatus,
    pub queue_position: Option<i32>,
    pub image_prompt_payload: Value,
    pub preview_asset_key: Option<String>,
    pub failure_reason: Option<String>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StaticPageRenderOutputStatus {
    Queued,
    Rendering,
    Rendered,
    Failed,
    Cancelled,
}

impl StaticPageRenderOutputStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Rendering => "rendering",
            Self::Rendered => "rendered",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "queued" => Some(Self::Queued),
            "rendering" => Some(Self::Rendering),
            "rendered" => Some(Self::Rendered),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct StaticPageRenderOutput {
    pub id: StaticPageRenderOutputId,
    pub tenant_id: TenantId,
    pub draft_id: StaticPageDraftId,
    pub assistant_run_id: AssistantRunId,
    pub owner_user_id: Option<UserId>,
    pub image_job_id: Option<StaticPageImageJobId>,
    pub status: StaticPageRenderOutputStatus,
    pub html: String,
    pub asset_manifest: Value,
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::{
        AuthAuditOutcome, AuthChallengePurpose, AuthSessionMethod, DocumentLifecycle,
        LlmInvocationFinishReason, LlmInvocationMode, LlmInvocationSourceKind, PublishedSurface,
        ToolExecutionSourceKind,
    };

    #[test]
    fn published_surface_serde_uses_lowercase_wire_format() {
        assert_eq!(
            serde_json::to_string(&PublishedSurface::Pc).expect("surface serializes"),
            "\"pc\""
        );
        assert_eq!(
            serde_json::from_str::<PublishedSurface>("\"mobile\"").expect("surface deserializes"),
            PublishedSurface::Mobile
        );
    }

    #[test]
    fn document_lifecycle_roundtrips_through_stable_strings() {
        assert_eq!(DocumentLifecycle::Indexed.as_str(), "indexed");
        assert_eq!(
            DocumentLifecycle::from_str("received").expect("document lifecycle parses"),
            DocumentLifecycle::Received
        );
    }

    #[test]
    fn tool_execution_source_kind_roundtrips_through_stable_strings() {
        assert_eq!(
            ToolExecutionSourceKind::DatasetOutput.as_str(),
            "dataset_output"
        );
        assert_eq!(
            ToolExecutionSourceKind::from_str("chat_message")
                .expect("tool execution source kind parses"),
            ToolExecutionSourceKind::ChatMessage
        );
        assert_eq!(
            ToolExecutionSourceKind::from_str("workflow_execution")
                .expect("workflow execution source kind parses"),
            ToolExecutionSourceKind::WorkflowExecution
        );
    }

    #[test]
    fn llm_invocation_source_kind_roundtrips_through_stable_strings() {
        assert_eq!(
            LlmInvocationSourceKind::DatasetOutput.as_str(),
            "dataset_output"
        );
        assert_eq!(
            LlmInvocationSourceKind::from_str("chat_message")
                .expect("llm invocation source kind parses"),
            LlmInvocationSourceKind::ChatMessage
        );
        assert_eq!(
            LlmInvocationSourceKind::from_str("workflow_execution")
                .expect("workflow execution llm source kind parses"),
            LlmInvocationSourceKind::WorkflowExecution
        );
    }

    #[test]
    fn llm_invocation_mode_roundtrips_through_stable_strings() {
        assert_eq!(LlmInvocationMode::Placeholder.as_str(), "placeholder");
        assert_eq!(
            LlmInvocationMode::from_str("provider").expect("llm invocation mode parses"),
            LlmInvocationMode::Provider
        );
    }

    #[test]
    fn llm_invocation_finish_reason_preserves_unknown_values() {
        assert_eq!(
            LlmInvocationFinishReason::from_str("tool_calls"),
            LlmInvocationFinishReason::ToolCalls
        );
        assert_eq!(
            LlmInvocationFinishReason::from_str("custom_reason"),
            LlmInvocationFinishReason::Other("custom_reason".to_string())
        );
    }

    #[test]
    fn auth_account_enums_roundtrip_through_snake_case_wire_values() {
        assert_eq!(AuthChallengePurpose::RecoverKey.as_str(), "recover_key");
        assert_eq!(
            AuthChallengePurpose::from_str("recover_key"),
            Some(AuthChallengePurpose::RecoverKey)
        );
        assert_eq!(
            serde_json::to_string(&AuthChallengePurpose::BindEmail)
                .expect("auth challenge purpose serializes"),
            "\"bind_email\""
        );
        assert_eq!(AuthSessionMethod::EmailCode.as_str(), "email_code");
        assert_eq!(
            AuthSessionMethod::from_str("local_key"),
            Some(AuthSessionMethod::LocalKey)
        );
        assert_eq!(
            serde_json::from_str::<AuthSessionMethod>("\"email_key\"")
                .expect("auth session method deserializes"),
            AuthSessionMethod::EmailKey
        );
        assert_eq!(AuthAuditOutcome::Succeeded.as_str(), "succeeded");
        assert_eq!(
            AuthAuditOutcome::from_str("failed"),
            Some(AuthAuditOutcome::Failed)
        );
        assert_eq!(
            serde_json::to_string(&AuthAuditOutcome::Failed)
                .expect("auth audit outcome serializes"),
            "\"failed\""
        );
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReportRenderOutputStatus {
    Rendered,
    Failed,
}

impl ReportRenderOutputStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Rendered => "rendered",
            Self::Failed => "failed",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "rendered" => Some(Self::Rendered),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tenant {
    pub id: TenantId,
    pub key: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
    pub id: UserId,
    pub tenant_id: TenantId,
    pub email: String,
    pub display_name: String,
    pub roles: Vec<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dataset {
    pub id: DatasetId,
    pub tenant_id: TenantId,
    pub owner_user_id: Option<UserId>,
    pub key: String,
    pub title: String,
    pub description: Option<String>,
    pub lifecycle: DatasetLifecycle,
    pub visibility: DatasetVisibility,
    pub default_secret_binding_ids: Vec<SecretBindingId>,
    pub metadata: BTreeMap<String, Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Document {
    pub id: DocumentId,
    pub tenant_id: TenantId,
    pub dataset_id: DatasetId,
    pub owner_user_id: Option<UserId>,
    pub title: String,
    pub object_key: String,
    pub content_type: String,
    pub lifecycle: DocumentLifecycle,
    pub secret_binding_ids: Vec<SecretBindingId>,
    pub metadata: BTreeMap<String, Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentChunk {
    pub id: DocumentChunkId,
    pub tenant_id: TenantId,
    pub dataset_id: DatasetId,
    pub document_id: DocumentId,
    pub chunk_index: i32,
    pub content: String,
    pub token_count: i32,
    pub state: DocumentChunkState,
    pub metadata: BTreeMap<String, Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SecretBinding {
    pub id: SecretBindingId,
    pub tenant_id: TenantId,
    pub dataset_id: DatasetId,
    pub document_id: Option<DocumentId>,
    pub scope_level: SecretScopeLevel,
    pub provider_key: String,
    pub cipher_text: String,
    pub fingerprint: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SecretGrant {
    pub id: SecretGrantId,
    pub secret_binding_id: SecretBindingId,
    pub tenant_id: TenantId,
    pub user_id: UserId,
    pub device_fingerprint: String,
    pub state: SecretGrantState,
    pub granted_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmailVerificationChallenge {
    pub id: EmailVerificationChallengeId,
    pub tenant_id: TenantId,
    pub email_normalized: String,
    pub purpose: AuthChallengePurpose,
    pub code_hash: String,
    pub attempt_count: i32,
    pub max_attempts: i32,
    pub expires_at: DateTime<Utc>,
    pub consumed_at: Option<DateTime<Utc>>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserSession {
    pub id: UserSessionId,
    pub tenant_id: TenantId,
    pub user_id: UserId,
    pub device_fingerprint: String,
    pub session_token_hash: String,
    pub auth_method: AuthSessionMethod,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthAuditEvent {
    pub id: AuthAuditEventId,
    pub tenant_id: TenantId,
    pub user_id: Option<UserId>,
    pub session_id: Option<UserSessionId>,
    pub email_normalized: Option<String>,
    pub event_name: String,
    pub outcome: AuthAuditOutcome,
    pub ip_hash: Option<String>,
    pub device_fingerprint: Option<String>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowDefinitionRecord {
    pub kind: WorkflowKind,
    pub version: String,
    pub summary: String,
    pub is_active: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowExecution {
    pub id: WorkflowExecutionId,
    pub tenant_id: TenantId,
    pub dataset_id: Option<DatasetId>,
    pub report_plan_id: Option<ReportPlanId>,
    pub kind: WorkflowKind,
    pub version: String,
    pub stage: String,
    pub status: WorkflowStatus,
    pub attempt: u32,
    pub context: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowEventRecord {
    pub id: WorkflowEventId,
    pub execution_id: WorkflowExecutionId,
    pub sequence_no: i64,
    pub event_name: String,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowTask {
    pub id: WorkflowTaskId,
    pub tenant_id: TenantId,
    pub execution_id: WorkflowExecutionId,
    pub queue: String,
    pub task_key: String,
    pub payload: Value,
    pub status: WorkflowTaskStatus,
    pub attempt: u32,
    pub max_attempts: u32,
    pub available_at: DateTime<Utc>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReportPlan {
    pub id: ReportPlanId,
    pub tenant_id: TenantId,
    pub dataset_id: DatasetId,
    pub owner_user_id: Option<UserId>,
    pub title: String,
    pub objective: String,
    pub status: ReportPlanStatus,
    pub theme_key: String,
    pub current_ast_version_id: Option<ReportPlanAstVersionId>,
    pub modules: Vec<ReportModule>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReportPlanAstVersion {
    pub id: ReportPlanAstVersionId,
    pub plan_id: ReportPlanId,
    pub version_no: i32,
    pub ast: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReportRenderOutput {
    pub id: ReportRenderOutputId,
    pub tenant_id: TenantId,
    pub execution_id: WorkflowExecutionId,
    pub plan_id: ReportPlanId,
    pub dataset_id: DatasetId,
    pub ast_version_id: ReportPlanAstVersionId,
    pub surface: PublishedSurface,
    pub status: ReportRenderOutputStatus,
    pub asset_manifest: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryDirectory {
    pub id: MemoryDirectoryId,
    pub tenant_id: TenantId,
    pub dataset_id: DatasetId,
    pub execution_id: WorkflowExecutionId,
    pub version_no: i32,
    pub directory_nodes: i32,
    pub refreshed_chunks: i32,
    pub directory_manifest: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RetrievalEvidence {
    pub id: RetrievalEvidenceId,
    pub tenant_id: TenantId,
    pub dataset_id: DatasetId,
    pub execution_id: WorkflowExecutionId,
    pub document_id: DocumentId,
    pub document_chunk_id: DocumentChunkId,
    pub chunk_index: i32,
    pub source_locator: String,
    pub content_excerpt: String,
    pub summary: String,
    pub payload_filter_key: String,
    pub embedding_model: String,
    pub recall_score: f64,
    pub evidence_manifest: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DatasetOutput {
    pub id: DatasetOutputId,
    pub tenant_id: TenantId,
    pub execution_id: WorkflowExecutionId,
    pub dataset_id: DatasetId,
    pub prompt: String,
    pub output_text: String,
    pub memory_directory_id: Option<MemoryDirectoryId>,
    pub retrieval_evidence_ids: Vec<RetrievalEvidenceId>,
    pub output_manifest: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatSession {
    pub id: ChatSessionId,
    pub tenant_id: TenantId,
    pub dataset_id: DatasetId,
    pub execution_id: WorkflowExecutionId,
    pub title: String,
    pub latest_memory_directory_id: Option<MemoryDirectoryId>,
    pub latest_dataset_output_id: Option<DatasetOutputId>,
    pub session_manifest: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: ChatMessageId,
    pub tenant_id: TenantId,
    pub session_id: ChatSessionId,
    pub role: ChatMessageRole,
    pub turn_index: i32,
    pub content: String,
    pub message_manifest: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LlmInvocation {
    pub id: LlmInvocationId,
    pub tenant_id: TenantId,
    pub execution_id: WorkflowExecutionId,
    pub source_kind: LlmInvocationSourceKind,
    pub dataset_output_id: Option<DatasetOutputId>,
    pub chat_message_id: Option<ChatMessageId>,
    pub sequence_no: i32,
    pub mode: LlmInvocationMode,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub request_id: Option<String>,
    pub finish_reason: Option<LlmInvocationFinishReason>,
    pub latency_ms: Option<u64>,
    pub usage: Option<LlmTokenUsage>,
    pub system_prompt_key: Option<String>,
    pub system_prompt_version: Option<String>,
    pub tool_trace_count: Option<usize>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolExecution {
    pub id: ToolExecutionId,
    pub tenant_id: TenantId,
    pub execution_id: WorkflowExecutionId,
    pub source_kind: ToolExecutionSourceKind,
    pub dataset_output_id: Option<DatasetOutputId>,
    pub chat_message_id: Option<ChatMessageId>,
    pub sequence_no: i32,
    pub call_id: Option<String>,
    pub tool_name: String,
    pub tool_snapshot: Option<Value>,
    pub status: ToolExecutionStatus,
    pub arguments: Option<Value>,
    pub result: Option<Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReportModule {
    pub id: ReportModuleId,
    pub plan_id: ReportPlanId,
    pub sort_order: i32,
    pub kind: ReportModuleKind,
    pub title: String,
    pub expected_copy: Option<String>,
    pub chart_intent: Option<String>,
    pub data_binding_slot: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublishedReport {
    pub id: PublishedReportId,
    pub tenant_id: TenantId,
    pub dataset_id: DatasetId,
    pub plan_id: ReportPlanId,
    pub slug: String,
    pub current_version_id: Option<PublishedReportVersionId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublishedReportVersion {
    pub id: PublishedReportVersionId,
    pub report_id: PublishedReportId,
    pub version_no: i32,
    pub surface: PublishedSurface,
    pub asset_manifest: Value,
    pub created_at: DateTime<Utc>,
}
